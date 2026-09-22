// SPDX-License-Identifier: Apache-2.0
use crate::{fail, media_probe, native_process, Cancellation, Result, Source};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, time::Duration};

const MAX_FRAMES: usize = 100000;
#[derive(Debug, Deserialize)]
struct Frame {
    pts: Option<i64>,
    nb_samples: Option<u32>,
}
fn frames<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Frame>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<Frame>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("at most 100000 decoded audio frames")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = Vec::new();
            while let Some(frame) = seq.next_element()? {
                if result.len() == MAX_FRAMES {
                    return Err(serde::de::Error::custom(
                        "Decoded audio frame limit exceeded",
                    ));
                }
                result.push(frame);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_seq(Bounded)
}
#[derive(Deserialize)]
struct Report {
    #[serde(deserialize_with = "frames")]
    frames: Vec<Frame>,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct TimeBase {
    pub numerator: u32,
    pub denominator: u32,
}
impl TimeBase {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self> {
        let (n, d) = value
            .and_then(|s| s.split_once('/'))
            .ok_or_else(|| fail("unsupported", "Missing rational media time base."))?;
        let numerator = n
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| fail("unsupported", "Invalid media time base."))?;
        let denominator = d
            .parse::<u32>()
            .ok()
            .filter(|d| *d > 0 && *d <= i32::MAX as u32)
            .ok_or_else(|| fail("unsupported", "Invalid media time base."))?;
        Ok(Self {
            numerator,
            denominator,
        })
    }
}
#[derive(Debug, Serialize)]
pub struct AudioTimeline {
    pub sha256: String,
    pub stream_index: u32,
    pub sample_rate: u32,
    pub origin_ticks: i64,
    pub time_base: TimeBase,
    pub duration_ticks: i64,
    pub decoded_samples: u64,
    pub decoded_frames: usize,
    pub continuous_sample_clock: bool,
}
fn continuous(
    report: &Report,
    base: TimeBase,
    origin: i64,
    rate: u32,
    cancellation: &Cancellation,
) -> Result<u64> {
    if report.frames.is_empty()
        || !(0..=i64::MAX / 4).contains(&origin)
        || !(8000..=384000).contains(&rate)
    {
        return Err(fail("unsupported", "Unsupported audio clock."));
    }
    let mut samples = 0u64;
    for frame in &report.frames {
        cancellation.check()?;
        let pts = frame
            .pts
            .filter(|pts| *pts >= origin && *pts <= i64::MAX / 4)
            .ok_or_else(|| fail("unsupported", "Incomplete decoded audio timestamps."))?;
        let count = frame
            .nb_samples
            .filter(|n| *n > 0 && *n <= rate * 60)
            .ok_or_else(|| fail("unsupported", "Invalid decoded audio sample count."))?;
        let clock = (pts - origin) as u128 * u128::from(base.numerator) * u128::from(rate);
        let expected = u128::from(samples) * u128::from(base.denominator);
        if clock != expected {
            return Err(fail("unsupported","Audio has gaps, overlaps or imprecise timestamps. Normalize its clock before time-based trimming."));
        }
        samples += u64::from(count);
        if samples > 21601 * u64::from(rate) {
            return Err(fail("limit", "Decoded audio exceeds six hours."));
        }
    }
    Ok(samples)
}
pub fn inspect(
    input: &Path,
    directory: &Path,
    cancellation: &Cancellation,
) -> Result<AudioTimeline> {
    let mut source = Source::open_with_limit(input, cancellation.clone(), 2 * 1024 * 1024 * 1024)?;
    let info = media_probe::inspect(source.snapshot.path(), directory, cancellation)?;
    if info.audio_tracks != 1 {
        return Err(fail(
            "unsupported",
            "Choose one audio track for timeline inspection.",
        ));
    }
    let audio = info
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .expect("counted audio");
    let rate = audio
        .sample_rate
        .as_deref()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|r| (8000..=384000).contains(r))
        .ok_or_else(|| fail("unsupported", "Unsupported audio sample rate."))?;
    let base = TimeBase::parse(audio.time_base.as_deref())?;
    let origin = audio.start_pts.unwrap_or(0);
    let duration = audio
        .duration_ts
        .filter(|d| {
            *d > 0
                && (u128::try_from(*d).unwrap() * u128::from(base.numerator))
                    <= 21600 * u128::from(base.denominator)
        })
        .ok_or_else(|| fail("unsupported", "Audio has no bounded, measurable duration."))?;
    let origin_seconds = origin as f64 * f64::from(base.numerator) / f64::from(base.denominator);
    let container_origin = info
        .container_start_time
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| fail("unsupported", "Invalid container start time."))?;
    if !(0.0..=21600.0).contains(&origin_seconds)
        || (origin_seconds - container_origin).abs() > 0.001
    {
        return Err(fail(
            "unsupported",
            "The audio and container use different start clocks.",
        ));
    }
    let executable = directory.canonicalize()?.join(if cfg!(windows) {
        "bin/ffprobe.exe"
    } else {
        "bin/ffprobe"
    });
    let mut command = Command::new(executable);
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
            "-max_alloc",
            "268435456",
            "-threads",
            "2",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac",
            "-select_streams",
            &audio.index.to_string(),
            "-show_frames",
            "-show_entries",
            "frame=pts,nb_samples",
            "-of",
            "json=compact=1",
        ])
        .arg(source.snapshot.path());
    let bytes = native_process::run(
        command,
        cancellation,
        Duration::from_secs(120),
        16 * 1024 * 1024,
    )?;
    let report: Report = serde_json::from_slice(&bytes).map_err(|_| {
        fail(
            "unsupported",
            "Audio timeline is invalid or exceeds 100000 decoded frames.",
        )
    })?;
    let samples = continuous(&report, base, origin, rate, cancellation)?;
    source.check(input)?;
    Ok(AudioTimeline {
        sha256: source.hash,
        stream_index: audio.index,
        sample_rate: rate,
        origin_ticks: origin,
        time_base: base,
        duration_ticks: duration,
        decoded_samples: samples,
        decoded_frames: report.frames.len(),
        continuous_sample_clock: true,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rational_clock_checks_detect_gaps_overlaps_and_missing_pts() {
        let base = TimeBase::parse(Some("1/48000")).unwrap();
        for next in [105, 107] {
            let report = Report {
                frames: vec![
                    Frame {
                        pts: Some(100),
                        nb_samples: Some(6),
                    },
                    Frame {
                        pts: Some(next),
                        nb_samples: Some(4),
                    },
                ],
            };
            assert!(continuous(&report, base, 100, 48000, &Cancellation::default()).is_err());
        }
        let report = Report {
            frames: vec![
                Frame {
                    pts: Some(100),
                    nb_samples: Some(6),
                },
                Frame {
                    pts: Some(106),
                    nb_samples: Some(4),
                },
            ],
        };
        assert_eq!(
            continuous(&report, base, 100, 48000, &Cancellation::default()).unwrap(),
            10
        );
        let report = Report {
            frames: vec![Frame {
                pts: None,
                nb_samples: Some(6),
            }],
        };
        assert!(continuous(&report, base, 100, 48000, &Cancellation::default()).is_err());
        assert!(TimeBase::parse(Some("1/0")).is_err());
    }
    #[test]
    fn decoded_frame_array_is_bounded_during_parsing() {
        let text = format!("{{\"frames\":[{}]}}", vec!["{}"; MAX_FRAMES + 1].join(","));
        assert!(serde_json::from_str::<Report>(&text).is_err());
    }
}
