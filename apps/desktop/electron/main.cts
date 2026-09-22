import { app, BrowserWindow, Menu, dialog, ipcMain, nativeTheme, shell, protocol, net, type IpcMainInvokeEvent } from 'electron';
import { spawn, type ChildProcess } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { promises as fs } from 'node:fs';
import { basename, dirname, join, parse, resolve, relative, isAbsolute, sep } from 'node:path';
import {validateImageExport,outputDimensions} from '../src/image-export.js';
import { pathToFileURL } from 'node:url';
import type { Appearance, SourceFile, SavedFile, TableOutput, ImageSource, ImageSavedFile } from '../src/contracts.js';

app.setName('Fileform Preview');
app.setAppUserModelId('app.fileform.DesktopPreview');
const ownsInstance=app.requestSingleInstanceLock();
if(!ownsInstance)app.quit();
let window:BrowserWindow|null=null;
let windowCreation:Promise<void>|null=null;
let busy=false;
let cancelCurrent:(()=>void)|null=null;
let mode:Appearance='system';
const children=new Set<ChildProcess>();
const sources=new Map<string,SourceFile & {path:string;sha256:string}>();
const saved=new Map<string,string>();
const images=new Map<string,ImageSource & {path:string;sha256:string}>();
const rendererRoot=resolve(__dirname,'../../dist');
const page='fileform://app/index.html';
protocol.registerSchemesAsPrivileged([{scheme:'fileform',privileges:{standard:true,secure:true,supportFetchAPI:true,corsEnabled:true}}]);
function authorize(event:IpcMainInvokeEvent) {
  if (!window || event.sender!==window.webContents || event.senderFrame!==window.webContents.mainFrame || event.senderFrame.url.split('#')[0]!==page) throw new Error('Request is not from the app window.');
}
async function exclusive<T>(work:()=>Promise<T>):Promise<T> {
  if(busy) throw new Error('Wait for the current file to finish.');
  busy=true; try { return await work(); } finally { busy=false; }
}
function worker(request:unknown):Promise<any> {
  const filename='fileform-worker'+(process.platform==='win32'?'.exe':'');
  const executable=app.isPackaged ? join(process.resourcesPath,'native',filename) : resolve(__dirname,'../../../../target/release',filename);
  return new Promise((resolve,reject)=>{
    const temp=app.getPath('temp');
    const environment:NodeJS.ProcessEnv={TMPDIR:temp,TMP:temp,TEMP:temp};
    if(process.platform==='win32'){environment.SystemRoot=process.env.SystemRoot;environment.PATH=join(process.env.SystemRoot??'C:\\Windows','System32');}else{environment.PATH='/usr/bin:/bin';}
    const child=spawn(executable,[],{stdio:['pipe','pipe','pipe'],windowsHide:true,env:environment});
    children.add(child);
    const chunks:Buffer[]=[]; let bytes=0; let stopped=false;
    let hardStop:NodeJS.Timeout|undefined;
    let cancellationRequested=false;
    const cancel=()=>{
      if(cancellationRequested)return;
      cancellationRequested=true;
      child.stdin.end('cancel\n');
      hardStop=setTimeout(()=>{stopped=true;child.kill('SIGKILL');},2000);
    };
    cancelCurrent=cancel;
    const timer=setTimeout(cancel,45_000);
    child.stdout.on('data',(data:Buffer)=>{bytes+=data.length;if(bytes>1_048_576){stopped=true;child.kill();}else chunks.push(data);});
    child.stderr.resume();
    child.on('error',()=>{clearTimeout(timer);clearTimeout(hardStop);children.delete(child);if(cancelCurrent===cancel)cancelCurrent=null;reject(new Error('The native worker could not start.'));});
    child.on('close',(code)=>{
      clearTimeout(timer);clearTimeout(hardStop);children.delete(child);if(cancelCurrent===cancel)cancelCurrent=null;
      if(stopped){reject(new Error('The worker stopped before returning a receipt. Check the selected output folder.'));return;}
      try {
        const response=JSON.parse(Buffer.concat(chunks).toString('utf8'));
        if(response.version!==1) throw new Error('Unsupported worker response.');
        if(code!==0 || response.ok!==true) throw new Error(typeof response.error?.message==='string'?response.error.message.slice(0,1000):'The file could not be processed.');
        resolve(response.result);
      } catch(error){reject(error instanceof Error?error:new Error('Invalid worker response.'));}
    });
    child.stdin.on('error',()=>{});
    child.stdin.write(JSON.stringify(request)+'\n');
  });
}
ipcMain.handle('fileform:cancel',(event)=>{authorize(event);cancelCurrent?.();});
const count=(x:unknown):x is number=>Number.isSafeInteger(x) && Number(x)>=0;
ipcMain.handle('fileform:choose',async(event)=>{
  authorize(event);
  return exclusive(async()=>{
    const result=await dialog.showOpenDialog(window!,{properties:['openFile'],filters:[{name:'Tables',extensions:['csv','tsv','json']}]});
    if(result.canceled || result.filePaths.length!==1)return null;
    const path=await fs.realpath(result.filePaths[0]);
    const inspected=await worker({operation:'inspect',input:path});
    if(inspected.kind!=='inspection' || !count(inspected.rows) || !count(inspected.columns) || !count(inspected.bytes) || typeof inspected.sha256!=='string' || !/^[a-f0-9]{64}$/.test(inspected.sha256))throw new Error('Invalid inspection receipt.');
    if(!Array.isArray(inspected.outputs)||!inspected.outputs.length||!inspected.outputs.every((x:unknown)=>x==='json'||x==='csv'||x==='tsv'))throw new Error('Invalid output capabilities.');
    const source={outputs:inspected.outputs as TableOutput[],scalarTypesBecomeText:parse(path).ext.toLowerCase()==='.json',id:randomUUID(),name:basename(path),bytes:inspected.bytes,rows:inspected.rows,columns:inspected.columns};
    sources.clear();sources.set(source.id,{...source,path,sha256:inspected.sha256});return source;
  });
});
ipcMain.handle('fileform:choose-image',async(event)=>{
  authorize(event);
  return exclusive(async()=>{
    const result=await dialog.showOpenDialog(window!,{properties:['openFile'],filters:[{name:'Images',extensions:['png','jpg','jpeg','tif','tiff']}]});
    if(result.canceled||result.filePaths.length!==1)return null;
    const path=await fs.realpath(result.filePaths[0]);
    const info=await worker({operation:'inspect_image',input:path,preview:true});
    if(info.kind!=='image_inspection'||!count(info.display_width)||!count(info.display_height)||info.display_width===0||info.display_height===0||info.display_width*info.display_height>80_000_000||!count(info.bytes)||typeof info.has_alpha!=='boolean'||typeof info.conversion_available!=='boolean'||typeof info.sha256!=='string'||!/^[a-f0-9]{64}$/.test(info.sha256))throw new Error('Invalid image inspection receipt.');
    const preview=info.preview;
    if(preview!==null&&(!preview||!count(preview.width)||!count(preview.height)||preview.width<1||preview.height<1||preview.width>128||preview.height>128||!Array.isArray(preview.rgba)||preview.rgba.length!==preview.width*preview.height*4||!preview.rgba.every((x:unknown)=>count(x)&&x<=255)))throw new Error('Invalid image preview.');
    const source:ImageSource={id:randomUUID(),name:basename(path),bytes:info.bytes,width:info.display_width,height:info.display_height,hasAlpha:info.has_alpha,canConvert:info.conversion_available,preview};
    images.clear();images.set(source.id,{...source,path,sha256:info.sha256});return source;
  });
});
ipcMain.handle('fileform:save-image',async(event,id:unknown,options:unknown)=>{
  authorize(event);
  if(typeof id!=='string'||!images.has(id))throw new Error('Choose the image again.');
  const source=images.get(id)!;
  const {format,background,quality,crop,maxDimension,maxBytes,minimumQuality}=validateImageExport(options,source.width,source.height,source.hasAlpha);
  if(!source.canConvert)throw new Error('This image requires preservation support that is still being implemented.');
  const expected=outputDimensions(crop?.width??source.width,crop?.height??source.height,maxDimension);
  const extension=format==='jpeg'?'jpg':format;
  const extensions=format==='jpeg'?['jpg','jpeg']:format==='tiff'?['tif','tiff']:['png'];
  return exclusive(async()=>{
    const choice=await dialog.showSaveDialog(window!,{defaultPath:join(dirname(source.path),parse(source.name).name+'-converted.'+extension),filters:[{name:format.toUpperCase()+' image',extensions}],properties:['createDirectory']});
    if(choice.canceled||!choice.filePath)return null;
    if(!extensions.map(value=>'.'+value).includes(parse(choice.filePath).ext.toLowerCase()))throw new Error('Use a filename matching the selected format.');
    const receipt=await worker({operation:'convert_image',input:source.path,output:choice.filePath,expected_source_sha256:source.sha256,background,quality,crop,max_dimension:maxDimension,max_bytes:maxBytes,minimum_quality:minimumQuality});
    if(receipt.kind!=='saved_image'||receipt.output!==choice.filePath||receipt.width!==expected.width||receipt.height!==expected.height||!count(receipt.bytes)||typeof receipt.sha256!=='string'||!/^[a-f0-9]{64}$/.test(receipt.sha256))throw new Error('The image receipt could not be validated. Check the output folder.');
    const maximumQuality=quality??85;const floor=maxBytes===undefined?maximumQuality:(minimumQuality??Math.min(35,maximumQuality));
    if(!count(receipt.attempts)||receipt.attempts<1||receipt.attempts>(format==='jpeg'&&maxBytes!==undefined?11:1)||(maxBytes!==undefined&&receipt.bytes>maxBytes)||(format==='jpeg'?(!count(receipt.quality)||receipt.quality<floor||receipt.quality>maximumQuality):receipt.quality!==null))throw new Error('The image receipt does not meet the requested limits. Check the output folder.');
    const result:ImageSavedFile={id:randomUUID(),name:basename(choice.filePath),bytes:receipt.bytes,width:receipt.width,height:receipt.height,quality:receipt.quality};
    if(saved.size>=100)saved.delete(saved.keys().next().value!);
    saved.set(result.id,choice.filePath);return result;
  });
});
ipcMain.handle('fileform:save',async(event,id:unknown,format:unknown)=>{
  authorize(event);
  if(format!=='json'&&format!=='csv'&&format!=='tsv')throw new Error('Choose JSON, CSV or TSV.');
  if(typeof id!=='string'||!sources.has(id))throw new Error('Choose the source file again.');
  const source=sources.get(id)!;
  if(!source.outputs.includes(format))throw new Error('This output format is unavailable for the selected table.');
  return exclusive(async()=>{
    const choice=await dialog.showSaveDialog(window!,{defaultPath:join(dirname(source.path),parse(source.name).name+'-converted.'+format),filters:[{name:format.toUpperCase()+' table',extensions:[format]}],properties:['createDirectory']});
    if(choice.canceled||!choice.filePath)return null;
    const output=choice.filePath;
    if(parse(output).ext.toLowerCase()!=='.'+format)throw new Error('Use a .'+format+' filename for the selected format.');
    const receipt=await worker({operation:'convert_table',input:source.path,output,expected_source_sha256:source.sha256});
    if(receipt.kind!=='saved'||receipt.output!==output||receipt.rows!==source.rows||!count(receipt.bytes))throw new Error('The saved receipt could not be validated. Check the output folder.');
    const result:SavedFile={id:randomUUID(),name:basename(output),bytes:receipt.bytes,rows:receipt.rows};
    if(saved.size>=100)saved.delete(saved.keys().next().value!);
    saved.set(result.id,output);return result;
  });
});
ipcMain.handle('fileform:reveal',(event,id:unknown)=>{
  authorize(event);if(typeof id!=='string'||!saved.has(id))throw new Error('This result is no longer in the current session.');
  shell.showItemInFolder(saved.get(id)!);
});
ipcMain.handle('fileform:appearance',async(event,value:unknown)=>{
  authorize(event);
  if(value!==undefined){
    if(value!=='system'&&value!=='light'&&value!=='dark')throw new Error('Invalid appearance.');
    mode=value;nativeTheme.themeSource=mode;
    await fs.mkdir(app.getPath('userData'),{recursive:true});
    await fs.writeFile(join(app.getPath('userData'),'appearance.json'),JSON.stringify({mode}));
  }
  return {mode,dark:nativeTheme.shouldUseDarkColors};
});
async function showWindow(){
  if(!ownsInstance)return;
  await app.whenReady();
  if(window&&!window.isDestroyed()){
    if(window.isMinimized())window.restore();
    window.show();window.focus();return;
  }
  if(!windowCreation)windowCreation=createWindow().finally(()=>{windowCreation=null;});
  await windowCreation;
}
if(ownsInstance)app.on('second-instance',()=>{void showWindow();});
async function createWindow(){
  try{const prefs=JSON.parse(await fs.readFile(join(app.getPath('userData'),'appearance.json'),'utf8'));if(['system','light','dark'].includes(prefs.mode))mode=prefs.mode;}catch{}
  nativeTheme.themeSource=mode;
  window=new BrowserWindow({width:1180,height:760,minWidth:760,minHeight:560,show:false,title:'Fileform Preview',backgroundColor:nativeTheme.shouldUseDarkColors?'#111416':'#f7f9fa',webPreferences:{preload:join(__dirname,'preload.cjs'),contextIsolation:true,nodeIntegration:false,sandbox:true,webSecurity:true}});
  window.webContents.setWindowOpenHandler(()=>({action:'deny'}));
  window.webContents.on('will-navigate',(event,url)=>{if(url!==page)event.preventDefault();});
  window.webContents.session.setPermissionRequestHandler((_contents,_permission,callback)=>callback(false));
  window.once('ready-to-show',()=>{window!.maximize();window!.show();});
  window.on('close',(event)=>{if(children.size){event.preventDefault();dialog.showMessageBoxSync(window!,{type:'info',message:'A file is still being processed.',detail:'Please wait for it to finish before closing.',buttons:['Keep working']});}});
  window.on('closed',()=>{window=null;});
  await window.loadURL(page);
}
if(ownsInstance)app.whenReady().then(async()=>{
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    ...(process.platform==='darwin'?[{role:'appMenu' as const}]:[]),
    {label:'File',submenu:[
      {label:'Open…',accelerator:'CommandOrControl+O',click:()=>{
        if(busy)return;
        void showWindow().then(()=>{
          if(!busy&&!window?.isDestroyed())window?.webContents.send('fileform:open-request');
        });
      }},
      {type:'separator'},
      {role:process.platform==='darwin'?'close':'quit'},
    ]},
    {role:'editMenu'},
    {label:'View',submenu:[{role:'resetZoom'},{role:'zoomIn'},{role:'zoomOut'},{type:'separator'},{role:'togglefullscreen'}]},
    {role:'windowMenu'},
  ]));
  protocol.handle('fileform',(request)=>{
    try{
      const url=new URL(request.url);
      if(url.hostname!=='app'||request.method!=='GET'||url.username||url.password)return new Response('Not found',{status:404});
      const asset=resolve(rendererRoot,'.'+decodeURIComponent(url.pathname));
      const part=relative(rendererRoot,asset);
      if(part==='..'||part.startsWith('..'+sep)||isAbsolute(part))return new Response('Not found',{status:404});
      return net.fetch(pathToFileURL(asset).href);
    }catch{return new Response('Not found',{status:404});}
  });
  await showWindow();
});
app.on('activate',()=>{void showWindow();});
app.on('window-all-closed',()=>{if(process.platform!=='darwin')app.quit();});
app.on('before-quit',(event)=>{if(children.size){event.preventDefault();dialog.showMessageBoxSync({type:'info',message:'A file is still being processed.',detail:'Please wait for it to finish before quitting.',buttons:['Keep working']});}});
