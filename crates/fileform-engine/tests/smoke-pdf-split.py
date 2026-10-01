#!/usr/bin/env python3
"""Split output is complete-or-absent, including later-group failure/cancel."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from pdf_fixtures import fixture, visual_fixture
root=Path(__file__).resolve().parents[3]
pdf,renderer=[Path(v).resolve() for v in sys.argv[1:]]
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
def run(*args):
    return subprocess.run([str(v) for v in args],capture_output=True,timeout=180)
def split(request):
    result=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,timeout=180)
    return json.loads(result.stdout)
with tempfile.TemporaryDirectory(prefix='fileform-pdf-split-') as folder:
    base=Path(folder)
    source=base/'source.pdf';fixture(source,text=True)
    visual=base/'visual.pdf';visual_fixture(visual)
    inputs=[source,visual]
    hashes=[hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs]
    output=base/'parts'
    result=run(cli,'split-pdf',output,pdf,renderer,*inputs)
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    assert sorted(p.name for p in output.iterdir())==['part-0001.pdf','part-0002.pdf','part-0003.pdf']
    for part in receipt['parts']:
        path=output/part['name']
        assert path.parent==output and part['pages']==1
        assert hashlib.sha256(path.read_bytes()).hexdigest()==part['sha256']
    assert run(cli,'split-pdf',output,pdf,renderer,*inputs).returncode!=0
    renamed=run(cli,'split-pdf',output,pdf,renderer,*inputs,'--collision','rename','--every','2')
    assert renamed.returncode==0,renamed.stderr
    renamed_receipt=json.loads(renamed.stdout)
    assert Path(renamed_receipt['output'])==base/'parts (1)' and [p['pages'] for p in renamed_receipt['parts']]==[2,1]
    empty=base/'existing-empty';empty.mkdir()
    assert run(cli,'split-pdf',empty,pdf,renderer,*inputs).returncode!=0
    assert not list(empty.iterdir())
    request={'operation':'split_pdf','inputs':[str(p) for p in inputs], 'output':str(base/'grouped'),
             'directory':str(pdf),'renderer_directory':str(renderer),'allow_document_changes':True,
             'groups':[[{'source_index':0,'page_index':1},{'source_index':1,'page_index':0,'clockwise_rotation':90}],
                       [{'source_index':0,'page_index':0}]]}
    result=split(request)
    assert result['ok'],result
    assert [p['pages'] for p in result['result']['parts']]==[2,1]
    # Ranges address the concatenated PDF/image inputs, preserve order and
    # duplicates, and separate output groups with semicolons.
    for flag,value,expected in [('--ranges','3,1;2,2',[[[1,0],[0,0]],[[0,1],[0,1]]]),
                                ('--every','2',[[[0,0],[0,1]],[[1,0]]]),
                                ('--after','1,2',[[[0,0]],[[0,1]],[[1,0]]])]:
        planned=base/(flag[2:]+'-plan')
        result=run(cli,'split-pdf',planned,pdf,renderer,*inputs,flag,value,'--dry-run')
        assert result.returncode==0,result.stderr
        plan=json.loads(result.stdout)
        assert plan['status']=='planned' and not planned.exists() and plan['parts']==[]
        actual=[[[p['source_index'],p['page_index']] for p in group] for group in plan['groups']]
        assert actual==expected and plan['source_sha256']==hashes,(flag,actual)
        destination=base/(flag[2:]+'-saved')
        result=run(cli,'split-pdf',destination,pdf,renderer,*inputs,flag,value)
        assert result.returncode==0,result.stderr
        saved=json.loads(result.stdout)
        assert [part['pages'] for part in saved['parts']]==[len(group) for group in expected]
    for args in [('--every','0'),('--ranges','4'),('--ranges','1;'),('--after','3'),
                 ('--after','2,1'),('--every','2','--ranges','1'),('--dry-run','--dry-run')]:
        destination=base/'invalid-plan'
        assert run(cli,'split-pdf',destination,pdf,renderer,*inputs,*args).returncode!=0 and not destination.exists()
    worker_plan={**request,'output':str(base/'worker-plan'),'groups':None,
                 'selection':{'mode':'ranges','ranges':'2;1,3'},'dry_run':True}
    reply=split(worker_plan);assert reply['ok'] and reply['result']['status']=='planned' and not (base/'worker-plan').exists(),reply
    assert not split({**worker_plan,'groups':request['groups']})['ok']
    assert not split({**worker_plan,'selection':None,'groups':[[{'source_index':0,'page_index':99}]]})['ok']
    before=set(base.iterdir())
    rejected=base/'rejected'
    invalid={**request,'output':str(rejected),'groups':[[{'source_index':0,'page_index':0}],[{'source_index':0,'page_index':99}]]}
    assert not split(invalid)['ok'] and not rejected.exists()
    assert set(base.iterdir())==before  # Includes cleanup of the successfully staged first part.
    cancelled=base/'cancelled'
    active={**request,'output':str(cancelled),'groups':[[{'source_index':0,'page_index':0}]]*50}
    process=subprocess.Popen([str(worker)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        process.stdin.write(json.dumps({'request':active,'cancel_on_disconnect':True})+'\n');process.stdin.flush()
        deadline=time.monotonic()+30
        while not any(p.is_dir() and (p/'parts/part-0001.pdf').exists() for p in base.iterdir() if p not in before):
            assert process.poll() is None,'Worker exited before the staged-part cancellation checkpoint'
            assert time.monotonic()<deadline,'No staged first part within the cancellation test budget'
            time.sleep(.02)
        assert not cancelled.exists()
        process.stdin.write('cancel\n');process.stdin.flush()
        stdout,stderr=process.communicate(timeout=30)
        reply=json.loads(stdout)
        assert not reply['ok'] and reply['error']['code']=='cancelled',(reply,stderr)
    finally:
        if process.poll() is None: process.kill();process.wait()
    assert not cancelled.exists() and set(base.iterdir())==before
    assert [hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs]==hashes
print('PDF split: individual/grouped parts, receipts, existing-folder protection, late-group rollback, cancellation after first staged part, complete cleanup and unchanged inputs passed')
