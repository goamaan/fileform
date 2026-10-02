# Verification workflows

The verification workflows remain manually dispatchable. A release candidate needs successful
full Core verification and Portable desktop preview runs for its exact source,
plus the documented signing, artifact and installation acceptance checks. Path
filtering during development is not a substitute for release verification.

- Core verification: Swift source/tests/package, native macOS reference, reference
  tools, workflow configuration and shipped LICENSE/NOTICE. Its four Rust-only
  smoke scripts are excluded so their edits do not cancel a long reference run.
- Portable desktop preview: Rust workspace/toolchain, Electron app, the four
  portable smoke scripts, routing check, workflow configuration and LICENSE/NOTICE.
- Dedicated Windows media, PDF and OCR workflows build/check native dependencies
  and real fixtures. Their evidence does not establish desktop GUI acceptance.
- Website changes use the website build/publication workflow, not either native
  engine matrix merely because they share a repository.

The reference matrix retains macOS 14/26 compilation, pinned media/PDF packs, full
Swift tests, native app tests, CLI E2E and development archive packaging. The
portable matrix retains Windows/macOS Rust tests, Clippy, process/image/cancellation
smoke, desktop validation tests, notices and package creation. No stages were
removed when the path filters were separated.

`python3 Tools/check-ci-routing.py` checks push/PR filter agreement, representative
change routing, manual dispatch and directly invoked Tools scripts. Its matcher
intentionally supports only the literal/*/** filters currently used; it fails on
unrecognized syntax. It does not claim to interpret arbitrary GitHub YAML/globs
or discover all transitive script dependencies. Add helpers and new verification
scripts to the appropriate triggers and extend the routing cases when needed.

GitHub evaluates positive/negative path patterns in order; exclusions follow the
broad Tools include in the core workflow. See the [official workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#onpushpull_requestpull_request_targetpathspaths-ignore).

Evidence: full reference run 34894538829 at
2259e35317f7010a2dd5d5f9fe4625c55b13e7e6 passed on both macOS runners, including the
expanded-table verification correction. Later image/frontend increments require
their own portable evidence; a cancelled job is never recorded as a pass.

Desktop layout commit `d9089e2fd224f4240b84dbefd779a0c95e6f8e60` passed portable
workflow run `35773207660` on both macOS 26 and Windows 2025 (September 22, 2026).
This proves the configured build/tests and installer packaging, not manual Windows
GUI acceptance or signed public distribution.

## September 23 checkpoint

[Desktop run 35829743542](https://github.com/goamaan/fileform/actions/runs/35829743542)
passed both macOS 26 and Windows 2025 jobs at 61f2caa. This includes the source-type
smoke and native tests/builds plus preview packaging; it does not establish manual
Windows GUI acceptance or final bundled-tool release readiness.


October 1: [Windows PDF parity run 36921065308](https://github.com/goamaan/fileform/actions/runs/36921065308)
at `36dd0b1f2ebfb9c409c5a68cfe0a416ce12a13f3` passed source-verified qpdf,
PDFium evaluation attestation/helper, source-built OCR, and all real PDF runtime
fixtures including intrinsic embedded-image extraction and targeted lossy image
optimization/fit. The previous extraction flush failure is corrected. This is
native Windows automated evidence, not Electron integration or manual GUI acceptance.

October 1: [Windows PDF planning/policy run 36924240580](https://github.com/goamaan/fileform/actions/runs/36924240580)
at `9c8c075` passed all PDF fixtures, including native range/interval/marker
planning and actual rename receipts. Media run `36924240639` and desktop preview
run `36924240663` at the same commit passed their existing suites.

[Windows audio selection run 36925202391](https://github.com/goamaan/fileform/actions/runs/36925202391)
at `10975e7` passed real distinct-track selection, exact M4A trimming, existing
media smoke and Media Foundation encoding/decoding. This covers audio-only routes;
the subsequent selected-video-audio increment requires its own runtime evidence.

[Windows selected video audio run 36926000283](https://github.com/goamaan/fileform/actions/runs/36926000283)
at `90d400c` passed remux/convert/fit/exact/keyframe video selection with real distinct
tracks, picture/audio/packet proofs, mute and rejection checks, plus prior media
and audio selection suites. Native playback export is a subsequent increment.

[Windows playback run 36926777751](https://github.com/goamaan/fileform/actions/runs/36926777751)
at `5b305ec` passed complete selected audio/video playback exports with offset
normalization, content/duration/dimension/hash verification, cancellation and
changed-source cleanup, plus prior native media suites. Player/cache integration
and signed-packaged-app acceptance remain open.

[Windows HEIC dependency build 36927492490](https://github.com/goamaan/fileform/actions/runs/36927492490)
at `6ac9bed` passed the pinned libheif/libde265 shared-library source build. It proves
dependencies only; the subsequent isolated helper and runtime fixtures have a new CI gate.

Windows at `911fc26`: [HEIC run 36935031114](https://github.com/goamaan/fileform/actions/runs/36935031114)
passed the helper, relocatable shared-library pack, Rust common-input pipeline and
ImageIO/color fixtures after the LF checkout fix.
[PDF run 36935031141](https://github.com/goamaan/fileform/actions/runs/36935031141)
passed the complete PDF suite plus HEIC image assembly and image OCR. Subsequent
container/orientation changes require their own current-head runtime acceptance.

## Shared desktop intake checkpoint — October 1

The shared inspector/task planner integration runs 13 main action routes with the
real bundled runtime, including embedded PDF images/text, English HEIC/scanned-PDF
OCR, table/image/media conversion and PDF/audio/video trimming or splitting. It
checks unchanged source hashes and rejects stale PDF bindings before output.
The desktop workflow runs this after compilation on both platforms.

Local checks: 103 Rust tests passed (2 explicit subprocess fixtures ignored by the
ordinary runner), Clippy passed, Windows MSVC cross-target compile passed, four
desktop crop/export tests passed, routing passed, and all 13 integration routes
passed with the complete staged runtime. Decimal PDF quality requests now have a
regression check: arbitrary-precision JSON numbers previously failed when buffered
inside the tagged request. Packaged Mac GUI native-dialog checks saved PDF text,
split two PDF pages, converted a table and extracted audio from video; PDF/audio outputs reopened in
the native inspector, text content was verified, and both themes were inspected.
Exhaustive GUI, real Finder drop and manual Windows GUI remain unverified.

The earlier full-runtime [run 36941390949](https://github.com/goamaan/fileform/actions/runs/36941390949)
passed Mac staging/packaging but failed Windows packaging because Node's `x64`
name was compared with the runtime's `x86_64`. Both loader and builder now normalize
that alias. The corrected shared-intake increment still requires current-head CI;
the older run must not be cited as a complete two-platform pass.


`ebfc2164fcdf87112c584095cfd9e96efe968438` passes the complete bundled desktop
[run 36947127261](https://github.com/goamaan/fileform/actions/runs/36947127261) on
macOS 26 and Windows 2025: native tests/checks, all 13 shared workflow module routes,
complete processing runtime, notices, Mac archive and Windows NSIS packaging.
The dedicated Windows HEIC (36947127247), media (36947127238), OCR (36947127235)
and PDF (36947127243) runs also pass. This closes the observed Windows architecture
alias failure, not signing/public-release or manual Windows GUI acceptance.

## PDF desktop page editing — October 1

Local shared workflow integration now also verifies native page thumbnails bound
to the inspected identity, stale-content rejection/temporary cleanup, source-ID
validation, explicit page order/duplicates/rotations, rotated page-image exports
and marker split plans. The same test runs in the desktop CI matrix.

Packaged Mac GUI: owned two-page PDF thumbnails loaded; moving/rotating/duplicating,
toolbar undo/redo, removing and undoing removal worked. Inserting an owned HEIC
kept the existing edits. The saved four-page PDF reopened with rotations 90/90/0/0,
text page 2 twice, then page 1 and the inserted image. Visual split-after-page-1
saved two complete one-page documents. A separate edit switched from PDF to PNG
without losing page order/rotation; its two 144-DPI outputs reopened as 400x600
images with the independently checked blue-page-2 then red-page-1 content.
Both theme layouts were inspected. Preview data is capped to 12 pages per request,
240-pixel bounds, 512 KB per encoded PNG, and 96 retained thumbnails; temporary
preview folders are removed after success/failure. Renderer paths are not exposed.

Current PDF editor limitations: selecting/deselecting source files resets the
per-editor undo/redo stack (existing page edits for retained sources are preserved),
history is not persisted across other task families/restarts, and native menu/
keyboard history plus large-document/drag/Windows GUI acceptance require the final
whole-app pass. Native metadata/interactive/protected-file limits remain explicit.
This is a broad app route increment, not a full original-policy or release claim.

The shared action catalog also checks inspected capabilities: silent video has no
audio conversion/trim actions, and non-convertible images do not offer processing
that requires decoded SDR pixels. Native integration creates and inspects a muted
video to verify the audio-action rejection before any save. Packaged Mac action-
first QA selected that owned silent video for audio extraction; the app explained
the unavailable action and offered only video conversion/trim.

`bf471aebe80c86cffe7e8817b3aca4005a28b92b` passes the complete desktop
[run 36952131974](https://github.com/goamaan/fileform/actions/runs/36952131974) on
macOS 26 and Windows 2025, including the expanded real PDF editing/thumbnail/marker
module workflows, contextual silent-video checks, complete runtimes, notices and
Mac/NSIS packaging. Manual Windows GUI and production signing/release gates remain
open. The next breadth-first implementation is media playback/waveform/trim UI.
