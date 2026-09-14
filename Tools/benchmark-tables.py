#!/usr/bin/env python3
"""Compare optimized native CLIs on identical local CSV fixtures (macOS only).

This measures end-to-end wall time and peak per-process RSS, not GUI memory or
cold-cache startup. Every measured output is independently decoded and checked.
"""
import argparse
import csv
from datetime import datetime, timezone
import hashlib
import io
import json
import platform
import re
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--runs', type=int, default=5)
args = parser.parse_args()
if platform.system() != 'Darwin':
    parser.error('This measurement uses macOS /usr/bin/time -l; Windows measurements need their own runner.')
if not 3 <= args.runs <= 20:
    parser.error('--runs must be between 3 and 20')
root = Path(__file__).resolve().parent.parent
executables = {'swift': root / '.build/release/fileform', 'rust': root / 'target/release/fileform-native'}
for binary in executables.values():
    if not binary.is_file():
        parser.error(f'Build the release CLI first: {binary}')
report = {
    'recordedAt': datetime.now(timezone.utc).isoformat(),
    'sourceCommit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    'sourceHadUncommittedChanges': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=root, text=True).strip()),
    'scope': 'CSV to JSON CLI conversion, process startup through verified output and receipt; warm filesystem caches',
    'limitations': ['No Electron or SwiftUI memory measurement', 'No claim of identical implementation work', 'Sequential local runs; other machine activity is uncontrolled', 'Output formatting differs; semantic cells are compared', 'Peak RSS is per process, not total system memory'],
    'machine': {'os': platform.platform(), 'model': subprocess.check_output(['sysctl', '-n', 'hw.model'], text=True).strip(), 'memoryBytes': int(subprocess.check_output(['sysctl', '-n', 'hw.memsize'], text=True))},
    'versions': {name: subprocess.check_output(command, text=True).strip() for name, command in [('rust', ['rustc', '--version']), ('swift', ['swift', '--version'])]},
    'binaries': {name: {'sha256': hashlib.sha256(binary.read_bytes()).hexdigest()} for name, binary in executables.items()},
    'cases': [],
}
with tempfile.TemporaryDirectory(prefix='fileform-benchmark-') as temporary:
    folder = Path(temporary)
    for label, count, width in [('small', 100, 4), ('rows', 50_000, 6), ('wide', 2_000, 100)]:
        source = folder / f'{label}.csv'
        columns = [f'column_{index}' for index in range(width)]
        stream = io.StringIO(newline='')
        writer = csv.writer(stream)
        writer.writerow(columns)
        for row in range(count):
            writer.writerow([f'{row:06d}' if column == 0 else ('日本,語\nquoted "cell"' if column == 1 else f'value-{column}') for column in range(width)])
        content = stream.getvalue().encode('utf-8')
        assert len(content) <= 8 * 1024 * 1024
        source.write_bytes(content)
        expected = list(csv.DictReader(io.StringIO(content.decode('utf-8'), newline='')))
        samples = {'swift': [], 'rust': []}
        # One unrecorded warmup per CLI, followed by alternating order.
        for run in range(args.runs + 1):
            for name in (['swift', 'rust'] if run % 2 == 0 else ['rust', 'swift']):
                output = folder / f'{label}-{name}-{run}.json'
                command = ([str(executables[name]), 'convert', str(source), '--to', 'json', '--output', str(output), '--json'] if name == 'swift' else [str(executables[name]), 'convert-table', str(source), str(output)])
                start = time.perf_counter()
                result = subprocess.run(['/usr/bin/time', '-l', *command], capture_output=True, text=True, timeout=120)
                seconds = time.perf_counter() - start
                if result.returncode:
                    raise RuntimeError(f'{name} failed: {result.stderr[:2000]} {result.stdout[:2000]}')
                peak = re.search(r'^\s*(\d+)\s+maximum resident set size\s*$', result.stderr, re.MULTILINE)
                if not peak:
                    raise RuntimeError('Peak RSS was not reported by /usr/bin/time')
                assert json.loads(output.read_text()) == expected, f'{name} changed cell values'
                assert source.read_bytes() == content
                if run:
                    samples[name].append({'wallSeconds': seconds, 'peakRSSBytes': int(peak.group(1)), 'outputBytes': output.stat().st_size})
                output.unlink()
        summary = {name: {'medianWallSeconds': statistics.median(s['wallSeconds'] for s in values), 'medianPeakRSSBytes': statistics.median(s['peakRSSBytes'] for s in values), 'samples': values} for name, values in samples.items()}
        report['cases'].append({'name': label, 'rows': count, 'columns': width, 'inputBytes': len(content), 'inputSHA256': hashlib.sha256(content).hexdigest(), 'results': summary})
        print(f'{label}: Swift {summary["swift"]["medianWallSeconds"]:.3f}s; Rust {summary["rust"]["medianWallSeconds"]:.3f}s', flush=True)
output = root / 'Artifacts/Verification/table-benchmark.json'
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(report, indent=2) + '\n')
print(f'Verified comparison saved to {output}')
