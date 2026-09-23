// SPDX-License-Identifier: Apache-2.0
use crate::{digest, fail, native_process, pdf_render, Cancellation, Result, Source};
use serde::Serialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
const TEXT_LIMIT: usize = 4_000_000;
#[derive(Debug, Serialize)]
pub struct TextReceipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: String,
    pub page_index: u32,
    pub unicode_scalars: usize,
    pub renderer_version: String,
}
fn invalid() -> crate::Failure {
    fail(
        "verification",
        "The PDF helper returned invalid or incomplete text.",
    )
}
fn decode(bytes: &[u8]) -> Result<&str> {
    let mut lines = bytes.splitn(3, |b| *b == b'\n');
    if lines.next() != Some(b"FT1") {
        return Err(invalid());
    }
    let counts = lines.next().filter(|v| v.len() <= 15).ok_or_else(invalid)?;
    let mut parts = counts.split(|b| *b == b' ');
    let mut number = || -> Result<usize> {
        let part = parts.next().ok_or_else(invalid)?;
        if part.is_empty() || !part.iter().all(u8::is_ascii_digit) {
            return Err(invalid());
        }
        std::str::from_utf8(part)
            .ok()
            .and_then(|v| v.parse().ok())
            .ok_or_else(invalid)
    };
    let scalars = number()?;
    let length = number()?;
    if parts.next().is_some() || scalars > 1_000_000 || length > TEXT_LIMIT {
        return Err(invalid());
    }
    let text = std::str::from_utf8(lines.next().ok_or_else(invalid)?).map_err(|_| invalid())?;
    if text.len() != length || text.chars().count() != scalars || text.contains('\0') {
        return Err(invalid());
    }
    Ok(text)
}
pub fn extract(
    input: &Path,
    output: &Path,
    directory: &Path,
    page: u32,
    cancel: &Cancellation,
) -> Result<TextReceipt> {
    if page >= 1000
        || !output
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("txt"))
    {
        return Err(fail(
            "invalid_request",
            "Choose a page index from 0–999 and a TXT output.",
        ));
    }
    if output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let (mut command, version) = pdf_render::command(directory, cancel)?;
    let mut source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024)?;
    let working = tempfile::tempdir()?;
    command
        .current_dir(working.path())
        .arg(source.snapshot.path())
        .arg(page.to_string())
        .arg("text");
    let bytes = native_process::run(command, cancel, Duration::from_secs(60), TEXT_LIMIT + 32)?;
    let text = decode(&bytes)?;
    cancel.check()?;
    let parent = output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(text.as_bytes())?;
    temporary.flush()?;
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, TEXT_LIMIT as u64)?;
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
    Ok(TextReceipt {
        output: output.into(),
        bytes,
        sha256,
        source_sha256: source.hash,
        page_index: page,
        unicode_scalars: text.chars().count(),
        renderer_version: version,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_protocol_requires_complete_utf8_scalar_and_byte_counts() {
        assert_eq!(decode(b"FT1\n0 0\n").unwrap(), "");
        assert_eq!(decode("FT1\n2 6\nΩ😀".as_bytes()).unwrap(), "Ω😀");
        for bad in [
            b"".as_slice(),
            b"FT1\n0 0\nx",
            b"FT1\n1 1\n\xff",
            b"FT1\n1 1\n\0",
            b"FT1\n1 2\nx",
            b"FT1\n1000001 1\nx",
            b"FT1\n1 4000001\nx",
            b"FT1\n1 1 1\nx",
        ] {
            assert!(decode(bad).is_err());
        }
    }
}
