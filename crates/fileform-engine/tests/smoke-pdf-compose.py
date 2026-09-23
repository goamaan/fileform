#!/usr/bin/env python3
"""Real PDF merge and selected/duplicated/rotated-page composition."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from pdf_fixtures import fixture, visual_fixture, unicode_fixture
root = Path(__file__).resolve().parents[3]
pdf, renderer = [Path(v).resolve() for v in sys.argv[1:]]
suffix = '.exe' if sys.platform == 'win32' else ''
cli = root/'target/release'/('fileform-native'+suffix)
worker = root/'target/release'/('fileform-worker'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=180)
def compose(request):
    result = subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,timeout=180)
    return json.loads(result.stdout)
with tempfile.TemporaryDirectory(prefix='fileform-compose-') as folder:
    base=Path(folder)
    text=base/'text.pdf';fixture(text,text=True)
    image=base/'image.pdf';visual_fixture(image)
    unicode=base/'unicode.pdf';unicode_fixture(unicode)
    inherited=base/'inherited.pdf';fixture(inherited,inherited=True)
    inputs=[text,image,unicode,inherited]
    hashes=[hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs]
    output=base/'merged.pdf'
    result=run(cli,'merge-pdf',output,pdf,renderer,*inputs)
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    assert receipt['pages']==6 and receipt['source_sha256']==hashes
    assert receipt['sha256']==hashlib.sha256(output.read_bytes()).hexdigest()
    saved=output.read_bytes()
    assert run(cli,'merge-pdf',output,pdf,renderer,*inputs).returncode!=0
    assert output.read_bytes()==saved
    selected=base/'selected.pdf'
    pages=[{'source_index':2,'page_index':0,'clockwise_rotation':90},
           {'source_index':0,'page_index':1,'clockwise_rotation':180},
           {'source_index':1,'page_index':0,'clockwise_rotation':-90},
           {'source_index':0,'page_index':1},
           {'source_index':3,'page_index':1,'clockwise_rotation':90}]
    request={'operation':'compose_pdf','inputs':[str(p) for p in inputs], 'output':str(selected),
             'directory':str(pdf),'renderer_directory':str(renderer),'pages':pages,'allow_document_changes':True}
    result=compose(request)
    assert result['ok'],result
    assert result['result']['pages']==5
    geometry=json.loads(run(cli,'inspect-pdf-pages',selected,pdf).stdout)['pages']
    assert [p['rotation'] for p in geometry]==[90,180,270,0,0]
    assert [p['media_box'] for p in geometry]==[[0,0,200,200],[0,0,300,200],[-10,-20,70,80],[0,0,300,200],[-10,-20,210,320]]
    for changes in [{'allow_document_changes':False}, {'pages':[]},
                    {'pages':[{'source_index':4,'page_index':0}]},
                    {'pages':[{'source_index':0,'page_index':99}]},
                    {'pages':[{'source_index':0,'page_index':0,'clockwise_rotation':45}]}]:
        rejected=base/'rejected.pdf'
        result=compose({**request,**changes,'output':str(rejected)})
        assert not result['ok'] and not rejected.exists(),result
    assert [hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs]==hashes
print('PDF composition: merge, source/page order, duplicate pages with independent rotations, inherited boxes, Unicode/image/text proofs, source preservation, collision and invalid-selection rejection passed')
