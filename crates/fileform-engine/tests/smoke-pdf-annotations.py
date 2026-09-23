#!/usr/bin/env python3
"""Static annotations/widgets: screen visibility, crop origin and rotation."""
from pathlib import Path
import subprocess
import sys
import tempfile
from pdf_fixtures import annotation_fixture
helper=Path(sys.argv[1]).resolve()
qpdf=Path(sys.argv[2]).resolve()
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=60)
def raster(path):
    result=run(helper,path,0,540)
    assert result.returncode==0,result.stderr
    _,dimensions,_,pixels=result.stdout.split(b'\n',3)
    w,h=map(int,dimensions.split())
    assert len(pixels)==w*h*3
    return w,h,pixels
def pixel(image,x,y):
    offset=(y*image[0]+x)*3
    return tuple(image[2][offset:offset+3])
with tempfile.TemporaryDirectory(prefix='fileform-annotations-') as folder:
    base=Path(folder)
    for generated in [False,True]:
        source=base/f'forms-{generated}.pdf'
        annotation_fixture(source,generated_widget=generated)
        original=source.read_bytes()
        assert run(qpdf,'--check',source).returncode==0
        reference=raster(source)
        assert reference[:2]==(360,540)
        samples=[(50,170,(255,255,0)),(50,310,(0,255,255)),(50,450,(255,255,255)),(50,380,(255,255,255))]
        for x,y,color in samples: assert pixel(reference,x,y)==color,(generated,x,y,pixel(reference,x,y))
        for rotation in [90,180,270]:
            rotated=base/f'rotated-{rotation}.pdf'
            annotation_fixture(rotated,rotation=rotation,generated_widget=generated)
            image=raster(rotated)
            assert image[:2]==((540,360) if rotation!=180 else (360,540))
            for x,y,color in samples:
                rx,ry={90:(539-y,x),180:(359-x,539-y),270:(y,359-x)}[rotation]
                assert pixel(image,rx,ry)==color,(generated,rotation,x,y,pixel(image,rx,ry))
        revealed=base/'revealed.pdf';annotation_fixture(revealed,reveal_hidden=True,generated_widget=generated)
        image=raster(revealed)
        assert pixel(image,50,450)==(255,0,255)
        assert pixel(image,50,380) in [(255,127,0),(255,128,0)]
        assert source.read_bytes()==original
        rewrite=base/f'rewrite-{generated}.pdf'
        assert run(qpdf,'--object-streams=generate',source,rewrite).returncode==0
        assert raster(rewrite)==reference
    print('PDF annotations: explicit/generated text widgets, free text, Hidden/NoView flags, printable-only hidden exclusion, crop origins, all rotations, rewrite pixels and unchanged sources passed')
