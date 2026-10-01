// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, media_probe, media_time_trim, media_timeline, media_video_timeline,
    media_video_trim, output_collision, Cancellation, MediaInterval, MediaTime, OutputCollision,
    Result, Source, VideoTrimOptions,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playback {
    pub input: PathBuf,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub audio_stream: Option<u32>,
    #[serde(default)]
    pub mute_audio: bool,
    pub max_dimension: Option<u32>,
    pub expected_source_sha256: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct PlaybackReceipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: String,
    pub duration: MediaTime,
    pub source_audio_stream_index: Option<u32>,
    pub source_video_stream_index: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub warnings: Vec<&'static str>,
}
fn duration(ticks: i64, base: media_timeline::TimeBase) -> Result<MediaTime> {
    let ticks = u64::try_from(ticks)
        .ok()
        .and_then(|v| v.checked_mul(u64::from(base.numerator)))
        .ok_or_else(|| fail("verification", "Invalid playback duration."))?;
    let time = MediaTime {
        ticks,
        timescale: base.denominator,
    };
    time.validate()?;
    Ok(time)
}
pub fn export(options: &Playback, cancel: &Cancellation) -> Result<PlaybackReceipt> {
    let maximum = options.max_dimension.unwrap_or(1280);
    if !(64..=1920).contains(&maximum) {
        return Err(fail(
            "invalid_request",
            "Choose playback dimensions from 64 to 1920 pixels.",
        ));
    }
    if options.output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let mut source =
        Source::open_with_limit(&options.input, cancel.clone(), 2 * 1024 * 1024 * 1024)?;
    if options
        .expected_source_sha256
        .as_ref()
        .is_some_and(|v| v != &source.hash)
    {
        return Err(fail("source_changed", "The inspected source changed."));
    }
    let info = media_probe::inspect(source.snapshot.path(), &options.directory, cancel)?;
    let audio = media_probe::retained_audio(&info, options.audio_stream, options.mute_audio)?;
    let is_video = info.video_tracks > 0;
    if !is_video && (options.mute_audio || options.max_dimension.is_some()) {
        return Err(fail(
            "invalid_request",
            "Audio playback does not use mute or picture dimensions.",
        ));
    }
    let extension = if is_video { "mp4" } else { "wav" };
    if !options
        .output
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case(extension))
    {
        return Err(fail(
            "invalid_request",
            format!("This playback preview requires {extension} output."),
        ));
    }
    let parent = options
        .output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let working = tempfile::tempdir_in(parent)?;
    let candidate = working.path().join(format!("playback.{extension}"));
    let (duration, video_index) = if is_video {
        let timeline =
            media_video_timeline::inspect(source.snapshot.path(), &options.directory, cancel)?;
        let length = duration(timeline.duration_ticks, timeline.time_base)?;
        media_video_trim::trim(
            source.snapshot.path(),
            &candidate,
            &options.directory,
            VideoTrimOptions {
                interval: MediaInterval {
                    start: MediaTime {
                        ticks: 0,
                        timescale: 1,
                    },
                    end: length,
                },
                audio_stream: options.audio_stream,
                max_dimension: Some(maximum),
                mute_audio: options.mute_audio,
            },
            Some(&source.hash),
            cancel,
        )?;
        (length, Some(timeline.stream_index))
    } else {
        let audio = audio.ok_or_else(|| fail("unsupported", "No playback track was found."))?;
        let timeline = media_timeline::inspect(
            source.snapshot.path(),
            &options.directory,
            Some(audio.index),
            cancel,
        )?;
        let length = duration(timeline.duration_ticks, timeline.time_base)?;
        media_time_trim::trim(
            source.snapshot.path(),
            &candidate,
            &options.directory,
            MediaInterval {
                start: MediaTime {
                    ticks: 0,
                    timescale: 1,
                },
                end: length,
            },
            Some(audio.index),
            Some(&source.hash),
            cancel,
        )?;
        (length, None)
    };
    let after = media_probe::inspect(&candidate, &options.directory, cancel)?;
    let expected_seconds = duration.ticks as f64 / f64::from(duration.timescale);
    let tolerance = audio
        .and_then(|v| v.sample_rate.as_deref())
        .and_then(|v| v.parse::<u32>().ok())
        .map_or(0.001, |rate| (2048.0 / f64::from(rate)).max(0.1));
    let start = after
        .container_start_time
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(f64::NAN);
    let picture = after.streams.iter().find(|v| v.codec_type == "video");
    if after.video_tracks != usize::from(is_video)
        || after.audio_tracks != usize::from(audio.is_some())
        || after.streams.len() != after.audio_tracks + after.video_tracks
        || !start.is_finite()
        || start.abs() > 0.001
        || (after.duration_seconds - expected_seconds).abs() > tolerance
        || picture.is_some_and(|v| {
            v.width.is_none_or(|n| n > maximum) || v.height.is_none_or(|n| n > maximum)
        })
    {
        return Err(fail("verification","Playback streams, duration, normalized clock or picture bounds changed. No output was saved."));
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&candidate)?;
    let (bytes, sha256) = digest(&mut file, cancel, 2 * 1024 * 1024 * 1024)?;
    file.sync_all()?;
    drop(file);
    source.check(&options.input)?;
    if identity != same_file::Handle::from_path(parent)? {
        return Err(fail("output_changed", "The output folder changed."));
    }
    let output = output_collision::publish_path(
        tempfile::TempPath::try_from_path(candidate)?,
        &options.output,
        OutputCollision::Fail,
        cancel,
    )?;
    Ok(PlaybackReceipt{output,bytes,sha256,source_sha256:source.hash,duration,
        source_audio_stream_index:audio.map(|v| v.index),source_video_stream_index:video_index,
        width:picture.and_then(|v| v.width),height:picture.and_then(|v| v.height),
        warnings:vec!["Playback uses a normalized preview file. Final transformations must use the original recording; video previews use lossy H.264/AAC."]})
}
