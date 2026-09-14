#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Source-owned continuous-tone image PDF, text, vector Forms, boxes and metadata.

The image is a deterministic synthetic landscape with photographic-like texture,
not a third-party photo. Uses only the Python standard library.
"""
import math
from pathlib import Path
import sys
import zlib
from importlib.util import spec_from_file_location, module_from_spec

_spec = spec_from_file_location('image_fixtures', Path(__file__).with_name('generate-pdf-image-fixtures.py'))
_module = module_from_spec(_spec); _spec.loader.exec_module(_module)
stream = _module.stream


def generate(work, width=960):
    work.mkdir(parents=True, exist_ok=True)
    height = width * 2 // 3
    samples = bytearray()
    seed = 123456789
    for y in range(height):
        for x in range(width):
            seed = (1664525 * seed + 1013904223) & 0xffffffff
            grain = ((seed >> 24) - 128) * 0.13
            horizon = height * (0.43 + 0.12 * math.sin(x / width * 8))
            if y < horizon:
                t = y / height
                color = (75 + 100*t, 125 + 90*t, 220 - 30*t)
            else:
                t = y / height
                texture = 25 * math.sin(x * .09 + y * .035) * math.cos(y * .12)
                color = (42 + 85*t + texture, 85 + 60*t + texture, 50 + 30*t + texture)
            samples += bytes(max(0, min(255, round(v + grain))) for v in color)
    objects = [b'<< /Type /Catalog /Pages 2 0 R /Metadata 11 0 R >>',
        b'<< /Type /Pages /Kids [4 0 R 6 0 R 8 0 R] /Count 3 /Resources 14 0 R >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    for index in range(3):
        objects.append((f'<< /Type /Page /Parent 2 0 R /MediaBox [10 20 610 820] /CropBox [15 25 605 815] '
            f'/BleedBox [17 27 603 813] /TrimBox [20 30 600 810] /ArtBox [25 35 595 805] '
            f'/Rotate {index * 90} /Contents {5 + index * 2} 0 R >>').encode())
        content = f'BT /F1 20 Tf 40 770 Td (Fileform landscape page {index+1}) Tj ET\n'
        content += 'q 520 0 0 350 40 370 cm /Photo Do Q\nq 80 0 0 80 440 260 cm /Gray Do Q\n/Form Do\n'
        content += ''.join(f'q 0.1 0.3 0.7 RG 40 {80 + row} m 550 {80 + row} l S Q\n' for row in range(100))
        objects.append(stream('<<', content.encode()))
    objects.append(stream(f'<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /BitsPerComponent 8 /ColorSpace /DeviceRGB /Filter /FlateDecode', zlib.compress(samples)))
    xmp = b'<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" /></rdf:RDF></x:xmpmeta>'
    objects.append(stream('<< /Type /Metadata /Subtype /XML', xmp))
    objects.append(b'<< /Title (Fileform image optimization fixture) /Author (Fileform source-owned fixture) /CustomValue (Retain this metadata) >>')
    objects.append(stream('<< /Type /XObject /Subtype /Form /BBox [0 0 600 800] /Resources << /Font << /F1 3 0 R >> >>',
        b'BT /F1 14 Tf 40 300 Td (Vector Form text retained) Tj ET\nq 0.9 0.2 0.1 rg 40 250 300 20 re f Q'))
    objects.append(b'<< /Font << /F1 3 0 R >> /XObject << /Photo 10 0 R /Form 13 0 R /Gray 15 0 R /Unsupported 16 0 R >> >>')
    objects.append(stream('<< /Type /XObject /Subtype /Image /Width 64 /Height 64 /BitsPerComponent 8 /ColorSpace /DeviceGray', bytes((x*4+y*3)%256 for y in range(64) for x in range(64))))
    objects.append(stream('<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 8 /ColorSpace /DeviceCMYK', bytes([0, 100, 200, 0]*4)))
    # Own writer adds scalar Info metadata to the trailer.
    data = bytearray(b'%PDF-1.7\n%\xe2\xe3\xcf\xd3\n'); offsets = [0]
    for index, obj in enumerate(objects, 1):
        offsets.append(len(data)); data += f'{index} 0 obj\n'.encode() + obj + b'\nendobj\n'
    xref = len(data); data += f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
    for offset in offsets[1:]: data += f'{offset:010} 00000 n \n'.encode()
    data += f'trailer << /Size {len(offsets)} /Root 1 0 R /Info 12 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
    (work/'landscape.pdf').write_bytes(data)
    (work/'landscape-rgb.bin').write_bytes(samples)
    (work/'fixture.json').write_text(__import__('json').dumps({'width': width, 'height': height, 'source': 'deterministic synthetic landscape, source-owned'}, indent=2)+'\n')


if __name__ == '__main__': generate(Path(sys.argv[1]), int(sys.argv[2]) if len(sys.argv) > 2 else 960)
