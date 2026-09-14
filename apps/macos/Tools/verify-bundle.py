#!/usr/bin/env python3
"""Audit an installed bundle; never rebuild or use development engine paths."""
import argparse, hashlib, json, subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('app', type=Path)
parser.add_argument('--report', type=Path)
parser.add_argument('--developer-id', action='store_true')
args = parser.parse_args()
app = args.app.resolve()
required = ['Contents/MacOS/Fileform', 'Contents/Helpers/fileform-worker',
            'Contents/Resources/MediaPack/bin/ffmpeg', 'Contents/Resources/MediaPack/bin/ffprobe',
            'Contents/Resources/PDFPack/bin/qpdf', 'Contents/Resources/Notices/FileformCore-LICENSE.txt',
            'Contents/Resources/Fonts/JetBrainsMono-Regular.ttf']
for name in required:
    assert (app/name).is_file(), f'Missing bundled requirement: {name}'
subprocess.run(['codesign', '--verify', '--deep', '--strict', str(app)], check=True, capture_output=True)
signature = subprocess.run(['codesign', '-d', '--verbose=4', str(app)], capture_output=True, text=True).stderr
if args.developer_id:
    assert 'Authority=Developer ID Application:' in signature, 'Developer ID signature is required'
for pack, names in [('MediaPack', ['ffmpeg', 'ffprobe']), ('PDFPack', ['qpdf'])]:
    base = app/'Contents/Resources'/pack
    manifest = json.loads((base/'manifest.json').read_text())
    for name in names:
        assert hashlib.sha256((base/'bin'/name).read_bytes()).hexdigest() == manifest['executables'][name], f'{pack}/{name} digest mismatch'

magic = {b'\xcf\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca', b'\xca\xfe\xba\xbf', b'\xbf\xba\xfe\xca'}
images = []
for file in app.rglob('*'):
    if file.is_symlink():
        assert file.resolve().is_relative_to(app), f'External bundle symlink: {file.relative_to(app)}'
    if not file.is_file(): continue
    with file.open('rb') as stream:
        if stream.read(4) not in magic: continue
    if args.developer_id:
        nested = subprocess.run(['codesign', '-d', '--verbose=4', str(file)], capture_output=True, text=True, check=True).stderr
        assert 'Authority=Developer ID Application:' in nested, f'Missing Developer ID: {file.relative_to(app)}'
        team = next(line for line in signature.splitlines() if line.startswith('TeamIdentifier='))
        assert team in nested.splitlines(), f'Unexpected signing team: {file.relative_to(app)}'
        assert 'runtime' in nested and 'Timestamp=' in nested, f'Missing hardened runtime or timestamp: {file.relative_to(app)}'
    linked = subprocess.check_output(['otool', '-L', str(file)], text=True)
    libraries = [line.strip().split(' (', 1)[0] for line in linked.splitlines()[1:] if line.startswith('\t')]
    for library in libraries:
        assert library.startswith(('/System/Library/', '/usr/lib/', '@rpath/', '@loader_path/', '@executable_path/')), f'External dependency: {file.relative_to(app)} -> {library}'
    images.append({'file': str(file.relative_to(app)), 'libraries': libraries})
assert len(images) >= 5, 'Expected app, worker and three bundled engine executables'
report = {'bundle': str(app), 'signatureVerified': True, 'developerID': 'Authority=Developer ID Application:' in signature,
          'requiredFilesPresent': True, 'packDigestsVerified': True, 'externalAbsoluteDependencies': [], 'machOImages': images}
if args.report:
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2)+'\n')
print(f'Bundle verified: {len(images)} Mach-O images; required engines, fonts and notices present.')
