// SPDX-License-Identifier: Apache-2.0
use crate::{
    fail, media_probe, media_timeline::TimeBase, native_process, Cancellation, Result, Source,
};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, time::Duration};
const MAX_RECORDS: usize = 200000;
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Record {
    Packet {
        pts: Option<i64>,
        dts: Option<i64>,
        duration: Option<i64>,
        flags: Option<String>,
    },
    Frame {
        pts: Option<i64>,
        duration: Option<i64>,
        key_frame: Option<u8>,
    },
}
fn records<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Record>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<Record>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("at most 200000 packet/frame records")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut records = Vec::new();
            while let Some(record) = seq.next_element()? {
                if records.len() == MAX_RECORDS {
                    return Err(serde::de::Error::custom(
                        "Video timing record limit exceeded",
                    ));
                }
                records.push(record);
            }
            Ok(records)
        }
    }
    deserializer.deserialize_seq(Bounded)
}
#[derive(Deserialize)]
struct Report {
    #[serde(deserialize_with = "records")]
    packets_and_frames: Vec<Record>,
}
#[derive(Debug, Serialize)]
pub struct VideoTimeline {
    pub sha256: String,
    pub stream_index: u32,
    pub origin_ticks: i64,
    pub time_base: TimeBase,
    pub frame_ticks: i64,
    pub duration_ticks: i64,
    pub decoded_frames: u32,
    pub keyframe_indices: Vec<u32>,
    pub has_reordered_packets: bool,
    pub constant_frame_clock: bool,
}
struct Clock {
    step: i64,
    duration: i64,
    count: u32,
    keys: Vec<u32>,
    reordered: bool,
}
fn prove(
    report: Report,
    origin: i64,
    base: TimeBase,
    cancellation: &Cancellation,
) -> Result<Clock> {
    let invalid = || {
        fail("unsupported","Video trimming requires complete, continuous constant-rate packet and decoded-frame timestamps.")
    };
    let mut packets = Vec::new();
    let mut frames = Vec::new();
    for record in report.packets_and_frames {
        cancellation.check()?;
        match record {
            Record::Packet {
                pts,
                dts,
                duration,
                flags,
            } => {
                let pts = pts
                    .filter(|n| (-i64::MAX / 4..=i64::MAX / 4).contains(n))
                    .ok_or_else(invalid)?;
                let duration = duration
                    .filter(|n| *n > 0 && *n <= i64::MAX / 4)
                    .ok_or_else(invalid)?;
                packets.push((pts, dts, duration, flags.is_some_and(|s| s.contains('K'))));
            }
            Record::Frame {
                pts,
                duration,
                key_frame,
            } => {
                let pts = pts
                    .filter(|n| *n >= origin && *n <= i64::MAX / 4)
                    .ok_or_else(invalid)?;
                let duration = duration
                    .filter(|n| *n > 0 && *n <= i64::MAX / 4)
                    .ok_or_else(invalid)?;
                let key = key_frame.filter(|n| *n <= 1).ok_or_else(invalid)?;
                frames.push((pts, duration, key == 1));
            }
        }
    }
    if frames.is_empty() || frames.len() > 100000 || frames.len() != packets.len() {
        return Err(invalid());
    }
    packets.sort_by_key(|p| p.0);
    let step = frames[0].1;
    let mut keys = Vec::new();
    let mut reordered = false;
    for (index, (frame, packet)) in frames.iter().zip(&packets).enumerate() {
        cancellation.check()?;
        let expected = (index as i64)
            .checked_mul(step)
            .and_then(|n| origin.checked_add(n))
            .ok_or_else(invalid)?;
        if frame.0 != expected || packet.0 != expected || frame.1 != step || packet.2 != step {
            return Err(invalid());
        }
        if frame.2 && packet.3 {
            keys.push(index as u32);
        }
        reordered |= packet.1 != Some(packet.0);
    }
    let duration = (frames.len() as i64)
        .checked_mul(step)
        .ok_or_else(invalid)?;
    if duration as u128 * u128::from(base.numerator) > 21600 * u128::from(base.denominator) {
        return Err(fail("limit", "Video timeline exceeds six hours."));
    }
    Ok(Clock {
        step,
        duration,
        count: frames.len() as u32,
        keys,
        reordered,
    })
}
pub fn inspect(
    input: &Path,
    directory: &Path,
    cancellation: &Cancellation,
) -> Result<VideoTimeline> {
    let mut source = Source::open_with_limit(input, cancellation.clone(), 2 * 1024 * 1024 * 1024)?;
    let info = media_probe::inspect(source.snapshot.path(), directory, cancellation)?;
    if info.video_tracks != 1 {
        return Err(fail(
            "unsupported",
            "Choose a single video track for timeline inspection.",
        ));
    }
    let video = info
        .streams
        .iter()
        .find(|s| {
            s.codec_type == "video" && !s.disposition.as_ref().is_some_and(|d| d.attached_pic != 0)
        })
        .expect("counted video");
    let base = TimeBase::parse(video.time_base.as_deref())?;
    let origin = video.start_pts.unwrap_or(0);
    if !(0..=i64::MAX / 4).contains(&origin)
        || (origin as u128 * u128::from(base.numerator)) > 21600 * u128::from(base.denominator)
    {
        return Err(fail("unsupported", "Unsupported video source origin."));
    }
    let origin_seconds = origin as f64 * f64::from(base.numerator) / f64::from(base.denominator);
    let container = info
        .container_start_time
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| fail("unsupported", "Invalid container start time."))?;
    if (origin_seconds - container).abs() > 0.001 {
        return Err(fail(
            "unsupported",
            "The video and container use different start clocks.",
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
            "mov,matroska,webm,avi",
            "-select_streams",
            &video.index.to_string(),
            "-show_packets",
            "-show_frames",
            "-show_entries",
            "packet=pts,dts,duration,flags:frame=pts,duration,key_frame",
            "-of",
            "json=compact=1",
        ])
        .arg(source.snapshot.path());
    let bytes = native_process::run(
        command,
        cancellation,
        Duration::from_secs(120),
        32 * 1024 * 1024,
    )?;
    let report: Report = serde_json::from_slice(&bytes)
        .map_err(|_| fail("unsupported", "Invalid or oversized video timing report."))?;
    let clock = prove(report, origin, base, cancellation)?;
    source.check(input)?;
    Ok(VideoTimeline {
        sha256: source.hash,
        stream_index: video.index,
        origin_ticks: origin,
        time_base: base,
        frame_ticks: clock.step,
        duration_ticks: clock.duration,
        decoded_frames: clock.count,
        keyframe_indices: clock.keys,
        has_reordered_packets: clock.reordered,
        constant_frame_clock: true,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn report(second: i64, reordered: bool) -> Report {
        Report {
            packets_and_frames: vec![
                Record::Packet {
                    pts: Some(0),
                    dts: Some(if reordered { -10 } else { 0 }),
                    duration: Some(10),
                    flags: Some("K__".into()),
                },
                Record::Frame {
                    pts: Some(0),
                    duration: Some(10),
                    key_frame: Some(1),
                },
                Record::Packet {
                    pts: Some(second),
                    dts: Some(second),
                    duration: Some(10),
                    flags: Some("___".into()),
                },
                Record::Frame {
                    pts: Some(second),
                    duration: Some(10),
                    key_frame: Some(0),
                },
            ],
        }
    }
    #[test]
    fn packet_frame_records_are_bounded_during_parsing() {
        let text = format!(
            "{{\"packets_and_frames\":[{}]}}",
            vec![r#"{"type":"frame"}"#; MAX_RECORDS + 1].join(",")
        );
        let error = serde_json::from_str::<Report>(&text)
            .err()
            .expect("oversized report rejected");
        assert!(error.to_string().contains("record limit"));
    }
    #[test]
    fn decoded_frames_must_match_continuous_packet_clock() {
        let base = TimeBase {
            numerator: 1,
            denominator: 100,
        };
        let cancel = Cancellation::default();
        let clock = prove(report(10, true), 0, base, &cancel).unwrap();
        assert_eq!((clock.count, clock.step, clock.duration), (2, 10, 20));
        assert_eq!(clock.keys, vec![0]);
        assert!(clock.reordered);
        assert!(prove(report(11, false), 0, base, &cancel).is_err());
        assert!(prove(report(9, false), 0, base, &cancel).is_err());
        let mut missing = report(10, false);
        missing.packets_and_frames.pop();
        assert!(prove(missing, 0, base, &cancel).is_err());
    }
}
