#!/usr/bin/env python3
"""Original JPEG streams and exact RGB/gray/soft-mask extraction with provenance."""
import base64
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from pdf_fixtures import write_pdf, fixture
from png_fixture_decode import decode
root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve();suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix);worker=root/'target/release'/('fileform-worker'+suffix)
rgb=bytes([255,0,0,0,128,255]);mask=bytes([0,127]);gray=bytes([30,200])
def run(*args):return subprocess.run([str(v) for v in args],capture_output=True,timeout=120)
def stream(dictionary,data):return f'<< {dictionary} /Length {len(data)} >>\nstream\n'.encode()+data+b'\nendstream'
def image(samples,color='/DeviceRGB',filter='',extra='',width=2,height=1):
    return stream(f'/Type /XObject /Subtype /Image /Width {width} /Height {height} /BitsPerComponent 8 /ColorSpace {color} {filter} {extra}',samples)
def document(path,jpeg,bad=False,many=0):
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',
             b'<< /Type /Pages /Count 2 /Kids [3 0 R 4 0 R] /Resources 6 0 R >>',
             b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Contents 5 0 R >>',
             b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources 6 0 R /Contents 5 0 R >>',
             stream('',b''),
             b'<< /XObject << /Alpha 7 0 R /Flate 8 0 R /Nested 9 0 R /AnotherNested 9 0 R /Unsupported 10 0 R /Jpeg 11 2 R /Gray 12 0 R /ASCII85 15 0 R /RunLength 16 0 R >> >>',
             image(rgb[:-1] if bad else rgb,extra='/SMask 13 0 R'),
             image(zlib.compress(rgb),filter='/Filter /FlateDecode'),
             stream('/Type /XObject /Subtype /Form /BBox [0 0 200 300] /Resources 14 0 R',b''),
             image(bytes([0,0,0,0]),color='/DeviceCMYK',width=1),
             image(jpeg,filter='/Filter /DCTDecode'),
             image(gray.hex().encode()+b'>',color='/DeviceGray',filter='/Filter /ASCIIHexDecode'),
             image(mask,color='/DeviceGray'),
             b'<< /XObject << /Reused 7 0 R /Cycle 9 0 R >> >>',
             image(base64.a85encode(rgb)+b'~>',filter='/Filter /ASCII85Decode'),
             image(bytes([len(rgb)-1])+rgb+bytes([128]),filter='/Filter /RunLengthDecode')]
    if many:
        additions=[]
        for i in range(many):
            number=len(objects)+1
            objects.append(image(rgb))
            additions.append(f'/Many{i:03} {number} 0 R'.encode())
        objects[5]=objects[5].replace(b' >> >>',b' '+b' '.join(additions)+b' >> >>')
    write_pdf(path,objects,generations={11:2})
def request(inputs,output=None,**options):
    return {'operation':'extract_pdf_images','inputs':[str(p) for p in inputs],'output':str(output) if output else None,'directory':str(pack),**options}
def execute(value):
    response=subprocess.run([str(worker)],input=json.dumps(value)+'\n',text=True,capture_output=True,timeout=120)
    return json.loads(response.stdout)
with tempfile.TemporaryDirectory(prefix='fileform-extract-images-') as folder:
    base=Path(folder)
    png=base/'original.png'
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    png.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,1,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0'+rgb))+chunk(b'IEND',b''))
    encoded=base/'original.jpg';assert run(cli,'convert-image',png,encoded).returncode==0
    jpeg=encoded.read_bytes();source=base/'source.pdf';document(source,jpeg);original=source.read_bytes()
    planned=run(cli,'plan-pdf-images',pack,source);assert planned.returncode==0,planned.stderr
    plan=json.loads(planned.stdout)
    assert (plan['discovered'],plan['supported'],plan['skipped'])==(7,6,1),plan
    by={v['object_number']:v for v in plan['candidates']}
    assert by[11]['generation']==2 and by[11]['encoding_outcome']=='preserved_encoded_bytes'
    assert by[7]['resource_pages']==[0,1] and len(by[7]['resource_paths'])==6
    assert 13 not in by  # Auxiliary soft mask is not exported unless independently referenced.
    assert by[10]['skip_reason'] and by[10]['color_space']=='/DeviceCMYK'
    selected=execute(request([source],dry_run=True,pages=[{'source_index':0,'page_index':1}]*2))
    assert selected['ok'] and all(v['resource_pages']==[1] for v in selected['result']['candidates'])
    output=base/'images'
    result=run(cli,'extract-pdf-images',output,pack,source);assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    assert len(list(output.iterdir()))==6
    for candidate in receipt['candidates']:
        if candidate['skip_reason']:assert candidate['artifact_name'] is None;continue
        path=output/candidate['artifact_name'];content=path.read_bytes()
        assert candidate['sha256']==hashlib.sha256(content).hexdigest() and candidate['bytes']==len(content)
        if candidate['object_number']==11:assert content==jpeg
        else:
            w,h,_,pixels=decode(path,rgba=True);assert (w,h)==(2,1)
            if candidate['object_number']==7:expected=bytes([255,0,0,0,0,128,255,127])
            elif candidate['object_number']==12:expected=bytes([30,30,30,255,200,200,200,255])
            else:expected=bytes([255,0,0,255,0,128,255,255])
            assert bytes(pixels)==expected,(candidate,pixels)
    assert not execute(request([source],output))['ok']
    alias=base/'alias.pdf';os.link(source,alias)
    assert not execute(request([source,alias],dry_run=True))['ok']
    empty=base/'empty.pdf';fixture(empty)
    no_images=execute(request([empty],dry_run=True));assert no_images['ok'] and no_images['result']['discovered']==0
    rejected=base/'rejected'
    assert not execute(request([empty],rejected))['ok'] and not rejected.exists()
    assert not execute(request([source],rejected,pages=[{'source_index':0,'page_index':0,'clockwise_rotation':90}]))['ok']
    bad=base/'bad.pdf';document(bad,jpeg,bad=True)
    before=set(base.iterdir())
    failed=execute(request([bad],rejected));assert not failed['ok'] and not rejected.exists(),failed
    assert set(base.iterdir())==before
    active_source=base/'active.pdf';document(active_source,jpeg,many=50)
    before=set(base.iterdir());cancelled=base/'cancelled'
    process=subprocess.Popen([str(worker)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        process.stdin.write(json.dumps({'request':request([active_source],cancelled),'cancel_on_disconnect':True})+'\n');process.stdin.flush()
        deadline=time.monotonic()+30
        while not any(p.is_dir() and (p/'parts').is_dir() and list((p/'parts').iterdir()) for p in base.iterdir() if p not in before):
            assert process.poll() is None and time.monotonic()<deadline
            time.sleep(.01)
        assert not cancelled.exists()
        process.stdin.write('cancel\n');process.stdin.flush()
        stdout,stderr=process.communicate(timeout=30)
        failed=json.loads(stdout)
        assert not failed['ok'] and failed['error']['code']=='cancelled',(failed,stderr)
    finally:
        if process.poll() is None:process.kill();process.wait()
    assert not cancelled.exists() and set(base.iterdir())==before
    assert source.read_bytes()==original
    print('Embedded PDF images: exact JPEG/generation, RGB/gray/alpha including hidden RGB, Flate/ASCIIHex/ASCII85/RunLength, inherited/nested/cyclic resources, reuse/dedup/provenance, skips and failure cleanup passed')
