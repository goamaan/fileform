# Cross-platform desktop decision and implementation plan

September 14, 2026. Status: Electron selected; first real table workflow implemented. Full portable
parity and benchmarks are not yet complete. See PORTABLE_PROGRESS.md. Read USER_REQUIREMENTS.md for the full mandate.

## Selected architecture

The user explicitly selected Electron over Tauri on September 14, 2026 after the
initial comparison, preferring Electron's ecosystem. This decision supersedes the
previous Tauri recommendation. Implement Electron + React/TypeScript with a
standalone Rust worker/CLI. Do not build a Tauri shell or reopen this choice without
new user direction. Preserve the Swift/macOS application as the verified reference
until advertised workflow parity is demonstrated.

Electron owns windows, native dialogs, lifecycle and a narrow preload bridge.
Portable Rust owns transformation planning, file transactions, scheduling and
worker/tool execution. Heavy decoding, conversion, hashing and verification stay
outside both the renderer and Electron's main event loop. The same native engine
serves the independent CLI. No Node HTTP server is needed just to connect the UI.

Both web shells support custom CSS/canvas interfaces. SwiftUI also supports
custom interfaces; Windows portability, not an inability to style SwiftUI, is the
reason to change. This remains a substantial UI rewrite plus backend port, not
merely wrapping the existing application in an Electron window.

## Current architecture: what carries over

The current app uses SwiftUI/AppKit. Swift domain records and most orchestration
are memory-safe application code. FFmpeg/ffprobe and qpdf run as native processes,
and a separate Swift worker handles native operations. Electron is not currently
used anywhere in file processing.

The engine itself is macOS-dependent: ImageIO/CoreGraphics handle image encoding,
PDFKit/AppKit handle PDF composition/rendering/text and verification, Vision does
OCR, and Darwin/POSIX APIs implement file identity, descriptor passing and child
process handling. Replacing the GUI does not make these paths work on Windows.
See PORTABILITY_INVENTORY.json, generated from actual source imports.

FFmpeg, qpdf and prospective PDF/image libraries include C/C++ code. A Rust or
Swift wrapper cannot make them memory-safe. Keep parsers isolated in subprocesses,
use safe Rust for new custom logic, audit unavoidable native boundaries, and retain
file-size/pixel/page/time/concurrency limits. No production Python dependency.

## Alternatives

| Route | Strengths for Fileform | Costs and risks | Decision |
|---|---|---|---|
| Tauri + Rust | Custom web UI; native Rust control plane; uses OS WebViews rather than shipping Chromium | WKWebView/WebView2 differences, Rust + web toolchains, platform-specific QA | Considered; not selected by the user |
| Electron + native worker | Consistent bundled Chromium; mature desktop ecosystem and renderer testing | Larger distribution and baseline runtime; frequent Chromium/Node updates; IPC/security discipline | Selected by the user; prove a real vertical slice |
| Qt Quick + Rust/C++ worker | Cross-platform desktop stack and highly custom QML UI | QML/toolchain maintenance; native dependency and license obligations; less reuse of web UI work | Valid, less aligned with this project's design/contributor path |
| Flutter + native worker | Custom rendered desktop UI on both target systems | Dart ecosystem and platform plugin maintenance; separate web design implementation | Valid, no clear advantage here |
| Separate SwiftUI + Windows UI | Strong native platform conventions | Two UI implementations, duplicated interaction/QA effort, still needs portable engine | Preserve SwiftUI as reference, not the long-term dual-UI plan |

The initial Tauri size argument was not a measured Fileform benchmark. Neither framework guarantees a fast app.
Electron's overhead mainly affects download size, launch, idle memory and UI work;
it does not intrinsically slow an independently executed FFmpeg/qpdf operation.
Avoid loading file bytes, full-resolution rasters or unbounded waveform arrays in
the renderer. Send bounded metadata/progress and lazily requested previews.

## What T3 Code uses

T3 Code's desktop package explicitly depends on Electron 44.1.0, electron-updater
and electron-builder. Its desktop entry is dist-electron/main.cjs. This was checked
against upstream commit 6f00d3881a197dd33c2cb43c6a11a9e759e56089. Its architecture
shows Electron is viable for a serious custom desktop UI; it is not evidence that
Fileform needs the same runtime or T3 Code's authentication/backend stack.

## Target boundaries

- apps/desktop: Electron + React UI, a thin main process and a typed preload
  bridge. Renderer has no general shell/process/file-system authority. Use
  context isolation, renderer sandboxing, Node integration off, strict CSP,
  sender validation and explicit navigation/external-link policy.
- crates/fileform-core: portable domain, validation, planning, scheduling and file
  transactions. Safe Rust by default; narrow reviewed platform adapters if needed.
- crates/fileform-worker: isolated job execution and native tool adapters; bounded
  framed IPC, cancellation, progress, errors and output receipts.
- crates/fileform-cli: same engine and contracts, usable without the desktop app.
- apps/macos and root Swift package: temporary working compatibility/reference
  implementation. Do not delete until parity is demonstrated.
- website: minimal free-project site, public downloads/docs/contribution links.

Use opaque file/job handles at the UI boundary. The native layer validates access,
output locations and operations; do not accept arbitrary executable names or shell
strings. Do not open a local HTTP server merely to communicate with a worker.
Bundle reviewed per-platform tools and verify them after final signing. Preserve
sources, licenses and reproducible build recipes for all native dependencies.

## Backend migration map

