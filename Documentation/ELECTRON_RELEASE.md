# Electron release preparation

The desktop remains a migration preview. Signing proves publisher identity and
bundle integrity; it does not establish feature parity or release acceptance.
Keep the preview application identifier until the data migration and final
release gates in CROSS_PLATFORM.md are satisfied.

## macOS

Use a Mac with the Developer ID Application certificate and its private key in
Keychain. Store notarization credentials with Apple's `notarytool store-credentials`
and use the profile name below. Never put passwords or certificate exports in the
repository or shell scripts.

From `apps/desktop`, after building the release Rust worker at the repository root:

```sh
npm ci
npm run build
npm run notices
export FILEFORM_SIGNING_IDENTITY='Your certificate name (TEAMID)'
npx electron-builder --dir --config electron-builder.signed.cjs --config.directories.output=artifacts/signed-candidate
node scripts/notarize-mac.mjs 'artifacts/signed-candidate/mac-arm64/Fileform Preview.app' Fileform
```

The signed configuration fails without a signing identity, enables hardened
runtime, signs the bundled native worker, and limits Electron entitlements to JIT.
It retains the production Electron fuses from the base configuration. Ordinary
CI preview builds continue using `electron-builder.cjs` without signing secrets.

The notarization helper verifies the Developer ID signature and hardened runtime,
submits an archive through the named Keychain profile, requires Apple's Accepted
status, staples the ticket, then checks codesign, stapler and Gatekeeper. Temporary
submission archives remain in the system temporary directory for diagnosis.
Create the downloadable archive **after** stapling, extract it into a fresh
location, verify it again and exercise real transformations in that extracted app.
Do not publish an earlier submission archive without the stapled ticket.

Outstanding release gates include Intel Mac acceptance, final feature parity,
clean installation, signed update verification and rollback/error handling,
public release metadata and durable downloadable assets.

## Windows

The existing Windows CI builds the same React/Electron renderer and Rust worker,
runs automated native checks and creates an NSIS installer. Manual Windows GUI
acceptance and trusted Windows code signing remain separate requirements. No
Windows signing identity has been configured. Do not describe CI artifacts as
signed or as the final public release.
