#!/usr/bin/env python3
"""Native PDF inspection against a real, source-verified qpdf pack."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve()
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
qpdf=pack/'bin'/('qpdf'+suffix)
def run(*args):return subprocess.run([str(x) for x in args],capture_output=True,check=True,timeout=120)
def fixture(path):
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 2 /Kids [3 0 R 4 0 R] >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << >> >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << >> >>']
    data=bytearray(b'%PDF-1.7\n');offsets=[0]
    for index,obj in enumerate(objects,1):
        offsets.append(len(data));data.extend(str(index).encode()+b' 0 obj\n'+obj+b'\nendobj\n')
    xref=len(data);data.extend(b'xref\n0 5\n0000000000 65535 f \n')
    for offset in offsets[1:]:data.extend(f'{offset:010} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode());path.write_bytes(data)
with tempfile.TemporaryDirectory(prefix='fileform-pdf-') as folder:
    base=Path(folder);source=base/'two-pages.pdf';fixture(source)
    expected=hashlib.sha256(source.read_bytes()).hexdigest()
    assert json.loads(run(cli,'verify-pdf-pack',pack).stdout)['version']
    result=json.loads(run(cli,'inspect-pdf',source,pack).stdout)
    assert result['pages']==2 and result['qpdf_check_passed'] and result['sha256']==expected
    response=subprocess.run([str(worker)],input=json.dumps({'operation':'inspect_pdf','input':str(source),'directory':str(pack)})+'\n',text=True,capture_output=True,check=True,timeout=120)
    assert json.loads(response.stdout)['result']==result
    damaged=base/'damaged.pdf';damaged.write_bytes(b'%PDF-1.7\ninvalid')
    protected=base/'protected.pdf'
    run(qpdf,'--encrypt','fixture-user','fixture-owner','256','--',source,protected)
    for path in [damaged,protected]:
        response=subprocess.run([str(cli),'inspect-pdf',str(path),str(pack)],capture_output=True,timeout=120)
        assert response.returncode!=0
    assert hashlib.sha256(source.read_bytes()).hexdigest()==expected
    print(json.dumps({'pages':2,'qpdfCheckPassed':True,'workerMatchesCli':True,'malformedRejected':True,'passwordProtectedRejected':True,'sourceUnchanged':True}))
