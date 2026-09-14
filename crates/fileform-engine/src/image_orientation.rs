// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};

/// Read only the TIFF IFD0 orientation field; do not silently default malformed
/// EXIF to orientation 1. PNG eXIf payloads start with the TIFF header.
pub(crate) fn parse_exif(data: &[u8]) -> Result<u8> {
    if data.len() > 1024 * 1024 {
        return Err(fail("limit", "EXIF metadata exceeds 1 MiB."));
    }
    let little = match data.get(..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return Err(invalid()),
    };
    let short = |offset: usize| -> Result<u16> {
        let bytes: [u8; 2] = data
            .get(offset..offset.saturating_add(2))
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        Ok(if little {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    };
    let long = |offset: usize| -> Result<u32> {
        let bytes: [u8; 4] = data
            .get(offset..offset.saturating_add(4))
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        Ok(if little {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    };
    if short(2)? != 42 {
        return Err(invalid());
    }
    let offset = long(4)? as usize;
    if offset < 8 {
        return Err(invalid());
    }
    let entries = usize::from(short(offset)?);
    if entries > 4096 {
        return Err(fail("limit", "EXIF directory exceeds 4096 entries."));
    }
    let end = offset
        .checked_add(2)
        .and_then(|n| n.checked_add(entries * 12))
        .and_then(|n| n.checked_add(4))
        .ok_or_else(invalid)?;
    if end > data.len() {
        return Err(invalid());
    }
    let mut orientation = None;
    for i in 0..entries {
        let entry = offset + 2 + i * 12;
        if short(entry)? == 0x0112 {
            if orientation.is_some() || short(entry + 2)? != 3 || long(entry + 4)? != 1 {
                return Err(invalid());
            }
            let value = short(entry + 8)?;
            if !(1..=8).contains(&value) {
                return Err(invalid());
            }
            orientation = Some(value as u8);
        }
    }
    Ok(orientation.unwrap_or(1))
}
fn invalid() -> crate::Failure {
    fail("invalid_image", "EXIF orientation metadata is malformed.")
}

pub(crate) fn apply(
    pixels: image::RgbaImage,
    orientation: u8,
    cancellation: &Cancellation,
) -> Result<image::RgbaImage> {
    cancellation.check()?;
    if !(1..=8).contains(&orientation) {
        return Err(invalid());
    }
    if orientation == 1 {
        return Ok(pixels);
    }
    let (width, height) = pixels.dimensions();
    let (out_width, out_height) = if orientation >= 5 {
        (height, width)
    } else {
        (width, height)
    };
    let bytes = pixels.as_raw().len();
    let mut output = Vec::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| fail("limit", "Not enough memory to orient the image."))?;
    output.resize(bytes, 0);
    for y in 0..height {
        for x in 0..width {
            if x % 4096 == 0 {
                cancellation.check()?;
            }
            let (dx, dy) = match orientation {
                2 => (width - 1 - x, y),
                3 => (width - 1 - x, height - 1 - y),
                4 => (x, height - 1 - y),
                5 => (y, x),
                6 => (height - 1 - y, x),
                7 => (height - 1 - y, width - 1 - x),
                8 => (y, width - 1 - x),
                _ => unreachable!(),
            };
            let from = (u64::from(y) * u64::from(width) + u64::from(x)) as usize * 4;
            let to = (u64::from(dy) * u64::from(out_width) + u64::from(dx)) as usize * 4;
            output[to..to + 4].copy_from_slice(&pixels.as_raw()[from..from + 4]);
        }
    }
    cancellation.check()?;
    image::RgbaImage::from_raw(out_width, out_height, output).ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn exif(orientation: u16, little: bool) -> Vec<u8> {
        if little {
            let mut bytes = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
            bytes.extend_from_slice(&orientation.to_le_bytes());
            bytes.extend_from_slice(&[0; 6]);
            bytes
        } else {
            let mut bytes = b"MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01".to_vec();
            bytes.extend_from_slice(&orientation.to_be_bytes());
            bytes.extend_from_slice(&[0; 6]);
            bytes
        }
    }
    #[test]
    fn all_eight_orientations_match_known_pixel_layouts() {
        let expected = [
            "ABCDEF", "BADCFE", "FEDCBA", "EFCDAB", "ACEBDF", "ECAFDB", "FDBECA", "BDFACE",
        ];
        for (index, layout) in expected.iter().enumerate() {
            let pixels = image::RgbaImage::from_raw(
                2,
                3,
                b"ABCDEF".iter().flat_map(|n| [*n, 0, 0, *n]).collect(),
            )
            .unwrap();
            let output = apply(pixels, index as u8 + 1, &Cancellation::default()).unwrap();
            assert_eq!(output.dimensions(), if index < 4 { (2, 3) } else { (3, 2) });
            assert_eq!(
                output.pixels().map(|p| p[0]).collect::<Vec<_>>(),
                layout.as_bytes()
            );
            assert!(output.pixels().all(|p| p[0] == p[3]));
            for endian in [false, true] {
                assert_eq!(
                    parse_exif(&exif(index as u16 + 1, endian)).unwrap(),
                    index as u8 + 1
                );
            }
        }
    }
    #[test]
    fn invalid_and_truncated_orientation_tags_fail() {
        for value in [0, 9, u16::MAX] {
            assert!(parse_exif(&exif(value, true)).is_err());
        }
        let valid = exif(1, true);
        for length in 0..valid.len() {
            assert!(parse_exif(&valid[..length]).is_err());
        }
        let mut wrong_type = valid.clone();
        wrong_type[12] = 4;
        assert!(parse_exif(&wrong_type).is_err());
        let mut wrong_count = valid.clone();
        wrong_count[14] = 2;
        assert!(parse_exif(&wrong_count).is_err());
        let mut duplicate = valid[..22].to_vec();
        duplicate[8..10].copy_from_slice(&2u16.to_le_bytes());
        duplicate.extend_from_slice(&valid[10..22]);
        duplicate.extend_from_slice(&[0; 4]);
        assert!(parse_exif(&duplicate).is_err());
        assert_eq!(parse_exif(b"II\x2a\0\x08\0\0\0\0\0\0\0\0\0").unwrap(), 1);
        let mut bad_offset = valid;
        bad_offset[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_exif(&bad_offset).is_err());
    }
    #[test]
    fn orientation_stops_when_cancelled() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let error = apply(image::RgbaImage::new(2, 3), 6, &cancellation).unwrap_err();
        assert_eq!(error.code, "cancelled");
    }
}
