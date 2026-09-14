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
def png(width, height, pixels, depth=8):
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, depth, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\x00' + pixels)) + chunk(b'IEND', b'')
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
    rejected = []
    for name, data in [('missing-end', content[:-12]), ('trailing-bytes', content + b'extra'), ('animated', content[:33] + chunk(b'acTL', struct.pack('>II', 2, 0)) + content[33:]), ('oversized-dimensions', png(80_000_001, 1, b'\x00'*4)), ('high-depth', png(2, 1, b'\x00'*16, 16))]:
        path = folder / (name + '.png')
        path.write_bytes(data)
        failure = subprocess.run([str(cli), 'inspect-image', str(path)], capture_output=True, text=True)
        assert failure.returncode != 0, name
        rejected.append(name)
    assert source.read_bytes() == content
    evidence = root / 'Artifacts/Verification/portable-images.json'
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text(json.dumps({'platform':os.name,'cliAndWorkerMatch':True,'exactRGBAPixels':True,'originalUnchanged':True,'rejected':rejected,'scope':'PNG pixel inspection only; no portable image export yet'}, indent=2) + '\n')
print('Native PNG inspection and independent pixel checks passed.')
