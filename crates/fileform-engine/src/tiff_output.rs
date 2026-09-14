// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use std::io::{Read, Seek, Write};
use tiff::tags::Tag;
pub(crate) fn encode<W: Write + Seek>(
    writer: &mut W,
    pixels: &image::RgbaImage,
    cancellation: &Cancellation,
) -> Result<Vec<u8>> {
    cancellation.check()?;
    let profile = moxcms::ColorProfile::new_srgb()
        .encode()
        .map_err(|e| fail("encoding", e.to_string()))?;
    let mut encoder = tiff::encoder::TiffEncoder::new(&mut *writer)
        .map_err(error)?
        .with_compression(tiff::encoder::Compression::Lzw);
    let mut image = encoder
        .new_image::<tiff::encoder::colortype::RGBA8>(pixels.width(), pixels.height())
        .map_err(error)?;
    image
        .rows_per_strip((1024 * 1024 / (pixels.width().max(1) * 4)).max(1))
        .map_err(error)?;
    image
        .encoder()
        .write_tag(Tag::ExtraSamples, &[2u16][..])
        .map_err(error)?;
    image
        .encoder()
        .write_tag(Tag::Orientation, 1u16)
        .map_err(error)?;
    image
        .encoder()
        .write_tag(Tag::IccProfile, profile.as_slice())
        .map_err(error)?;
    image.write_data(pixels.as_raw()).map_err(error)?;
    writer.flush()?;
    cancellation.check()?;
    Ok(profile)
}
pub(crate) fn verify<R: Read + Seek>(
    input: R,
    pixels: &image::RgbaImage,
    profile: &[u8],
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    let mut limits = tiff::decoder::Limits::default();
    limits.decoding_buffer_size = 512 * 1024 * 1024;
    limits.intermediate_buffer_size = 64 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoder = tiff::decoder::Decoder::new(input)
        .map_err(error)?
        .with_limits(limits);
    if decoder.dimensions().map_err(error)? != pixels.dimensions()
        || decoder.colortype().map_err(error)? != tiff::ColorType::RGBA(8)
        || decoder.more_images()
        || decoder
            .get_tag_unsigned::<u16>(Tag::Orientation)
            .map_err(error)?
            != 1
        || decoder.get_tag_u16_vec(Tag::ExtraSamples).map_err(error)? != [2]
        || decoder.get_tag_u8_vec(Tag::IccProfile).map_err(error)? != profile
    {
        return Err(fail(
            "verification",
            "TIFF dimensions, alpha, profile or image count differ from the rendered result.",
        ));
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(pixels.as_raw().len())
        .map_err(|_| fail("limit", "Not enough memory to verify TIFF."))?;
    decoded.resize(pixels.as_raw().len(), 0);
    decoder.read_image_bytes(&mut decoded).map_err(error)?;
    if &decoded != pixels.as_raw() {
        return Err(fail(
            "verification",
            "TIFF pixels differ from the rendered result.",
        ));
    }
    cancellation.check()?;
    Ok(())
}
fn error(error: tiff::TiffError) -> crate::Failure {
    fail("image_encoding", error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgba_tiff_retains_exact_pixels_and_required_tags() {
        let pixels = image::RgbaImage::from_raw(
            2,
            2,
            vec![
                255, 0, 0, 0, 0, 255, 0, 128, 0, 0, 255, 255, 42, 84, 126, 17,
            ],
        )
        .unwrap();
        let mut writer = std::io::Cursor::new(Vec::new());
        let signal = Cancellation::default();
        let profile = encode(&mut writer, &pixels, &signal).unwrap();
        writer.set_position(0);
        verify(&mut writer, &pixels, &profile, &signal).unwrap();
        writer.set_position(0);
        assert!(verify(&mut writer, &image::RgbaImage::new(1, 1), &profile, &signal).is_err());
    }
}
