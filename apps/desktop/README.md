# Electron desktop migration preview

Electron + React UI with an isolated, optimized Rust worker. This is the first
working migration slice, not the full Fileform replacement or a public release.
The native macOS reference remains in `apps/macos` with the broader capabilities.

The app has one file picker/drop surface, contextual actions and an action-first
chooser. Thirteen main actions connect table/image conversion, PDF assembly/split/
compression/image export, audio/video conversion/trim and embedded/English OCR
text export. Image crop/resize/fit controls remain contextual. PDF thumbnails and page
order/rotate/remove/duplicate/insert controls, toolbar undo/redo and visual split
markers are connected; arrangements survive switching PDF/page-image output. Audio/video playback, posters, waveforms and visual trimming are connected through
private streamed preview leases; exports continue to use original files. All five
reviewed tool packs are bundled. Main processing stays native and file IDs are
opaque to the renderer. Follow Documentation/BREADTH_FIRST_PARITY.md.
Light/dark/system appearance and normal display-filling launch are implemented.
No accounts, activation or payments are part of Fileform.

## Build on the target operating system

Build the Rust CLI/worker on the target operating system, then stage all five
reviewed native packs using `Tools/stage-desktop-runtime.py`. Pass `--worker`,
`--cli`, `--media`, `--pdf`, `--renderer`, `--ocr`, `--heic` and a new
`--destination Artifacts/DesktopRuntime`. The stager verifies declared file hashes,
architecture, licenses and actual native loading before publishing the runtime.

```sh
cargo build --release --workspace --locked
npm ci --prefix apps/desktop
npm run build --prefix apps/desktop
npm run notices --prefix apps/desktop
cd apps/desktop
npx electron-builder --dir --config electron-builder.cjs --publish never
```

`FILEFORM_RUNTIME_ROOT` can select a separately staged development runtime.
Packaged apps resolve their own `Contents/Resources/native` (Mac) or equivalent
Windows resources. No developer environment variable or external tool installation
is required by the packaged image/table flows.

Windows CI imports successful recipe-matching main tool-pack builds, stages them
with the current Rust worker and produces an NSIS installer. Mac CI builds the
pinned tools and stages a matching arm64 runtime. Intel Mac staging has not yet
been added to this evaluated build route. Current Mac media/PDF packs require
macOS 14; the installer declares that actual floor.

`private: true` prevents accidental npm publication; this source is Apache-2.0.
Native dependency notices and source archives remain in their packaged folders.
The unsigned development runtime is not a production release. The old worker-only
signing recipe now refuses a full-runtime build until nested tool signing, pack
manifest refresh and enclosing bundle verification are implemented correctly.

## Boundaries

The preload bridge exposes only picker, conversion, result reveal and appearance
operations. The main process validates the originating frame and uses opaque file
IDs; arbitrary renderer paths/commands are not accepted. The worker receives
bounded JSON on stdin and returns a versioned receipt. It snapshots and verifies
sources, preserves table values, and publishes with no-clobber semantics.

The renderer is sandboxed, isolated from Node, restricted by CSP and served from a
local custom protocol. Unused Node execution/inspection hooks are disabled in
packaged fuses. ASAR integrity and ASAR-only loading are enabled. The worker gets
a minimal environment. Work blocks ordinary close/quit until it finishes.

## Verification and open gates

Mac native-dialog end-to-end checks passed for inspection, saving, light mode,
appearance persistence, existing-output rejection and Finder reveal. Saved bytes
match the standalone CLI and an independent CSV parser. The hardened package was
retested after changing the protocol, environment and fuses.

Native Mac/Windows runtime tests and the current Rust test suite are documented
in Documentation/CI_VERIFICATION.md. Full-runtime package evidence is recorded in
Documentation/ELECTRON_RELEASE.md. Native success does not establish desktop
exposure or signed-release acceptance.

Remaining: complete original editing/playback policy and history/large-file
acceptance, automatic multilingual OCR, URLs,
batches/setups/persistence, exhaustive shared-intake/GUI acceptance,
Windows packaged-app checks, native isolation/crash cleanup, final performance,
complete licensing/rebuild delivery, full-runtime signing/notarization, secure
updates and final website/copy acceptance.

## Shared workflow verification

After a desktop build and complete runtime stage:

```sh
node apps/desktop/tests/native-workflows.mjs
```

This runs the actual inspector/task planner against real bundled tools, generated
owned documents and owned image/media fixtures. It verifies all 13 native action
routes, image/scanned-PDF OCR, unchanged source hashes and stale-source rejection.
It is module/process integration, not renderer or native-dialog automation.
Packaged Mac UI checks separately exercised table conversion, PDF text/split and audio extraction
with native dialogs, resulting files, and theme switching. Windows CI runs the
same integration script before packaging; manual Windows GUI remains unavailable.
