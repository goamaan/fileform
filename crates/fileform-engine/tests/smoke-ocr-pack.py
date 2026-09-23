#!/usr/bin/env python3
"""Real native CLI/worker verification of OCR executable and model integrity."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve()
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
def verify(directory):
    return subprocess.run([str(cli),'verify-ocr-pack',str(directory)],capture_output=True,timeout=30)
result=verify(pack)
assert result.returncode==0,result.stderr
receipt=json.loads(result.stdout)
assert receipt['recognition_languages']==['eng'] and not receipt['automatic_language_detection']
assert set(receipt['assets'])=={'tessdata/eng.traineddata','tessdata/osd.traineddata'}
response=subprocess.run([str(worker)],input=json.dumps({'operation':'verify_ocr_pack','directory':str(pack)})+'\n',capture_output=True,text=True,check=True,timeout=30)
assert json.loads(response.stdout)['result']==receipt
with tempfile.TemporaryDirectory(prefix='fileform-ocr-pack-') as folder:
    local=Path(folder)
    shutil.copytree(pack/'bin',local/'bin')
    shutil.copytree(pack/'tessdata',local/'tessdata')
    shutil.copy2(pack/'manifest.json',local/'manifest.json')
    model=local/'tessdata/eng.traineddata'
    with model.open('ab') as output: output.write(b'tampered')
    assert verify(local).returncode!=0
    model.unlink()
    assert verify(local).returncode!=0
    shutil.copy2(pack/'tessdata/eng.traineddata',model)
    manifest=json.loads((local/'manifest.json').read_text(encoding='utf-8'))
    manifest['networkProtocols']=True
    (local/'manifest.json').write_text(json.dumps(manifest),encoding='utf-8')
    assert verify(local).returncode!=0
print('OCR pack: real CLI/worker receipts, explicit English-only capability, modified/missing model rejection and offline-policy checks passed')
