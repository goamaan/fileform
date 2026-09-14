// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use image::{ImageDecoder, ImageEncoder};
use std::io::{BufRead, Seek, SeekFrom, Write};

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Background {
    White,
    Black,
}
fn flatten(
    pixels: &image::RgbaImage,
    background: Background,
    cancellation: &Cancellation,
) -> Result<Vec<u8>> {
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(pixels.as_raw().len() / 4 * 3)
        .map_err(|_| fail("limit", "Not enough memory to prepare JPEG pixels."))?;
    let background = match background {
        Background::White => 255u32,
        Background::Black => 0,
    };
    for (index, pixel) in pixels.pixels().enumerate() {
        if index % 4096 == 0 {
            cancellation.check()?;
        }
        let alpha = u32::from(pixel[3]);
        for color in &pixel.0[..3] {
            rgb.push(((u32::from(*color) * alpha + background * (255 - alpha) + 127) / 255) as u8);
        }
    }
    Ok(rgb)
}
pub(crate) fn encode<W: Write>(
    writer: &mut W,
    pixels: &image::RgbaImage,
    background: Background,
    quality: u8,
    cancellation: &Cancellation,
) -> Result<Vec<u8>> {
    if !(1..=100).contains(&quality) {
        return Err(fail(
            "invalid_request",
            "JPEG quality must be between 1 and 100.",
        ));
    }
    let rgb = flatten(pixels, background, cancellation)?;
    let profile = moxcms::ColorProfile::new_srgb()
        .encode()
        .map_err(|e| fail("encoding", e.to_string()))?;
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut *writer, quality);
    encoder
        .set_icc_profile(profile.clone())
        .map_err(|e| fail("encoding", e.to_string()))?;
    encoder
        .encode(
            &rgb,
            pixels.width(),
            pixels.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| fail("encoding", e.to_string()))?;
    writer.flush()?;
    cancellation.check()?;
    Ok(profile)
}
pub(crate) fn verify<R: BufRead + Seek>(
    mut input: R,
    width: u32,
    height: u32,
    profile: &[u8],
    cancellation: &Cancellation,
) -> Result<()> {
    input.seek(SeekFrom::End(-2))?;
    let mut end = [0u8; 2];
    input.read_exact(&mut end)?;
    if end != [0xff, 0xd9] {
        return Err(fail("verification", "JPEG ending is incomplete."));
    }
    input.seek(SeekFrom::Start(0))?;
    let mut reader = image::ImageReader::with_format(input, image::ImageFormat::Jpeg);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| fail("verification", e.to_string()))?;
    if decoder.dimensions() != (width, height)
        || decoder.color_type() != image::ColorType::Rgb8
        || decoder
            .icc_profile()
            .map_err(|e| fail("verification", e.to_string()))?
            .as_deref()
            != Some(profile)
        || decoder
            .orientation()
            .map_err(|e| fail("verification", e.to_string()))?
            != image::metadata::Orientation::NoTransforms
    {
        return Err(fail(
            "verification",
            "JPEG dimensions, color profile or orientation differ from the rendered image.",
        ));
    }
    let size = usize::try_from(decoder.total_bytes())
        .map_err(|_| fail("limit", "JPEG buffer size overflow."))?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(size)
        .map_err(|_| fail("limit", "Not enough memory to verify JPEG."))?;
    decoded.resize(size, 0);
    decoder
        .read_image(&mut decoded)
        .map_err(|e| fail("verification", e.to_string()))?;
    cancellation.check()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_pixels_composite_against_explicit_backgrounds() {
        let pixels =
            image::RgbaImage::from_raw(3, 1, vec![255, 0, 0, 128, 10, 20, 30, 0, 10, 20, 30, 255])
                .unwrap();
        assert_eq!(
            flatten(&pixels, Background::White, &Cancellation::default()).unwrap(),
            [255, 127, 127, 255, 255, 255, 10, 20, 30]
        );
        assert_eq!(
            flatten(&pixels, Background::Black, &Cancellation::default()).unwrap(),
            [128, 0, 0, 0, 0, 0, 10, 20, 30]
        );
    }
    #[test]
    fn encoded_jpeg_has_verified_color_dimensions_and_ending() {
        let pixels = image::RgbaImage::from_pixel(8, 8, image::Rgba([100, 150, 200, 255]));
        let mut output = Vec::new();
        let signal = Cancellation::default();
        let profile = encode(&mut output, &pixels, Background::White, 85, &signal).unwrap();
        verify(std::io::Cursor::new(&output), 8, 8, &profile, &signal).unwrap();
        assert!(verify(
            std::io::Cursor::new(&output[..output.len() - 2]),
            8,
            8,
            &profile,
            &signal
        )
        .is_err());
        assert!(verify(std::io::Cursor::new(&output), 9, 8, &profile, &signal).is_err());
    }
}
