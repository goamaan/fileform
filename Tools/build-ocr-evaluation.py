#!/usr/bin/env python3
"""Build an evaluation-only native OCR pack from pinned sources, not system installs."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--work',type=Path,required=True,help='New build directory')
parser.add_argument('--jobs',type=int,default=4)
args=parser.parse_args()
if sys.platform not in ('darwin','win32'): raise RuntimeError('Evaluation currently targets macOS and Windows')
if not 1 <= args.jobs <= 16: raise ValueError('Choose 1–16 build jobs')
work=args.work.resolve()
if work.exists(): raise FileExistsError('Use a new directory to avoid mixing build/source state')
work.mkdir(parents=True)
def digest(path):
    value=hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda:stream.read(65536),b''): value.update(chunk)
    return value.hexdigest()
def download(url,name,expected):
    path=work/name
    with urllib.request.urlopen(url,timeout=60) as response,path.open('wb') as output:
        total=0
        while True:
            chunk=response.read(65536)
            if not chunk: break
            total+=len(chunk)
            if total>64*1024*1024: raise RuntimeError('Source/model download exceeds 64 MiB')
            output.write(chunk)
    if digest(path)!=expected: raise RuntimeError(f'Hash mismatch: {name}')
    return path
def extract(archive):
    with tarfile.open(archive) as source:
        members=source.getmembers()
        if sum(member.size for member in members)>1024*1024*1024: raise RuntimeError('Source archive exceeds extraction budget')
        for member in members:
            name=PurePosixPath(member.name)
            if name.is_absolute() or '..' in name.parts or chr(92) in member.name or ':' in member.name: raise RuntimeError('Unsafe archive path')
            if not (member.isfile() or member.isdir()): raise RuntimeError('Archive links/special files need explicit review')
        source.extractall(work,members=members)
    return work/PurePosixPath(members[0].name).parts[0]
lept_archive=download('https://github.com/DanBloomberg/leptonica/releases/download/1.87.0/leptonica-1.87.0.tar.gz','leptonica-1.87.0.tar.gz','c73363397f96eb1295602bf44d708a994ad42046c791bf03ea0505d829bdb6a7')
tess_archive=download('https://codeload.github.com/tesseract-ocr/tesseract/tar.gz/db0ec62f81b0737fbbe184d8fea40af5738f8eef','tesseract-5.5.3.tar.gz','21dbaff9867d937c156d0b898e90b726aa339129cfebd7e1365cd5fee5046d67')
lept,tess=extract(lept_archive),extract(tess_archive)
prefix=work/'install'
windows=sys.platform=='win32'
common=['-DCMAKE_BUILD_TYPE=Release',f'-DCMAKE_INSTALL_PREFIX={prefix}','-DBUILD_SHARED_LIBS=OFF']
common+=['-A','x64','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded','-DCMAKE_POLICY_DEFAULT_CMP0091=NEW'] if windows else ['-G','Ninja','-DCMAKE_OSX_DEPLOYMENT_TARGET=13.0']
commands=[]
def run(command,label):
    commands.append(command)
    (work/'commands.json').write_text(json.dumps(commands,indent=2),encoding='utf-8')
    with (work/(label+'.log')).open('wb') as log:
        subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
def build(source,name,flags):
    directory=work/(name+'-build')
    run(['cmake','-S',str(source),'-B',str(directory),*common,*flags],name+'-configure')
    run(['cmake','--build',str(directory),'--config','Release','--parallel',str(args.jobs)],name+'-build')
    run(['cmake','--install',str(directory),'--config','Release'],name+'-install')
build(lept,'lept',['-DBUILD_PROG=OFF','-DSW_BUILD=OFF',*[f'-DENABLE_{codec}=OFF' for codec in ['ZLIB','PNG','GIF','JPEG','TIFF','WEBP','OPENJPEG']]])
build(tess,'tess',[f'-DCMAKE_PREFIX_PATH={prefix}','-DBUILD_TRAINING_TOOLS=OFF','-DBUILD_TESTS=OFF','-DOPENMP_BUILD=OFF','-DGRAPHICS_DISABLED=ON','-DENABLE_NATIVE=OFF','-DDISABLE_CURL=ON','-DDISABLE_ARCHIVE=ON','-DDISABLE_TIFF=ON','-DINSTALL_CONFIGS=OFF', '-DCMAKE_CXX_FLAGS='+('/DTESSERACT_DISABLE_DEBUG_FONTS' if windows else '-DTESSERACT_DISABLE_DEBUG_FONTS')] + (['-DWIN32_MT_BUILD=ON'] if windows else []))
models={
    'eng.traineddata':'7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2',
    'osd.traineddata':'9cf5d576fcc47564f11265841e5ca839001e7e6f38ff7f7aacf46d15a96b00ff',
}
model_base='https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd/'
pack=work/'OCRPack'
for directory in ['bin','tessdata','licenses','sources']: (pack/directory).mkdir(parents=True)
for name,expected in models.items(): shutil.copy2(download(model_base+name,name,expected),pack/'tessdata'/name)
executable='tesseract.exe' if windows else 'tesseract'
shutil.copy2(prefix/'bin'/executable,pack/'bin'/executable)
shutil.copy2(lept/'leptonica-license.txt',pack/'licenses/leptonica.txt')
shutil.copy2(tess/'LICENSE',pack/'licenses/tesseract.txt')
# Model repository uses Apache-2.0; preserve its exact pinned license.
shutil.copy2(download(model_base+'LICENSE','tessdata-license.txt','cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30'),pack/'licenses/tessdata_fast.txt')
for archive in [lept_archive,tess_archive]: shutil.copy2(archive,pack/'sources'/archive.name)
manifest={'schemaVersion':1,'id':'app.fileform.ocr','version':'5.5.3-lept1.87.0-evaluation.1',
          'architecture':'x86_64' if windows else platform.machine().lower(),'evaluationOnly':True,
          'networkProtocols':False,'executables':{'tesseract':digest(pack/'bin'/executable)},
          'assets':{'tessdata/'+name:hash for name,hash in models.items()},'languages':['eng'],
          'limitations':['No automatic language detection','No engine adapter or process containment acceptance yet']}
(pack/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(pack)
