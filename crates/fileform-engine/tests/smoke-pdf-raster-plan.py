#!/usr/bin/env python3
"""Real DPI planning and exact-size native raster evaluation; not image export."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from pdf_fixtures import fixture, visual_fixture, annotation_fixture
root=Path(__file__).resolve().parents[3]
pdf=Path(sys.argv[1]).resolve();helper=Path(sys.argv[2]).resolve()
cli=root/'target/release'/('fileform-native.exe' if sys.platform=='win32' else 'fileform-native')
worker=root/'target/release'/('fileform-worker.exe' if sys.platform=='win32' else 'fileform-worker')
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=60)
def plan(path,dpi):
    result=run(cli,'plan-pdf-raster',path,pdf,dpi)
    assert result.returncode==0,result.stderr
    return json.loads(result.stdout)
with tempfile.TemporaryDirectory(prefix='fileform-raster-plan-') as folder:
    base=Path(folder)
    source=base/'pages.pdf';fixture(source,text=True)
    original=source.read_bytes()
    planned=plan(source,600)
    assert [(p['width'],p['height']) for p in planned['pages']]==[(1667,2500),(2500,1667)]
    assert planned['source_sha256']==hashlib.sha256(original).hexdigest()
    request={'operation':'plan_pdf_raster','input':str(source),'directory':str(pdf),'dpi':600}
    reply=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,check=True,timeout=60)
    assert json.loads(reply.stdout)['result']==planned
    for p in planned['pages']:
        result=run(helper,source,p['page_index'],f"raster:{p['width']}x{p['height']}",'crop',*p['crop_box'],p['rotation']//90)
        assert result.returncode==0,result.stderr
        _,size,_,pixels=result.stdout.split(b'\n',3)
        assert tuple(map(int,size.split()))==(p['width'],p['height'])
        assert len(pixels)==p['decoded_rgb_bytes']
    inherited=base/'inherited.pdf';fixture(inherited,inherited=True)
    assert [(p['width'],p['height'],p['rotation']) for p in plan(inherited,144)['pages']]==[(600,400,270)]*2
    unit=base/'unit.pdf';visual_fixture(unit,user_unit=2)
    assert [(p['width'],p['height']) for p in plan(unit,72)['pages']]==[(120,120)]
    huge=base/'huge.pdf';visual_fixture(huge,user_unit=75000)
    assert run(cli,'plan-pdf-raster',huge,pdf,600).returncode!=0
    for dpi in [0,35,601]: assert run(cli,'plan-pdf-raster',source,pdf,dpi).returncode!=0
    form=base/'form.pdf';annotation_fixture(form)
    a=run(helper,form,0,540).stdout
    b=run(helper,form,0,'raster:360x540','crop').stdout
    assert a==b  # Exact target retains the established appearance geometry.
    xfa=base/'xfa.pdf';annotation_fixture(xfa,xfa=True)
    assert run(cli,'plan-pdf-raster',xfa,pdf,144).returncode!=0
    for target in ['raster:0x1','raster:16385x1','raster:8001x8000','raster:100x100x2']:
        result=run(helper,source,0,target)
        assert result.returncode!=0 and not result.stdout
    assert source.read_bytes()==original
print('PDF raster planning: explicit DPI, crop/rotation/UserUnit, CLI/worker parity, exact dimensions beyond preview caps, appearance equality, limits and unchanged source passed')
