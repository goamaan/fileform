#!/usr/bin/env python3
"""Independent PNG decoding and physical-resolution checks for real PDF exports."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib
from pdf_fixtures import fixture, visual_fixture, annotation_fixture
root=Path(__file__).resolve().parents[3]
pdf,renderer=[Path(v).resolve() for v in sys.argv[1:]]
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
helper=renderer/'bin'/('fileform-pdf-render'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=120)
def decode(path):
    data=path.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n'
    offset=8;chunks={};compressed=bytearray()
    while offset<len(data):
        length,=struct.unpack('>I',data[offset:offset+4]);kind=data[offset+4:offset+8]
        value=data[offset+8:offset+8+length]
        crc,=struct.unpack('>I',data[offset+8+length:offset+12+length])
        assert zlib.crc32(kind+value)==crc
        offset+=12+length
        if kind==b'IDAT':compressed.extend(value)
        else:chunks[kind]=value
        if kind==b'IEND':assert offset==len(data);break
    w,h,depth,color,compression,filtering,interlaced=struct.unpack('>IIBBBBB',chunks[b'IHDR'])
    assert (depth,color,compression,filtering,interlaced)==(8,2,0,0,0)
    scan=zlib.decompress(compressed);stride=w*3
    assert len(scan)==h*(stride+1)
    previous=bytearray(stride);pixels=bytearray()
    for y in range(h):
        start=y*(stride+1);kind=scan[start];row=bytearray(scan[start+1:start+1+stride])
        assert kind<=4
        if kind==2:row=bytearray((a+b)&255 for a,b in zip(row,previous))
        elif kind:
            for x in range(stride):
                left=row[x-3] if x>=3 else 0;up=previous[x];corner=previous[x-3] if x>=3 else 0
                if kind==1:prediction=left
                elif kind==3:prediction=(left+up)//2
                else:
                    p=left+up-corner;a=abs(p-left);b=abs(p-up);c=abs(p-corner)
                    prediction=left if a<=b and a<=c else (up if b<=c else corner)
                row[x]=(row[x]+prediction)&255
        pixels.extend(row);previous=row
    assert chunks[b'sRGB']==b'\0'
    return w,h,struct.unpack('>IIB',chunks[b'pHYs']),pixels
with tempfile.TemporaryDirectory(prefix='fileform-pdf-png-') as folder:
    base=Path(folder)
    cases=[('large',lambda p:fixture(p,text=True),0,600),
           ('inherited',lambda p:fixture(p,inherited=True),1,144),
           ('units',lambda p:visual_fixture(p,user_unit=2),0,72),
           ('forms',lambda p:annotation_fixture(p,generated_widget=True),0,144)]
    for name,create,page,dpi in cases:
        source=base/f'{name}.pdf';create(source);original=source.read_bytes()
        output=base/f'{name}.png'
        result=run(cli,'export-pdf-png',source,output,pdf,renderer,page,dpi)
        assert result.returncode==0,result.stderr
        receipt=json.loads(result.stdout)
        w,h,physical,pixels=decode(output)
        assert (w,h)==(receipt['width'],receipt['height'])
        assert physical==(receipt['pixels_per_meter'],receipt['pixels_per_meter'],1)
        assert abs(physical[0]*.0254-dpi)<.03
        plan=json.loads(run(cli,'plan-pdf-raster',source,pdf,dpi).stdout)['pages'][page]
        raw=run(helper,source,page,f'raster:{w}x{h}','crop',*plan['crop_box'],plan['rotation']//90)
        assert raw.returncode==0 and bytes(pixels)==raw.stdout.split(b'\n',3)[3]
        saved=output.read_bytes()
        assert receipt['sha256']==hashlib.sha256(saved).hexdigest() and receipt['bytes']==len(saved)
        assert receipt['source_sha256']==hashlib.sha256(original).hexdigest()
        assert run(cli,'export-pdf-png',source,output,pdf,renderer,page,dpi).returncode!=0
        assert output.read_bytes()==saved and source.read_bytes()==original
    request={'operation':'export_pdf_png','input':str(base/'forms.pdf'),'output':str(base/'worker.png'),
             'directory':str(pdf),'renderer_directory':str(renderer),'page_index':0,'dpi':144}
    reply=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,check=True,timeout=120)
    assert json.loads(reply.stdout)['result']['sha256']==hashlib.sha256((base/'forms.png').read_bytes()).hexdigest()
    rejected=base/'rejected.png'
    for page,dpi in [(99,144),(0,601)]:
        assert run(cli,'export-pdf-png',base/'forms.pdf',rejected,pdf,renderer,page,dpi).returncode!=0
        assert not rejected.exists()
print('PDF PNG export: independent complete pixel/CRC decode, exact dimensions, DPI/sRGB metadata, inheritance/UserUnit/forms, CLI/worker parity, no-clobber and unchanged sources passed')
