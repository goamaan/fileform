#!/usr/bin/env python3
"""Real helper acceptance; pass helper executable and verified qpdf executable."""
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
from pdf_fixtures import fixture, visual_fixture, unicode_fixture

helper = Path(sys.argv[1]).resolve()
qpdf = Path(sys.argv[2]).resolve()

def run(*args):
    return subprocess.run([str(x) for x in args], capture_output=True, timeout=60)

def raster(path, page=0, edge=512, box="crop"):
    result = run(helper, path, page, edge, box)
    assert result.returncode == 0, result.stderr
    magic, dimensions, maximum, data = result.stdout.split(b'\n', 3)
    assert magic == b'P6' and maximum == b'255'
    width, height = map(int, dimensions.split())
    assert 1 <= width <= edge and 1 <= height <= edge
    assert len(data) == width * height * 3
    return width, height, data

def text(path, page=0):
    result = run(helper, path, page, 'text')
    assert result.returncode == 0, result.stderr
    magic, counts, payload = result.stdout.split(b'\n', 2)
    scalars, length = map(int, counts.split())
    value = payload.decode('utf-8')
    assert magic == b'FT1' and len(value) == scalars and len(payload) == length
    return value

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
    assert text(source) == text(rewritten) == 'Fileform page 1'
    assert text(source, 1) == 'Fileform page 2'
    changed = base / 'changed.pdf'
    changed.write_bytes(source.read_bytes().replace(b'1 0 0 rg', b'0 1 0 rg'))
    assert raster(changed) != first
    plain = base / 'shapes.pdf'
    fixture(plain)
    assert raster(plain) != first  # Text removal is detected.
    assert text(plain) == ''
    unicode = base / 'unicode.pdf'
    unicode_fixture(unicode)
    assert text(unicode) == 'Ω中😀́'
    for invalid, supplementary in [(True, b'D83DDE00'), (False, b'D83D'), (False, b'DE00')]:
        unicode_fixture(unicode, invalid=invalid, supplementary=supplementary)
        result = run(helper, unicode, 0, 'text')
        assert result.returncode != 0 and not result.stdout
    inherited = base / 'inherited.pdf'
    fixture(inherited, inherited=True)
    assert raster(inherited)[:2] == (512, 342)  # Crop 200x300, rotation 270.
    visual = base / 'image-and-transparency.pdf'
    visual_fixture(visual)
    crop = raster(visual)
    media = raster(visual, box='media')
    assert crop[:2] == (120, 120) and media[:2] == (160, 200)
    def pixel(image, x, y):
        offset = (y * image[0] + x) * 3
        return tuple(image[2][offset:offset+3])
    assert pixel(crop, 20, 60) == (255, 0, 0)  # Embedded image red quadrant.
    assert pixel(crop, 60, 60) == (0, 255, 0)
    assert all(126 <= value <= 129 for value in pixel(crop, 20, 100))  # Yellow over blue.
    for rotation in [90, 180, 270]:
        rotated = base / f'rotation-{rotation}.pdf'
        visual_fixture(rotated, rotation=rotation)
        rendered = raster(rotated)
        for x, y in [(20, 60), (60, 60), (20, 100), (100, 20)]:
            rx, ry = {90: (119-y, x), 180: (119-x, 119-y), 270: (y, 119-x)}[rotation]
            assert pixel(rendered, rx, ry) == pixel(crop, x, y)
        assert raster(rotated, box='media')[:2] == ((200, 160) if rotation != 180 else (160, 200))
    visual_rewritten = base / 'visual-rewritten.pdf'
    assert run(qpdf, '--object-streams=generate', '--stream-data=compress', visual, visual_rewritten).returncode == 0
    assert raster(visual_rewritten) == crop and raster(visual_rewritten, box='media') == media
    outside = base / 'outside-crop.pdf'
    outside.write_bytes(visual.read_bytes().replace(b'0 1 1 rg', b'1 0 1 rg'))
    assert raster(outside) == crop  # Cropped previews alone miss this change.
    assert raster(outside, box='media') != media
    unit = base / 'user-unit.pdf'
    visual_fixture(unit, user_unit=2)
    assert raster(unit) == crop  # PDFium ignores UserUnit: independent geometry is required.
    assert run(helper, visual, 0, 512, 'invalid-box').returncode != 0
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
print('PDF renderer: pixels, Unicode extraction/rejection, text presence, rewrite equality, change detection, crop/media bounds, all rotations, embedded image/transparency, UserUnit limitation, bounds, Unicode path, invalid/protected input and source safety passed')
