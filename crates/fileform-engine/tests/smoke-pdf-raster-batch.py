#!/usr/bin/env python3
"""Ordered mixed-source page images and complete-folder failure/cancel handling."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from pdf_fixtures import fixture, annotation_fixture
from png_fixture_decode import decode
root=Path(__file__).resolve().parents[3]
pdf,renderer=[Path(v).resolve() for v in sys.argv[1:]]
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
helper=renderer/'bin'/('fileform-pdf-render'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=180)
def execute(request):
    result=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,timeout=180)
    return json.loads(result.stdout)
def chunk(kind,data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
with tempfile.TemporaryDirectory(prefix='fileform-page-images-') as folder:
    base=Path(folder)
    source=base/'pages.pdf';fixture(source,text=True)
    form=base/'form.pdf';annotation_fixture(form,rotation=90,generated_widget=True)
    image=base/'image.png'
    image.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,3,8,6,0,0,0))+
                     chunk(b'IDAT',zlib.compress((b'\0'+bytes([255,0,0,0,0,255,0,255]))*3))+chunk(b'IEND',b''))
    inputs=[source,form,image];originals=[p.read_bytes() for p in inputs]
    selected=[{'source_index':0,'page_index':1,'clockwise_rotation':90},
              {'source_index':1,'page_index':0,'clockwise_rotation':-90},
              {'source_index':0,'page_index':1}, {'source_index':2,'page_index':0}]
    output=base/'ordered'
    request={'operation':'export_pdf_images','inputs':[str(p) for p in inputs],'output':str(output),
             'directory':str(pdf),'renderer_directory':str(renderer),'dpi':144,'format':'png',
             'pages':selected,'allow_rasterization':True}
    reply=execute(request);assert reply['ok'],reply
    receipt=reply['result'];assert len(receipt['parts'])==4
    assert receipt['source_sha256']==[hashlib.sha256(v).hexdigest() for v in originals]
    for index,(selection,part) in enumerate(zip(selected,receipt['parts'])):
        assert part['name']==f'{index+1:03}.png'
        for key in ['source_index','page_index']:assert part[key]==selection[key]
        path=output/part['name'];w,h,physical,pixels=decode(path)
        assert (w,h)==(part['width'],part['height']) and physical[2]==1
        assert abs(physical[0]*.0254-144)<.03
        assert hashlib.sha256(path.read_bytes()).hexdigest()==part['sha256']
        if index<3:
            plan=json.loads(run(cli,'plan-pdf-raster',inputs[selection['source_index']],pdf,144).stdout)['pages'][selection['page_index']]
            rotation=(plan['rotation']+selection.get('clockwise_rotation',0))%360
            ref=run(helper,inputs[selection['source_index']],selection['page_index'],f'raster:{w}x{h}','crop',*plan['crop_box'],rotation//90)
            assert ref.returncode==0 and bytes(pixels)==ref.stdout.split(b'\n',3)[3]
        else:
            assert (w,h)==(4,6) and pixels[:3]==b'\xff\xff\xff'
    assert (receipt['parts'][0]['width'],receipt['parts'][0]['height'])==(400,600)
    assert (receipt['parts'][2]['width'],receipt['parts'][2]['height'])==(600,400)
    assert not execute(request)['ok']
    empty=base/'existing';empty.mkdir()
    assert not execute({**request,'output':str(empty)})['ok'] and not list(empty.iterdir())
    jpeg=base/'jpeg'
    result=run(cli,'export-pdf-images',jpeg,pdf,renderer,'jpeg',72,source)
    assert result.returncode==0,result.stderr
    assert sorted(p.name for p in jpeg.iterdir())==['001.jpg','002.jpg']
    for page,path in enumerate(sorted(jpeg.iterdir())):
        plain=base/f'jpeg-{page}.jpg'
        assert run(cli,'export-pdf-jpeg',source,plain,pdf,renderer,page,72).returncode==0
        assert run(cli,'convert-image',path,base/f'batch-{page}.png').returncode==0
        assert run(cli,'convert-image',plain,base/f'plain-{page}.png').returncode==0
        assert decode(base/f'batch-{page}.png')[3]==decode(base/f'plain-{page}.png')[3]
    before=set(base.iterdir());rejected=base/'rejected'
    for change in [{'pages':[]},{'pages':[{'source_index':0,'page_index':99}]},
                   {'allow_rasterization':False},{'quality':90},{'dpi':601},
                   {'pages':[{'source_index':0,'page_index':0,'clockwise_rotation':45}]}]:
        assert not execute({**request,**change,'output':str(rejected)})['ok'] and not rejected.exists()
        assert set(base.iterdir())==before
    # Observe a verified first staged image before cancelling or changing an
    # unused bound source. The final folder must never become visible.
    active={**request,'pages':[{'source_index':0,'page_index':0}]*50}
    for action in ['cancel','change-unused']:
        destination=base/action
        process=subprocess.Popen([str(worker)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            process.stdin.write(json.dumps({'request':{**active,'output':str(destination)},'cancel_on_disconnect':True})+'\n');process.stdin.flush()
            deadline=time.monotonic()+30
            while not any(p.is_dir() and (p/'parts/001.png').exists() for p in base.iterdir() if p not in before):
                assert process.poll() is None and time.monotonic()<deadline
                time.sleep(.02)
            assert not destination.exists()
            if action=='cancel':process.stdin.write('cancel\n');process.stdin.flush()
            else:form.write_bytes(originals[1]+b'\n%changed unused source\n')
            # Keep the supervisor alive during work, then wait for completion.
            process.wait(timeout=120)
            stdout=process.stdout.read();stderr=process.stderr.read()
            reply=json.loads(stdout)
            assert not reply['ok'] and reply['error']['code']==('cancelled' if action=='cancel' else 'source_changed'),(reply,stderr)
        finally:
            if process.poll() is None:process.kill();process.wait()
            process.stdin.close();process.stdout.close();process.stderr.close()
            form.write_bytes(originals[1])
        assert not destination.exists() and set(base.iterdir())==before
    assert [p.read_bytes() for p in inputs]==originals
print('Page images: ordered/duplicate/rotated selections, PDF/image inputs, PNG/JPEG parity, DPI/provenance, exclusive folder publication, invalid requests, cancellation and unused-source change cleanup passed')
