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
from pdf_fixtures import fixture
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
