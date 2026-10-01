#!/usr/bin/env python3
"""Grid/color/auxiliary/EXIF HEIF cases against the ImageIO reference."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from png_fixture_decode import decode
root=Path(__file__).resolve().parents[3];pack=Path(sys.argv[1]).resolve()
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
helper=pack/'bin'/('fileform-heic-decode'+suffix)
fixtures=root/'native/heic-decode/container-fixtures'
environment={**os.environ,'FILEFORM_HEIC_PACK':str(pack)}
def run(*args):return subprocess.run([str(v) for v in args],capture_output=True,timeout=90,env=environment)
def orient(p,w,h,o):
    nw,nh=(h,w) if o>=5 else (w,h);out=bytearray(nw*nh*4)
    for y in range(h):
        for x in range(w):
            nx,ny={1:(x,y),2:(w-1-x,y),3:(w-1-x,h-1-y),4:(x,h-1-y),5:(y,x),6:(h-1-y,x),7:(h-1-y,w-1-x),8:(y,w-1-x)}[o]
            out[(ny*nw+nx)*4:(ny*nw+nx+1)*4]=p[(y*w+x)*4:(y*w+x+1)*4]
    return bytes(out)
manifest=json.loads((fixtures/'manifest.json').read_text())
for name,expected in manifest['sha256'].items():assert hashlib.sha256((fixtures/name).read_bytes()).hexdigest()==expected
with tempfile.TemporaryDirectory(prefix='fileform-heic-containers-') as folder:
    base=Path(folder)
    for source in sorted(fixtures.glob('*.heic')):
        original=source.read_bytes();output=base/(source.stem+'.png')
        result=run(cli,'convert-image',source,output)
        if source.stem in ['apple-gain','iso-gain','pq','hlg','tone-map-item']:
            assert result.returncode!=0 and not output.exists(),source.name
            rejected=run(helper,source);assert rejected.returncode!=0 and not rejected.stdout
            continue
        assert result.returncode==0,(source.name,result.stderr)
        info=json.loads(run(cli,'inspect-image',source).stdout)
        oracle=json.loads(source.with_suffix('.json').read_text())
        w,h,_,pixels=decode(output,rgba=True)
        size=(oracle['height'],oracle['width']) if oracle['orientation']>=5 else (oracle['width'],oracle['height'])
        assert (w,h)==size and info['orientation']==1
        expected=orient((fixtures/(source.stem+'.srgb.rgba')).read_bytes(),oracle['width'],oracle['height'],oracle['orientation'])
        delta=[abs(a-b) for a,b in zip(pixels,expected)]
        if source.stem=='nclx-709':
            # ImageIO labels this case sRGB. The portable path follows explicit
            # H.273/BT.709 transfer metadata; verify that deliberate difference
            # with an independent scalar transfer calculation.
            payload=run(helper,source).stdout;_,header,body=payload.split(b'\n',2);fields=list(map(int,header.split()))
            native=body[fields[4]+fields[7]:]
            def conversion(value):
                encoded=value/255
                alpha=1.09929682680944;threshold=4.5*.018053968510807
                linear=encoded/4.5 if encoded<threshold else ((encoded+alpha-1)/alpha)**(1/.45)
                return round(255*(12.92*linear if linear<=.0031308 else 1.055*linear**(1/2.4)-.055))
            analytical=[abs(pixels[i]-conversion(value)) for i,value in enumerate(native) if i%4!=3]
            assert max(analytical)<=2 and sum(analytical)/len(analytical)<.2
        else:
            assert max(delta)<=7 and sum(delta)/len(delta)<.5,(source.name,max(delta),sum(delta)/len(delta))
        if source.stem.startswith('nclx'):assert not info['has_icc'] and info['color_interpretation']=='heic_nclx'
        if source.stem=='exif-only':assert oracle['orientation']==1 and (w,h)==(128,96)
        if source.stem=='conflicting-exif':assert oracle['orientation']==3
        if source.stem=='grid':assert (w,h)==(256,192)
        if source.stem=='depth':assert not info['has_alpha']
        assert source.read_bytes()==original
    assert not any(p.is_dir() for p in base.iterdir())
print('HEIF containers: repeated-tile grid, explicit P3/BT709 NCLX, ignored EXIF-only/conflicting EXIF with authoritative HEIF transforms, ordinary depth auxiliary retention, Apple/ISO gain-map and PQ/HLG/tmap rejection, ImageIO and independent transfer references with explicit BT709 difference, unchanged originals passed')
