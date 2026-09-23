// SPDX-License-Identifier: Apache-2.0
use crate::{
    directory_transaction::DirectoryTransaction,
    fail,
    pdf_compose::{self, Composition, PdfPageSelection},
    Cancellation, Result,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Split {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    pub groups: Option<Vec<Vec<PdfPageSelection>>>,
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
    pub output: PathBuf,
    pub parts: Vec<SplitPart>,
    pub source_sha256: Vec<String>,
    pub warnings: Vec<&'static str>,
}
pub fn split(options: &Split, cancel: &Cancellation) -> Result<SplitReceipt> {
    if !options.allow_document_changes {
        return Err(fail("unsupported", "PDF splitting creates new documents; metadata, bookmarks, form behavior and signature certification may change."));
    }
    if let Some(groups) = &options.groups {
        validate(groups)?;
    }
    let transaction = DirectoryTransaction::new(&options.output)?;
    let mut prepared = pdf_compose::prepare(&options.inputs, &options.directory, cancel)?;
    let all;
    let groups = match &options.groups {
        Some(groups) => groups,
        None => {
            all = pdf_compose::individual_pages(&prepared);
            &all
        }
    };
    validate(groups)?;
    let mut parts = Vec::with_capacity(groups.len());
    let mut total = 0u64;
    let mut source_sha256 = Vec::new();
    let mut warnings = Vec::new();
    for (index, pages) in groups.iter().enumerate() {
        cancel.check()?;
        let name = format!("part-{:04}.pdf", index + 1);
        let group = Composition {
            inputs: options.inputs.clone(),
            output: transaction.path().join(&name),
            directory: options.directory.clone(),
            renderer_directory: options.renderer_directory.clone(),
            pages: Some(pages.clone()),
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
    let receipt = SplitReceipt {
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
    transaction.commit(cancel)?;
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
