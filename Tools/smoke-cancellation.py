#!/usr/bin/env python3
"""Cancel a real worker after output staging begins; verify cleanup on both OSes."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

root = Path(__file__).resolve().parent.parent
worker = root / 'target/release' / ('fileform-worker.exe' if os.name == 'nt' else 'fileform-worker')
with tempfile.TemporaryDirectory(prefix='fileform-cancel-') as temp:
    folder = Path(temp)
    snapshots = folder / 'snapshots'
    outputs = folder / 'outputs'
    snapshots.mkdir()
    outputs.mkdir()
    source = folder / 'large.csv'
    content = b'name,value\n' + (b'example,' + b'1' * 64 + b'\n') * 90_000
    source.write_bytes(content)
    output = outputs / 'result.json'
    env = dict(os.environ, TMPDIR=str(snapshots), TMP=str(snapshots), TEMP=str(snapshots))
    process = subprocess.Popen([str(worker)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    try:
        process.stdin.write((json.dumps({'operation': 'convert_table', 'input': str(source), 'output': str(output)}) + '\n').encode())
        process.stdin.flush()
        deadline = time.monotonic() + 15
        while not list(outputs.iterdir()):
            assert process.poll() is None, 'Worker finished before staging was observed'
            assert time.monotonic() < deadline, 'No output staging observed'
            time.sleep(0.001)
        process.stdin.write(b'cancel\n')
        process.stdin.flush()
        stdout, stderr = process.communicate(timeout=5)
        reply = json.loads(stdout)
        assert process.returncode == 1 and reply['error']['code'] == 'cancelled', reply
        assert not list(outputs.iterdir()), 'Output or staging survived cancellation'
        assert not list(snapshots.iterdir()), 'Source snapshot survived cancellation'
        assert source.read_bytes() == content
    finally:
        if process.poll() is None:
            process.kill()
            process.communicate()
    evidence = root / 'Artifacts/Verification/portable-cancellation.json'
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text(json.dumps({'platform': os.name, 'cancelledDuringStaging': True, 'outputAbsent': True, 'stagingCleaned': True, 'snapshotsCleaned': True, 'originalUnchanged': True}, indent=2) + '\n')
print('Cooperative worker cancellation and cleanup passed.')
