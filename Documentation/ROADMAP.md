# Roadmap

Fileform is migrating to Electron + a native Rust worker for macOS and Windows.
The Swift app/engine remain the working reference. This roadmap is not a promise
that every listed feature is available or a commitment to release dates.

The immediate priority is verified existing behavior, platform parity, public
source/downloads, secure updates and a concise interface. The portable app supports CSV/TSV/flat JSON conversion and bounded PNG/JPEG/TIFF
image conversion, crop, resize and size fitting. It does not yet replace the
complete reference app. See PORTABLE_IMAGES.md for preservation limits.

Detailed scope is retained in [REQUIREMENTS.json](REQUIREMENTS.json). Historical
reference statuses are not portable acceptance. Each completed row needs actual
source/build/content/UI evidence appropriate to its scope.

## Release and parity

- **UX01** — File import through picker, drag/drop, menu and keyboard.
- **UX02** — Task search, navigation and compact layouts.
- **UX03** — Single-file and mixed batch workflows.
- **UX04** — Saved setups and reusable results.
- **UX05** — Local workspace and recent-result persistence.
- **UX06** — Progress, cancellation, errors and retry.
- **UX07** — Open, reveal, share and drag saved results.
- **UX08** — Output folders and filename conflicts.
- **UX09** — Actionable errors and protected-document handling.
- **UX10** — Keyboard access, appearances and accessible visuals.
- **UX11** — Useful application settings.
- **UX12** — Opt-in privacy-reviewed diagnostics.
- **UX13** — PDF and media visual editing.
- **IMG01** — Image conversion, including existing PNG/JPEG and HEIC input routes.
- **IMG02** — Image crop and resize.
- **IMG03** — Verified size limits and quality controls.
- **IMG04** — Image color profile and metadata policies.
- **MED01** — Audio/video conversion and audio extraction.
- **MED02** — MP3 conversion.
- **MED03** — Exact and fast media trimming.
- **MED04** — Audio/video task routing.
- **MED05** — Media size limits and resolution controls.
- **DATA01** — CSV, TSV and flat JSON conversion.
- **DATA02** — Local text extraction and scan recognition.
- **PDF01** — Combine PDFs and images.
- **PDF02** — Organize, rotate, remove and insert pages.
- **PDF03** — Split PDFs by markers, intervals and ranges.
- **PDF04** — Page image export and embedded-image extraction.
- **PDF05** — Lossless and image-based PDF compression.
- **LINK01** — Direct media URL lookup, save and trim.
- **WEB01** — Minimal free-project website.
- **WEB03** — Accurate platform status, privacy and license information.
- **REL01** — Signed application delivery and secure updates.
- **REL02** — Independent free CLI and reproducible engine packs.
- **WIN01** — Real Windows executable and installer builds.
- **WIN02** — Windows runtime and UI verification, with manual gaps recorded.
- **OSS01** — One reviewed public repository and contributor/security documentation.
- **SEC01** — Native worker isolation and cross-platform file-safety hardening.
- **PERF01** — Measured startup, memory and transformation performance.

## Deferred breadth

- **IMG05** — Additional WebP, HEIC, AVIF and TIFF writing.
- **IMG06** — Animated image and video/image workflows.
- **MED06** — Broader media editing and joining.
- **DATA03** — Publishing and document formats.
- **DATA04** — Office format conversion.
- **DATA05** — PDF to editable Office formats.
- **PDF06** — PDF password protection and unlocking.
- **PDF07** — Searchable-PDF OCR.
- **PDF08** — PDF annotation, editing and crop.
- **PDF09** — PDF forms.
- **PDF10** — PDF signatures.
- **PDF11** — PDF redaction.
- **PDF12** — Document comparison.
- **PDF13** — Repair and PDF/A workflows.
- **PDF14** — HTML to PDF.
- **LINK02** — Broader media-service downloads.
- **AI01** — Optional transcripts and subtitles.
- **AI02** — Optional structured scan extraction.
- **AI03** — Optional translation and document intelligence.
- **AI04** — Optional image transformations.
- **WEB02** — Optional community release notifications.

Payments, accounts, activation, trials, device limits and paid upgrades are cancelled.
Optional provider work must remain explicit; local conversion cannot silently use
a cloud service. Manual Windows testing may be recorded as pending when a Windows
machine is unavailable, but a real Windows build is still required.
