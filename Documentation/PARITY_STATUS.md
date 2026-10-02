# Release and parity status

Reviewed October 1, 2026 against the active checkout. This is a work checklist,
not release acceptance. Native capabilities below have bounded implementations
and fixture evidence; they do not imply every original option or UI flow is done.
The complete requirement IDs remain in [REQUIREMENTS.json](REQUIREMENTS.json).

## What is available where

| Area | Native engine / CLI / worker | Electron app | Still required |
| --- | --- | --- | --- |
| Tables | CSV, TSV, flat JSON conversion and verification | Shared intake, conversion, save/open/reveal | Mixed batches, persistence and final UX checks |
| Images | PNG/JPEG/TIFF and common SDR HEIC conversion, crop, resize, quality/byte fit, orientation/color checks | Shared intake and contextual image editor are wired | Camera-specific/high-depth and remaining original format/color acceptance, explicit BT.709 policy difference; Final unified preview/UX acceptance |
| Audio | WAV/FLAC/AAC/MP3 conversion/extraction, size fit, selected audio tracks, exact WAV/FLAC/M4A trims and fast AAC trim | Conversion/fit/trim/track controls, complete audio playback/waveforms and visual range controls wired | Wider original-policy/format/GUI acceptance, history/persistence and crash/resource cleanup |
| Video | Remux, SDR H.264 conversion/resize/fit, exact and keyframe-copy trim, posters and complete normalized playback exports | Conversion/fit/trim/track/mute controls, poster/playback/waveforms and visual range controls wired | Wider playback including VFR and original-policy/format/GUI acceptance, history/persistence and crash/resource cleanup; VFR trim remains an explicit original limitation |
| PDF assembly | Merge PDF/images, selection/order/duplicate/rotation, range/interval/marker split and complete-folder publication | Thumbnails, page order/rotation/remove/duplicate/insert, undo/redo and visual marker/interval/range splits wired | Remaining original policies, complete history/source-change and large-document/GUI acceptance |
| PDF compression | All-page lossless rewrite and targeted image recompression/resize/byte fit with exact expected-graph checks | Lossless/lossy compression and byte limit wired | Wider original-policy acceptance and GUI acceptance |
| PDF images | DPI-based single/batch PNG/JPEG page export and intrinsic embedded JPEG/PNG extraction with provenance | Page/embedded exports wired | Broader embedded-image acceptance and GUI acceptance |
| Text / OCR | Per-page and whole-document embedded text; explicit-English image/PDF OCR | Embedded text and explicit-English image/PDF OCR wired | Automatic multilingual behavior, wider quality/orientation/form coverage and final UI |
| Direct URLs | No portable request route | Not wired | Existing direct-media lookup/download/trim workflow |

Native implementation and limits: [images](PORTABLE_IMAGES.md),
[media](PORTABLE_MEDIA.md), [PDF](PORTABLE_PDF.md), [OCR](OCR_RESEARCH.md).
Request inventory: `crates/fileform-engine/src/lib.rs`. Desktop exposure:
`apps/desktop/src/contracts.ts` and `apps/desktop/electron/main.cts`.
The shared bridge now exposes 13 contextual actions across these families, plus
native picker/drop intake, cancellation, save/open/reveal and appearance settings.
All action planners pass real native integration tests. Representative packaged
Mac GUI checks pass for PDF text/split and media-to-audio extraction. This does not
mean every option, drop path, original editor or Windows GUI flow is accepted.

## Work order — breadth first

The user's October 1 steering supersedes native-domain-first work. Follow
[BREADTH_FIRST_PARITY.md](BREADTH_FIRST_PARITY.md) for the actionable sequence:

1. Bundle the native foundation and establish shared file-first intake/jobs/results.
2. Connect the existing table/image/PDF/audio/video/text routes into working app flows.
3. Close broad URL/automatic OCR, batch, editing, setup and persistence gaps.
4. Run every main action end-to-end in the actual application and Windows builds.
5. Then deepen recorded domain edge cases, finish safety/performance acceptance,
   final UI/copy polish and signed download/update verification.

Preserve all native work and original scope. Do not expand PDF/HEIC corner-case
coverage while another main family has no desktop route. Safety, truthful limits
and relevant verification apply in every phase. The builder now requires a complete staged native runtime. Mac package loading
and HEIC GUI export pass; new full-runtime CI and production signing remain open.

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
