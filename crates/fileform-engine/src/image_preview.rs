// SPDX-License-Identifier: Apache-2.0
use crate::{Cancellation, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ImagePreview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Area-average a small display preview with alpha-weighted color. The preview
/// is never an export source. Its pixel payload is at most 64 KiB before JSON.
pub(crate) fn make(image: &image::RgbaImage, cancellation: &Cancellation) -> Result<ImagePreview> {
    cancellation.check()?;
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    let (out_width, out_height) = if longest <= 128 {
        (width, height)
    } else {
        (
            (u64::from(width) * 128 / u64::from(longest)).max(1) as u32,
            (u64::from(height) * 128 / u64::from(longest)).max(1) as u32,
        )
    };
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(out_width as usize * out_height as usize * 4)
        .map_err(|_| crate::fail("limit", "Not enough memory for the image preview."))?;
    for y in 0..out_height {
        cancellation.check()?;
        let top = u64::from(y) * u64::from(height) / u64::from(out_height);
        let bottom = u64::from(y + 1) * u64::from(height) / u64::from(out_height);
        for x in 0..out_width {
            let left = u64::from(x) * u64::from(width) / u64::from(out_width);
            let right = u64::from(x + 1) * u64::from(width) / u64::from(out_width);
            let mut alpha = 0u64;
            let mut color = [0u64; 3];
            for sy in top..bottom {
                for sx in left..right {
                    if sx % 4096 == 0 {
                        cancellation.check()?;
                    }
                    let pixel = image.get_pixel(sx as u32, sy as u32);
                    alpha += u64::from(pixel[3]);
                    for channel in 0..3 {
                        color[channel] += u64::from(pixel[channel]) * u64::from(pixel[3]);
                    }
                }
            }
            for value in color {
                rgba.push((value + alpha / 2).checked_div(alpha).unwrap_or(0) as u8);
            }
            let count = (bottom - top) * (right - left);
            rgba.push(((alpha + count / 2) / count) as u8);
        }
    }
    Ok(ImagePreview {
        width: out_width,
        height: out_height,
        rgba,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnail_is_bounded_and_does_not_mix_transparent_color() {
        let mut image = image::RgbaImage::new(256, 2);
        for y in 0..2 {
            for x in 0..256 {
                image.put_pixel(
                    x,
                    y,
                    if x % 2 == 0 {
                        image::Rgba([255, 0, 0, 255])
                    } else {
                        image::Rgba([0, 0, 255, 0])
                    },
                );
            }
        }
        let result = make(&image, &Cancellation::default()).unwrap();
        assert_eq!((result.width, result.height), (128, 1));
        assert!(result
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [255, 0, 0, 128]));
        assert_eq!(image.get_pixel(1, 0).0, [0, 0, 255, 0]);
    }
    #[test]
    fn tiny_images_are_not_upscaled_and_cancel_is_honored() {
        let image = image::RgbaImage::from_raw(1, 1, vec![12, 34, 56, 127]).unwrap();
        let result = make(&image, &Cancellation::default()).unwrap();
        assert_eq!((result.width, result.height), (1, 1));
        assert_eq!(result.rgba, [12, 34, 56, 127]);
        let signal = Cancellation::default();
        signal.cancel();
        assert!(make(&image, &signal).is_err());
    }
}
