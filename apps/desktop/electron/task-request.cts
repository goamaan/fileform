import type {AssetRecord} from './assets.cjs';
import type {NativeRuntime} from './runtime.cjs';
import {availableTasks,type TaskID} from '../src/tasks.js';
import {validateImageExport} from '../src/image-export.js';
import type {TaskOptions} from '../src/contracts.js';
export interface TaskPlan {request:Record<string,unknown>;extension:string;folder:boolean}
function time(value:unknown){
 if(typeof value!=='string'||value.length>24||!/^\d*(?:\.\d{1,9})?$/.test(value)||value===''||value==='.')throw new Error('Use non-negative seconds.');
 const [whole,fraction='']=value.split('.');const scale=10**fraction.length;const ticks=Number(whole||0)*scale+Number(fraction||0);
 if(!Number.isSafeInteger(ticks)||ticks<0||ticks>21600*scale)throw new Error('Choose a time within six hours.');
 return {ticks,timescale:scale};
}
export function planTask(records:AssetRecord[],task:TaskID,raw:unknown,output:string,runtime:NativeRuntime):TaskPlan{
 const definition=availableTasks(records.map(v=>v.asset.family)).find(v=>v.id===task);
 if(!definition)throw new Error('Choose an action available for these files.');
 if(!raw||typeof raw!=='object'||Array.isArray(raw))throw new Error('Invalid action options.');
 const options=raw as TaskOptions;
 if(!definition.outputFormats.includes(options.format))throw new Error('Choose an available output format.');
 const integer=(v:unknown,min:number,max:number,message:string)=>{if(!Number.isSafeInteger(v)||Number(v)<min||Number(v)>max)throw new Error(message);return Number(v);};
 if(options.maxBytes!==undefined)integer(options.maxBytes,1,512*1024*1024,'Choose a byte limit up to 512 MiB.');
 if(options.maxDimension!==undefined)integer(options.maxDimension,1,16384,'Choose a positive picture bound.');
 if(options.audioStream!==undefined&&!records[0].asset.audioTracks?.some(v=>v.index===options.audioStream))throw new Error('Choose an existing audio track.');
 if(options.muteAudio&&options.audioStream!==undefined)throw new Error('Choose a track or mute audio, not both.');
 const source=records[0];const input=source.path;const expected_source_sha256=source.sha256;
 const inputs=records.map(v=>v.path);const hashes=records.map(v=>v.sha256);
 const pdf={inputs,expected_source_sha256:hashes,output,directory:runtime.pack('pdf'),renderer_directory:runtime.pack('renderer')};
 const media={input,output,directory:runtime.pack('media'),expected_source_sha256,audio_stream:options.audioStream};
 let request:Record<string,unknown>;let folder=false;
 if(task==='table.convert'){
  if(!source.asset.table?.outputs.includes(options.format as any))throw new Error('This table cannot use that output format.');
  request={operation:'convert_table',input,output,expected_source_sha256};
 }else if(task==='image.convert'){
  const image=source.asset.image!;
  const validated=validateImageExport({format:options.format,background:options.background,quality:options.quality,crop:options.crop,maxDimension:options.maxDimension,maxBytes:options.maxBytes,minimumQuality:options.minimumQuality},image.width,image.height,image.hasAlpha);
  request={operation:'convert_image',input,output,expected_source_sha256,background:validated.background,quality:validated.quality,crop:validated.crop,max_dimension:validated.maxDimension,max_bytes:validated.maxBytes,minimum_quality:validated.minimumQuality};
 }else if(task==='pdf.combine'){
  request={operation:'compose_pdf',...pdf,page_ranges:options.pages||undefined,allow_document_changes:true};
 }else if(task==='pdf.split'){
  folder=true;
  const selection=options.pages?{mode:'ranges',ranges:options.pages}:options.splitAfter?{mode:'after',positions:options.splitAfter}:options.splitEvery?{mode:'every',count:integer(options.splitEvery,1,1000,'Choose pages per part from 1 to 1000.')}:undefined;
  request={operation:'split_pdf',...pdf,selection,allow_document_changes:true};
 }else if(task==='pdf.images'){
  folder=true;request={operation:'export_pdf_images',...pdf,page_ranges:options.pages||undefined,format:options.format,dpi:integer(options.dpi??144,36,600,'Choose 36 to 600 DPI.'),quality:options.format==='jpeg'?integer(options.quality??85,5,100,'Choose JPEG quality from 5 to 100.'):undefined,allow_rasterization:true};
 }else if(task==='pdf.extract-images'){
  folder=true;request={operation:'extract_pdf_images',inputs,expected_source_sha256:hashes,output,directory:runtime.pack('pdf'),page_ranges:options.pages||undefined};
 }else if(task==='pdf.compress'){
  request=options.lossy?{operation:'optimize_pdf_images',input,output,expected_source_sha256,directory:runtime.pack('pdf'),renderer_directory:runtime.pack('renderer'),quality:(options.quality??80)/100,minimum_quality:(options.minimumQuality??50)/100,max_dimension:options.maxDimension,max_bytes:options.maxBytes,allow_lossy:true}:{operation:'optimize_pdf',input,output,expected_source_sha256,directory:runtime.pack('pdf'),renderer_directory:runtime.pack('renderer'),max_bytes:options.maxBytes};
 }else if(task==='text.extract'||task==='text.ocr'){
  request=source.asset.family==='image'?{operation:'ocr_image',input,output,expected_source_sha256,directory:runtime.pack('ocr'),language:'eng'}:{operation:'export_pdf_text',input,output,expected_source_sha256,directory:runtime.pack('pdf'),renderer_directory:runtime.pack('renderer'),ocr:task==='text.ocr'?{directory:runtime.pack('ocr'),language:'eng'}:undefined};
 }else if(task==='audio.convert'){
  request=options.maxBytes?{operation:'fit_audio',...media,max_bytes:options.maxBytes}:{operation:'convert_audio',...media};
 }else if(task==='video.convert'){
  request=options.maxBytes?{operation:'fit_video',...media,audio_stream:undefined,options:{max_bytes:options.maxBytes,max_dimension:options.maxDimension,audio_stream:options.audioStream}}:{operation:'convert_video',...media,options:{max_dimension:options.maxDimension}};
 }else{
  const interval={start:time(options.start??'0'),end:time(options.end??String(source.asset.duration??0))};
  if(interval.start.ticks*interval.end.timescale>=interval.end.ticks*interval.start.timescale)throw new Error('Choose an increasing time interval.');
  if(task==='audio.trim'){
   if(options.fast&&options.format!=='m4a')throw new Error('Fast audio trim writes M4A.');
   request={operation:options.fast?'copy_audio_trim':'trim_audio_time',...media,interval};
  }else request={operation:options.fast?'copy_video_trim':'trim_video',...media,audio_stream:undefined,options:{interval,audio_stream:options.audioStream,mute_audio:options.muteAudio??false}};
 }
 return {request,extension:options.format==='jpeg'?'jpg':options.format,folder};
}
