// SPDX-License-Identifier: Apache-2.0
use crate::{fail, image_input::DecodedImage, Result};
use std::{
    collections::BTreeMap,
    io::{BufRead, Seek, SeekFrom},
};
struct Metadata {
    width: u32,
    height: u32,
    gray: bool,
    orientation: u8,
    icc: Option<Vec<u8>>,
    exif: bool,
    pending: bool,
    color_hint: crate::exif_color::Hint,
}
fn byte<R: BufRead>(input: &mut R) -> Result<u8> {
    let mut b = [0];
    input.read_exact(&mut b)?;
    Ok(b[0])
}
fn metadata<R: BufRead + Seek>(input: &mut R) -> Result<Metadata> {
    let length = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(0))?;
    if byte(input)? != 255 || byte(input)? != 0xd8 {
        return Err(fail("invalid_image", "Not a JPEG image."));
    }
    let mut frame = None;
    let mut ended = false;
    let mut scan = false;
    let mut saw_scan = false;
    let mut orientation = 1;
    let mut exif = false;
    let mut color_hint = crate::exif_color::Hint::Unspecified;
    let mut pending = false;
    let mut parts = BTreeMap::new();
    let mut expected_parts = None;
    let mut profile_bytes = 0usize;
    let mut metadata_bytes = 0usize;
    for _ in 0..100_000 {
        let marker = loop {
            let first = byte(input)?;
            if first != 255 {
                if scan {
                    continue;
                }
                return Err(fail("invalid_image", "Invalid JPEG marker."));
            }
            let mut marker = byte(input)?;
            while marker == 255 {
                marker = byte(input)?;
            }
            if scan && (marker == 0 || (0xd0..=0xd7).contains(&marker)) {
                continue;
            }
            break marker;
        };
        if marker == 0xd9 {
            if !saw_scan || input.stream_position()? != length {
                return Err(fail(
                    "invalid_image",
                    "JPEG has an incomplete or trailing image.",
                ));
            }
            ended = true;
            break;
        }
        if marker == 0xd8 || marker == 0 || (0xd0..=0xd7).contains(&marker) {
            return Err(fail("invalid_image", "Unexpected JPEG marker."));
        }
        if marker == 1 {
            continue;
        }
        let size = usize::from(u16::from_be_bytes([byte(input)?, byte(input)?]));
        if size < 2
            || input
                .stream_position()?
                .checked_add((size - 2) as u64)
                .is_none_or(|end| end > length)
        {
            return Err(fail("invalid_image", "Truncated JPEG segment."));
        }
        let mut data = Vec::new();
        data.try_reserve_exact(size - 2)
            .map_err(|_| fail("limit", "JPEG metadata allocation failed."))?;
        data.resize(size - 2, 0);
        input.read_exact(&mut data)?;
        if (0xe0..=0xef).contains(&marker) {
            metadata_bytes += data.len();
            if metadata_bytes > 8 * 1024 * 1024 {
                return Err(fail("limit", "JPEG metadata exceeds 8 MiB."));
            }
        }
        match marker {
            0xc0 | 0xc2 => {
                if frame.is_some() || data.len() < 6 || data[0] != 8 {
                    return Err(fail(
                        "unsupported",
                        "JPEG requires a single eight-bit frame.",
                    ));
                }
                let height = u32::from(u16::from_be_bytes([data[1], data[2]]));
                let width = u32::from(u16::from_be_bytes([data[3], data[4]]));
                let components = data[5];
                if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 80_000_000 {
                    return Err(fail("limit", "JPEG exceeds the 80-megapixel limit."));
                }
                if ![1, 3].contains(&components) || data.len() != 6 + usize::from(components) * 3 {
                    return Err(fail(
                        "unsupported",
                        "JPEG component layout requires a preservation workflow.",
                    ));
                }
                frame = Some((width, height, components == 1));
            }
            0xc1 | 0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => {
                return Err(fail(
                    "unsupported",
                    "This JPEG coding mode is not yet supported.",
                ))
            }
            0xe1 if data.starts_with(b"Exif\0\0") => {
                if exif {
                    return Err(fail("invalid_image", "Duplicate JPEG EXIF metadata."));
                }
                exif = true;
                orientation = crate::image_orientation::parse_exif(&data[6..])?;
                color_hint = crate::exif_color::read(&data[6..])?;
            }
            0xe2 if data.starts_with(b"ICC_PROFILE\0") => {
                if data.len() < 14 {
                    return Err(fail("invalid_image", "JPEG ICC segment is incomplete."));
                }
                let sequence = data[12];
                let count = data[13];
                if sequence == 0
                    || count == 0
                    || sequence > count
                    || expected_parts.is_some_and(|n| n != count)
                    || parts.contains_key(&sequence)
                {
                    return Err(fail("invalid_image", "JPEG ICC segments are inconsistent."));
                }
                expected_parts = Some(count);
                profile_bytes += data.len() - 14;
                if profile_bytes > 1024 * 1024 {
                    return Err(fail("limit", "ICC profile exceeds 1 MiB."));
                }
                parts.insert(sequence, data[14..].to_vec());
            }
            0xe0 if data.starts_with(b"JFIF\0") || data.starts_with(b"JFXX\0") => (),
            0xee if data.starts_with(b"Adobe") => (),
            0xe0..=0xef => pending = true,
            _ => (),
        }
        scan = marker == 0xda;
        if scan {
            saw_scan = true;
        }
    }
    if !ended || input.stream_position()? != length {
        return Err(fail("limit", "JPEG exceeds the marker limit."));
    }
    let (width, height, gray) =
        frame.ok_or_else(|| fail("invalid_image", "JPEG frame is missing."))?;
    let icc = if let Some(count) = expected_parts {
        if parts.len() != usize::from(count) {
            return Err(fail(
                "invalid_image",
                "JPEG ICC profile has missing segments.",
            ));
        }
        let mut profile = Vec::new();
        profile
            .try_reserve_exact(profile_bytes)
            .map_err(|_| fail("limit", "ICC allocation failed."))?;
        for (_, part) in parts {
            profile.extend_from_slice(&part);
        }
        Some(profile)
    } else {
        None
    };
    input.seek(SeekFrom::Start(0))?;
    Ok(Metadata {
        width,
        height,
        gray,
        orientation,
        icc,
        exif,
        pending,
        color_hint,
    })
}
pub(crate) fn decode<R: BufRead + Seek>(mut input: R) -> Result<DecodedImage> {
    let mut meta = metadata(&mut input)?;
    let options = zune_core::options::DecoderOptions::default()
        .set_strict_mode(true)
        .set_max_width(meta.width as usize)
        .set_max_height(meta.height as usize)
        .jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(input, options);
    decoder
        .decode_headers()
        .map_err(|e| fail("invalid_image", e.to_string()))?;
    if decoder.dimensions() != Some((meta.width as usize, meta.height as usize)) {
        return Err(fail(
            "invalid_image",
            "JPEG dimensions changed while decoding.",
        ));
    }
    let bytes = meta.width as usize * meta.height as usize * 4;
    if decoder.output_buffer_size() != Some(bytes) {
        return Err(fail(
            "invalid_image",
            "JPEG output layout differs from expected RGBA.",
        ));
    }
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(bytes)
        .map_err(|_| fail("limit", "Not enough memory for JPEG pixels."))?;
    rgba.resize(bytes, 0);
    decoder
        .decode_into(&mut rgba)
        .map_err(|e| fail("invalid_image", e.to_string()))?;
    let pixels = image::RgbaImage::from_raw(meta.width, meta.height, rgba)
        .ok_or_else(|| fail("invalid_image", "JPEG pixel buffer is incomplete."))?;
    let has_icc = meta.icc.is_some();
    let mut color_override = None;
    if !has_icc {
        match meta.color_hint {
            crate::exif_color::Hint::AdobeRgb if !meta.gray => {
                meta.icc = Some(
                    moxcms::ColorProfile::new_adobe_rgb()
                        .encode()
                        .map_err(|e| fail("invalid_image", e.to_string()))?,
                );
                color_override = Some("exif_adobe_rgb");
            }
            crate::exif_color::Hint::Srgb => color_override = Some("exif_srgb"),
            crate::exif_color::Hint::Pending | crate::exif_color::Hint::AdobeRgb => {
                meta.pending = true
            }
            _ => (),
        }
    }
    Ok(DecodedImage {
        color_override,
        pixels,
        orientation: meta.orientation,
        has_alpha: false,
        has_icc,
        icc_profile: meta.icc,
        source_gray: meta.gray,
        srgb: meta.color_hint == crate::exif_color::Hint::Srgb,
        gamma: None,
        chromaticities: None,
        has_cicp: false,
        has_exif: meta.exif,
        has_color_metadata: has_icc || meta.color_hint != crate::exif_color::Hint::Unspecified,
        has_hdr_metadata: false,
        preservation_pending: meta.pending,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        let image = image::RgbaImage::from_pixel(8, 4, image::Rgba([100, 150, 200, 255]));
        crate::jpeg_output::encode(
            &mut bytes,
            &image,
            crate::Background::White,
            85,
            &crate::Cancellation::default(),
        )
        .unwrap();
        bytes
    }
    fn insert(data: &[u8], marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut result = data[..2].to_vec();
        result.extend_from_slice(&[255, marker]);
        result.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        result.extend_from_slice(payload);
        result.extend_from_slice(&data[2..]);
        result
    }
    #[test]
    fn strict_baseline_and_progressive_gray_decode() {
        let image = decode(Cursor::new(fixture())).unwrap();
        assert_eq!(image.pixels.dimensions(), (8, 4));
        assert!(!image.has_alpha);
        assert!(image.has_icc);
        let gray = decode(Cursor::new(
            include_bytes!("../tests/fixtures/progressive-gray.jpg").as_slice(),
        ))
        .unwrap();
        assert!(gray.source_gray);
        assert!(gray.pixels.pixels().all(|p| p.0 == [128, 128, 128, 255]));
    }
    #[test]
    fn exif_orientation_and_preservation_flags_survive() {
        let original = fixture();
        let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        exif.extend_from_slice(&[6, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            decode(Cursor::new(insert(&original, 0xe1, &exif)))
                .unwrap()
                .orientation,
            6
        );
        let marked = decode(Cursor::new(insert(
            &original,
            0xe1,
            b"http://ns.adobe.com/xap/1.0/\0<metadata/>",
        )))
        .unwrap();
        assert!(marked.preservation_pending);
    }
    #[test]
    fn truncated_trailing_and_incomplete_icc_are_rejected() {
        let original = fixture();
        assert!(decode(Cursor::new(&original[..original.len() - 2])).is_err());
        let mut trailing = original.clone();
        trailing.push(0);
        assert!(decode(Cursor::new(trailing)).is_err());
        assert!(decode(Cursor::new(insert(
            &original,
            0xe2,
            b"ICC_PROFILE\0\x02\x02bad"
        )))
        .is_err());
        assert!(decode(Cursor::new(insert(&original, 0xe1, b"Exif\0\0bad"))).is_err());
    }
}
