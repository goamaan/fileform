// SPDX-License-Identifier: Apache-2.0
use crate::{
    directory_transaction::DirectoryTransaction,
    fail,
    pdf_compose::{self, Composition, PdfPageSelection},
    pdf_selection::{self, SplitSelection},
    Cancellation, Result,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Split {
    #[serde(default)]
    pub collision: crate::OutputCollision,
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    pub groups: Option<Vec<Vec<PdfPageSelection>>>,
    pub selection: Option<SplitSelection>,
    #[serde(default)]
    pub dry_run: bool,
    pub allow_document_changes: bool,
}
#[derive(Debug, Serialize)]
pub struct SplitPart {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub pages: usize,
}
#[derive(Debug, Serialize)]
pub struct SplitReceipt {
    pub status: &'static str,
    pub groups: Vec<Vec<PdfPageSelection>>,
    pub output: PathBuf,
    pub parts: Vec<SplitPart>,
    pub source_sha256: Vec<String>,
    pub warnings: Vec<&'static str>,
}
pub fn split(options: &Split, cancel: &Cancellation) -> Result<SplitReceipt> {
    if !options.allow_document_changes {
        return Err(fail("unsupported", "PDF splitting creates new documents; metadata, bookmarks, form behavior and signature certification may change."));
    }
    if options.groups.is_some() && options.selection.is_some() {
        return Err(fail(
            "invalid_request",
            "Choose explicit groups or a split selection, not both.",
        ));
    }
    if let Some(groups) = &options.groups {
        validate(groups)?;
    }
    let transaction = if options.dry_run {
        None
    } else {
        Some(DirectoryTransaction::with_collision(
            &options.output,
            options.collision,
        )?)
    };
    let mut prepared = pdf_compose::prepare(&options.inputs, &options.directory, cancel)?;
    let all;
    let groups = match &options.groups {
        Some(groups) => groups,
        None => {
            all = match &options.selection {
                Some(selection) => pdf_selection::select(
                    selection,
                    &pdf_compose::individual_pages(&prepared)
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>(),
                )?,
                None => pdf_compose::individual_pages(&prepared),
            };
            &all
        }
    };
    validate(groups)?;
    for page in groups.iter().flatten() {
        if page.clockwise_rotation % 90 != 0
            || prepared
                .geometries
                .get(page.source_index)
                .is_none_or(|v| page.page_index as usize >= v.len())
        {
            return Err(fail(
                "invalid_request",
                "Choose existing pages and rotations in multiples of 90 degrees.",
            ));
        }
    }
    if options.dry_run {
        pdf_compose::check_sources(&mut prepared, &options.inputs)?;
        return Ok(SplitReceipt { status: "planned", output: options.output.clone(), groups: groups.clone(), parts: Vec::new(), source_sha256: prepared.sources.iter().map(|v| v.hash.clone()).collect(), warnings: vec!["Splitting creates new documents; document-level metadata and interactive behavior may change."] });
    }
    let transaction = transaction.expect("execution transaction");
    let mut parts = Vec::with_capacity(groups.len());
    let mut total = 0u64;
    let mut source_sha256 = Vec::new();
    let mut warnings = Vec::new();
    for (index, pages) in groups.iter().enumerate() {
        cancel.check()?;
        let name = format!("part-{:04}.pdf", index + 1);
        let group = Composition {
            collision: crate::OutputCollision::Fail,
            inputs: options.inputs.clone(),
            output: transaction.path().join(&name),
            directory: options.directory.clone(),
            renderer_directory: options.renderer_directory.clone(),
            pages: Some(pages.clone()),
            page_ranges: None,
            allow_document_changes: true,
        };
        let part = pdf_compose::compose_prepared(
            &group,
            &mut prepared,
            cancel,
            512 * 1024 * 1024 - total,
        )?;
        total += part.bytes;
        if total > 512 * 1024 * 1024 {
            return Err(fail(
                "limit",
                "Combined split outputs exceed 512 MiB. No folder was saved.",
            ));
        }
        if index == 0 {
            source_sha256 = part.source_sha256;
            warnings = part.warnings;
        }
        parts.push(SplitPart {
            name,
            bytes: part.bytes,
            sha256: part.sha256,
            pages: part.pages,
        });
    }
    // Each group checks sources; repeat immediately before publishing the set.
    pdf_compose::check_sources(&mut prepared, &options.inputs)?;
    let mut receipt = SplitReceipt {
        status: "saved",
        groups: groups.clone(),
        output: options.output.clone(),
        parts,
        source_sha256,
        warnings,
    };
    if serde_json::to_vec(&receipt)?.len() > 900_000 {
        return Err(fail(
            "limit",
            "Split receipt exceeds the response limit. No folder was saved.",
        ));
    }
    receipt.output = transaction.commit(cancel)?;
    Ok(receipt)
}
fn validate(groups: &[Vec<PdfPageSelection>]) -> Result<()> {
    if groups.is_empty()
        || groups.len() > 1000
        || groups.iter().any(Vec::is_empty)
        || groups
            .iter()
            .try_fold(0usize, |n, g| n.checked_add(g.len()))
            .is_none_or(|n| n > 1000)
    {
        return Err(fail(
            "limit",
            "Choose nonempty PDF groups totaling at most 1000 pages.",
        ));
    }
    Ok(())
}
