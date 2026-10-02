// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest,
    directory_transaction::DirectoryTransaction,
    fail, pdf_embedded_encode,
    pdf_image_graph::{ImageCandidate, ImageGraph, ImageObject},
    pdf_inspect, Cancellation, PdfPageSelection, Result, Source,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    #[serde(default)]
    pub collision: crate::OutputCollision,
    pub inputs: Vec<PathBuf>,
    pub expected_source_sha256: Option<Vec<String>>,
    pub output: Option<PathBuf>,
    pub directory: PathBuf,
    pub pages: Option<Vec<PdfPageSelection>>,
    pub page_ranges: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}
#[derive(Debug, Serialize)]
pub struct ExtractionReceipt {
    pub output: Option<PathBuf>,
    pub candidates: Vec<ImageCandidate>,
    pub discovered: usize,
    pub supported: usize,
    pub skipped: usize,
    pub source_sha256: Vec<String>,
    pub warnings: Vec<&'static str>,
}
pub fn extract(options: &Extraction, cancel: &Cancellation) -> Result<ExtractionReceipt> {
    if options.pages.is_some() && options.page_ranges.is_some() {
        return Err(fail(
            "invalid_request",
            "Choose explicit pages or page ranges, not both.",
        ));
    }
    if !(1..=128).contains(&options.inputs.len())
        || options
            .pages
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 1000)
    {
        return Err(fail(
            "invalid_request",
            "Choose 1–128 PDF sources and 1–1000 selected pages.",
        ));
    }
    if !options.dry_run && options.output.is_none() {
        return Err(fail("invalid_request", "Choose an output folder."));
    }
    let mut sources: Vec<Source> = Vec::new();
    let mut total = 0u64;
    let mut counts = Vec::new();
    for input in &options.inputs {
        let source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024 - total)?;
        if sources.iter().any(|v| v.identity == source.identity) {
            return Err(fail("invalid_request","Bind each physical PDF once; select repeated pages instead of duplicate files or aliases."));
        }
        total += source.snapshot.as_file().metadata()?.len();
        let info = pdf_inspect::inspect(source.snapshot.path(), &options.directory, cancel)?;
        counts.push(info.pages);
        sources.push(source);
    }
    crate::source_binding::multiple(&sources, options.expected_source_sha256.as_deref())?;
    let ranged = options
        .page_ranges
        .as_ref()
        .map(|v| {
            crate::pdf_selection::pages(v, &counts.iter().map(|v| *v as usize).collect::<Vec<_>>())
        })
        .transpose()?;
    let all;
    let pages = match options.pages.as_ref().or(ranged.as_ref()) {
        Some(pages) => pages,
        None => {
            if counts.iter().map(|v| *v as usize).sum::<usize>() > 1000 {
                return Err(fail(
                    "limit",
                    "Select at most 1000 pages for image extraction.",
                ));
            }
            all = counts
                .iter()
                .enumerate()
                .flat_map(|(source_index, &count)| {
                    (0..count).map(move |page_index| PdfPageSelection {
                        source_index,
                        page_index,
                        clockwise_rotation: 0,
                    })
                })
                .collect::<Vec<_>>();
            &all
        }
    };
    for page in pages {
        if page.clockwise_rotation != 0
            || counts
                .get(page.source_index)
                .is_none_or(|v| page.page_index >= *v)
        {
            return Err(fail(
                "invalid_request",
                "Embedded images use intrinsic pixels: choose existing pages with no rotation.",
            ));
        }
    }
    let mut images: Vec<ImageObject> = Vec::new();
    let mut samples = 0u64;
    let mut provenance = 0usize;
    for (source_index, source) in sources.iter().enumerate() {
        let selected = pages
            .iter()
            .filter(|v| v.source_index == source_index)
            .map(|v| v.page_index)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if selected.is_empty() {
            continue;
        }
        let graph = ImageGraph::load(source.snapshot.path(), &options.directory, cancel)?;
        let discovered = graph.discover(source_index, &selected, cancel)?;
        if images.len() + discovered.len() > 1000 {
            return Err(fail("limit", "Extract at most 1000 unique image objects."));
        }
        for image in &discovered {
            let c = &image.candidate;
            provenance += c.resource_paths.iter().map(String::len).sum::<usize>();
            if c.skip_reason.is_none() {
                samples += u64::from(c.width.unwrap_or(0)) * u64::from(c.height.unwrap_or(0)) * 4;
            }
        }
        if samples > 512 * 1024 * 1024 || provenance > 4 * 1024 * 1024 {
            return Err(fail(
                "limit",
                "Embedded images exceed the combined sample or provenance budget.",
            ));
        }
        images.extend(discovered);
    }
    let supported = images
        .iter()
        .filter(|v| v.candidate.skip_reason.is_none())
        .count();
    if !options.dry_run && supported == 0 {
        return Err(fail(
            "unsupported",
            if images.is_empty() {
                "No embedded image objects were found in selected PDF resources."
            } else {
                "All discovered image objects are unsupported. Inspect their skip reasons."
            },
        ));
    }
    let transaction = if options.dry_run {
        None
    } else {
        Some(DirectoryTransaction::with_collision(
            options.output.as_ref().expect("validated output"),
            options.collision,
        )?)
    };
    let mut bytes = 0u64;
    for (index, image) in images.iter_mut().enumerate() {
        cancel.check()?;
        if let Some(transaction) = &transaction {
            if image.candidate.skip_reason.is_none() {
                let extension =
                    if image.candidate.encoding_outcome == Some("preserved_encoded_bytes") {
                        "jpg"
                    } else {
                        "png"
                    };
                let name = format!(
                    "{:03}-object-{}-{}.{extension}",
                    index + 1,
                    image.candidate.object_number,
                    image.candidate.generation
                );
                let destination = transaction.path().join(&name);
                pdf_embedded_encode::encode(
                    sources[image.candidate.source_index].snapshot.path(),
                    &options.directory,
                    image,
                    &destination,
                    512 * 1024 * 1024 - bytes,
                    cancel,
                )?;
                let mut file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&destination)?;
                let (count, hash) = digest(&mut file, cancel, 512 * 1024 * 1024 - bytes)?;
                file.sync_all()?;
                bytes += count;
                image.candidate.artifact_name = Some(name);
                image.candidate.bytes = Some(count);
                image.candidate.sha256 = Some(hash);
            }
        }
    }
    for (source, input) in sources.iter_mut().zip(&options.inputs) {
        source.check(input)?;
    }
    let discovered = images.len();
    let skipped = discovered - supported;
    let mut warnings=vec!["Resource references may include unused images. Inline images, annotation appearances and painted layout are not enumerated."];
    if skipped > 0 {
        warnings.push("Unsupported images were skipped; review each candidate reason. This is a partial extraction.");
    }
    if discovered == 0 {
        warnings.push("No embedded images were found in the selected page resources.");
    } else if supported == 0 {
        warnings.push("None of the discovered image objects can be exported with the supported encoding policy.");
    }
    let mut receipt = ExtractionReceipt {
        output: if options.dry_run {
            None
        } else {
            options.output.clone()
        },
        candidates: images.into_iter().map(|v| v.candidate).collect(),
        discovered,
        supported,
        skipped,
        source_sha256: sources.iter().map(|v| v.hash.clone()).collect(),
        warnings,
    };
    if serde_json::to_vec(&receipt)?.len() > 900_000 {
        return Err(fail(
            "limit",
            "Embedded-image receipt exceeds the response limit. No folder was saved.",
        ));
    }
    if let Some(transaction) = transaction {
        receipt.output = Some(transaction.commit(cancel)?);
    }
    Ok(receipt)
}
