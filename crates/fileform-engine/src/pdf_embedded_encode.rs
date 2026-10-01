// SPDX-License-Identifier: Apache-2.0
use crate::{
    fail, jpeg_input, native_process,
    pdf_image_graph::{self, ImageObject},
    pdf_inspect, Cancellation, LimitedWriter, Result,
};
use sha2::{Digest, Sha256};
use std::{
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    time::Duration,
};
fn invalid() -> crate::Failure {
    fail(
        "verification",
        "The embedded image samples or encoded output failed verification.",
    )
}
pub(crate) fn stream(
    input: &Path,
    pack: &Path,
    reference: &str,
    raw: bool,
    maximum: u64,
    cancel: &Cancellation,
) -> Result<tempfile::NamedTempFile> {
    let (number, generation) = pdf_image_graph::reference(reference)?;
    let mut file = tempfile::NamedTempFile::new()?;
    let mut command = pdf_inspect::command(pack)?;
    command
        .arg(format!("--show-object={number},{generation}"))
        .arg(if raw {
            "--raw-stream-data"
        } else {
            "--filtered-stream-data"
        })
        .arg(input);
    native_process::run_to_file(command, cancel, Duration::from_secs(60), &mut file, maximum)?;
    Ok(file)
}
pub(crate) fn encode(
    input: &Path,
    pack: &Path,
    image: &ImageObject,
    destination: &Path,
    maximum: u64,
    cancel: &Cancellation,
) -> Result<()> {
    let candidate = &image.candidate;
    let (width, height) = candidate.width.zip(candidate.height).ok_or_else(invalid)?;
    let pixels = u64::from(width) * u64::from(height);
    let jpeg = candidate.encoding_outcome == Some("preserved_encoded_bytes");
    let mut samples = stream(
        input,
        pack,
        &image.reference,
        jpeg,
        if jpeg {
            256 * 1024 * 1024
        } else {
            pixels * image.channels as u64
        },
        cancel,
    )?;
    if jpeg {
        let decoded = jpeg_input::decode_pdf_image(
            BufReader::new(samples.as_file_mut()),
            width,
            height,
            image.channels == 1,
        )?;
        drop(decoded);
        samples.as_file_mut().seek(SeekFrom::Start(0))?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let mut writer = LimitedWriter {
            inner: file,
            cancellation: cancel.clone(),
            bytes: 0,
            maximum_bytes: maximum,
        };
        let mut buffer = [0u8; 65536];
        loop {
            cancel.check()?;
            let count = samples.as_file_mut().read(&mut buffer)?;
            if count == 0 {
                break;
            }
            writer.write_all(&buffer[..count])?;
        }
        writer.flush()?;
        drop(writer);
        let original = crate::digest(samples.as_file_mut(), cancel, 256 * 1024 * 1024)?;
        let copied = crate::digest(&mut std::fs::File::open(destination)?, cancel, maximum)?;
        if original != copied {
            return Err(invalid());
        }
        return Ok(());
    }
    if samples.as_file().metadata()?.len() != pixels * image.channels as u64 {
        return Err(invalid());
    }
    let mut alpha = match &image.soft_mask {
        Some(reference) => Some(stream(input, pack, reference, false, pixels, cancel)?),
        None => None,
    };
    if alpha
        .as_ref()
        .is_some_and(|v| v.as_file().metadata().map_or(true, |m| m.len() != pixels))
    {
        return Err(invalid());
    }
    let mut expected = Sha256::new();
    {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let limited = LimitedWriter {
            inner: file,
            cancellation: cancel.clone(),
            bytes: 0,
            maximum_bytes: maximum,
        };
        let mut encoder = png::Encoder::new(limited, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(|_| invalid())?;
        let mut output = writer
            .stream_writer_with_size(65536)
            .map_err(|_| invalid())?;
        let mut colors = vec![0u8; width as usize * image.channels];
        let mut mask = vec![255u8; width as usize];
        let mut rgba = vec![0u8; width as usize * 4];
        for _ in 0..height {
            cancel.check()?;
            samples.as_file_mut().read_exact(&mut colors)?;
            if let Some(alpha) = &mut alpha {
                alpha.as_file_mut().read_exact(&mut mask)?;
            }
            for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let start = index * image.channels;
                pixel[0] = colors[start];
                pixel[1] = colors[start + usize::from(image.channels == 3)];
                pixel[2] = colors[start + if image.channels == 3 { 2 } else { 0 }];
                pixel[3] = mask[index];
            }
            expected.update(&rgba);
            output.write_all(&rgba)?;
        }
        output.finish().map_err(|_| invalid())?;
        writer.finish().map_err(|_| invalid())?;
    }
    let mut ending = [0u8; 12];
    let mut file = std::fs::File::open(destination)?;
    file.seek(SeekFrom::End(-12))?;
    file.read_exact(&mut ending)?;
    if ending != [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82] {
        return Err(invalid());
    }
    let mut options = png::DecodeOptions::default();
    options.set_ignore_adler32(false);
    options.set_skip_ancillary_crc_failures(false);
    let mut decoder =
        png::Decoder::new_with_options(BufReader::new(std::fs::File::open(destination)?), options);
    decoder.set_limits(png::Limits {
        bytes: 64 * 1024 * 1024,
    });
    let mut decoded = decoder.read_info().map_err(|_| invalid())?;
    let info = decoded.info();
    if info.width != width
        || info.height != height
        || info.bit_depth != png::BitDepth::Eight
        || info.color_type != png::ColorType::Rgba
        || info.interlaced
        || info.animation_control.is_some()
    {
        return Err(invalid());
    }
    let mut actual = Sha256::new();
    let mut rows = 0;
    while let Some(row) = decoded.next_row().map_err(|_| invalid())? {
        cancel.check()?;
        if row.data().len() != width as usize * 4 {
            return Err(invalid());
        }
        actual.update(row.data());
        rows += 1;
    }
    decoded.finish().map_err(|_| invalid())?;
    if rows != height || actual.finalize() != expected.finalize() {
        return Err(invalid());
    }
    Ok(())
}
