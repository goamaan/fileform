#!/usr/bin/env python3
"""Independent PNG fixtures through the real native CLI and worker."""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib

root = Path(__file__).resolve().parent.parent
suffix = '.exe' if os.name == 'nt' else ''
cli = root / 'target/release' / ('fileform-native' + suffix)
worker = root / 'target/release' / ('fileform-worker' + suffix)
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
def png(width, height, pixels, depth=8, exif=None, icc=None):
    stride = width * 4 * (depth // 8)
    scanlines = b''.join(b'\x00' + pixels[row * stride:(row + 1) * stride] for row in range(height))
    metadata = chunk(b'eXIf', exif) if exif is not None else b''
    if icc is not None:
        metadata += chunk(b'iCCP', b'Fileform QA\x00\x00' + zlib.compress(icc))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, depth, 6, 0, 0, 0)) + metadata + chunk(b'IDAT', zlib.compress(scanlines)) + chunk(b'IEND', b'')
with tempfile.TemporaryDirectory(prefix='fileform-image-') as temp:
    folder = Path(temp)
    source = folder / 'pixels.png'
    pixels = bytes([255, 0, 0, 0, 0, 128, 255, 127])
    content = png(2, 1, pixels)
    source.write_bytes(content)
    result = subprocess.run([str(cli), 'inspect-image', str(source)], capture_output=True, text=True, check=True)
    info = json.loads(result.stdout)
    assert info['kind'] == 'image_inspection' and info['width'] == 2 and info['height'] == 1
    assert info['has_alpha'] and not info['conversion_available']
    assert info['decoded_rgba_sha256'] == hashlib.sha256(pixels).hexdigest()
    request = json.dumps({'operation':'inspect_image', 'input':str(source)})
    reply = subprocess.run([str(worker)], input=request, capture_output=True, text=True, check=True)
    assert json.loads(reply.stdout)['result'] == info
    layouts = ['ABCDEF', 'BADCFE', 'FEDCBA', 'EFCDAB', 'ACEBDF', 'ECAFDB', 'FDBECA', 'BDFACE']
    orientation_pixels = b''.join(bytes([n, 0, 0, n]) for n in b'ABCDEF')
    for orientation, layout in enumerate(layouts, 1):
        exif = b'II' + struct.pack('<HIH', 42, 8, 1) + struct.pack('<HHIHHI', 0x112, 3, 1, orientation, 0, 0)
        path = folder / f'orientation-{orientation}.png'
        original = png(2, 3, orientation_pixels, exif=exif)
        path.write_bytes(original)
        result = subprocess.run([str(cli), 'inspect-image', str(path)], capture_output=True, text=True, check=True)
        oriented = json.loads(result.stdout)
        expected_pixels = b''.join(bytes([n, 0, 0, n]) for n in layout.encode())
        assert oriented['orientation'] == orientation
        assert (oriented['display_width'], oriented['display_height']) == ((2, 3) if orientation < 5 else (3, 2))
        assert oriented['oriented_rgba_sha256'] == hashlib.sha256(expected_pixels).hexdigest()
        assert path.read_bytes() == original
    subprocess.run(['cargo','run','--quiet','--release','--locked','--example','write_image_profiles','--',str(folder)], cwd=root, check=True)
    color_pixels = bytes([200,100,50,37,0,0,0,0])
    color_checks = {}
    for name in ['srgb','display-p3']:
        path = folder / (name + '.png')
        tagged = png(2,1,color_pixels,icc=(folder / (name + '.icc')).read_bytes())
        path.write_bytes(tagged)
        checked = json.loads(subprocess.run([str(cli),'inspect-image',str(path)],capture_output=True,text=True,check=True).stdout)
        assert checked['has_icc'] and checked['icc_srgb_rgba_sha256']
        if name == 'srgb':
            assert checked['icc_srgb_rgba_sha256'] == hashlib.sha256(color_pixels).hexdigest()
        else:
            assert checked['icc_srgb_rgba_sha256'] != checked['oriented_rgba_sha256']
        assert path.read_bytes() == tagged
        color_checks[name] = checked['icc_srgb_rgba_sha256']
    rejected = []
    for name, data in [('malformed-icc', png(2,1,pixels,icc=b'invalid')), ('malformed-exif', png(2, 1, pixels, exif=b'bad')), ('missing-end', content[:-12]), ('trailing-bytes', content + b'extra'), ('animated', content[:33] + chunk(b'acTL', struct.pack('>II', 2, 0)) + content[33:]), ('oversized-dimensions', png(80_000_001, 1, b'\x00'*4)), ('high-depth', png(2, 1, b'\x00'*16, 16))]:
        path = folder / (name + '.png')
        path.write_bytes(data)
        failure = subprocess.run([str(cli), 'inspect-image', str(path)], capture_output=True, text=True)
        assert failure.returncode != 0, name
        rejected.append(name)
    assert source.read_bytes() == content
    evidence = root / 'Artifacts/Verification/portable-images.json'
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text(json.dumps({'platform':os.name,'cliAndWorkerMatch':True,'exactRGBAPixels':True,'allEightOrientationsVerified':True,'iccProfileChecks':color_checks,'originalUnchanged':True,'rejected':rejected,'scope':'PNG pixel inspection only; no portable image export yet'}, indent=2) + '\n')
print('Native PNG inspection and independent pixel checks passed.')
