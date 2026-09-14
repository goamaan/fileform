# Contributing to Fileform

Fileform is a free, open-source file-transformation app. Help is welcome with
reproducible bugs, platform testing, documentation, accessibility and code.

Read the README and `Documentation/CROSS_PLATFORM.md` first. The Electron/Rust
migration is in progress; the Swift implementation is the behavior reference.
Do not remove an existing capability or call a disabled route finished.

## Development

The root Rust toolchain and npm lockfiles pin the portable build. See
`apps/desktop/README.md` for desktop commands and `apps/macos/README.md` for the
native reference. Shared transformation logic belongs in the engine, not the UI.

For a change, run relevant checks:

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --release --workspace --locked
python3 Tools/smoke-portable.py
npm ci --prefix apps/desktop
npm run build --prefix apps/desktop
```

Swift changes also need their relevant Swift/native tests. UI changes need a real
app check. Describe the tested operating system and architecture; building a
Windows target is not the same as testing its installed interface.

## Pull requests

Keep a change focused. Explain the problem, resulting behavior and verification.
Add regression coverage for meaningful behavior changes. Preserve originals,
no-clobber publication, bounded resources and cancellation. Use tiny generated or
permitted test fixtures; never submit private documents or credentials.

Original contributions are under Apache-2.0, the project's license. Keep existing
copyright/attribution notices and identify third-party code or dependencies.
No contributor agreement or payment is required.

Discuss broad new features in an issue before a large implementation. During the
migration, working release behavior and platform parity take priority over breadth.