| Existing route | Portable approach to prove |
|---|---|
| Media conversion/trim, waveform and metadata | Retain FFmpeg/ffprobe contracts; Windows builds, pipe handling and process-tree cancellation |
| PDF structural/image compression and extraction | Retain qpdf operations and graph proofs; port orchestration/encoding and independent checks |
| Image convert/crop/fit | Evaluate safe Rust image codecs versus libvips; preserve sRGB, alpha/background, size search and exact crop contracts |
| PDF render/text/assembly validation | Evaluate PDFium plus qpdf; compare page boxes, rotations, text, visible appearances and resource limits |
| OCR | Evaluate Tesseract for shared local OCR; test output quality and reading-order limits against current fixtures |
| Tables and text | Portable Rust parsing/serialization with bounded allocation and current conversion semantics |
| File safety and cancellation | Unix handles/process groups and Windows file IDs/reparse-point handling/Job Objects; atomic no-clobber finalization |
| Persistence, selected folders and previews | Platform file permissions and paths, bounded preview caches, recovery from moved/missing sources |

Candidate libraries are not approved dependencies until their exact versions,
licenses, build artifacts, limits and fixture behavior have been checked. Preserve
capability-specific platform facts; a missing Windows backend remains an open
requirement, not silently "complete" through a disabled button.

## Implementation sequence and acceptance

1. Finish reviewed monorepo import, remove commerce UI/configuration, and update
   documentation/AGENTS. Keep private history and notarized baselines intact.
2. Build one real Electron vertical slice: native file picker → isolated Rust worker →
   bounded metadata/preview → verified transformed output → reveal/open. Build its
   Windows artifact in CI immediately; no fake conversion or success state.
3. Measure current Swift app and the candidate with the same fixtures: cold/warm
   launch, time to interactive, idle process-tree memory/CPU, active/peak memory,
   conversion throughput, cancellation latency and UI responsiveness under load.
   Record hardware, OS, runtime versions and runs; do not invent universal budgets.
4. Optimize the selected Electron implementation against those measurements.
   Use lazy previews, bounded IPC, virtualized large lists and native workers;
   avoid synchronous file I/O, oversized payloads and work on the main event loop.
5. Port current routes and UI in bounded parity increments: tables/images, media,
   PDFs/OCR, batching, saved setups/history, previews, drag/drop, keyboard and themes.
6. Add Windows process/file safety and native pack build/test jobs, then real NSIS
   installers (MSI optional). Prefer native Windows CI to fragile cross-compilation.
7. Add Electron ecosystem packaging and signed updates: evaluate/pin
   electron-builder + electron-updater against the current official contracts.
   This replaces the previously planned Sparkle integration; do not add Tauri.
   Use public GitHub release artifacts/feeds, with no login or entitlement service.
8. Sign/notarize Mac delivery; configure Windows signing when a certificate/service
   is available. Never describe unsigned Windows artifacts as signed. Test update,
   cancellation, reinstall, rollback/recovery, offline work and file associations.
9. Release only truthful supported-platform/capability claims. Windows build success
   is mandatory; native Windows worker/CLI tests and automated UI checks should run
   in CI. Record manual Windows GUI testing as pending when no Windows machine is
   available, as the user explicitly permits. Mac end-to-end UI testing is required.

## Maintenance and release rules

Pin Rust/npm/tool dependencies; review security updates and keep the lockfiles.
Electron ships Chromium/Node and requires timely runtime security releases.
Pin a supported release, track its lifecycle and test both target operating systems. Use a
strict CSP, narrow IPC commands, no remote script execution, bounded event streams
and native-only heavy work. Preserve original files and verified atomic outputs.

Signed engine manifests must be calculated after nested binary signing, before
outer app signing/notarization. The existing Organizer export changed helper
hashes; retain the working direct-signing sequence and test the new bundler's order.

## Primary sources checked

- [T3 Code desktop package](https://github.com/pingdotgg/t3code/blob/6f00d3881a197dd33c2cb43c6a11a9e759e56089/apps/desktop/package.json)
- [Electron performance](https://www.electronjs.org/docs/latest/tutorial/performance)
- [Electron security](https://www.electronjs.org/docs/latest/tutorial/security)
- [Tauri architecture](https://tauri.app/concept/architecture/)
- [Tauri native sidecars](https://tauri.app/develop/sidecar/)
- [Tauri Windows packaging](https://tauri.app/distribute/windows-installer/)
- [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)
- [Qt supported platforms](https://doc.qt.io/qt-6/supported-platforms.html)
- [Flutter desktop](https://docs.flutter.dev/platform-integration/desktop)

These sources establish architecture/support facts. The recommendation and
migration sequencing are Fileform-specific engineering judgments, not benchmarks.

## Electron ecosystem implementation details

Use React + TypeScript with a small Vite build, a separate main/preload build and
an explicit typed bridge. Evaluate electron-builder/electron-updater first for
macOS DMG/ZIP, Windows NSIS and GitHub-release update metadata; use a supported,
pinned Electron release rather than copying T3 Code's version blindly. Playwright's
Electron support, native OS interaction and engine fixture tests provide different
levels of evidence and should be used together. Keep native codecs/engines outside
ASAR, include licenses and verify the exact packaged executables on each platform.

Do not copy the old Swift app's signing entitlements into Electron. Its worker
currently inherits a macOS App Sandbox from a sandboxed parent. The Electron
parent/renderer/helper model changes that assumption; explicitly design and test
worker containment and signing for the new process tree. Hardened runtime,
renderer sandboxing and macOS App Sandbox are different mechanisms. Preserve
narrow native file access and test process-tree cancellation on Windows as well.

Use streaming bounded worker messages, throttled progress, lazy previews and
virtualized file/page lists. Keep parsing, hashing, conversion, validation and
large allocations in the native worker, not Node's main loop. Treat tool stderr
as untrusted diagnostics. Review update publisher/signature verification on both
platforms and delay restart while jobs are active. Old workspace data needs a
versioned, reversible migration; changing shells must not discard saved results.
