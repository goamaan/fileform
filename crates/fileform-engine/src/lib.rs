// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

mod table_reader;
use table_reader::TableReader;
pub const MAX_INPUT: u64 = 8 * 1024 * 1024;
const MAX_OUTPUT: u64 = 128 * 1024 * 1024;
const MAX_ROWS: u64 = 100_000;

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Inspect {
        input: PathBuf,
    },
    ConvertTable {
        input: PathBuf,
        output: PathBuf,
        expected_source_sha256: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
}
type Result<T> = std::result::Result<T, Failure>;
fn fail(code: &'static str, message: impl Into<String>) -> Failure {
    Failure {
        code,
        message: message.into(),
    }
}
impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        fail("io", e.to_string())
    }
}
impl From<serde_json::Error> for Failure {
    fn from(e: serde_json::Error) -> Self {
        fail("invalid_json", e.to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct Inspection {
    pub sha256: String,
    pub bytes: u64,
    pub rows: u64,
    pub columns: usize,
    pub column_names_preview: Vec<String>,
    pub outputs: Vec<&'static str>,
}
#[derive(Debug, Serialize)]
pub struct Receipt {
    pub output: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub rows: u64,
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Inspection(Inspection),
    Saved(Receipt),
}

fn delimiter(path: &Path) -> Result<u8> {
    match path
        .extension()
        .and_then(|x| x.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => Ok(b','),
        Some("tsv") => Ok(b'\t'),
        _ => Err(fail(
            "unsupported",
            "This worker currently accepts CSV and TSV tables.",
        )),
    }
}
fn digest(file: &mut File) -> Result<(u64, String)> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > MAX_OUTPUT {
            return Err(fail("limit", "File exceeds the verification limit."));
        }
        hash.update(&buffer[..n]);
    }
    Ok((
        bytes,
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    ))
}
struct Source {
    input: File,
    snapshot: NamedTempFile,
    identity: same_file::Handle,
    hash: String,
}
impl Source {
    fn open(path: &Path) -> Result<Self> {
        let mut input = File::open(path)?;
        let metadata = input.metadata()?;
        if !metadata.is_file() {
            return Err(fail("invalid_input", "Choose a regular file."));
        }
        if metadata.len() > MAX_INPUT {
            return Err(fail("limit", "Tables are limited to 8 MiB."));
        }
        let identity = same_file::Handle::from_file(input.try_clone()?)?;
        let mut snapshot = NamedTempFile::new()?;
        let copied = io::copy(&mut (&mut input).take(MAX_INPUT + 1), &mut snapshot)?;
        if copied > MAX_INPUT {
            return Err(fail("limit", "Table grew beyond 8 MiB."));
        }
        let (_, hash) = digest(snapshot.as_file_mut())?;
        let mut source = Self {
            input,
            snapshot,
            identity,
            hash,
        };
        source.check(path)?;
        Ok(source)
    }
    fn check(&mut self, path: &Path) -> Result<()> {
        let identity = same_file::Handle::from_path(path)?;
        if self.identity != identity || digest(&mut self.input)?.1 != self.hash {
            return Err(fail(
                "source_changed",
                "The source changed. Add the file again.",
            ));
        }
        Ok(())
    }
    fn reader(&mut self, delimiter: u8) -> Result<TableReader<&mut File>> {
        self.snapshot.as_file_mut().seek(SeekFrom::Start(0))?;
        TableReader::new(self.snapshot.as_file_mut(), delimiter)
    }
}
fn headers<R: Read>(reader: &mut TableReader<R>) -> Result<Vec<String>> {
    let names = reader
        .next_record()?
        .ok_or_else(|| fail("invalid_table", "A header row is required."))?;
    if names.is_empty() || names.len() > 1000 || names.iter().any(|x| x.trim().is_empty()) {
        return Err(fail("invalid_table", "Use 1–1000 non-empty column names."));
    }
    if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
        return Err(fail(
            "invalid_table",
            "Duplicate column names cannot be represented in JSON.",
        ));
    }
    Ok(names)
}
fn check_record(record: &[String], rows: u64) -> Result<()> {
    if rows > MAX_ROWS || record.iter().map(String::len).sum::<usize>() > 8 * 1024 * 1024 {
        return Err(fail("limit", "Table exceeds the row or record limit."));
    }
    Ok(())
}
struct LimitedWriter<W> {
    inner: W,
    bytes: u64,
}
impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes + bytes.len() as u64 > MAX_OUTPUT {
            return Err(io::Error::other("Table output exceeds 128 MiB."));
        }
        let n = self.inner.write(bytes)?;
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub fn execute(request: Request) -> Result<Response> {
    let input = match &request {
        Request::Inspect { input } | Request::ConvertTable { input, .. } => input,
    };
    let separator = delimiter(input)?;
    let mut source = Source::open(input)?;
    match request {
        Request::Inspect { input } => {
            let mut reader = source.reader(separator)?;
            let columns = headers(&mut reader)?;
            let mut rows = 0;
            while let Some(record) = reader.next_record()? {
                rows += 1;
                check_record(&record, rows)?;
                if record.len() != columns.len() {
                    return Err(fail("invalid_table", "Rows have different column counts."));
                }
            }
            source.check(&input)?;
            Ok(Response::Inspection(Inspection {
                sha256: source.hash.clone(),
                bytes: source.input.metadata()?.len(),
                rows,
                columns: columns.len(),
                column_names_preview: columns
                    .iter()
                    .take(20)
                    .map(|s| s.chars().take(128).collect())
                    .collect(),
                outputs: vec!["json", "csv", "tsv"],
            }))
        }
        Request::ConvertTable {
            input,
            output,
            expected_source_sha256,
        } => {
            if expected_source_sha256
                .as_ref()
                .is_some_and(|hash| hash != &source.hash)
            {
                return Err(fail(
                    "source_changed",
                    "The inspected file changed. Add it again.",
                ));
            }
            let output_separator = match output
                .extension()
                .and_then(|x| x.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("json") => None,
                Some("csv") => Some(b','),
                Some("tsv") => Some(b'\t'),
                _ => {
                    return Err(fail(
                        "invalid_output",
                        "Choose a .json, .csv or .tsv output filename.",
                    ))
                }
            };
            if output.try_exists()? {
                return Err(fail(
                    "collision",
                    "The output already exists. Choose another filename.",
                ));
            }
            let parent = output
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let directory = same_file::Handle::from_path(parent)?;
            let mut temporary = NamedTempFile::new_in(parent)?;
            let mut reader = source.reader(separator)?;
            let columns = headers(&mut reader)?;
            let mut rows = 0;
            {
                let mut writer = LimitedWriter {
                    inner: io::BufWriter::new(temporary.as_file_mut()),
                    bytes: 0,
                };
                if let Some(separator) = output_separator {
                    write_record(&mut writer, &columns, separator)?;
                } else {
                    writer.write_all(b"[")?;
                }
                while let Some(record) = reader.next_record()? {
                    rows += 1;
                    check_record(&record, rows)?;
                    if record.len() != columns.len() {
                        return Err(fail("invalid_table", "Rows have different column counts."));
                    }
                    if let Some(separator) = output_separator {
                        write_record(&mut writer, &record, separator)?;
                    } else {
                        if rows > 1 {
                            writer.write_all(b",")?;
                        }
                        let object: std::collections::BTreeMap<_, _> =
                            columns.iter().zip(record.iter()).collect();
                        serde_json::to_writer(&mut writer, &object)?;
                    }
                }
                if output_separator.is_none() {
                    writer.write_all(b"]\n")?;
                }
                writer.flush()?;
            }
            drop(reader);
            if let Some(output_separator) = output_separator {
                verify_delimited(
                    &mut source,
                    separator,
                    temporary.as_file_mut(),
                    output_separator,
                    rows,
                )?;
            } else {
                verify_table(&mut source, separator, temporary.as_file_mut(), rows)?;
            }
            let (bytes, sha256) = digest(temporary.as_file_mut())?;
            source.check(&input)?;
            if directory != same_file::Handle::from_path(parent)? {
                return Err(fail("output_changed", "The output folder changed."));
            }
            temporary.as_file().sync_all()?;
            temporary.persist_noclobber(&output).map_err(|e| {
                fail(
                    if e.error.kind() == io::ErrorKind::AlreadyExists {
                        "collision"
                    } else {
                        "io"
                    },
                    e.error.to_string(),
                )
            })?;
            Ok(Response::Saved(Receipt {
                output,
                bytes,
                sha256,
                rows,
            }))
        }
    }
}

