import { app, BrowserWindow, dialog, ipcMain, nativeTheme, shell, protocol, net, type IpcMainInvokeEvent } from 'electron';
import { spawn, type ChildProcess } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { promises as fs } from 'node:fs';
import { basename, dirname, join, parse, resolve, relative, isAbsolute, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import type { Appearance, SourceFile, SavedFile, TableOutput } from '../src/contracts.js';

app.setName('Fileform Preview');
app.setAppUserModelId('app.fileform.DesktopPreview');
let window:BrowserWindow|null=null;
let busy=false;
let mode:Appearance='system';
const children=new Set<ChildProcess>();
const sources=new Map<string,SourceFile & {path:string;sha256:string}>();
const saved=new Map<string,string>();
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
    const timer=setTimeout(()=>{stopped=true;child.kill();hardStop=setTimeout(()=>child.kill('SIGKILL'),2000);},45_000);
    child.stdout.on('data',(data:Buffer)=>{bytes+=data.length;if(bytes>1_048_576){stopped=true;child.kill();}else chunks.push(data);});
    child.stderr.resume();
    child.on('error',()=>{clearTimeout(timer);clearTimeout(hardStop);children.delete(child);reject(new Error('The native worker could not start.'));});
    child.on('close',(code)=>{
      clearTimeout(timer);clearTimeout(hardStop);children.delete(child);
      if(stopped){reject(new Error('The worker stopped before returning a receipt. Check the selected output folder.'));return;}
      try {
        const response=JSON.parse(Buffer.concat(chunks).toString('utf8'));
        if(response.version!==1) throw new Error('Unsupported worker response.');
        if(code!==0 || response.ok!==true) throw new Error(typeof response.error?.message==='string'?response.error.message.slice(0,1000):'The file could not be processed.');
        resolve(response.result);
      } catch(error){reject(error instanceof Error?error:new Error('Invalid worker response.'));}
    });
    child.stdin.on('error',()=>{});
    child.stdin.end(JSON.stringify(request));
  });
}
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
app.whenReady().then(async()=>{
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
  await createWindow();
});
app.on('activate',()=>{if(!window)void createWindow();});
app.on('window-all-closed',()=>{if(process.platform!=='darwin')app.quit();});
app.on('before-quit',(event)=>{if(children.size){event.preventDefault();dialog.showMessageBoxSync({type:'info',message:'A file is still being processed.',detail:'Please wait for it to finish before quitting.',buttons:['Keep working']});}});
