// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};

/// Transform unassociated color samples in small blocks. Alpha is never sent to
/// the CMS or recomputed, including the hidden RGB values of transparent pixels.
pub(crate) fn normalize_icc(
    pixels: &mut image::RgbaImage,
    icc: &[u8],
    source_gray: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    if icc.len() > 1024 * 1024 {
        return Err(fail("limit", "ICC profile exceeds 1 MiB."));
    }
    let profile = ColorProfile::new_from_slice(icc)
        .map_err(|error| fail("invalid_image", format!("Invalid ICC profile: {error}")))?;
    let layout = match (source_gray, profile.color_space) {
        (false, DataColorSpace::Rgb) => Layout::Rgb,
        (true, DataColorSpace::Gray) => Layout::Gray,
        _ => {
            return Err(fail(
                "invalid_image",
                "ICC color space does not match the PNG samples.",
            ))
        }
    };
    let destination = ColorProfile::new_srgb();
    let transform = profile
        .create_transform_8bit(
            layout,
            &destination,
            Layout::Rgb,
            TransformOptions::default(),
        )
        .map_err(|error| {
            fail(
                "unsupported",
                format!("ICC conversion is unsupported: {error}"),
            )
        })?;
    let mut input = Vec::new();
    let mut output = Vec::new();
    input
        .try_reserve_exact(4096 * 3)
        .map_err(|_| fail("limit", "Not enough memory for color conversion."))?;
    output
        .try_reserve_exact(4096 * 3)
        .map_err(|_| fail("limit", "Not enough memory for color conversion."))?;
    for block in pixels.as_mut().chunks_mut(4096 * 4) {
        cancellation.check()?;
        input.clear();
        for pixel in block.as_chunks::<4>().0 {
            if source_gray {
                input.push(pixel[0]);
            } else {
                input.extend_from_slice(&pixel[..3]);
            }
        }
        output.resize(block.len() / 4 * 3, 0);
        transform.transform(&input, &mut output).map_err(|error| {
            fail(
                "invalid_image",
                format!("ICC pixel transform failed: {error}"),
            )
        })?;
        for (pixel, rgb) in block
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(output.as_chunks::<3>().0)
        {
            pixel[..3].copy_from_slice(rgb);
        }
    }
    cancellation.check()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn srgb_roundtrip_keeps_alpha_and_pixel_values() {
        let original = [0, 0, 0, 0, 200, 100, 50, 127, 255, 255, 255, 255];
        let mut pixels = image::RgbaImage::from_raw(3, 1, original.to_vec()).unwrap();
        normalize_icc(
            &mut pixels,
            &ColorProfile::new_srgb().encode().unwrap(),
            false,
            &Cancellation::default(),
        )
        .unwrap();
        for (a, b) in pixels.as_raw().iter().zip(original) {
            assert!(a.abs_diff(b) <= 1);
        }
        assert_eq!(
            pixels.pixels().map(|p| p[3]).collect::<Vec<_>>(),
            [0, 127, 255]
        );
    }
    #[test]
    fn display_p3_changes_color_without_touching_alpha() {
        let mut pixels = image::RgbaImage::from_raw(1, 1, vec![200, 100, 50, 37]).unwrap();
        normalize_icc(
            &mut pixels,
            &ColorProfile::new_display_p3().encode().unwrap(),
            false,
            &Cancellation::default(),
        )
        .unwrap();
        let pixel = pixels.get_pixel(0, 0);
        // Independent D65 P3-to-sRGB matrix + sRGB transfer reference, rounded.
        for (actual, expected) in pixel.0[..3].iter().zip([215u8, 93, 31]) {
            assert!(actual.abs_diff(expected) <= 2, "{pixel:?}");
        }
        assert_eq!(pixel[3], 37);
    }
    #[test]
    fn gray_profile_expands_to_srgb_and_rejects_wrong_sample_space() {
        let profile = ColorProfile::new_gray_with_gamma(2.2).encode().unwrap();
        let mut pixels = image::RgbaImage::from_raw(1, 1, vec![128, 128, 128, 71]).unwrap();
        normalize_icc(&mut pixels, &profile, true, &Cancellation::default()).unwrap();
        let pixel = pixels.get_pixel(0, 0);
        assert!(pixel[0].abs_diff(129) <= 1);
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
        assert_eq!(pixel[3], 71);
        assert!(normalize_icc(&mut pixels, &profile, false, &Cancellation::default()).is_err());
        assert!(normalize_icc(
            &mut pixels,
            &ColorProfile::new_srgb().encode().unwrap(),
            true,
            &Cancellation::default()
        )
        .is_err());
    }
    #[test]
    fn invalid_profiles_and_cancelled_transforms_fail() {
        let mut pixels = image::RgbaImage::new(1, 1);
        assert!(normalize_icc(&mut pixels, b"invalid", false, &Cancellation::default()).is_err());
        let signal = Cancellation::default();
        signal.cancel();
        assert_eq!(
            normalize_icc(&mut pixels, &[], false, &signal)
                .unwrap_err()
                .code,
            "cancelled"
        );
    }
}
