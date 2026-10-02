import {promises as fs} from 'node:fs';
import {join,resolve,relative,isAbsolute,sep} from 'node:path';
export type PackName='media'|'pdf'|'renderer'|'ocr'|'heic';
const identifiers:Record<PackName,string>={media:'app.fileform.media',pdf:'app.fileform.pdf',renderer:'app.fileform.pdf-render',ocr:'app.fileform.ocr',heic:'app.fileform.heic'};
export interface NativeRuntime {root:string;worker:string;cli:string;pack:(name:PackName)=>string}
export async function loadRuntime(root:string):Promise<NativeRuntime> {
  const canonical=await fs.realpath(root);
  const manifestPath=join(canonical,'runtime.json');
  if((await fs.stat(manifestPath)).size>1_048_576)throw new Error('The processing tools are incomplete. Reinstall Fileform.');
  const manifest=JSON.parse(await fs.readFile(manifestPath,'utf8'));
  if(manifest.schemaVersion!==1||manifest.platform!==process.platform||manifest.architecture!==(process.arch==='arm64'?'aarch64':process.arch==='x64'?'x86_64':process.arch))throw new Error('The processing tools do not match this system.');
  const suffix=process.platform==='win32'?'.exe':'';
  async function file(relativePath:string){
    const path=await fs.realpath(join(canonical,relativePath));const part=relative(canonical,path);
    if(part==='..'||part.startsWith('..'+sep)||isAbsolute(part)||!(await fs.stat(path)).isFile())throw new Error('The processing tools are incomplete. Reinstall Fileform.');
    return path;
  }
  const paths={} as Record<PackName,string>;
  for(const name of Object.keys(identifiers) as PackName[]){
    const entry=manifest.packs?.[name];
    if(entry?.id!==identifiers[name]||entry.directory!==name)throw new Error('A required processing tool is missing.');
    const folder=await fs.realpath(join(canonical,name));const part=relative(canonical,folder);
    if(part==='..'||part.startsWith('..'+sep)||isAbsolute(part))throw new Error('Invalid processing tool location.');
    await file(name+'/manifest.json');paths[name]=folder;
  }
  return {root:canonical,worker:await file('fileform-worker'+suffix),cli:await file('fileform-native'+suffix),pack:name=>paths[name]};
}
export function runtimeRoot(packaged:boolean,resources:string,project:string):string {
  return packaged?join(resources,'native'):resolve(process.env.FILEFORM_RUNTIME_ROOT??join(project,'Artifacts','DesktopRuntime'));
}
