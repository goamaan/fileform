// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};

/// Downsample using exact pixel-area overlap in linear sRGB with premultiplied
/// alpha. No full-size float intermediate and no hidden-RGB color bleeding.
pub(crate) fn limit(
    image: image::RgbaImage,
    maximum: u32,
    cancellation: &Cancellation,
) -> Result<image::RgbaImage> {
    cancellation.check()?;
    let (width, height) = image.dimensions();
    if maximum == 0 || width == 0 || height == 0 {
        return Err(fail(
            "invalid_request",
            "Maximum image dimension must be positive.",
        ));
    }
    let longest = width.max(height);
    if maximum >= longest {
        return Ok(image);
    }
    let scaled = |value: u32| {
        ((u64::from(value) * u64::from(maximum) + u64::from(longest) / 2) / u64::from(longest))
            .max(1) as u32
    };
    let (out_width, out_height) = (scaled(width), scaled(height));
    let bytes = u64::from(out_width)
        .checked_mul(u64::from(out_height))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| fail("limit", "Resize dimensions overflow."))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| fail("limit", "Not enough memory for the resized image."))?;
    let linear: [f64; 256] = std::array::from_fn(|index| {
        let value = index as f64 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    let step_x = f64::from(width) / f64::from(out_width);
    let step_y = f64::from(height) / f64::from(out_height);
    for y in 0..out_height {
        cancellation.check()?;
        let top = f64::from(y) * step_y;
        let bottom = f64::from(y + 1) * step_y;
        for x in 0..out_width {
            let left = f64::from(x) * step_x;
            let right = f64::from(x + 1) * step_x;
            let mut alpha = 0.0;
            let mut colors = [0.0; 3];
            for sy in top.floor() as u32..(bottom.ceil() as u32).min(height) {
                let vertical = bottom.min(f64::from(sy) + 1.0) - top.max(f64::from(sy));
                for sx in left.floor() as u32..(right.ceil() as u32).min(width) {
                    if sx % 4096 == 0 {
                        cancellation.check()?;
                    }
                    let weight =
                        vertical * (right.min(f64::from(sx) + 1.0) - left.max(f64::from(sx)));
                    let pixel = image.get_pixel(sx, sy);
                    let weighted_alpha = f64::from(pixel[3]) / 255.0 * weight;
                    alpha += weighted_alpha;
                    for channel in 0..3 {
                        colors[channel] += linear[usize::from(pixel[channel])] * weighted_alpha;
                    }
                }
            }
            for color in colors {
                let value = if alpha > 0.0 {
                    (color / alpha).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let encoded = if value <= 0.0031308 {
                    12.92 * value
                } else {
                    1.055 * value.powf(1.0 / 2.4) - 0.055
                };
                output.push((encoded * 255.0).round().clamp(0.0, 255.0) as u8);
            }
            output.push(
                (alpha / (step_x * step_y) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8,
            );
        }
    }
    image::RgbaImage::from_raw(out_width, out_height, output)
        .ok_or_else(|| fail("invalid_image", "Resized pixel buffer is incomplete."))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linear_light_average_and_transparency_are_correct() {
        let image =
            image::RgbaImage::from_raw(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255]).unwrap();
        assert_eq!(
            limit(image, 1, &Cancellation::default()).unwrap().as_raw(),
            &[188, 188, 188, 255]
        );
        let image = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]).unwrap();
        assert_eq!(
            limit(image, 1, &Cancellation::default()).unwrap().as_raw(),
            &[255, 0, 0, 128]
        );
    }
    #[test]
    fn fractional_area_weights_preserve_edges() {
        let image =
            image::RgbaImage::from_raw(3, 1, vec![0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255])
                .unwrap();
        assert_eq!(
            limit(image, 2, &Cancellation::default()).unwrap().as_raw(),
            &[156, 156, 156, 255, 156, 156, 156, 255]
        );
    }
    #[test]
    fn aspect_ratio_no_enlargement_and_limits_are_respected() {
        let image = image::RgbaImage::from_pixel(6, 4, image::Rgba([20, 40, 60, 80]));
        assert_eq!(
            limit(image.clone(), 3, &Cancellation::default())
                .unwrap()
                .dimensions(),
            (3, 2)
        );
        assert_eq!(
            limit(image.clone(), 100, &Cancellation::default()).unwrap(),
            image
        );
        assert!(limit(image.clone(), 0, &Cancellation::default()).is_err());
        let signal = Cancellation::default();
        signal.cancel();
        assert!(limit(image, 3, &signal).is_err());
    }
}
