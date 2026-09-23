// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, native_process, pdf_inspect, pdf_pages, pdf_preservation, Cancellation, Result,
    Source,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfPageSelection {
    pub source_index: usize,
    pub page_index: u32,
    #[serde(default)]
    pub clockwise_rotation: i32,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    pub pages: Option<Vec<PdfPageSelection>>,
    pub allow_document_changes: bool,
}
#[derive(Debug, Serialize)]
pub struct CompositionReceipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub source_sha256: Vec<String>,
    pub pages: usize,
    pub warnings: Vec<&'static str>,
}
pub fn compose(options: &Composition, cancel: &Cancellation) -> Result<CompositionReceipt> {
    if !options.allow_document_changes {
        return Err(fail("unsupported", "PDF assembly creates a new document and cannot preserve document-level metadata, bookmarks, form behavior or signature certification."));
    }
    if !(1..=128).contains(&options.inputs.len())
        || !options
            .output
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("pdf"))
    {
        return Err(fail(
            "invalid_request",
            "Choose 1–128 PDF, PNG, JPEG or TIFF inputs and a PDF output.",
        ));
    }
    if options
        .pages
        .as_ref()
        .is_some_and(|pages| !(1..=1000).contains(&pages.len()))
    {
        return Err(fail("limit", "PDF assembly supports 1–1000 output pages."));
    }
    if options.output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let mut state = prepare(&options.inputs, &options.directory, cancel)?;
    compose_prepared(options, &mut state, cancel, 512 * 1024 * 1024)
}

