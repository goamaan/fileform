# Portable media migration

Status: media-pack verification and bounded media inspection are available in
Rust/CLI/worker. WAV/FLAC/M4A/MP3 audio conversion and extraction are implemented;
video conversion and trimming are not yet ported. Media
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


## Audio conversion and extraction

`fileform-native convert-audio INPUT OUTPUT.{wav,flac,m4a,mp3} PACK_DIRECTORY`
and the `convert_audio` worker request convert one audio track or extract it from
video. Requests may include `expected_source_sha256` to reject stale selections.
WAV uses 16-bit PCM; FLAC uses its native encoder; M4A/AAC and MP3 use 128 kb/s.
Descriptive metadata and chapters are removed. WAV's explicit 16-bit format may
reduce source precision; `lossy_codec` describes the encoder, not a guarantee of
sample-identical conversion for every source. FLAC rejects floating-point PCM or
bit depth above 24. MP3 accepts only mono/stereo at 32, 44.1 or 48 kHz. Output
channel count and sample rate must match; implicit resampling/downmixing is not
accepted. Multiple audio tracks require a future explicit selection workflow.

Encoding uses a private staging directory, no-clobber publication, a 512 MiB
output limit and a duration-scaled timeout. The output is inspected for codec,
container, tracks, duration (0.25-second tolerance), channels and sample rate, then
fully decoded with error detection before publication. Source hash/identity and
output-directory identity are checked again, and the staged file is synced.
Cooperative cancellation removes staging; forced worker-exit cleanup and hostile
filesystem races remain open hardening requirements. No UI route is added yet.

Run `python3 crates/fileform-engine/tests/smoke-media.py PACK_DIRECTORY` after a
release workspace build for reproducible real-tool checks. The Mac pack passed
all four outputs, exact decoded PCM for the 16-bit WAV/FLAC fixture, exact decoded
video-to-WAV extraction, existing-output preservation, stale-source rejection,
cancellation cleanup and unsupported 32-bit FLAC rejection. The 61 active Rust
tests, Clippy, release build and Windows target check passed. The real-tool script
is not yet run in Windows CI because the Windows media pack is still pending.

## Windows pack build

`crates/fileform-engine/tools/build-media-windows.sh` builds the pinned FFmpeg
9.0.1 and LAME 3.100 sources in MSYS2 UCRT64, verifies the FFmpeg source signature
and both source hashes, disables networking and optional auto-detected libraries,
and requests static linking. It retains source archives, licenses, flags, compiler
and package versions. An imported-DLL check rejects non-system dependencies.
The build tool packages are recorded but not yet frozen in a reproducible MSYS2
snapshot; bit-for-bit reproducibility is not claimed.

`.github/workflows/media-windows.yml` builds the pack on Windows 2025, builds the
Rust CLI/worker with static CRT, and runs the real-tool audio smoke suite. It keeps
pack and diagnostic artifacts for 14 days; this is not signed public distribution.
Initial workflow execution is pending as of this implementation commit. Do not
mark Windows media parity achieved until the job and real audio smoke pass.

