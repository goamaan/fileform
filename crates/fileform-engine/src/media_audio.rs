// SPDX-License-Identifier: Apache-2.0
use crate::{digest, fail, media_pack, media_probe, native_process, Cancellation, Result, Source};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, time::Duration};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleRange {
    pub start: u64,
    pub end: u64,
}
impl SampleRange {
    fn count(self) -> Result<u64> {
        self.end
            .checked_sub(self.start)
            .filter(|v| *v > 0)
            .ok_or_else(|| {
                fail(
                    "invalid_request",
                    "Choose a nonempty, increasing sample range.",
                )
            })
    }
    fn filter(self) -> String {
        format!(
            "atrim=start_sample={}:end_sample={},asetpts=PTS-STARTPTS",
            self.start, self.end
        )
    }
}
const MAX_OUTPUT: u64 = 512 * 1024 * 1024;
#[derive(Debug, Serialize)]
pub struct AudioReceipt {
    pub output: std::path::PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub duration_seconds: f64,
    pub codec: String,
    pub channels: u32,
    pub sample_rate: String,
    pub lossy_codec: bool,
    pub trimmed_samples: Option<SampleRange>,
}
fn format(output: &Path) -> Result<(&'static str, &'static str)> {
    match output
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("wav") => Ok(("wav", "pcm_s16le")),
        Some("flac") => Ok(("flac", "flac")),
        Some("m4a") => Ok(("ipod", "aac")),
        Some("mp3") => Ok(("mp3", "libmp3lame")),
        _ => Err(fail(
            "invalid_request",
            "Choose WAV, FLAC, M4A or MP3 output.",
        )),
    }
}
fn command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command.env_clear();
    #[cfg(windows)]
    {
        if let Some(system) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system);
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
        "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac",
        "-threads",
        "2",
    ]);
    command
}
pub fn convert(
    input: &Path,
    output: &Path,
    directory: &Path,
    expected: Option<&str>,
    trim: Option<SampleRange>,
    cancellation: &Cancellation,
) -> Result<AudioReceipt> {
    let (muxer, encoder) = format(output)?;
    if let Some(range) = trim {
        range.count()?;
        if !matches!(muxer, "wav" | "flac") {
            return Err(fail(
                "unsupported",
                "Exact sample trimming currently writes WAV or FLAC.",
            ));
        }
    }
    let pack = media_pack::verify(directory, cancellation)?;
    if encoder == "libmp3lame" && !pack.supports_mp3 {
        return Err(fail(
            "engine_unavailable",
            "The media pack does not declare MP3 support.",
        ));
    }
    if output.try_exists()? {
        return Err(fail(
            "collision",
            "The output already exists. Choose another name.",
        ));
    }
    let mut source = Source::open_with_limit(input, cancellation.clone(), 2 * 1024 * 1024 * 1024)?;
    if expected.is_some_and(|hash| hash != source.hash) {
        return Err(fail(
            "source_changed",
            "The inspected source changed. Add it again.",
        ));
    }
    let inspection = media_probe::inspect(source.snapshot.path(), directory, cancellation)?;
    if inspection.audio_tracks != 1 {
        return Err(fail(
            "unsupported",
            "Choose media with exactly one audio track. Track selection is not available yet.",
        ));
    }
    let audio = inspection
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .expect("counted audio");
    let bit_depth = audio.bits_per_sample.unwrap_or(0).max(
        audio
            .bits_per_raw_sample
            .as_deref()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0),
    );
    if encoder == "flac"
        && (audio
            .codec_name
            .as_deref()
            .is_some_and(|c| c.starts_with("pcm_f"))
            || bit_depth > 24)
    {
        return Err(fail(
            "unsupported",
            "FLAC cannot preserve floating-point or greater-than-24-bit audio in this route.",
        ));
    }
    let channels = audio
        .channels
        .filter(|n| *n > 0 && *n <= 32)
        .ok_or_else(|| {
            fail(
                "unsupported",
                "The audio channel count could not be established.",
            )
        })?;
    let sample_rate = audio
        .sample_rate
        .as_ref()
        .filter(|v| v.parse::<u32>().is_ok_and(|n| n > 0 && n <= 384000))
        .ok_or_else(|| {
            fail(
                "unsupported",
                "The audio sample rate could not be established.",
            )
        })?;
    if encoder == "libmp3lame"
        && (!(1..=2).contains(&channels)
            || !["32000", "44100", "48000"].contains(&sample_rate.as_str()))
    {
        return Err(fail("unsupported","MP3 requires mono/stereo audio at 32, 44.1 or 48 kHz. No implicit downmixing or resampling is performed."));
    }
    let source_duration = audio
        .duration
        .as_deref()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.0)
        .unwrap_or(inspection.duration_seconds);
    let rate = sample_rate
        .parse::<u64>()
        .map_err(|_| fail("unsupported", "Invalid audio sample rate."))?;
    let expected_duration = if let Some(range) = trim {
        if range.end > rate.saturating_mul(21600) {
            return Err(fail("limit", "Trim exceeds the six-hour sample limit."));
        }
        range.count()? as f64 / rate as f64
    } else {
        source_duration
    };
    let parent = output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent_identity = same_file::Handle::from_path(parent)?;
    let staging = tempfile::tempdir_in(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(staging.path())?;
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let executable = directory
        .canonicalize()?
        .join(format!("bin/ffmpeg{suffix}"));
    let mut encode = command(&executable);
    encode.arg("-i").arg(source.snapshot.path()).args([
        "-map",
        "0:a:0",
        "-vn",
        "-sn",
        "-dn",
        "-map_metadata",
        "-1",
        "-map_chapters",
        "-1",
        "-c:a",
        encoder,
    ]);
    if let Some(range) = trim {
        encode.args(["-af", &range.filter()]);
    }
    match encoder {
        "libmp3lame" => {
            encode.args([
                "-b:a",
                "128000",
                "-write_xing",
                "1",
                "-id3v2_version",
                "0",
                "-write_id3v1",
                "0",
            ]);
        }
        "aac" => {
            encode.args(["-b:a", "128000", "-movflags", "+faststart"]);
        }
        "flac" => {
            encode.args(["-compression_level", "8"]);
        }
        _ => {}
    }
    encode
        .args(["-threads", "2", "-f", muxer, "-y"])
        .arg(temporary.path());
    let deadline = Duration::from_secs_f64((expected_duration * 4.0 + 60.0).clamp(120.0, 43200.0));
    native_process::run_with_output_limit(
        encode,
        cancellation,
        deadline,
        512 * 1024,
        Some((temporary.path(), MAX_OUTPUT)),
    )?;
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancellation, MAX_OUTPUT)?;
    if bytes == 0 {
        return Err(fail("verification", "The audio output is empty."));
    }
    let result = media_probe::inspect(temporary.path(), directory, cancellation)?;
    let output_audio = result.streams.iter().find(|s| s.codec_type == "audio");
    let expected_container = if muxer == "ipod" { "mov" } else { muxer };
    if !result
        .format
        .split(',')
        .any(|value| value == expected_container)
    {
        return Err(fail(
            "verification",
            "Output container differs from the requested result.",
        ));
    }
    let expected_codec = if encoder == "libmp3lame" {
        "mp3"
    } else {
        encoder
    };
    if result.audio_tracks != 1
        || result.video_tracks != 0
        || result.streams.len() != 1
        || (result.duration_seconds - expected_duration).abs() > 0.25
        || !output_audio.is_some_and(|s| {
            s.codec_name.as_deref() == Some(expected_codec)
                && s.channels == Some(channels)
                && s.sample_rate.as_ref() == Some(sample_rate)
        })
    {
        return Err(fail(
            "verification",
            "Output codec, tracks, duration or audio layout differ from the requested result.",
        ));
    }
    if let Some(range) = trim {
        let track = output_audio.expect("verified audio");
        if track.duration_ts.and_then(|n| u64::try_from(n).ok()) != Some(range.count()?)
            || track.time_base.as_deref() != Some(format!("1/{rate}").as_str())
        {
            return Err(fail(
                "verification",
                "The output does not contain the requested number of audio samples.",
            ));
        }
        let bits = track.bits_per_sample.unwrap_or(0).max(
            track
                .bits_per_raw_sample
                .as_deref()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0),
        );
        let pcm = match bits {
            8 => "pcm_s8",
            16 => "pcm_s16le",
            24 => "pcm_s24le",
            _ => {
                return Err(fail(
                    "verification",
                    "Unsupported output precision for exact trim verification.",
                ))
            }
        };
        let hash_audio = |path: &Path, selection: Option<SampleRange>| -> Result<Vec<u8>> {
            let mut hash = command(&executable);
            hash.arg("-i")
                .arg(path)
                .args(["-map", "0:a:0", "-vn", "-sn", "-dn"]);
            if let Some(range) = selection {
                hash.args(["-af", &range.filter()]);
            }
            hash.args(["-c:a", pcm, "-f", "hash", "-hash", "sha256", "-"]);
            native_process::run(hash, cancellation, deadline, 4096)
        };
        let expected_hash = hash_audio(source.snapshot.path(), Some(range))?;
        let actual_hash = hash_audio(temporary.path(), None)?;
        if expected_hash != actual_hash || !expected_hash.starts_with(b"SHA256=") {
            return Err(fail(
                "verification",
                "Trimmed audio samples do not match the selected source interval.",
            ));
        }
    }
    let mut decode = command(&executable);
    decode
        .args(["-err_detect", "explode"])
        .arg("-i")
        .arg(temporary.path())
        .args(["-map", "0:a:0", "-f", "null", "-"]);
    native_process::run(decode, cancellation, deadline, 512 * 1024)?;
    source.check(input)?;
    if parent_identity != same_file::Handle::from_path(parent)? {
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
    Ok(AudioReceipt {
        output: output.to_path_buf(),
        bytes,
        sha256,
        duration_seconds: result.duration_seconds,
        codec: expected_codec.into(),
        channels,
        sample_rate: sample_rate.clone(),
        trimmed_samples: trim,
        lossy_codec: matches!(encoder, "aac" | "libmp3lame"),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_are_explicit_and_case_insensitive() {
        assert!(SampleRange { start: 1, end: 1 }.count().is_err());
        assert!(SampleRange { start: 2, end: 1 }.count().is_err());
        assert_eq!(
            SampleRange {
                start: 123,
                end: 456
            }
            .count()
            .unwrap(),
            333
        );
        assert_eq!(
            format(Path::new("sound.WAV")).unwrap(),
            ("wav", "pcm_s16le")
        );
        assert_eq!(
            format(Path::new("sound.mp3")).unwrap(),
            ("mp3", "libmp3lame")
        );
        assert!(format(Path::new("sound.mp4")).is_err());
        assert!(format(Path::new("sound")).is_err());
    }
}
