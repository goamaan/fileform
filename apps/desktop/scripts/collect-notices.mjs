// SPDX-License-Identifier: Apache-2.0
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {downloadArtifact} from '@electron/get';
import yauzl from 'yauzl';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const output=path.join(root,'Artifacts/desktop-notices');
fs.rmSync(output,{recursive:true,force:true});fs.mkdirSync(output,{recursive:true});
const records=[];
function copy(source,destination){
  if(!fs.statSync(source).isFile())throw new Error('Expected a license file');
  const bytes=fs.readFileSync(source);const target=path.join(output,destination);
  fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,bytes);
  return {file:destination,sha256:createHash('sha256').update(bytes).digest('hex')};
}
records.push({name:'Fileform',license:'Apache-2.0',files:[copy(path.join(root,'LICENSE'),'Fileform/LICENSE'),copy(path.join(root,'NOTICE'),'Fileform/NOTICE')]});
const host=execFileSync('rustc',['-vV'],{cwd:root,encoding:'utf8'}).match(/^host: (.+)$/m)?.[1];
if(!host)throw new Error('Cannot determine the native Rust target');
const metadata=JSON.parse(execFileSync('cargo',['metadata','--format-version','1','--locked','--filter-platform',host],{cwd:root,encoding:'utf8',maxBuffer:16*1024*1024}));
const nodes=new Map(metadata.resolve.nodes.map(node=>[node.id,node]));
const reached=new Set();const pending=[...metadata.workspace_members];
while(pending.length){const id=pending.pop();if(reached.has(id))continue;reached.add(id);for(const dep of nodes.get(id)?.deps??[])pending.push(dep.pkg);}
for(const pkg of metadata.packages.filter(p=>p.source&&reached.has(p.id))){
  const directory=path.dirname(pkg.manifest_path);const names=fs.readdirSync(directory).filter(name=>/^(licen[cs]e|copying|notice|unlicense)([._-].*)?$/i.test(name)&&fs.lstatSync(path.join(directory,name)).isFile());
  if(pkg.license_file){
    const resolved=path.resolve(directory,pkg.license_file);const relative=path.relative(directory,resolved);
    if(relative.startsWith('..')||path.isAbsolute(relative))throw new Error(`License escapes package: ${pkg.name}`);
    if(!names.includes(relative))names.push(relative);
  }
  if(!names.length)throw new Error(`Missing license text for ${pkg.name} ${pkg.version}`);
  records.push({name:pkg.name,version:pkg.version,license:pkg.license,files:names.map(name=>copy(path.join(directory,name),`rust/${pkg.name}-${pkg.version}/${name}`))});
}
const sysroot=execFileSync('rustc',['--print','sysroot'],{cwd:root,encoding:'utf8'}).trim();
const rustDoc=path.join(sysroot,'share/doc/rust');
const rustFiles=[];
for(const name of ['COPYRIGHT.html','COPYRIGHT-library.html'])if(fs.existsSync(path.join(rustDoc,name)))rustFiles.push(copy(path.join(rustDoc,name),`rust-toolchain/${name}`));
const licenseDirectory=path.join(rustDoc,'licenses');
if(fs.existsSync(licenseDirectory))for(const name of fs.readdirSync(licenseDirectory))if(fs.lstatSync(path.join(licenseDirectory,name)).isFile())rustFiles.push(copy(path.join(licenseDirectory,name),`rust-toolchain/licenses/${name}`));
if(!rustFiles.length)throw new Error('Rust toolchain notices are missing');
records.push({name:'Rust toolchain',version:execFileSync('rustc',['--version'],{cwd:root,encoding:'utf8'}).trim(),files:rustFiles});
for(const name of ['react','react-dom','scheduler','electron']){
  const directory=path.join(root,'apps/desktop/node_modules',name);const pkg=JSON.parse(fs.readFileSync(path.join(directory,'package.json'),'utf8'));
  const file=['LICENSE','LICENSE.txt','LICENSE.md'].find(n=>fs.existsSync(path.join(directory,n)));
  if(!file)throw new Error(`Missing npm notice: ${name}`);
  records.push({name,version:pkg.version,license:pkg.license,files:[copy(path.join(directory,file),`javascript/${name}/${file}`)]});
}
const electronDirectory=path.join(root,'apps/desktop/node_modules/electron');
const electronVersion=JSON.parse(fs.readFileSync(path.join(electronDirectory,'package.json'),'utf8')).version;
const checksums=JSON.parse(fs.readFileSync(path.join(electronDirectory,'checksums.json'),'utf8'));
const archivePath=await downloadArtifact({version:electronVersion,artifactName:'electron',platform:process.platform,arch:process.arch,checksums});
const chromiumBytes=await new Promise((resolve,reject)=>{
  yauzl.open(archivePath,{lazyEntries:true},(error,archive)=>{
    if(error){reject(error);return;}
    archive.on('error',reject);
    archive.on('end',()=>reject(new Error('Chromium notices are missing from the pinned Electron archive')));
    archive.on('entry',entry=>{
      if(entry.fileName!=='LICENSES.chromium.html'){archive.readEntry();return;}
      if(entry.uncompressedSize>20*1024*1024){archive.close();reject(new Error('Unexpectedly large Chromium notices'));return;}
      archive.openReadStream(entry,(error,stream)=>{
        if(error){archive.close();reject(error);return;}
        const chunks=[];let bytes=0;
        stream.on('data',chunk=>{bytes+=chunk.length;if(bytes>20*1024*1024)stream.destroy(new Error('Notice limit exceeded'));else chunks.push(chunk);});
        stream.on('error',error=>{archive.close();reject(error);});
        stream.on('end',()=>{archive.close();resolve(Buffer.concat(chunks));});
      });
    });
    archive.readEntry();
  });
});
const chromiumTarget='javascript/electron/LICENSES.chromium.html';
fs.mkdirSync(path.dirname(path.join(output,chromiumTarget)),{recursive:true});
fs.writeFileSync(path.join(output,chromiumTarget),chromiumBytes);
records.push({name:'Chromium and Electron runtime components',version:electronVersion,files:[{file:chromiumTarget,sha256:createHash('sha256').update(chromiumBytes).digest('hex')}]});
fs.writeFileSync(path.join(output,'index.json'),JSON.stringify({generatedFrom:'locked dependencies and installed toolchain',components:records},null,2)+'\n');
console.log(`Collected notices for ${records.length} components.`);
