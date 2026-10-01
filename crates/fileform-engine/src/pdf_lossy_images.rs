// SPDX-License-Identifier: Apache-2.0
use crate::{
    fail, image_resize, jpeg_input, pdf_embedded_encode, pdf_image_graph::ImageObject,
    Cancellation, LimitedWriter, Result,
};
use std::{
    io::{BufReader, Read},
    path::Path,
};
pub(crate) fn decode(
    input: &Path,
    pack: &Path,
    image: &ImageObject,
    cancel: &Cancellation,
) -> Result<image::RgbaImage> {
    let width = image
        .candidate
        .width
        .ok_or_else(|| fail("verification", "Missing image width."))?;
    let height = image
        .candidate
        .height
        .ok_or_else(|| fail("verification", "Missing image height."))?;
    let count = u64::from(width) * u64::from(height);
    let jpeg = image.candidate.filters == ["/DCTDecode"];
    let mut file = pdf_embedded_encode::stream(
        input,
        pack,
        &image.reference,
        jpeg,
        if jpeg {
            256 * 1024 * 1024
        } else {
            count * image.channels as u64
        },
        cancel,
    )?;
    if jpeg {
        let decoded = jpeg_input::decode_pdf_image(BufReader::new(file.as_file_mut()),width,height,image.channels==1)
            .map_err(|error| if error.code=="verification" {fail("unsupported","JPEG orientation or sample interpretation is unsupported; original image retained.")}else{error})?;
        if decoded.has_icc
            || decoded.icc_profile.is_some()
            || decoded.preservation_pending
            || decoded.has_hdr_metadata
        {
            return Err(fail(
                "unsupported",
                "JPEG profile or sample interpretation is unsupported; original image retained.",
            ));
        }
        return Ok(decoded.pixels);
    }
    if file.as_file().metadata()?.len() != count * image.channels as u64 {
        return Err(fail(
            "verification",
            "Decoded PDF image sample count is incorrect.",
        ));
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count as usize * 4)
        .map_err(|_| fail("limit", "Not enough memory to decode the image."))?;
    let mut buffer = vec![0u8; 4096 * image.channels];
    let mut remaining = count;
    while remaining > 0 {
        cancel.check()?;
        let length = remaining.min(4096) as usize * image.channels;
        file.as_file_mut().read_exact(&mut buffer[..length])?;
        for sample in buffer[..length].chunks(image.channels) {
            pixels.extend_from_slice(&[
                sample[0],
                sample[usize::from(image.channels == 3)],
                sample[if image.channels == 3 { 2 } else { 0 }],
                255,
            ]);
        }
        remaining -= length as u64 / image.channels as u64;
    }
    image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| fail("verification", "Invalid PDF image pixel layout."))
}
pub(crate) fn encode(
    input: &Path,
    pack: &Path,
    image: &ImageObject,
    output: &Path,
    dimensions: (u32, u32),
    quality: u8,
    cancel: &Cancellation,
) -> Result<()> {
    let (width, height) = dimensions;
    let pixels = decode(input, pack, image, cancel)?;
    let pixels = image_resize::exact(pixels, width, height, cancel)?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(width as usize * height as usize * image.channels)
        .map_err(|_| fail("limit", "Not enough memory for JPEG samples."))?;
    for (i, pixel) in pixels.pixels().enumerate() {
        if i % 4096 == 0 {
            cancel.check()?;
        }
        if image.channels == 1 {
            samples.push(pixel[0]);
        } else {
            samples.extend_from_slice(&pixel.0[..3]);
        }
    }
    drop(pixels);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let limited = LimitedWriter {
        inner: file,
        cancellation: cancel.clone(),
        bytes: 0,
        maximum_bytes: 128 * 1024 * 1024,
    };
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(limited, quality);
    encoder
        .encode(
            &samples,
            width,
            height,
            if image.channels == 1 {
                image::ExtendedColorType::L8
            } else {
                image::ExtendedColorType::Rgb8
            },
        )
        .map_err(|e| fail("encoding", e.to_string()))?;
    drop(encoder);
    drop(samples);
    cancel.check()?;
    let verified = jpeg_input::decode_pdf_image(
        BufReader::new(std::fs::File::open(output)?),
        width,
        height,
        image.channels == 1,
    )?;
    if verified.has_icc || verified.icc_profile.is_some() {
        return Err(fail(
            "verification",
            "Replacement JPEG unexpectedly contains a color profile.",
        ));
    }
    Ok(())
}
