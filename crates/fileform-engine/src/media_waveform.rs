// SPDX-License-Identifier: Apache-2.0
use crate::{fail, media_probe, media_timeline, native_process, Cancellation, Result, Source};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path, process::Command, time::Duration};
#[derive(Debug, Serialize, Default)]
pub struct Envelope {
    pub minimum: Vec<f64>,
    pub maximum: Vec<f64>,
}
#[derive(Debug, Serialize)]
pub struct Waveform {
    pub sha256: String,
    pub stream_index: u32,
    pub sample_rate: u32,
    pub sample_count: u64,
    pub samples_per_bucket: u64,
    pub source_origin_ticks: i64,
    pub source_time_base: media_timeline::TimeBase,
    pub channels: Vec<Envelope>,
}
fn append(
    values: BTreeMap<String, String>,
    channels: &mut [Envelope],
    covered: &mut u64,
    samples: u64,
    per_bucket: u64,
    bins: u32,
) -> Result<()> {
    let invalid = || {
        fail(
            "verification",
            "Waveform measurements are incomplete or invalid.",
        )
    };
    if values.len() != channels.len() * 2 + 1 || channels[0].minimum.len() >= bins as usize {
        return Err(invalid());
    }
    let count = values
        .get("Overall.Number_of_samples")
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0 && v.fract() == 0.0 && *v <= samples as f64)
        .ok_or_else(invalid)? as u64;
    if count != (samples - *covered).min(per_bucket) {
        return Err(invalid());
    }
    for (index, channel) in channels.iter_mut().enumerate() {
        let read = |suffix: &str| {
            values
                .get(&format!("{}.{suffix}", index + 1))
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && v.abs() <= 1000000.0)
                .ok_or_else(invalid)
        };
        let low = read("Min_level")?;
        let high = read("Max_level")?;
        if low > high {
            return Err(invalid());
        }
        channel.minimum.push(low);
        channel.maximum.push(high);
    }
    *covered += count;
    Ok(())
}
fn parse(
    bytes: &[u8],
    count: usize,
    samples: u64,
    per_bucket: u64,
    bins: u32,
    cancel: &Cancellation,
) -> Result<Vec<Envelope>> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| fail("verification", "Invalid waveform text."))?;
    let mut channels: Vec<_> = (0..count).map(|_| Envelope::default()).collect();
    let mut values = None;
    let mut covered = 0;
    for line in text.lines() {
        cancel.check()?;
        if let Some(frame) = line.strip_prefix("frame:") {
            if let Some(previous) = values.take() {
                append(
                    previous,
                    &mut channels,
                    &mut covered,
                    samples,
                    per_bucket,
                    bins,
                )?;
            }
            let index = frame
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<usize>().ok());
            if index != Some(channels[0].minimum.len()) {
                return Err(fail("verification", "Waveform bucket order changed."));
            }
            values = Some(BTreeMap::new());
        } else if let Some(value) = line.strip_prefix("lavfi.astats.") {
            let (key, value) = value
                .split_once('=')
                .ok_or_else(|| fail("verification", "Malformed waveform measurement."))?;
            let row = values
                .as_mut()
                .ok_or_else(|| fail("verification", "Waveform bucket header is missing."))?;
            if row.insert(key.to_string(), value.to_string()).is_some() || row.len() > count * 2 + 1
            {
                return Err(fail("verification", "Duplicate or excess waveform fields."));
            }
        }
    }
    if let Some(last) = values {
        append(last, &mut channels, &mut covered, samples, per_bucket, bins)?;
    }
    if covered != samples || channels[0].minimum.is_empty() {
        return Err(fail(
            "verification",
            "Waveform does not cover the complete measured recording.",
        ));
    }
    Ok(channels)
}
pub fn waveform(
    input: &Path,
    directory: &Path,
    bins: u32,
    cancel: &Cancellation,
) -> Result<Waveform> {
    if !(16..=4096).contains(&bins) {
        return Err(fail(
            "invalid_request",
            "Choose 16 to 4096 waveform buckets.",
        ));
    }
    let mut source = Source::open_with_limit(input, cancel.clone(), 2 * 1024 * 1024 * 1024)?;
    let timeline = media_timeline::inspect(source.snapshot.path(), directory, cancel)?;
    let info = media_probe::inspect(source.snapshot.path(), directory, cancel)?;
    let count = info
        .streams
        .iter()
        .find(|s| s.index == timeline.stream_index)
        .and_then(|s| s.channels)
        .filter(|n| (1..=8).contains(n))
        .ok_or_else(|| {
            fail(
                "unsupported",
                "Waveform preview supports one to eight channels.",
            )
        })?;
    let samples = (timeline.duration_ticks as u128
        * u128::from(timeline.time_base.numerator)
        * u128::from(timeline.sample_rate))
    .div_ceil(u128::from(timeline.time_base.denominator));
    let samples = u64::try_from(samples)
        .map_err(|_| fail("limit", "Waveform sample count exceeds its limit."))?;
    if samples == 0 || samples > timeline.decoded_samples {
        return Err(fail(
            "unsupported",
            "Decoded audio does not cover the measured waveform duration.",
        ));
    }
    let per_bucket = samples.div_ceil(u64::from(bins));
    if per_bucket * u64::from(count) * 4 > 128 * 1024 * 1024 {
        return Err(fail(
            "limit",
            "Choose more buckets for this recording's rate and channel count.",
        ));
    }
    let executable = directory.canonicalize()?.join(if cfg!(windows) {
        "bin/ffmpeg.exe"
    } else {
        "bin/ffmpeg"
    });
    let mut command = Command::new(executable);
    command.env_clear();
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    let filter=format!("aformat=sample_fmts=flt,atrim=end_sample={samples},asetpts=N/SR/TB,asetnsamples=n={per_bucket}:p=0,astats=metadata=1:reset=1:measure_perchannel=Min_level+Max_level:measure_overall=Number_of_samples,ametadata=mode=print:file=-");
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
            "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac",
            "-threads",
            "2",
            "-i",
        ])
        .arg(source.snapshot.path())
        .args([
            "-map",
            &format!("0:{}", timeline.stream_index),
            "-vn",
            "-sn",
            "-dn",
            "-af",
            &filter,
            "-f",
            "null",
            "-",
        ]);
    let bytes = native_process::run(command, cancel, Duration::from_secs(300), 8 * 1024 * 1024)?;
    let channels = parse(&bytes, count as usize, samples, per_bucket, bins, cancel)?;
    source.check(input)?;
    let result = Waveform {
        sha256: source.hash,
        stream_index: timeline.stream_index,
        sample_rate: timeline.sample_rate,
        sample_count: samples,
        samples_per_bucket: per_bucket,
        source_origin_ticks: timeline.origin_ticks,
        source_time_base: timeline.time_base,
        channels,
    };
    if serde_json::to_vec(&serde_json::to_value(&result)?)?.len() > 900000 {
        return Err(fail(
            "limit",
            "Choose fewer waveform buckets to keep the preview bounded.",
        ));
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waveform_parser_rejects_padding_missing_channels_and_nonfinite_values() {
        let text=b"frame:0 pts:0\nlavfi.astats.1.Min_level=-0.5\nlavfi.astats.1.Max_level=0.25\nlavfi.astats.Overall.Number_of_samples=3.000000\n";
        let channels = parse(text, 1, 3, 3, 16, &Cancellation::default()).unwrap();
        assert_eq!(channels[0].minimum, vec![-0.5]);
        assert!(parse(text, 1, 2, 3, 16, &Cancellation::default()).is_err());
        assert!(parse(text, 2, 3, 3, 16, &Cancellation::default()).is_err());
        let invalid = String::from_utf8_lossy(text).replace("-0.5", "NaN");
        assert!(parse(invalid.as_bytes(), 1, 3, 3, 16, &Cancellation::default()).is_err());
    }
}
