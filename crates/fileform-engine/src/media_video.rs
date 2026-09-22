// SPDX-License-Identifier: Apache-2.0
use crate::{digest, fail, media_pack, media_probe, native_process, Cancellation, Result, Source};
use serde::Serialize;
use std::{path::Path, process::Command, time::Duration};

const MAX_OUTPUT: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Debug, Serialize)]
pub struct VideoReceipt {
    pub output: std::path::PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub audio_tracks: usize,
    pub stream_copy: bool,
}
fn command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command.env_clear();
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    command.args([
        "-hide_banner",
        "-nostdin",
        "-v",
        "error",
        "-nostats",
        "-xerror",
        "-max_alloc",
        "268435456",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        "mov,matroska,webm,avi",
        "-threads",
        "2",
    ]);
    command
}
fn video(info: &media_probe::MediaInspection) -> Result<&media_probe::Stream> {
    if info.video_tracks != 1
        || info.audio_tracks > 1
        || info.streams.len() != 1 + info.audio_tracks
    {
        return Err(fail(
            "unsupported",
            "Choose one video track with at most one audio track and no extra streams.",
        ));
    }
    let video = info
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .ok_or_else(|| fail("unsupported", "No video track was found."))?;
    if video.codec_name.as_deref() != Some("h264")
        || video.pix_fmt.as_deref() != Some("yuv420p")
        || matches!(
            video.color_transfer.as_deref(),
            Some("smpte2084" | "arib-std-b67")
        )
    {
        return Err(fail(
            "unsupported",
            "Stream-copy conversion currently accepts 8-bit SDR H.264 video.",
        ));
    }
    if info
        .streams
        .iter()
        .any(|s| s.codec_type == "audio" && s.codec_name.as_deref() != Some("aac"))
    {
        return Err(fail(
            "unsupported",
            "Stream-copy conversion requires AAC audio. Audio transcoding is a separate route.",
        ));
    }
    Ok(video)
}
fn start(stream: &media_probe::Stream) -> Result<f64> {
    stream
        .start_time
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .ok()
        .filter(|x| x.is_finite())
        .ok_or_else(|| fail("unsupported", "Unsupported media start time."))
}
pub fn remux(
    input: &Path,
    output: &Path,
    directory: &Path,
    expected: Option<&str>,
    cancellation: &Cancellation,
) -> Result<VideoReceipt> {
    let muxer = match output
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4") => "mp4",
        Some("mov") => "mov",
        _ => return Err(fail("invalid_request", "Choose MP4 or MOV output.")),
    };
    media_pack::verify(directory, cancellation)?;
    if output.try_exists()? {
        return Err(fail(
            "collision",
            "The output already exists. Choose another name.",
        ));
    }
    let mut source = Source::open_with_limit(input, cancellation.clone(), MAX_OUTPUT)?;
    if expected.is_some_and(|hash| hash != source.hash) {
        return Err(fail("source_changed", "The inspected source changed."));
    }
    let before = media_probe::inspect(source.snapshot.path(), directory, cancellation)?;
    let original = video(&before)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent_id = same_file::Handle::from_path(parent)?;
    let staging = tempfile::tempdir_in(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(staging.path())?;
    let executable = directory.canonicalize()?.join(if cfg!(windows) {
        "bin/ffmpeg.exe"
    } else {
        "bin/ffmpeg"
    });
    let deadline =
        Duration::from_secs_f64((before.duration_seconds * 4.0 + 60.0).clamp(120.0, 43200.0));
    let mut copy = command(&executable);
    copy.arg("-i")
        .arg(source.snapshot.path())
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            "-f",
            muxer,
            "-y",
        ])
        .arg(temporary.path());
    native_process::run_with_output_limit(
        copy,
        cancellation,
        deadline,
        512 * 1024,
        Some((temporary.path(), MAX_OUTPUT)),
    )?;
    let after = media_probe::inspect(temporary.path(), directory, cancellation)?;
    let copied = video(&after)?;
    if !after.format.split(',').any(|x| x == "mov")
        || before.audio_tracks != after.audio_tracks
        || (before.duration_seconds - after.duration_seconds).abs() > 0.25
        || original.width != copied.width
        || original.height != copied.height
        || original.color_transfer != copied.color_transfer
        || original.color_primaries != copied.color_primaries
        || original.color_space != copied.color_space
        || original.color_range != copied.color_range
        || original.sample_aspect_ratio != copied.sample_aspect_ratio
        || original.side_data_list != copied.side_data_list
    {
        return Err(fail(
            "verification",
            "Video dimensions, color, orientation or duration changed.",
        ));
    }
    if let Some(audio) = before.streams.iter().find(|s| s.codec_type == "audio") {
        let result = after
            .streams
            .iter()
            .find(|s| s.codec_type == "audio")
            .ok_or_else(|| fail("verification", "Output audio is missing."))?;
        if audio.channels != result.channels
            || audio.sample_rate != result.sample_rate
            || ((start(audio)? - start(original)?) - (start(result)? - start(copied)?)).abs()
                > 0.001
        {
            return Err(fail(
                "verification",
                "Audio layout or audio/video start alignment changed.",
            ));
        }
    }
    for track in if before.audio_tracks == 1 {
        vec!["0:v:0", "0:a:0"]
    } else {
        vec!["0:v:0"]
    } {
        let hash = |path: &Path| -> Result<Vec<u8>> {
            let mut decode = command(&executable);
            decode
                .args(["-err_detect", "explode", "-noautorotate"])
                .arg("-i")
                .arg(path)
                .args(["-map", track]);
            if track == "0:v:0" {
                decode.args([
                    "-c:v",
                    "rawvideo",
                    "-pix_fmt",
                    "yuv420p",
                    "-fps_mode",
                    "passthrough",
                ]);
            } else {
                decode.args(["-c:a", "pcm_s32le"]);
            }
            decode.args(["-threads", "2", "-f", "hash", "-hash", "sha256", "-"]);
            native_process::run(decode, cancellation, deadline, 4096)
        };
        let original_hash = hash(source.snapshot.path())?;
        if !original_hash.starts_with(b"SHA256=") || original_hash != hash(temporary.path())? {
            return Err(fail(
                "verification",
                "Decoded video or audio content changed.",
            ));
        }
    }
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancellation, MAX_OUTPUT)?;
    source.check(input)?;
    if parent_id != same_file::Handle::from_path(parent)? {
        return Err(fail("output_changed", "The output folder changed."));
    }
    temporary.as_file().sync_all()?;
    cancellation.check()?;
    temporary.persist_noclobber(output).map_err(|e| {
        fail(
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                "collision"
            } else {
                "io"
            },
            e.error.to_string(),
        )
    })?;
    Ok(VideoReceipt {
        output: output.to_path_buf(),
        bytes,
        sha256,
        duration_seconds: after.duration_seconds,
        width: copied.width.unwrap(),
        height: copied.height.unwrap(),
        audio_tracks: after.audio_tracks,
        stream_copy: true,
    })
}
