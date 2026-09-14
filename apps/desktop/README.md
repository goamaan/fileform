# Electron desktop migration preview

Electron + React UI with an isolated, optimized Rust worker. This is the first
working migration slice, not the full Fileform replacement or a public release.
The native macOS reference remains in `apps/macos` with the broader capabilities.

Current flow: native picker → CSV/TSV inspection → native save dialog → verified
JSON output → reveal in Finder/Explorer. Files are not processed in the renderer.
Light/dark/system appearance and normal display-filling launch are implemented.
No account, activation, trial, device limit or payment integration exists.

## Build on the target operating system

From the repository root:

```sh
cargo build --release --workspace --locked
npm ci --prefix apps/desktop
npm run build --prefix apps/desktop
cd apps/desktop
npx electron-builder --dir --config electron-builder.cjs
```

On Windows, use `npx electron-builder --win nsis --x64 --config electron-builder.cjs`
for an installer after building the native Windows worker. Do not package a Mac
worker into a Windows app. CI is configured to build on native Windows/macOS hosts.
`private: true` in package.json prevents accidental npm publication; this source is
open source under the root Apache-2.0 license.

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

Nine Rust tests and Clippy pass. The Rust workspace passes a Windows MSVC-target
check on this Mac. This is **not** a linked Windows executable or Windows runtime
acceptance. The new CI workflow has not run yet because public repository
consolidation/publication is still pending.

Before public release: full operation parity, Windows build/runtime acceptance,
worker cancellation/job recovery and directory-race hardening, persistence/data
migration, performance benchmarks, complete third-party notices, code signing,
notarization, updates, release metadata and the redesigned website.
