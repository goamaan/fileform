# Portable implementation progress

September 14, 2026. Electron is the user-selected shell. The goal remains the full
free/open-source macOS and Windows application; this record does not narrow it.

## First implemented slice

- Rust 1.98.1 is pinned per project; the machine's default toolchain was not changed.
- `fileform-engine` implements strict streaming CSV/TSV parsing and JSON export.
  Limits match the reference input/row/column bounds: 8 MiB, 100,000 rows, 1,000
  columns. Output is bounded at 128 MiB. JSON input and other outputs are not ported.
- Sources are snapshotted, hashed and rechecked. An inspected hash can prevent
  saving changed content. Results are decoded/compared against source records,
  hashed, synced and published without replacing an existing path.
- Safe Rust is enforced for custom engine/CLI/worker code. This does not claim
  complete hostile-filesystem race resistance or the safety of future C/C++ tools.
- A standalone CLI and one-request worker share the same implementation.
- Electron 44.3.0, React 19.3.0, TypeScript 7.0.2 and electron-builder 26.15.3 are
  pinned with lockfiles. React is bundled into the renderer; no separate React
  runtime dependencies are copied into the desktop package.
- The Electron UI uses native open/save dialogs, opaque source/result IDs, typed
  preload methods, bounded worker messages, restricted environment, a local custom
  protocol, CSP, renderer sandboxing/context isolation and packaging fuses.

## Evidence

- `cargo test --workspace`: 9 tests passed, including strict quoted/unicode data,
  invalid tables, source changes, concurrent publication and source/output aliases.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo check --workspace --target x86_64-pc-windows-msvc`: passed. Not a linked
  Windows binary, and not Windows runtime verification.
- `Tools/smoke-portable.py`: real worker/CLI processes passed; values and originals
  retained, collisions and malformed requests rejected. Script is platform-neutral.
- Packaged Mac Electron UI: native picker, real inspection, save dialog, conversion,
  Finder reveal, Light/System appearance and persisted preference passed. Existing
  output rejection preserved both its bytes and modification time.
- Hardened package: `fileform://app/index.html` loaded; a second native-dialog
  conversion matched the CLI bytes after protocol/environment/fuse changes.
- Electron fuse reader confirmed disabled RunAsNode, Node options/CLI inspection,
  file-protocol extra privileges; ASAR integrity and ASAR-only loading enabled.
  Its older enum labels Electron's additional default fuse as undefined; no claim
  is made that all runtime fuses were changed.
- npm installation audit reported zero known vulnerabilities at installation time.
  This is not a substitute for the pending full source/dependency security review.

Local proof: `Artifacts/Verification/electron-table/` and
`Artifacts/Verification/portable-smoke.json`. Preview packages are under
`apps/desktop/artifacts/`. They are development previews, not signed customer builds.

## Next required work

1. Complete source/license/secret review and public canonical repository migration.
2. Run the new native Windows/macOS desktop CI and retain real Windows installer
   artifacts. Do not confuse configured CI with executed CI.
3. Continue worker safety/cancellation, input/output parity and UI migration against
   the existing Swift contracts and fixtures; keep the reference app functional.
4. Record comparable startup, idle memory, active memory and throughput benchmarks.
   One local process RSS sample exists; it is not a comparative performance result.
5. Complete the free Vicinae-inspired website, public downloads, secure updates,
   contributor/security docs and release installation acceptance.


## Public consolidation and Windows build

The canonical public repository is now https://github.com/goamaan/fileform.
Private native/site repositories are archived; their histories remain private.
The minimal free-project website is deployed publicly at
https://fileform.amaangokak18.chatgpt.site from the canonical website source via a
private deployment mirror. The mirror's .fileform-source.json records provenance.

Desktop CI run 34846825786 at b5a175c3750c0408e4590d31ab2f6ba46614f39c passed on
windows-2025 and macos-26. Windows ran the Rust tests, Clippy, release compilation,
real CLI/worker smoke and Electron NSIS packaging. Artifact
fileform-preview-Windows-X64 contains a 112,429,353-byte installer, SHA-256
0fde56640709f59d29a534afb869593fa674e67e1f6d97c4e6ebfacfc616eec5.
Its PE header has no embedded Authenticode signature. This is an unsigned preview;
manual installation/UI testing and full Windows workflow parity remain open.

The core/reference matrix passed macOS 26. The newly added macOS 14 app build
exposed older-SDK Undo callback isolation annotations. Synchronous MainActor
bridges fix those callbacks; all 63 local native tests pass. A fresh matrix run
will verify the older compiler. No platform check was removed to hide the failure.

## Delimited output migration

The portable worker and CLI now save CSV and TSV as well as JSON. Electron has
an output-format selector and native save-dialog filters. The main process
validates the format and extension, and the engine independently validates the
output extension. Delimited output streams one record at a time with CRLF and
quote escaping, then reopens and compares every cell and the complete row count
before publication. Existing files remain protected by the same no-clobber path.

Verification: 12 Rust tests pass; Clippy and Windows-target type checking pass.
The process smoke independently parses CSV/TSV with Python, converts them back to
JSON through the CLI, and rejects output collisions. Packaged Mac Electron saved
both formats through actual native dialogs; independent parsing matched every
source cell and the original SHA-256 stayed unchanged. Evidence:
Artifacts/Verification/electron-table/formats-verified.json.
JSON input, cancellation and full non-table capability parity are still open.
The new Windows CI run must verify these output changes on Windows.

Reference CI run 34884604844: the macOS 14 job now passed its complete verification,
including native compilation and CLI E2E, after the SwiftUI preference fix.
