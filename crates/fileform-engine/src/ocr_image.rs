// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, image_resize, native_process, ocr_pack, prepare_image, Cancellation, Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrLanguage {
    Eng,
}
#[derive(Debug, Serialize)]
pub struct OcrReceipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: String,
    pub language: &'static str,
    pub ocr_performed: bool,
    pub automatic_language_detection: bool,
    pub raster_width: u32,
    pub raster_height: u32,
    pub warnings: Vec<&'static str>,
}
fn copy_model(
    directory: &Path,
    working: &Path,
    expected: &str,
    cancel: &Cancellation,
) -> Result<()> {
    let input = directory.join("tessdata/eng.traineddata");
    if !std::fs::metadata(&input)?.is_file() {
        return Err(fail(
            "engine_unavailable",
            "The OCR model is not a regular file.",
        ));
    }
    let mut input = File::open(input)?;
    if !input.metadata()?.is_file() {
        return Err(fail(
            "engine_unavailable",
            "The OCR model is not a regular file.",
        ));
    }
    std::fs::create_dir(working.join("tessdata"))?;
    let mut output = File::create(working.join("tessdata/eng.traineddata"))?;
    let mut hash = Sha256::new();
    let mut total = 0;
    let mut buffer = [0u8; 65536];
    loop {
        cancel.check()?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count;
        if total > 64 * 1024 * 1024 {
            return Err(fail("limit", "The OCR model exceeds its size limit."));
        }
        output.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
    }
    output.flush()?;
    let actual = hash
        .finalize()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    if actual != expected {
        return Err(fail(
            "engine_unavailable",
            "The OCR model changed during preparation.",
        ));
    }
    Ok(())
}
pub fn recognize(
    input: &Path,
    output: &Path,
    directory: &Path,
    language: &OcrLanguage,
    cancel: &Cancellation,
) -> Result<OcrReceipt> {
    if !output
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("txt"))
    {
        return Err(fail("invalid_request", "Choose a TXT output for OCR."));
    }
    if output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let session = OcrSession::new(directory, language, cancel)?;
    let (mut source, inspection, pixels) = prepare_image(input, cancel)?;
    if !inspection.conversion_available {
        return Err(fail(
            "unsupported",
            "OCR requires a supported SDR still image.",
        ));
    }
    let pixels = image_resize::limit(pixels, 4096, cancel)?;
    let (width, height) = pixels.dimensions();
    {
        let mut file = std::io::BufWriter::new(session.create_raster()?);
        writeln!(file, "P6\n{width} {height}\n255")?;
        let mut block = Vec::with_capacity(4096 * 3);
        for chunk in pixels.as_raw().chunks(4096 * 4) {
            cancel.check()?;
            block.clear();
            for pixel in chunk.as_chunks::<4>().0 {
                let alpha = u32::from(pixel[3]);
                for color in &pixel[..3] {
                    block.push(
                        ((u32::from(*color) * alpha + 255 * (255 - alpha) + 127) / 255) as u8,
                    );
                }
            }
            file.write_all(&block)?;
        }
        file.flush()?;
    }
    drop(pixels);
    let text = session
        .recognize(cancel, Duration::from_secs(60))?
        .ok_or_else(|| {
            fail(
                "no_text",
                "No text was recognized. Try a sharper or higher-contrast image.",
            )
        })?;
    let parent = output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(text.as_bytes())?;
    temporary.write_all(b"\n")?;
    temporary.flush()?;
    let mut expected = Sha256::new();
    expected.update(text.as_bytes());
    expected.update(b"\n");
    let expected = expected
        .finalize()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 4_000_001)?;
    if sha256 != expected {
        return Err(fail(
            "verification",
            "Saved OCR text differs from the recognized result.",
        ));
    }
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
    Ok(OcrReceipt {
        output: output.into(),
        bytes,
        sha256,
        source_sha256: source.hash,
        language: session.language,
        ocr_performed: true,
        automatic_language_detection: false,
        raster_width: width,
        raster_height: height,
        warnings: vec!["English OCR only. Review spelling, numbers and reading order."],
    })
}

pub(crate) struct OcrSession {
    working: tempfile::TempDir,
    executable: PathBuf,
    pub(crate) language: &'static str,
}
impl OcrSession {
    pub(crate) fn new(
        directory: &Path,
        language: &OcrLanguage,
        cancel: &Cancellation,
    ) -> Result<Self> {
        let language = match language {
            OcrLanguage::Eng => "eng",
        };
        let pack = ocr_pack::verify(directory, cancel)?;
        let working = tempfile::tempdir()?;
        copy_model(
            directory,
            working.path(),
            &pack.assets["tessdata/eng.traineddata"],
            cancel,
        )?;
        let executable = directory.canonicalize()?.join(if cfg!(windows) {
            "bin/tesseract.exe"
        } else {
            "bin/tesseract"
        });
        Ok(Self {
            working,
            executable,
            language,
        })
    }
    pub(crate) fn create_raster(&self) -> Result<File> {
        Ok(File::create(self.working.path().join("input.ppm"))?)
    }
    pub(crate) fn recognize(
        &self,
        cancel: &Cancellation,
        timeout: Duration,
    ) -> Result<Option<String>> {
        let mut command = Command::new(&self.executable);
        command.env_clear();
        #[cfg(windows)]
        if let Some(system) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system);
        }
        // Only owned ASCII relative filenames reach the native parser. Model copies
        // are rehashed; arbitrary image formats, config files and user paths do not.
        command.current_dir(self.working.path()).args([
            "input.ppm",
            "stdout",
            "--tessdata-dir",
            "tessdata",
            "-l",
            self.language,
            "--oem",
            "1",
            "--psm",
            "3",
        ]);
        let result = native_process::run(
            command,
            cancel,
            timeout.min(Duration::from_secs(60)),
            4_000_000,
        )?;
        let text = std::str::from_utf8(&result)
            .map_err(|_| fail("verification", "OCR returned invalid UTF-8."))?
            .trim();
        if text.contains('\0') {
            return Err(fail("verification", "OCR returned invalid text."));
        }
        Ok(if text.is_empty() {
            None
        } else {
            Some(text.to_owned())
        })
    }
}
