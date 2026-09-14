// SPDX-License-Identifier: Apache-2.0
//! Bounded PNG pixel inspection. Rendering/color conversion is a separate step.
use crate::{fail, Result};
use std::io::{BufRead, Seek, SeekFrom};

const MAX_PIXELS: u64 = 80_000_000;
const SCRATCH_BUDGET: usize = 64 * 1024 * 1024;
pub(crate) struct DecodedPng {
    pub pixels: image::RgbaImage,
    pub orientation: u8,
    pub has_alpha: bool,
    pub has_icc: bool,
    pub icc_profile: Option<Vec<u8>>,
    pub source_gray: bool,
    pub srgb: bool,
    pub gamma: Option<f32>,
    pub chromaticities: Option<[f64; 8]>,
    pub has_cicp: bool,
    pub has_exif: bool,
    pub has_color_metadata: bool,
    pub has_hdr_metadata: bool,
}
fn pixel_count(width: u32, height: u32) -> Result<usize> {
    let count = u64::from(width) * u64::from(height);
    if count == 0 || count > MAX_PIXELS {
        return Err(fail("limit", "Images are limited to 80 megapixels."));
    }
    usize::try_from(count)
        .map_err(|_| fail("limit", "Image dimensions exceed this platform's limits."))
}
pub(crate) fn decode<R: BufRead + Seek>(mut input: R) -> Result<DecodedPng> {
    if input.seek(SeekFrom::End(0))? < 20 {
        return Err(fail("invalid_image", "PNG is incomplete."));
    }
    input.seek(SeekFrom::End(-12))?;
    let mut end = [0u8; 12];
    input.read_exact(&mut end)?;
    if end != [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82] {
        return Err(fail(
            "invalid_image",
            "PNG must end with a complete IEND record.",
        ));
    }
    input.seek(SeekFrom::Start(0))?;
    let metadata_mask = crate::png_metadata::preflight(&mut input)?;
    let mut decoder = png::Decoder::new(input);
    decoder.set_limits(png::Limits {
        bytes: SCRATCH_BUDGET,
    });
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(png_error)?;
    let info = reader.info();
    let count = pixel_count(info.width, info.height)?;
    if info.animation_control.is_some() {
        return Err(fail(
            "unsupported",
            "Animated PNG conversion is not supported.",
        ));
    }
    if info.bit_depth == png::BitDepth::Sixteen {
        return Err(fail(
            "unsupported",
            "High-bit-depth image preservation is not implemented.",
        ));
    }
    let source_gray = matches!(
        info.color_type,
        png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha
    );
    let width = info.width;
    let height = info.height;
    let bytes = reader
        .output_buffer_size()
        .ok_or_else(|| fail("limit", "Image buffer size overflow."))?;
    if bytes > count * 4 {
        return Err(fail("limit", "Decoded image exceeds its pixel budget."));
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(bytes)
        .map_err(|_| fail("limit", "Not enough memory to decode the image."))?;
    decoded.resize(bytes, 0);
    let output = reader.next_frame(&mut decoded).map_err(png_error)?;
    // next_frame alone does not prove that the final PNG records are present.
    reader.finish().map_err(png_error)?;
    let info = reader.info();
    let present = [
        info.gama_chunk.is_some(),
        info.chrm_chunk.is_some(),
        info.srgb.is_some(),
        info.icc_profile.is_some(),
        info.exif_metadata.is_some(),
        info.coding_independent_code_points.is_some(),
        info.mastering_display_color_volume.is_some(),
        info.content_light_level.is_some(),
    ];
    for (index, exists) in present.iter().enumerate() {
        if metadata_mask & (1 << index) != 0 && !exists {
            return Err(fail(
                "invalid_image",
                "PNG metadata could not be decoded reliably.",
            ));
        }
    }
    let srgb = info.srgb.is_some();
    let gamma = info.gama_chunk.map(|value| value.into_value());
    let chromaticities = info.chrm_chunk.map(|c| {
        [
            c.white.0, c.white.1, c.red.0, c.red.1, c.green.0, c.green.1, c.blue.0, c.blue.1,
        ]
        .map(|v| f64::from(v.into_value()))
    });
    let has_cicp = info.coding_independent_code_points.is_some();
    let has_icc = info.icc_profile.is_some();
    if info
        .icc_profile
        .as_ref()
        .is_some_and(|icc| icc.len() > 1024 * 1024)
    {
        return Err(fail("limit", "ICC profile exceeds 1 MiB."));
    }
    let icc_profile = info.icc_profile.as_deref().map(<[u8]>::to_vec);
    let has_exif = info.exif_metadata.is_some();
    let orientation = info
        .exif_metadata
        .as_deref()
        .map(crate::image_orientation::parse_exif)
        .transpose()?
        .unwrap_or(1);
    let has_hdr_metadata =
        info.mastering_display_color_volume.is_some() || info.content_light_level.is_some();
    let has_color_metadata = has_icc
        || info.srgb.is_some()
        || info.gama_chunk.is_some()
        || info.chrm_chunk.is_some()
        || info.coding_independent_code_points.is_some()
        || has_hdr_metadata;
    if output.width != width || output.height != height || output.bit_depth != png::BitDepth::Eight
    {
        return Err(fail(
            "invalid_image",
            "PNG decoded dimensions or depth changed.",
        ));
    }
    decoded.truncate(output.buffer_size());
    let has_alpha = matches!(
        output.color_type,
        png::ColorType::Rgba | png::ColorType::GrayscaleAlpha
    );
    let rgba = if output.color_type == png::ColorType::Rgba {
        decoded
    } else {
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(count * 4)
            .map_err(|_| fail("limit", "Not enough memory to prepare the image."))?;
        match output.color_type {
            png::ColorType::Rgb => {
                for pixel in decoded.as_chunks::<3>().0 {
                    rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
                }
            }
            png::ColorType::Grayscale => {
                for gray in decoded {
                    rgba.extend_from_slice(&[gray, gray, gray, 255]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for pixel in decoded.as_chunks::<2>().0 {
                    rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
                }
            }
            _ => return Err(fail("unsupported", "PNG pixel layout is unsupported.")),
        }
        rgba
    };
    let pixels = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| fail("invalid_image", "PNG pixel buffer is incomplete."))?;
    Ok(DecodedPng {
        pixels,
        orientation,
        has_alpha,
        has_icc,
        icc_profile,
        source_gray,
        srgb,
        gamma,
        chromaticities,
        has_cicp,
        has_exif,
        has_color_metadata,
        has_hdr_metadata,
    })
}
fn png_error(error: png::DecodingError) -> crate::Failure {
    fail("invalid_image", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn encoded(color: png::ColorType, depth: png::BitDepth, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(color);
            encoder.set_depth(depth);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(data).unwrap();
            writer.finish().unwrap();
        }
        bytes
    }
    #[test]
    fn rgba_pixels_and_transparency_round_trip_exactly() {
        let pixels = [255, 0, 0, 0, 0, 128, 255, 127];
        let result = decode(Cursor::new(encoded(
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &pixels,
        )))
        .unwrap();
        assert_eq!(result.pixels.as_raw(), &pixels);
        assert!(result.has_alpha);
    }
    #[test]
    fn grayscale_expands_without_changing_values() {
        let result = decode(Cursor::new(encoded(
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[7, 250],
        )))
        .unwrap();
        assert_eq!(result.pixels.as_raw(), &[7, 7, 7, 255, 250, 250, 250, 255]);
        assert!(!result.has_alpha);
    }
    #[test]
    fn missing_end_and_corrupt_pixels_are_rejected() {
        let bytes = encoded(
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            &[1, 2, 3, 4, 5, 6],
        );
        assert!(decode(Cursor::new(&bytes[..bytes.len() - 12])).is_err());
        let mut corrupt = bytes;
        corrupt[45] ^= 0xff;
        assert!(decode(Cursor::new(corrupt)).is_err());
    }
    #[test]
    fn high_depth_and_excessive_dimensions_are_rejected() {
        assert!(decode(Cursor::new(encoded(
            png::ColorType::Grayscale,
            png::BitDepth::Sixteen,
            &[0, 1, 0, 2]
        )))
        .is_err());
        assert!(pixel_count(80_000_001, 1).is_err());
        assert!(pixel_count(u32::MAX, u32::MAX).is_err());
        assert!(pixel_count(0, 1).is_err());
        assert_eq!(pixel_count(10_000, 8_000).unwrap(), 80_000_000);
    }
}
