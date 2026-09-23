// SPDX-License-Identifier: Apache-2.0
use crate::{fail, pdf_pages, Cancellation, Result};
use serde::Serialize;
use std::path::Path;
#[derive(Debug, Serialize)]
pub struct RasterPagePlan {
    pub page_index: u32,
    pub crop_box: [f64; 4],
    pub rotation: u32,
    pub user_unit: f64,
    pub width: u32,
    pub height: u32,
    pub decoded_rgb_bytes: u64,
}
#[derive(Debug, Serialize)]
pub struct RasterPlan {
    pub source_sha256: String,
    pub dpi: u16,
    pub pages: Vec<RasterPagePlan>,
}
fn dimensions(page: &pdf_pages::PageGeometry, dpi: u16, rotation: i32) -> Result<RasterPagePlan> {
    if !(36..=600).contains(&dpi) || rotation % 90 != 0 {
        return Err(fail(
            "invalid_request",
            "Choose 36–600 DPI and a right-angle rotation.",
        ));
    }
    let rotation = (page.rotation + rotation.rem_euclid(360) as u32) % 360;
    let box_ = page.effective_crop_box;
    let scale = page.user_unit * f64::from(dpi) / 72.0;
    let (width, height) = if rotation == 90 || rotation == 270 {
        ((box_[3] - box_[1]) * scale, (box_[2] - box_[0]) * scale)
    } else {
        ((box_[2] - box_[0]) * scale, (box_[3] - box_[1]) * scale)
    };
    let (width, height) = (width.ceil(), height.ceil());
    if !width.is_finite()
        || !height.is_finite()
        || !(1.0..=16384.0).contains(&width)
        || !(1.0..=16384.0).contains(&height)
        || width * height > 64_000_000.0
    {
        return Err(fail("limit", "Requested PDF resolution exceeds 16384 pixels per side or 64 million pixels. Choose a lower DPI."));
    }
    Ok(RasterPagePlan {
        page_index: page
            .position
            .checked_sub(1)
            .ok_or_else(|| fail("verification", "Invalid PDF page position."))?,
        crop_box: box_,
        rotation,
        user_unit: page.user_unit,
        width: width as u32,
        height: height as u32,
        decoded_rgb_bytes: width as u64 * height as u64 * 3,
    })
}
pub fn plan(input: &Path, directory: &Path, dpi: u16, cancel: &Cancellation) -> Result<RasterPlan> {
    if !(36..=600).contains(&dpi) {
        return Err(fail(
            "invalid_request",
            "Choose a resolution from 36 to 600 DPI.",
        ));
    }
    let inspection = pdf_pages::inspect(input, directory, cancel)?;
    if inspection
        .special_preservation_keys
        .iter()
        .any(|v| matches!(v.as_str(), "/Encrypt" | "/XFA"))
    {
        return Err(fail(
            "unsupported",
            "PDF page export requires an unencrypted document without XFA forms.",
        ));
    }
    let mut pages = Vec::with_capacity(inspection.pages.len());
    for page in &inspection.pages {
        cancel.check()?;
        pages.push(dimensions(page, dpi, 0)?);
    }
    let result = RasterPlan {
        source_sha256: inspection.sha256,
        dpi,
        pages,
    };
    if serde_json::to_vec(&result)?.len() > 900_000 {
        return Err(fail("limit", "PDF raster plan exceeds the response limit."));
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn page() -> pdf_pages::PageGeometry {
        pdf_pages::PageGeometry {
            position: 1,
            media_box: [-10., -20., 200., 300.],
            crop_box: [0., 0., 180.25, 270.25],
            effective_crop_box: [0., 0., 180.25, 270.25],
            bleed_box: [0., 0., 180.25, 270.25],
            trim_box: [0., 0., 180.25, 270.25],
            art_box: [0., 0., 180.25, 270.25],
            rotation: 0,
            user_unit: 1.,
        }
    }
    #[test]
    fn dpi_rounding_units_and_rotations_are_explicit() {
        let mut page = page();
        let result = dimensions(&page, 144, 0).unwrap();
        assert_eq!((result.width, result.height), (361, 541));
        page.user_unit = 2.;
        let result = dimensions(&page, 72, 90).unwrap();
        assert_eq!(
            (result.width, result.height, result.rotation),
            (541, 361, 90)
        );
        page.rotation = 270;
        let result = dimensions(&page, 72, 90).unwrap();
        assert_eq!(
            (result.width, result.height, result.rotation),
            (361, 541, 0)
        );
    }
    #[test]
    fn excessive_dpi_or_pixel_counts_are_rejected_not_reduced() {
        let mut page = page();
        assert!(dimensions(&page, 601, 0).is_err());
        assert!(dimensions(&page, 72, 45).is_err());
        page.effective_crop_box = [0., 0., 10000., 10000.];
        assert!(dimensions(&page, 72, 0).is_err());
        page.effective_crop_box = [0., 0., 16385., 1.];
        assert!(dimensions(&page, 72, 0).is_err());
    }
}
