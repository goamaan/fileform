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
