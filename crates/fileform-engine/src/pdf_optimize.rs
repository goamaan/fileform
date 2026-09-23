// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, native_process, pdf_graph, pdf_inspect, pdf_pages, pdf_preservation,
    Cancellation, Result, Source,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Debug, Serialize)]
pub struct OptimizationReceipt {
    pub status: &'static str,
    pub output: Option<PathBuf>,
    pub input_bytes: u64,
    pub output_bytes: Option<u64>,
    pub source_sha256: String,
    pub output_sha256: Option<String>,
    pub pages_verified: u32,
    pub graph_sha256: String,
    pub warnings: Vec<&'static str>,
}
pub fn optimize(
    input: &Path,
    output: &Path,
    directory: &Path,
    renderer: &Path,
    maximum: Option<u64>,
    cancel: &Cancellation,
) -> Result<OptimizationReceipt> {
    if !output
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("pdf"))
        || maximum.is_some_and(|v| v == 0 || v > 512 * 1024 * 1024)
    {
        return Err(fail(
            "invalid_request",
            "Choose PDF output and a byte limit from 1 to 512 MiB.",
        ));
    }
    if output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let mut source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024)?;
    let input_bytes = source.snapshot.as_file().metadata()?.len();
    let before = pdf_inspect::inspect(source.snapshot.path(), directory, cancel)?;
    let graph = pdf_graph::inspect(source.snapshot.path(), directory, cancel)?.graph;
    if !graph.special_preservation_keys.is_empty() {
        return Err(fail("unsupported", "Lossless compression does not yet support signed, encrypted, annotated, tagged, outlined or interactive PDFs."));
    }
    let geometry = pdf_pages::inspect(source.snapshot.path(), directory, cancel)?;
    if geometry.pages.len() != before.pages as usize {
        return Err(fail("verification", "PDF page inspections disagree."));
    }
    let proof = pdf_preservation::pages(source.snapshot.path(), renderer, &geometry.pages, cancel)?;
    let parent = output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut command = pdf_inspect::command(directory)?;
    command
        .args([
            "--compress-streams=y",
            "--decode-level=generalized",
            "--recompress-flate",
            "--compression-level=9",
            "--object-streams=generate",
        ])
        .arg(source.snapshot.path())
        .arg(temporary.path());
    native_process::run_with_output_limit(
        command,
        cancel,
        Duration::from_secs(120),
        64 * 1024,
        Some((temporary.path(), 512 * 1024 * 1024)),
    )?;
    let after = pdf_inspect::inspect(temporary.path(), directory, cancel)?;
    let after_graph = pdf_graph::inspect(temporary.path(), directory, cancel)?.graph;
    let after_geometry = pdf_pages::inspect(temporary.path(), directory, cancel)?;
    if before.pages != after.pages
        || after_geometry.pages.len() != after.pages as usize
        || graph.graph_sha256 != after_graph.graph_sha256
        || geometry.pages != after_geometry.pages
        || !after_graph.special_preservation_keys.is_empty()
        || proof
            != pdf_preservation::pages(temporary.path(), renderer, &after_geometry.pages, cancel)?
    {
        return Err(fail("verification", "The compressed PDF did not preserve its graph, geometry, text and page images. No output was saved."));
    }
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 512 * 1024 * 1024)?;
    source.check(input)?;
    cancel.check()?;
    if maximum.is_some_and(|limit| bytes > limit) {
        return Err(fail(
            "target_unmet",
            "The PDF cannot meet this byte limit using lossless compression. No output was saved.",
        ));
    }
    let mut receipt = OptimizationReceipt { status: "not_smaller", output: None, input_bytes,
        output_bytes: None, source_sha256: source.hash, output_sha256: None,
        pages_verified: before.pages, graph_sha256: graph.graph_sha256,
        warnings: vec!["Structural compression may raise the PDF version to 1.5. Images are not resampled or recompressed with a lossy codec."] };
    if maximum.is_none() && bytes >= input_bytes {
        return Ok(receipt);
    }
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
    receipt.status = "saved";
    receipt.output = Some(output.into());
    receipt.output_bytes = Some(bytes);
    receipt.output_sha256 = Some(sha256);
    Ok(receipt)
}
