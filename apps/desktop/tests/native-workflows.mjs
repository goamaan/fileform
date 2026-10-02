// SPDX-License-Identifier: Apache-2.0
// Real tool/worker integration through the same desktop inspector/task planner.
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {promises as fs} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url);
const {inspectAsset}=require('../dist-main/electron/assets.cjs');
const {planTask}=require('../dist-main/electron/task-request.cjs');
const {loadRuntime}=require('../dist-main/electron/runtime.cjs');
const {previewPages}=require('../dist-main/electron/page-preview.cjs');
const {availableTasksForAssets}=require('../dist-main/src/tasks.js');
const {MediaPreviewCache}=require('../dist-main/electron/media-preview.cjs');
const root=resolve(import.meta.dirname,'../../..');
const runtime=await loadRuntime(resolve(process.env.FILEFORM_RUNTIME_ROOT??join(root,'Artifacts/DesktopRuntime')));
const executable=resolve(process.env.FILEFORM_WORKER??runtime.worker);
function worker(request,ok=true){return new Promise((resolve,reject)=>{
 const child=spawn(executable,[],{stdio:['pipe','pipe','pipe'],env:{...process.env,FILEFORM_HEIC_PACK:runtime.pack('heic')}});let out='';let error='';
 const timer=setTimeout(()=>{child.kill();reject(new Error('Worker timeout'));},180000);
 child.stdout.on('data',v=>out+=v);child.stderr.on('data',v=>error+=v);child.on('error',reject);
 child.on('close',code=>{clearTimeout(timer);try{const response=JSON.parse(out);assert.equal(response.ok,ok,JSON.stringify(response));resolve(ok?response.result:response.error);}catch(e){reject(e);}});
 child.stdin.end(JSON.stringify(request)+'\n');
});}
const folder=await fs.mkdtemp(join(tmpdir(),'fileform-desktop-flows-'));
try{
 const table=join(folder,'table.csv');await fs.writeFile(table,'name,count\nFileform,4827\n');
 // Source-owned PDF, no external documents.
 const content='BT /F1 12 Tf 10 40 Td (Fileform document) Tj ET\n';
 const bodies=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Count 1 /Kids [3 0 R] >>','<< /Type /Page /Parent 2 0 R /MediaBox [0 0 180 72] /Resources << /Font << /F1 6 0 R >> /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>',`<< /Length ${Buffer.byteLength(content)} >>\nstream\n${content}endstream`,'<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 6 >>\nstream\nabcdef\nendstream','<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];
 let pdf='%PDF-1.4\n';const offsets=[0];for(let i=0;i<bodies.length;i++){offsets.push(Buffer.byteLength(pdf));pdf+=`${i+1} 0 obj\n${bodies[i]}\nendobj\n`;}
 const xref=Buffer.byteLength(pdf);pdf+=`xref\n0 ${bodies.length+1}\n0000000000 65535 f \n`;for(const offset of offsets.slice(1))pdf+=String(offset).padStart(10,'0')+' 00000 n \n';pdf+=`trailer\n<< /Size ${bodies.length+1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
 const document=join(folder,'document.pdf');await fs.writeFile(document,pdf);
 const image=join(root,'native/heic-decode/fixtures/srgb.heic');
 const video=join(root,'crates/fileform-engine/tests/fixtures/h264-aac.mp4');
 const records={};for(const [name,path] of Object.entries({table,image,pdf:document,video})){records[name]=await inspectAsset(path,worker,runtime);assert.equal(records[name].asset.family,name==='pdf'?'pdf':name);}
 const audio=join(folder,'audio.wav');await worker({operation:'convert_audio',input:video,output:audio,directory:runtime.pack('media')});records.audio=await inspectAsset(audio,worker,runtime);assert.equal(records.audio.asset.family,'audio');
 const cases=[
  ['table.convert',['table'],{format:'json'},'table.json'],
  ['image.convert',['image'],{format:'png'},'image.png'],
  ['pdf.combine',['pdf','image'],{format:'pdf'},'combined.pdf'],
  ['pdf.split',['pdf'],{format:'pdf',splitEvery:1},'split'],
  ['pdf.images',['pdf'],{format:'png',dpi:72},'pages'],
  ['audio.convert',['audio'],{format:'mp3'},'audio.mp3'],
  ['audio.trim',['audio'],{format:'wav',start:'.25',end:'1.001'},'trim.wav'],
  ['video.convert',['video'],{format:'mp4',maxDimension:32},'video.mp4'],
  ['video.trim',['video'],{format:'mp4',start:'1.3',end:'1.7',fast:true},'trim.mp4'],
 ];
 for(const [task,ids,options,name] of cases){const output=join(folder,name);const plan=planTask(ids.map(id=>records[id]),task,options,output,runtime);const result=await worker(plan.request);assert.equal(result.output,output);const stat=await fs.stat(output);assert.equal(stat.isDirectory(),plan.folder);console.log(task,'real desktop module flow passed');}
 const silentPath=join(folder,'silent.mp4');await worker(planTask([records.video],'video.trim',{format:'mp4',start:'0',end:'1',fast:true,muteAudio:true},silentPath,runtime).request);
 const silent=await inspectAsset(silentPath,worker,runtime);assert.equal(silent.asset.audioTracks.length,0);assert(!availableTasksForAssets([silent.asset]).some(v=>v.id.startsWith('audio.')));assert.throws(()=>planTask([silent],'audio.convert',{format:'wav'},'',runtime));
 console.log('Contextual actions reject silent-video audio extraction before saving');
 const cache=new MediaPreviewCache(folder);
 try {
  const signal=new AbortController().signal;
  const playback=await cache.prepare(records.video,{audioOnly:false},worker,runtime,signal);
  assert.equal(playback.kind,'video');assert(playback.poster.startsWith('data:image/png;base64,'));assert(playback.waveform.channels.length>0);assert.equal(playback.duration.ticks/playback.duration.timescale,2);
  assert(!playback.url.includes(folder));assert.equal((await cache.respond('unknown',new Request('fileform://app/preview/unknown'))).status,404);
  const response=await cache.respond(playback.id,new Request(playback.url));assert.equal(response.status,200);const bytes=Buffer.from(await response.arrayBuffer());assert(bytes.length>0);
  const ranged=await cache.respond(playback.id,new Request(playback.url,{headers:{Range:'bytes=2-9'}}));assert.equal(ranged.status,206);assert(Buffer.from(await ranged.arrayBuffer()).equals(bytes.subarray(2,10)));
  const suffix=await cache.respond(playback.id,new Request(playback.url,{headers:{Range:'bytes=-8'}}));assert(Buffer.from(await suffix.arrayBuffer()).equals(bytes.subarray(-8)));
  const head=await cache.respond(playback.id,new Request(playback.url,{method:'HEAD'}));assert.equal(Number(head.headers.get('Content-Length')),bytes.length);assert.equal((await head.arrayBuffer()).byteLength,0);
  for(const range of ['bytes=999999999999999999999-','bytes=3-1','bytes=0-1,4-5','bytes=-0'])assert.equal((await cache.respond(playback.id,new Request(playback.url,{headers:{Range:range}}))).status,416);
  const proxy=join(folder,'reopened-playback.mp4');await fs.writeFile(proxy,bytes);assert.equal((await inspectAsset(proxy,worker,runtime)).asset.family,'video');
  cache.release(playback.id);assert.equal((await cache.respond(playback.id,new Request(playback.url))).status,404);
  const audioPlayback=await cache.prepare(records.video,{audioOnly:true},worker,runtime,signal);assert.equal(audioPlayback.kind,'audio');assert.equal(audioPlayback.poster,undefined);
  const audioProxy=join(folder,'reopened-audio.wav');await fs.writeFile(audioProxy,Buffer.from(await (await cache.respond(audioPlayback.id,new Request(audioPlayback.url))).arrayBuffer()));assert.equal((await inspectAsset(audioProxy,worker,runtime)).asset.family,'audio');
  cache.release(audioPlayback.id);
  await assert.rejects(cache.prepare({...records.video,sha256:'0'.repeat(64)},{audioOnly:false},worker,runtime,signal),/changed/);
  const cancelled=new AbortController();cancelled.abort();await assert.rejects(cache.prepare(records.video,{audioOnly:false},worker,runtime,cancelled.signal));
  const midway=new AbortController();let operations=0;
  await assert.rejects(cache.prepare(silent,{audioOnly:false},async request=>{operations++;const result=await worker(request);midway.abort();return result;},runtime,midway.signal));
  assert.equal(operations,1); // Cancellation after the native export cannot start later preview steps or publish a lease.
 }finally{await cache.close();}
 assert(!(await fs.readdir(folder)).some(v=>v.startsWith('fileform-playback-')));console.log('Native playback/waveforms, streamed seeking, leases, identity/cancellation and cache cleanup passed');

 const combined=await inspectAsset(join(folder,'combined.pdf'),worker,runtime);
 const thumbs=await previewPages([{record:records.pdf,pageIndex:0}],worker,runtime,folder);
 assert.equal(thumbs[0].sourceID,records.pdf.asset.id);assert(thumbs[0].dataURL.startsWith('data:image/png;base64,'));assert(thumbs[0].width<=240&&thumbs[0].height<=240);
 await assert.rejects(previewPages([{record:{...records.pdf,sha256:'0'.repeat(64)},pageIndex:0}],worker,runtime,folder),/changed/);
 assert(!(await fs.readdir(folder)).some(v=>v.startsWith('fileform-pages-')));console.log('PDF thumbnail identity and cleanup passed');
 const pageOrder=[{sourceID:records.image.asset.id,pageIndex:0,rotation:270},{sourceID:records.pdf.asset.id,pageIndex:0,rotation:90},{sourceID:records.pdf.asset.id,pageIndex:0,rotation:180}];
 const organized=join(folder,'organized.pdf');await worker(planTask([records.pdf,records.image],'pdf.combine',{format:'pdf',pageOrder},organized,runtime).request);
 const geometry=await worker({operation:'inspect_pdf_pages',input:organized,directory:runtime.pack('pdf')});assert.equal(geometry.pages.length,3);assert.deepEqual(geometry.pages.map(v=>v.rotation),[270,90,180]);
 const rastered=await worker(planTask([records.pdf,records.image],'pdf.images',{format:'png',pageOrder,dpi:72},join(folder,'organized-pages'),runtime).request);assert.equal(rastered.parts.length,3);assert.deepEqual(rastered.parts.map(v=>v.clockwise_rotation),[270,90,180]);
 const marked=await worker(planTask([combined],'pdf.split',{format:'pdf',splitAfter:[1]},join(folder,'marked-split'),runtime).request);assert.deepEqual(marked.parts.map(v=>v.pages),[1,1]);
 assert.throws(()=>planTask([records.pdf],'pdf.combine',{format:'pdf',pageOrder:[{sourceID:records.image.asset.id,pageIndex:0,rotation:0}]},'',runtime));
 console.log('PDF page order, duplication, rotation, selected raster export and visual marker planning passed');
 const embedded=planTask([records.pdf],'pdf.extract-images',{format:'images'},join(folder,'embedded'),runtime);
 const extracted=await worker(embedded.request);assert(extracted.supported>0);console.log('pdf.extract-images real desktop module flow passed');
 const embeddedText=join(folder,'embedded.txt');await worker(planTask([records.pdf],'text.extract',{format:'txt'},embeddedText,runtime).request);assert((await fs.readFile(embeddedText,'utf8')).includes('Fileform document'));console.log('text.extract real desktop module flow passed');
 const textSource=await inspectAsset(join(root,'crates/fileform-engine/tests/fixtures/ocr-page.heic'),worker,runtime);
 const textPlan=planTask([textSource],'text.ocr',{format:'txt'},join(folder,'ocr.txt'),runtime);
 await worker(textPlan.request);assert.equal(await fs.readFile(join(folder,'ocr.txt'),'utf8'),'Fileform page 1\n');console.log('text.ocr real desktop module flow passed');
 const scanned=join(folder,'scanned.pdf');await worker(planTask([textSource],'pdf.combine',{format:'pdf'},scanned,runtime).request);
 const scannedSource=await inspectAsset(scanned,worker,runtime);await worker(planTask([scannedSource],'text.ocr',{format:'txt'},join(folder,'pdf-ocr.txt'),runtime).request);assert((await fs.readFile(join(folder,'pdf-ocr.txt'),'utf8')).includes('Fileform page 1'));
 const optimized=await worker(planTask([combined],'pdf.compress',{format:'pdf',lossy:true,maxBytes:100000},join(folder,'optimized.pdf'),runtime).request);assert(['saved','not_smaller'].includes(optimized.status));if(optimized.status==='saved')assert((await fs.stat(join(folder,'optimized.pdf'))).isFile());console.log('pdf.compress real desktop module flow passed');
 console.log('text.ocr scanned-PDF desktop module flow passed');
 for(const record of Object.values(records)){assert.equal((await inspectAsset(record.path,worker,runtime)).sha256,record.sha256);}
 const stale={...records.pdf,sha256:'0'.repeat(64)};
 const output=join(folder,'stale.pdf');const plan=planTask([stale],'pdf.combine',{format:'pdf'},output,runtime);
 const error=await worker(plan.request,false);assert.equal(error.code,'source_changed');await assert.rejects(fs.stat(output));
 assert.throws(()=>planTask([records.table],'video.trim',{format:'mp4'},'',runtime));
 console.log('Shared desktop inspector/task planner: real table/image/PDF/audio/video paths, native outputs and stale-source protection passed.');
}finally{await fs.rm(folder,{recursive:true,force:true});}
