#!/bin/bash
set -euo pipefail
# Run interactively. notarytool hides password entry and stores it in Keychain.
# No password is written to the repository, shell history or command arguments.
printf 'Fileform notarization setup\nUse the Apple Account for your Developer ID certificate.\n\n'
read -r -p 'Apple Account email: ' FILEFORM_ACCOUNT_EMAIL
if [ -z "$FILEFORM_ACCOUNT_EMAIL" ]; then echo 'An Apple Account email is required.' >&2; exit 1; fi
FILEFORM_DEVELOPER_TEAM="${FILEFORM_DEVELOPER_TEAM:-}"
if [ -z "$FILEFORM_DEVELOPER_TEAM" ]; then read -r -p 'Apple Developer Team ID: ' FILEFORM_DEVELOPER_TEAM; fi
if [ -z "$FILEFORM_DEVELOPER_TEAM" ]; then echo 'A Developer Team ID is required.' >&2; exit 1; fi
xcrun notarytool store-credentials Fileform \
    --apple-id "$FILEFORM_ACCOUNT_EMAIL" --team-id "$FILEFORM_DEVELOPER_TEAM"
printf '\nKeychain profile Fileform is ready. You can close this window.\n'