// Encode one record at a time, including empty single-column records. Quote
// delimiters, newlines and embedded quotes; never infer spreadsheet value types.
fn write_record<W: Write>(writer: &mut W, record: &[String], separator: u8) -> Result<()> {
    for (index, cell) in record.iter().enumerate() {
        if index > 0 {
            writer.write_all(&[separator])?;
        }
        let quote = cell
            .as_bytes()
            .iter()
            .any(|b| [separator, b'"', b'\r', b'\n'].contains(b));
        if quote {
            writer.write_all(b"\"")?;
        }
        for part in cell.split_inclusive('"') {
            writer.write_all(part.as_bytes())?;
            if quote && part.ends_with('"') {
                writer.write_all(b"\"")?;
            }
        }
        if quote {
            writer.write_all(b"\"")?;
        }
    }
    writer.write_all(b"\r\n")?;
    Ok(())
}

fn verify_delimited(
    source: &mut Source,
    separator: u8,
    output: &mut File,
    output_separator: u8,
    expected_rows: u64,
) -> Result<()> {
    let mut original = source.reader(separator)?;
    output.seek(SeekFrom::Start(0))?;
    let mut decoded = TableReader::new(output, output_separator)?;
    let mut records = 0;
    loop {
        let left = original.next_record()?;
        let right = decoded.next_record()?;
        if left != right {
            return Err(fail("verification", "Output cells differ from source."));
        }
        if left.is_none() {
            break;
        }
        records += 1;
    }
    if records != expected_rows + 1 {
        return Err(fail("verification", "Output row count differs."));
    }
    Ok(())
}

