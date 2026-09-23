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
pub struct TextExport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    #[serde(default)]
    pub allow_missing_text: bool,
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
            let text = pdf_text::decode(&bytes)?.trim();
            if text.is_empty() {
                missing.push(page);
                if !options.allow_missing_text {
                    return Err(fail(
                        "ocr_required",
                        format!(
                            "Page {} has no embedded text and needs OCR. No output was saved.",
                            page + 1
                        ),
                    ));
                }
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
                b"[No embedded text on this page; OCR required.]"
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
    Ok(DocumentTextReceipt { output:options.output.clone(),bytes,sha256,source_sha256:source.hash,pages:info.pages,pages_without_embedded_text:missing,ocr_performed:false,
        warnings:vec!["Embedded text only; OCR was not performed. Images, formatting and interactive features are omitted. Review reading order."] })
}
