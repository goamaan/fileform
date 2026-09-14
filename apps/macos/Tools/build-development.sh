#!/bin/bash
set -euo pipefail
FILEFORM_APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FILEFORM_CORE_ROOT="$(cd "$FILEFORM_APP_ROOT/../.." && pwd)"
FILEFORM_BUILD_CONFIGURATION="${FILEFORM_BUILD_CONFIGURATION:-Debug}"
case "$FILEFORM_BUILD_CONFIGURATION" in
    Debug) FILEFORM_SWIFT_CONFIGURATION=debug; FILEFORM_OUTPUT="$FILEFORM_APP_ROOT/Artifacts/Fileform.app" ;;
    Release) FILEFORM_SWIFT_CONFIGURATION=release; FILEFORM_OUTPUT="$FILEFORM_APP_ROOT/Artifacts/Release/Fileform.app" ;;
    *) echo 'Build configuration must be Debug or Release.' >&2; exit 2 ;;
esac
cd "$FILEFORM_APP_ROOT"
mkdir -p Artifacts
command -v xcodegen >/dev/null || { echo 'Install XcodeGen 2.46.0 to generate the project.' >&2; exit 1; }
xcodegen generate
xcodebuild -project Fileform.xcodeproj -scheme Fileform -configuration "$FILEFORM_BUILD_CONFIGURATION" \
    -derivedDataPath Artifacts/DerivedData build CODE_SIGN_IDENTITY=- CODE_SIGNING_ALLOWED=YES \
    > Artifacts/build.log 2>&1 || { tail -n 70 Artifacts/build.log; exit 1; }
mkdir -p "$(dirname "$FILEFORM_OUTPUT")"
if [ -d "$FILEFORM_OUTPUT" ]; then rm -rf "$FILEFORM_OUTPUT"; fi
ditto "Artifacts/DerivedData/Build/Products/$FILEFORM_BUILD_CONFIGURATION/Fileform.app" "$FILEFORM_OUTPUT"
if [ -f "$FILEFORM_CORE_ROOT/Artifacts/MediaPack/manifest.json" ]; then
    ditto "$FILEFORM_CORE_ROOT/Artifacts/MediaPack" "$FILEFORM_OUTPUT/Contents/Resources/MediaPack"
    for executable in ffmpeg ffprobe; do
        codesign --force --sign - --options runtime --entitlements Configuration/Engine.entitlements \
            "$FILEFORM_OUTPUT/Contents/Resources/MediaPack/bin/$executable"
    done
    FILEFORM_BUNDLED_PACK="$FILEFORM_OUTPUT/Contents/Resources/MediaPack" python3 - <<'PY'
import hashlib,json,os
from pathlib import Path
p=Path(os.environ['FILEFORM_BUNDLED_PACK']);f=p/'manifest.json';d=json.loads(f.read_text())
for name in ['ffmpeg','ffprobe']:d['executables'][name]=hashlib.sha256((p/'bin'/name).read_bytes()).hexdigest()
f.write_text(json.dumps(d,indent=2)+'\n')
PY
fi
if [ -f "$FILEFORM_CORE_ROOT/Artifacts/PDFPack/manifest.json" ]; then
    ditto "$FILEFORM_CORE_ROOT/Artifacts/PDFPack" "$FILEFORM_OUTPUT/Contents/Resources/PDFPack"
    codesign --force --sign - --options runtime --entitlements Configuration/Engine.entitlements \
        "$FILEFORM_OUTPUT/Contents/Resources/PDFPack/bin/qpdf"
    FILEFORM_BUNDLED_PDF_PACK="$FILEFORM_OUTPUT/Contents/Resources/PDFPack" python3 - <<'PY'
import hashlib,json,os
from pathlib import Path
p=Path(os.environ['FILEFORM_BUNDLED_PDF_PACK']);f=p/'manifest.json';d=json.loads(f.read_text())
d['executables']['qpdf']=hashlib.sha256((p/'bin/qpdf').read_bytes()).hexdigest()
f.write_text(json.dumps(d,indent=2)+'\n')
PY
fi
swift build --package-path "$FILEFORM_CORE_ROOT" -c "$FILEFORM_SWIFT_CONFIGURATION" --product fileform-worker >/dev/null
mkdir -p "$FILEFORM_OUTPUT/Contents/Helpers"
cp "$FILEFORM_CORE_ROOT/.build/$FILEFORM_SWIFT_CONFIGURATION/fileform-worker" "$FILEFORM_OUTPUT/Contents/Helpers/fileform-worker"
codesign --force --sign - --options runtime --entitlements Configuration/Engine.entitlements \
    "$FILEFORM_OUTPUT/Contents/Helpers/fileform-worker"
mkdir -p "$FILEFORM_OUTPUT/Contents/Resources/Notices"
if [ -d "$FILEFORM_OUTPUT/Contents/Resources/MediaPack/licenses" ]; then
    ditto "$FILEFORM_OUTPUT/Contents/Resources/MediaPack/licenses" "$FILEFORM_OUTPUT/Contents/Resources/Notices/MediaPack"
fi
if [ -d "$FILEFORM_OUTPUT/Contents/Resources/PDFPack/licenses" ]; then
    ditto "$FILEFORM_OUTPUT/Contents/Resources/PDFPack/licenses" "$FILEFORM_OUTPUT/Contents/Resources/Notices/PDFPack"
fi
cp "$FILEFORM_CORE_ROOT/LICENSE" "$FILEFORM_OUTPUT/Contents/Resources/Notices/FileformCore-LICENSE.txt"
cp "$FILEFORM_CORE_ROOT/NOTICE" "$FILEFORM_OUTPUT/Contents/Resources/Notices/FileformCore-NOTICE.txt"
codesign --force --sign - --options runtime --entitlements Configuration/Fileform.entitlements "$FILEFORM_OUTPUT"
codesign --verify --deep --strict "$FILEFORM_OUTPUT"
echo "$FILEFORM_BUILD_CONFIGURATION app (ad-hoc signed, not notarized): $FILEFORM_OUTPUT"
