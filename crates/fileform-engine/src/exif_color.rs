// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Result};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hint {
    Unspecified,
    Srgb,
    AdobeRgb,
    Pending,
}
struct Reader<'a> {
    data: &'a [u8],
    little: bool,
}
impl Reader<'_> {
    fn bytes<const N: usize>(&self, offset: usize) -> Result<[u8; N]> {
        self.data
            .get(offset..offset.checked_add(N).ok_or_else(invalid)?)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())
    }
    fn short(&self, offset: usize) -> Result<u16> {
        let b = self.bytes(offset)?;
        Ok(if self.little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }
    fn long(&self, offset: usize) -> Result<u32> {
        let b = self.bytes(offset)?;
        Ok(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    fn field(&self, offset: usize, tag: u16) -> Result<Option<usize>> {
        if offset < 8 {
            return Err(invalid());
        }
        let count = usize::from(self.short(offset)?);
        if count > 4096 {
            return Err(fail("limit", "EXIF color directory exceeds 4096 entries."));
        }
        let end = offset.checked_add(2 + count * 12 + 4).ok_or_else(invalid)?;
        if end > self.data.len() {
            return Err(invalid());
        }
        let mut found = None;
        for i in 0..count {
            let entry = offset + 2 + i * 12;
            if self.short(entry)? == tag {
                if found.is_some() {
                    return Err(invalid());
                }
                found = Some(entry);
            }
        }
        Ok(found)
    }
    fn pointer(&self, offset: usize, tag: u16) -> Result<Option<usize>> {
        let Some(entry) = self.field(offset, tag)? else {
            return Ok(None);
        };
        if self.short(entry + 2)? != 4 || self.long(entry + 4)? != 1 {
            return Err(invalid());
        }
        Ok(Some(self.long(entry + 8)? as usize))
    }
}
fn invalid() -> crate::Failure {
    fail("invalid_image", "EXIF color metadata is malformed.")
}
pub(crate) fn read(data: &[u8]) -> Result<Hint> {
    if data.len() > 1024 * 1024 {
        return Err(fail("limit", "EXIF metadata exceeds 1 MiB."));
    }
    let little = match data.get(..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return Err(invalid()),
    };
    let r = Reader { data, little };
    if r.short(2)? != 42 {
        return Err(invalid());
    }
    let root = r.long(4)? as usize;
    let Some(exif) = r.pointer(root, 0x8769)? else {
        return Ok(Hint::Unspecified);
    };
    if exif == root {
        return Err(invalid());
    }
    // An explicit gamma needs its own interpretation unless an ICC profile wins.
    if r.field(exif, 0xa500)?.is_some() {
        return Ok(Hint::Pending);
    }
    let color = if let Some(entry) = r.field(exif, 0xa001)? {
        if r.short(entry + 2)? != 3 || r.long(entry + 4)? != 1 {
            return Err(invalid());
        }
        Some(r.short(entry + 8)?)
    } else {
        None
    };
    let interop = if let Some(offset) = r.pointer(exif, 0xa005)? {
        if offset == root || offset == exif {
            return Err(invalid());
        }
        if let Some(entry) = r.field(offset, 1)? {
            if r.short(entry + 2)? != 2 || r.long(entry + 4)? != 4 {
                return Ok(Hint::Pending);
            }
            Some(r.bytes::<4>(entry + 8)?)
        } else {
            None
        }
    } else {
        None
    };
    match (color, interop.as_ref()) {
        (Some(1), Some(b"R03\0")) | (Some(2), Some(b"R98\0")) => Ok(Hint::Pending),
        (Some(1), _) => Ok(Hint::Srgb),
        (Some(2), _) | (None | Some(0xffff), Some(b"R03\0")) => Ok(Hint::AdobeRgb),
        (None, Some(b"R98\0")) | (Some(0xffff), Some(b"R98\0")) => Ok(Hint::Srgb),
        (None, None) => Ok(Hint::Unspecified),
        _ => Ok(Hint::Pending),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(color: u16, index: &[u8; 4]) -> Vec<u8> {
        let mut d = b"II\x2a\0\x08\0\0\0\x01\0\x69\x87\x04\0\x01\0\0\0\x1a\0\0\0\0\0\0\0".to_vec();
        d.extend_from_slice(&[2, 0, 1, 0xa0, 3, 0, 1, 0, 0, 0]);
        d.extend_from_slice(&color.to_le_bytes());
        d.extend_from_slice(&[0, 0, 5, 0xa0, 4, 0, 1, 0, 0, 0, 56, 0, 0, 0, 0, 0, 0, 0]);
        d.extend_from_slice(&[1, 0, 1, 0, 2, 0, 4, 0, 0, 0]);
        d.extend_from_slice(index);
        d.extend_from_slice(&[0; 4]);
        d
    }
    #[test]
    fn standard_and_compatibility_hints_are_distinguished() {
        assert_eq!(read(&fixture(1, b"R98\0")).unwrap(), Hint::Srgb);
        assert_eq!(read(&fixture(0xffff, b"R03\0")).unwrap(), Hint::AdobeRgb);
        assert_eq!(read(&fixture(2, b"R03\0")).unwrap(), Hint::AdobeRgb);
        assert_eq!(read(&fixture(1, b"R03\0")).unwrap(), Hint::Pending);
        assert_eq!(read(&fixture(0xffff, b"???\0")).unwrap(), Hint::Pending);
    }
    #[test]
    fn big_endian_color_directories_are_read() {
        let mut data = fixture(1, b"R98\0");
        data[..2].copy_from_slice(b"MM");
        for offset in [2, 8, 10, 12, 26, 28, 30, 36, 40, 42, 56, 58, 60] {
            data[offset..offset + 2].reverse();
        }
        for offset in [4, 14, 18, 22, 32, 44, 48, 52, 62, 70] {
            data[offset..offset + 4].reverse();
        }
        assert_eq!(read(&data).unwrap(), Hint::Srgb);
    }
    #[test]
    fn malformed_color_directories_fail() {
        let original = fixture(1, b"R98\0");
        for end in 0..original.len() {
            assert!(read(&original[..end]).is_err());
        }
        let mut cycle = original;
        cycle[18..22].copy_from_slice(&8u32.to_le_bytes());
        assert!(read(&cycle).is_err());
    }
}
