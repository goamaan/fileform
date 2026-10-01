#!/usr/bin/env python3
"""Stage reviewed native tools into one self-contained desktop runtime.

Only declared executables/libraries/assets, notices and source archives are copied.
Build caches/logs, credentials and arbitrary contents are never imported wholesale.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--worker',type=Path,required=True)
parser.add_argument('--cli',type=Path,required=True)
for name in ['media','pdf','renderer','ocr','heic']:parser.add_argument('--'+name,type=Path,required=True)
parser.add_argument('--destination',type=Path,required=True)
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
windows=sys.platform=='win32';suffix='.exe' if windows else ''
destination=args.destination.resolve()
if destination.exists():raise FileExistsError('Use a new desktop runtime directory')
destination.parent.mkdir(parents=True,exist_ok=True)
cli=args.cli.resolve();worker=args.worker.resolve()
if not cli.is_file() or not worker.is_file():raise FileNotFoundError('Build the native CLI and worker first')
expected_arch={'arm64':'aarch64','amd64':'x86_64'}.get(platform.machine().lower(),platform.machine().lower())
expected={
 'media':('app.fileform.media',['ffmpeg','ffprobe'],[],[]),
 'pdf':('app.fileform.pdf',['qpdf'],[],[]),
 'renderer':('app.fileform.pdf-render',['fileform-pdf-render'],['pdfium.dll' if windows else 'libpdfium.dylib'],[]),
 'ocr':('app.fileform.ocr',['tesseract'],[],['tessdata/eng.traineddata','tessdata/osd.traineddata']),
 'heic':('app.fileform.heic',['fileform-heic-decode'],['heif.dll','libde265.dll'] if windows else ['libheif.1.dylib','libde265.0.dylib'],[]),
}
def hash_file(path):
 value=hashlib.sha256()
 with path.open('rb') as source:
  for chunk in iter(lambda:source.read(65536),b''):value.update(chunk)
 return value.hexdigest()
def contained(root,path):
 result=path.resolve(strict=True)
 if root!=result and root not in result.parents:raise RuntimeError('A runtime input escapes its reviewed directory')
 if not result.is_file():raise RuntimeError('Expected a regular runtime input')
 return result
copied=0
inventory={}
def copy(source,target,limit=512*1024*1024):
 global copied
 if source.stat().st_size>limit:raise RuntimeError('Runtime file exceeds its copy bound')
 copied+=source.stat().st_size
 if copied>1024*1024*1024:raise RuntimeError('Desktop runtime exceeds 1 GiB')
 target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target)
 inventory[target.relative_to(staging).as_posix()]=hash_file(target)
def copy_tree(source,target):
 files=sorted(source.rglob('*'))
 if len(files)>4096:raise RuntimeError('Too many retained notice/source files')
 for file in files:
  if file.is_dir():
   if file.is_symlink():raise RuntimeError('Do not import linked directories')
   continue
  copy(contained(source.resolve(),file),target/file.relative_to(source),256*1024*1024)
def run(command):
 result=subprocess.run([str(v) for v in command],capture_output=True,timeout=180)
 if result.returncode:raise RuntimeError('Native runtime verification failed: '+result.stderr.decode('utf-8',errors='replace')[:2000])
 return json.loads(result.stdout)
with tempfile.TemporaryDirectory(prefix='fileform-runtime-stage-',dir=destination.parent) as folder:
 staging=Path(folder)/'payload';staging.mkdir()
 copy(worker,staging/('fileform-worker'+suffix));copy(cli,staging/('fileform-native'+suffix))
 packs={};minimum_macos='13.0'
 for name,(identity,executables,libraries,assets) in expected.items():
  source=getattr(args,name).resolve();manifest_path=contained(source,source/'manifest.json')
  if manifest_path.stat().st_size>65536:raise RuntimeError('Oversized pack manifest')
  manifest=json.loads(manifest_path.read_text(encoding='utf-8'))
  arch={'arm64':'aarch64','amd64':'x86_64'}.get(manifest.get('architecture'),manifest.get('architecture'))
  if manifest.get('schemaVersion')!=1 or manifest.get('id')!=identity or arch!=expected_arch or manifest.get('networkProtocols') is True:raise RuntimeError('Missing or incompatible '+name+' pack')
  target=staging/name;target.mkdir();copy(manifest_path,target/'manifest.json',65536)
  declared={}
  if set(manifest.get('executables',{}))!=set(executables) or set(manifest.get('libraries',{}))!=set(libraries):raise RuntimeError('Unexpected native executable/library inventory')
  for executable in executables:declared['bin/'+executable+suffix]=manifest['executables'][executable]
  for library in libraries:declared['bin/'+library]=manifest['libraries'][library]
  asset_map=manifest.get('assets',{})
  if not set(assets)<=set(asset_map):raise RuntimeError('Missing required model assets')
  for relative,digest in asset_map.items():
   path=Path(relative)
   if path.is_absolute() or any(v in ('..','.') for v in path.parts) or '\\' in relative or ':' in relative:raise RuntimeError('Unsafe declared asset path')
   declared[relative]=digest
  for relative,digest in declared.items():
   file=contained(source,source/relative)
   if not isinstance(digest,str) or hash_file(file)!=digest:raise RuntimeError('Native input hash mismatch: '+name+'/'+relative)
   copy(file,target/relative,64*1024*1024 if relative in asset_map else 512*1024*1024)
   if hash_file(target/relative)!=digest:raise RuntimeError('Copied native input changed')
  licenses=source/'licenses'
  if not licenses.is_dir() or not any(licenses.iterdir()):raise RuntimeError('Missing native component licenses: '+name)
  copy_tree(licenses,target/'licenses')
  for file in ['LICENSE','NOTICE','LICENSE.txt','NOTICE.txt','args.gn','VERSION']:
   if (source/file).is_file():copy(contained(source,source/file),target/file,20*1024*1024)
  if (source/'sources').is_dir():copy_tree(source/'sources',target/'sources')
  version=manifest.get('minimumMacOS','13.0')
  if not windows and tuple(map(int,version.split('.')))>tuple(map(int,minimum_macos.split('.'))):minimum_macos=version
  packs[name]={'id':identity,'version':manifest['version'],'directory':name,'evaluationOnly':bool(manifest.get('evaluationOnly') or 'evaluation' in manifest['version'])}
 # Provide reviewed helper/build sources for component rebuilds without importing
 # local caches, commands containing machine paths, or private repository history.
 recipes=['native/pdf-render/main.cpp','native/pdf-render/CMakeLists.txt','native/heic-decode/main.cpp','native/heic-decode/CMakeLists.txt','Tools/build-ocr-evaluation.py','Tools/build-heic-evaluation.py','crates/fileform-engine/tools/build-media-windows.sh','crates/fileform-engine/tools/build-pdf-windows.sh']
 for relative in recipes:copy(root/relative,staging/'rebuild'/relative,1024*1024)
 for name,command in [('media','verify-media-pack'),('pdf','verify-pdf-pack'),('ocr','verify-ocr-pack'),('heic','verify-heic-pack')]:run([staging/('fileform-native'+suffix),command,staging/name])
 # Load the renderer's actual shared library using an owned one-page fixture.
 objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Resources << >> /Contents 4 0 R >>',b'<< /Length 0 >>\nstream\n\nendstream']
 pdf=bytearray(b'%PDF-1.4\n');offsets=[0]
 for i,obj in enumerate(objects,1):offsets.append(len(pdf));pdf+=f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n'
 start=len(pdf);pdf+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode()
 for offset in offsets[1:]:pdf+=f'{offset:010} 00000 n \n'.encode()
 pdf+=f'trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode()
 source=Path(folder)/'probe.pdf';source.write_bytes(pdf)
 run([staging/('fileform-native'+suffix),'render-pdf-page',source,Path(folder)/'probe.png',staging/'renderer',0])
 (staging/'runtime.json').write_text(json.dumps({'schemaVersion':1,'architecture':expected_arch,'platform':sys.platform,'minimumMacOS':None if windows else minimum_macos,'packs':packs,'files':inventory,'releaseAccepted':False},indent=2)+'\n',encoding='utf-8')
 staging.rename(destination)
print(destination)
