#!/usr/bin/env python3
"""A controlled English raster baseline, not OCR/language parity acceptance."""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
pack=Path(sys.argv[1]).resolve()
manifest=json.loads((pack/'manifest.json').read_text(encoding='utf-8'))
assert manifest['evaluationOnly'] and manifest['networkProtocols'] is False
suffix='.exe' if sys.platform=='win32' else ''
binary=pack/'bin'/('tesseract'+suffix)
assert hashlib.sha256(binary.read_bytes()).hexdigest()==manifest['executables']['tesseract']
for relative,expected in manifest['assets'].items():
    assert hashlib.sha256((pack/relative).read_bytes()).hexdigest()==expected
raw=gzip.decompress((Path(__file__).parent/'fixtures/ocr-page.ppm.gz').read_bytes())
assert hashlib.sha256(raw).hexdigest()=='5c2a05c38d6e400212f9355bd8d6ec502f5f8769f51a53353393e1bbcfbc5030'
def recognize(path):
    return subprocess.run([str(binary),str(path),'stdout','--tessdata-dir',str(pack/'tessdata'),'-l','eng','--oem','1','--psm','3'],capture_output=True,timeout=60)
with tempfile.TemporaryDirectory(prefix='fileform-ocr-') as folder:
    base=Path(folder)
    source=base/'page.ppm';source.write_bytes(raw)
    result=recognize(source)
    assert result.returncode==0,result.stderr
    assert result.stdout.decode('utf-8').strip()=='Fileform page 1',result.stdout
    assert b'function not present' not in result.stderr and b'font pixa not made' not in result.stderr,result.stderr
    assert source.read_bytes()==raw
    blank=base/'blank.ppm';blank.write_bytes(b'P6\n128 128\n255\n'+b'\xff'*(128*128*3))
    result=recognize(blank)
    assert result.returncode==0 and not result.stdout.strip(),(result.stdout,result.stderr)
    bad=base/'bad.ppm';bad.write_bytes(b'P6\n128 128\n255\n')
    assert recognize(bad).returncode!=0
print('OCR evaluation: pinned pack/model hashes, real English raster recognition, blank input, malformed input and unchanged source passed; multilingual/engine acceptance remains open')
