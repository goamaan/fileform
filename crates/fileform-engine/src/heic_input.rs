// SPDX-License-Identifier: Apache-2.0
use crate::{
    fail, image_input::DecodedImage, image_orientation, native_pack, native_process, Cancellation,
    Result,
};
use serde::Serialize;
use std::{
    io::{BufRead, BufReader, Read, Seek},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
#[derive(Debug, Serialize)]
pub struct HeicPackVerification {
    pub version: String,
    pub architecture: String,
}
pub fn verify(directory: &Path, cancel: &Cancellation) -> Result<HeicPackVerification> {
    let libraries = if cfg!(windows) {
        ["heif.dll", "libde265.dll"]
    } else {
        ["libheif.1.dylib", "libde265.0.dylib"]
    };
    let pack = native_pack::verify_with_libraries(
        directory,
        cancel,
        "app.fileform.heic",
        &["fileform-heic-decode"],
        &libraries,
        true,
    )?;
    Ok(HeicPackVerification {
        version: pack.version,
        architecture: pack.architecture,
    })
}
fn directory() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("FILEFORM_HEIC_PACK") {
        return Ok(path.into());
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            for path in [parent.join("HEICPack"), parent.join("../HEICPack")] {
                if path.join("manifest.json").is_file() {
                    return Ok(path);
                }
            }
        }
    }
    Err(fail(
        "engine_unavailable",
        "The verified HEIC decoder pack is missing.",
    ))
}
pub(crate) fn decode(input: &Path, cancel: &Cancellation) -> Result<DecodedImage> {
    let directory = directory()?;
    verify(&directory, cancel)?;
    let root = directory.canonicalize()?;
    let mut command = Command::new(root.join(if cfg!(windows) {
        "bin/fileform-heic-decode.exe"
    } else {
        "bin/fileform-heic-decode"
    }));
    command.env_clear();
    #[cfg(windows)]
    if let Some(system) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system);
    }
    let working = tempfile::tempdir()?;
    command.current_dir(working.path()).arg(input);
    let mut raster = tempfile::NamedTempFile::new_in(working.path())?;
    native_process::run_to_file(
        command,
        cancel,
        Duration::from_secs(120),
        &mut raster,
        80_000_000 * 4 + 1024 * 1024 + 65536 + 256,
    )?;
    raster.as_file_mut().rewind()?;
    read(BufReader::new(raster.as_file_mut()), cancel)
}
fn invalid() -> crate::Failure {
    fail(
        "verification",
        "The HEIC decoder returned invalid pixel or metadata framing.",
    )
}
fn line(reader: &mut impl BufRead) -> Result<Vec<u8>> {
    let mut value = Vec::new();
    loop {
        let mut byte = [0];
        reader.read_exact(&mut byte)?;
        if byte[0] == b'\n' {
            return Ok(value);
        }
        if value.len() == 256 {
            return Err(invalid());
        }
        value.push(byte[0]);
    }
}
fn bytes(reader: &mut impl Read, count: usize, cancel: &Cancellation) -> Result<Vec<u8>> {
    let mut value = Vec::new();
    value
        .try_reserve_exact(count)
        .map_err(|_| fail("limit", "Not enough memory for HEIC samples."))?;
    value.resize(count, 0);
    for block in value.chunks_mut(65536) {
        cancel.check()?;
        reader.read_exact(block)?;
    }
    Ok(value)
}
fn color(primaries: u16, transfer: u16) -> Result<Option<Vec<u8>>> {
    if matches!(transfer, 16 | 18) {
        return Err(fail(
            "unsupported",
            "HDR HEIC needs a preservation workflow.",
        ));
    }
    if matches!(primaries, 0 | 2) && matches!(transfer, 0 | 2) {
        return Ok(None);
    }
    if !matches!(primaries, 1 | 4 | 5 | 6 | 7 | 8 | 9 | 11 | 12 | 22)
        || !matches!(
            transfer,
            1 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15
        )
    {
        return Err(fail(
            "unsupported",
            "This HEIC color description requires a supported ICC profile.",
        ));
    }
    let cicp = moxcms::CicpProfile {
        color_primaries: moxcms::CicpColorPrimaries::try_from(primaries as u8)
            .map_err(|_| invalid())?,
        transfer_characteristics: moxcms::TransferCharacteristics::try_from(transfer as u8)
            .map_err(|_| invalid())?,
        matrix_coefficients: moxcms::MatrixCoefficients::Identity,
        full_range: true,
    };
    let mut profile = moxcms::ColorProfile::new_from_cicp(cicp);
    if profile.red_trc.is_none()
        || profile.green_trc.is_none()
        || profile.blue_trc.is_none()
        || !profile
            .rgb_to_xyz_matrix()
            .v
            .iter()
            .flatten()
            .all(|v| v.is_finite())
    {
        return Err(fail(
            "unsupported",
            "The HEIC color space cannot be converted.",
        ));
    }
    // Samples are already RGB. Resolve NCLX into a matrix/TRC ICC profile so the
    // existing CMS transforms once; do not mark unresolved CICP as accepted.
    profile.cicp = None;
    profile.encode().map(Some).map_err(|_| invalid())
}
fn read(mut reader: impl BufRead, cancel: &Cancellation) -> Result<DecodedImage> {
    cancel.check()?;
    if line(&mut reader)? != b"FH1" {
        return Err(invalid());
    }
    let header = line(&mut reader)?;
    let values = header
        .split(|v| *v == b' ')
        .map(|v| {
            if v.is_empty() || !v.iter().all(u8::is_ascii_digit) {
                return Err(invalid());
            }
            std::str::from_utf8(v)
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>>>()?;
    let [width, height, alpha, premultiplied, icc_size, primaries, transfer, exif_size, transformed] =
        values.as_slice()
    else {
        return Err(invalid());
    };
    if *width == 0
        || *height == 0
        || *width > u64::from(u32::MAX)
        || *height > u64::from(u32::MAX)
        || width.checked_mul(*height).is_none_or(|v| v > 80_000_000)
        || *alpha > 1
        || *premultiplied > 1
        || *transformed > 1
        || *icc_size > 1024 * 1024
        || *exif_size > 65536
        || *primaries > 255
        || *transfer > 255
    {
        return Err(invalid());
    }
    if matches!(*transfer, 16 | 18) {
        return Err(fail("unsupported", "Decoded HDR HEIC is unsupported."));
    }
    let icc = bytes(&mut reader, *icc_size as usize, cancel)?;
    let exif = bytes(&mut reader, *exif_size as usize, cancel)?;
    let exif_orientation = if !exif.is_empty() {
        let offset = u32::from_be_bytes(
            exif.get(..4)
                .ok_or_else(invalid)?
                .try_into()
                .map_err(|_| invalid())?,
        ) as usize;
        let offset = offset.checked_add(4).ok_or_else(invalid)?;
        image_orientation::parse_exif(exif.get(offset..).ok_or_else(invalid)?)?
    } else {
        1
    };
    let orientation = if *transformed == 1 {
        1
    } else {
        exif_orientation
    };
    let count = (*width as usize)
        .checked_mul(*height as usize)
        .and_then(|v| v.checked_mul(4))
        .ok_or_else(invalid)?;
    let mut raw = bytes(&mut reader, count, cancel)?;
    let mut extra = [0];
    if reader.read(&mut extra)? != 0 {
        return Err(invalid());
    }
    for block in raw.chunks_mut(4096 * 4) {
        cancel.check()?;
        for pixel in block.as_chunks_mut::<4>().0 {
            if *alpha == 0 && pixel[3] != 255 {
                return Err(invalid());
            }
            if *premultiplied == 1 {
                let a = u32::from(pixel[3]);
                for value in &mut pixel[..3] {
                    *value = (u32::from(*value) * 255 + a / 2)
                        .checked_div(a)
                        .unwrap_or(0)
                        .min(255) as u8;
                }
            }
        }
    }
    let has_icc = !icc.is_empty();
    let source_gray = if has_icc {
        let profile = moxcms::ColorProfile::new_from_slice(&icc).map_err(|_| invalid())?;
        if profile.cicp.is_some_and(|v| {
            matches!(
                v.transfer_characteristics,
                moxcms::TransferCharacteristics::Smpte2084 | moxcms::TransferCharacteristics::Hlg
            )
        }) {
            return Err(fail(
                "unsupported",
                "HDR HEIC ICC metadata requires a preservation workflow.",
            ));
        }
        if !matches!(
            profile.color_space,
            moxcms::DataColorSpace::Rgb | moxcms::DataColorSpace::Gray
        ) {
            return Err(fail("unsupported", "HEIC ICC sample space is unsupported."));
        }
        profile.color_space == moxcms::DataColorSpace::Gray
    } else {
        false
    };
    if source_gray {
        for block in raw.chunks(4096 * 4) {
            cancel.check()?;
            if block
                .as_chunks::<4>()
                .0
                .iter()
                .any(|v| v[0] != v[1] || v[1] != v[2])
            {
                return Err(fail(
                    "verification",
                    "HEIC gray profile does not match decoded samples.",
                ));
            }
        }
    }
    let profile = if has_icc {
        Some(icc)
    } else {
        color(*primaries as u16, *transfer as u16)?
    };
    let override_name = if has_icc {
        "heic_icc"
    } else if profile.is_some() {
        "heic_nclx"
    } else {
        "heic_assumed_srgb"
    };
    Ok(DecodedImage {
        pixels: image::RgbaImage::from_raw(*width as u32, *height as u32, raw)
            .ok_or_else(invalid)?,
        orientation,
        has_alpha: *alpha == 1,
        has_icc,
        icc_profile: profile,
        source_gray,
        srgb: false,
        gamma: None,
        chromaticities: None,
        has_cicp: false,
        has_exif: !exif.is_empty(),
        has_color_metadata: has_icc || *primaries != 0 || *transfer != 0,
        has_hdr_metadata: false,
        color_override: Some(override_name),
        preservation_pending: false,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_rejects_unbounded_ambiguous_truncated_extra_or_invalid_alpha() {
        let cancel = Cancellation::default();
        let input = b"FH1\n1 1 0 0 0 0 0 0 0\n\x01\x02\x03\xff";
        assert_eq!(
            read(std::io::Cursor::new(input), &cancel)
                .unwrap()
                .pixels
                .as_raw(),
            &[1, 2, 3, 255]
        );
        assert_eq!(
            read(
                std::io::Cursor::new(b"FH1\n1 1 1 1 0 0 0 0 0\n\x32\x19\x00\x7f"),
                &cancel
            )
            .unwrap()
            .pixels
            .as_raw(),
            &[100, 50, 0, 127]
        );
        let profile = moxcms::ColorProfile::new_bt2020_pq().encode().unwrap();
        let mut hdr = format!("FH1\n1 1 0 0 {} 0 0 0 0\n", profile.len()).into_bytes();
        hdr.extend(profile);
        hdr.extend([255, 255, 255, 255]);
        assert_eq!(
            read(std::io::Cursor::new(hdr), &cancel).err().unwrap().code,
            "unsupported"
        );
        for input in [
            b"FH1\n1 1 0 0 0 0 0 0 0\n\x01\x02\x03\x00".as_slice(),
            b"FH1\n1 1 0 0 0 0 0 0 0\n123",
            b"FH1\n1 1 0 0 0 0 0 0 0\n12345",
            b"FH1\n1 1 0 0 0 0 16 0 0\n",
            b"FH1\n80000001 1 0 0 0 0 0 0 0\n",
            b"FH1\n1  1 0 0 0 0 0 0 0\n",
        ] {
            assert!(read(std::io::Cursor::new(input), &cancel).is_err());
        }
    }
    #[test]
    fn nclx_profile_converts_p3_and_linear_sdr_without_changing_alpha() {
        let cancel = Cancellation::default();
        let mut pixels = image::RgbaImage::from_raw(1, 1, vec![200, 100, 50, 37]).unwrap();
        crate::image_color::normalize_icc(
            &mut pixels,
            &color(12, 13).unwrap().unwrap(),
            false,
            &cancel,
        )
        .unwrap();
        for (actual, expected) in pixels.get_pixel(0, 0).0[..3].iter().zip([215, 93, 31]) {
            assert!(actual.abs_diff(expected) <= 2, "{pixels:?}");
        }
        assert_eq!(pixels.get_pixel(0, 0)[3], 37);
        let mut pixels = image::RgbaImage::from_raw(1, 1, vec![128, 128, 128, 71]).unwrap();
        crate::image_color::normalize_icc(
            &mut pixels,
            &color(1, 8).unwrap().unwrap(),
            false,
            &cancel,
        )
        .unwrap();
        assert!(pixels.get_pixel(0, 0)[0].abs_diff(188) <= 1);
        assert_eq!(pixels.get_pixel(0, 0)[3], 71);
        assert!(color(9, 16).is_err());
        assert!(color(2, 13).is_err());
        assert!(color(12, 2).is_err());
    }
}
