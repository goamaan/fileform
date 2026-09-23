# Release and parity status

Reviewed September 23, 2026 against the active checkout. This is a work checklist,
not release acceptance. Native capabilities below have bounded implementations
and fixture evidence; they do not imply every original option or UI flow is done.
The complete requirement IDs remain in [REQUIREMENTS.json](REQUIREMENTS.json).

## What is available where

| Area | Native engine / CLI / worker | Electron app | Still required |
| --- | --- | --- | --- |
| Tables | CSV, TSV, flat JSON conversion and verification | Picker, conversion, save/reveal | Unified intake, mixed batches, persistence and final UX checks |
| Images | PNG/JPEG/TIFF conversion, crop, resize, quality/byte fit, orientation/color checks | These limited routes are wired | Existing HEIC input parity; remaining original format/color cases; final preview/UX acceptance |
| Audio | WAV/FLAC/AAC/MP3 conversion/extraction, size fit, exact lossless trims and fast AAC trim | Not wired | Track selection, remaining trim/format policies, playback and editor integration |
| Video | Remux, SDR H.264 conversion/resize/fit, exact and keyframe-copy trim, posters | Not wired | VFR/track/extended-color gaps, playback proxies and editor integration |
| PDF assembly | Merge PDF/images, selection/order/duplicate/rotation, complete-folder split | Not wired | Marker/range/interval planning and editing UI; remaining original policies |
| PDF compression | All-page lossless rewrite with graph/geometry/text/render checks | Not wired | Targeted lossy image optimization and its preservation proofs |
| PDF images | Bounded single-page PNG preview, static annotations/widgets | Not wired | Full-resolution DPI-based PNG/JPEG page export, ordered directory output, embedded-image discovery/extraction |
| Text / OCR | Per-page and whole-document embedded text; explicit-English image/PDF OCR | Not wired | Automatic multilingual behavior, wider quality/orientation/form coverage and final UI |
| Direct URLs | No portable request route | Not wired | Existing direct-media lookup/download/trim workflow |

Native implementation and limits: [images](PORTABLE_IMAGES.md),
[media](PORTABLE_MEDIA.md), [PDF](PORTABLE_PDF.md), [OCR](OCR_RESEARCH.md).
Request inventory: `crates/fileform-engine/src/lib.rs`. Desktop exposure:
`apps/desktop/src/contracts.ts` and `apps/desktop/electron/main.cts`.
The current desktop bridge exposes table/image operations, cancellation, reveal,
Open File and appearance settings; it does not expose the media/PDF/OCR APIs yet.

## Work order

1. Finish original advertised native behavior. The next PDF work is full-resolution
   page images and embedded-image extraction, followed by targeted image compression.
   Automatic OCR language handling remains required. Retain the media/image/URL
   gaps above; do not quietly reduce them to the tested subset.
2. Connect restored capabilities to a coherent desktop workflow. Final entry is
   **one file-drop/choose surface with contextual actions**, plus action-first
   shortcuts such as Merge PDFs. The Tables/Images categories are temporary and
   explicitly rejected as the final navigation. Revisit the generated HTML's
   richer composition; keep restrained Linear/Vicinae-inspired typography/motion.
3. Complete mixed batches, task search, saved setups, workspace/recent-result
   persistence, useful progress/retry, output-folder/conflict policies, result
   opening/sharing/dragging, keyboard access and settings. Native success alone
   does not close UX01–UX13.
4. Package and test the actual release. The current Electron builder bundles the
   Rust worker and notices, **not the media/PDF/render/OCR packs**. Include reviewed
   binaries/models/licenses; keep the standalone CLI usable. Complete resource
   budgets, process-tree/hard-kill cleanup, file safety and measured performance.
5. Sign/notarize a current feature-complete Mac candidate, complete Windows signing
   and secure update delivery, and test install/update/re-download paths. Produce
   Windows artifacts and automated runtime evidence; label unavailable manual
   Windows GUI testing explicitly.
6. Finish app-wide and website copy/design QA, current screenshots and truthful
   downloads/docs/contribution links. The site needs current platform/capability
   wording before release. Public source is available; a release is not established
   merely by a public repository, preview installer or green native test suite.

## Existing foundations to preserve

- One public Apache-2.0 project with contributor/security documents and retained
  third-party notices; private histories/backups remain separate.
- Electron + React with Rust/native processing on both platforms. Tauri is not selected.
- No commerce, accounts, paid-license gates, activation, trials or device limits.
- Desktop single-instance handling, normal maximized startup (not a macOS full-screen
  Space), and light/dark/system appearance settings. Recheck all in the final UI.
- Source snapshots, no-clobber publication and cancellation checks, including
  complete split-folder publication. These do not close all isolation/race/crash gates.
- Mac signing/notarization proof exists for an earlier preview, and Windows native
  builds/runtime tests exist for specific increments. [Release preparation](ELECTRON_RELEASE.md)
  and [CI verification](CI_VERIFICATION.md) explain why neither proves the final release.

## Evidence rules

Keep native tests, UI tests, packaging, signing, installation and update evidence
separate. A passing older run proves that increment only. Current-head CI and
real app checks are still required after integration. Do not mark a requirement
complete until its full scope is demonstrated. Deferred breadth and cancelled
commerce items stay in the original ledger; this checklist does not reactivate them.
