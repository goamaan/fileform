// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Result};
pub(crate) fn qualities(
    quality: u8,
    minimum: Option<u8>,
    maximum_bytes: Option<u64>,
    jpeg: bool,
) -> Result<Vec<u8>> {
    if maximum_bytes.is_some_and(|n| n == 0 || n > 512 * 1024 * 1024) {
        return Err(fail(
            "invalid_request",
            "Image byte limit must be between 1 byte and 512 MiB.",
        ));
    }
    if minimum.is_some() && (!jpeg || maximum_bytes.is_none()) {
        return Err(fail(
            "invalid_request",
            "Minimum quality requires JPEG output and a byte limit.",
        ));
    }
    let floor = minimum.unwrap_or(35.min(quality));
    if floor == 0 || floor > quality {
        return Err(fail(
            "invalid_request",
            "Minimum quality must be positive and no higher than the chosen quality.",
        ));
    }
    if !jpeg || maximum_bytes.is_none() {
        return Ok(vec![quality]);
    }
    let mut values: Vec<u8> = (0..=10u16)
        .map(|step| quality - ((u16::from(quality - floor) * step + 5) / 10) as u8)
        .collect();
    values.dedup();
    Ok(values)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_is_descending_bounded_and_includes_the_floor() {
        assert_eq!(
            qualities(85, Some(35), Some(1000), true).unwrap(),
            [85, 80, 75, 70, 65, 60, 55, 50, 45, 40, 35]
        );
        assert_eq!(qualities(40, Some(40), Some(1000), true).unwrap(), [40]);
        for quality in 1..=100 {
            let values = qualities(quality, None, Some(1000), true).unwrap();
            assert!(values.len() <= 11);
            assert_eq!(values[0], quality);
            assert_eq!(*values.last().unwrap(), 35.min(quality));
            assert!(values.windows(2).all(|w| w[0] > w[1]));
        }
    }
    #[test]
    fn meaningless_or_invalid_limits_are_rejected() {
        assert!(qualities(85, None, Some(0), true).is_err());
        assert!(qualities(85, Some(86), Some(100), true).is_err());
        assert!(qualities(85, Some(35), None, true).is_err());
        assert!(qualities(85, Some(35), Some(100), false).is_err());
        assert_eq!(qualities(85, None, Some(100), false).unwrap(), [85]);
    }
}
