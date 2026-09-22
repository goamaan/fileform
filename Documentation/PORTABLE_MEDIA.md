# Portable media migration

Status: media-pack integrity verification is available in Rust/CLI/worker. Media
inspection, conversion, extraction and trimming are not yet exposed by the
portable app. Preserve the original Swift implementations as parity references.

## Pack verification

`fileform-native verify-media-pack DIRECTORY` checks the existing schema-1
`app.fileform.media` manifest, its declared architecture, disabled-network policy,
and SHA-256 values for both tools. Windows resolves `bin/ffmpeg.exe` and
`bin/ffprobe.exe`; macOS resolves the extensionless names. Manifest reads are
limited to 64 KiB; each nonempty executable is limited to 512 MiB and hashed in
64 KiB blocks with cancellation checks. Resolved tool paths must remain inside
the pack. Unix files must have executable permission.

The worker accepts `{"operation":"verify_media_pack","directory":"..."}` and
returns the same bounded metadata through its existing response envelope.
`supports_mp3` reports the manifest's libmp3lame declaration, not a runtime encode
probe. Architecture is also a manifest check, not executable-header inspection.

The manifest must come from the trusted distribution. Matching a mutable manifest
is not publisher authentication, process containment or protection against a pack
being replaced between verification and execution. Final executable signing must
precede manifest regeneration and enclosing app signing. Tool execution needs its
own identity/revalidation boundary, resource limits and process-tree cancellation.
The Electron preload does not expose arbitrary pack selection or tool execution.

## Next parity work

1. Build and verify equivalent no-network FFmpeg/ffprobe/LAME packs on Windows;
   retain upstream source archives, licenses, pinned inputs and build recipes.
2. Add bounded native process execution, timeouts and cancellation on both systems.
3. Port inspection/stream routing and conversion/audio extraction, then MP3, trim,
   size/resolution controls, previews and output validation from the Swift reference.
4. Verify actual output streams/duration/content, no-clobber publication, source
   preservation, cancellation cleanup and malformed inputs through CLI and worker.
5. Integrate into the final unified file-first/action-first UI, following the
   user's parity-first priority; do not add a permanent Media workspace.

September 22 evidence: 57 Rust tests and Clippy passed; Windows MSVC target check
passed. The real Mac `9.0.1-fileform.2` pack passed through both CLI and worker,
including matching hashes for FFmpeg and ffprobe. This is not a Windows tool-pack
build or media transformation acceptance. Native Windows CI runs the platform
filename/tampering tests using disposable fixture bytes, not FFmpeg executables.
