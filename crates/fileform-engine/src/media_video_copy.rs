// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, media_audio, media_packets, media_probe, media_timeline::TimeBase, media_video,
    media_video_timeline, native_process, Cancellation, MediaInterval, MediaTime, Result, Source,
    VideoTrimOptions,
};
use serde::Serialize;
use std::{path::Path, time::Duration};
#[derive(Debug, Serialize)]
pub struct CopyReceipt {
    pub output: std::path::PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub requested_interval: MediaInterval,
    pub realized_interval: MediaInterval,
    pub start_frame: u32,
    pub end_frame: u32,
    pub duration_seconds: f64,
    pub duration_tolerance_seconds: f64,
    pub audio_tracks: usize,
    pub muted_audio: bool,
}
fn seconds(ticks: i64, base: TimeBase) -> f64 {
    ticks as f64 * f64::from(base.numerator) / f64::from(base.denominator)
}
pub fn trim(
    input: &Path,
    output: &Path,
    directory: &Path,
    options: VideoTrimOptions,
    expected: Option<&str>,
    cancel: &Cancellation,
) -> Result<CopyReceipt> {
    options.interval.validate()?;
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
    if output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let mut source = Source::open_with_limit(input, cancel.clone(), 2 * 1024 * 1024 * 1024)?;
    if expected.is_some_and(|hash| hash != source.hash) {
        return Err(fail("source_changed", "The inspected source changed."));
    }
    let info = media_probe::inspect(source.snapshot.path(), directory, cancel)?;
    let picture = media_video::video(&info, false)?;
    if picture.codec_name.as_deref() != Some("h264") {
        return Err(fail("unsupported", "Fast video trim requires H.264 video."));
    }
    media_video::trim_picture(&info)?;
    if !info.format.split(',').any(|s| s == "mov") {
        return Err(fail(
            "unsupported",
            "Fast video trim requires MP4/MOV-family H.264/AAC input.",
        ));
    }
    let clock = media_video_timeline::inspect(source.snapshot.path(), directory, cancel)?;
    if clock.has_reordered_packets {
        return Err(fail(
            "unsupported",
            "Fast video trim requires no reordered packets. Use exact mode.",
        ));
    }
    let base = clock.time_base;
    if u128::from(options.interval.end.ticks) * u128::from(base.denominator)
        > clock.duration_ticks as u128
            * u128::from(base.numerator)
            * u128::from(options.interval.end.timescale)
    {
        return Err(fail(
            "invalid_request",
            "Selection exceeds the video duration.",
        ));
    }
    let compare = |frame: u32, time: MediaTime| {
        (u128::from(frame)
            * clock.frame_ticks as u128
            * u128::from(base.numerator)
            * u128::from(time.timescale))
        .cmp(&(u128::from(time.ticks) * u128::from(base.denominator)))
    };
    let start = *clock
        .keyframe_indices
        .iter()
        .rev()
        .find(|n| !compare(**n, options.interval.start).is_gt())
        .ok_or_else(|| fail("unsupported", "No preceding verified keyframe."))?;
    let end = clock
        .keyframe_indices
        .iter()
        .find(|n| !compare(**n, options.interval.end).is_lt())
        .copied()
        .unwrap_or(clock.decoded_frames);
    if start >= end {
        return Err(fail(
            "invalid_request",
            "No complete keyframe interval selected.",
        ));
    }
    let time = |n: u32| MediaTime {
        ticks: u64::from(n) * clock.frame_ticks as u64 * u64::from(base.numerator),
        timescale: base.denominator,
    };
    let realized = MediaInterval {
        start: time(start),
        end: time(end),
    };
    let begin = seconds(i64::from(start) * clock.frame_ticks, base);
    let finish = seconds(i64::from(end) * clock.frame_ticks, base);
    let retained_audio = info
        .streams
        .iter()
        .find(|s| s.codec_type == "audio" && !options.mute_audio);
    let mut tolerance = 0.001;
    let mut retained_packets = None;
    if let Some(audio) = retained_audio {
        media_audio::validate_trim_source(audio, false)?;
        if audio.codec_name.as_deref() != Some("aac")
            || !(0..=i64::MAX / 4).contains(&audio.start_pts.unwrap_or(0))
        {
            return Err(fail(
                "unsupported",
                "Retained audio must be AAC with a bounded source clock.",
            ));
        }
        let audio_base = TimeBase::parse(audio.time_base.as_deref())?;
        let rate = audio
            .sample_rate
            .as_deref()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|n| (8000..=384000).contains(n))
            .ok_or_else(|| fail("unsupported", "Invalid audio rate."))?;
        if (seconds(audio.start_pts.unwrap_or(0), audio_base) - seconds(clock.origin_ticks, base))
            .abs()
            > 1.0 / f64::from(rate)
        {
            return Err(fail(
                "unsupported",
                "Audio and video clocks differ. Normalize or mute audio.",
            ));
        }
        let packets = media_packets::read(source.snapshot.path(), directory, audio.index, cancel)?;
        if packets
            .iter()
            .any(|p| p.dts != Some(p.pts) || seconds(p.duration, audio_base) > 1.0)
        {
            return Err(fail(
                "unsupported",
                "Retained AAC packet timing is unsupported.",
            ));
        }
        tolerance += packets
            .iter()
            .map(|p| seconds(p.duration, audio_base))
            .fold(0.0, f64::max);
        retained_packets = Some(packets);
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let staging = tempfile::tempdir_in(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(staging.path())?;
    let executable = directory.canonicalize()?.join(if cfg!(windows) {
        "bin/ffmpeg.exe"
    } else {
        "bin/ffmpeg"
    });
    let mut copy = media_video::command(&executable);
    copy.args(["-ss", &format!("{begin:.9}"), "-i"])
        .arg(source.snapshot.path())
        .args([
            "-t",
            &format!("{:.9}", finish - begin),
            "-map",
            &format!("0:{}", picture.index),
        ]);
    if let Some(audio) = retained_audio {
        copy.args(["-map", &format!("0:{}", audio.index)]);
    } else {
        copy.arg("-an");
    }
    copy.args([
        "-map_metadata",
        "-1",
        "-map_chapters",
        "-1",
        "-c",
        "copy",
        "-copytb",
        "1",
        "-avoid_negative_ts",
        "disabled",
        "-f",
        muxer,
        "-y",
    ])
    .arg(temporary.path());
    native_process::run_with_output_limit(
        copy,
        cancel,
        Duration::from_secs(120),
        512 * 1024,
        Some((temporary.path(), 2 * 1024 * 1024 * 1024)),
    )?;
    let after = media_probe::inspect(temporary.path(), directory, cancel)?;
    let target = media_video::video(&after, false)?;
    if !after.format.split(',').any(|s| s == "mov")
        || target.codec_name != picture.codec_name
        || target.pix_fmt != picture.pix_fmt
        || after.audio_tracks != usize::from(retained_audio.is_some())
        || (after.duration_seconds - (finish - begin)).abs() > tolerance
        || target.width != picture.width
        || target.height != picture.height
        || target.side_data_list != picture.side_data_list
        || target.sample_aspect_ratio != picture.sample_aspect_ratio
        || target.color_transfer != picture.color_transfer
        || target.color_primaries != picture.color_primaries
        || target.color_space != picture.color_space
        || target.color_range != picture.color_range
        || target
            .nb_frames
            .as_deref()
            .and_then(|n| n.parse::<u32>().ok())
            != Some(end - start)
    {
        return Err(fail(
            "verification",
            "Copied video layout, timing or display properties changed.",
        ));
    }
    let original = media_packets::read(source.snapshot.path(), directory, picture.index, cancel)?;
    let copied = media_packets::read(temporary.path(), directory, target.index, cancel)?;
    media_packets::verify_copy(
        &original,
        &copied,
        media_packets::CopyWindow {
            base,
            origin: clock.origin_ticks,
            start: begin,
            end: finish,
            tolerance: 0.001,
        },
        TimeBase::parse(target.time_base.as_deref())?,
    )?;
    if let Some(audio) = retained_audio {
        let output_audio = after
            .streams
            .iter()
            .find(|s| s.codec_type == "audio")
            .ok_or_else(|| fail("verification", "Copied audio is missing."))?;
        if output_audio.codec_name.as_deref() != Some("aac")
            || audio.channels != output_audio.channels
            || audio.sample_rate != output_audio.sample_rate
        {
            return Err(fail("verification", "Copied audio layout changed."));
        }
        let original = retained_packets.as_ref().expect("verified retained audio");
        let copied = media_packets::read(temporary.path(), directory, output_audio.index, cancel)?;
        media_packets::verify_copy(
            original,
            &copied,
            media_packets::CopyWindow {
                base: TimeBase::parse(audio.time_base.as_deref())?,
                origin: audio.start_pts.unwrap_or(0),
                start: begin,
                end: finish,
                tolerance,
            },
            TimeBase::parse(output_audio.time_base.as_deref())?,
        )?;
    }
    let hash = |path: &Path, selection: bool| -> Result<Vec<u8>> {
        let mut decode = media_video::command(&executable);
        decode
            .args(["-err_detect", "explode", "-noautorotate", "-i"])
            .arg(path)
            .args(["-map", "0:v:0"]);
        if selection {
            decode.args([
                "-vf",
                &format!("trim=start_frame={start}:end_frame={end},setpts=PTS-STARTPTS"),
            ]);
        }
        decode.args([
            "-c:v",
            "rawvideo",
            "-pix_fmt",
            "yuv420p",
            "-fps_mode",
            "passthrough",
            "-threads",
            "2",
            "-f",
            "hash",
            "-hash",
            "sha256",
            "-",
        ]);
        native_process::run(decode, cancel, Duration::from_secs(120), 4096)
    };
    let wanted = hash(source.snapshot.path(), true)?;
    if !wanted.starts_with(b"SHA256=") || wanted != hash(temporary.path(), false)? {
        return Err(fail(
            "verification",
            "Copied pictures do not match the selected source frames.",
        ));
    }
    let mut decode = media_video::command(&executable);
    decode
        .args(["-err_detect", "explode", "-i"])
        .arg(temporary.path())
        .args(["-f", "null", "-"]);
    native_process::run(decode, cancel, Duration::from_secs(120), 512 * 1024)?;
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 2 * 1024 * 1024 * 1024)?;
    source.check(input)?;
    if identity != same_file::Handle::from_path(parent)? {
        return Err(fail("output_changed", "Output folder changed."));
    }
    temporary.as_file().sync_all()?;
    cancel.check()?;
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
    Ok(CopyReceipt {
        output: output.to_path_buf(),
        bytes,
        sha256,
        requested_interval: options.interval,
        realized_interval: realized,
        start_frame: start,
        end_frame: end,
        duration_seconds: after.duration_seconds,
        duration_tolerance_seconds: tolerance,
        audio_tracks: after.audio_tracks,
        muted_audio: options.mute_audio,
    })
}
