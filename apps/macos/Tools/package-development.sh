#!/bin/bash
set -euo pipefail
FILEFORM_APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$FILEFORM_APP_ROOT"
Tools/build-development.sh
mkdir -p Artifacts/releases
FILEFORM_STAGE="$(mktemp -d /tmp/fileform-dmg-stage.XXXXXX)"
FILEFORM_MOUNT="$(mktemp -d /tmp/fileform-dmg-check.XXXXXX)"
FILEFORM_ARTIFACT_STAGE="$(mktemp -d "$FILEFORM_APP_ROOT/Artifacts/releases/.staging.XXXXXX")"
FILEFORM_VERSION="$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' Artifacts/Fileform.app/Contents/Info.plist)"
FILEFORM_NAME="Fileform-$FILEFORM_VERSION-development-$(uname -m)"
FILEFORM_IMAGE="$FILEFORM_ARTIFACT_STAGE/$FILEFORM_NAME.dmg"
detach_owned_image() {
    FILEFORM_OWNED_IMAGE="$FILEFORM_IMAGE" python3 - <<'PY'
import os,plistlib,subprocess
p=subprocess.run(['hdiutil','info','-plist'],capture_output=True,check=True)
for image in plistlib.loads(p.stdout).get('images',[]):
 if image.get('image-path')==os.environ['FILEFORM_OWNED_IMAGE']:
  devices=[e['dev-entry'] for e in image.get('system-entities',[]) if e.get('dev-entry')]
  if devices:subprocess.run(['hdiutil','detach',devices[0]],check=True,stdout=subprocess.DEVNULL)
PY
}
cleanup() {
    if detach_owned_image; then rm -rf "$FILEFORM_STAGE" "$FILEFORM_MOUNT" "$FILEFORM_ARTIFACT_STAGE"
    else echo "Could not detach the task's test image; retained staging at $FILEFORM_ARTIFACT_STAGE" >&2; fi
}
trap cleanup EXIT
ditto Artifacts/Fileform.app "$FILEFORM_STAGE/Fileform.app"
ln -s /Applications "$FILEFORM_STAGE/Applications"
cat > "$FILEFORM_STAGE/Read Me.txt" <<'TEXT'
Fileform — development preview

This is an ad-hoc signed development build for evaluation, not notarized customer distribution.
The app creates new results and retains your originals. Review conversions and recognized text for your intended use.
Fileform is free and open source under Apache-2.0. Open-source engine and dependency licenses are inside Fileform.app/Contents/Resources.
Get releases, documentation and support from the public Fileform repository.
TEXT
hdiutil create -format UDZO -volname 'Fileform Development' -srcfolder "$FILEFORM_STAGE" "$FILEFORM_IMAGE" >/dev/null
ditto -c -k --keepParent Artifacts/Fileform.app "$FILEFORM_ARTIFACT_STAGE/$FILEFORM_NAME.zip"
# A failed attach can still create a device. Clean up only this unique staged
# image before one bounded retry; never detach unrelated mounted images.
if ! hdiutil attach -nobrowse -readonly -mountpoint "$FILEFORM_MOUNT" "$FILEFORM_IMAGE" >/dev/null; then
    detach_owned_image
    hdiutil attach -nobrowse -readonly -mountpoint "$FILEFORM_MOUNT" "$FILEFORM_IMAGE" >/dev/null
fi
test -x "$FILEFORM_MOUNT/Fileform.app/Contents/MacOS/Fileform"
codesign --verify --deep --strict "$FILEFORM_MOUNT/Fileform.app"
/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$FILEFORM_MOUNT/Fileform.app/Contents/Info.plist" | grep -qx 'app.fileform.Fileform'
detach_owned_image
(cd "$FILEFORM_ARTIFACT_STAGE" && shasum -a 256 "$FILEFORM_NAME.dmg" "$FILEFORM_NAME.zip" > "$FILEFORM_NAME.sha256")
FILEFORM_RELEASE_PATH="$FILEFORM_ARTIFACT_STAGE" FILEFORM_RELEASE_NAME="$FILEFORM_NAME" python3 - <<'PY'
from pathlib import Path
import hashlib,json,os,subprocess
p=Path(os.environ['FILEFORM_RELEASE_PATH']);name=os.environ['FILEFORM_RELEASE_NAME']
d={'schemaVersion':1,'product':'Fileform','distribution':'development','notarized':False,
   'appRevision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
   'coreRevision':subprocess.check_output(['git','-C','../..','rev-parse','HEAD'],text=True).strip(),
   'coreSourceDirty':bool(subprocess.check_output(['git','-C','../..','status','--porcelain'],text=True).strip()),
   'sourceDirty':bool(subprocess.check_output(['git','status','--porcelain'],text=True).strip()),
   'files':{f'{name}.{ext}':hashlib.sha256((p/f'{name}.{ext}').read_bytes()).hexdigest() for ext in ['dmg','zip']}}
(p/f'{name}.json').write_text(json.dumps(d,indent=2)+'\n')
PY
FILEFORM_DELIVERY="$FILEFORM_APP_ROOT/Artifacts/releases/$FILEFORM_NAME-$(git rev-parse --short HEAD)-${FILEFORM_ARTIFACT_STAGE##*.}"
# Publish the verified set together on the same filesystem. Failed verification
# leaves all earlier deliveries intact instead of mixing old/new receipts.
mv "$FILEFORM_ARTIFACT_STAGE" "$FILEFORM_DELIVERY"
echo "Mounted and verified development delivery: $FILEFORM_DELIVERY"
