#!/usr/bin/env python3
"""Real image OCR via Rust, including Unicode paths, alpha and model isolation."""
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve()
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=90)
def chunk(kind,data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
def png(path,w,h,rgb,orientation=1,transparent=False):
    if transparent: rgb=b''.join(rgb[i:i+3]+b'\0' for i in range(0,len(rgb),3))
    channels=4 if transparent else 3
    scan=b''.join(b'\0'+rgb[y*w*channels:(y+1)*w*channels] for y in range(h))
    exif=b'II'+struct.pack('<HIH',42,8,1)+struct.pack('<HHIHHI',0x112,3,1,orientation,0,0)
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,6 if transparent else 2,0,0,0))+chunk(b'eXIf',exif)+chunk(b'IDAT',zlib.compress(scan))+chunk(b'IEND',b''))
raw=gzip.decompress((Path(__file__).parent/'fixtures/ocr-page.ppm.gz').read_bytes())
_,dimensions,_,rgb=raw.split(b'\n',3)
w,h=map(int,dimensions.split())
with tempfile.TemporaryDirectory(prefix='fileform-ocr-image-') as folder:
    base=Path(folder)
    local_pack=base/'模型'
    shutil.copytree(pack/'bin',local_pack/'bin');shutil.copytree(pack/'tessdata',local_pack/'tessdata')
    shutil.copy2(pack/'manifest.json',local_pack/'manifest.json')
    source=base/'图像.png';png(source,w,h,rgb)
    original=source.read_bytes()
    output=base/'识别.txt'
    result=run(cli,'ocr-image',source,output,local_pack,'eng')
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    expected=b'Fileform page 1\n'
    assert output.read_bytes()==expected
    assert receipt['ocr_performed'] and receipt['language']=='eng' and not receipt['automatic_language_detection']
    assert receipt['source_sha256']==hashlib.sha256(original).hexdigest()
    assert receipt['sha256']==hashlib.sha256(expected).hexdigest() and receipt['bytes']==len(expected)
    assert (receipt['raster_width'],receipt['raster_height'])==(w,h)
    assert run(cli,'ocr-image',source,output,local_pack,'eng').returncode!=0 and output.read_bytes()==expected
    request={'operation':'ocr_image','input':str(source),'output':str(base/'worker.txt'),'directory':str(local_pack),'language':'eng'}
    response=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,check=True,timeout=90)
    assert json.loads(response.stdout)['result']['sha256']==receipt['sha256']
    # Stored counterclockwise pixels plus EXIF clockwise orientation restore the scan.
    turned=bytearray(len(rgb))
    for y in range(h):
        for x in range(w):
            offset=((w-1-x)*h+y)*3
            turned[offset:offset+3]=rgb[(y*w+x)*3:(y*w+x)*3+3]
    rotated=base/'rotated.png';png(rotated,h,w,turned,orientation=6)
    rotated_output=base/'rotated.txt'
    result=run(cli,'ocr-image',rotated,rotated_output,local_pack,'eng')
    assert result.returncode==0 and rotated_output.read_bytes()==expected,result.stderr
    hidden=base/'hidden.png';png(hidden,w,h,rgb,transparent=True)
    rejected=base/'rejected.txt'
    result=run(cli,'ocr-image',hidden,rejected,local_pack,'eng')
    assert result.returncode!=0 and b'no_text' in result.stderr and not rejected.exists()
    for extension,options in [('jpg',[]),('tiff',[])]:
        image=base/f'scan.{extension}'
        assert run(cli,'convert-image',source,image,*options).returncode==0
        text=base/f'{extension}.txt'
        result=run(cli,'ocr-image',image,text,local_pack,'eng')
        assert result.returncode==0 and text.read_bytes()==expected,result.stderr
    model=local_pack/'tessdata/eng.traineddata'
    with model.open('ab') as stream:stream.write(b'changed')
    assert run(cli,'ocr-image',source,rejected,local_pack,'eng').returncode!=0 and not rejected.exists()
    assert source.read_bytes()==original
print('Image OCR: PNG/JPEG/TIFF, real text/hash receipts, CLI/worker parity, Unicode image/pack/output paths, orientation, invisible-text exclusion, collision/model-tamper rejection and source preservation passed')
