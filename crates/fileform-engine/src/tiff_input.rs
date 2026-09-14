// SPDX-License-Identifier: Apache-2.0
use crate::{fail, image_input::DecodedImage, Result};
use std::{
    collections::BTreeSet,
    io::{Read, Seek, SeekFrom},
};
use tiff::tags::Tag;
fn number<R: Read>(input: &mut R, bytes: usize, little: bool) -> Result<u64> {
    let mut data = [0u8; 8];
    input.read_exact(&mut data[..bytes])?;
    Ok(if little {
        u64::from_le_bytes(data)
    } else {
        u64::from_be_bytes(data) >> (64 - bytes * 8)
    })
}
fn preflight<R: Read + Seek>(input: &mut R) -> Result<BTreeSet<u16>> {
    let length = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(0))?;
    let mut order = [0u8; 2];
    input.read_exact(&mut order)?;
    let little = match &order {
        b"II" => true,
        b"MM" => false,
        _ => return Err(fail("invalid_image", "Not a TIFF image.")),
    };
    let version = number(input, 2, little)?;
    let (count_bytes, entry_bytes, inline, offset_bytes) = match version {
        42 => (2, 12, 4, 4),
        43 => {
            if number(input, 2, little)? != 8 || number(input, 2, little)? != 0 {
                return Err(fail("invalid_image", "Invalid BigTIFF header."));
            }
            (8, 20, 8, 8)
        }
        _ => return Err(fail("unsupported", "Unknown TIFF version.")),
    };
    let directory = number(input, offset_bytes, little)?;
    if directory < (if version == 42 { 8 } else { 16 }) || directory >= length {
        return Err(fail("invalid_image", "TIFF directory is outside the file."));
    }
    input.seek(SeekFrom::Start(directory))?;
    let count = number(input, count_bytes, little)?;
    if count > 4096 {
        return Err(fail("limit", "TIFF directory exceeds 4096 entries."));
    }
    let end = directory
        .checked_add(count_bytes as u64)
        .and_then(|n| n.checked_add(count * entry_bytes))
        .and_then(|n| n.checked_add(offset_bytes as u64))
        .ok_or_else(|| fail("limit", "TIFF directory overflow."))?;
    if end > length {
        return Err(fail("invalid_image", "TIFF directory is truncated."));
    }
    let mut tags = BTreeSet::new();
    let mut metadata_bytes = 0u64;
    for _ in 0..count {
        let tag = number(input, 2, little)? as u16;
        if !tags.insert(tag) {
            return Err(fail("invalid_image", "Duplicate TIFF tag."));
        }
        let kind = number(input, 2, little)?;
        let values = number(input, if version == 42 { 4 } else { 8 }, little)?;
        let unit = match kind {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 | 16 | 17 | 18 => 8,
            _ => return Err(fail("unsupported", "Unsupported TIFF field type.")),
        };
        let size = values
            .checked_mul(unit)
            .ok_or_else(|| fail("limit", "TIFF field size overflow."))?;
        metadata_bytes = metadata_bytes
            .checked_add(size)
            .ok_or_else(|| fail("limit", "TIFF metadata size overflow."))?;
        if size > 1024 * 1024 || metadata_bytes > 8 * 1024 * 1024 {
            return Err(fail("limit", "TIFF metadata exceeds its size budget."));
        }
        let value = number(input, inline, little)?;
        if size > inline as u64 && value.checked_add(size).is_none_or(|end| end > length) {
            return Err(fail("invalid_image", "TIFF field extends beyond the file."));
        }
    }
    if number(input, offset_bytes, little)? != 0 || tags.contains(&330) {
        return Err(fail(
            "unsupported",
            "Multi-image TIFF preservation is not implemented.",
        ));
    }
    input.seek(SeekFrom::Start(0))?;
    Ok(tags)
}
pub(crate) fn decode<R: Read + Seek>(
    mut input: R,
    cancellation: &crate::Cancellation,
) -> Result<DecodedImage> {
    cancellation.check()?;
    let tags = preflight(&mut input)?;
    let mut limits = tiff::decoder::Limits::default();
    limits.decoding_buffer_size = 512 * 1024 * 1024;
    limits.intermediate_buffer_size = 64 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoder = tiff::decoder::Decoder::new(input)
        .map_err(error)?
        .with_limits(limits);
    let (width, height) = decoder.dimensions().map_err(error)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 80_000_000 {
        return Err(fail("limit", "TIFF exceeds the 80-megapixel limit."));
    }
    let (channels, gray, alpha) = match decoder.colortype().map_err(error)? {
        tiff::ColorType::RGB(8) => (3, false, false),
        tiff::ColorType::RGBA(8) => (4, false, true),
        tiff::ColorType::Gray(8) => (1, true, false),
        tiff::ColorType::GrayA(8) => (2, true, true),
        _ => {
            return Err(fail(
                "unsupported",
                "This TIFF pixel layout requires a preservation workflow.",
            ))
        }
    };
    let orientation = decoder
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(error)?
        .unwrap_or(1);
    if !(1..=8).contains(&orientation) {
        return Err(fail("invalid_image", "Invalid TIFF orientation."));
    }
    let extras = decoder
        .find_tag_unsigned_vec::<u16>(Tag::ExtraSamples)
        .map_err(error)?
        .unwrap_or_default();
    if alpha && (extras.len() != 1 || ![1, 2].contains(&extras[0])) {
        return Err(fail(
            "unsupported",
            "TIFF alpha representation is unsupported.",
        ));
    }
    let has_icc = tags.contains(&34675);
    let icc_profile = if has_icc {
        Some(decoder.get_tag_u8_vec(Tag::IccProfile).map_err(error)?)
    } else {
        None
    };
    let pending = tags.contains(&700)
        || (!has_icc
            && [290, 291, 301, 318, 319, 342, 34665]
                .iter()
                .any(|tag| tags.contains(tag)));
    let count = width as usize * height as usize;
    let layout = decoder.image_buffer_layout().map_err(error)?;
    if layout.complete_len > 512 * 1024 * 1024 || (layout.planes != 1 && layout.planes != channels)
    {
        return Err(fail("limit", "TIFF pixel layout exceeds supported limits."));
    }
    let row_stride = layout
        .row_stride
        .ok_or_else(|| fail("invalid_image", "Missing TIFF row stride."))?
        .get();
    let plane_stride = layout
        .plane_stride
        .ok_or_else(|| fail("invalid_image", "Missing TIFF plane stride."))?
        .get();
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(layout.complete_len)
        .map_err(|_| fail("limit", "Not enough memory for TIFF pixels."))?;
    decoded.resize(layout.complete_len, 0);
    decoder.read_image_bytes(&mut decoded).map_err(error)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(count * 4)
        .map_err(|_| fail("limit", "Not enough memory to prepare TIFF."))?;
    for index in 0..count {
        if index % 4096 == 0 {
            cancellation.check()?;
        }
        let x = index % width as usize;
        let y = index / width as usize;
        let sample = |channel: usize| -> Result<u8> {
            let position = if layout.planes == 1 {
                y.checked_mul(row_stride)
                    .and_then(|v| v.checked_add(x * channels + channel))
            } else {
                channel.checked_mul(plane_stride).and_then(|v| {
                    y.checked_mul(row_stride)
                        .and_then(|row| row.checked_add(x))
                        .and_then(|offset| v.checked_add(offset))
                })
            };
            position
                .and_then(|p| decoded.get(p).copied())
                .ok_or_else(|| {
                    fail(
                        "invalid_image",
                        "TIFF sample is outside its decoded buffer.",
                    )
                })
        };
        let a = if alpha { sample(channels - 1)? } else { 255 };
        for channel in 0..3 {
            let value = sample(if gray { 0 } else { channel })?;
            rgba.push(if alpha && extras[0] == 1 {
                ((u32::from(value) * 255 + u32::from(a) / 2)
                    .checked_div(u32::from(a))
                    .unwrap_or(0))
                .min(255) as u8
            } else {
                value
            });
        }
        rgba.push(a);
    }
    let pixels = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| fail("invalid_image", "TIFF pixel buffer is incomplete."))?;
    Ok(DecodedImage {
        pixels,
        orientation: orientation as u8,
        has_alpha: alpha,
        has_icc,
        icc_profile,
        source_gray: gray,
        srgb: false,
        gamma: None,
        chromaticities: None,
        has_cicp: false,
        has_exif: tags.contains(&34665),
        has_color_metadata: has_icc || pending,
        has_hdr_metadata: false,
        preservation_pending: pending,
        color_override: None,
    })
}
fn error(error: tiff::TiffError) -> crate::Failure {
    fail("invalid_image", error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn own_tiff_export_reopens_with_exact_alpha_and_pixels() {
        let pixels = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 0, 10, 20, 30, 128]).unwrap();
        let mut output = Cursor::new(Vec::new());
        crate::tiff_output::encode(&mut output, &pixels, &crate::Cancellation::default()).unwrap();
        output.set_position(0);
        let decoded = decode(output, &crate::Cancellation::default()).unwrap();
        assert_eq!(decoded.pixels, pixels);
        assert!(decoded.has_alpha && decoded.has_icc);
        assert!(!decoded.preservation_pending);
    }
    #[test]
    fn associated_alpha_is_unpremultiplied() {
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = tiff::encoder::TiffEncoder::new(&mut output).unwrap();
            let mut image = encoder
                .new_image::<tiff::encoder::colortype::RGBA8>(1, 1)
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ExtraSamples, &[1u16][..])
                .unwrap();
            image.write_data(&[64, 32, 16, 128]).unwrap();
        }
        output.set_position(0);
        assert_eq!(
            decode(output, &crate::Cancellation::default())
                .unwrap()
                .pixels
                .as_raw(),
            &[128, 64, 32, 128]
        );
    }
    #[test]
    fn bigtiff_grayscale_is_supported() {
        let mut output = Cursor::new(Vec::new());
        tiff::encoder::TiffEncoder::new_big(&mut output)
            .unwrap()
            .write_image::<tiff::encoder::colortype::Gray8>(2, 1, &[20, 40])
            .unwrap();
        output.set_position(0);
        let decoded = decode(output, &crate::Cancellation::default()).unwrap();
        assert_eq!(decoded.pixels.as_raw(), &[20, 20, 20, 255, 40, 40, 40, 255]);
    }
    #[test]
    fn separate_rgb_planes_are_interleaved_correctly() {
        let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
        bytes.extend_from_slice(&10u16.to_le_bytes());
        for (tag, kind, count, value) in [
            (256u16, 4u16, 1u32, 2u32),
            (257, 4, 1, 1),
            (258, 3, 3, 134),
            (259, 3, 1, 1),
            (262, 3, 1, 2),
            (273, 4, 3, 140),
            (277, 3, 1, 3),
            (278, 4, 1, 1),
            (279, 4, 3, 152),
            (284, 3, 1, 2),
        ] {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(&count.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for _ in 0..3 {
            bytes.extend_from_slice(&8u16.to_le_bytes());
        }
        for offset in [164u32, 166, 168, 2, 2, 2] {
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes.extend_from_slice(&[10, 40, 20, 50, 30, 60]);
        let result = decode(Cursor::new(bytes), &crate::Cancellation::default()).unwrap();
        assert_eq!(result.pixels.as_raw(), &[10, 20, 30, 255, 40, 50, 60, 255]);
    }
    #[test]
    fn multiple_images_and_high_depth_are_rejected() {
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = tiff::encoder::TiffEncoder::new(&mut output).unwrap();
            encoder
                .write_image::<tiff::encoder::colortype::Gray8>(1, 1, &[8])
                .unwrap();
            encoder
                .write_image::<tiff::encoder::colortype::Gray8>(1, 1, &[9])
                .unwrap();
        }
        output.set_position(0);
        assert!(decode(output, &crate::Cancellation::default()).is_err());
        let mut output = Cursor::new(Vec::new());
        tiff::encoder::TiffEncoder::new(&mut output)
            .unwrap()
            .write_image::<tiff::encoder::colortype::Gray16>(1, 1, &[300])
            .unwrap();
        output.set_position(0);
        assert!(decode(output, &crate::Cancellation::default()).is_err());
    }
}
