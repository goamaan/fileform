// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, media_probe, media_timeline::TimeBase, media_video, native_process, png_pipeline,
    Cancellation, MediaTime, Result, Source,
};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::{fs::File, io::BufReader, path::Path, time::Duration};
#[derive(Debug, Serialize)]
pub struct Poster {
    pub output: std::path::PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: String,
    pub width: u32,
    pub height: u32,
    pub requested_time: MediaTime,
    pub realized_time: MediaTime,
    pub frame_index: u32,
}
#[derive(Deserialize)]
struct Frame {
    pts: i64,
    duration: i64,
}
fn frames<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<Frame>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<Frame>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("at most 100000 frame timestamps")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(frame) = seq.next_element()? {
                if values.len() == 100000 {
                    return Err(serde::de::Error::custom("Frame limit exceeded"));
                }
                values.push(frame);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Bounded)
}
#[derive(Deserialize)]
struct Frames {
    #[serde(deserialize_with = "frames")]
    frames: Vec<Frame>,
}
fn frame_at(
    time: MediaTime,
    frames: &[Frame],
    base: TimeBase,
    origin: i64,
) -> Result<(u32, MediaTime)> {
    time.validate()?;
    if frames.is_empty() || !(0..=i64::MAX / 4).contains(&origin) {
        return Err(fail("unsupported", "No measurable video clock."));
    }
    let mut selected = None;
    let mut previous = None;
    let mut end = 0u128;
    for (index, frame) in frames.iter().enumerate() {
        if frame.pts < origin
            || frame.pts > i64::MAX / 4
            || !(1..=i64::MAX / 4).contains(&frame.duration)
            || previous.is_some_and(|pts| pts >= frame.pts)
        {
            return Err(fail(
                "unsupported",
                "Video frame timestamps are incomplete or unordered.",
            ));
        }
        let start = (frame.pts - origin) as u128 * u128::from(base.numerator);
        end = ((frame.pts - origin + frame.duration) as u128 * u128::from(base.numerator)).max(end);
        if end > 21600 * u128::from(base.denominator) {
            return Err(fail("limit", "Poster timeline exceeds six hours."));
        }
        if start * u128::from(time.timescale)
            <= u128::from(time.ticks) * u128::from(base.denominator)
        {
            selected = Some((
                index as u32,
                MediaTime {
                    ticks: u64::try_from(start)
                        .map_err(|_| fail("limit", "Frame clock overflow."))?,
                    timescale: base.denominator,
                },
            ));
        }
        previous = Some(frame.pts);
    }
    if u128::from(time.ticks) * u128::from(base.denominator) >= end * u128::from(time.timescale) {
        return Err(fail(
            "invalid_request",
            "Choose a poster time before the recording ends.",
        ));
    }
    selected.ok_or_else(|| fail("invalid_request", "No frame at the selected time."))
}
pub fn poster(
    input: &Path,
    output: &Path,
    directory: &Path,
    time: MediaTime,
    maximum: u32,
    cancel: &Cancellation,
) -> Result<Poster> {
    time.validate()?;
    if !(1..=4096).contains(&maximum)
        || !output
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        return Err(fail(
            "invalid_request",
            "Choose PNG output and a preview bound from 1 to 4096 pixels.",
        ));
    }
    if output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let mut source = Source::open_with_limit(input, cancel.clone(), 2 * 1024 * 1024 * 1024)?;
    let info = media_probe::inspect(source.snapshot.path(), directory, cancel)?;
    if info.video_tracks != 1 {
        return Err(fail("unsupported", "Choose one video track for a poster."));
    }
    let video = info
        .streams
        .iter()
        .find(|s| {
            s.codec_type == "video" && !s.disposition.as_ref().is_some_and(|d| d.attached_pic != 0)
        })
        .expect("counted video");
    if matches!(
        video.color_transfer.as_deref(),
        Some("smpte2084" | "arib-std-b67")
    ) || !matches!(
        video.pix_fmt.as_deref(),
        Some(
            "yuv420p"
                | "yuv422p"
                | "yuv444p"
                | "yuvj420p"
                | "yuvj422p"
                | "yuvj444p"
                | "nv12"
                | "nv21"
                | "gray"
                | "rgba"
                | "argb"
                | "bgra"
                | "abgr"
                | "rgb24"
                | "bgr24"
                | "rgb0"
                | "bgr0"
                | "0rgb"
                | "0bgr"
                | "yuva420p"
                | "yuva422p"
                | "yuva444p"
        )
    ) {
        return Err(fail("unsupported","This poster route needs supported 8-bit SDR video. Extended-color preview support is pending."));
    }
    let base = TimeBase::parse(video.time_base.as_deref())?;
    let origin = video.start_pts.unwrap_or(0);
    let mut probe = Command::new(directory.canonicalize()?.join(if cfg!(windows) {
        "bin/ffprobe.exe"
    } else {
        "bin/ffprobe"
    }));
    probe.env_clear();
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            probe.env("SystemRoot", root);
        }
    }
    probe
        .args([
            "-v",
            "error",
            "-max_alloc",
            "268435456",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            "mov,matroska,webm,avi",
            "-threads",
            "2",
            "-select_streams",
            &video.index.to_string(),
            "-show_frames",
            "-show_entries",
            "frame=pts,duration",
            "-of",
            "json=compact=1",
        ])
        .arg(source.snapshot.path());
    let report = native_process::run(probe, cancel, Duration::from_secs(120), 16 * 1024 * 1024)?;
    let report: Frames = serde_json::from_slice(&report)
        .map_err(|_| fail("unsupported", "Invalid or oversized video frame timeline."))?;
    let (index, realized) = frame_at(time, &report.frames, base, origin)?;
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
    let mut command = media_video::command(&executable);
    command.arg("-i").arg(source.snapshot.path()).args(["-map",&format!("0:{}",video.index),"-an","-sn","-dn","-vf",&format!("select=eq(n\\,{index}),scale={maximum}:{maximum}:force_original_aspect_ratio=decrease"),"-frames:v","1","-c:v","png","-pix_fmt","rgba","-f","image2","-update","1","-y"]).arg(temporary.path());
    native_process::run_with_output_limit(
        command,
        cancel,
        Duration::from_secs(120),
        512 * 1024,
        Some((temporary.path(), 80 * 1024 * 1024)),
    )?;
    let image =
        png_pipeline::decode_bounded(BufReader::new(File::open(temporary.path())?), maximum)?;
    let (width, height) = image.pixels.dimensions();
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 80 * 1024 * 1024)?;
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
    Ok(Poster {
        output: output.to_path_buf(),
        bytes,
        sha256,
        source_sha256: source.hash,
        width,
        height,
        requested_time: time,
        realized_time: realized,
        frame_index: index,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poster_uses_preceding_variable_frame_and_excludes_end_boundary() {
        let frames = vec![
            Frame {
                pts: 0,
                duration: 10,
            },
            Frame {
                pts: 10,
                duration: 20,
            },
            Frame {
                pts: 30,
                duration: 10,
            },
        ];
        let base = TimeBase {
            numerator: 1,
            denominator: 100,
        };
        assert_eq!(
            frame_at(MediaTime::decimal(".25").unwrap(), &frames, base, 0)
                .unwrap()
                .0,
            1
        );
        assert_eq!(
            frame_at(MediaTime::decimal(".35").unwrap(), &frames, base, 0)
                .unwrap()
                .0,
            2
        );
        assert!(frame_at(MediaTime::decimal(".4").unwrap(), &frames, base, 0).is_err());
    }
}
