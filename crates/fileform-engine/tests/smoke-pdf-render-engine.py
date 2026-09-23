#!/usr/bin/env python3
"""Exercise the real CLI/worker renderer and verified pack with generated input."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from pdf_fixtures import fixture, visual_fixture

root = Path(__file__).resolve().parents[3]
pack = Path(sys.argv[1]).resolve()
suffix = '.exe' if sys.platform == 'win32' else ''
cli = root / 'target/release' / ('fileform-native' + suffix)
worker = root / 'target/release' / ('fileform-worker' + suffix)
def run(*args):
    return subprocess.run([str(v) for v in args], capture_output=True, timeout=90)
with tempfile.TemporaryDirectory(prefix='fileform-render-engine-') as folder:
    base = Path(folder)
    source = base / 'source ü.pdf'
    fixture(source, text=True)
    original = source.read_bytes()
    output = base / 'page.png'
    result = run(cli, 'render-pdf-page', source, output, pack, 0)
    assert result.returncode == 0, result.stderr
    receipt = json.loads(result.stdout)
    assert (receipt['width'], receipt['height'], receipt['page_index']) == (342, 512, 0)
    assert receipt['source_sha256'] == hashlib.sha256(original).hexdigest()
    saved = output.read_bytes()
    assert saved.startswith(b'\x89PNG\r\n\x1a\n')
    assert receipt['sha256'] == hashlib.sha256(saved).hexdigest()
    assert receipt['bytes'] == len(saved)
    assert run(cli, 'render-pdf-page', source, output, pack, 0).returncode != 0
    assert output.read_bytes() == saved
    worker_output = base / 'worker.png'
    request = {'operation': 'render_pdf_page', 'input': str(source), 'output': str(worker_output),
               'directory': str(pack), 'page_index': 0, 'max_dimension': 512}
    result = subprocess.run([str(worker)], input=json.dumps(request)+'\n', text=True,
                            capture_output=True, timeout=90, check=True)
    worker_receipt = json.loads(result.stdout)['result']
    assert worker_receipt['sha256'] == receipt['sha256']
    assert worker_output.read_bytes() == saved
    visual = base / 'geometry.pdf'
    visual_fixture(visual)
    media_output = base / 'media.png'
    media = run(cli, 'render-pdf-page', visual, media_output, pack, 0, '--media-box')
    assert media.returncode == 0, media.stderr
    media_receipt = json.loads(media.stdout)
    assert (media_receipt['width'], media_receipt['height'], media_receipt['page_box']) == (160, 200, 'media')
    assert receipt['page_box'] == 'crop'
    request.update(input=str(visual), output=str(base / 'worker-media.png'), page_box='media')
    result = subprocess.run([str(worker)], input=json.dumps(request)+'\n', text=True,
                            capture_output=True, timeout=90, check=True)
    assert json.loads(result.stdout)['result']['sha256'] == media_receipt['sha256']
    for page in [2, 1000]:
        rejected = base / f'rejected-{page}.png'
        assert run(cli, 'render-pdf-page', source, rejected, pack, page).returncode != 0
        assert not rejected.exists()
    bad_pack = base / 'tampered-pack'
    shutil.copytree(pack, bad_pack)
    library = bad_pack / 'bin' / ('pdfium.dll' if sys.platform == 'win32' else 'libpdfium.dylib')
    with library.open('ab') as stream:
        stream.write(b'tampered')
    rejected = base / 'tampered.png'
    result = run(cli, 'render-pdf-page', source, rejected, bad_pack, 0)
    assert result.returncode != 0 and not rejected.exists()
    assert b'engine_unavailable' in result.stderr
    assert source.read_bytes() == original
print('PDF engine render: CLI/worker PNG parity, dimensions/hashes, no-clobber, invalid-page cleanup, library tamper rejection and unchanged input passed')