The build follows [FFmpeg Windows guidance](https://www.ffmpeg.org/platform.html)
and the pinned [MSYS2 setup action](https://github.com/msys2/setup-msys2/tree/66cd2cce69caa17b53920067426061ca1de3a884).
Windows video-encoder selection and media packaging into Electron remain separate
parity work; this job exercises existing WAV/FLAC/M4A/MP3 conversion and extraction.

### Output-size completion safeguard

Audio encoding no longer uses FFmpeg's `-fs` option: it can truncate an encode
while reporting success. The process runner checks the staged file size while the
encoder runs and after exit, and kills/reaps the encoder when the size is exceeded.
The subsequent digest also enforces the publication limit. Temporary disk use can
briefly exceed the limit between checks; no oversized or limit-truncated result is
published. A real subprocess regression test covers a successful child that wrote
an oversized file.

The real-tool smoke suite additionally verifies exact 24-bit FLAC samples and
rejects floating-point FLAC input, unsupported MP3 sample-rate conversion and
multiple audio tracks without publishing output. All checks passed on Mac; 62
active Rust tests, Clippy and Windows target checking passed. Windows media job
35776416445 is currently building its source-verified tool pack, not yet accepted.

## Exact audio sample trimming

`fileform-native trim-audio INPUT OUTPUT.{wav,flac} PACK_DIRECTORY START_SAMPLE END_SAMPLE`
uses a zero-based, half-open interval of **decoded audio samples**. The worker
accepts `trim_audio` with `samples: {start, end}` and optional
`expected_source_sha256`. These boundaries are not source-clock timestamps;
source-time mapping, delayed/discontinuous tracks and fast packet-copy trimming
remain separate parity work. Do not present this as the complete original trim UI.

The route reuses conversion safety and format policies, applies atrim's sample
indices, resets output timestamps, and verifies exact output duration ticks against
the sample rate. It then hashes decoded PCM for the selected source interval and
the whole output at the output's precision; the hashes must match before saving.
Only WAV and FLAC are offered for this exact-sample route. WAV retains its explicit
16-bit output policy. Source ranges beyond the decoded file fail verification and
never publish a shortened result. Empty/reversed ranges fail validation.

The real-tool smoke suite passed 12,345–54,321 sample WAV/FLAC cuts, a one-sample
WAV cut, a 24-bit FLAC cut and invalid/end-beyond-source cases. Independent decoded
PCM slices match exactly. All existing audio conversion checks also passed, along
with 62 active Rust tests, Clippy, release build and Windows target checking.
Windows tool-pack runtime validation remains pending in the live CI build.

## Video stream-copy conversion

`fileform-native remux-video INPUT OUTPUT.{mp4,mov} PACK_DIRECTORY` and worker
`remux_video` copy compatible streams without encoding them again. The current
route accepts one 8-bit yuv420p SDR H.264 video track and at most one AAC audio
track, with no additional streams. Other video formats require the still-pending
transcoding route. Metadata/chapters are removed; tested display rotation survives.

Runtime verification compares dimensions, color fields, pixel aspect ratio,
rotation, duration, audio layout and relative audio/video start alignment, then
fully decodes and hashes picture/audio content on both sides. Output size is
monitored at 2 GiB; source identity/hash and directory identity are rechecked before
synced no-clobber publication. This inherits the documented direct-process and
filesystem-race limitations. It is not a formal proof of every packet timestamp
for arbitrary input; broader timing fixtures and source-clock editing remain open.

The real-tool smoke suite proves identical packet hashes/PTS/durations for the
included generated H.264/AAC fixture through MP4→MOV→MP4. Silent and 90-degree
rotated variants pass; unsupported MPEG-4 video is rejected without output.
Mac CLI/worker checks, 62 active Rust tests, Clippy, release build and Windows target
checking passed. The generated fixture is checked in with provenance so Windows
CI can exercise this route without requiring a video encoder. Windows execution
is not yet established while the tool-pack build remains active.

## Desktop supervisor lifetime

Electron submits `{request: <engine request>, cancel_on_disconnect: true}` and
keeps the worker's stdin open. The worker treats control-pipe EOF, read failure or
an invalid control message as cancellation for these supervised jobs. A normal
`cancel\n` also cancels. Bare engine requests preserve the previous one-shot CLI
semantics: EOF alone does not cancel them. No arbitrary IPC event or shell API is
exposed to the renderer.

The process smoke suite now closes a real supervisor pipe after output staging
starts and checks cancellation, source preservation, snapshot cleanup and absence
of published/staged output. It also proves that a connected supervisor completes
normally. These checks run in both existing desktop CI jobs. This closes an
unexpected-Electron-exit gap while the worker remains alive; it does not establish
cleanup after forcibly killing the worker itself or full process-tree containment.

The Windows recipe also copies installed MSYS2 license files into package-labelled
folders and retains full package metadata. This includes build-tool notices as
well as runtime notices, so the inventory is deliberately broader than the linked
binary set. Final distribution still requires auditing the resulting imports,
static dependencies, source/relinking materials and exact generated pack; copying
license files alone is not a completed release compliance review. The notice
collection step awaits execution in the next Windows pack build.

Windows CI caches only an exact pack match keyed by the build-recipe hash and a
fingerprint of installed MSYS2 package versions. No fallback/prefix cache is used.
A restored pack is still verified by the current Rust CLI and all current real
media tests run; test results are never cached. The cache is saved only after a
successful job. Changes to the source pins, flags, notices recipe or installed
toolchain invalidate it. A cache miss remains a supported clean source build.
The workflow also retains the JSON smoke summary with its pack artifacts.
