#!/usr/bin/env python3
"""Whole-document embedded text retains page boundaries and exposes OCR gaps."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from pdf_fixtures import fixture, unicode_fixture
root=Path(__file__).resolve().parents[3]
pdf,renderer=[Path(v).resolve() for v in sys.argv[1:]]
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=180)
with tempfile.TemporaryDirectory(prefix='fileform-document-text-') as folder:
    base=Path(folder)
    source=base/'text.pdf';fixture(source,text=True)
    original=source.read_bytes()
    output=base/'text.txt'
    result=run(cli,'export-pdf-text',source,output,pdf,renderer)
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    expected=b'Page 1\n\nFileform page 1\n\n\x0c\n\nPage 2\n\nFileform page 2\n'
    assert output.read_bytes()==expected
    assert receipt['pages']==2 and receipt['pages_without_embedded_text']==[] and not receipt['ocr_performed']
    assert receipt['bytes']==len(expected) and receipt['sha256']==hashlib.sha256(expected).hexdigest()
    assert receipt['source_sha256']==hashlib.sha256(original).hexdigest()
    assert run(cli,'export-pdf-text',source,output,pdf,renderer).returncode!=0
    assert output.read_bytes()==expected
    request={'operation':'export_pdf_text','input':str(source),'output':str(base/'worker.txt'),
             'directory':str(pdf),'renderer_directory':str(renderer)}
    reply=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,check=True,timeout=180)
    assert json.loads(reply.stdout)['result']['sha256']==receipt['sha256']
    assert (base/'worker.txt').read_bytes()==expected
    mixed=base/'mixed.pdf'
    mixed.write_bytes(original.replace(b'Fileform page 2',b' '*len(b'Fileform page 2')))
    failed=base/'failed.txt'
    before=set(base.iterdir())
    result=run(cli,'export-pdf-text',mixed,failed,pdf,renderer)
    assert result.returncode!=0 and b'ocr_required' in result.stderr
    assert not failed.exists() and set(base.iterdir())==before
    partial=base/'partial.txt'
    result=run(cli,'export-pdf-text',mixed,partial,pdf,renderer,'--allow-missing-text')
    assert result.returncode==0,result.stderr
    assert json.loads(result.stdout)['pages_without_embedded_text']==[1]
    assert b'[No embedded text on this page; OCR required.]' in partial.read_bytes()
    unicode=base/'unicode.pdf';unicode_fixture(unicode)
    single=base/'single.txt'
    result=run(cli,'export-pdf-text',unicode,single,pdf,renderer)
    assert result.returncode==0,result.stderr
    assert single.read_text()=='Ω中😀́\n'
    unicode_fixture(unicode,invalid=True)
    invalid=base/'invalid.txt'
    assert run(cli,'export-pdf-text',unicode,invalid,pdf,renderer).returncode!=0 and not invalid.exists()
    assert source.read_bytes()==original
print('PDF document text: page labels/separators, exact Unicode, CLI/worker parity, source/output hashes, collisions, missing-text failure/explicit disclosure and staging cleanup passed')
