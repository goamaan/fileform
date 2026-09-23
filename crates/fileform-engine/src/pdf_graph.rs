// SPDX-License-Identifier: Apache-2.0
use crate::{fail, native_process, pdf_inspect, Cancellation, Result, Source};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, time::Duration};
fn reference(value: &str) -> bool {
    let mut parts = value.split(' ');
    let (Some(number), Some(generation), Some("R"), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    number
        .parse::<u32>()
        .is_ok_and(|n| n > 0 && n <= i32::MAX as u32)
        && generation.parse::<u16>().is_ok()
}
fn number(value: &str) -> Result<String> {
    let (negative, value) = value
        .strip_prefix('-')
        .map_or((false, value), |v| (true, v));
    let mut exponent = 0i64;
    let mantissa = if let Some((left, right)) = value.split_once(['e', 'E']) {
        exponent = right
            .parse::<i64>()
            .map_err(|_| fail("limit", "PDF numeric exponent exceeds its bound."))?;
        left
    } else {
        value
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    exponent = exponent
        .checked_sub(fraction.len() as i64)
        .ok_or_else(|| fail("limit", "PDF numeric scale overflow."))?;
    let mut digits = format!("{whole}{fraction}")
        .trim_start_matches('0')
        .to_string();
    if digits.is_empty() {
        return Ok("0".into());
    }
    let trimmed = digits.trim_end_matches('0').len();
    exponent = exponent
        .checked_add((digits.len() - trimmed) as i64)
        .ok_or_else(|| fail("limit", "PDF numeric scale overflow."))?;
    digits.truncate(trimmed);
    Ok(format!(
        "{}{digits}e{exponent}",
        if negative { "-" } else { "" }
    ))
}
struct Walk<'a> {
    objects: &'a Map<String, Value>,
    seen: BTreeMap<String, usize>,
    hash: Sha256,
    nodes: usize,
    cancel: &'a Cancellation,
}
impl Walk<'_> {
    fn add(&mut self, value: &str) {
        self.hash.update((value.len() as u64).to_be_bytes());
        self.hash.update(value.as_bytes());
    }
    fn visit(
        &mut self,
        value: &Value,
        depth: usize,
        stream_dict: bool,
        trailer: bool,
    ) -> Result<()> {
        self.cancel.check()?;
        self.nodes += 1;
        if depth > 128 || self.nodes > 1000000 {
            return Err(fail(
                "limit",
                "PDF graph traversal exceeds its depth or node bound.",
            ));
        }
        if let Some(reference) = value.as_str().filter(|s| reference(s)) {
            if let Some(index) = self.seen.get(reference) {
                self.add(&format!("reference:{index}"));
                return Ok(());
            }
            let object = self
                .objects
                .get(&format!("obj:{reference}"))
                .ok_or_else(|| fail("verification", "Referenced PDF object is missing."))?;
            let index = self.seen.len();
            self.seen.insert(reference.into(), index);
            self.add(&format!("object:{index}"));
            return self.visit(object, depth + 1, false, false);
        }
        match value {
            Value::Object(values) => {
                self.add("dictionary");
                let mut keys: Vec<_> = values.keys().collect();
                keys.sort();
                for key in keys {
                    if stream_dict && key == "/Length" {
                        continue;
                    }
                    if trailer && matches!(key.as_str(), "/ID" | "/Size" | "/Prev" | "/XRefStm") {
                        continue;
                    }
                    if trailer
                        && values.get("/Type").and_then(Value::as_str) == Some("/XRef")
                        && matches!(
                            key.as_str(),
                            "/Type" | "/W" | "/Index" | "/Length" | "/Filter" | "/DecodeParms"
                        )
                    {
                        continue;
                    }
                    self.add(key);
                    self.visit(
                        &values[key],
                        depth + 1,
                        key == "dict" && values.contains_key("data"),
                        false,
                    )?;
                }
                self.add("end-dictionary");
            }
            Value::Array(values) => {
                self.add(&format!("array:{}", values.len()));
                for value in values {
                    self.visit(value, depth + 1, false, false)?;
                }
                self.add("end-array");
            }
            Value::String(value) => {
                self.add("string");
                self.add(value);
            }
            Value::Number(value) => self.add(&format!("number:{}", number(&value.to_string())?)),
            Value::Bool(value) => self.add(if *value { "true" } else { "false" }),
            Value::Null => self.add("null"),
        }
        Ok(())
    }
}
#[derive(Debug, Serialize)]
pub struct GraphDigest {
    pub graph_sha256: String,
    pub objects: usize,
    pub reached_objects: usize,
    pub visited_nodes: usize,
}
pub(crate) fn fingerprint(document: &Value, cancel: &Cancellation) -> Result<GraphDigest> {
    let qpdf = document
        .get("qpdf")
        .and_then(Value::as_array)
        .filter(|q| q.len() == 2)
        .ok_or_else(|| fail("verification", "Invalid qpdf graph envelope."))?;
    if qpdf[0].get("jsonversion").and_then(Value::as_u64) != Some(2) {
        return Err(fail("verification", "Unsupported qpdf graph version."));
    }
    let objects = qpdf[1]
        .as_object()
        .filter(|m| m.len() <= 100000)
        .ok_or_else(|| fail("limit", "PDF graph exceeds 100000 objects."))?;
    let trailer = objects
        .get("trailer")
        .and_then(|v| v.get("value"))
        .filter(|v| v.is_object())
        .ok_or_else(|| fail("verification", "PDF trailer is missing."))?;
    for (key, object) in objects {
        if key != "trailer" && !key.strip_prefix("obj:").is_some_and(reference) {
            return Err(fail("verification", "Invalid PDF object key."));
        }
        if let Some(stream) = object.get("stream") {
            if !stream.get("dict").is_some_and(Value::is_object)
                || !stream.get("data").is_some_and(Value::is_string)
            {
                return Err(fail(
                    "verification",
                    "PDF stream data is missing from the proof.",
                ));
            }
        }
    }
    let mut walk = Walk {
        objects,
        seen: BTreeMap::new(),
        hash: Sha256::new(),
        nodes: 0,
        cancel,
    };
    walk.visit(trailer, 0, false, true)?;
    let graph_sha256 = walk
        .hash
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(GraphDigest {
        graph_sha256,
        objects: objects.len(),
        reached_objects: walk.seen.len(),
        visited_nodes: walk.nodes,
    })
}
#[derive(Debug, Serialize)]
pub struct GraphInspection {
    pub source_sha256: String,
    #[serde(flatten)]
    pub graph: GraphDigest,
}
pub fn inspect(input: &Path, directory: &Path, cancel: &Cancellation) -> Result<GraphInspection> {
    pdf_inspect::verify_pack(directory, cancel)?;
    let mut source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024)?;
    let mut command = pdf_inspect::command(directory)?;
    command
        .args([
            "--json",
            "--json-key=qpdf",
            "--json-stream-data=inline",
            "--decode-level=generalized",
        ])
        .arg(source.snapshot.path());
    let bytes = native_process::run(command, cancel, Duration::from_secs(120), 128 * 1024 * 1024)?;
    let graph: Value = serde_json::from_slice(&bytes)?;
    let graph = fingerprint(&graph, cancel)?;
    source.check(input)?;
    Ok(GraphInspection {
        source_sha256: source.hash,
        graph,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn document(id: u32, data: &str) -> Value {
        serde_json::json!({"qpdf":[{"jsonversion":2},{format!("obj:{id} 0 R"):{"stream":{"dict":{"/Length":99,"/Self":format!("{id} 0 R")},"data":data}},"trailer":{"value":{"/Root":format!("{id} 0 R"),"/ID":["ignored"],"/Size":100}}}]})
    }
    #[test]
    fn renumbering_and_serialization_do_not_hide_content_changes() {
        let cancel = Cancellation::default();
        let first = document(1, "YWJj");
        let mut renumbered = document(9, "YWJj");
        renumbered["qpdf"][1]["obj:9 0 R"]["stream"]["dict"]["/Length"] = 123.into();
        assert_eq!(
            fingerprint(&first, &cancel).unwrap().graph_sha256,
            fingerprint(&renumbered, &cancel).unwrap().graph_sha256
        );
        assert_ne!(
            fingerprint(&first, &cancel).unwrap().graph_sha256,
            fingerprint(&document(1, "YWJk"), &cancel)
                .unwrap()
                .graph_sha256
        );
        renumbered["qpdf"][1]["obj:9 0 R"]["stream"]["dict"]["/ID"] = "content".into();
        assert_ne!(
            fingerprint(&first, &cancel).unwrap().graph_sha256,
            fingerprint(&renumbered, &cancel).unwrap().graph_sha256
        );
    }
    #[test]
    fn missing_references_and_stream_payloads_are_rejected() {
        let cancel = Cancellation::default();
        let mut missing = document(1, "YWJj");
        missing["qpdf"][1]
            .as_object_mut()
            .unwrap()
            .remove("obj:1 0 R");
        assert!(fingerprint(&missing, &cancel).is_err());
        let mut missing = document(1, "YWJj");
        missing["qpdf"][1]["obj:1 0 R"]["stream"]
            .as_object_mut()
            .unwrap()
            .remove("data");
        assert!(fingerprint(&missing, &cancel).is_err());
    }
    #[test]
    fn numbers_are_exact_without_float_rounding() {
        assert_eq!(number("1.000").unwrap(), number("1e0").unwrap());
        assert_eq!(number("-0.0").unwrap(), "0");
        let a: Value = serde_json::from_str("0.123456789012345678901").unwrap();
        let b: Value = serde_json::from_str("0.123456789012345678902").unwrap();
        assert_ne!(
            number(&a.to_string()).unwrap(),
            number(&b.to_string()).unwrap()
        );
    }
}
