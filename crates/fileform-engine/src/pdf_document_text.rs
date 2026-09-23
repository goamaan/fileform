// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, native_process, pdf_inspect, pdf_render, pdf_text, Cancellation, LimitedWriter,
    Result, Source,
};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfOcrOptions {
    pub directory: PathBuf,
    pub language: crate::OcrLanguage,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextExport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    #[serde(default)]
    pub allow_missing_text: bool,
    pub ocr: Option<PdfOcrOptions>,
}
#[derive(Debug, Serialize)]
pub struct DocumentTextReceipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: String,
    pub pages: u32,
    pub pages_without_embedded_text: Vec<u32>,
    pub ocr_performed: bool,
    pub ocr_pages: Vec<u32>,
    pub pages_without_recognized_text: Vec<u32>,
    pub ocr_language: Option<&'static str>,
    pub warnings: Vec<&'static str>,
}
pub fn export(options: &TextExport, cancel: &Cancellation) -> Result<DocumentTextReceipt> {
    if !options
        .output
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("txt"))
    {
        return Err(fail("invalid_request", "Choose a TXT output."));
    }
    if options.output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let mut source = Source::open_with_limit(&options.input, cancel.clone(), 512 * 1024 * 1024)?;
    let info = pdf_inspect::inspect(source.snapshot.path(), &options.directory, cancel)?;
    let parent = options
        .output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let working = tempfile::tempdir()?;
    let mut missing = Vec::new();
    let mut ocr_pages = Vec::new();
    let mut empty_pages = Vec::new();
    let mut fallback = options.ocr.as_ref().map(|config| DocumentOcr {
        input: source.snapshot.path().to_path_buf(),
        pdf_directory: &options.directory,
        renderer_directory: &options.renderer_directory,
        config,
        session: None,
        geometry: None,
        page_count: info.pages,
    });
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut expected = sha2::Sha256::new();
    use sha2::Digest;
    {
        let mut writer = LimitedWriter {
            inner: temporary.as_file_mut(),
            cancellation: cancel.clone(),
            bytes: 0,
            maximum_bytes: 512 * 1024 * 1024,
        };
        let mut version = None;
        for page in 0..info.pages {
            let (mut command, current) = pdf_render::command(&options.renderer_directory, cancel)?;
            if version.as_ref().is_some_and(|v| v != &current) {
                return Err(fail(
                    "engine_unavailable",
                    "The PDF renderer changed during extraction.",
                ));
            }
            version = Some(current);
            command
                .current_dir(working.path())
                .arg(source.snapshot.path())
                .arg(page.to_string())
                .arg("text");
            let timeout = deadline
                .checked_duration_since(Instant::now())
                .filter(|v| !v.is_zero())
                .ok_or_else(|| {
                    fail(
                        "limit",
                        "PDF text extraction exceeded ten minutes. No output was saved.",
                    )
                })?
                .min(Duration::from_secs(60));
            let bytes = native_process::run(command, cancel, timeout, 4_000_032)?;
            let mut text = std::borrow::Cow::Borrowed(pdf_text::decode(&bytes)?.trim());
            let mut did_ocr = false;
            if text.is_empty() {
                missing.push(page);
                if let Some(fallback) = &mut fallback {
                    did_ocr = true;
                    ocr_pages.push(page);
                    if let Some(recognized) = fallback.recognize(
                        page,
                        deadline,
                        version.as_deref().expect("recorded renderer"),
                        cancel,
                    )? {
                        text = std::borrow::Cow::Owned(recognized);
                    }
                    if text.is_empty() && info.pages == 1 {
                        return Err(fail(
                            "no_text",
                            "No text was recognized. No output was saved.",
                        ));
                    }
                } else if !options.allow_missing_text {
                    return Err(fail(
                        "ocr_required",
                        format!(
                            "Page {} has no embedded text and needs OCR. No output was saved.",
                            page + 1
                        ),
                    ));
                }
            }
            if text.is_empty() {
                empty_pages.push(page);
            }
            let mut write = |value: &[u8]| -> Result<()> {
                for chunk in value.chunks(64 * 1024) {
                    writer.write_all(chunk)?;
                    expected.update(chunk);
                }
                Ok(())
            };
            if page > 0 {
                write(b"\n\n\x0c\n\n")?;
            }
            if info.pages > 1 {
                write(format!("Page {}\n\n", page + 1).as_bytes())?;
            }
            write(if text.is_empty() {
                if did_ocr {
                    b"[No text recognized on this page.]"
                } else {
                    b"[No embedded text on this page; OCR required.]"
                }
            } else {
                text.as_bytes()
            })?;
        }
        writer.write_all(b"\n")?;
        expected.update(b"\n");
        writer.flush()?;
    }
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 512 * 1024 * 1024)?;
    let expected = expected
        .finalize()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    if sha256 != expected {
        return Err(fail(
            "verification",
            "The written text differs from the extracted pages.",
        ));
    }
    source.check(&options.input)?;
    if identity != same_file::Handle::from_path(parent)? {
        return Err(fail("output_changed", "Output folder changed."));
    }
    temporary.as_file().sync_all()?;
    cancel.check()?;
    temporary.persist_noclobber(&options.output).map_err(|e| {
        fail(
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                "collision"
            } else {
                "io"
            },
            e.error.to_string(),
        )
    })?;
    let ocr_performed = !ocr_pages.is_empty();
    Ok(DocumentTextReceipt {
        output: options.output.clone(),
        bytes,
        sha256,
        source_sha256: source.hash,
        pages: info.pages,
        pages_without_embedded_text: missing,
        ocr_performed,
        ocr_language: if ocr_performed { Some("eng") } else { None },
        ocr_pages,
        pages_without_recognized_text: empty_pages,
        warnings: vec![if ocr_performed {
            "English OCR was used on pages without embedded text. Review spelling, numbers and reading order; formatting and interactive features are omitted."
        } else {
            "Embedded text only; OCR was not performed. Images, formatting and interactive features are omitted. Review reading order."
        }],
    })
}

