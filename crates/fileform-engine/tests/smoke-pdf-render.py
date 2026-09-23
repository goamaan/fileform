#!/usr/bin/env python3
"""Real helper acceptance; pass helper executable and verified qpdf executable."""
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
from pdf_fixtures import fixture

helper = Path(sys.argv[1]).resolve()
qpdf = Path(sys.argv[2]).resolve()

def run(*args):
    return subprocess.run([str(x) for x in args], capture_output=True, timeout=60)

def raster(path, page=0, edge=512):
    result = run(helper, path, page, edge)
    assert result.returncode == 0, result.stderr
    magic, dimensions, maximum, data = result.stdout.split(b'\n', 3)
    assert magic == b'P6' and maximum == b'255'
    width, height = map(int, dimensions.split())
    assert 1 <= width <= edge and 1 <= height <= edge
    assert len(data) == width * height * 3
    return width, height, data

with tempfile.TemporaryDirectory(prefix='fileform-render-') as folder:
    base = Path(folder)
    source = base / 'text and shapes ü.pdf'
    fixture(source, text=True)
    original = hashlib.sha256(source.read_bytes()).hexdigest()
    first = raster(source)
    second = raster(source, page=1)
    assert first[:2] == (342, 512) and second[:2] == (512, 342)
    assert b'\xff\x00\x00' in first[2] and b'\x00\x00\xff' in second[2]
    assert b'\x00\x00\x00' in first[2]  # Standard-font text actually rendered.
    assert raster(source, edge=1)[:2] == (1, 1)
    assert raster(source, edge=2048)[:2] == (400, 600)  # Two pixels/point cap.
    rewritten = base / 'rewritten.pdf'
    rewrite = run(qpdf, '--object-streams=generate', '--stream-data=compress', source, rewritten)
    assert rewrite.returncode == 0, rewrite.stderr
    assert raster(rewritten) == first
    changed = base / 'changed.pdf'
    changed.write_bytes(source.read_bytes().replace(b'1 0 0 rg', b'0 1 0 rg'))
    assert raster(changed) != first
    plain = base / 'shapes.pdf'
    fixture(plain)
    assert raster(plain) != first  # Text removal is detected.
    inherited = base / 'inherited.pdf'
    fixture(inherited, inherited=True)
    assert raster(inherited)[:2] == (512, 342)  # Crop 200x300, rotation 270.
    malformed = base / 'bad.pdf'
    malformed.write_bytes(b'not a PDF')
    for path, page, edge in [(malformed, 0, 512), (source, 2, 512),
                              (source, -1, 512), (source, 0, 2049),
                              (source, '0x0', 512), (source, 0, 0),
                              (base, 0, 512)]:
        failure = run(helper, path, page, edge)
        assert failure.returncode != 0 and not failure.stdout
    protected = base / 'protected.pdf'
    encrypted = run(qpdf, '--encrypt', 'user-password', 'owner-password', '256', '--', source, protected)
    assert encrypted.returncode == 0, encrypted.stderr
    assert run(helper, protected, 0, 512).returncode != 0
    assert hashlib.sha256(source.read_bytes()).hexdigest() == original
print('PDF renderer: pixels, text presence, rewrite equality, change detection, crop/rotation, bounds, Unicode path, invalid/protected input and source safety passed')
