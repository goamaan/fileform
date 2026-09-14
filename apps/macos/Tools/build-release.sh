#!/bin/bash
set -euo pipefail
FILEFORM_APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Optimized app and worker with the same bundled-engine/notices assembly.
# This is a release candidate, not customer delivery until notarization passes.
FILEFORM_BUILD_CONFIGURATION=Release "$FILEFORM_APP_ROOT/Tools/build-development.sh"
python3 "$FILEFORM_APP_ROOT/Tools/verify-bundle.py" "$FILEFORM_APP_ROOT/Artifacts/Release/Fileform.app" \
    --report "$FILEFORM_APP_ROOT/Artifacts/Release/bundle.json"
