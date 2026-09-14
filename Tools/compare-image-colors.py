#!/usr/bin/env python3
"""macOS reference comparison; requires Pillow for independent output decoding."""
import hashlib
import json
from pathlib import Path
import platform
import struct
import subprocess
import tempfile
import zlib
from PIL import Image

if platform.system() != 'Darwin':
    raise SystemExit('This comparison requires the existing macOS Swift reference CLI.')
root=Path(__file__).resolve().parent.parent
swift=root/'.build/release/fileform'
rust=root/'target/release/fileform-native'
def chunk(kind,data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
with tempfile.TemporaryDirectory(prefix='fileform-color-reference-') as temp:
    folder=Path(temp)
    profile_paths={'srgb':Path('/System/Library/ColorSync/Profiles/sRGB Profile.icc'), 'display-p3':Path('/System/Library/ColorSync/Profiles/Display P3.icc')}
    profiles={name:path.read_bytes() for name,path in profile_paths.items()}
    cases=[]
    for name in ['srgb','display-p3','linear-gamma','orientation-6']:
        width,height=(2,3) if name=='orientation-6' else (2,1)
        pixels=bytes([200,100,50,255,30,160,220,255]) if height==1 else b''.join(bytes([n,0,0,255]) for n in [20,40,60,80,100,120])
        metadata=b''
        if name in profiles: metadata=chunk(b'iCCP',b'Fileform QA\0\0'+zlib.compress(profiles[name]))
        if name=='linear-gamma': metadata=chunk(b'gAMA',struct.pack('>I',100000))
        if name=='orientation-6': metadata=chunk(b'eXIf',b'II'+struct.pack('<HIH',42,8,1)+struct.pack('<HHIHHI',0x112,3,1,6,0,0))
        scanlines=b''.join(b'\0'+pixels[y*width*4:(y+1)*width*4] for y in range(height))
        source=folder/(name+'.png')
        content=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+metadata+chunk(b'IDAT',zlib.compress(scanlines))+chunk(b'IEND',b'')
        source.write_bytes(content)
        native=folder/(name+'-swift.png'); portable=folder/(name+'-rust.png')
        subprocess.run([str(swift),'convert',str(source),'--to','png','--output',str(native),'--json'],capture_output=True,check=True)
        subprocess.run([str(rust),'convert-image',str(source),str(portable)],capture_output=True,check=True)
        with Image.open(native) as n, Image.open(portable) as p:
            a=n.convert('RGBA'); b=p.convert('RGBA')
            assert a.size==b.size
            left=a.tobytes(); right=b.tobytes()
            maximum=max(abs(x-y) for x,y in zip(left,right))
            assert maximum<=2, (name,maximum,list(left),list(right))
            assert left[3::4]==right[3::4]
            cases.append({'case':name,'dimensions':a.size,'maximumChannelDifference':maximum,'alphaMatches':True})
        assert source.read_bytes()==content
    report={'platform':platform.platform(),'scope':'Four opaque PNG fixtures; not complete image/color parity','decoder':'Pillow '+Image.__version__,'tolerance':2,'profileSHA256':{name:hashlib.sha256(data).hexdigest() for name,data in profiles.items()},'cases':cases,'binaries':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in [('swift',swift),('rust',rust)]}}
    target=root/'Artifacts/Verification/native-color-comparison.json';target.parent.mkdir(parents=True,exist_ok=True);target.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(cases))
