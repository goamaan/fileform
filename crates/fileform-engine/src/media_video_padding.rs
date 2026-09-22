// SPDX-License-Identifier: Apache-2.0
use crate::{fail, media_video, native_process, Cancellation, Result};
use serde::Deserialize;
use std::{path::Path, process::Command, time::Duration};
use tempfile::NamedTempFile;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Padding {
    pub width: u32,
    pub height: u32,
    pub encoded_width: u32,
    pub encoded_height: u32,
}
#[derive(Debug, Deserialize, PartialEq)]
struct Geometry {
    width: u32,
    height: u32,
    crop_left: u32,
    crop_right: u32,
    crop_top: u32,
    crop_bottom: u32,
}
#[derive(Deserialize)]
struct Report {
    frames: Vec<Geometry>,
}
impl Padding {
    pub fn required(width: u32, height: u32) -> Option<Self> {
        (width < 64 || height < 48).then_some(Self {
            width,
            height,
            encoded_width: width.max(128),
            encoded_height: height.max(96),
        })
    }
    fn crop(self, frame: &Geometry) -> Result<(u32, u32)> {
        let width = frame
            .width
            .checked_sub(frame.crop_left)
            .and_then(|v| v.checked_sub(frame.crop_right));
        let height = frame
            .height
            .checked_sub(frame.crop_top)
            .and_then(|v| v.checked_sub(frame.crop_bottom));
        if width != Some(self.encoded_width)
            || height != Some(self.encoded_height)
            || self.width > self.encoded_width
            || self.height > self.encoded_height
        {
            return Err(fail(
                "verification",
                "Encoder padding geometry could not be verified.",
            ));
        }
        let right = frame
            .crop_right
            .checked_add(self.encoded_width - self.width)
            .ok_or_else(|| fail("verification", "Invalid encoder crop."))?;
        let bottom = frame
            .crop_bottom
            .checked_add(self.encoded_height - self.height)
            .ok_or_else(|| fail("verification", "Invalid encoder crop."))?;
        Ok((right, bottom))
    }
}
/// Restore the requested visible rectangle after encoding a padded small frame.
/// The second remux rebuilds MP4/MOV display dimensions from the corrected SPS.
pub(crate) fn restore(
    input: NamedTempFile,
    directory: &Path,
    muxer: &str,
    padding: Padding,
    cancellation: &Cancellation,
    timeout: Duration,
) -> Result<NamedTempFile> {
    let root = directory.canonicalize()?;
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let ffmpeg = root.join(format!("bin/ffmpeg{suffix}"));
    let mut probe = Command::new(root.join(format!("bin/ffprobe{suffix}")));
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
            "mov",
            "-apply_cropping",
            "0",
            "-read_intervals",
            "%+#8",
            "-select_streams",
            "v:0",
            "-show_frames",
            "-show_entries",
            "frame=width,height,crop_left,crop_right,crop_top,crop_bottom",
            "-of",
            "json",
        ])
        .arg(input.path());
    let bytes = native_process::run(probe, cancellation, Duration::from_secs(30), 256 * 1024)?;
    let report: Report = serde_json::from_slice(&bytes)
        .map_err(|_| fail("verification", "Invalid encoder crop report."))?;
    let first = report
        .frames
        .first()
        .ok_or_else(|| fail("verification", "Encoder returned no picture."))?;
    if report.frames.iter().any(|frame| frame != first) {
        return Err(fail(
            "verification",
            "Encoder crop geometry changed between frames.",
        ));
    }
    let (right, bottom) = padding.crop(first)?;
    let parent = input.path().parent().expect("temporary file parent");
    let cropped = NamedTempFile::new_in(parent)?;
    let mut crop = media_video::command(&ffmpeg);
    crop.arg("-i")
        .arg(input.path())
        .args([
            "-map",
            "0",
            "-c",
            "copy",
            "-bsf:v",
            &format!("h264_metadata=crop_right={right}:crop_bottom={bottom}"),
            "-f",
            muxer,
            "-y",
        ])
        .arg(cropped.path());
    native_process::run_with_output_limit(
        crop,
        cancellation,
        timeout,
        512 * 1024,
        Some((cropped.path(), media_video::MAX_OUTPUT)),
    )?;
    let output = NamedTempFile::new_in(parent)?;
    let mut remux = media_video::command(&ffmpeg);
    remux
        .arg("-i")
        .arg(cropped.path())
        .args([
            "-map",
            "0",
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            "-f",
            muxer,
            "-y",
        ])
        .arg(output.path());
    native_process::run_with_output_limit(
        remux,
        cancellation,
        timeout,
        512 * 1024,
        Some((output.path(), media_video::MAX_OUTPUT)),
    )?;
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cropping_preserves_existing_encoder_padding() {
        let padding = Padding::required(32, 24).unwrap();
        assert_eq!((padding.encoded_width, padding.encoded_height), (128, 96));
        let frame = Geometry {
            width: 192,
            height: 96,
            crop_left: 0,
            crop_right: 64,
            crop_top: 0,
            crop_bottom: 0,
        };
        assert_eq!(padding.crop(&frame).unwrap(), (160, 72));
        assert!(Padding::required(64, 48).is_none());
        assert!(Padding::required(48, 64).is_some());
        assert!(padding
            .crop(&Geometry {
                crop_right: 193,
                ..frame
            })
            .is_err());
    }
}
