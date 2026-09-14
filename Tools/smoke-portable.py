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
    delimited_outputs = []
    for extension, delimiter in [('csv', ','), ('tsv', '\t')]:
        target = folder / ('converted.' + extension)
        request = {'operation': 'convert_table', 'input': str(source), 'output': str(target),
                   'expected_source_sha256': inspected['result']['sha256']}
        reply = subprocess.run([str(worker)], input=json.dumps(request), encoding='utf-8', capture_output=True, check=True)
        assert json.loads(reply.stdout)['ok']
        with target.open(encoding='utf-8', newline='') as file:
            assert list(csv.DictReader(file, delimiter=delimiter)) == expected
        roundtrip = folder / ('roundtrip-' + extension + '.json')
        subprocess.run([str(cli), 'convert-table', str(target), str(roundtrip)], capture_output=True, check=True)
        assert json.loads(roundtrip.read_text(encoding='utf-8')) == expected
        original_output = target.read_bytes()
        duplicate_output = subprocess.run([str(worker)], input=json.dumps(request), encoding='utf-8', capture_output=True)
        assert duplicate_output.returncode != 0 and target.read_bytes() == original_output
        delimited_outputs.append(extension)
    json_source = folder / 'numbers.json'
    json_source.write_text('[{"n":9007199254740993,"x":-0.001200e+999,"empty":null,"flag":true}]', encoding='utf-8')
    json_result = folder / 'numbers.tsv'
    subprocess.run([str(cli), 'convert-table', str(json_source), str(json_result)], capture_output=True, check=True)
    with json_result.open(encoding='utf-8', newline='') as file:
        assert list(csv.reader(file, delimiter='\t')) == [['n', 'x', 'empty', 'flag'], ['9007199254740993', '-0.001200e+999', '', 'true']]
    before = output.read_bytes()
    duplicate = subprocess.run([str(cli), 'convert-table', str(source), str(output)], encoding='utf-8', capture_output=True)
    assert duplicate.returncode != 0 and output.read_bytes() == before
    assert source.read_bytes() == content
    malformed = subprocess.run([str(worker)], input='{"operation":"unknown"}', encoding='utf-8', capture_output=True)
    assert malformed.returncode != 0 and not json.loads(malformed.stdout)['ok']
    report = {'platform': os.name, 'worker': True, 'cli': True, 'unicodeAndValuesRetained': True,
              'flatJSONNumericLexemesRetained': True, 'delimitedOutputs': delimited_outputs, 'collisionRejected': True, 'originalUnchanged': True,
              'outputSHA256': hashlib.sha256(before).hexdigest(), 'receipt': json.loads(converted.stdout)}
    # The temporary absolute output path is not part of retained evidence.
    report['receipt']['output'] = output.name
    evidence = root / 'Artifacts/Verification/portable-smoke.json'
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print('Portable worker and CLI smoke passed.')
