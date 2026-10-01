// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, native_process, pdf_pages, pdf_raster_plan, pdf_render, Cancellation,
    LimitedWriter, Result, Source,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageExport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    pub page_index: u32,
    pub dpi: u16,
    pub quality: Option<u8>,
}
#[derive(Debug, Serialize)]
pub struct RasterReceipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: String,
    pub page_index: u32,
    pub width: u32,
    pub height: u32,
    pub dpi: u16,
    pub pixels_per_meter: Option<u32>,
    pub format: &'static str,
    pub quality: Option<u8>,
    pub warnings: Vec<&'static str>,
}
fn invalid() -> crate::Failure {
    fail(
        "verification",
        "The PDF raster or encoded image failed verification.",
    )
}
fn line(reader: &mut impl BufRead, expected: &[u8]) -> Result<()> {
    let mut bytes = Vec::new();
    reader.take(33).read_until(b'\n', &mut bytes)?;
    if bytes != expected {
        return Err(invalid());
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Png,
    Jpeg,
}
pub(crate) fn export(
    options: &PageExport,
    format: Format,
    cancel: &Cancellation,
) -> Result<RasterReceipt> {
    validate(options, format)?;
    let mut source = Source::open_with_limit(&options.input, cancel.clone(), 512 * 1024 * 1024)?;
    let inspection = pdf_pages::inspect(source.snapshot.path(), &options.directory, cancel)?;
    if inspection
        .special_preservation_keys
        .iter()
        .any(|v| matches!(v.as_str(), "/Encrypt" | "/XFA"))
    {
        return Err(fail(
            "unsupported",
            "Page export requires an unencrypted PDF without XFA forms.",
        ));
    }
    let geometry = inspection
        .pages
        .get(options.page_index as usize)
        .ok_or_else(|| fail("invalid_request", "The selected PDF page does not exist."))?;
    let candidate = render_prepared(
        options,
        format,
        PreparedRaster {
            document: source.snapshot.path(),
            geometry,
            source_sha256: &source.hash,
        },
        512 * 1024 * 1024,
        cancel,
    )?;
    source.check(&options.input)?;
    candidate.publish(cancel)
}
pub(crate) fn validate(options: &PageExport, format: Format) -> Result<()> {
    let extension = options
        .output
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("");
    let valid = match format {
        Format::Png => extension.eq_ignore_ascii_case("png") && options.quality.is_none(),
        Format::Jpeg => {
            (extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg"))
                && options.quality.is_none_or(|v| (5..=100).contains(&v))
        }
    };
    if !(36..=600).contains(&options.dpi) || !valid {
        return Err(fail(
            "invalid_request",
            "Choose the matching image format, 36–600 DPI, and JPEG quality from 5 to 100.",
        ));
    }
    if options.output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    Ok(())
}
pub(crate) struct PreparedRaster<'a> {
    pub document: &'a Path,
    pub geometry: &'a pdf_pages::PageGeometry,
    pub source_sha256: &'a str,
}
pub(crate) struct RasterCandidate {
    temporary: tempfile::NamedTempFile,
    pub receipt: RasterReceipt,
    parent: PathBuf,
    identity: same_file::Handle,
}
impl RasterCandidate {
    pub(crate) fn publish(self, cancel: &Cancellation) -> Result<RasterReceipt> {
        if self.identity != same_file::Handle::from_path(self.parent)? {
            return Err(fail("output_changed", "Output folder changed."));
        }
        self.temporary.as_file().sync_all()?;
        cancel.check()?;
        self.temporary
            .persist_noclobber(&self.receipt.output)
            .map_err(|e| {
                fail(
                    if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                        "collision"
                    } else {
                        "io"
                    },
                    e.error.to_string(),
                )
            })?;
        Ok(self.receipt)
    }
}
pub(crate) fn render_prepared(
    options: &PageExport,
    format: Format,
    prepared: PreparedRaster<'_>,
    maximum_bytes: u64,
    cancel: &Cancellation,
) -> Result<RasterCandidate> {
    validate(options, format)?;
    if maximum_bytes == 0 {
        return Err(fail("limit", "Page-image output budget is exhausted."));
    }
    let plan = pdf_raster_plan::dimensions(prepared.geometry, options.dpi, 0)?;
    let working = tempfile::tempdir()?;
    let mut raster = tempfile::NamedTempFile::new_in(working.path())?;
    let (mut command, _) = pdf_render::command(&options.renderer_directory, cancel)?;
    command
        .current_dir(working.path())
        .arg(prepared.document)
        .arg(options.page_index.to_string())
        .arg(format!("raster:{}x{}", plan.width, plan.height))
        .arg("crop")
        .args(plan.crop_box.iter().map(|v| v.to_string()))
        .arg((plan.rotation / 90).to_string());
    let raster_bytes = native_process::run_to_file(
        command,
        cancel,
        Duration::from_secs(60),
        &mut raster,
        plan.decoded_rgb_bytes + 64,
    )?;
    let dimensions = format!("{} {}\n", plan.width, plan.height);
    if raster_bytes != 3 + dimensions.len() as u64 + 4 + plan.decoded_rgb_bytes {
        return Err(invalid());
    }
    let mut reader = BufReader::new(raster.as_file_mut());
    line(&mut reader, b"P6\n")?;
    line(&mut reader, dimensions.as_bytes())?;
    line(&mut reader, b"255\n")?;
    let parent = options
        .output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let quality = match format {
        Format::Png => None,
        Format::Jpeg => Some(options.quality.unwrap_or(85)),
    };
    let ppm = match format {
        Format::Png => Some(encode_png(
            &mut reader,
            &mut temporary,
            &plan,
            options.dpi,
            maximum_bytes,
            cancel,
        )?),
        Format::Jpeg => {
            encode_jpeg(
                &mut reader,
                &mut temporary,
                &plan,
                options.dpi,
                quality.expect("JPEG quality"),
                maximum_bytes,
                cancel,
            )?;
            None
        }
    };
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, maximum_bytes)?;
    let mut warnings=vec!["Rendered as an image. Searchable text, vectors, interactive behavior and document metadata are not retained."];
    if quality.is_some() {
        warnings.push("JPEG adds lossy encoding at the requested quality.");
    }
    let receipt = RasterReceipt {
        output: options.output.clone(),
        bytes,
        sha256,
        source_sha256: prepared.source_sha256.to_owned(),
        page_index: options.page_index,
        width: plan.width,
        height: plan.height,
        dpi: options.dpi,
        pixels_per_meter: ppm,
        format: match format {
            Format::Png => "png",
            Format::Jpeg => "jpeg",
        },
        quality,
        warnings,
    };
    Ok(RasterCandidate {
        temporary,
        receipt,
        parent: parent.into(),
        identity,
    })
}

