// SPDX-License-Identifier: Apache-2.0
use crate::{
    fail, native_process, pdf_inspect, prepare_image, Cancellation, LimitedWriter, Result,
};
use sha2::{Digest, Sha256};
use std::{
    io::{Seek, Write},
    path::Path,
    time::Duration,
};

/// Write only Fileform-generated objects: no document strings or external paths
/// are interpolated into PDF syntax. Samples remain straight-alpha sRGB.
pub(crate) fn prepare(
    input: &Path,
    directory: &Path,
    maximum: u64,
    cancel: &Cancellation,
) -> Result<tempfile::NamedTempFile> {
    let (_, inspection, pixels) = prepare_image(input, cancel)?;
    if !inspection.conversion_available {
        return Err(fail(
            "unsupported",
            "PDF assembly supports the verified SDR image pipeline only.",
        ));
    }
    let (width, height) = pixels.dimensions();
    let count = u64::from(width) * u64::from(height);
    let profile = moxcms::ColorProfile::new_srgb()
        .encode()
        .map_err(|e| fail("encoding", e.to_string()))?;
    let content = format!("q {width} 0 0 {height} 0 0 cm /Image Do Q\n");
    if count * 4 + profile.len() as u64 + 4096 > maximum {
        return Err(fail(
            "limit",
            "Prepared PDF image pages exceed the 512 MiB staging limit.",
        ));
    }
    let mut file = tempfile::NamedTempFile::new()?;
    let mut writer = LimitedWriter {
        inner: std::io::BufWriter::new(file.as_file_mut()),
        cancellation: cancel.clone(),
        bytes: 0,
        maximum_bytes: maximum,
    };
    writer.write_all(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")?;
    let mut offsets = vec![0u64];
    let mut object = |writer: &mut LimitedWriter<std::io::BufWriter<&mut std::fs::File>>,
                      id: u32|
     -> Result<()> {
        offsets.push(writer.stream_position()?);
        writeln!(writer, "{id} 0 obj")?;
        Ok(())
    };
    object(&mut writer, 1)?;
    writer.write_all(b"<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;
    object(&mut writer, 2)?;
    writer.write_all(b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>\nendobj\n")?;
    object(&mut writer, 3)?;
    writeln!(writer, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << /XObject << /Image 4 0 R >> >> /Contents 7 0 R >>\nendobj")?;
    let mut hashes = Vec::new();
    for (id, channels) in [(4, 3usize), (5, 1usize)] {
        object(&mut writer, id)?;
        let color = if id == 4 {
            "[/ICCBased 6 0 R] /SMask 5 0 R"
        } else {
            "/DeviceGray"
        };
        writeln!(writer, "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /BitsPerComponent 8 /ColorSpace {color} /Interpolate false /Length {} >>\nstream", count * channels as u64)?;
        let mut hash = Sha256::new();
        let mut row = Vec::with_capacity(4096 * channels);
        for source in pixels.as_raw().chunks(4096 * 4) {
            cancel.check()?;
            row.clear();
            for pixel in source.as_chunks::<4>().0 {
                if channels == 3 {
                    row.extend_from_slice(&pixel[..3]);
                } else {
                    row.push(pixel[3]);
                }
            }
            writer.write_all(&row)?;
            hash.update(&row);
        }
        hashes.push((
            id,
            count as usize * channels,
            <[u8; 32]>::from(hash.finalize()),
        ));
        writer.write_all(b"\nendstream\nendobj\n")?;
    }
    object(&mut writer, 6)?;
    writeln!(
        writer,
        "<< /N 3 /Alternate /DeviceRGB /Length {} >>\nstream",
        profile.len()
    )?;
    writer.write_all(&profile)?;
    writer.write_all(b"\nendstream\nendobj\n")?;
    hashes.push((6, profile.len(), Sha256::digest(&profile).into()));
    object(&mut writer, 7)?;
    writeln!(
        writer,
        "<< /Length {} >>\nstream\n{content}endstream\nendobj",
        content.len()
    )?;
    let xref = writer.stream_position()?;
    writer.write_all(b"xref\n0 8\n0000000000 65535 f \n")?;
    for offset in offsets.iter().skip(1) {
        writeln!(writer, "{offset:010} 00000 n ")?;
    }
    writeln!(
        writer,
        "trailer\n<< /Size 8 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF"
    )?;
    writer.flush()?;
    drop(writer);
    drop(pixels);
    // Independently parse the generated image, alpha and ICC streams before
    // using this page as a composition reference. Release decoded pixels first.
    pdf_inspect::verify_pack(directory, cancel)?;
    for (id, length, expected) in hashes {
        let mut command = pdf_inspect::command(directory)?;
        command
            .arg(format!("--show-object={id}"))
            .arg("--filtered-stream-data")
            .arg(file.path());
        let bytes = native_process::run(command, cancel, Duration::from_secs(60), length)?;
        if bytes.len() != length || <[u8; 32]>::from(Sha256::digest(&bytes)) != expected {
            return Err(fail(
                "verification",
                "PDF image samples, transparency or color profile changed.",
            ));
        }
    }
    Ok(file)
}
