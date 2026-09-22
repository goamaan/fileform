// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, media_packets, media_probe, media_timeline::TimeBase, native_process,
    Cancellation, MediaInterval, MediaTime, Result, Source,
};
use serde::Serialize;
use std::{path::Path, process::Command, time::Duration};
#[derive(Debug, Serialize)]
pub struct CopyReceipt {
    pub output: std::path::PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub requested_interval: MediaInterval,
    pub realized_interval: MediaInterval,
    pub duration_seconds: f64,
    pub duration_tolerance_seconds: f64,
    pub copied_packets: usize,
}
fn seconds(ticks: i64, base: TimeBase) -> f64 {
    ticks as f64 * f64::from(base.numerator) / f64::from(base.denominator)
}
pub fn trim(
    input: &Path,
    output: &Path,
    directory: &Path,
    interval: MediaInterval,
    expected: Option<&str>,
    cancel: &Cancellation,
) -> Result<CopyReceipt> {
    interval.validate()?;
    if !output
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("m4a"))
    {
        return Err(fail("invalid_request", "Fast audio trim writes M4A."));
    }
    if output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let mut source = Source::open_with_limit(input, cancel.clone(), 2 * 1024 * 1024 * 1024)?;
    if expected.is_some_and(|hash| hash != source.hash) {
        return Err(fail("source_changed", "The inspected source changed."));
    }
    let info = media_probe::inspect(source.snapshot.path(), directory, cancel)?;
    if info.audio_tracks != 1 || !info.format.split(',').any(|v| v == "mov") {
        return Err(fail(
            "unsupported",
            "Fast audio trim requires one AAC track in an MP4/MOV-family file.",
        ));
    }
    let audio = info
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .expect("counted audio");
    if audio.codec_name.as_deref() != Some("aac") {
        return Err(fail("unsupported", "Fast audio trim requires AAC packets."));
    }
    let base = TimeBase::parse(audio.time_base.as_deref())?;
    let origin = audio.start_pts.unwrap_or(0);
    let duration = audio
        .duration_ts
        .filter(|d| *d > 0 && seconds(*d, base) <= 21600.0)
        .ok_or_else(|| fail("unsupported", "Unmeasurable audio duration."))?;
    let container = info
        .container_start_time
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(f64::NAN);
    if !(0..=i64::MAX / 4).contains(&origin)
        || seconds(origin, base) > 21600.0
        || !container.is_finite()
        || (seconds(origin, base) - container).abs() > 0.001
    {
        return Err(fail("unsupported", "Audio and container clocks differ."));
    }
    if u128::from(interval.end.ticks) * u128::from(base.denominator)
        > duration as u128 * u128::from(base.numerator) * u128::from(interval.end.timescale)
    {
        return Err(fail("invalid_request", "Selection exceeds audio duration."));
    }
    let original = media_packets::read(source.snapshot.path(), directory, audio.index, cancel)?;
    let usable: Vec<_> = original.iter().filter(|p| p.pts >= origin).collect();
    if usable.is_empty()
        || usable.iter().any(|p| p.dts != Some(p.pts))
        || usable.windows(2).any(|p| p[0].pts >= p[1].pts)
    {
        return Err(fail(
            "unsupported",
            "Fast trim requires ordered audio packets.",
        ));
    }
    let before_or_at = |pts: i64, time: MediaTime| {
        (pts - origin) as u128 * u128::from(base.numerator) * u128::from(time.timescale)
            <= u128::from(time.ticks) * u128::from(base.denominator)
    };
    if usable
        .iter()
        .any(|p| seconds(p.duration, base) > 1.0 || p.pts - origin > duration)
    {
        return Err(fail(
            "unsupported",
            "AAC packet bounds exceed the measurable audio timeline.",
        ));
    }
    let start = usable
        .iter()
        .rev()
        .find(|p| before_or_at(p.pts, interval.start))
        .ok_or_else(|| fail("unsupported", "No preceding audio packet boundary."))?
        .pts
        - origin;
    let end = usable
        .iter()
        .find(|p| {
            (p.pts - origin) as u128
                * u128::from(base.numerator)
                * u128::from(interval.end.timescale)
                >= u128::from(interval.end.ticks) * u128::from(base.denominator)
        })
        .map_or(duration, |p| p.pts - origin);
    if start >= end || end > duration {
        return Err(fail("invalid_request", "Invalid packet interval."));
    }
    let time = |ticks: i64| -> Result<MediaTime> {
        Ok(MediaTime {
            ticks: u64::try_from(ticks as u128 * u128::from(base.numerator))
                .map_err(|_| fail("limit", "Clock overflow."))?,
            timescale: base.denominator,
        })
    };
    let realized = MediaInterval {
        start: time(start)?,
        end: time(end)?,
    };
    let tolerance = usable
        .iter()
        .map(|p| seconds(p.duration, base))
        .fold(0.0, f64::max)
        + 0.001;
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
    let mut command = Command::new(&executable);
    command.env_clear();
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    command
        .args([
            "-v",
            "error",
            "-nostdin",
            "-xerror",
            "-max_alloc",
            "268435456",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            "mov",
            "-i",
        ])
        .arg(source.snapshot.path())
        .args([
            "-ss",
            &format!(
                "{:.6}",
                (seconds(start, base) * 1000000.0).floor() / 1000000.0
            ),
            "-t",
            &format!("{:.9}", seconds(end - start, base)),
            "-map",
            &format!("0:{}", audio.index),
            "-vn",
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-c:a",
            "copy",
            "-copytb",
            "1",
            "-avoid_negative_ts",
            "disabled",
            "-f",
            "ipod",
            "-y",
        ])
        .arg(temporary.path());
    native_process::run_with_output_limit(
        command,
        cancel,
        Duration::from_secs(120),
        512 * 1024,
        Some((temporary.path(), 512 * 1024 * 1024)),
    )?;
    let after = media_probe::inspect(temporary.path(), directory, cancel)?;
    let result = after
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .ok_or_else(|| fail("verification", "Output audio is missing."))?;
    if !after.format.split(',').any(|v| v == "mov")
        || after.streams.len() != 1
        || result.codec_name.as_deref() != Some("aac")
        || audio.channels != result.channels
        || audio.sample_rate != result.sample_rate
        || (after.duration_seconds - seconds(end - start, base)).abs() > tolerance
    {
        return Err(fail(
            "verification",
            "Copied audio layout or duration changed.",
        ));
    }
    let copied = media_packets::read(temporary.path(), directory, result.index, cancel)?;
    let output_base = TimeBase::parse(result.time_base.as_deref())?;
    let matches = |a: &media_packets::Packet, b: &media_packets::Packet| {
        a.data_hash == b.data_hash
            && b.dts == Some(b.pts)
            && (seconds(a.duration, base) - seconds(b.duration, output_base)).abs() <= 0.001
            && (seconds(a.pts - origin - start, base) - seconds(b.pts, output_base)).abs() <= 0.001
    };
    let first = original
        .iter()
        .position(|p| matches(p, &copied[0]))
        .ok_or_else(|| {
            fail(
                "verification",
                "Copied packets start outside the approved interval.",
            )
        })?;
    if first + copied.len() > original.len()
        || original[first..first + copied.len()]
            .iter()
            .zip(&copied)
            .any(|(a, b)| !matches(a, b))
    {
        return Err(fail(
            "verification",
            "Encoded audio packets or timestamps changed.",
        ));
    }
    let actual_start = seconds(original[first].pts - origin, base);
    let last = &original[first + copied.len() - 1];
    let actual_end = seconds(last.pts + last.duration - origin, base);
    if (actual_start - seconds(start, base)).abs() > tolerance
        || (actual_end - seconds(end, base)).abs() > tolerance
        || actual_start > seconds(start, base) + 0.001
        || actual_end < seconds(end, base) - 0.001
    {
        return Err(fail(
            "verification",
            "Copied packet coverage differs from the approved interval.",
        ));
    }
    let mut decode = crate::media_video::command(&executable);
    decode
        .args(["-err_detect", "explode", "-i"])
        .arg(temporary.path())
        .args(["-map", "0:a:0", "-f", "null", "-"]);
    native_process::run(decode, cancel, Duration::from_secs(120), 512 * 1024)?;
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 512 * 1024 * 1024)?;
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
        requested_interval: interval,
        realized_interval: realized,
        duration_seconds: after.duration_seconds,
        duration_tolerance_seconds: tolerance,
        copied_packets: copied.len(),
    })
}
