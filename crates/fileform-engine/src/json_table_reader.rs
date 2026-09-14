// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read};

/// A bounded, row-at-a-time flat JSON reader. Numeric tokens never pass through
/// floating point; strings are decoded by serde_json's strict string decoder.
pub struct JsonTableReader<R: Read> {
    input: BufReader<R>,
    columns: Option<Vec<String>>,
    first_row: Option<Vec<String>>,
    finished: bool,
}
impl<R: Read> JsonTableReader<R> {
    pub fn new(input: R) -> Result<Self> {
        let mut reader = Self {
            input: BufReader::new(input),
            columns: None,
            first_row: None,
            finished: false,
        };
        if reader.peek()? == Some(0xef) {
            for byte in [0xef, 0xbb, 0xbf] {
                reader.require_raw(byte)?;
            }
        }
        reader.require(b'[')?;
        Ok(reader)
    }
    fn peek(&mut self) -> Result<Option<u8>> {
        Ok(self.input.fill_buf()?.first().copied())
    }
    fn take(&mut self) -> Result<Option<u8>> {
        let byte = self.peek()?;
        if byte.is_some() {
            self.input.consume(1);
        }
        Ok(byte)
    }
    fn whitespace(&mut self) -> Result<()> {
        while self.peek()?.is_some_and(|b| b" \r\n\t".contains(&b)) {
            self.take()?;
        }
        Ok(())
    }
    fn require_raw(&mut self, byte: u8) -> Result<()> {
        if self.take()? != Some(byte) {
            return Err(fail(
                "invalid_json",
                "Expected a flat JSON array of objects.",
            ));
        }
        Ok(())
    }
    fn require(&mut self, byte: u8) -> Result<()> {
        self.whitespace()?;
        self.require_raw(byte)
    }
    fn string(&mut self) -> Result<String> {
        self.require(b'"')?;
        let mut token = vec![b'"'];
        let mut escaped = false;
        loop {
            let byte = self
                .take()?
                .ok_or_else(|| fail("invalid_json", "A JSON string was not closed."))?;
            token.push(byte);
            if token.len() as u64 > crate::MAX_INPUT {
                return Err(fail("limit", "JSON string exceeds the input limit."));
            }
            if !escaped && byte == b'"' {
                break;
            }
            escaped = !escaped && byte == b'\\';
        }
        Ok(serde_json::from_slice(&token)?)
    }
    fn scalar(&mut self) -> Result<String> {
        self.whitespace()?;
        if self.peek()? == Some(b'"') {
            return self.string();
        }
        let mut token = Vec::new();
        while let Some(byte) = self.peek()? {
            if b" \r\n\t,}]".contains(&byte) {
                break;
            }
            token.push(self.take()?.unwrap());
            if token.len() as u64 > crate::MAX_INPUT {
                return Err(fail("limit", "JSON cell exceeds the input limit."));
            }
        }
        match token.as_slice() {
            b"null" => Ok(String::new()),
            b"true" => Ok("true".into()),
            b"false" => Ok("false".into()),
            bytes if valid_number(bytes) => Ok(String::from_utf8(token).expect("number grammar is ASCII")),
            _ => Err(fail("invalid_table", "JSON cells must be strings, numbers, booleans or null. Nested values are unsupported.")),
        }
    }
    fn object(&mut self) -> Result<(Vec<String>, Vec<String>)> {
        self.require(b'{')?;
        let mut columns = Vec::new();
        let mut values = Vec::new();
        let mut seen = BTreeSet::new();
        loop {
            let key = self.string()?;
            if key.trim().is_empty() || !seen.insert(key.clone()) || columns.len() >= 1000 {
                return Err(fail(
                    "invalid_table",
                    "Use 1–1000 unique, non-empty JSON column names.",
                ));
            }
            columns.push(key);
            self.require(b':')?;
            values.push(self.scalar()?);
            self.whitespace()?;
            match self.take()? {
                Some(b'}') => break,
                Some(b',') => (),
                _ => {
                    return Err(fail(
                        "invalid_json",
                        "Expected a comma or closing object brace.",
                    ))
                }
            }
        }
        Ok((columns, values))
    }
    pub fn next_record(&mut self) -> Result<Option<Vec<String>>> {
        if self.finished {
            return Ok(None);
        }
        if self.columns.is_none() {
            let (columns, values) = self.object()?;
            self.columns = Some(columns.clone());
            self.first_row = Some(values);
            return Ok(Some(columns));
        }
        if let Some(row) = self.first_row.take() {
            return Ok(Some(row));
        }
        self.whitespace()?;
        match self.take()? {
            Some(b']') => {
                self.whitespace()?;
                if self.peek()?.is_some() {
                    return Err(fail(
                        "invalid_json",
                        "Unexpected content after the JSON table.",
                    ));
                }
                self.finished = true;
                Ok(None)
            }
            Some(b',') => {
                let (columns, values) = self.object()?;
                let mut row: BTreeMap<_, _> = columns.into_iter().zip(values).collect();
                let expected = self.columns.as_ref().unwrap();
                if row.len() != expected.len() {
                    return Err(fail("invalid_table", "JSON rows have different columns."));
                }
                let values = expected
                    .iter()
                    .map(|key| {
                        row.remove(key).ok_or_else(|| {
                            fail("invalid_table", "JSON rows have different columns.")
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Some(values))
            }
            _ => Err(fail(
                "invalid_json",
                "Expected a comma or closing array bracket.",
            )),
        }
    }
}

fn valid_number(bytes: &[u8]) -> bool {
    let mut i = usize::from(bytes.first() == Some(&b'-'));
    match bytes.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            i += 1;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => return false,
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == bytes.len()
}
