# Fileform

Free, open-source file tools for your desktop.

Convert images, audio, video and tables; organize and compress PDFs; crop images,
trim recordings and extract text. Ordinary conversions run locally and create new
outputs. No account, trial, activation or payment is required.

## Status

The Swift engine/CLI and native macOS application are working reference
implementations. They are being consolidated here with the website and a new
macOS/Windows desktop. The first Electron/Rust preview now performs real CSV/TSV-to-JSON conversion.
The other workflows still use the reference implementation; Windows installer
verification and full migration parity remain pending. See the
[cross-platform plan](Documentation/CROSS_PLATFORM.md) and
[user requirements](Documentation/USER_REQUIREMENTS.md).

Current Mac build target: macOS 14 or later. The complete tested runtime is arm64;
A Windows x64 preview installer now builds in CI and its native worker checks pass.
Full Windows workflow/UI acceptance and Intel Mac release support remain open.
Do not assume every input can convert to every format. The CLI capability command
and operation documentation describe the implemented routes and limits.

## Repository

- `Sources/`, `Tests/`, `Package.swift`: working Swift engine and CLI.
- `apps/macos/`: native SwiftUI reference app, using the root package.
- `apps/desktop/`: Electron + React migration preview.
- `crates/`: portable Rust engine, worker and CLI.
- `website/`: project website; free-project redesign is in progress.
- `Documentation/`: contracts, architecture, dependencies and migration plan.
- `Tools/`: pinned native pack builds, CLI checks and packaging.

## Build the current engine and app

On a Mac with the required Xcode/Swift toolchain:

```sh
swift build
swift test --no-parallel
.build/debug/fileform capabilities
```

Media/PDF tools use the reviewed packs built by `Tools/build-media-pack.sh` and
`Tools/build-pdf-pack.sh`. Their source, flags, notices and hashes are retained.
For the app, install XcodeGen, build both packs, then run:

```sh
apps/macos/Tools/build-development.sh
```

See [dependencies](Documentation/Dependencies.md) and the individual operation
contracts before changing supported behavior. The Windows build and worker/CLI checks pass in CI; native Windows interface
acceptance and full workflow parity remain open.

## License

Original Fileform source and assets use [Apache-2.0](LICENSE). Bundled fonts,
codecs and other components retain their own licenses and notices. Signing keys,
credentials and private development archives are never part of this repository.
