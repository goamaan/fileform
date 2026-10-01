#!/usr/bin/env python3
"""Build pinned HEIC decoder dependencies for evaluation; no app integration claim."""
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
parser.add_argument('--work',type=Path,required=True)
parser.add_argument('--jobs',type=int,default=4)
args=parser.parse_args()
if sys.platform not in ('darwin','win32'):raise RuntimeError('Use macOS or Windows')
if not 1<=args.jobs<=16:raise ValueError('Choose 1–16 build jobs')
work=args.work.resolve()
if work.exists():raise FileExistsError('Use a new directory to avoid mixing dependency state')
work.mkdir(parents=True)
pins=[('libde265','1.1.3','554228bd17788c99a7e63b37ab5634722190e6e2bf60c1dcb01cef328e133905'),
      ('libheif','1.23.5','fd9036064c4432f0550d15072ddf34956a248279ee9aeaff0fba3fa0f77d8f1a')]
def digest(path):
    value=hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda:source.read(65536),b''):value.update(chunk)
    return value.hexdigest()
def source(name,version,expected):
    archive=work/(name+'-'+version+'.tar.gz')
    with urllib.request.urlopen('https://github.com/strukturag/'+name+'/releases/download/v'+version+'/'+archive.name,timeout=60) as response,archive.open('wb') as output:
        total=0
        for chunk in iter(lambda:response.read(65536),b''):
            total+=len(chunk)
            if total>64*1024*1024:raise RuntimeError('Source download exceeds 64 MiB')
            output.write(chunk)
    if digest(archive)!=expected:raise RuntimeError('Source hash mismatch: '+archive.name)
    with tarfile.open(archive) as compressed:
        members=compressed.getmembers()
        if len(members)>100000 or sum(v.size for v in members)>512*1024*1024:raise RuntimeError('Source extraction exceeds limits')
        for member in members:
            path=PurePosixPath(member.name)
            if path.is_absolute() or '..' in path.parts or '\\' in member.name or ':' in member.name or not(member.isfile() or member.isdir()):raise RuntimeError('Archive entry needs review: '+member.name)
        compressed.extractall(work,members=members)
    return work/(name+'-'+version)
commands=[]
def run(command,label):
    commands.append(command);(work/'commands.json').write_text(json.dumps(commands,indent=2),encoding='utf-8')
    with (work/(label+'.log')).open('wb') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
prefix=work/'install';windows=sys.platform=='win32'
common=['-DCMAKE_BUILD_TYPE=Release',f'-DCMAKE_INSTALL_PREFIX={prefix}','-DBUILD_SHARED_LIBS=ON']
common+=['-A','x64','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded','-DCMAKE_POLICY_DEFAULT_CMP0091=NEW'] if windows else ['-G','Ninja','-DCMAKE_OSX_DEPLOYMENT_TARGET=13.0']
def build(path,label,flags):
    directory=work/(label+'-build')
    run(['cmake','-S',str(path),'-B',str(directory),*common,*flags],label+'-configure')
    run(['cmake','--build',str(directory),'--config','Release','--parallel',str(args.jobs)],label+'-build')
    run(['cmake','--install',str(directory),'--config','Release'],label+'-install')
sources={name:source(name,version,sha) for name,version,sha in pins}
build(sources['libde265'],'de265',[f'-D{option}=OFF' for option in ['ENABLE_DECODER','ENABLE_ENCODER','ENABLE_SDL','ENABLE_SHERLOCK265','ENABLE_INTERNAL_DEVELOPMENT_TOOLS','WITH_FUZZERS']])
include=prefix/'include'
if not (include/'libde265/de265.h').is_file():raise RuntimeError('Missing installed codec headers')
codec=next((p for p in [prefix/'lib/de265.lib',prefix/'lib/libde265.lib',prefix/'lib/libde265.dylib'] if p.is_file()),None)
if codec is None:raise RuntimeError('Missing pinned codec library')
disabled=['X265','KVAZAAR','UVG266','VVDEC','VVENC','X264','OpenH264_DECODER','DAV1D','AOM_DECODER','AOM_ENCODER','SvtEnc','RAV1E','JPEG_DECODER','JPEG_ENCODER','OpenJPEG_DECODER','OpenJPEG_ENCODER','FFMPEG_DECODER','OPENJPH_ENCODER','UNCOMPRESSED_CODEC','WEBCODECS','LIBSHARPYUV','EXAMPLES','GDK_PIXBUF','HEADER_COMPRESSION','FUZZERS']
flags=[f'-DCMAKE_PREFIX_PATH={prefix}',f'-DLIBDE265_INCLUDE_DIR={include}',f'-DLIBDE265_LIBRARY={codec}',
       '-DWITH_LIBDE265=ON','-DWITH_LIBDE265_PLUGIN=OFF','-DENABLE_PLUGIN_LOADING=OFF',
       '-DENABLE_PARALLEL_TILE_DECODING=OFF','-DBUILD_TESTING=OFF','-DBUILD_DEVELOPMENT_TOOLS=OFF','-DBUILD_DOCUMENTATION=OFF',
       *[f'-DWITH_{option}=OFF' for option in disabled]]
build(sources['libheif'],'heif',flags)
licenses=work/'licenses';licenses.mkdir()
for name,path in sources.items():
    for file in path.glob('COPYING*'):
        if file.is_file():shutil.copy2(file,licenses/(name+'-'+file.name))
for name in ['de265','heif']:
    cache=(work/(name+'-build/CMakeCache.txt')).read_text(encoding='utf-8')
    (work/(name+'-cache.txt')).write_text(cache,encoding='utf-8')
manifest={'evaluationOnly':True,'architecture':'x86_64' if windows else platform.machine().lower(),
          'sourceArchives':{name+'-'+version+'.tar.gz':sha for name,version,sha in pins},
          'decoderPlugins':False,'dependencies':{'libheif':'1.23.5','libde265':'1.1.3'},
          'integrationStatus':'Dependencies only; helper, runtime, color/orientation/alpha and distribution acceptance remain open.'}
(work/'evaluation.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(prefix)
