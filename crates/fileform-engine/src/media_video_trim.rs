// SPDX-License-Identifier: Apache-2.0
use crate::{
    fail, media_probe, media_timeline, media_video, media_video_timeline, Cancellation,
    MediaInterval, MediaTime, Result, SampleRange, Source, VideoEncoding,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoTrimOptions {
    pub interval: MediaInterval,
    #[serde(default)]
    pub mute_audio: bool,
}
#[derive(Debug, Serialize)]
pub struct VideoTrimReceipt {
    #[serde(flatten)]
    pub video: media_video::VideoReceipt,
    pub requested_interval: MediaInterval,
    pub realized_interval: MediaInterval,
    pub start_frame: u32,
    pub end_frame: u32,
    pub audio_samples: Option<SampleRange>,
    pub source_origin_ticks: i64,
    pub source_time_base: media_timeline::TimeBase,
    pub muted_audio: bool,
}
fn frame_range(
    interval: MediaInterval,
    timeline: &media_video_timeline::VideoTimeline,
) -> Result<(u32, u32, MediaInterval)> {
    interval.validate()?;
    let base = timeline.time_base;
    if u128::from(interval.end.ticks) * u128::from(base.denominator)
        > timeline.duration_ticks as u128
            * u128::from(base.numerator)
            * u128::from(interval.end.timescale)
    {
        return Err(fail(
            "invalid_request",
            "The selected interval exceeds the video duration.",
        ));
    }
    let frame = |time: MediaTime| -> Result<u32> {
        let n = (u128::from(time.ticks) * u128::from(base.denominator)).div_ceil(
            u128::from(time.timescale) * u128::from(base.numerator) * timeline.frame_ticks as u128,
        );
        u32::try_from(n).map_err(|_| {
            fail(
                "invalid_request",
                "Frame position exceeds the supported range.",
            )
        })
    };
    let start = frame(interval.start)?;
    let end = frame(interval.end)?;
    if start >= end || end > timeline.decoded_frames {
        return Err(fail(
            "invalid_request",
            "The interval contains no complete frame onsets.",
        ));
    }
    let time = |index: u32| -> Result<MediaTime> {
        Ok(MediaTime {
            ticks: u64::try_from(
                u128::from(index) * timeline.frame_ticks as u128 * u128::from(base.numerator),
            )
            .map_err(|_| fail("limit", "Frame clock exceeds supported range."))?,
            timescale: base.denominator,
        })
    };
    Ok((
        start,
        end,
        MediaInterval {
            start: time(start)?,
            end: time(end)?,
        },
    ))
}
pub fn trim(
    input: &Path,
    output: &Path,
    directory: &Path,
    options: VideoTrimOptions,
    expected: Option<&str>,
    cancellation: &Cancellation,
) -> Result<VideoTrimReceipt> {
    options.interval.validate()?;
    let mut source = Source::open_with_limit(input, cancellation.clone(), 2 * 1024 * 1024 * 1024)?;
    if expected.is_some_and(|hash| hash != source.hash) {
        return Err(fail("source_changed", "The inspected source changed."));
    }
    let timeline = media_video_timeline::inspect(source.snapshot.path(), directory, cancellation)?;
    let (start, end, realized) = frame_range(options.interval, &timeline)?;
    let info = media_probe::inspect(source.snapshot.path(), directory, cancellation)?;
    let audio_samples = if info.audio_tracks > 0 && !options.mute_audio {
        let audio = media_timeline::inspect(source.snapshot.path(), directory, cancellation)?;
        let video_clock = timeline.origin_ticks as u128
            * u128::from(timeline.time_base.numerator)
            * u128::from(audio.time_base.denominator);
        let audio_clock = audio.origin_ticks as u128
            * u128::from(audio.time_base.numerator)
            * u128::from(timeline.time_base.denominator);
        if video_clock.abs_diff(audio_clock) * u128::from(audio.sample_rate)
            > u128::from(timeline.time_base.denominator) * u128::from(audio.time_base.denominator)
        {
            return Err(fail("unsupported","Video and audio start on different clocks. Normalize synchronization or explicitly mute audio."));
        }
        let first = realized.start.ceil_samples(audio.sample_rate)?;
        let last = realized.end.ceil_samples(audio.sample_rate)?;
        if first >= last || last > audio.decoded_samples {
            return Err(fail(
                "unsupported",
                "The audio does not cover the selected picture interval.",
            ));
        }
        Some(SampleRange {
            start: first,
            end: last,
        })
    } else {
        None
    };
    source.check(input)?;
    let duration_seconds =
        (realized.end.ticks - realized.start.ticks) as f64 / f64::from(realized.end.timescale);
    let operation = media_video::VideoOperation {
        encoding: Some(VideoEncoding::default()),
        encode_audio: true,
        mute_audio: options.mute_audio,
        selection: Some(media_video::VideoSelection {
            start_frame: start,
            end_frame: end,
            audio_samples,
            duration_seconds,
        }),
    };
    let video = media_video::transform(
        input,
        output,
        directory,
        Some(&source.hash),
        operation,
        cancellation,
    )?;
    Ok(VideoTrimReceipt {
        video,
        requested_interval: options.interval,
        realized_interval: realized,
        start_frame: start,
        end_frame: end,
        audio_samples,
        source_origin_ticks: timeline.origin_ticks,
        source_time_base: timeline.time_base,
        muted_audio: options.mute_audio,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_onsets_are_selected_with_rational_ceil_boundaries() {
        let timeline = media_video_timeline::VideoTimeline {
            sha256: String::new(),
            stream_index: 0,
            origin_ticks: 0,
            time_base: media_timeline::TimeBase {
                numerator: 1,
                denominator: 10240,
            },
            frame_ticks: 1024,
            duration_ticks: 20480,
            decoded_frames: 20,
            keyframe_indices: vec![0, 12],
            has_reordered_packets: false,
            constant_frame_clock: true,
        };
        let interval = MediaInterval {
            start: MediaTime::decimal(".35").unwrap(),
            end: MediaTime::decimal("1.21").unwrap(),
        };
        let (start, end, realized) = frame_range(interval, &timeline).unwrap();
        assert_eq!((start, end), (4, 13));
        assert_eq!((realized.start.ticks, realized.end.ticks), (4096, 13312));
        assert!(frame_range(
            MediaInterval {
                start: MediaTime::decimal(".01").unwrap(),
                end: MediaTime::decimal(".02").unwrap()
            },
            &timeline
        )
        .is_err());
    }
}
