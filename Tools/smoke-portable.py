#!/usr/bin/env python3
"""Actual worker/CLI process check; intentionally usable on Windows and macOS."""
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
suffix = '.exe' if os.name == 'nt' else ''
worker = root / 'target/release' / ('fileform-worker' + suffix)
cli = root / 'target/release' / ('fileform-native' + suffix)
with tempfile.TemporaryDirectory(prefix='fileform-smoke-') as directory:
    folder = Path(directory)
    source = folder / 'Unicode table.csv'
    content = 'name,value,note\r\nαlpha,001,"line 1\nline 2"\r\nbeta,2,\r\n'.encode()
    source.write_bytes(content)
    request = {'operation': 'inspect', 'input': str(source)}
    result = subprocess.run([str(worker)], input=json.dumps(request), encoding='utf-8', capture_output=True, check=True)
    inspected = json.loads(result.stdout)
    assert inspected['ok'] and inspected['result']['rows'] == 2
    output = folder / 'result.json'
    converted = subprocess.run([str(cli), 'convert-table', str(source), str(output)], encoding='utf-8', capture_output=True, check=True)
    with source.open(encoding='utf-8', newline='') as file:
        expected = list(csv.DictReader(file))
    assert json.loads(output.read_text(encoding='utf-8')) == expected
    before = output.read_bytes()
    duplicate = subprocess.run([str(cli), 'convert-table', str(source), str(output)], encoding='utf-8', capture_output=True)
    assert duplicate.returncode != 0 and output.read_bytes() == before
    assert source.read_bytes() == content
    malformed = subprocess.run([str(worker)], input='{"operation":"unknown"}', encoding='utf-8', capture_output=True)
    assert malformed.returncode != 0 and not json.loads(malformed.stdout)['ok']
    report = {'platform': os.name, 'worker': True, 'cli': True, 'unicodeAndValuesRetained': True,
              'collisionRejected': True, 'originalUnchanged': True,
              'outputSHA256': hashlib.sha256(before).hexdigest(), 'receipt': json.loads(converted.stdout)}
    # The temporary absolute output path is not part of retained evidence.
    report['receipt']['output'] = output.name
    evidence = root / 'Artifacts/Verification/portable-smoke.json'
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print('Portable worker and CLI smoke passed.')
