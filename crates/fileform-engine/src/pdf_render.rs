// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, native_pack, native_process, png_pipeline, Cancellation, Result, Source,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfRenderBox {
    #[default]
    Crop,
    Media,
}
impl PdfRenderBox {
    fn argument(self) -> &'static str {
        match self {
            Self::Crop => "crop",
            Self::Media => "media",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RenderReceipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: String,
    pub width: u32,
    pub height: u32,
    pub page_index: u32,
    pub page_box: PdfRenderBox,
    pub renderer_version: String,
}

fn invalid() -> crate::Failure {
    fail(
        "verification",
        "The PDF renderer returned an invalid image.",
    )
}
fn raster(bytes: &[u8], maximum: u32) -> Result<(u32, u32, &[u8])> {
    let mut lines = bytes.splitn(4, |b| *b == b'\n');
    if lines.next() != Some(b"P6") {
        return Err(invalid());
    }
    let dimensions = lines.next().filter(|v| v.len() <= 9).ok_or_else(invalid)?;
    let mut parts = dimensions.split(|b| *b == b' ');
    let mut dimension = || -> Result<u32> {
        let part = parts.next().ok_or_else(invalid)?;
        if part.is_empty() || !part.iter().all(u8::is_ascii_digit) {
            return Err(invalid());
        }
        std::str::from_utf8(part)
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|v| (1..=maximum).contains(v))
            .ok_or_else(invalid)
    };
    let width = dimension()?;
    let height = dimension()?;
    if parts.next().is_some() || lines.next() != Some(b"255") {
        return Err(invalid());
    }
    let pixels = lines.next().ok_or_else(invalid)?;
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|v| v.checked_mul(3))
        .ok_or_else(invalid)?;
    if pixels.len() != expected {
        return Err(invalid());
    }
    Ok((width, height, pixels))
}

pub fn render(
    input: &Path,
    output: &Path,
    directory: &Path,
    page: u32,
    maximum: u32,
    page_box: PdfRenderBox,
    cancel: &Cancellation,
) -> Result<RenderReceipt> {
    if page >= 1000
        || !(1..=2048).contains(&maximum)
        || !output
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("png"))
    {
        return Err(fail(
            "invalid_request",
            "Choose a page index from 0–999, PNG output, and a bound from 1–2048 pixels.",
        ));
    }
    if output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let library = if cfg!(windows) {
        "pdfium.dll"
    } else {
        "libpdfium.dylib"
    };
    let pack = native_pack::verify_with_libraries(
        directory,
        cancel,
        "app.fileform.pdf-render",
        &["fileform-pdf-render"],
        &[library],
        false,
    )?;
    let mut source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024)?;
    let root = directory.canonicalize()?;
    let mut command = Command::new(root.join(if cfg!(windows) {
        "bin/fileform-pdf-render.exe"
    } else {
        "bin/fileform-pdf-render"
    }));
    command.env_clear();
    #[cfg(windows)]
    if let Some(system) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system);
    }
    let working = tempfile::tempdir()?;
    command
        .current_dir(working.path())
        .arg(source.snapshot.path())
        .arg(page.to_string())
        .arg(maximum.to_string())
        .arg(page_box.argument());
    let bytes = native_process::run(
        command,
        cancel,
        Duration::from_secs(60),
        maximum as usize * maximum as usize * 3 + 64,
    )?;
    let (width, height, pixels) = raster(&bytes, maximum)?;
    cancel.check()?;
    let parent = output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut encoder = png::Encoder::new(temporary.as_file_mut(), width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|_| invalid())?;
        writer.write_image_data(pixels).map_err(|_| invalid())?;
        writer.finish().map_err(|_| invalid())?;
    }
    cancel.check()?;
    let decoded =
        png_pipeline::decode_bounded(BufReader::new(File::open(temporary.path())?), maximum)?;
    if decoded.pixels.dimensions() != (width, height)
        || decoded
            .pixels
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(pixels.as_chunks::<3>().0.iter())
            .any(|(rgba, rgb)| rgba[..3] != *rgb || rgba[3] != 255)
    {
        return Err(invalid());
    }
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 20 * 1024 * 1024)?;
    source.check(input)?;
    if identity != same_file::Handle::from_path(parent)? {
        return Err(fail("output_changed", "Output folder changed."));
    }
    temporary.as_file().sync_all()?;
    cancel.check()?;
    temporary.persist_noclobber(output).map_err(|e| {
        fail(
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                "collision"
            } else {
                "io"
            },
            e.error.to_string(),
        )
    })?;
    Ok(RenderReceipt {
        output: output.into(),
        bytes,
        sha256,
        source_sha256: source.hash,
        width,
        height,
        page_index: page,
        page_box,
        renderer_version: pack.version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_truncated_or_ambiguous_rasters() {
        assert_eq!(
            raster(b"P6\n1 1\n255\n\x01\x02\x03", 1).unwrap(),
            (1, 1, &[1, 2, 3][..])
        );
        for bytes in [
            b"P6\n1 1\n255\n\x01\x02".as_slice(),
            b"P6\n1 1\n255\n1234",
            b"P6\n2049 1\n255\n",
            b"P6\n0 0\n255\n",
            b"P6\n+1 1\n255\n123",
            b"P6\n1 1 1\n255\n123",
            b"P6\n1 1\n256\n123",
            b"P3\n1 1\n255\n123",
        ] {
            assert!(raster(bytes, 2048).is_err());
        }
    }
}
