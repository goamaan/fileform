#!/bin/bash
set -euo pipefail
# Embed the complete engine runtime into a built archive before Organizer export.
FILEFORM_APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FILEFORM_ARCHIVE="${1:?Pass a completed Fileform.xcarchive}"
: "${FILEFORM_SIGNING_IDENTITY:?Set the Developer ID Application identity}"
case "$FILEFORM_SIGNING_IDENTITY" in
    'Developer ID Application:'*) ;;
    *) echo 'Developer ID Application identity required.' >&2; exit 2 ;;
esac
FILEFORM_ARCHIVED_APP="$FILEFORM_ARCHIVE/Products/Applications/Fileform.app"
FILEFORM_RUNTIME="$FILEFORM_APP_ROOT/Artifacts/Release/Fileform.app"
test -x "$FILEFORM_ARCHIVED_APP/Contents/MacOS/Fileform"
/usr/libexec/PlistBuddy -c 'Add :FileformDistribution string developer-id' "$FILEFORM_ARCHIVED_APP/Contents/Info.plist" 2>/dev/null || \
    /usr/libexec/PlistBuddy -c 'Set :FileformDistribution developer-id' "$FILEFORM_ARCHIVED_APP/Contents/Info.plist"
python3 "$FILEFORM_APP_ROOT/Tools/verify-bundle.py" "$FILEFORM_RUNTIME"
mkdir -p "$FILEFORM_ARCHIVED_APP/Contents/Helpers"
cp "$FILEFORM_RUNTIME/Contents/Helpers/fileform-worker" "$FILEFORM_ARCHIVED_APP/Contents/Helpers/fileform-worker"
for resource in MediaPack PDFPack Notices; do
    ditto "$FILEFORM_RUNTIME/Contents/Resources/$resource" "$FILEFORM_ARCHIVED_APP/Contents/Resources/$resource"
done
# Current engines are arm64; avoid delivering a misleading universal executable.
FILEFORM_APP_ARCHS="$(lipo -archs "$FILEFORM_ARCHIVED_APP/Contents/MacOS/Fileform")"
if [ "$FILEFORM_APP_ARCHS" != arm64 ]; then
    lipo "$FILEFORM_ARCHIVED_APP/Contents/MacOS/Fileform" -thin arm64 -output "$FILEFORM_ARCHIVED_APP/Contents/MacOS/Fileform.arm64"
    mv "$FILEFORM_ARCHIVED_APP/Contents/MacOS/Fileform.arm64" "$FILEFORM_ARCHIVED_APP/Contents/MacOS/Fileform"
fi
python3 - "$FILEFORM_ARCHIVE/Info.plist" <<'PY'
import plistlib,sys
from pathlib import Path
p=Path(sys.argv[1]); d=plistlib.loads(p.read_bytes())
d['ApplicationProperties']['Architectures']=['arm64']
p.write_bytes(plistlib.dumps(d))
PY
for relative in Contents/Resources/MediaPack/bin/ffmpeg Contents/Resources/MediaPack/bin/ffprobe Contents/Resources/PDFPack/bin/qpdf Contents/Helpers/fileform-worker; do
    lipo "$FILEFORM_ARCHIVED_APP/$relative" -verify_arch arm64
    codesign --force --timestamp --options runtime --sign "$FILEFORM_SIGNING_IDENTITY" \
        --entitlements "$FILEFORM_APP_ROOT/Configuration/Engine.entitlements" "$FILEFORM_ARCHIVED_APP/$relative"
done
python3 - "$FILEFORM_ARCHIVED_APP" <<'PY'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1])/'Contents/Resources'
for pack,names in [('MediaPack',['ffmpeg','ffprobe']),('PDFPack',['qpdf'])]:
 p=root/pack; f=p/'manifest.json'; d=json.loads(f.read_text())
 for name in names:d['executables'][name]=hashlib.sha256((p/'bin'/name).read_bytes()).hexdigest()
 f.write_text(json.dumps(d,indent=2)+'\n')
PY
codesign --force --timestamp --options runtime --sign "$FILEFORM_SIGNING_IDENTITY" \
    --entitlements "$FILEFORM_APP_ROOT/Configuration/Fileform.entitlements" "$FILEFORM_ARCHIVED_APP"
python3 "$FILEFORM_APP_ROOT/Tools/verify-bundle.py" "$FILEFORM_ARCHIVED_APP" --developer-id --report "$FILEFORM_ARCHIVE/bundle-audit.json"
echo 'Archive runtime prepared for Organizer. Recheck exported engine hashes before customer delivery.'
