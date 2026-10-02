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

 const combined=await inspectAsset(join(folder,'combined.pdf'),worker,runtime);
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