fn encode_png(
    reader: &mut impl Read,
    temporary: &mut tempfile::NamedTempFile,
    plan: &pdf_raster_plan::RasterPagePlan,
    dpi: u16,
    maximum_bytes: u64,
    cancel: &Cancellation,
) -> Result<u32> {
    let ppm = (f64::from(dpi) / 0.0254).round() as u32;
    let mut expected = Sha256::new();
    {
        let limited = LimitedWriter {
            inner: temporary.as_file_mut(),
            cancellation: cancel.clone(),
            bytes: 0,
            maximum_bytes,
        };
        let mut encoder = png::Encoder::new(limited, plan.width, plan.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder.write_header().map_err(|_| invalid())?;
        let mut stream = writer
            .stream_writer_with_size(64 * 1024)
            .map_err(|_| invalid())?;
        let mut remaining = plan.decoded_rgb_bytes;
        let mut buffer = [0u8; 64 * 1024];
        while remaining > 0 {
            cancel.check()?;
            let count = remaining.min(buffer.len() as u64) as usize;
            reader.read_exact(&mut buffer[..count])?;
            expected.update(&buffer[..count]);
            stream.write_all(&buffer[..count])?;
            remaining -= count as u64;
        }
        stream.finish().map_err(|_| invalid())?;
        writer.finish().map_err(|_| invalid())?;
    }
    let mut encoded = temporary.reopen()?;
    encoded.seek(SeekFrom::End(-12))?;
    let mut ending = [0u8; 12];
    encoded.read_exact(&mut ending)?;
    if ending != [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82] {
        return Err(invalid());
    }
    encoded.seek(SeekFrom::Start(0))?;
    let mut decoding = png::DecodeOptions::default();
    decoding.set_ignore_adler32(false);
    decoding.set_skip_ancillary_crc_failures(false);
    let mut decoder = png::Decoder::new_with_options(BufReader::new(encoded), decoding);
    decoder.set_limits(png::Limits {
        bytes: 64 * 1024 * 1024,
    });
    let mut decoded = decoder.read_info().map_err(|_| invalid())?;
    let info = decoded.info();
    if info.width != plan.width
        || info.height != plan.height
        || info.color_type != png::ColorType::Rgb
        || info.bit_depth != png::BitDepth::Eight
        || info.interlaced
        || info.animation_control.is_some()
        || info.frame_control.is_some()
        || info.srgb != Some(png::SrgbRenderingIntent::Perceptual)
        || !info
            .pixel_dims
            .is_some_and(|v| v.xppu == ppm && v.yppu == ppm && v.unit == png::Unit::Meter)
    {
        return Err(invalid());
    }
    let mut actual = Sha256::new();
    let mut rows = 0u32;
    while let Some(row) = decoded.next_row().map_err(|_| invalid())? {
        cancel.check()?;
        if row.data().len() != plan.width as usize * 3 {
            return Err(invalid());
        }
        actual.update(row.data());
        rows += 1;
    }
    decoded.finish().map_err(|_| invalid())?;
    if rows != plan.height || actual.finalize() != expected.finalize() {
        return Err(invalid());
    }
    Ok(ppm)
}
fn encode_jpeg(
    reader: &mut impl Read,
    temporary: &mut tempfile::NamedTempFile,
    plan: &pdf_raster_plan::RasterPagePlan,
    dpi: u16,
    quality: u8,
    maximum_bytes: u64,
    cancel: &Cancellation,
) -> Result<()> {
    use image::ImageEncoder;
    let mut rgb = Vec::new();
    let length = usize::try_from(plan.decoded_rgb_bytes)
        .map_err(|_| fail("limit", "Raster buffer size overflow."))?;
    rgb.try_reserve_exact(length)
        .map_err(|_| fail("limit", "Not enough memory to encode the requested JPEG."))?;
    rgb.resize(length, 0);
    for chunk in rgb.chunks_mut(64 * 1024) {
        cancel.check()?;
        reader.read_exact(chunk)?;
    }
    let profile = moxcms::ColorProfile::new_srgb()
        .encode()
        .map_err(|e| fail("encoding", e.to_string()))?;
    {
        let limited = LimitedWriter {
            inner: temporary.as_file_mut(),
            cancellation: cancel.clone(),
            bytes: 0,
            maximum_bytes,
        };
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(limited, quality);
        encoder.set_pixel_density(image::codecs::jpeg::PixelDensity::dpi(dpi));
        encoder
            .set_icc_profile(profile.clone())
            .map_err(|_| invalid())?;
        encoder
            .encode(
                &rgb,
                plan.width,
                plan.height,
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| fail("encoding", e.to_string()))?;
    }
    drop(rgb); // Do not retain the raw RGB allocation while independently decoding.
    cancel.check()?;
    let mut file = temporary.reopen()?;
    let mut header = [0u8; 20];
    file.read_exact(&mut header)?;
    let mut expected = vec![
        0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 2, 1,
    ];
    expected.extend_from_slice(&dpi.to_be_bytes());
    expected.extend_from_slice(&dpi.to_be_bytes());
    expected.extend_from_slice(&[0, 0]);
    if header.as_slice() != expected {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(0))?;
    crate::jpeg_output::verify(
        BufReader::new(file),
        plan.width,
        plan.height,
        &profile,
        cancel,
    )
}
