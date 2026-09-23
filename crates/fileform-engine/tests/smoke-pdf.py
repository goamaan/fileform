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
def fixture(path, annotated=False, inherited=False, text=False):
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 2 /Kids [3 0 R 4 0 R] >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << >> /Contents 5 0 R >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << >> /Contents 6 0 R >>']
    if inherited:
        objects[1]=objects[1].replace(b' >>',b' /MediaBox [-10 -20 210 320] /CropBox [0 0 200 300] /Rotate -90 >>')
        objects[2]=objects[2].replace(b'/MediaBox [0 0 200 300] ',b'/BleedBox [1 2 100 200] ')
        objects[3]=objects[3].replace(b'/MediaBox [0 0 300 200] ',b'/TrimBox null ')
    if annotated: objects[2]=objects[2].replace(b'/Resources',b'/Annots [] /Resources')
    contents=[b'q 1 0 0 rg 10 10 50 50 re f Q\n',b'q 0 0 1 rg 20 20 40 40 re f Q\n']
    if text:
        objects[2]=objects[2].replace(b'/Resources << >>',b'/Resources << /Font << /F1 7 0 R >> >>')
        objects[3]=objects[3].replace(b'/Resources << >>',b'/Resources << /Font << /F1 7 0 R >> >>')
        contents=[content+f'BT /F1 12 Tf 20 100 Td (Fileform page {i+1}) Tj ET\n'.encode() for i,content in enumerate(contents)]
    for content in contents:
        objects.append(f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream')
    if text: objects.append(b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
    data=bytearray(b'%PDF-1.7\n');offsets=[0]
    for index,obj in enumerate(objects,1):
        offsets.append(len(data));data.extend(str(index).encode()+b' 0 obj\n'+obj+b'\nendobj\n')
    xref=len(data);data.extend(f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:data.extend(f'{offset:010} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode());path.write_bytes(data)
with tempfile.TemporaryDirectory(prefix='fileform-pdf-') as folder:
    base=Path(folder);source=base/'two-pages.pdf';fixture(source)
    expected=hashlib.sha256(source.read_bytes()).hexdigest()
    assert json.loads(run(cli,'verify-pdf-pack',pack).stdout)['version']
    result=json.loads(run(cli,'inspect-pdf',source,pack).stdout)
    assert result['pages']==2 and result['qpdf_check_passed'] and result['sha256']==expected
    response=subprocess.run([str(worker)],input=json.dumps({'operation':'inspect_pdf','input':str(source),'directory':str(pack)})+'\n',text=True,capture_output=True,check=True,timeout=120)
    assert json.loads(response.stdout)['result']==result
    geometry=json.loads(run(cli,'inspect-pdf-pages',source,pack).stdout)
    assert [p['media_box'] for p in geometry['pages']]==[[0,0,200,300],[0,0,300,200]]
    inherited=base/'inherited.pdf';fixture(inherited,inherited=True)
    inherited_geometry=json.loads(run(cli,'inspect-pdf-pages',inherited,pack).stdout)
    assert all(p['rotation']==270 and p['media_box']==[-10,-20,210,320] and p['crop_box']==[0,0,200,300] for p in inherited_geometry['pages'])
    assert inherited_geometry['pages'][0]['bleed_box']==[1,2,100,200]
    assert inherited_geometry['pages'][1]['trim_box']==[0,0,200,300]
    graph=json.loads(run(cli,'inspect-pdf-graph',source,pack).stdout)
    worker_graph=subprocess.run([str(worker)],input=json.dumps({'operation':'inspect_pdf_graph','input':str(source),'directory':str(pack)})+'\n',text=True,capture_output=True,check=True,timeout=120)
    assert json.loads(worker_graph.stdout)['result']==graph
    rewritten=base/'rewritten.pdf'
    run(qpdf,'--object-streams=generate','--compress-streams=y','--recompress-flate',source,rewritten)
    after=json.loads(run(cli,'inspect-pdf-graph',rewritten,pack).stdout)
    assert graph['graph_sha256']==after['graph_sha256']
    changed=base/'changed.pdf';changed.write_bytes(source.read_bytes().replace(b'1 0 0 rg',b'0 1 0 rg'))
    assert json.loads(run(cli,'inspect-pdf-graph',changed,pack).stdout)['graph_sha256']!=graph['graph_sha256']
    assert graph['special_preservation_keys']==[]
    annotated=base/'annotated.pdf';fixture(annotated,True)
    assert '/Annots' in json.loads(run(cli,'inspect-pdf-graph',annotated,pack).stdout)['special_preservation_keys']
    text_source=base/'text-and-shapes.pdf';fixture(text_source,text=True)
    assert json.loads(run(cli,'inspect-pdf',text_source,pack).stdout)['pages']==2
    assert json.loads(run(cli,'inspect-pdf-graph',text_source,pack).stdout)['special_preservation_keys']==[]
    damaged=base/'damaged.pdf';damaged.write_bytes(b'%PDF-1.7\ninvalid')
    protected=base/'protected.pdf'
    run(qpdf,'--encrypt','fixture-user','fixture-owner','256','--',source,protected)
    for path in [damaged,protected]:
        response=subprocess.run([str(cli),'inspect-pdf',str(path),str(pack)],capture_output=True,timeout=120)
        assert response.returncode!=0
    assert hashlib.sha256(source.read_bytes()).hexdigest()==expected
    print(json.dumps({'pages':2,'qpdfCheckPassed':True,'workerMatchesCli':True,'malformedRejected':True,'passwordProtectedRejected':True,'sourceUnchanged':True,'rewriteGraphPreserved':True,'changedContentDetected':True,'specialPreservationKeysDetected':True,'pageGeometryAndInheritanceVerified':True}))