struct DocumentOcr<'a> {
    input: PathBuf,
    pdf_directory: &'a Path,
    renderer_directory: &'a Path,
    config: &'a PdfOcrOptions,
    session: Option<crate::ocr_image::OcrSession>,
    geometry: Option<Vec<crate::pdf_pages::PageGeometry>>,
    page_count: u32,
}
impl DocumentOcr<'_> {
    fn recognize(
        &mut self,
        page: u32,
        deadline: Instant,
        expected_version: &str,
        cancel: &Cancellation,
    ) -> Result<Option<String>> {
        if self.geometry.is_none() {
            let inspection = crate::pdf_pages::inspect(&self.input, self.pdf_directory, cancel)?;
            if inspection
                .special_preservation_keys
                .iter()
                .any(|v| matches!(v.as_str(), "/Annots" | "/AcroForm" | "/XFA" | "/Encrypt"))
            {
                return Err(fail("unsupported", "PDF OCR does not yet support encrypted documents, annotations or form appearances. No output was saved."));
            }
            let pages = inspection.pages;
            if pages.len() != self.page_count as usize {
                return Err(fail("verification", "PDF page inspections disagree."));
            }
            self.geometry = Some(pages);
        }
        if self.session.is_none() {
            self.session = Some(crate::ocr_image::OcrSession::new(
                &self.config.directory,
                &self.config.language,
                cancel,
            )?);
        }
        let geometry = self
            .geometry
            .as_ref()
            .and_then(|v| v.get(page as usize))
            .ok_or_else(|| fail("verification", "Missing PDF page geometry."))?;
        let (mut command, version) = pdf_render::command(self.renderer_directory, cancel)?;
        if version != expected_version {
            return Err(fail(
                "engine_unavailable",
                "The PDF renderer changed during OCR.",
            ));
        }
        command
            .arg(&self.input)
            .arg(page.to_string())
            .args(["ocr", "media"])
            .args(geometry.media_box.iter().map(|v| v.to_string()))
            .arg((geometry.rotation / 90).to_string());
        let raster =
            native_process::run(command, cancel, remaining(deadline)?, 4096 * 4096 * 3 + 64)?;
        pdf_render::raster(&raster, 4096)?;
        let session = self.session.as_ref().expect("initialized OCR session");
        {
            let mut file = session.create_raster()?;
            for chunk in raster.chunks(65536) {
                cancel.check()?;
                file.write_all(chunk)?;
            }
            file.flush()?;
        }
        drop(raster);
        session.recognize(cancel, remaining(deadline)?)
    }
}
fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|v| !v.is_zero())
        .map(|v| v.min(Duration::from_secs(60)))
        .ok_or_else(|| {
            fail(
                "limit",
                "PDF text extraction exceeded ten minutes. No output was saved.",
            )
        })
}
