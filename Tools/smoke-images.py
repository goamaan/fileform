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
def png(width, height, pixels, depth=8, exif=None, icc=None, extra=b''):
    stride = width * 4 * (depth // 8)
    scanlines = b''.join(b'\x00' + pixels[row * stride:(row + 1) * stride] for row in range(height))
    metadata = chunk(b'eXIf', exif) if exif is not None else b''
    if icc is not None:
        metadata += chunk(b'iCCP', b'Fileform QA\x00\x00' + zlib.compress(icc))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, depth, 6, 0, 0, 0)) + metadata + extra + chunk(b'IDAT', zlib.compress(scanlines)) + chunk(b'IEND', b'')
with tempfile.TemporaryDirectory(prefix='fileform-image-') as temp:
    folder = Path(temp)
    source = folder / 'pixels.png'
    pixels = bytes([255, 0, 0, 0, 0, 128, 255, 127])
    content = png(2, 1, pixels)
    source.write_bytes(content)
    result = subprocess.run([str(cli), 'inspect-image', str(source)], capture_output=True, text=True, check=True)
    info = json.loads(result.stdout)
    assert info['kind'] == 'image_inspection' and info['width'] == 2 and info['height'] == 1
    assert info['has_alpha'] and info['conversion_available']
    assert info['decoded_rgba_sha256'] == hashlib.sha256(pixels).hexdigest()
    request = json.dumps({'operation':'inspect_image', 'input':str(source)})
    reply = subprocess.run([str(worker)], input=request, capture_output=True, text=True, check=True)
    assert json.loads(reply.stdout)['result'] == info
    preview_reply = subprocess.run([str(worker)],input=json.dumps({'operation':'inspect_image','input':str(source),'preview':True}),capture_output=True,text=True,check=True)
    preview = json.loads(preview_reply.stdout)['result']['preview']
    assert preview['width']==2 and preview['height']==1
    assert preview['rgba']==[0,0,0,0,0,128,255,127]
    assert len(preview_reply.stdout.encode()) < 1_048_576
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
    gamma_pixels = bytes([128,128,128,42,0,0,0,0])
    gamma_chunk = chunk(b'gAMA',struct.pack('>I',100000))
    color_cases = [
        ('linear-gamma',gamma_chunk,'gamma_chromaticities'),
        ('srgb-precedence',gamma_chunk + chunk(b'sRGB',b'\x00'),'srgb'),
        ('extended-color',chunk(b'cICP',bytes([9,16,0,1])),'extended_color_pending'),
    ]
    for name, metadata, interpretation in color_cases:
        path = folder / (name + '.png')
        path.write_bytes(png(2,1,gamma_pixels,extra=metadata))
        checked = json.loads(subprocess.run([str(cli),'inspect-image',str(path)],capture_output=True,text=True,check=True).stdout)
        assert checked['color_interpretation'] == interpretation, checked
        if name == 'linear-gamma':
            # Linear 128/255 maps to sRGB 188, with one code value CMS tolerance.
            expected_hashes = {hashlib.sha256(bytes([v,v,v,42,0,0,0,0])).hexdigest() for v in [187,188,189]}
            assert checked['srgb_rgba_sha256'] in expected_hashes, checked
        elif name == 'srgb-precedence':
            assert checked['srgb_rgba_sha256'] == hashlib.sha256(gamma_pixels).hexdigest()
        else:
            assert checked['srgb_rgba_sha256'] is None and checked['icc_srgb_rgba_sha256'] is None
    exported = folder / 'exported.png'
    receipt = json.loads(subprocess.run([str(cli),'convert-image',str(source),str(exported)],capture_output=True,text=True,check=True).stdout)
    assert receipt['kind'] == 'saved_image'
    saved_info = json.loads(subprocess.run([str(cli),'inspect-image',str(exported)],capture_output=True,text=True,check=True).stdout)
    assert saved_info['decoded_rgba_sha256'] == hashlib.sha256(pixels).hexdigest()
    assert saved_info['orientation'] == 1 and saved_info['color_interpretation'] == 'srgb'
    before = exported.read_bytes()
    worker_output = folder / 'worker-export.png'
    reply = subprocess.run([str(worker)],input=json.dumps({'operation':'convert_image','input':str(source),'output':str(worker_output),'expected_source_sha256':info['sha256']}),capture_output=True,text=True,check=True)
    assert json.loads(reply.stdout)['ok'] and worker_output.read_bytes() == before
    collision = subprocess.run([str(cli),'convert-image',str(source),str(exported)],capture_output=True,text=True)
    assert collision.returncode != 0 and exported.read_bytes() == before
    no_background = folder / 'missing-background.jpg'
    denied_jpeg = subprocess.run([str(cli),'convert-image',str(source),str(no_background)],capture_output=True,text=True)
    assert denied_jpeg.returncode != 0 and not no_background.exists()
    for background in ['white','black']:
        jpeg = folder / (background + '.jpg')
        result = subprocess.run([str(worker)],input=json.dumps({'operation':'convert_image','input':str(source),'output':str(jpeg),'background':background}),capture_output=True,text=True,check=True)
        assert json.loads(result.stdout)['result']['width']==2
        data=jpeg.read_bytes()
        assert data[:2]==b'\xff\xd8' and data[-2:]==b'\xff\xd9'
        offset=2; dimensions=None; profile_found=False
        while offset<len(data)-2:
            assert data[offset]==255
            marker=data[offset+1]
            if marker==0xda: break
            length=struct.unpack('>H',data[offset+2:offset+4])[0]
            payload=data[offset+4:offset+2+length]
            if marker==0xc0: dimensions=struct.unpack('>HH',payload[1:5])[::-1]
            if marker==0xe2 and payload.startswith(b'ICC_PROFILE\0'): profile_found=True
            offset+=2+length
        assert dimensions==(2,1) and profile_found
        repeated=subprocess.run([str(cli),'convert-image',str(source),str(jpeg),'--background',background],capture_output=True,text=True)
        assert repeated.returncode!=0 and jpeg.read_bytes()==data
    stale_output = folder / 'stale.png'
    stale = subprocess.run([str(worker)],input=json.dumps({'operation':'convert_image','input':str(source),'output':str(stale_output),'expected_source_sha256':'0'*64}),capture_output=True,text=True)
    assert stale.returncode != 0 and not stale_output.exists()
    for orientation in range(1,9):
        oriented_input = folder / f'orientation-{orientation}.png'
        oriented_output = folder / f'export-{orientation}.png'
        subprocess.run([str(cli),'convert-image',str(oriented_input),str(oriented_output)],capture_output=True,text=True,check=True)
        saved = json.loads(subprocess.run([str(cli),'inspect-image',str(oriented_output)],capture_output=True,text=True,check=True).stdout)
        expected_pixels = b''.join(bytes([n,0,0,n]) for n in layouts[orientation-1].encode())
        assert saved['decoded_rgba_sha256'] == hashlib.sha256(expected_pixels).hexdigest()
        assert saved['orientation'] == 1
    rejected = []
    for name, data in [('bad-color-crc', png(2,1,pixels,extra=gamma_chunk[:-1]+bytes([gamma_chunk[-1]^1]))), ('duplicate-gamma', png(2,1,pixels,extra=gamma_chunk+gamma_chunk)), ('zero-gamma', png(2,1,pixels,extra=chunk(b'gAMA',struct.pack('>I',0)))), ('malformed-icc', png(2,1,pixels,icc=b'invalid')), ('malformed-exif', png(2, 1, pixels, exif=b'bad')), ('missing-end', content[:-12]), ('trailing-bytes', content + b'extra'), ('animated', content[:33] + chunk(b'acTL', struct.pack('>II', 2, 0)) + content[33:]), ('oversized-dimensions', png(80_000_001, 1, b'\x00'*4)), ('high-depth', png(2, 1, b'\x00'*16, 16))]:
        path = folder / (name + '.png')
        path.write_bytes(data)
        failure = subprocess.run([str(cli), 'inspect-image', str(path)], capture_output=True, text=True)
        assert failure.returncode != 0, name
        denied_output = folder / (name + '-denied.png')
        denied = subprocess.run([str(cli),'convert-image',str(path),str(denied_output)],capture_output=True,text=True)
        assert denied.returncode != 0 and not denied_output.exists()
        rejected.append(name)
    assert source.read_bytes() == content
    evidence = root / 'Artifacts/Verification/portable-images.json'
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text(json.dumps({'platform':os.name,'cliAndWorkerMatch':True,'exactRGBAPixels':True,'allEightOrientationsVerified':True,'iccProfileChecks':color_checks,'gammaAndPrecedenceChecks':True,'originalUnchanged':True,'rejected':rejected,'verifiedPNGExport':True,'jpegBackgroundAndContainerChecks':True,'boundedNativePreview':True,'scope':'PNG inspection and normalized PNG export; other image workflows pending'}, indent=2) + '\n')
print('Native PNG inspection and independent pixel checks passed.')
