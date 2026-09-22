# Portable media migration

Status: media-pack verification and bounded media inspection are available in
Rust/CLI/worker. Conversion, extraction and trimming are not yet ported. Media
operations are not yet exposed by the portable app. Preserve the original Swift implementations as parity references.

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


## Native media inspection

`fileform-native inspect-media FILE PACK_DIRECTORY` and the worker's `inspect_media`
request use the verified pack, snapshot the input, and run ffprobe outside the UI.
Inputs are bounded to 2 GiB, duration to six hours, streams to 64 and video frames
to 80 megapixels. The source identity/hash is rechecked after inspection. Reported
streams retain codec, dimensions, sample rate, channels, pixel format, transfer and
aspect-ratio metadata; attached artwork is not counted as a video track.

The process adapter captures each output pipe up to 512 KiB, limits execution to
30 seconds, closes stdin, clears inherited environment (retaining SystemRoot on
Windows), and kills/reaps the direct process on cancellation or timeout. Windows
uses CREATE_NO_WINDOW. FFprobe allows only file/pipe protocols and an explicit
MOV/Matroska/WebM/AVI/WAV/FLAC/MP3/Ogg/AAC demuxer list. No network input route is
added. Its 256 MiB max_alloc flag bounds individual allocations, not total process
memory. This is inspection metadata, not a conversion eligibility/preservation
proof. Additional original formats and richer timing/rotation metadata remain.

The adapter is private to bundled tools which do not create descendants. Windows
Job Objects/Unix process-group containment and hard worker-exit cleanup are still
release requirements; do not extend this to arbitrary commands or advertise full
OS sandboxing. Pack verification is not yet race-proof executable launch binding.

Verification: 60 Rust tests pass, including real subprocess success, failure,
output overflow, timeout and cancellation. One ignored test function is explicitly
launched by the process tests as a child fixture, not an omitted acceptance test.
Clippy, release build and Windows MSVC target check pass. Local real-pack CLI and
worker tests inspect a 2-second WAV and a 64×48, 2-second MP4 (audio + video), reject
invalid bytes and preserve source hashes. Real Windows media-pack execution is
still pending; these local fixtures do not establish Windows media parity.
