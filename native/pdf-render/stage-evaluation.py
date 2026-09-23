#!/usr/bin/env python3
"""Stage a local evaluation pack from a verified PDFium archive and built helper.

This does not authenticate an archive, sign a release or approve redistribution.
"""
import hashlib
import json
from pathlib import Path
import platform
import shutil
import sys

build, archive, destination = [Path(v).resolve() for v in sys.argv[1:]]
flags = (archive / 'args.gn').read_text()
assert 'pdf_enable_v8 = false' in flags and 'pdf_enable_xfa = false' in flags
assert not destination.exists(), 'Choose a new evaluation pack directory'
windows = sys.platform == 'win32'
helper = 'fileform-pdf-render' + ('.exe' if windows else '')
library = 'pdfium.dll' if windows else 'libpdfium.dylib'
destination.mkdir(parents=True)
(destination / 'bin').mkdir()
for name in [helper, library]:
    shutil.copy2(build / name, destination / 'bin' / name)
shutil.copy2(archive / 'LICENSE', destination / 'LICENSE')
shutil.copytree(archive / 'licenses', destination / 'licenses')
for name in ['args.gn', 'VERSION']:
    shutil.copy2(archive / name, destination / name)
def digest(name):
    return hashlib.sha256((destination / 'bin' / name).read_bytes()).hexdigest()
(destination / 'manifest.json').write_text(json.dumps({
    'schemaVersion': 1, 'id': 'app.fileform.pdf-render',
    'version': 'pdfium-8066-helper-2-evaluation',
    'architecture': platform.machine().lower(),
    'executables': {'fileform-pdf-render': digest(helper)},
    'libraries': {library: digest(library)},
}, indent=2) + '\n')
