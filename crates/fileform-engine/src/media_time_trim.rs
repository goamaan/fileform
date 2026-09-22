// SPDX-License-Identifier: Apache-2.0
use crate::{fail, media_audio, media_timeline, Cancellation, Result, SampleRange};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MediaTime {
    pub ticks: u64,
    pub timescale: u32,
}
impl MediaTime {
    fn validate(self) -> Result<()> {
        if self.timescale == 0 || u128::from(self.ticks) > 21600 * u128::from(self.timescale) {
            return Err(fail(
                "invalid_request",
                "Choose a time within six hours with a positive timescale.",
            ));
        }
        Ok(())
    }
    pub fn decimal(value: &str) -> Result<Self> {
        let invalid = || {
            fail(
                "invalid_request",
                "Use non-negative decimal seconds with at most nine fractional digits.",
            )
        };
        if value.is_empty() || value.len() > 24 {
            return Err(invalid());
        }
        let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
        if whole.is_empty() && fraction.is_empty()
            || fraction.len() > 9
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|b| b.is_ascii_digit())
        {
            return Err(invalid());
        }
        let scale = 10u32.pow(fraction.len() as u32);
        let whole = if whole.is_empty() {
            0
        } else {
            whole.parse::<u64>().map_err(|_| invalid())?
        };
        let fractional = if fraction.is_empty() {
            0
        } else {
            fraction.parse::<u64>().map_err(|_| invalid())?
        };
        let ticks = whole
            .checked_mul(u64::from(scale))
            .and_then(|v| v.checked_add(fractional))
            .ok_or_else(invalid)?;
        let time = Self {
            ticks,
            timescale: scale,
        };
        time.validate()?;
        Ok(time)
    }
    fn ceil_samples(self, rate: u32) -> Result<u64> {
        self.validate()?;
        let value =
            (u128::from(self.ticks) * u128::from(rate)).div_ceil(u128::from(self.timescale));
        u64::try_from(value).map_err(|_| {
            fail(
                "invalid_request",
                "Sample position exceeds the supported range.",
            )
        })
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MediaInterval {
    pub start: MediaTime,
    pub end: MediaTime,
}
impl MediaInterval {
    fn validate(self) -> Result<()> {
        self.start.validate()?;
        self.end.validate()?;
        if u128::from(self.start.ticks) * u128::from(self.end.timescale)
            >= u128::from(self.end.ticks) * u128::from(self.start.timescale)
        {
            return Err(fail(
                "invalid_request",
                "Choose a nonempty, increasing time interval.",
            ));
        }
        Ok(())
    }
    fn samples(self, timeline: &media_timeline::AudioTimeline) -> Result<SampleRange> {
        self.validate()?;
        let duration = u128::try_from(timeline.duration_ticks)
            .map_err(|_| fail("unsupported", "Invalid audio duration."))?;
        if u128::from(self.end.ticks) * u128::from(timeline.time_base.denominator)
            > duration * u128::from(timeline.time_base.numerator) * u128::from(self.end.timescale)
        {
            return Err(fail(
                "invalid_request",
                "The selected interval exceeds the audio duration.",
            ));
        }
        let start = self.start.ceil_samples(timeline.sample_rate)?;
        let end = self.end.ceil_samples(timeline.sample_rate)?;
        if start >= end || end > timeline.decoded_samples {
            return Err(fail(
                "invalid_request",
                "The interval has no complete sample onsets or exceeds decoded audio.",
            ));
        }
        Ok(SampleRange { start, end })
    }
}
#[derive(Debug, Serialize)]
pub struct TimedAudioReceipt {
    #[serde(flatten)]
    pub audio: media_audio::AudioReceipt,
    pub requested_interval: MediaInterval,
    pub realized_interval: MediaInterval,
    pub source_origin_ticks: i64,
    pub source_time_base: media_timeline::TimeBase,
}
pub fn trim(
    input: &Path,
    output: &Path,
    directory: &Path,
    interval: MediaInterval,
    expected: Option<&str>,
    cancellation: &Cancellation,
) -> Result<TimedAudioReceipt> {
    interval.validate()?;
    let timeline = media_timeline::inspect(input, directory, cancellation)?;
    if expected.is_some_and(|hash| hash != timeline.sha256) {
        return Err(fail("source_changed", "The inspected source changed."));
    }
    let samples = interval.samples(&timeline)?;
    let audio = media_audio::convert(
        input,
        output,
        directory,
        Some(&timeline.sha256),
        Some(samples),
        None,
        cancellation,
    )?;
    Ok(TimedAudioReceipt {
        audio,
        requested_interval: interval,
        realized_interval: MediaInterval {
            start: MediaTime {
                ticks: samples.start,
                timescale: timeline.sample_rate,
            },
            end: MediaTime {
                ticks: samples.end,
                timescale: timeline.sample_rate,
            },
        },
        source_origin_ticks: timeline.origin_ticks,
        source_time_base: timeline.time_base,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_seconds_and_half_open_sample_onsets_are_exact() {
        assert_eq!(
            MediaTime::decimal("0.000000001")
                .unwrap()
                .ceil_samples(44100)
                .unwrap(),
            1
        );
        assert_eq!(
            MediaTime::decimal(".25")
                .unwrap()
                .ceil_samples(44100)
                .unwrap(),
            11025
        );
        assert_eq!(
            MediaTime::decimal("1.001")
                .unwrap()
                .ceil_samples(44100)
                .unwrap(),
            44145
        );
        for text in [
            "",
            ".",
            "-1",
            "NaN",
            "1e3",
            "1.2.3",
            "0.0000000001",
            "21600.000000001",
            "18446744073709551615.9",
        ] {
            assert!(MediaTime::decimal(text).is_err(), "{text}");
        }
        assert!(MediaTime {
            ticks: 1,
            timescale: 0
        }
        .ceil_samples(44100)
        .is_err());
        let timeline = media_timeline::AudioTimeline {
            sha256: String::new(),
            stream_index: 0,
            sample_rate: 44100,
            origin_ticks: 44100,
            time_base: media_timeline::TimeBase {
                numerator: 1,
                denominator: 44100,
            },
            duration_ticks: 88200,
            decoded_samples: 88200,
            decoded_frames: 22,
            continuous_sample_clock: true,
        };
        let interval = MediaInterval {
            start: MediaTime::decimal(".25").unwrap(),
            end: MediaTime::decimal("1.001").unwrap(),
        };
        let samples = interval.samples(&timeline).unwrap();
        assert_eq!((samples.start, samples.end), (11025, 44145));
        assert!(MediaInterval {
            start: interval.end,
            end: interval.start
        }
        .samples(&timeline)
        .is_err());
        assert!(MediaInterval {
            start: interval.start,
            end: MediaTime::decimal("2.1").unwrap()
        }
        .samples(&timeline)
        .is_err());
    }
}
