#!/usr/bin/env python3
"""JPEG page exports: JFIF/ICC, independent pixel decode and lossy quality."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
from pdf_fixtures import fixture, visual_fixture, annotation_fixture
from png_fixture_decode import decode
root=Path(__file__).resolve().parents[3]
pdf,renderer=[Path(v).resolve() for v in sys.argv[1:]]
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
helper=renderer/'bin'/('fileform-pdf-render'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=120)
def markers(path,dpi):
    data=path.read_bytes();assert data[:2]==b'\xff\xd8' and data[-2:]==b'\xff\xd9'
    offset=2;icc={};total=None;jfif=False
    while offset<len(data):
        assert data[offset]==255
        marker=data[offset+1];length=struct.unpack('>H',data[offset+2:offset+4])[0]
        value=data[offset+4:offset+2+length];offset+=2+length
        if marker==0xda:break
        if marker==0xe0 and value.startswith(b'JFIF\0'):
            assert value[7]==1 and struct.unpack('>HH',value[8:12])==(dpi,dpi)
            jfif=True
        if marker==0xe2 and value.startswith(b'ICC_PROFILE\0'):
            assert value[12] not in icc
            total=value[13];icc[value[12]]=value[14:]
    assert jfif and total and set(icc)==set(range(1,total+1))
    profile=b''.join(icc[i] for i in range(1,total+1))
    assert profile[16:20]==b'RGB ' and profile[36:40]==b'acsp'
    return data
with tempfile.TemporaryDirectory(prefix='fileform-pdf-jpeg-') as folder:
    base=Path(folder)
    cases=[('shapes',lambda p:fixture(p,text=True),144),('units',lambda p:visual_fixture(p,user_unit=2),144),
           ('forms',lambda p:annotation_fixture(p,rotation=90,generated_widget=True),144)]
    for name,create,dpi in cases:
        source=base/f'{name}.pdf';create(source);original=source.read_bytes()
        outputs=[]
        for quality in [5,85,100]:
            output=base/f'{name}-{quality}.jpg'
            result=run(cli,'export-pdf-jpeg',source,output,pdf,renderer,0,dpi,quality)
            assert result.returncode==0,result.stderr
            receipt=json.loads(result.stdout);saved=markers(output,dpi)
            assert receipt['format']=='jpeg' and receipt['quality']==quality and receipt['pixels_per_meter'] is None
            assert receipt['sha256']==hashlib.sha256(saved).hexdigest() and receipt['bytes']==len(saved)
            assert receipt['source_sha256']==hashlib.sha256(original).hexdigest()
            assert any('lossy' in warning for warning in receipt['warnings'])
            # Image input uses zune-jpeg, independent of the page JPEG encoder.
            png=base/f'{name}-{quality}.png'
            result=run(cli,'convert-image',output,png)
            assert result.returncode==0,result.stderr
            w,h,_,pixels=decode(png)
            assert (w,h)==(receipt['width'],receipt['height'])
            plan=json.loads(run(cli,'plan-pdf-raster',source,pdf,dpi).stdout)['pages'][0]
            reference=run(helper,source,0,f'raster:{w}x{h}','crop',*plan['crop_box'],plan['rotation']//90)
            assert reference.returncode==0,reference.stderr
            raw=reference.stdout.split(b'\n',3)[3]
            error=sum(abs(a-b) for a,b in zip(raw,pixels))/len(raw)
            assert error<8,(name,quality,error)
            outputs.append((len(saved),error))
            assert run(cli,'export-pdf-jpeg',source,output,pdf,renderer,0,dpi,quality).returncode!=0
            assert output.read_bytes()==saved
        assert outputs[0][0]<outputs[-1][0] and outputs[-1][1]<outputs[0][1],outputs
        assert source.read_bytes()==original
    request={'operation':'export_pdf_jpeg','input':str(base/'forms.pdf'),'output':str(base/'worker.jpg'),
             'directory':str(pdf),'renderer_directory':str(renderer),'page_index':0,'dpi':144,'quality':85}
    reply=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,check=True,timeout=120)
    worker_receipt=json.loads(reply.stdout)['result']
    assert worker_receipt['sha256']==hashlib.sha256((base/'worker.jpg').read_bytes()).hexdigest()
    markers(base/'worker.jpg',144)
    # The ICC encoder writes its creation time; verify decoded parity, not timestamps.
    assert run(cli,'convert-image',base/'worker.jpg',base/'worker.png').returncode==0
    assert decode(base/'worker.png')[3]==decode(base/'forms-85.png')[3]
    large=base/'large.jpg'
    result=run(cli,'export-pdf-jpeg',base/'shapes.pdf',large,pdf,renderer,0,600)
    assert result.returncode==0,result.stderr
    assert (json.loads(result.stdout)['width'],json.loads(result.stdout)['height'],json.loads(result.stdout)['quality'])==(1667,2500,85)
    markers(large,600)
    rejected=base/'rejected.jpg'
    for quality in [0,4,101]:
        assert run(cli,'export-pdf-jpeg',base/'forms.pdf',rejected,pdf,renderer,0,144,quality).returncode!=0
        assert not rejected.exists()
print('PDF JPEG: full independent decode, JFIF DPI/ICC metadata, quality/error tradeoffs, forms/rotation/UserUnit, CLI/worker parity, collisions and source preservation passed')
