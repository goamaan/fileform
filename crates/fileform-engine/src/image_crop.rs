// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
pub(crate) fn apply(
    image: image::RgbaImage,
    crop: PixelCrop,
    cancellation: &Cancellation,
) -> Result<image::RgbaImage> {
    cancellation.check()?;
    if crop.width == 0
        || crop.height == 0
        || crop
            .x
            .checked_add(crop.width)
            .is_none_or(|end| end > image.width())
        || crop
            .y
            .checked_add(crop.height)
            .is_none_or(|end| end > image.height())
    {
        return Err(fail(
            "invalid_request",
            "The crop must be a non-empty region inside the oriented image.",
        ));
    }
    if crop.x == 0 && crop.y == 0 && crop.width == image.width() && crop.height == image.height() {
        return Ok(image);
    }
    let bytes = usize::try_from(u64::from(crop.width) * u64::from(crop.height) * 4)
        .map_err(|_| fail("limit", "Crop buffer size overflow."))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| fail("limit", "Not enough memory for the cropped image."))?;
    let stride = image.width() as usize * 4;
    let row_bytes = crop.width as usize * 4;
    for y in crop.y..crop.y + crop.height {
        cancellation.check()?;
        let start = y as usize * stride + crop.x as usize * 4;
        output.extend_from_slice(&image.as_raw()[start..start + row_bytes]);
    }
    image::RgbaImage::from_raw(crop.width, crop.height, output)
        .ok_or_else(|| fail("invalid_image", "Crop buffer is incomplete."))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crop_preserves_exact_selected_pixels_and_alpha() {
        let image = image::RgbaImage::from_raw(3, 2, (0u8..24).collect()).unwrap();
        let cropped = apply(
            image,
            PixelCrop {
                x: 1,
                y: 0,
                width: 2,
                height: 2,
            },
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(cropped.dimensions(), (2, 2));
        assert_eq!(
            cropped.as_raw(),
            &[4, 5, 6, 7, 8, 9, 10, 11, 16, 17, 18, 19, 20, 21, 22, 23]
        );
    }
    #[test]
    fn invalid_overflowing_and_cancelled_crops_fail() {
        for crop in [
            PixelCrop {
                x: 0,
                y: 0,
                width: 0,
                height: 1,
            },
            PixelCrop {
                x: 2,
                y: 0,
                width: 1,
                height: 1,
            },
            PixelCrop {
                x: u32::MAX,
                y: 0,
                width: 2,
                height: 1,
            },
            PixelCrop {
                x: 0,
                y: 1,
                width: 1,
                height: 2,
            },
        ] {
            assert!(apply(image::RgbaImage::new(2, 2), crop, &Cancellation::default()).is_err());
        }
        let signal = Cancellation::default();
        signal.cancel();
        assert_eq!(
            apply(
                image::RgbaImage::new(2, 2),
                PixelCrop {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1
                },
                &signal
            )
            .unwrap_err()
            .code,
            "cancelled"
        );
    }
}
