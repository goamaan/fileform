#!/bin/bash
set -euo pipefail
FILEFORM_APP_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FILEFORM_CORE_ROOT="$FILEFORM_APP_ROOT/../.."
FILEFORM_PROBE="$FILEFORM_APP_ROOT/Artifacts/WorkerSandboxProbe.app"
swift build --package-path "$FILEFORM_CORE_ROOT" --product fileform-worker >/dev/null
mkdir -p "$FILEFORM_PROBE/Contents/MacOS" "$FILEFORM_PROBE/Contents/Helpers"
swiftc -parse-as-library -swift-version 6 -target "$(uname -m)-apple-macos14.0" \
  -I "$FILEFORM_CORE_ROOT/.build/debug/Modules" \
  "$FILEFORM_APP_ROOT/Tools/WorkerSandboxProbe/Probe.swift" \
  "$FILEFORM_CORE_ROOT"/.build/debug/FileformCore.build/*.swift.o \
  "$FILEFORM_CORE_ROOT"/.build/debug/FileformDomain.build/*.swift.o \
  -o "$FILEFORM_PROBE/Contents/MacOS/WorkerSandboxProbe"
cat > "$FILEFORM_PROBE/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>app.fileform.WorkerSandboxProbe</string>
<key>CFBundleName</key><string>WorkerSandboxProbe</string>
<key>CFBundleExecutable</key><string>WorkerSandboxProbe</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
</dict></plist>
PLIST
cp "$FILEFORM_CORE_ROOT/.build/debug/fileform-worker" "$FILEFORM_PROBE/Contents/Helpers/fileform-worker"
codesign --force --sign - --options runtime --entitlements "$FILEFORM_APP_ROOT/Configuration/Engine.entitlements" "$FILEFORM_PROBE/Contents/Helpers/fileform-worker"
codesign --force --sign - --options runtime --entitlements "$FILEFORM_APP_ROOT/Configuration/Fileform.entitlements" "$FILEFORM_PROBE"
codesign --verify --deep --strict "$FILEFORM_PROBE"
echo "$FILEFORM_PROBE"
