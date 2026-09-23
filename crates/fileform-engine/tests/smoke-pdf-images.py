#!/usr/bin/env python3
"""Image-to-PDF orientation, alpha, color declarations and mixed assembly."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib
from pdf_fixtures import fixture
root = Path(__file__).resolve().parents[3]
pdf, renderer = [Path(v).resolve() for v in sys.argv[1:]]
suffix = '.exe' if sys.platform == 'win32' else ''
cli = root/'target/release'/('fileform-native'+suffix)
helper = renderer/'bin'/('fileform-pdf-render'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=180)
def chunk(kind,data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
def png(path,pixels,orientation=1,depth=8):
    exif=b'II'+struct.pack('<HIH',42,8,1)+struct.pack('<HHIHHI',0x112,3,1,orientation,0,0)
    stride=2*4*(depth//8)
    scanlines=b''.join(b'\0'+pixels[i*stride:(i+1)*stride] for i in range(3))
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,3,depth,6,0,0,0))+
                     chunk(b'eXIf',exif)+chunk(b'IDAT',zlib.compress(scanlines))+chunk(b'IEND',b''))
with tempfile.TemporaryDirectory(prefix='fileform-pdf-images-') as folder:
    base=Path(folder)
    source_pdf=base/'text.pdf';fixture(source_pdf,text=True)
    inputs=[]
    values=[40,70,100,130,160,190]
    pixels=b''.join(bytes([v,0,0,255]) for v in values)
    layouts=['ABCDEF','BADCFE','FEDCBA','EFCDAB','ACEBDF','ECAFDB','FDBECA','BDFACE']
    for orientation in range(1,9):
        image=base/f'orientation-{orientation}.png'
        png(image,pixels,orientation)
        inputs.append(image)
    alpha=base/'alpha.png'
    png(alpha,bytes([255,0,0,0,0,0,255,128,0,255,0,255,255,255,255,255,0,0,0,255,255,0,0,255]))
    inputs.append(alpha)
    for extension,options in [('jpg',['--background','white']),('tiff',[])]:
        converted=base/f'converted.{extension}'
        result=run(cli,'convert-image',inputs[0],converted,*options)
        assert result.returncode==0,result.stderr
        inputs.append(converted)
    originals=[p.read_bytes() for p in inputs]
    output=base/'assembled.pdf'
    result=run(cli,'merge-pdf',output,pdf,renderer,*inputs,source_pdf)
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    assert receipt['pages']==13 and any('sRGB' in v for v in receipt['warnings'])
    geometry=json.loads(run(cli,'inspect-pdf-pages',output,pdf).stdout)['pages']
    for page,layout in enumerate(layouts):
        w,h=(2,3) if page<4 else (3,2)
        assert geometry[page]['media_box']==[0,0,w,h]
        # Render 1:1: enlarged PDF previews may interpolate samples.
        raster=run(helper,output,page,3)
        assert raster.returncode==0,raster.stderr
        magic,dimensions,maximum,data=raster.stdout.split(b'\n',3)
        rw,rh=map(int,dimensions.split())
        assert (rw,rh)==(w,h)
        for i,letter in enumerate(layout):
            x,y=i%w,i//w
            actual=data[(y*rw+x)*3:(y*rw+x)*3+3]
            expected=values[ord(letter)-ord('A')]
            assert abs(actual[0]-expected)<=1 and actual[1]<=1 and actual[2]<=1,(page,i,list(actual),expected)
    raster=run(helper,output,8,3)
    data=raster.stdout.split(b'\n',3)[3]
    assert data[:3]==b'\xff\xff\xff'
    assert all(abs(a-b)<=1 for a,b in zip(data[3:6],[127,127,255]))
    assert [p.read_bytes() for p in inputs]==originals
    assert receipt['sha256']==hashlib.sha256(output.read_bytes()).hexdigest()
    high=base/'16-bit.png';png(high,bytes([0,0,0,0,0,0,255,255])*6,depth=16)
    rejected=base/'rejected.pdf'
    assert run(cli,'merge-pdf',rejected,pdf,renderer,high).returncode!=0 and not rejected.exists()
print('PDF images: all eight EXIF orientations, page sizes, sRGB colors, transparency, mixed PDF/image merge, hashes, unchanged sources and high-depth rejection passed')
