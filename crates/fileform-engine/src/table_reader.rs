// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Result};
use std::io::{BufRead, BufReader, Bytes, Read};

/// Streaming equivalent of the existing strict delimited-table contract.
/// Blank records, CR/CRLF, escaped quotes and embedded line breaks are preserved.
pub(crate) struct TableReader<R: Read> {
    bytes: Bytes<BufReader<R>>,
    separator: u8,
    skip_lf: bool,
}
impl<R: Read> TableReader<R> {
    pub fn new(input: R, separator: u8) -> Result<Self> {
        let mut reader = BufReader::new(input);
        if reader.fill_buf()?.starts_with(&[0xef, 0xbb, 0xbf]) {
            reader.consume(3);
        }
        Ok(Self {
            bytes: reader.bytes(),
            separator,
            skip_lf: false,
        })
    }
    pub fn next_record(&mut self) -> Result<Option<Vec<String>>> {
        let mut row = Vec::new();
        let mut cell = Vec::new();
        let (mut quoted, mut closed, mut started) = (false, false, false);
        for byte in self.bytes.by_ref() {
            let byte = byte?;
            if self.skip_lf {
                self.skip_lf = false;
                if byte == b'\n' {
                    continue;
                }
            }
            if quoted {
                if byte == b'"' {
                    quoted = false;
                    closed = true;
                } else {
                    cell.push(byte);
                }
                continue;
            }
            if closed {
                if byte == b'"' {
                    cell.push(byte);
                    quoted = true;
                    closed = false;
                    continue;
                }
                if byte != self.separator && byte != b'\r' && byte != b'\n' {
                    return Err(fail(
                        "invalid_table",
                        "Unexpected text after a quoted cell.",
                    ));
                }
                closed = false;
            }
            match byte {
                b'"' => {
                    if !cell.is_empty() {
                        return Err(fail("invalid_table", "Quote inside an unquoted cell."));
                    }
                    quoted = true;
                    started = true;
                }
                b'\r' | b'\n' => {
                    self.skip_lf = byte == b'\r';
                    row.push(text(cell)?);
                    return Ok(Some(row));
                }
                byte if byte == self.separator => {
                    row.push(text(std::mem::take(&mut cell))?);
                    started = true;
                    if row.len() >= 1000 {
                        return Err(fail("limit", "Tables are limited to 1000 columns."));
                    }
                }
                byte => {
                    cell.push(byte);
                    started = true;
                }
            }
        }
        if quoted {
            return Err(fail("invalid_table", "A quoted cell was never closed."));
        }
        if started || !row.is_empty() || !cell.is_empty() {
            row.push(text(cell)?);
            Ok(Some(row))
        } else {
            Ok(None)
        }
    }
}
fn text(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| fail("invalid_table", "Save the table as valid UTF-8."))
}