fn verify_table(
    source: &mut Source,
    separator: u8,
    output: &mut File,
    expected_rows: u64,
) -> Result<()> {
    use serde::de::{self, SeqAccess, Visitor};
    struct Rows<'a> {
        reader: TableReader<&'a mut File>,
        columns: Vec<String>,
        expected: u64,
    }
    impl<'de> Visitor<'de> for Rows<'_> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("the verified table rows")
        }
        fn visit_seq<A: SeqAccess<'de>>(mut self, mut seq: A) -> std::result::Result<(), A::Error> {
            let mut rows = 0;
            while let Some(object) =
                seq.next_element::<std::collections::BTreeMap<String, String>>()?
            {
                let record = self
                    .reader
                    .next_record()
                    .map_err(|e| de::Error::custom(e.message))?
                    .ok_or_else(|| de::Error::custom("Unexpected output row"))?;
                if object.len() != self.columns.len()
                    || self.columns.iter().zip(record.iter()).any(|(key, value)| {
                        object.get(key).map(String::as_str) != Some(value.as_str())
                    })
                {
                    return Err(de::Error::custom("Output values differ from source"));
                }
                rows += 1;
            }
            if rows != self.expected
                || self
                    .reader
                    .next_record()
                    .map_err(|e| de::Error::custom(e.message))?
                    .is_some()
            {
                return Err(de::Error::custom("Output row count differs"));
            }
            Ok(())
        }
    }
    let mut reader = source.reader(separator)?;
    let columns = headers(&mut reader)?;
    output.seek(SeekFrom::Start(0))?;
    let mut decoder = serde_json::Deserializer::from_reader(io::BufReader::new(output));
    serde::de::Deserializer::deserialize_seq(
        &mut decoder,
        Rows {
            reader,
            columns,
            expected: expected_rows,
        },
    )?;
    decoder.end()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn convert(text: &[u8], extension: &str) -> (tempfile::TempDir, PathBuf, Result<Response>) {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join(format!("source.{extension}"));
        let output = dir.path().join("output.json");
        std::fs::write(&input, text).unwrap();
        let result = execute(Request::ConvertTable {
            input: input.clone(),
            output: output.clone(),
            expected_source_sha256: None,
        });
        assert_eq!(std::fs::read(&input).unwrap(), text);
        (dir, output, result)
    }
    #[test]
    fn quoted_unicode_multiline_and_empty_values_survive() {
        let (_dir, output, result) = convert(
            "\u{feff}name,note,value\r\n\"日本,語\",\"line 1\nline \"\"2\"\"\",001\r\nempty,,\r\n"
                .as_bytes(),
            "csv",
        );
        let Response::Saved(receipt) = result.unwrap() else {
            panic!("expected saved output")
        };
        assert_eq!(receipt.rows, 2);
        let actual: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
        assert_eq!(
            actual,
            serde_json::json!([{"name":"日本,語","note":"line 1\nline \"2\"","value":"001"},{"name":"empty","note":"","value":""}])
        );
    }
    #[test]
    fn blank_single_column_records_are_retained() {
        let (_dir, output, result) = convert(b"name\n\none\r\n", "csv");
        result.unwrap();
        let actual: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
        assert_eq!(actual, serde_json::json!([{"name":""},{"name":"one"}]));
    }
    #[test]
    fn tsv_carriage_returns_and_quotes_work() {
        let (_dir, output, result) = convert(b"a\tb\r\"x\ty\"\t2\r", "tsv");
        result.unwrap();
        let actual: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
        assert_eq!(actual, serde_json::json!([{"a":"x\ty","b":"2"}]));
    }
    #[test]
    fn invalid_tables_never_publish_and_clean_staging() {
        for text in [
            b"a,a\n1,2".as_slice(),
            b"a, \n1,2",
            b"a,b\n1",
            b"a\n\"unclosed",
            b"a\nx\"y",
            b"a\n\"x\" trailing",
            b"a\n\xff",
            b"",
        ] {
            let (dir, output, result) = convert(text, "csv");
            assert!(result.is_err(), "accepted {text:?}");
            assert!(!output.exists());
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }
    #[test]
    fn existing_output_and_source_alias_are_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.csv");
        let output = dir.path().join("output.json");
        std::fs::write(&input, b"a\noriginal\n").unwrap();
        std::fs::hard_link(&input, &output).unwrap();
        let error = execute(Request::ConvertTable {
            input: input.clone(),
            output: output.clone(),
            expected_source_sha256: None,
        })
        .unwrap_err();
        assert_eq!(error.code, "collision");
        assert_eq!(std::fs::read(input).unwrap(), b"a\noriginal\n");
        assert_eq!(std::fs::read(output).unwrap(), b"a\noriginal\n");
    }
    #[test]
    fn changed_source_is_detected_after_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.csv");
        std::fs::write(&input, b"a\none\n").unwrap();
        let mut source = Source::open(&input).unwrap();
        std::fs::write(&input, b"a\ntwo\n").unwrap();
        assert_eq!(source.check(&input).unwrap_err().code, "source_changed");
    }
    #[test]
    fn oversized_inputs_fail_before_processing() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("large.csv");
        File::create(&input)
            .unwrap()
            .set_len(MAX_INPUT + 1)
            .unwrap();
        assert_eq!(
            execute(Request::Inspect { input }).unwrap_err().code,
            "limit"
        );
    }
    #[test]
    fn inspected_hash_prevents_saving_changed_content() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.csv");
        let output = dir.path().join("output.json");
        std::fs::write(&input, b"a\none\n").unwrap();
        let Response::Inspection(inspection) = execute(Request::Inspect {
            input: input.clone(),
        })
        .unwrap() else {
            panic!("inspection expected")
        };
        std::fs::write(&input, b"a\ntwo\n").unwrap();
        let error = execute(Request::ConvertTable {
            input: input.clone(),
            output: output.clone(),
            expected_source_sha256: Some(inspection.sha256),
        })
        .unwrap_err();
        assert_eq!(error.code, "source_changed");
        assert!(!output.exists());
        assert_eq!(std::fs::read(input).unwrap(), b"a\ntwo\n");
    }
    #[test]
    fn delimited_outputs_preserve_special_cells_and_column_order() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.csv");
        let text = "z,a,note\r\n001,日本語,\"comma, tab\t quote \"\" and newline\nend\"\r\n,,\r\n";
        std::fs::write(&input, text).unwrap();
        for (index, extension) in ["csv", "tsv", "CSV", "TSV"].iter().enumerate() {
            let output = dir.path().join(format!("result-{index}.{extension}"));
            let Response::Saved(receipt) = execute(Request::ConvertTable {
                input: input.clone(),
                output: output.clone(),
                expected_source_sha256: None,
            })
            .unwrap() else {
                panic!("expected receipt")
            };
            assert_eq!(receipt.rows, 2);
            let separator = if extension.eq_ignore_ascii_case("csv") {
                b','
            } else {
                b'\t'
            };
            let mut reader = TableReader::new(File::open(&output).unwrap(), separator).unwrap();
            assert_eq!(reader.next_record().unwrap().unwrap(), ["z", "a", "note"]);
            assert_eq!(
                reader.next_record().unwrap().unwrap(),
                ["001", "日本語", "comma, tab\t quote \" and newline\nend"]
            );
            assert_eq!(reader.next_record().unwrap().unwrap(), ["", "", ""]);
            assert!(reader.next_record().unwrap().is_none());
            assert_eq!(std::fs::read_to_string(&input).unwrap(), text);
        }
    }
    #[test]
    fn delimited_blank_rows_and_header_only_tables_survive() {
        for text in ["name\r\n\r\none\r\n", "name\r\n"] {
            let dir = tempfile::tempdir().unwrap();
            let input = dir.path().join("source.tsv");
            let output = dir.path().join("result.csv");
            std::fs::write(&input, text).unwrap();
            execute(Request::ConvertTable {
                input,
                output: output.clone(),
                expected_source_sha256: None,
            })
            .unwrap();
            assert_eq!(std::fs::read_to_string(output).unwrap(), text);
        }
    }
    #[test]
    fn delimited_verification_rejects_corrupted_values() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.csv");
        std::fs::write(&input, "a,b\n1,2\n").unwrap();
        let mut source = Source::open(&input).unwrap();
        let mut output = NamedTempFile::new().unwrap();
        output.write_all(b"a\tb\r\n1\t3\r\n").unwrap();
        assert_eq!(
            verify_delimited(&mut source, b',', output.as_file_mut(), b'\t', 1)
                .unwrap_err()
                .code,
            "verification"
        );
    }
    #[test]
    fn concurrent_publish_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.csv");
        let output = dir.path().join("output.json");
        std::fs::write(&input, b"a\nvalue\n").unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let jobs: Vec<_> = (0..2)
            .map(|_| {
                let (input, output, barrier) = (input.clone(), output.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    execute(Request::ConvertTable {
                        input,
                        output,
                        expected_source_sha256: None,
                    })
                })
            })
            .collect();
        let results: Vec<_> = jobs.into_iter().map(|job| job.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter_map(|r| r.as_ref().err())
                .next()
                .unwrap()
                .code,
            "collision"
        );
        let actual: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
        assert_eq!(actual, serde_json::json!([{"a":"value"}]));
    }
}
