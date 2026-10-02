// SPDX-License-Identifier: Apache-2.0
use crate::{
    directory_transaction::DirectoryTransaction,
    fail, pdf_compose,
    pdf_raster_export::{self, Format, PageExport, PreparedRaster},
    pdf_raster_plan, Cancellation, PdfPageSelection, Result,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterBatch {
    #[serde(default)]
    pub collision: crate::OutputCollision,
    pub inputs: Vec<PathBuf>,
    pub expected_source_sha256: Option<Vec<String>>,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    pub pages: Option<Vec<PdfPageSelection>>,
    pub page_ranges: Option<String>,
    pub dpi: u16,
    pub format: Format,
    pub quality: Option<u8>,
    pub allow_rasterization: bool,
}
#[derive(Debug, Serialize)]
pub struct RasterPart {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub source_index: usize,
    pub page_index: u32,
    pub clockwise_rotation: i32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Serialize)]
pub struct BatchReceipt {
    pub output: PathBuf,
    pub parts: Vec<RasterPart>,
    pub source_sha256: Vec<String>,
    pub dpi: u16,
    pub format: Format,
    pub quality: Option<u8>,
    pub warnings: Vec<&'static str>,
}
pub fn export(options: &RasterBatch, cancel: &Cancellation) -> Result<BatchReceipt> {
    if options.pages.is_some() && options.page_ranges.is_some() {
        return Err(fail(
            "invalid_request",
            "Choose explicit pages or page ranges, not both.",
        ));
    }
    if !options.allow_rasterization {
        return Err(fail("unsupported", "Page image export rasterizes text and vectors and does not retain document metadata or interactive behavior."));
    }
    if !(36..=600).contains(&options.dpi)
        || match options.format {
            Format::Png => options.quality.is_some(),
            Format::Jpeg => options.quality.is_some_and(|v| !(5..=100).contains(&v)),
        }
    {
        return Err(fail(
            "invalid_request",
            "Choose 36–600 DPI and JPEG quality 5–100; PNG does not use quality.",
        ));
    }
    if options
        .pages
        .as_ref()
        .is_some_and(|v| v.is_empty() || v.len() > 1000)
    {
        return Err(fail("limit", "Choose 1–1000 page images."));
    }
    let transaction = DirectoryTransaction::with_collision(&options.output, options.collision)?;
    let mut prepared = pdf_compose::prepare(&options.inputs, &options.directory, cancel)?;
    crate::source_binding::multiple(&prepared.sources, options.expected_source_sha256.as_deref())?;
    let ranged = options
        .page_ranges
        .as_ref()
        .map(|v| {
            crate::pdf_selection::pages(
                v,
                &prepared.geometries.iter().map(Vec::len).collect::<Vec<_>>(),
            )
        })
        .transpose()?;
    let all;
    let selected = match options.pages.as_ref().or(ranged.as_ref()) {
        Some(pages) => pages,
        None => {
            all = pdf_compose::individual_pages(&prepared)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            &all
        }
    };
    if selected.is_empty() || selected.len() > 1000 {
        return Err(fail("limit", "Choose 1–1000 page images."));
    }
    // Validate every selected page before expensive rendering or partial staging.
    let mut geometry = Vec::with_capacity(selected.len());
    for page in selected {
        let original = prepared
            .geometries
            .get(page.source_index)
            .and_then(|v| v.get(page.page_index as usize))
            .ok_or_else(|| {
                fail(
                    "invalid_request",
                    "A selected page is outside its source document.",
                )
            })?;
        if prepared.special[page.source_index]
            .iter()
            .any(|v| matches!(v.as_str(), "/Encrypt" | "/XFA"))
        {
            return Err(fail(
                "unsupported",
                "Page images require unencrypted sources without XFA forms.",
            ));
        }
        let plan = pdf_raster_plan::dimensions(original, options.dpi, page.clockwise_rotation)?;
        let mut rotated = original.clone();
        rotated.rotation = plan.rotation;
        geometry.push(rotated);
    }
    let extension = match options.format {
        Format::Png => "png",
        Format::Jpeg => "jpg",
    };
    let mut parts = Vec::with_capacity(selected.len());
    let mut total = 0u64;
    let mut warnings = Vec::new();
    for (index, (page, geometry)) in selected.iter().zip(&geometry).enumerate() {
        cancel.check()?;
        let name = format!("{:03}.{extension}", index + 1);
        let request = PageExport {
            input: prepared.documents[page.source_index].clone(),
            output: transaction.path().join(&name),
            directory: options.directory.clone(),
            renderer_directory: options.renderer_directory.clone(),
            page_index: page.page_index,
            dpi: options.dpi,
            quality: options.quality,
        };
        let source = PreparedRaster {
            document: &prepared.documents[page.source_index],
            geometry,
            source_sha256: &prepared.sources[page.source_index].hash,
        };
        let candidate = pdf_raster_export::render_prepared(
            &request,
            options.format,
            source,
            512 * 1024 * 1024 - total,
            cancel,
        )?;
        total += candidate.receipt.bytes;
        let receipt = candidate.publish(cancel)?;
        if index == 0 {
            warnings = receipt.warnings;
        }
        parts.push(RasterPart {
            name,
            bytes: receipt.bytes,
            sha256: receipt.sha256,
            source_index: page.source_index,
            page_index: page.page_index,
            clockwise_rotation: page.clockwise_rotation,
            width: receipt.width,
            height: receipt.height,
        });
    }
    pdf_compose::check_sources(&mut prepared, &options.inputs)?;
    let mut receipt = BatchReceipt {
        output: options.output.clone(),
        parts,
        source_sha256: prepared.sources.iter().map(|v| v.hash.clone()).collect(),
        dpi: options.dpi,
        format: options.format,
        quality: match options.format {
            Format::Png => None,
            Format::Jpeg => Some(options.quality.unwrap_or(85)),
        },
        warnings,
    };
    if serde_json::to_vec(&receipt)?.len() > 900_000 {
        return Err(fail(
            "limit",
            "Page-image receipt exceeds the response limit. No folder was saved.",
        ));
    }
    receipt.output = transaction.commit(cancel)?;
    Ok(receipt)
}
