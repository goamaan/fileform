#!/usr/bin/env python3
"""Real HEIC inputs through the same bounded image/CLI/worker pipeline."""
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
cli=root/'target/release'/('fileform-native'+suffix);worker=root/'target/release'/('fileform-worker'+suffix)
fixtures=root/'native/heic-decode/fixtures'
environment={**os.environ,'FILEFORM_HEIC_PACK':str(pack)}
def run(*args,env=environment):return subprocess.run([str(v) for v in args],capture_output=True,timeout=180,env=env)
def good(*args):
    value=run(*args);assert value.returncode==0,value.stderr;return value

def execute(request,ok=True,cancel=False):
    value=subprocess.run([str(worker)],input=json.dumps(request)+'\n'+('cancel\n' if cancel else ''),text=True,capture_output=True,timeout=180,env=environment)
    reply=json.loads(value.stdout);assert reply['ok']==ok,reply;return reply

def orient(pixels,w,h,o):
    nw,nh=(h,w) if o>=5 else (w,h);output=bytearray(nw*nh*4)
    for y in range(h):
        for x in range(w):
            nx,ny={1:(x,y),2:(w-1-x,y),3:(w-1-x,h-1-y),4:(x,h-1-y),5:(y,x),6:(h-1-y,x),7:(h-1-y,w-1-x),8:(y,w-1-x)}[o]
            output[(ny*nw+nx)*4:(ny*nw+nx+1)*4]=pixels[(y*w+x)*4:(y*w+x+1)*4]
    return bytes(output)

