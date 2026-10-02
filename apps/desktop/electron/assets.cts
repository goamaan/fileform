import {promises as fs} from 'node:fs';
import {basename,extname} from 'node:path';
import {randomUUID} from 'node:crypto';
import type {AssetSource,AudioTrack,ImageSource,SourceFile,TableOutput} from '../src/contracts.js';
import type {NativeRuntime} from './runtime.cjs';
export interface AssetRecord {asset:AssetSource;path:string;sha256:string}
export type Worker=(request:unknown)=>Promise<any>;
const count=(value:unknown):value is number=>Number.isSafeInteger(value)&&Number(value)>=0;
const hash=(value:unknown):value is string=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
export async function inspectAsset(path:string,worker:Worker,runtime:NativeRuntime):Promise<AssetRecord>{
  path=await fs.realpath(path);const metadata=await fs.stat(path);
  if(!metadata.isFile())throw new Error('Choose a regular file.');
  const ext=extname(path).toLowerCase();const id=randomUUID();const name=basename(path);
  let receipt:any;let asset:AssetSource;
  if(['.csv','.tsv','.json'].includes(ext)){
    receipt=await worker({operation:'inspect',input:path});
    if(receipt.kind!=='inspection'||!count(receipt.rows)||!count(receipt.columns)||!Array.isArray(receipt.outputs)||!receipt.outputs.every((v:unknown)=>v==='json'||v==='csv'||v==='tsv'))throw new Error('Invalid table inspection.');
    const table:SourceFile={id,name,bytes:receipt.bytes,rows:receipt.rows,columns:receipt.columns,outputs:receipt.outputs as TableOutput[],scalarTypesBecomeText:ext==='.json'};
    asset={id,name,bytes:receipt.bytes,family:'table',table};
  }else if(['.png','.jpg','.jpeg','.tif','.tiff','.heic','.heif'].includes(ext)){
    receipt=await worker({operation:'inspect_image',input:path,preview:true});
    if(receipt.kind!=='image_inspection'||!count(receipt.display_width)||!count(receipt.display_height)||receipt.display_width<1||receipt.display_height<1||receipt.display_width*receipt.display_height>80_000_000||typeof receipt.has_alpha!=='boolean'||typeof receipt.conversion_available!=='boolean')throw new Error('Invalid image inspection.');
    const p=receipt.preview;
    if(p!==null&&(!p||!count(p.width)||!count(p.height)||p.width<1||p.height<1||p.width>128||p.height>128||!Array.isArray(p.rgba)||p.rgba.length!==p.width*p.height*4||!p.rgba.every((v:unknown)=>count(v)&&v<=255)))throw new Error('Invalid image preview.');
    const image:ImageSource={id,name,bytes:receipt.bytes,width:receipt.display_width,height:receipt.display_height,hasAlpha:receipt.has_alpha,canConvert:receipt.conversion_available,preview:p};
    asset={id,name,bytes:receipt.bytes,family:'image',image};
  }else if(ext==='.pdf'){
    receipt=await worker({operation:'inspect_pdf',input:path,directory:runtime.pack('pdf')});
    if(receipt.kind!=='pdf_inspection'||!count(receipt.pages)||receipt.pages<1||receipt.pages>1000)throw new Error('Invalid PDF inspection.');
    asset={id,name,bytes:receipt.bytes,family:'pdf',pages:receipt.pages};
  }else{
    receipt=await worker({operation:'inspect_media',input:path,directory:runtime.pack('media')});
    if(receipt.kind!=='media_inspection'||!count(receipt.video_tracks)||!count(receipt.audio_tracks)||!Array.isArray(receipt.streams)||receipt.streams.length>64||!Number.isFinite(receipt.duration_seconds)||receipt.duration_seconds<=0||receipt.duration_seconds>21600)throw new Error('Invalid media inspection.');
    const tracks:AudioTrack[]=receipt.streams.filter((v:any)=>v.codec_type==='audio').map((v:any)=>{
      if(!count(v.index)||!count(v.channels)||v.channels<1||!Number.isSafeInteger(Number(v.sample_rate)))throw new Error('Invalid audio track.');
      return {index:v.index,codec:typeof v.codec_name==='string'?v.codec_name:'Audio',channels:v.channels,sampleRate:Number(v.sample_rate),language:typeof v.tags?.language==='string'?v.tags.language.slice(0,32):undefined};
    });
    asset={id,name,bytes:receipt.bytes,family:receipt.video_tracks>0?'video':'audio',duration:receipt.duration_seconds,audioTracks:tracks};
  }
  if(!count(receipt.bytes)||!hash(receipt.sha256))throw new Error('Invalid file identity receipt.');
  return {asset,path,sha256:receipt.sha256};
}
