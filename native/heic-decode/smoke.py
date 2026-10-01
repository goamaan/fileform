#!/usr/bin/env python3
"""Real ImageIO-generated HEIC fixtures and independently oriented pixel oracles."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
helper=Path(sys.argv[1]).resolve();fixtures=Path(sys.argv[2]).resolve()
def run(source):return subprocess.run([str(helper),str(source)],capture_output=True,timeout=30)
def orient(pixels,w,h,orientation):
    nw,nh=(h,w) if orientation>=5 else (w,h);output=bytearray(nw*nh*4)
    for y in range(h):
        for x in range(w):
            nx,ny={1:(x,y),2:(w-1-x,y),3:(w-1-x,h-1-y),4:(x,h-1-y),5:(y,x),6:(h-1-y,x),7:(h-1-y,w-1-x),8:(y,w-1-x)}[orientation]
            output[(ny*nw+nx)*4:(ny*nw+nx+1)*4]=pixels[(y*w+x)*4:(y*w+x+1)*4]
    return bytes(output)
manifest=json.loads((fixtures/'manifest.json').read_text())
for name,digest in manifest['sha256'].items():assert hashlib.sha256((fixtures/name).read_bytes()).hexdigest()==digest
for source in sorted(fixtures.glob('*.heic')):
    receipt=json.loads(source.with_suffix('.json').read_text());result=run(source)
    if receipt['count']>1:
        assert result.returncode!=0 and result.stdout==b'' and b'single HEIC' in result.stderr;continue
    assert result.returncode==0,result.stderr
    magic,header,body=result.stdout.split(b'\n',2);assert magic==b'FH1'
    width,height,alpha,premultiplied,icc_size,primaries,transfer,exif_size,transformed=map(int,header.split())
    expected_size=(receipt['height'],receipt['width']) if receipt['orientation']>=5 else (receipt['width'],receipt['height'])
    assert (width,height)==expected_size and alpha==int(receipt['alpha']) and premultiplied in (0,1)
    assert 0<=icc_size<=4*1024*1024 and 0<=exif_size<=65536 and transformed in (0,1)
    assert len(body)==icc_size+exif_size+width*height*4 and transfer not in (16,18)
    if source.stem=='p3':assert icc_size>0 and body[36:40]==b'acsp'
    pixels=body[icc_size+exif_size:]
    expected=orient(source.with_suffix('.rgba').read_bytes(),receipt['width'],receipt['height'],receipt['orientation'])
    if not premultiplied and alpha:pixels=bytes(value if i%4==3 else (value*pixels[i//4*4+3]+127)//255 for i,value in enumerate(pixels))
    difference=[abs(a-b) for a,b in zip(pixels,expected)]
    assert max(difference)<=2 and sum(difference)/len(difference)<.5,(source.name,max(difference))
with tempfile.TemporaryDirectory(prefix='fileform-heic-helper-') as folder:
    base=Path(folder)
    unicode=base/'résumé 日本語.heic';shutil.copy2(fixtures/'srgb.heic',unicode)
    assert run(unicode).returncode==0
    for name,data in [('empty',b''),('invalid',b'not HEIC'),('truncated',(fixtures/'srgb.heic').read_bytes()[:128])]:
        source=base/(name+'.heic');source.write_bytes(data)
        result=run(source);assert result.returncode!=0 and result.stdout==b'',(name,result.stderr)
print('HEIC helper: real sRGB/Display-P3/alpha, eight orientation transforms, ImageIO pixels within two levels, retained ICC and EXIF framing, Unicode paths, multiple-image/malformed/truncated rejection passed; Rust adapter and wider gain-map/depth/grid acceptance remain open')