good(cli,'verify-heic-pack',pack)
with tempfile.TemporaryDirectory(prefix='fileform-heic-engine-') as folder:
    base=Path(folder);originals={p.name:p.read_bytes() for p in fixtures.glob('*.heic')}
    for source in sorted(fixtures.glob('*.heic')):
        oracle=json.loads(source.with_suffix('.json').read_text());output=base/(source.stem+'.png')
        if oracle['count']>1:
            assert run(cli,'convert-image',source,output).returncode!=0 and not output.exists();continue
        inspect=json.loads(good(cli,'inspect-image',source).stdout)
        expected_size=(oracle['height'],oracle['width']) if oracle['orientation']>=5 else (oracle['width'],oracle['height'])
        assert (inspect['display_width'],inspect['display_height'])==expected_size and inspect['orientation']==1
        assert inspect['conversion_available'] and inspect['sha256']==hashlib.sha256(source.read_bytes()).hexdigest()
        if source.stem=='p3':assert inspect['has_icc'] and inspect['color_interpretation']=='heic_icc'
        receipt=json.loads(good(cli,'convert-image',source,output).stdout)
        w,h,_,raw=decode(output,rgba=True);assert (w,h)==expected_size
        assert receipt['sha256']==hashlib.sha256(output.read_bytes()).hexdigest() and receipt['bytes']==output.stat().st_size
        actual=bytes(value if i%4==3 else (value*raw[i//4*4+3]+127)//255 for i,value in enumerate(raw))
        expected=orient((fixtures/(source.stem+'.srgb.rgba')).read_bytes(),oracle['width'],oracle['height'],oracle['orientation'])
        differences=[abs(a-b) for a,b in zip(actual,expected)]
        # One decoder-space quantization level can grow near a P3->sRGB
        # clipping boundary. Check native samples against an independent matrix
        # as well as the ImageIO end-to-end reference, rather than hide drift.
        if source.stem=='p3':
            helper=pack/'bin'/('fileform-heic-decode'+suffix)
            payload=good(helper,source).stdout
            _,header,body=payload.split(b'\n',2);fields=list(map(int,header.split()))
            native=body[fields[4]+fields[7]:]
            def linear(value):
                value/=255;return value/12.92 if value<=.04045 else ((value+.055)/1.055)**2.4
            def encoded(value):
                value=max(0,min(1,value));return round(255*(12.92*value if value<=.0031308 else 1.055*value**(1/2.4)-.055))
            matrix=[[1.2247453,-.2249044,0],[-.042058,1.042080,0],[-.0196422,-.0786549,1.0985376]]
            analytical=[]
            for index in range(0,len(native),4):
                sample=[linear(v) for v in native[index:index+3]]
                expected_rgb=[encoded(sum(row[k]*sample[k] for k in range(3))) for row in matrix]
                analytical.extend(abs(a-b) for a,b in zip(raw[index:index+3],expected_rgb))
            assert max(analytical)<=2 and sum(analytical)/len(analytical)<.2
        assert max(differences)<=(7 if source.stem=='p3' else 2) and sum(differences)/len(differences)<.5,(source.name,max(differences),sum(differences)/len(differences))
    source=fixtures/'orientation-6.heic';plain=base/'orientation-6.png'
    selected=base/'crop.png';reference=base/'crop-reference.png'
    options=['--crop','4,7,30,20','--max-dimension',15]
    good(cli,'convert-image',source,selected,*options);good(cli,'convert-image',plain,reference,*options)
    assert decode(selected,rgba=True)[3]==decode(reference,rgba=True)[3] and decode(selected,rgba=True)[:2]==(15,10)
    tiff=base/'converted.tiff';roundtrip=base/'roundtrip.png'
    good(cli,'convert-image',source,tiff);good(cli,'convert-image',tiff,roundtrip)
    assert decode(roundtrip,rgba=True)[3]==decode(plain,rgba=True)[3]
    alpha=fixtures/'alpha.heic';rejected=base/'no-matte.jpg'
    assert run(cli,'convert-image',alpha,rejected).returncode!=0 and not rejected.exists()
    for background,expected in [('white',255),('black',0)]:
        output=base/(background+'.jpg');good(cli,'convert-image',alpha,output,'--background',background,'--quality',100)
        pixels=base/(background+'.png');good(cli,'convert-image',output,pixels)
        assert all(abs(value-expected)<=2 for value in decode(pixels)[3][:3])
    opaque=fixtures/'srgb.heic';floor=base/'floor.jpg'
    good(cli,'convert-image',opaque,floor,'--quality',40)
    fitted=base/'fitted.jpg'
    receipt=json.loads(good(cli,'convert-image',opaque,fitted,'--max-bytes',floor.stat().st_size,'--quality',90,'--minimum-quality',40).stdout)
    assert receipt['bytes']<=floor.stat().st_size and receipt['quality']>=40
    output=base/'worker.png'
    request={'operation':'convert_image','input':str(opaque),'output':str(output)}
    result=execute(request)['result'];assert result['sha256']==hashlib.sha256(output.read_bytes()).hexdigest()
    unchanged=output.read_bytes();assert not execute(request,ok=False)['ok'] and output.read_bytes()==unchanged
    for changes,cancel,code in [({'expected_source_sha256':'0'*64},False,'source_changed'),({},True,'cancelled')]:
        rejected=base/(code+'.png');reply=execute({**request,**changes,'output':str(rejected)},ok=False,cancel=cancel)
        assert reply['error']['code']==code and not rejected.exists()
    if len(sys.argv)==5:
        pdf,renderer,ocr=[Path(v).resolve() for v in sys.argv[2:]]
        combined=base/'heic-pages.pdf'
        good(cli,'merge-pdf',combined,pdf,renderer,fixtures/'alpha.heic',fixtures/'orientation-6.heic')
        geometry=json.loads(good(cli,'inspect-pdf-pages',combined,pdf).stdout)['pages']
        assert len(geometry)==2 and geometry[0]['media_box']==[0,0,128,96] and geometry[1]['media_box']==[0,0,96,128]
        text=base/'heic-ocr.txt'
        receipt=json.loads(good(cli,'ocr-image',root/'crates/fileform-engine/tests/fixtures/ocr-page.heic',text,ocr,'eng').stdout)
        assert text.read_bytes()==b'Fileform page 1\n' and receipt['ocr_performed']
        assert receipt['sha256']==hashlib.sha256(text.read_bytes()).hexdigest()
    missing={**environment,'FILEFORM_HEIC_PACK':str(base/'missing-pack')};output=base/'missing.png'
    assert run(cli,'convert-image',opaque,output,env=missing).returncode!=0 and not output.exists()
    assert {p.name:p.read_bytes() for p in fixtures.glob('*.heic')}==originals
    assert not any(p.is_dir() for p in base.iterdir()),'Staging leaked'
print('HEIC engine: relocatable verified pack, sRGB/P3 ICC color against ImageIO, alpha and all orientations, CLI/worker, crop/resize, TIFF roundtrip, explicit JPEG mattes, byte fitting, source/hash/metadata checks, collision/stale/cancel/missing-pack rejection and unchanged inputs passed; broader grid/gain-map/depth/EXIF cases remain open')
