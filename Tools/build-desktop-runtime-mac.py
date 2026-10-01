#!/usr/bin/env python3
"""Build the complete development desktop runtime on a native Mac CI host."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import tarfile
import urllib.request
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--destination',type=Path,required=True);args=parser.parse_args()
if sys.platform!='darwin':raise RuntimeError('Build Mac tools on macOS')
root=Path(__file__).resolve().parents[1]
def run(args):subprocess.run([str(v) for v in args],cwd=root,check=True)
run(['bash','Tools/build-media-pack.sh']);run(['bash','Tools/build-pdf-pack.sh'])
run([sys.executable,'Tools/build-ocr-evaluation.py','--work','Artifacts/desktop-ocr','--jobs',4])
run([sys.executable,'Tools/build-heic-evaluation.py','--work','Artifacts/desktop-heic','--jobs',4])
prefix=(root/'Artifacts/desktop-heic/install').resolve()
run(['cmake','-S','native/heic-decode','-B','Artifacts/desktop-heic-helper','-G','Ninja','-DCMAKE_BUILD_TYPE=Release','-DCMAKE_OSX_DEPLOYMENT_TARGET=13.0','-DHEIC_ROOT='+str(prefix)])
run(['cmake','--build','Artifacts/desktop-heic-helper','--parallel',4])
run([sys.executable,'native/heic-decode/stage-evaluation.py','Artifacts/desktop-heic-helper','Artifacts/desktop-heic','Artifacts/desktop-HEICPack'])
# Pinned, attested non-V8/non-XFA evaluation renderer; its production-source gate
# remains explicit. Architecture must match the worker and desktop host.
import platform
arch='arm64' if platform.machine().lower()=='arm64' else 'x64'
if arch!='arm64':raise RuntimeError('This existing evaluation archive is pinned for Mac arm64; add reviewed Intel provenance before packaging Intel.')
work=root/'Artifacts/desktop-pdfium';work.mkdir()
archive=work/'pdfium-mac-arm64.tgz'
with urllib.request.urlopen('https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8066/pdfium-mac-arm64.tgz',timeout=60) as response,archive.open('wb') as output:
 count=0
 for chunk in iter(lambda:response.read(65536),b''):
  count+=len(chunk)
  if count>128*1024*1024:raise RuntimeError('Renderer archive exceeds its bound')
  output.write(chunk)
# Fetch the known pin from the tracked research record rather than use a hash
# supplied by the downloaded archive itself.
import re
note=(root/'Documentation/PDFIUM_RESEARCH.md').read_text()
hashes=re.findall(r'\b[0-9a-f]{64}\b',note)
actual=hashlib.sha256(archive.read_bytes()).hexdigest()
if actual not in hashes:raise RuntimeError('Renderer source archive does not match the reviewed pin')
run(['gh','attestation','verify',archive,'--repo','bblanchon/pdfium-binaries'])
with tarfile.open(archive) as source:
 for member in source.getmembers():
  path=Path(member.name)
  if path.is_absolute() or '..' in path.parts or chr(92) in member.name or ':' in member.name or not(member.isfile() or member.isdir()):raise RuntimeError('Unsafe renderer archive path')
 source.extractall(work)
run(['cmake','-S','native/pdf-render','-B','Artifacts/desktop-pdf-render','-G','Ninja','-DCMAKE_BUILD_TYPE=Release','-DCMAKE_OSX_DEPLOYMENT_TARGET=13.0','-DPDFIUM_ROOT='+str(work)])
run(['cmake','--build','Artifacts/desktop-pdf-render','--parallel',4])
run([sys.executable,'native/pdf-render/stage-evaluation.py','Artifacts/desktop-pdf-render',work,'Artifacts/desktop-PDFRenderPack'])
run([sys.executable,'Tools/stage-desktop-runtime.py','--worker','target/release/fileform-worker','--cli','target/release/fileform-native','--media','Artifacts/MediaPack','--pdf','Artifacts/PDFPack','--renderer','Artifacts/desktop-PDFRenderPack','--ocr','Artifacts/desktop-ocr/OCRPack','--heic','Artifacts/desktop-HEICPack','--destination',args.destination])
