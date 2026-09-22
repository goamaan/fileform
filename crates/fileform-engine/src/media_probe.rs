// SPDX-License-Identifier: Apache-2.0
use crate::{fail, media_pack, native_process, Cancellation, Result, Source};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, time::Duration};

#[derive(Debug, Deserialize, Serialize)]
pub struct Stream {
    pub index: u32,
    pub codec_type: String,
    pub codec_name: Option<String>,
    pub duration: Option<String>,
    pub duration_ts: Option<i64>,
    pub nb_frames: Option<String>,
    pub time_base: Option<String>,
    pub start_time: Option<String>,
    pub color_primaries: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    pub side_data_list: Option<Vec<SideData>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub channels: Option<u32>,
    pub sample_rate: Option<String>,
    pub bits_per_sample: Option<u32>,
    pub bits_per_raw_sample: Option<String>,
    pub pix_fmt: Option<String>,
    pub color_transfer: Option<String>,
    pub sample_aspect_ratio: Option<String>,
    pub disposition: Option<Disposition>,
}
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct SideData {
    pub rotation: Option<i32>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Disposition {
    pub attached_pic: u8,
}
#[derive(Deserialize)]
struct Format {
    duration: Option<String>,
    format_name: String,
}
#[derive(Deserialize)]
struct Probe {
    streams: Vec<Stream>,
    format: Format,
}
#[derive(Debug, Serialize)]
pub struct MediaInspection {
    pub sha256: String,
    pub bytes: u64,
    pub duration_seconds: f64,
    pub format: String,
    pub streams: Vec<Stream>,
    pub audio_tracks: usize,
    pub video_tracks: usize,
}
fn parse(bytes: &[u8]) -> Result<(Probe, f64, usize, usize)> {
    let probe: Probe = serde_json::from_slice(bytes)
        .map_err(|_| fail("unsupported", "Invalid media inspection response."))?;
    let duration = probe
        .format
        .duration
        .as_deref()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= 21600.0)
        .ok_or_else(|| {
            fail(
                "unsupported",
                "Choose media with a known duration of up to six hours.",
            )
        })?;
    if probe.streams.len() > 64 {
        return Err(fail("limit", "The media contains too many streams."));
    }
    let mut audio = 0;
    let mut video = 0;
    for stream in &probe.streams {
        if stream.codec_type == "audio" {
            audio += 1;
        }
        if stream.codec_type == "video"
            && !stream
                .disposition
                .as_ref()
                .is_some_and(|d| d.attached_pic != 0)
        {
            let (width, height) = (stream.width.unwrap_or(0), stream.height.unwrap_or(0));
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 80_000_000 {
                return Err(fail("limit", "Unsupported media picture dimensions."));
            }
            video += 1;
        }
    }
    if audio + video == 0 {
        return Err(fail("unsupported", "No audio or video tracks were found."));
    }
    Ok((probe, duration, audio, video))
}
pub fn inspect(
    input: &Path,
    directory: &Path,
    cancellation: &Cancellation,
) -> Result<MediaInspection> {
    media_pack::verify(directory, cancellation)?;
    let root = directory.canonicalize()?;
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let executable = root.join(format!("bin/ffprobe{suffix}"));
    let mut source = Source::open_with_limit(input, cancellation.clone(), 2 * 1024 * 1024 * 1024)?;
    let mut command = Command::new(executable);
    command.env_clear();
    #[cfg(windows)]
    {
        if let Some(system) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system);
        }
    }
    command.args(["-v","error","-max_alloc","268435456","-threads","2","-protocol_whitelist","file,pipe","-format_whitelist","mov,matroska,webm,avi,wav,flac,mp3,ogg,aac","-show_entries","format=format_name,duration:stream=index,codec_type,codec_name,duration,duration_ts,nb_frames,time_base,start_time,width,height,channels,sample_rate,bits_per_sample,bits_per_raw_sample,pix_fmt,color_transfer,color_primaries,color_space,color_range,sample_aspect_ratio:stream_disposition=attached_pic:stream_side_data=rotation","-of","json"]);
    command.arg(source.snapshot.path());
    let bytes = native_process::run(command, cancellation, Duration::from_secs(30), 512 * 1024)?;
    let (probe, duration_seconds, audio_tracks, video_tracks) = parse(&bytes)?;
    source.check(input)?;
    Ok(MediaInspection {
        sha256: source.hash,
        bytes: source.input.metadata()?.len(),
        duration_seconds,
        format: probe.format.format_name,
        streams: probe.streams,
        audio_tracks,
        video_tracks,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_and_cover_art_routing() {
        let sample = serde_json::json!({"format":{"format_name":"mp3","duration":"2.5"},"streams":[{"index":0,"codec_type":"audio"},{"index":1,"codec_type":"video","disposition":{"attached_pic":1}}]});
        let (_, duration, audio, video) = parse(&serde_json::to_vec(&sample).unwrap()).unwrap();
        assert_eq!((duration, audio, video), (2.5, 1, 0));
        for duration in ["NaN", "inf", "0", "21601"] {
            let mut sample = sample.clone();
            sample["format"]["duration"] = duration.into();
            assert!(parse(&serde_json::to_vec(&sample).unwrap()).is_err());
        }
    }
}