pub(crate) struct Prepared {
    sources: Vec<Source>,
    geometries: Vec<Vec<pdf_pages::PageGeometry>>,
    prepared: Vec<tempfile::NamedTempFile>,
    documents: Vec<PathBuf>,
}
pub(crate) fn prepare(
    inputs: &[PathBuf],
    directory: &Path,
    cancel: &Cancellation,
) -> Result<Prepared> {
    if !(1..=128).contains(&inputs.len()) {
        return Err(fail("limit", "Choose 1–128 sources."));
    }
    let mut sources = Vec::new();
    let mut geometries = Vec::new();
    let mut prepared = Vec::new();
    let mut documents = Vec::new();
    let mut prepared_bytes = 0;
    let mut total = 0;
    for input in inputs {
        let mut source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024 - total)?;
        total += source.snapshot.as_file().metadata()?.len();
        let mut signature = [0u8; 8];
        let read = source.snapshot_reader()?.read(&mut signature)?;
        let is_image = read >= 2
            && (signature[..2] == [0xff, 0xd8]
                || signature[..2] == *b"II"
                || signature[..2] == *b"MM"
                || signature.starts_with(b"\x89PNG"));
        let document = if is_image {
            let page = crate::pdf_images::prepare(
                source.snapshot.path(),
                directory,
                512 * 1024 * 1024 - prepared_bytes,
                cancel,
            )?;
            prepared_bytes += page.as_file().metadata()?.len();
            let path = page.path().to_path_buf();
            prepared.push(page);
            path
        } else {
            source.snapshot.path().to_path_buf()
        };
        let info = pdf_inspect::inspect(&document, directory, cancel)?;
        let geometry = pdf_pages::inspect(&document, directory, cancel)?.pages;
        documents.push(document);
        if geometry.len() != info.pages as usize {
            return Err(fail("verification", "PDF page inspections disagree."));
        }
        sources.push(source);
        geometries.push(geometry);
    }
    Ok(Prepared {
        sources,
        geometries,
        prepared,
        documents,
    })
}
pub(crate) fn individual_pages(state: &Prepared) -> Vec<Vec<PdfPageSelection>> {
    state
        .geometries
        .iter()
        .enumerate()
        .flat_map(|(source_index, pages)| {
            (0..pages.len()).map(move |page_index| {
                vec![PdfPageSelection {
                    source_index,
                    page_index: page_index as u32,
                    clockwise_rotation: 0,
                }]
            })
        })
        .collect()
}
pub(crate) fn compose_prepared(
    options: &Composition,
    state: &mut Prepared,
    cancel: &Cancellation,
    maximum_bytes: u64,
) -> Result<CompositionReceipt> {
    if maximum_bytes == 0 {
        return Err(fail("limit", "The PDF output byte budget is exhausted."));
    }
    let Prepared {
        sources,
        geometries,
        prepared,
        documents,
    } = state;
    let all;
    let selected = match &options.pages {
        Some(pages) => pages,
        None => {
            all = geometries
                .iter()
                .enumerate()
                .flat_map(|(source_index, pages)| {
                    (0..pages.len()).map(move |page_index| PdfPageSelection {
                        source_index,
                        page_index: page_index as u32,
                        clockwise_rotation: 0,
                    })
                })
                .collect::<Vec<_>>();
            &all
        }
    };
    if !(1..=1000).contains(&selected.len()) {
        return Err(fail("limit", "PDF assembly supports 1–1000 output pages."));
    }
    let mut expected = Vec::with_capacity(selected.len());
    for selection in selected {
        let mut geometry = geometries
            .get(selection.source_index)
            .and_then(|v| v.get(selection.page_index as usize))
            .cloned()
            .ok_or_else(|| {
                fail(
                    "invalid_request",
                    "A selected page is outside its source document.",
                )
            })?;
        if selection.clockwise_rotation % 90 != 0 {
            return Err(fail(
                "invalid_request",
                "Choose a rotation in multiples of 90 degrees.",
            ));
        }
        geometry.rotation =
            (geometry.rotation + selection.clockwise_rotation.rem_euclid(360) as u32) % 360;
        expected.push(geometry);
    }
    // Group source verification without changing the requested output order.
    let mut proof = (0..selected.len()).map(|_| None).collect::<Vec<_>>();
    for (source_index, document) in documents.iter().enumerate() {
        let positions = selected
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (v.source_index == source_index).then_some(i))
            .collect::<Vec<_>>();
        if positions.is_empty() {
            continue;
        }
        let geometry = positions
            .iter()
            .map(|&i| expected[i].clone())
            .collect::<Vec<_>>();
        let verified =
            pdf_preservation::pages(document, &options.renderer_directory, &geometry, cancel)?;
        for (position, verified) in positions.into_iter().zip(verified) {
            proof[position] = Some(verified);
        }
    }
    let parent = options
        .output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let working = tempfile::tempdir()?;
    let mut job = tempfile::NamedTempFile::new_in(working.path())?;
    let pages = selected.iter().map(|v| serde_json::json!({"file": documents[v.source_index], "range":(v.page_index + 1).to_string()})).collect::<Vec<_>>();
    let rotation = expected
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{}:{}", v.rotation, i + 1))
        .collect::<Vec<_>>();
    serde_json::to_writer(
        job.as_file_mut(),
        &serde_json::json!({"empty":"", "outputFile": temporary.path(), "pages": pages, "rotate":rotation}),
    )?;
    job.flush()?;
    let mut command = pdf_inspect::command(&options.directory)?;
    let mut job_argument = std::ffi::OsString::from("--job-json-file=");
    job_argument.push(job.path());
    command.arg(job_argument);
    native_process::run_with_output_limit(
        command,
        cancel,
        Duration::from_secs(120),
        64 * 1024,
        Some((temporary.path(), maximum_bytes)),
    )?;
    let after = pdf_inspect::inspect(temporary.path(), &options.directory, cancel)?;
    let geometry = pdf_pages::inspect(temporary.path(), &options.directory, cancel)?.pages;
    for (i, page) in expected.iter_mut().enumerate() {
        page.position = i as u32 + 1;
    }
    if after.pages as usize != selected.len() || geometry != expected {
        return Err(fail(
            "verification",
            "Assembled PDF page order or geometry did not match the selection.",
        ));
    }
    let after = pdf_preservation::pages(
        temporary.path(),
        &options.renderer_directory,
        &geometry,
        cancel,
    )?;
    if !proof
        .iter()
        .zip(&after)
        .all(|(before, after)| before.as_ref() == Some(after))
    {
        return Err(fail(
            "verification",
            "Assembled PDF page text or appearance changed. No output was saved.",
        ));
    }
    let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, maximum_bytes)?;
    for (source, input) in sources.iter_mut().zip(&options.inputs) {
        source.check(input)?;
    }
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
    let mut warnings = vec!["A new PDF is created. Document-level metadata, bookmarks and form behavior may change; existing signatures do not certify it. Originals remain unchanged."];
    if !prepared.is_empty() {
        warnings.push("Each image becomes one sRGB page at one PDF point per oriented pixel. Descriptive metadata is omitted; transparency is retained as a soft mask.");
    }
    Ok(CompositionReceipt {
        output: options.output.clone(),
        bytes,
        sha256,
        source_sha256: sources.iter().map(|v| v.hash.clone()).collect(),
        pages: selected.len(),
        warnings,
    })
}

pub(crate) fn check_sources(state: &mut Prepared, inputs: &[PathBuf]) -> Result<()> {
    for (source, input) in state.sources.iter_mut().zip(inputs) {
        source.check(input)?;
    }
    Ok(())
}
