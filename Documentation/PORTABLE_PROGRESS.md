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

## Flat JSON input migration

The Rust table reader now streams flat JSON arrays row by row. It preserves the
first object's column order, accepts reordered keys in subsequent objects, and
retains numeric lexemes without floating-point conversion. Null becomes an empty
cell and booleans become true/false text, matching the reference table contract.
Duplicate decoded keys, blank names, mismatched columns, nested values, malformed
numbers/strings, trailing content and empty schema-less tables fail before saving.
Limits and output verification remain in force. JSON input offers only CSV/TSV;
Electron validates worker capabilities and explains scalar-to-text conversion.

Verification: 15 Rust tests, Clippy, Windows-target checking and real CLI/worker
smoke passed locally. Packaged macOS Electron selected a JSON file, showed only
CSV/TSV options and the type-conversion note, then saved CSV through a native
dialog. Python's independent CSV parser confirmed large integers, exponent text,
booleans and null mapping; the original was unchanged. Evidence is retained in
Artifacts/Verification/electron-table/json-input-verified.json.

The previous output-format CI run 34886459428 passed both Windows and macOS,
including the expanded delimited process smoke and Electron installers. The new
JSON changes require their own Windows CI run. Full app parity, process
cancellation, performance comparison, signing and secure updates remain open.

## Cooperative cancellation

The engine accepts a shared cancellation token and checks it during snapshot
copying, input reads, output writes, digest verification and before final
publication. A cancelled operation unwinds normally, removing its owned snapshot
and staging files. Cancellation after publication does not revoke a saved file;
a successful receipt remains authoritative in that race.

The internal worker accepts one compact JSON request line (EOF-terminated requests
remain supported) and an optional following `cancel` line. Electron keeps stdin
open, exposes a Cancel action, and uses cooperative cancellation at its 45-second
limit before a two-second forced-stop fallback. The fallback still reports an
uncertain outcome; cleanup after a hard kill and future codec process trees remain
release work. The standalone CLI remains usable without this control channel.

Evidence: all 15 Rust tests, Clippy, Windows-target checking and normal process
smoke pass. Tools/smoke-cancellation.py starts a real large-table worker, waits
until output staging exists, sends cancellation, and verifies a cancelled reply,
no output, no staging or snapshot files, and unchanged source bytes. This check is
now in both platform CI jobs and retained as portable-cancellation.json.
The packaged Mac app also completed a normal native-dialog conversion using the
new open stdin transport, independently verified against the original CSV. Manual
click-timing acceptance of Cancel itself is not yet recorded.

Flat-JSON CI run 34887093942 passed both Windows and macOS before this change.

## Performance evidence and reference correction

Tools/benchmark-tables.py now performs a repeatable optimized Swift/Rust CLI
comparison on identical files with independent output/source checks. Results and
limitations are recorded in Documentation/PERFORMANCE.md and its sample JSON.
The first run exposed an input/output limit mix-up in the Swift verifier; fixed
with separate 128 MiB output verification and bounded row-wise encoding. Four
focused regression/round-trip tests and all benchmark output checks passed.

Cancellation CI run 34887595037 passed Windows and macOS, including cooperative
cancellation after staging, temporary cleanup and installer packaging. Manual
Cancel-button timing and forced-stop recovery still require acceptance evidence.

## Image migration groundwork

The shared source layer now supports a caller-selected byte budget for snapshot
creation and later source verification, plus cancellable seeking over the owned
snapshot. Tables retain their 8 MiB input policy. This removes the table-specific
assumption from the common path before adding the 512 MiB image input policy;
no image route is exposed by this change.

Seventeen Rust tests pass, including growth beyond the selected source budget,
seek/read cancellation and snapshot independence from later original changes.
Clippy, Windows-target checking and real conversion/cancellation process smoke
also pass. The worker protocol and table UI are unchanged in this increment.

Documentation/PORTABLE_IMAGE_RESEARCH.md records reviewed image0.25.10, PNG/TIFF
metadata and moxcms APIs, and maps them against the native image contract. Next:
implement bounded format-specific inspection and the ICC-to-sRGB render pipeline,
then conversion/resize/crop/fit and Electron UI integration. ImageIO input coverage,
animation/high-depth/HDR rejection, orientation and transparency remain required;
using a generic decoder alone is not parity.
