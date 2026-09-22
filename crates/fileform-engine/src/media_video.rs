// SPDX-License-Identifier: Apache-2.0
use crate::{digest, fail, media_pack, media_probe, native_process, Cancellation, Result, Source};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, time::Duration};

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoEncoding {
    pub max_dimension: Option<u32>,
    pub bitrate: Option<u32>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoFit {
    pub max_bytes: u64,
    pub minimum_bitrate: Option<u32>,
    pub max_dimension: Option<u32>,
}
fn fit_rates(floor: u32) -> Result<Vec<u32>> {
    if !(50000..=100000000).contains(&floor) {
        return Err(fail(
            "invalid_request",
            "Video bitrate floor must be from 50 kb/s to 100 Mb/s.",
        ));
    }
    let mut rates: Vec<u32> = [2000000, 1500000, 1000000, 750000, 500000, 300000, 150000]
        .into_iter()
        .filter(|rate| *rate > floor)
        .collect();
    rates.push(floor);
    Ok(rates)
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct VideoSelection {
    pub start_frame: u32,
    pub end_frame: u32,
    pub audio_samples: Option<crate::SampleRange>,
    pub duration_seconds: f64,
}
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct VideoOperation {
    pub encoding: Option<VideoEncoding>,
    pub encode_audio: bool,
    pub mute_audio: bool,
    pub selection: Option<VideoSelection>,
}
fn dimensions(width: u32, height: u32, rotation: i32, maximum: Option<u32>) -> Result<(u32, u32)> {
    if rotation % 90 != 0 || width < 2 || height < 2 {
        return Err(fail(
            "unsupported",
            "Unsupported picture dimensions or rotation.",
        ));
    }
    let (width, height) = if rotation % 180 != 0 {
        (height, width)
    } else {
        (width, height)
    };
    if maximum.is_none() && (width % 2 != 0 || height % 2 != 0) {
        return Err(fail(
            "invalid_request",
            "H.264 needs even dimensions. Choose an explicit resize limit.",
        ));
    }
    if maximum.is_some_and(|n| n < 2) {
        return Err(fail(
            "invalid_request",
            "Video resize limit must be at least two pixels.",
        ));
    }
    let longest = width.max(height);
    let bound = maximum.unwrap_or(longest).min(longest);
    Ok((
        ((u64::from(width) * u64::from(bound) / u64::from(longest)) as u32 / 2 * 2).max(2),
        ((u64::from(height) * u64::from(bound) / u64::from(longest)) as u32 / 2 * 2).max(2),
    ))
}
fn rotation(video: &media_probe::Stream) -> i32 {
    video
        .side_data_list
        .as_ref()
        .and_then(|items| items.iter().find_map(|v| v.rotation))
        .unwrap_or(0)
}
pub(crate) const MAX_OUTPUT: u64 = 2 * 1024 * 1024 * 1024;
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
    pub requested_bitrate: Option<u32>,
    pub attempts: u32,
}
pub(crate) fn command(executable: &Path) -> Command {
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
pub(crate) fn video(
    info: &media_probe::MediaInspection,
    stream_copy: bool,
) -> Result<&media_probe::Stream> {
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
    if (stream_copy
        && (video.codec_name.as_deref() != Some("h264")
            || video.pix_fmt.as_deref() != Some("yuv420p")))
        || !matches!(
            video.pix_fmt.as_deref(),
            Some(
                "yuv420p"
                    | "yuv422p"
                    | "yuv444p"
                    | "yuvj420p"
                    | "yuvj422p"
                    | "yuvj444p"
                    | "nv12"
                    | "gray"
            )
        )
        || matches!(
            video.color_transfer.as_deref(),
            Some("smpte2084" | "arib-std-b67")
        )
    {
        return Err(fail(
            "unsupported",
            "This video route requires supported 8-bit SDR video without transparency.",
        ));
    }
    if stream_copy
        && info
            .streams
            .iter()
            .any(|s| s.codec_type == "audio" && s.codec_name.as_deref() != Some("aac"))
    {
        return Err(fail(
            "unsupported",
            "Stream-copy conversion requires AAC audio.",
        ));
    }
    Ok(video)
}
pub(crate) fn trim_picture(info: &media_probe::MediaInspection) -> Result<&media_probe::Stream> {
    let picture = video(info, false)?;
    if !picture
        .width
        .is_some_and(|n| (2..=8192).contains(&n) && n % 2 == 0)
        || !picture
            .height
            .is_some_and(|n| (2..=8192).contains(&n) && n % 2 == 0)
        || picture
            .sample_aspect_ratio
            .as_deref()
            .is_some_and(|v| !matches!(v, "1:1" | "0:1" | "N/A"))
        || rotation(picture) % 90 != 0
    {
        return Err(fail("unsupported","Trim requires even-sized, square-pixel video up to 8192 pixels per edge and right-angle rotation."));
    }
    Ok(picture)
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
pub fn transform(
    input: &Path,
    output: &Path,
    directory: &Path,
    expected: Option<&str>,
    operation: VideoOperation,
    cancellation: &Cancellation,
) -> Result<VideoReceipt> {
    let encoding = operation.encoding;
    let encode_audio = operation.encode_audio;
    if operation.selection.is_some_and(|s| {
        s.start_frame >= s.end_frame
            || !s.duration_seconds.is_finite()
            || s.duration_seconds <= 0.0
            || s.duration_seconds > 21600.0
            || encoding.is_none()
    }) {
        return Err(fail("invalid_request", "Invalid verified video selection."));
    }
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
    let original = video(&before, encoding.is_none())?;
    let expected_dimensions = if let Some(options) = encoding {
        if original
            .sample_aspect_ratio
            .as_deref()
            .is_some_and(|v| !matches!(v, "1:1" | "0:1" | "N/A"))
        {
            return Err(fail(
                "unsupported",
                "Non-square pixels require an explicit aspect-ratio workflow.",
            ));
        }
        if options
            .bitrate
            .is_some_and(|v| !(50000..=100000000).contains(&v))
        {
            return Err(fail(
                "invalid_request",
                "Video bitrate must be between 50 kb/s and 100 Mb/s.",
            ));
        }
        dimensions(
            original.width.unwrap(),
            original.height.unwrap(),
            rotation(original),
            options.max_dimension,
        )?
    } else {
        (original.width.unwrap(), original.height.unwrap())
    };
    let padding = if cfg!(windows) && encoding.is_some() {
        crate::media_video_padding::Padding::required(expected_dimensions.0, expected_dimensions.1)
    } else {
        None
    };
    let expected_audio_tracks = if operation.mute_audio {
        0
    } else {
        before.audio_tracks
    };
    let expected_duration = operation
        .selection
        .map_or(before.duration_seconds, |s| s.duration_seconds);
    let copy_audio = !encode_audio
        && before
            .streams
            .iter()
            .filter(|s| s.codec_type == "audio")
            .all(|s| s.codec_name.as_deref() == Some("aac"));
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
    copy.arg("-i").arg(source.snapshot.path()).args([
        "-map",
        "0:v:0",
        "-map_metadata",
        "-1",
        "-map_chapters",
        "-1",
    ]);
    if operation.mute_audio {
        copy.arg("-an");
    } else {
        copy.args(["-map", "0:a:0?"]);
    }
    if let Some(samples) = operation.selection.and_then(|s| s.audio_samples) {
        copy.args([
            "-af",
            &format!(
                "atrim=start_sample={}:end_sample={},asetpts=PTS-STARTPTS",
                samples.start, samples.end
            ),
        ]);
    }
    if let Some(options) = encoding {
        let encoder = if cfg!(target_os = "macos") {
            "h264_videotoolbox"
        } else if cfg!(windows) {
            "h264_mf"
        } else {
            return Err(fail(
                "unsupported",
                "Video encoding is implemented for macOS and Windows.",
            ));
        };
        let padding_filter = padding
            .map(|p| format!(",pad={}:{}:0:0", p.encoded_width, p.encoded_height))
            .unwrap_or_default();
        copy.args([
            "-vf",
            &format!(
                "{}scale={}:{},setsar=1{}",
                operation
                    .selection
                    .map(|s| format!(
                        "trim=start_frame={}:end_frame={},setpts=PTS-STARTPTS,",
                        s.start_frame, s.end_frame
                    ))
                    .unwrap_or_default(),
                expected_dimensions.0,
                expected_dimensions.1,
                padding_filter
            ),
            "-c:v",
            encoder,
        ]);
        if cfg!(target_os = "macos") {
            copy.args(["-allow_sw", "1"]);
        } else {
            copy.args(["-hw_encoding", "0"]);
        }
        copy.args([
            "-b:v",
            &options.bitrate.unwrap_or(2000000).to_string(),
            "-pix_fmt",
            "yuv420p",
            "-fps_mode",
            "passthrough",
            "-filter_threads",
            "2",
            "-threads",
            "2",
            "-c:a",
            if copy_audio { "copy" } else { "aac" },
        ]);
        if !copy_audio {
            copy.args(["-b:a", "128000"]);
        }
    } else {
        copy.args(["-c", "copy"]);
    }
    copy.args(["-movflags", "+faststart", "-f", muxer, "-y"])
        .arg(temporary.path());
    native_process::run_with_output_limit(
        copy,
        cancellation,
        deadline,
        512 * 1024,
        Some((temporary.path(), MAX_OUTPUT)),
    )?;
    if let Some(padding) = padding {
        temporary = crate::media_video_padding::restore(
            temporary,
            directory,
            muxer,
            padding,
            cancellation,
            deadline,
        )?;
    }
    let after = media_probe::inspect(temporary.path(), directory, cancellation)?;
    let copied = video(&after, true)?;
    if !after.format.split(',').any(|x| x == "mov")
        || expected_audio_tracks != after.audio_tracks
        || (expected_duration - after.duration_seconds).abs() > 0.25
        || Some(expected_dimensions.0) != copied.width
        || Some(expected_dimensions.1) != copied.height
        || (encoding.is_none()
            && (original.color_transfer != copied.color_transfer
                || original.color_primaries != copied.color_primaries
                || original.color_space != copied.color_space
                || original.color_range != copied.color_range
                || original.sample_aspect_ratio != copied.sample_aspect_ratio
                || original.side_data_list != copied.side_data_list))
        || (encoding.is_some()
            && (rotation(copied) != 0
                || copied.pix_fmt.as_deref() != Some("yuv420p")
                || copied.sample_aspect_ratio.as_deref() != Some("1:1")))
    {
        return Err(fail(
            "verification",
            "Video dimensions, color, orientation or duration changed.",
        ));
    }
    if encoding.is_some() {
        if (original.color_transfer.is_some() && original.color_transfer != copied.color_transfer)
            || (original.color_primaries.is_some()
                && original.color_primaries != copied.color_primaries)
            || (original.color_space.is_some() && original.color_space != copied.color_space)
        {
            return Err(fail(
                "verification",
                "Known video color interpretation changed.",
            ));
        }
        if let Some(count) = operation
            .selection
            .map(|s| u64::from(s.end_frame - s.start_frame))
            .or_else(|| {
                original
                    .nb_frames
                    .as_deref()
                    .and_then(|v| v.parse::<u64>().ok())
            })
        {
            if copied
                .nb_frames
                .as_deref()
                .and_then(|v| v.parse::<u64>().ok())
                != Some(count)
            {
                return Err(fail(
                    "verification",
                    "The encoded video frame count changed.",
                ));
            }
        }
    }
    if let Some(audio) = before
        .streams
        .iter()
        .find(|s| s.codec_type == "audio" && !operation.mute_audio)
    {
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
    for track in if expected_audio_tracks == 1 {
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
        let output_hash = hash(temporary.path())?;
        let preserve = encoding.is_none() || (track == "0:a:0" && copy_audio);
        if !output_hash.starts_with(b"SHA256=")
            || (preserve && output_hash != hash(source.snapshot.path())?)
        {
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
        stream_copy: encoding.is_none(),
        requested_bitrate: encoding.map(|options| options.bitrate.unwrap_or(2000000)),
        attempts: 1,
    })
}

pub fn fit(
    input: &Path,
    output: &Path,
    directory: &Path,
    options: VideoFit,
    expected: Option<&str>,
    cancellation: &Cancellation,
) -> Result<VideoReceipt> {
    if options.max_bytes == 0 || options.max_bytes > MAX_OUTPUT {
        return Err(fail(
            "invalid_request",
            "Choose a video limit from 1 byte to 2 GiB.",
        ));
    }
    let extension = output
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| fail("invalid_request", "Choose MP4 or MOV output."))?;
    if !matches!(extension.to_ascii_lowercase().as_str(), "mp4" | "mov") {
        return Err(fail("invalid_request", "Choose MP4 or MOV output."));
    }
    let rates = fit_rates(options.minimum_bitrate.unwrap_or(150000))?;
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
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let staging = tempfile::tempdir_in(parent)?;
    for (index, rate) in rates.into_iter().enumerate() {
        cancellation.check()?;
        let candidate = staging.path().join(format!("candidate.{extension}"));
        let mut result = transform(
            source.snapshot.path(),
            &candidate,
            directory,
            Some(&source.hash),
            VideoOperation {
                encoding: Some(VideoEncoding {
                    max_dimension: options.max_dimension,
                    bitrate: Some(rate),
                }),
                encode_audio: true,
                ..Default::default()
            },
            cancellation,
        )?;
        if result.bytes > options.max_bytes {
            std::fs::remove_file(&candidate)?;
            continue;
        }
        source.check(input)?;
        if identity != same_file::Handle::from_path(parent)? {
            return Err(fail("output_changed", "The output folder changed."));
        }
        cancellation.check()?;
        tempfile::TempPath::try_from_path(candidate)?
            .persist_noclobber(output)
            .map_err(|e| {
                fail(
                    if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                        "collision"
                    } else {
                        "io"
                    },
                    e.error.to_string(),
                )
            })?;
        result.output = output.to_path_buf();
        result.attempts = index as u32 + 1;
        return Ok(result);
    }
    Err(fail(
        "target_unmet",
        "No complete video met the byte limit within the selected constraints.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bitrate_search_is_descending_and_includes_the_floor() {
        for floor in [50000, 150000, 700000, 2500000, 100000000] {
            let rates = fit_rates(floor).unwrap();
            assert_eq!(rates.last(), Some(&floor));
            assert!(rates.len() <= 8 && rates.windows(2).all(|pair| pair[0] > pair[1]));
        }
        assert!(fit_rates(49999).is_err());
        assert!(fit_rates(100000001).is_err());
    }
    #[test]
    fn resize_is_explicit_even_bounded_and_orientation_aware() {
        assert_eq!(dimensions(1920, 1080, 90, Some(720)).unwrap(), (404, 720));
        assert_eq!(dimensions(64, 48, 0, Some(200)).unwrap(), (64, 48));
        assert!(dimensions(65, 49, 0, None).is_err());
        assert_eq!(dimensions(65, 49, 0, Some(64)).unwrap(), (64, 48));
        assert!(dimensions(64, 48, 45, None).is_err());
        assert!(dimensions(64, 48, 0, Some(1)).is_err());
    }
}
