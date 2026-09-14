// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Result};
use std::io::{BufRead, Seek, SeekFrom};

/// Validate color/EXIF chunks independently: the decoder may ignore malformed
/// ancillary metadata. Never reinterpret such a file as untagged sRGB.
pub(crate) fn preflight<R: BufRead + Seek>(input: &mut R) -> Result<u16> {
    let length = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(0))?;
    let mut signature = [0u8; 8];
    input.read_exact(&mut signature)?;
    if signature != *b"\x89PNG\r\n\x1a\n" {
        return Err(fail("invalid_image", "Not a PNG image."));
    }
    let mut position = 8u64;
    let mut seen = 0u16;
    for _ in 0..100_000 {
        if position.checked_add(12).is_none_or(|end| end > length) {
            return Err(fail("invalid_image", "Truncated PNG chunk."));
        }
        let mut header = [0u8; 8];
        input.read_exact(&mut header)?;
        let size = u32::from_be_bytes(header[..4].try_into().unwrap()) as u64;
        let kind = &header[4..];
        let end = position
            .checked_add(size)
            .and_then(|n| n.checked_add(12))
            .ok_or_else(|| fail("limit", "PNG chunk size overflow."))?;
        if end > length {
            return Err(fail("invalid_image", "PNG chunk extends beyond the file."));
        }
        let (flag, fixed) = match kind {
            b"gAMA" => (1, Some(4)),
            b"cHRM" => (2, Some(32)),
            b"sRGB" => (4, Some(1)),
            b"iCCP" => (8, None),
            b"eXIf" => (16, None),
            b"cICP" => (32, Some(4)),
            b"mDCV" => (64, Some(24)),
            b"cLLI" => (128, Some(8)),
            _ => (0, None),
        };
        if flag != 0 {
            if seen & flag != 0
                || fixed.is_some_and(|expected| size != expected)
                || size > 1024 * 1024
            {
                return Err(fail(
                    "invalid_image",
                    "Duplicate, oversized or malformed PNG metadata.",
                ));
            }
            seen |= flag;
            let mut hash = crc32fast::Hasher::new();
            hash.update(kind);
            let mut remaining = size;
            let mut buffer = [0u8; 65536];
            let mut prefix = [0u8; 4];
            let mut first = true;
            while remaining > 0 {
                let n = remaining.min(buffer.len() as u64) as usize;
                input.read_exact(&mut buffer[..n])?;
                if first {
                    let count = n.min(4);
                    prefix[..count].copy_from_slice(&buffer[..count]);
                    first = false;
                }
                hash.update(&buffer[..n]);
                remaining -= n as u64;
            }
            let mut crc = [0u8; 4];
            input.read_exact(&mut crc)?;
            if hash.finalize() != u32::from_be_bytes(crc) {
                return Err(fail("invalid_image", "PNG metadata checksum is invalid."));
            }
            if (kind == b"gAMA" && u32::from_be_bytes(prefix) == 0)
                || (kind == b"sRGB" && prefix[0] > 3)
                || (kind == b"cICP" && (prefix[2] != 0 || prefix[3] > 1))
            {
                return Err(fail(
                    "invalid_image",
                    "PNG color metadata has invalid values.",
                ));
            }
        } else {
            input.seek(SeekFrom::Start(end))?;
        }
        if kind == b"IEND" {
            if size != 0 || end != length {
                return Err(fail("invalid_image", "PNG has an invalid ending."));
            }
            input.seek(SeekFrom::Start(0))?;
            return Ok(seen);
        }
        position = end;
    }
    Err(fail("limit", "PNG exceeds 100,000 chunks."))
}
