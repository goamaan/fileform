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

## Native PNG pixel inspection

Pinned image 0.25.10 with explicit PNG/JPEG/TIFF features and direct png 0.18.1.
The first implemented image operation is `fileform-native inspect-image FILE.png`
(or the worker's inspect_image request). It snapshots inputs under the 512 MiB
image budget, enforces 80 million pixels, applies a PNG decoder scratch budget,
uses fallible pixel-buffer allocations, fully decodes, validates the final IEND
record and decoder completion, hashes RGBA pixels and rechecks the original.
RGBA alpha and grayscale values are retained exactly; palette/low-depth expansion
uses the PNG decoder. Animated PNG and 16-bit input are rejected. Metadata flags
report ICC/EXIF/color/HDR metadata presence, not validated profiles or HDR absence.

This operation is deliberately inspection-only: conversion_available is false.
Pixels are not yet orientation-corrected or normalized to sRGB. PNG/JPEG/TIFF
render/export, full metadata checks, resize/crop/quality/fit and Electron image
controls remain required. The decoder's scratch budget is not a process RSS cap;
concurrent buffers and OS containment still need acceptance evidence.

Verification: 21 Rust tests pass, plus Clippy and Windows-target checking. The real
CLI and worker agree on independently generated PNG fixtures and exact pixel
hashes. Process checks reject missing IEND, trailing bytes, APNG, excessive
pixel dimensions and 16-bit inputs. Table and cancellation process smoke still
pass. Tools/smoke-images.py runs in both desktop CI jobs and retains
Artifacts/Verification/portable-images.json. Notices collected for 62 components;
new image runtime dependencies are not omitted from distribution notices.

## PNG orientation

Implemented strict bounded TIFF IFD0 EXIF orientation parsing and all eight
pixel transforms. Inspection now reports raw and oriented dimensions/checksums
separately. Alpha bytes are retained, allocation is fallible and the transform
checks cancellation. PNG image conversion remains unavailable pending the rest
of the render/export pipeline; see Documentation/PORTABLE_IMAGES.md.

Verification: 24 Rust tests, Clippy and Windows-target checking pass. Independent
PNG/eXIf fixtures sent through the real CLI confirm all eight expected layouts,
dimension swaps and unchanged original bytes. Normal table and cancellation
process checks also pass. PNG pipeline CI run 34889052588 has passed macOS;
Windows was still running when this increment was prepared.

## Embedded ICC normalization

Added the native RGB/gray ICC-to-sRGB transform stage using pinned moxcms 0.8.1.
The transform preserves alpha outside the CMS, uses bounded pixel blocks and
checks cancellation. Image inspection exposes a separate optional ICC-normalized
pixel hash, without enabling export or claiming general HDR/color parity.

Verification: 28 Rust tests cover profile transforms and validation alongside
existing file/table/image contracts. Independent CLI process fixtures embed sRGB
and Display P3 profiles; malformed ICC is rejected and original bytes are retained.
The P3 sample matches an independently calculated D65 P3-to-sRGB matrix/transfer
result within two code values. Grayscale gamma and wrong-profile-space checks
also pass. Clippy, Windows-target checking and table/cancellation process smoke
passed; dependency notices still include all 62 components. See PORTABLE_IMAGES.md
for remaining rendering, metadata and export gates.

## PNG gamma and metadata integrity

Implemented native gamma/chromaticity profile construction and explicit color
interpretation in image receipts. The pipeline follows color-tag precedence and
defers extended-color inputs instead of applying a lower-priority ICC/sRGB guess.
A real malformed-file fixture exposed silent ancillary-metadata rejection in the
PNG decoder; a bounded independent chunk/CRC/presence preflight now closes that
fallback path. Tables and other shared file behavior remain unchanged.

Thirty Rust tests, Clippy, Windows-target checking, independent native image
process fixtures, and table/cancellation process smoke pass. Notices remain
complete for 62 components. Prior ICC-stage CI run 34889865878 passed both
Windows and macOS. The gamma/preflight increment needs its own platform CI.

The user reconfirmed one shared Electron/React and native Rust stack for both
platforms. The Swift app remains a migration reference, not a separate final
macOS product. Full parity and release delivery remain the active objective.

## Verified native PNG export

Implemented convert-image in the Rust CLI and worker: normalize orientation/color,
write a bounded sRGB PNG, reopen and compare all pixels, recheck the source, and
publish without clobbering. Inspection reports availability for this implemented
PNG path; extended-color inputs remain unavailable. The Electron app is still
awaiting image controls, and JPEG/TIFF, crop/resize/fit remain required.

Thirty existing Rust tests and Clippy pass. Expanded independent process tests
verify successful CLI/worker export agreement, all eight oriented pixel layouts,
alpha, collisions, stale source and invalid-image denial. Table/cancellation smoke
and Windows-target checking pass. A macOS comparison using standard Apple ICC
profiles and Pillow output decoding matched all channels exactly for four opaque
fixtures; limitations and the synthetic-profile discrepancy are recorded in
PORTABLE_IMAGES.md and the retained comparison JSON.

## Electron PNG workflow

Connected shared native image inspection/export to typed Electron preload calls
and an Images workspace beside Tables. Main-process handlers validate source and
saved-image receipts, keep paths behind opaque IDs, and retain native save dialogs,
source hashes, cancellation and output-collision protection. The renderer does
not perform image transformations. Source/result state survives workspace changes.

Desktop TypeScript/Vite build and packaging passed. Actual packaged Mac app E2E
opened an oriented transparent PNG, displayed the oriented dimensions, saved it
through a native dialog, and preserved the result across workspace switching.
Independent Pillow verification proved exact oriented pixels/alpha, an sRGB tag
and unchanged original bytes. Light/dark layouts were inspected and Dark restored.
New Windows CI must build this UI increment; manual Windows UI, image previews,
other codecs/editing workflows and signed release delivery remain unfinished.

## Desktop previews and single-instance behavior

Implemented bounded native-generated PNG previews and connected them to Electron.
Thirty-two Rust tests, Clippy, Windows-target checking and real image/table/cancel
process smoke passed. TypeScript/Vite and packaging passed. The packaged Mac app
visibly displayed the oriented transparent fixture using a native pixel buffer.

The user found six development apps left running. They were all idle test builds;
all were quit and one current build was launched. Added Electron's single-instance
lock and a shared show/restore-window path. A real second executable launch exited
with code 0 while the original PID remained and only one preview app was running.
The existing image/workspace survived, verified through the native UI. Evidence:
Artifacts/Verification/single-instance.json. Windows repeat-launch GUI acceptance
is still pending. AGENTS.md now requires quitting superseded QA builds and keeping
one current preview app open. Artifact copies on disk are not installed products.

## Shared JPEG export

Added PNG-to-JPEG output to Rust and Electron, including explicit background
selection for alpha input. Native compositing preserves the selected matte;
encoding embeds sRGB and verifies a full RGB decode, dimensions, metadata and
ending before no-clobber publication. PNG's exact output verification is retained.
The current JPEG quality is 85; quality/fit controls and JPEG input remain pending.

Thirty-four Rust tests and Clippy pass. Windows-target checking and real image,
table and cancellation smoke pass. The packaged Mac app blocked JPEG save until
White was selected, then exported through the native dialog. Independent Pillow
decoding verified the white result within one channel value and the black CLI
result exactly; originals remained unchanged. The earlier preview was quit before
launching this build, maintaining one running Fileform Preview app.

## JPEG quality selection

Added validated 1–100 JPEG quality through the native request, CLI options and
Electron slider. Default remains 85. CLI image options now accept background and
quality in either order and reject duplicates/unknowns; PNG rejects JPEG-only
options rather than ignoring them. Existing 34 Rust tests, Clippy, Windows-target
checking and expanded image/table/cancellation process smoke pass.

Packaged Mac acceptance set quality to 40 by keyboard, saved through the native
dialog and compared with CLI output. JPEG quantization, compressed image data and
decoded pixels matched; only the ICC creation timestamp differed. A detailed
fixture verified quality 20 output is smaller than quality 95. Invalid quality
values and PNG misuse produced no output. Earlier JPEG CI run 34893395784 passed
both platforms. The single old preview was quit before launching its replacement.

## Native exact crop

Ported integer pixel cropping after orientation/color normalization into the
shared engine, worker request and CLI --crop option. It preserves selected RGBA
values, rejects invalid/overflowing bounds, checks cancellation and reuses existing
verified PNG/JPEG publication. Electron's interactive crop UI remains unfinished.

Thirty-six Rust tests, Clippy, Windows-target checking and image/table/cancellation
process checks pass. The CLI's rotated-image crop matches known independent pixel
layouts and a Pillow orientation/crop comparison; source bytes are unchanged.
No extra GUI test build was opened for this native-only increment.

## Visual Electron cropping

Connected crop requests to the typed bridge and native save path. The preview
has draggable/keyboard corner controls, numeric bounds, a square preset and
undo/redo. All crop values and resulting dimensions are checked in main and Rust.
Node geometry checks cover corner bounds and non-empty integer regions on CI.

The packaged Mac test exposed an undo grouping bug during drag; corrected history
recording and pointer-capture handling, then retested the complete sequence.
Drag 12→10, undo 10→12, redo 12→10, keyboard adjustment and native save all passed.
The 10×12 output matched an independent oriented-image crop, with alpha and source
checksum preserved. TypeScript/Vite, packaging and geometry checks passed. Only
one preview build was left running. This remains partial overall app parity.

## Shared native image downscaling

Implemented no-upscale maximum-dimension resizing after orientation/color/crop,
using linear-light, alpha-weighted pixel-area resampling and fallible output
allocation. Worker and CLI validate positive limits. Image execution options now
use an internal options struct rather than extending positional parameters.

Thirty-nine Rust tests, Clippy, Windows-target checking and image/table/cancellation
process smoke pass. The real CLI's output matched an independent Pillow decode
and analytic linear-light expectation. Crop/resize composition and invalid-size
no-output behavior are covered. Electron's resize controls remain next work; the
one existing GUI preview was not replaced for this native-only increment.

## Electron image resizing

Connected native maximum-dimension downscaling to desktop controls and predicted
output dimensions. Refactored the internal image export bridge to a named options
object with strict validation and receipt dimension checks. Added shared dimension
and validation tests, including large integer cases matching native rounding.

Three desktop tests, TypeScript/Vite and packaging pass. Actual packaged Mac E2E
rejected zero, demonstrated no-upscale prediction, combined square cropping with
an eight-pixel limit, and saved an 8×8 PNG through the native dialog. Its bytes
matched an independent CLI execution; Pillow confirmed dimensions/alpha/sRGB and
the source stayed unchanged. Only one replacement preview app was left running.

## Verification routing

Confirmed full Swift-reference matrix 34894538829 passed both macOS 14 and 26.
Separated Rust-only smoke triggers from reference Tools triggers to prevent
unrelated edits superseding long pack builds. Added shipped LICENSE/NOTICE to
both package workflows and a routing/coverage check in both matrices. All build,
test, E2E and packaging stages remain; manual full verification is required for
a release candidate. Local routing validation passed 17 change classes and all
directly invoked Tools scripts. See CI_VERIFICATION.md for scope and limitations.

## Strict native JPEG input

Added streaming strict baseline/progressive JPEG decoding, bounded marker/metadata
preflight, consistent ICC assembly and shared EXIF orientation. The common image
carrier now serves PNG and JPEG. Supported JPEGs use the existing native render,
crop/resize and verified export path. Unknown application metadata defers export;
CMYK/other coding modes and EXIF-only color hints remain parity work. JPEG input
is currently exposed through CLI/worker, with the Electron picker connection next.

Forty-two Rust tests, Clippy, Windows-target checking and image/table/cancel process
checks pass. Independent Pillow comparisons on generated baseline/progressive
color JPEGs verified oriented dimensions and a maximum two-code-value difference,
with originals unchanged. Dependency notices cover all 62 components. No extra
GUI application was opened for this native-only increment.

## EXIF color hints and Electron JPEG import

Added bounded EXIF color-space/interop parsing so profile-less Adobe RGB JPEGs are
not silently treated as sRGB. Standard R03 and sRGB/R98 are handled, with explicit
compatibility for ColorSpace 2; ambiguous declarations defer export. Embedded ICC
retains precedence. Inferred-profile labels distinguish them from actual ICC data.

Forty-five Rust tests, Clippy, Windows-target checks and real image/table/cancel
process smoke pass. An EXIF Adobe RGB fixture normalized identically to its
ICC-tagged equivalent; ambiguous metadata produced no output. The shared Electron
picker now accepts JPEG. Packaged Mac E2E imported an oriented progressive JPEG,
showed its preview and saved PNG matching CLI bytes and independent Pillow decode
within two values. One old preview was quit before replacement; one remains open.

## Verified TIFF export

Added the third reference image output format, TIFF, to native processing and
Electron. It uses lossless LZW, unassociated alpha, normal orientation and sRGB
ICC tagging, with full pixel/metadata verification and existing no-clobber safety.
TIFF quality/background misuse is rejected; JPEG-only controls stay hidden.

Forty-six Rust tests, Clippy, Windows-target checking, image/table/cancellation
process smoke and three desktop tests pass. Notices still cover 62 components.
The packaged Mac app saved a TIFF through a native dialog; independent Pillow
verification proved exact oriented pixels/alpha, single-image structure, LZW and
required tags, with the source checksum unchanged. One preview was replaced by
one current build. TIFF input and broader parity/release gates remain open.

## Shared TIFF input and desktop reopening

Added bounded classic/BigTIFF input, single-image checks, 8-bit RGB/gray/alpha
layouts, associated-alpha conversion, ICC/orientation and explicit plane/stride
handling. A final source review caught the decoder's planar layout behavior;
added a separate-plane fixture before completing the port. Unknown preservation
metadata and unsupported pixel depths/layouts remain explicit gates.

Fifty-one Rust tests, Clippy, Windows-target and image/table/cancel process checks
pass. Independent RGB/RGBA/grayscale TIFF fixtures matched Pillow's oriented pixels.
The packaged Mac app reopened its own TIFF, previewed it and exported PNG matching
the original TIFF's RGBA exactly. The image guide was rewritten as a current support
reference; historical implementation evidence remains here and in Git history.
Only one preview build remains open. Broader TIFF variants and release work remain.

## Native target-byte fitting

Ported the reference workflow's bounded quality search: at most eleven tested
JPEG levels including the floor, with complete-file byte checks. PNG/TIFF use one
lossless candidate. Each failed candidate is discarded; target failure publishes
nothing. No dimensions change implicitly. Receipts include quality and attempts.

Fifty-three Rust tests, Clippy, Windows-target checks and process smoke pass.
A 6,630-byte target produced 5,812 bytes at quality 72 after four attempts. A tighter
3,000-byte target reached quality 20 after eleven attempts and decoded independently
with Pillow. Quality-floor and impossible-lossless tests left no output/staging.
TIFF input CI run 34901453290 passed Windows and macOS. Desktop fitting controls
remain next work; no additional GUI preview was opened for this native increment.

## Desktop byte-size fitting

Connected exact B/KB/MB limits and JPEG quality floors to native fitting, with
strict option and receipt validation. Results now display actual bytes and quality.
Four desktop tests, TypeScript/Vite and packaging pass. The real Mac app saved
6,084 bytes at quality 75 under 6,630 bytes, then correctly failed a 1 KB target
without output/staging. CLI quantization/pixels matched, originals matched the
generated fixture, and invalid quality floors blocked Save. Save-dialog cancellation
preserved the earlier result. One preview remains open with valid settings.

### September 22 — desktop workspace design

Reworked the Electron shell with compact navigation, system typography, subdued
surfaces, a central image canvas and a separate export inspector. Grouped format,
resize and file-size controls; kept crop controls beside the preview. Added
120ms control feedback and 180ms workspace/result entrances, disabled for reduced
motion. The generated App v2 HTML informed the layout; no private design archive
or paid settings were imported.

Verification: TypeScript/Vite build and all four crop/export contract tests pass.
Packaged macOS app manually checked in dark and light appearance, maximized and
restored 1180px window. Native picker → Orientation.png → square crop → 8px resize
→ Save completed as UI-polish-crop.png (129 bytes). Independent CLI inspection
confirmed 8×8 sRGB with alpha. Only one preview app is running. Windows manual UI
acceptance remains unavailable; these changes use the shared renderer. This is a
preview increment, not a signed or feature-complete release.

### September 22 — signed and notarized Electron preview

Added a fail-closed signed macOS builder configuration and a Keychain-profile
notarization helper. The helper rejects ad-hoc builds, verifies the app and worker
publisher match, requires Apple's Accepted status, staples and checks the ticket,
and assesses the result with Gatekeeper. Only JIT entitlement is granted; hardened
runtime and existing Electron fuses remain enabled.

The arm64 preview was accepted as submission
`f6b7eba5-388f-44de-a636-f0ad84fe1d1b`. A post-stapling ZIP was extracted to a fresh
folder; stapler validation and Gatekeeper returned Notarized Developer ID. Its
SHA-256 is `56d53a98c0dc8528e38beeda658f981fce04fa3ba12015468a499eceab3acf71`.
The extracted app launched and converted Orientation.png to a 12×16, 119-byte PNG.
CLI decoding confirmed exact pixels against the oriented source. The older preview
was closed first. Local artifacts and credentials remain outside tracked source.

See ELECTRON_RELEASE.md for reproduction and remaining release gates. This does
not establish Intel Mac acceptance, Windows signing, automatic updates, feature
parity, or public release availability.

### September 22 — native Open menu and keyboard import

Added File → Open with Command+O on macOS and Control+O on Windows. A narrow
preload subscription routes the command to the currently visible workspace;
listeners are removed on unmount and read committed state. Existing main-process
sender validation and exclusive operation guards remain in effect. No file paths
or IPC event objects are exposed through the subscription. Open is ignored while
an operation or native dialog is active. Native editing/window/zoom menu roles
remain available; reload/developer-tools actions are not included in the app menu.

Packaged macOS validation: Command+O imported a two-row CSV, exported JSON and
independent parsing matched both rows exactly. File → Open in Images imported the
orientation fixture through the native worker. Cancelling a subsequent picker
retained the image. Build and all four crop/export contract tests pass. The CUA
inspection reactivates a closed Mac window, so a truly windowless Open command is
not established by this manual check. Windows GUI acceptance remains pending.
Drag/drop, file associations and broader task navigation are still open UX01/UX02
requirements; this does not mark the complete import requirement done.

### September 22 — user correction: parity before final UX

The user rejected category workspaces and requested a universal file-first entry
plus an optional action-first route. The original HTML supports this direction.
Recorded the requirements in USER_REQUIREMENTS.md and AGENTS.md. Uncommitted
workspace-specific drag/drop work was preserved under ignored local
Artifacts/parked-workspace-drop, then removed from the active source. Its package
built but native drag/drop was not verified and it is not a delivered capability.

Next implementation priority is remaining original-app backend/workflow parity:
media conversion/extraction/trim, PDF composition/page operations/compression,
text/OCR, then batch/persistence/result workflows, with full requirements retained
in REQUIREMENTS.json. The final UX must organize those capabilities around files
and tasks, not engine-specific workspaces. Signing/updates and release gates remain
required. No claim of feature completeness is made.

### September 22 — portable media-pack validation

Ported the original media-pack integrity boundary to safe Rust, exposed through
CLI and worker. Manifest/file reads are bounded, tool hashes checked, declared
architecture/network policy validated, and cancellation supported. Windows uses
.exe tool names. Added four tests for verification/tampering, invalid manifests,
cancellation and Unix out-of-pack symlinks. All 57 Rust tests, Clippy, release build
and Windows target check passed. Both CLI and worker verified the existing real
Mac FFmpeg/LAME pack. Details and remaining media parity work are in
PORTABLE_MEDIA.md; no new category UI or transformation-completion claim was added.

### September 22 — native media inspection

Added a bounded direct-process adapter and ffprobe inspection through the Rust CLI
and worker. Uses verified pack, source snapshot/recheck, restricted demuxers and
protocols, bounded output pipes, timeout and cancellation/reaping. No category UI
was added. Real WAV/MP4 fixtures passed both entry points with exact expected
stream counts/duration; invalid input failed and source hashes stayed unchanged.
All 60 active tests, Clippy, release build and Windows target check passed. See
PORTABLE_MEDIA.md for process containment, executable-launch race, Windows pack,
format/metadata and transformation gaps still required for release.

### September 22 — native audio conversion and extraction

Added WAV/FLAC/M4A/MP3 conversion through Rust CLI/worker, including extraction from
single-audio-track video. Preserved original codec/metadata policies and FLAC/MP3
limits. Outputs are staged privately, inspected, fully decoded, synced and
published without overwriting. Added reproducible real-tool smoke coverage for
all formats, PCM equality, video extraction, collisions, stale source hashes,
cancellation cleanup and high-bit-depth FLAC rejection. All passed against the
real Mac pack; 61 active Rust tests, Clippy, release build and Windows target check
also passed. Media UI, video conversion/trim and Windows media-pack execution
remain pending. See PORTABLE_MEDIA.md for precision policies and hardening limits.

### September 22 — prevent size-limited partial audio

Replaced FFmpeg's truncating file-size flag with staged-output monitoring and a
post-exit size check. Exceeding the limit fails and reaps the encoder rather than
publishing a possibly shortened recording. Added a successful-but-oversized child
regression test. Extended real-tool coverage to exact 24-bit FLAC, float rejection,
unsupported MP3 resampling and multi-track rejection. The audio smoke suite, 62
active Rust tests, Clippy, release build and Windows target check passed. Windows
media run 35776416445 moved from tool setup to source compilation and remains live.

### September 22 — exact decoded-sample audio trims

Added CLI/worker trimming to WAV/FLAC using explicit decoded-sample boundaries.
Verified sample count and decoded source/output PCM hashes before no-clobber
publication. Real-file tests prove exact standard/24-bit sample slices, one-sample
output and no publication for invalid/past-end ranges. Existing media smoke tests,
62 active Rust tests, Clippy, release build and Windows target check pass. This is
not source-clock/packet-copy/video trim parity; those requirements remain open.
Windows media run 35776416445 remains actively compiling its tool pack.

### September 22 — H.264/AAC MP4/MOV stream-copy route

Ported video remuxing to Rust CLI/worker with staging, size monitoring, source and
output-folder checks, no-clobber publication and decoded-content verification.
Verified packet/timing equality on an original synthetic fixture through a
round trip, plus silent/rotated cases and unsupported-codec rejection. Included
the small generated fixture/provenance for Windows CI. Full media smoke, 62 active
Rust tests, Clippy, release and Windows target builds/checks pass locally. Video
re-encoding, resize/compression, trimming and final UI integration remain open.

### September 22 — cancel desktop jobs when their supervisor disconnects

Added an explicit supervised worker envelope and switched Electron to it. Closing
or invalidating the control pipe now cancels an active desktop job; bare requests
still support one-shot CLI use. Real process checks observed staging, disconnected
the parent pipe, and verified cancellation/no output/temporary cleanup/source
preservation. A connected-supervisor request and legacy CLI smoke also passed.
Rust tests, Clippy, release build, Windows target check and desktop TypeScript/Vite
build passed. Forced worker termination and process-tree containment remain open.
Windows media job 35776416445 is still actively building its source-verified pack.

### September 22 — Windows cancellation line-ending fix

Windows media run 35776416445 built the verified FFmpeg/LAME pack and Rust tools,
then passed conversion/extraction assertions before failing the cancellation case.
Python's Windows text pipe translated `cancel\n` to `cancel\r\n`; the bare worker
protocol only recognized LF and allowed the conversion to finish. The worker now
accepts both. Added LF and CRLF staging-cancellation regression cases to the
cross-platform process smoke suite. Both, supervisor disconnect/healthy completion,
62 active Rust tests, Clippy, release build and the full local media smoke pass.
Known-bug media runs 35778405811 and 35778161960 were cancelled deliberately so the
corrected revision can run; the original failed run is retained as evidence.
Windows acceptance remains pending the corrected real-tool run.

### September 22 — native H.264 video encode and resize

Extended the shared transaction/verification route with VideoToolbox/Media
Foundation H.264 encoding, explicit bounded even-dimension resizing, rotation
normalization and single-track audio preservation/transcoding. Added real CLI and
worker tests for output dimensions, full frame decoding/picture comparison,
rotation, PCM-to-AAC and invalid settings. Corrected the old rotation fixture:
FFmpeg 9 needs display_rotation rather than the ignored rotate metadata tag.
All local media smoke checks, 63 active tests, Clippy, release and Windows target
checks pass. Windows encoding runtime remains pending. No permanent category UI
was added; size fitting, source-clock/video trimming and complete UI parity remain.

### September 22 — audio byte-limit fitting and Windows media baseline

Added bounded AAC/MP3 bitrate fitting and single-attempt WAV/FLAC size checks to
CLI/worker. Candidates are fully verified, oversized candidates removed, floors
respected and only measured fits published. Receipts report requested bitrate and
attempt count. Full Mac media smoke includes actual fits, floor failures and
lossless successes/failures with cleanup; Rust tests, Clippy, release build and
Windows target checks pass.

Corrected Windows run 35778556255 at 60d82c2 passed real tool builds and its media
suite, establishing audio/extraction/sample-trim and stream-copy baseline runtime.
Newer video encoding/resize/rotation fixes and audio fitting are not covered by
that result. See PORTABLE_MEDIA.md for exact evidence boundaries and remaining
manual GUI/signing/integration requirements.

### September 22 — verified video byte-limit fitting

Added bounded native bitrate search with explicit quality floor/optional resize,
128 kb/s AAC fit policy, complete candidate verification and measured no-clobber
publication. Real Mac test fits a 60-frame detailed clip to 141,803 bytes under a
200,000-byte limit after five attempts, retaining all frames and dimensions.
Audio-bearing input and impossible-target cleanup pass. Video smoke, 64 active
Rust tests, Clippy, release and Windows target checks pass. Windows real encoding
and fitting remain pending the live Media Foundation pack/test job.

### September 22 — audio clock proofs and Windows encoder build correction

Ported bounded rational audio-timeline inspection to CLI/worker. Normal/offset
fixtures pass; a real AAC timestamp gap fails the continuity check. Parser/unit
coverage rejects oversized arrays, gaps, overlaps and missing timing. Full media
smoke, 66 active Rust tests, Clippy, release and Windows target checks pass.
Source-time trim planning still needs to use these proofs.

Windows encoder run 35779787420 failed with missing D3D11 types in FFmpeg's Media
Foundation source. Enabled its D3D11VA dependency explicitly and updated system-DLL
checks. Cancelled the remaining known-bad build 35781000000; corrected runtime
verification is not yet complete. This does not invalidate the earlier passing
Windows audio/stream-copy baseline, which did not enable this encoder.

### September 22 — source-time WAV/FLAC trimming

Connected verified audio clock inspection to exact rational time selection and
sample trimming through CLI/worker. Decimal seconds retain up to nanosecond input
precision; boundaries use half-open sample-onset semantics. Receipts report both
requested/realized intervals and source origin. Real normal/offset recordings
produce exact PCM slices; gaps, stale bindings and invalid ranges publish nothing.
Full media smoke, 67 active Rust tests, Clippy, release build and Windows target
check pass. Lossy/fast/video trim, track selection and final UI remain open.
The corrected Windows Media Foundation/D3D11 build is still active.

### September 22 — decoded video timeline proofs

Added bounded CLI/worker video timeline inspection comparing packet and decoded
frame clocks, with confirmed keyframes and reorder detection. Real CFR/reordered/
variable-timing fixtures pass their expected outcomes. Unit coverage checks clock
mismatch and the 200,000-record parser bound. Rust tests, Clippy, release build,
Windows target check and media smoke pass locally. This prepares exact/fast video
trim planning without claiming those output routes are complete. Corrected Windows
encoder build 35781819917 is still active.

### September 22 — exact video trim and targeted Windows diagnostics

Connected video/audio clock proofs to exact video trim execution, rational frame
selection, audio sample alignment and explicit muting. Real picture/audio checks,
invalid-range cleanup, both media smoke suites, 70 active Rust tests, Clippy,
release build and Windows target check pass locally.

Windows run 35781819917 built the MF/D3D11 pack and directly encoded/decoded H.264,
but the native 32×24 resize failed with an intentionally generic error. Added
bounded opt-in stderr diagnostics and a provenance-checked manual pack-reuse mode
for a targeted rerun. No cause or Windows trim acceptance is claimed yet.

### September 22 — preserve small output sizes on Windows

The MF diagnostic matrix isolated the native 32×24 failure to small dimensions,
not environment or passthrough timing. Added internal padding plus measured SPS
cropping and a display-dimension remux for small Windows encodes. Output dimensions
remain the requested values; no test or public constraint was weakened. Prototype
verification includes FFprobe, MP4 track headers and AVFoundation at 32×24. Unit,
Clippy, release/Windows target checks and Mac regressions pass; Windows runtime
validation of the adapter is pending the targeted rerun.

### September 22 — regression guard for video display dimensions

Added independent MP4/MOV track-header dimension checks to the real video smoke
suite. Resized and rotation-normalized output must report the intended display
rectangle as well as correct decoded pixels, preventing stale padded container
dimensions from passing decoder-only checks. The full Mac video suite passes.
Windows run 35786756236 remains active on the padding-adapter revision; the new
container assertions will run on the next current-revision validation.

### September 22 — Windows media adapter and end-to-end suite passed

Run 35786756236 at fa0dd71 passed the full real Windows media/video suites with the
verified reused MF/D3D11 pack. It proves 32×24 resize, upright 48×64 rotation,
60-frame fitting at 173,201 bytes after five attempts, exact nine-frame trim with
picture/audio checks, muting and the broader audio/timeline/cleanup cases. This
resolves the observed small-frame failure. The newer container-header assertions
are being run separately at the latest revision; manual GUI and final release
integration remain unverified.

### September 22 — packet content/timing evidence and display regression acceptance

Added bounded encoded-packet inspection for fast-trim verification, with payload
hashes, exact timestamps and a compact sequence digest. Independent audio/video
packet comparisons and invalid-stream rejection pass. All 73 active Rust tests,
Clippy, release build, Windows target check and media smoke pass locally. Fast
copied-output generation remains unfinished.

Windows run 35787219817 at 1ba5522 passed the independent container-display checks
and prior complete native media suites. This closes the display-dimension
regression gap; it does not cover the newly added packet-reader entry point or GUI.

### September 22 — fast AAC packet-copy output

Added M4A fast trimming for single AAC tracks in MP4/MOV-family files. Boundaries
snap outward and are reported; copied payload hashes, timestamps, durations,
coverage, layout and full decoding are verified before no-clobber publication.
Real middle/full-recording packet comparisons and invalid-range cleanup pass.
Rust tests, Clippy, release/Windows target checks and media smoke pass locally.
Windows execution of this route and fast video output remain pending.

### September 22 — restore original trim precision/channel guards

Reference comparison found that exact FLAC trim must reject floating-point decoded
AAC even though ordinary conversion permits it. Added decoded sample-format
inspection and shared trim validation for audio, fast AAC and retained video audio.
Real AAC/FLAC and nine-channel regressions pass; explicit conversion remains usable.
All 74 active Rust tests, Clippy, release build, Windows target check and media
smoke pass locally. Fast video packet-copy output remains the next media gap.
