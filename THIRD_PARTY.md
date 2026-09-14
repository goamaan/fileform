# Third-party components

Original Fileform source and assets use Apache-2.0. That does not relicense third-
party code, fonts, native engines or operating-system components.

- Website UI components derived from shadcn/ui retain its MIT notice in
  `ThirdPartyNotices/shadcn-ui.txt`.
- Bundled fonts retain the OFL notices beside their files.
- Swift, Rust and npm dependencies are pinned in their manifests/lockfiles and
  retain their upstream licenses. Release bundles must include their exact notices.
- FFmpeg/LAME and qpdf/libjpeg-turbo packs retain their own source, build flags,
  license texts and hashes. See `Documentation/Dependencies.md` and pack manifests.
- Electron/Chromium notices and Rust dependency notices must accompany desktop
  releases. The current internal preview is not the completed release-notice audit.

Do not add native binaries from a developer's PATH or silently change codec build
flags. Record source provenance, licenses, transitive dependencies and reproducible
build steps before adding a distributable engine.
