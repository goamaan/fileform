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


## Bundled processing foundation — October 1, 2026

The desktop builder now requires one staged complete native runtime rather than
shipping the worker alone. `Tools/stage-desktop-runtime.py` copies only declared
helper/CLI binaries, libraries, models, licenses and source archives from reviewed
packs; local build caches/logs and arbitrary folder contents are excluded. It
checks architecture/hashes and uses the current CLI to verify media/PDF/OCR/HEIC
packs and load the actual PDF renderer library with an owned fixture. The runtime
contains standalone CLI/worker plus five tool packs and rebuild source references.

The Electron main process resolves packaged tool folders locally and supplies the
HEIC path itself; no renderer-selected command or pack path is exposed. It no
longer uses a blanket 45-second kill timer for legitimate long media/PDF jobs.
Native timeouts/cooperative cancellation still apply; process-tree and hard-kill
cleanup remain release gates. Mac minimum system version now follows the actual
runtime (14.0 for these media/PDF packs).

Mac evidence: staged runtime verification passed; a real unsigned Electron bundle
contains the five packs (about 142 MB of native runtime including sources/notices).
Every pack verifies from inside that bundle, and its CLI converts a real HEIC.
Native-dialog GUI picker selected the owned Display-P3 HEIC fixture, displayed
its normalized preview and saved a verified 128x96 PNG through the packaged worker,
without a developer pack environment variable. This is actual app integration for
HEIC images, not media/PDF/OCR desktop action completion.

Windows CI imports successful same-repository main native builds only when their
relevant tool recipes match current source, retains imported-run provenance and
stages them with the current worker before packaging. Mac CI builds/stages the
full native set. These new package paths require their own CI acceptance.

The earlier worker-only signed builder is explicitly blocked for this larger
bundle: signing all nested tools changes their hashes, so manifests must be
regenerated before enclosing app signing. Do not run the old notarization recipe
and infer that the complete runtime is release ready. Reviewed production PDFium
provenance, source/relink distribution, Intel Mac support, current Windows package
runtime, full signing and secure update delivery remain outstanding.
