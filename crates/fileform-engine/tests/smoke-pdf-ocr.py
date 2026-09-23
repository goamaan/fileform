#!/usr/bin/env python3
"""Mixed embedded/scanned PDF text export through the real native pipeline."""
import gzip
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib
from pdf_fixtures import fixture, annotation_fixture
root=Path(__file__).resolve().parents[3]
pdf,renderer,ocr=[Path(v).resolve() for v in sys.argv[1:]]
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=240)
def chunk(kind,data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
def png(path,w,h,rgb):
    scan=b''.join(b'\0'+rgb[y*w*3:(y+1)*w*3] for y in range(h))
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(scan))+chunk(b'IEND',b''))
raw=gzip.decompress((Path(__file__).parent/'fixtures/ocr-page.ppm.gz').read_bytes())
_,dimensions,_,rgb=raw.split(b'\n',3);w,h=map(int,dimensions.split())
with tempfile.TemporaryDirectory(prefix='fileform-pdf-ocr-') as folder:
    base=Path(folder)
    scanned=base/'scan.png';png(scanned,w,h,rgb)
    blank=base/'blank.png';png(blank,w,h,b'\xff'*len(rgb))
    embedded=base/'embedded.pdf';fixture(embedded,text=True)
    mixed=base/'mixed.pdf'
    merge=run(cli,'merge-pdf',mixed,pdf,renderer,scanned,embedded,blank)
    assert merge.returncode==0,merge.stderr
    original=mixed.read_bytes()
    output=base/'recognized.txt'
    result=run(cli,'export-pdf-text',mixed,output,pdf,renderer,'--ocr',ocr,'eng')
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    expected='\n\n\f\n\n'.join(['Page 1\n\nFileform page 1','Page 2\n\nFileform page 1','Page 3\n\nFileform page 2','Page 4\n\n[No text recognized on this page.]'])+'\n'
    assert output.read_bytes()==expected.encode('utf-8'),output.read_bytes()
    assert receipt['ocr_performed'] and receipt['ocr_language']=='eng'
    assert receipt['ocr_pages']==[0,3] and receipt['pages_without_embedded_text']==[0,3]
    assert receipt['pages_without_recognized_text']==[3]
    assert receipt['sha256']==hashlib.sha256(output.read_bytes()).hexdigest()
    assert receipt['source_sha256']==hashlib.sha256(original).hexdigest()
    request={'operation':'export_pdf_text','input':str(mixed),'output':str(base/'worker.txt'),
             'directory':str(pdf),'renderer_directory':str(renderer),'ocr':{'directory':str(ocr),'language':'eng'}}
    response=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,check=True,timeout=240)
    assert json.loads(response.stdout)['result']['sha256']==receipt['sha256']
    assert (base/'worker.txt').read_bytes()==output.read_bytes()
    assert run(cli,'export-pdf-text',mixed,output,pdf,renderer,'--ocr',ocr,'eng').returncode!=0
    single=base/'blank.pdf'
    assert run(cli,'merge-pdf',single,pdf,renderer,blank).returncode==0
    rejected=base/'rejected.txt'
    result=run(cli,'export-pdf-text',single,rejected,pdf,renderer,'--ocr',ocr,'eng')
    assert result.returncode!=0 and b'no_text' in result.stderr and not rejected.exists()
    for generated in [False,True]:
        form=base/f'form-{generated}.pdf';annotation_fixture(form,generated_widget=generated)
        saved=form.read_bytes()
        form_text=base/f'form-{generated}.txt'
        result=run(cli,'export-pdf-text',form,form_text,pdf,renderer,'--ocr',ocr,'eng')
        assert result.returncode==0,result.stderr
        text=form_text.read_bytes()
        assert text==b'Visible form\n\nVisible note\n',text
        assert b'Hidden' not in text and b'CHANGED' not in text
        assert form.read_bytes()==saved
    annotated=base/'xfa.pdf';annotation_fixture(annotated,xfa=True)
    result=run(cli,'export-pdf-text',annotated,rejected,pdf,renderer,'--ocr',ocr,'eng')
    assert result.returncode!=0 and b'unsupported' in result.stderr and not rejected.exists()
    assert mixed.read_bytes()==original
print('PDF OCR: mixed embedded/scanned/blank pages, 4096-edge rendering, per-page provenance, CLI/worker parity, static form/annotation OCR, hidden-value exclusion, blank/XFA/collision rejection and unchanged source passed')
