#!/bin/bash
set -euo pipefail
# Produces a verified, immutable delivery; never publishes it.
FILEFORM_APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FILEFORM_INPUT="${1:-$FILEFORM_APP_ROOT/Artifacts/Release/Fileform.app}"
: "${FILEFORM_SIGNING_IDENTITY:?Set FILEFORM_SIGNING_IDENTITY to your Developer ID Application identity.}"
: "${FILEFORM_NOTARY_PROFILE:?Set FILEFORM_NOTARY_PROFILE to an existing notarytool keychain profile.}"
case "$FILEFORM_SIGNING_IDENTITY" in
    'Developer ID Application:'*) ;;
    *) echo 'A Developer ID Application identity is required.' >&2; exit 2 ;;
esac
python3 "$FILEFORM_APP_ROOT/Tools/verify-bundle.py" "$FILEFORM_INPUT"
security find-identity -v -p codesigning | grep -Fq "$FILEFORM_SIGNING_IDENTITY" || { echo 'The requested signing identity is not in the keychain.' >&2; exit 1; }
mkdir -p "$FILEFORM_APP_ROOT/Artifacts/notarized"
FILEFORM_STAGE="$(mktemp -d "$FILEFORM_APP_ROOT/Artifacts/notarized/Fileform-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX")"
# Keep submission and Apple diagnostics on failure; never replace old deliveries.
trap 'FILEFORM_EXIT=$?; if [ "$FILEFORM_EXIT" -ne 0 ]; then printf "Release preparation failed (%s). Evidence: %s\n" "$FILEFORM_EXIT" "$FILEFORM_STAGE" >&2; fi' EXIT
exec > >(tee "$FILEFORM_STAGE/release.log") 2>&1
ditto "$FILEFORM_INPUT" "$FILEFORM_STAGE/Fileform.app"
FILEFORM_SIGNED_APP="$FILEFORM_STAGE/Fileform.app"
/usr/libexec/PlistBuddy -c 'Add :FileformDistribution string developer-id' "$FILEFORM_SIGNED_APP/Contents/Info.plist" 2>/dev/null || \
    /usr/libexec/PlistBuddy -c 'Set :FileformDistribution developer-id' "$FILEFORM_SIGNED_APP/Contents/Info.plist"
for relative in Contents/Resources/MediaPack/bin/ffmpeg Contents/Resources/MediaPack/bin/ffprobe Contents/Resources/PDFPack/bin/qpdf Contents/Helpers/fileform-worker; do
    codesign --force --timestamp --options runtime --sign "$FILEFORM_SIGNING_IDENTITY" \
        --entitlements "$FILEFORM_APP_ROOT/Configuration/Engine.entitlements" "$FILEFORM_SIGNED_APP/$relative"
done
python3 - "$FILEFORM_SIGNED_APP" <<'PY'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1])/'Contents/Resources'
for pack,names in [('MediaPack',['ffmpeg','ffprobe']),('PDFPack',['qpdf'])]:
 p=root/pack; f=p/'manifest.json'; d=json.loads(f.read_text())
 for name in names:d['executables'][name]=hashlib.sha256((p/'bin'/name).read_bytes()).hexdigest()
 f.write_text(json.dumps(d,indent=2)+'\n')
PY
codesign --force --timestamp --options runtime --sign "$FILEFORM_SIGNING_IDENTITY" \
    --entitlements "$FILEFORM_APP_ROOT/Configuration/Fileform.entitlements" "$FILEFORM_SIGNED_APP"
python3 "$FILEFORM_APP_ROOT/Tools/verify-bundle.py" "$FILEFORM_SIGNED_APP" --developer-id --report "$FILEFORM_STAGE/bundle.json"
ditto -c -k --keepParent "$FILEFORM_SIGNED_APP" "$FILEFORM_STAGE/submission.zip"
xcrun notarytool submit "$FILEFORM_STAGE/submission.zip" --keychain-profile "$FILEFORM_NOTARY_PROFILE" --wait --output-format json > "$FILEFORM_STAGE/notarization.json"
python3 - "$FILEFORM_STAGE/notarization.json" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]))
if d.get('status')!='Accepted':raise SystemExit('Apple did not accept this submission. Inspect notarization.json; no delivery was produced.')
PY
xcrun stapler staple "$FILEFORM_SIGNED_APP"
xcrun stapler validate "$FILEFORM_SIGNED_APP"
spctl --assess --type execute --verbose "$FILEFORM_SIGNED_APP"
ditto -c -k --keepParent "$FILEFORM_SIGNED_APP" "$FILEFORM_STAGE/Fileform.zip"
mkdir "$FILEFORM_STAGE/unpacked"
ditto -x -k "$FILEFORM_STAGE/Fileform.zip" "$FILEFORM_STAGE/unpacked"
python3 "$FILEFORM_APP_ROOT/Tools/verify-bundle.py" "$FILEFORM_STAGE/unpacked/Fileform.app" --developer-id --report "$FILEFORM_STAGE/delivery-bundle.json"
xcrun stapler validate "$FILEFORM_STAGE/unpacked/Fileform.app"
spctl --assess --type execute --verbose "$FILEFORM_STAGE/unpacked/Fileform.app"
(cd "$FILEFORM_STAGE" && shasum -a 256 Fileform.zip > SHA256SUMS)
printf 'Verified notarized ZIP: %s/Fileform.zip\n' "$FILEFORM_STAGE"
