#!/usr/bin/env python3
"""Stage a relocatable local evaluation HEIC pack; does not sign a release."""
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
build,dependency,destination=[Path(v).resolve() for v in sys.argv[1:]]
assert not destination.exists(),'Use a new pack directory'
evidence=json.loads((dependency/'evaluation.json').read_text())
assert evidence['evaluationOnly'] and not evidence['decoderPlugins']
assert evidence['dependencies']=={'libheif':'1.23.5','libde265':'1.1.3'}
windows=sys.platform=='win32';helper='fileform-heic-decode'+('.exe' if windows else '')
libraries=['heif.dll','libde265.dll'] if windows else ['libheif.1.dylib','libde265.0.dylib']
destination.mkdir(parents=True);(destination/'bin').mkdir()
shutil.copy2(build/helper,destination/'bin'/helper)
for name in libraries:
    source=dependency/'install'/('bin' if windows else 'lib')/name
    shutil.copy2(source.resolve(),destination/'bin'/name)
if not windows:
    output=subprocess.run(['otool','-l',str(destination/'bin'/helper)],capture_output=True,text=True,check=True).stdout.splitlines()
    for index,line in enumerate(output):
        if line.strip()=='cmd LC_RPATH':
            path=output[index+2].strip().split('path ',1)[1].split(' (offset',1)[0]
            if path!='@executable_path':subprocess.run(['install_name_tool','-delete_rpath',path,str(destination/'bin'/helper)],check=True)
    subprocess.run(['install_name_tool','-add_rpath','@executable_path',str(destination/'bin'/helper)],check=True)
    for name in libraries+[helper]:subprocess.run(['codesign','--force','--sign','-',str(destination/'bin'/name)],check=True)
shutil.copytree(dependency/'licenses',destination/'licenses')
(destination/'sources').mkdir()
for name,expected in evidence['sourceArchives'].items():
    source=dependency/name
    assert hashlib.sha256(source.read_bytes()).hexdigest()==expected
    shutil.copy2(source,destination/'sources'/name)
for name in ['commands.json','evaluation.json','de265-cache.txt','heif-cache.txt']:shutil.copy2(dependency/name,destination/name)
def digest(name):return hashlib.sha256((destination/'bin'/name).read_bytes()).hexdigest()
(destination/'manifest.json').write_text(json.dumps({'schemaVersion':1,'id':'app.fileform.heic',
    'version':'heif1.23.5-de2651.1.3-helper1-evaluation','architecture':platform.machine().lower(),
    'evaluationOnly':True,'networkProtocols':False,'executables':{'fileform-heic-decode':digest(helper)},
    'libraries':{name:digest(name) for name in libraries}},indent=2)+'\n',encoding='utf-8')
print(destination)
