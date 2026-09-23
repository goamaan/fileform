#!/usr/bin/env python3
"""Real lossless compression, preservation and no-publication failure paths."""
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
cli = root / 'target/release' / ('fileform-native' + suffix)
worker = root / 'target/release' / ('fileform-worker' + suffix)
def run(*args):
    return subprocess.run([str(v) for v in args], capture_output=True, timeout=180)
def compress(source, output, *options):
    return run(cli, 'compress-pdf', source, output, pdf, renderer, *options)
with tempfile.TemporaryDirectory(prefix='fileform-pdf-optimize-') as folder:
    base = Path(folder)
    # A comment before xref offers removable bytes without changing objects.
    for name, create in [('text', lambda p: fixture(p, text=True)),
                         ('geometry', lambda p: fixture(p, inherited=True)),
                         ('image', visual_fixture), ('unicode', unicode_fixture)]:
        source = base / f'{name}.pdf'
        create(source)
        raw = source.read_bytes()
        xref = int(raw.rsplit(b'startxref\n', 1)[1].split(b'\n')[0])
        padding = b'%' + b' removable comment' * 1000 + b'\n'
        raw = raw[:xref] + padding + raw[xref:]
        prefix, ending = raw.rsplit(b'startxref\n', 1)
        source.write_bytes(prefix + b'startxref\n' + str(xref + len(padding)).encode() + b'\n' + ending.split(b'\n', 1)[1])
        original = source.read_bytes()
        output = base / f'{name}-compressed.pdf'
        result = compress(source, output)
        assert result.returncode == 0, (name, result.stderr)
        receipt = json.loads(result.stdout)
        assert receipt['status'] == 'saved' and receipt['output_bytes'] < len(original)
        assert receipt['output_bytes'] == output.stat().st_size
        assert receipt['output_sha256'] == hashlib.sha256(output.read_bytes()).hexdigest()
        assert receipt['source_sha256'] == hashlib.sha256(original).hexdigest()
        graph = json.loads(run(cli, 'inspect-pdf-graph', output, pdf).stdout)
        assert graph['graph_sha256'] == receipt['graph_sha256']
        assert source.read_bytes() == original
        saved = output.read_bytes()
        assert compress(source, output).returncode != 0 and output.read_bytes() == saved
    source = base / 'text.pdf'
    worker_output = base / 'worker.pdf'
    request = {'operation':'optimize_pdf', 'input':str(source), 'output':str(worker_output),
               'directory':str(pdf), 'renderer_directory':str(renderer)}
    result = subprocess.run([str(worker)], input=json.dumps(request)+'\n', text=True,
                            capture_output=True, check=True, timeout=180)
    receipt = json.loads(result.stdout)['result']
    assert receipt['status'] == 'saved' and receipt['pages_verified'] == 2
    unchanged = base / 'not-smaller.pdf'
    result = compress(worker_output, unchanged)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)['status'] == 'not_smaller' and not unchanged.exists()
    too_small = base / 'too-small.pdf'
    result = compress(source, too_small, '--max-bytes', 10)
    assert result.returncode != 0 and b'target_unmet' in result.stderr and not too_small.exists()
    fit = base / 'fit.pdf'
    result = compress(source, fit, '--max-bytes', 10000)
    assert result.returncode == 0 and fit.stat().st_size <= 10000, result.stderr
    annotated = base / 'annotated.pdf'
    fixture(annotated, annotated=True)
    rejected = base / 'rejected.pdf'
    result = compress(annotated, rejected)
    assert result.returncode != 0 and b'unsupported' in result.stderr and not rejected.exists()
    unicode_fixture(annotated, invalid=True)
    assert compress(annotated, rejected).returncode != 0 and not rejected.exists()
print('Lossless PDF: all-page text/geometry/image/Unicode preservation, CLI/worker, smaller output, not-smaller retention, fit limits, collision and unsupported rejection passed')
