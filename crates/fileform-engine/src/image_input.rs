// SPDX-License-Identifier: Apache-2.0
pub(crate) struct DecodedImage {
    pub preservation_pending: bool,
    pub pixels: image::RgbaImage,
    pub orientation: u8,
    pub has_alpha: bool,
    pub has_icc: bool,
    pub icc_profile: Option<Vec<u8>>,
    pub source_gray: bool,
    pub srgb: bool,
    pub gamma: Option<f32>,
    pub chromaticities: Option<[f64; 8]>,
    pub has_cicp: bool,
    pub has_exif: bool,
    pub has_color_metadata: bool,
    pub has_hdr_metadata: bool,
}
