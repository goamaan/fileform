// SPDX-License-Identifier: Apache-2.0
import {promises as fs,createReadStream,type ReadStream} from 'node:fs';
import {join} from 'node:path';
import {createHash,randomUUID} from 'node:crypto';
import {Readable} from 'node:stream';
import type {AssetRecord,Worker} from './assets.cjs';
import type {NativeRuntime} from './runtime.cjs';
import type {MediaPreview,MediaPreviewOptions} from '../src/contracts.js';

const MAX_BYTES=2*1024*1024*1024;
const integer=(v:unknown):v is number=>Number.isSafeInteger(v)&&Number(v)>=0;
const hash=(v:unknown):v is string=>typeof v==='string'&&/^[a-f0-9]{64}$/.test(v);
interface Entry {key:string;folder:string;path:string;bytes:number;dev:bigint;ino:bigint;sha256:string;users:number;readers:number;preview:Omit<MediaPreview,'id'|'url'>}

// Leased, generated recordings only: no original path or arbitrary filesystem URL
// crosses the interface. At most eight records / 2 GiB are retained; one bounded
// native preparation can temporarily add up to another 2 GiB before eviction.
export class MediaPreviewCache {
 private root:Promise<string>|undefined;
 private entries=new Map<string,Entry>();
 private leases=new Map<string,Entry>();
 private streams=new Set<ReadStream>();
 private closed=false;
 constructor(private temp:string){}
 private folder(){return this.root??=fs.mkdtemp(join(this.temp,'fileform-playback-'));}
 private async evict(needed:number){
  let bytes=[...this.entries.values()].reduce((n,v)=>n+v.bytes,0);
  for(const [key,entry] of this.entries){
   if(bytes+needed<=MAX_BYTES&&this.entries.size<8)break;
   if(entry.users||entry.readers)continue;
   this.entries.delete(key);bytes-=entry.bytes;await fs.rm(entry.folder,{recursive:true,force:true});
  }
  if(bytes+needed>MAX_BYTES||this.entries.size>=8)throw new Error('Close another preview before preparing this recording.');
 }
 private lease(entry:Entry):MediaPreview {
  if(this.leases.size>=64)throw new Error('Close another preview first.');
  const id=randomUUID();entry.users++;this.leases.set(id,entry);
  return {...entry.preview,id,url:'fileform://app/preview/'+id};
 }
 async prepare(record:AssetRecord,raw:unknown,worker:Worker,runtime:NativeRuntime,signal:AbortSignal):Promise<MediaPreview>{
  signal.throwIfAborted();if(this.closed)throw new Error('The preview window closed.');
  if(!raw||typeof raw!=='object'||Array.isArray(raw))throw new Error('Invalid preview options.');
  const options=raw as MediaPreviewOptions;
  if(typeof options.audioOnly!=='boolean'||options.muteAudio!==undefined&&typeof options.muteAudio!=='boolean'||!['audio','video'].includes(record.asset.family))throw new Error('Choose an audio or video file.');
  if(options.audioStream!==undefined&&!record.asset.audioTracks?.some(v=>v.index===options.audioStream))throw new Error('Choose an existing audio track.');
  if(options.muteAudio&&(options.audioStream!==undefined||options.audioOnly||record.asset.family!=='video'))throw new Error('Choose an audio track or mute the video.');
  const audioOnly=record.asset.family==='audio'||options.audioOnly;
  const key=[record.sha256,audioOnly,options.audioStream??'default',options.muteAudio??false].join(':');
  const cached=this.entries.get(key);if(cached)return this.lease(cached);
  if(this.leases.size>=64)throw new Error('Close another preview first.');
  await this.evict(0);signal.throwIfAborted();
  const folder=await fs.mkdtemp(join(await this.folder(),'recording-'));
  let retained=false;
  try {
   const output=join(folder,audioOnly?'playback.wav':'playback.mp4');
   const receipt=await worker({operation:'media_playback_preview',input:record.path,output,directory:runtime.pack('media'),audio_only:audioOnly,audio_stream:options.audioStream,mute_audio:options.muteAudio??false,max_dimension:audioOnly?undefined:960,expected_source_sha256:record.sha256});
   signal.throwIfAborted();
   const duration=receipt.duration;
   if(receipt.kind!=='media_playback_preview'||receipt.output!==output||receipt.source_sha256!==record.sha256||!hash(receipt.sha256)||!integer(receipt.bytes)||receipt.bytes<1||receipt.bytes>MAX_BYTES||!duration||!integer(duration.ticks)||duration.ticks<1||!integer(duration.timescale)||duration.timescale<1||duration.ticks/duration.timescale>21600)throw new Error('Invalid playback receipt.');
   if(receipt.source_audio_stream_index!==null&&!record.asset.audioTracks?.some(v=>v.index===receipt.source_audio_stream_index)||audioOnly&&receipt.source_audio_stream_index===null||options.audioStream!==undefined&&receipt.source_audio_stream_index!==options.audioStream||options.muteAudio&&receipt.source_audio_stream_index!==null)throw new Error('The preview audio track changed.');
   const metadata=await fs.lstat(output,{bigint:true});if(!metadata.isFile()||metadata.size!==BigInt(receipt.bytes))throw new Error('The playback file changed.');
   const digest=createHash('sha256');for await(const chunk of createReadStream(output,{signal}))digest.update(chunk);
   if(digest.digest('hex')!==receipt.sha256)throw new Error('The playback file could not be verified.');
   const preview:Entry['preview']={kind:audioOnly?'audio':'video',duration};
   if(receipt.source_audio_stream_index!==null){
    const waveform=await worker({operation:'media_waveform',input:record.path,directory:runtime.pack('media'),audio_stream:receipt.source_audio_stream_index,bins:2048});
    signal.throwIfAborted();
    if(waveform.kind!=='media_waveform'||waveform.sha256!==record.sha256||waveform.stream_index!==receipt.source_audio_stream_index||!integer(waveform.sample_rate)||waveform.sample_rate<1||!integer(waveform.sample_count)||waveform.sample_count<1||!integer(waveform.samples_per_bucket)||waveform.samples_per_bucket<1||!Array.isArray(waveform.channels)||!waveform.channels.length||waveform.channels.length>8)throw new Error('Invalid waveform receipt.');
    const channels=waveform.channels.map((channel:any)=>{
     if(!Array.isArray(channel.minimum)||!Array.isArray(channel.maximum)||!channel.minimum.length||channel.minimum.length>2048||channel.maximum.length!==channel.minimum.length||channel.minimum.length!==Math.ceil(waveform.sample_count/waveform.samples_per_bucket)||channel.minimum.some((v:unknown,i:number)=>typeof v!=='number'||!Number.isFinite(v)||Math.abs(v)>1e6||typeof channel.maximum[i]!=='number'||!Number.isFinite(channel.maximum[i])||Math.abs(channel.maximum[i])>1e6||v>channel.maximum[i]))throw new Error('Invalid waveform measurements.');
     return {minimum:channel.minimum,maximum:channel.maximum};
    });
    preview.waveform={sampleRate:waveform.sample_rate,sampleCount:waveform.sample_count,samplesPerBucket:waveform.samples_per_bucket,channels};
   }
   if(!audioOnly){
    const poster=join(folder,'poster.png');const image=await worker({operation:'media_poster',input:record.path,output:poster,directory:runtime.pack('media'),time:{ticks:0,timescale:1},max_dimension:640});
    signal.throwIfAborted();
    if(image.kind!=='media_poster'||image.output!==poster||image.source_sha256!==record.sha256||!hash(image.sha256)||!integer(image.bytes)||image.bytes>2_000_000||image.bytes<8||![image.width,image.height].every(v=>integer(v)&&v>=1&&v<=640))throw new Error('Invalid poster receipt.');
    const bytes=await fs.readFile(poster);if(bytes.length!==image.bytes||!bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))||createHash('sha256').update(bytes).digest('hex')!==image.sha256)throw new Error('The poster changed.');
    preview.poster='data:image/png;base64,'+bytes.toString('base64');await fs.unlink(poster);
   }
   signal.throwIfAborted();await this.evict(Number(metadata.size));signal.throwIfAborted();
   const entry:Entry={key,folder,path:output,bytes:Number(metadata.size),dev:metadata.dev,ino:metadata.ino,sha256:receipt.sha256,users:0,readers:0,preview};
   this.entries.set(key,entry);retained=true;return this.lease(entry);
  } finally {if(!retained)await fs.rm(folder,{recursive:true,force:true});}
 }
 release(id:string){const entry=this.leases.get(id);if(entry){this.leases.delete(id);entry.users--;}}
 async respond(id:string,request:Request):Promise<Response>{
  const entry=this.leases.get(id);if(!entry||this.closed||!['GET','HEAD'].includes(request.method))return new Response(null,{status:404});
  let start=0;let end=entry.bytes-1;let status=200;const range=request.headers.get('Range');
  if(range){
   const match=/^bytes=(\d*)-(\d*)$/.exec(range);
   if(!match||!match[1]&&!match[2])return new Response(null,{status:416,headers:{'Content-Range':'bytes */'+entry.bytes}});
   if(!match[1]){const tail=Number(match[2]);if(!Number.isSafeInteger(tail)||tail<=0)return new Response(null,{status:416});start=Math.max(0,entry.bytes-tail);}
   else {start=Number(match[1]);if(match[2])end=Math.min(end,Number(match[2]));}
   if(!Number.isSafeInteger(start)||!Number.isSafeInteger(end)||start>end||start<0||start>=entry.bytes)return new Response(null,{status:416,headers:{'Content-Range':'bytes */'+entry.bytes}});
   status=206;
  }
  const headers:Record<string,string>={'Content-Type':entry.preview.kind==='video'?'video/mp4':'audio/wav','Content-Length':String(end-start+1),'Accept-Ranges':'bytes','Cache-Control':'no-store','ETag':'"'+entry.sha256+'"'};
  if(status===206)headers['Content-Range']=`bytes ${start}-${end}/${entry.bytes}`;
  if(request.method==='HEAD')return new Response(null,{status,headers});
  const file=await fs.open(entry.path,'r');
  const metadata=await file.stat({bigint:true});if(!metadata.isFile()||metadata.dev!==entry.dev||metadata.ino!==entry.ino||metadata.size!==BigInt(entry.bytes)){await file.close();return new Response(null,{status:410});}
  if(request.signal.aborted){await file.close();return new Response(null,{status:499});}
  const stream=file.createReadStream({start,end,autoClose:true});entry.readers++;this.streams.add(stream);
  const abort=()=>stream.destroy();request.signal.addEventListener('abort',abort,{once:true});
  stream.once('close',()=>{request.signal.removeEventListener('abort',abort);entry.readers--;this.streams.delete(stream);});
  return new Response(Readable.toWeb(stream) as ReadableStream<Uint8Array>,{status,headers});
 }
 async close(){
  this.closed=true;this.leases.clear();
  await Promise.all([...this.streams].map(stream=>new Promise<void>(resolve=>{stream.once('close',resolve);stream.destroy();})));
  this.entries.clear();if(this.root)await fs.rm(await this.root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
 }
}
