// SPDX-License-Identifier: Apache-2.0
use crate::{fail, native_process, pdf_render, pdf_text, Cancellation, Result};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq)]
pub(crate) struct PageProof {
    pixels: [u8; 32],
    text: [u8; 32],
    renderer_version: String,
}
/// Inputs are private immutable snapshots owned by the caller. No additional disk snapshots
/// or preview artifacts are made inside the per-page loop.
pub(crate) fn pages(
    input: &Path,
    directory: &Path,
    geometry: &[crate::pdf_pages::PageGeometry],
    cancel: &Cancellation,
) -> Result<Vec<PageProof>> {
    if !(1..=1000).contains(&geometry.len()) {
        return Err(fail(
            "limit",
            "PDF page count exceeds its verification limit.",
        ));
    }
    let deadline = Instant::now() + Duration::from_secs(600);
    let working = tempfile::tempdir()?;
    let mut proofs = Vec::with_capacity(geometry.len());
    for geometry in geometry {
        let page = geometry
            .position
            .checked_sub(1)
            .filter(|v| *v < 1000)
            .ok_or_else(|| fail("invalid_request", "Invalid PDF page position."))?;
        let (mut render, version) = pdf_render::command(directory, cancel)?;
        render
            .current_dir(working.path())
            .arg(input)
            .arg(page.to_string())
            .args(["2048", "media"])
            .args(geometry.media_box.iter().map(|v| v.to_string()))
            .arg((geometry.rotation / 90).to_string());
        let bytes =
            native_process::run(render, cancel, remaining(deadline)?, 2048 * 2048 * 3 + 64)?;
        pdf_render::raster(&bytes, 2048)?;
        let pixels = Sha256::digest(&bytes).into();
        drop(bytes);
        let (mut text, text_version) = pdf_render::command(directory, cancel)?;
        if text_version != version {
            return Err(fail(
                "engine_unavailable",
                "The PDF renderer changed during verification.",
            ));
        }
        text.current_dir(working.path())
            .arg(input)
            .arg(page.to_string())
            .arg("text");
        let bytes = native_process::run(text, cancel, remaining(deadline)?, 4_000_032)?;
        let text = Sha256::digest(pdf_text::decode(&bytes)?.as_bytes()).into();
        proofs.push(PageProof {
            pixels,
            text,
            renderer_version: version,
        });
    }
    cancel.check()?;
    Ok(proofs)
}
fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|v| !v.is_zero())
        .map(|v| v.min(Duration::from_secs(60)))
        .ok_or_else(|| {
            fail(
                "limit",
                "PDF page verification exceeded its ten-minute budget. No output was saved.",
            )
        })
}
