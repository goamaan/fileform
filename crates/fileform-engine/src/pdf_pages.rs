// SPDX-License-Identifier: Apache-2.0
use crate::{fail, native_process, pdf_graph, pdf_inspect, Cancellation, Result, Source};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{collections::BTreeSet, path::Path, time::Duration};
fn resolve<'a>(mut value: &'a Value, objects: &'a Map<String, Value>) -> Result<&'a Value> {
    let mut seen = BTreeSet::new();
    for _ in 0..32 {
        let Some(reference) = value.as_str().filter(|v| pdf_graph::reference(v)) else {
            return Ok(value);
        };
        if !seen.insert(reference) {
            return Err(fail("verification", "Cyclic PDF geometry reference."));
        }
        value = objects
            .get(&format!("obj:{reference}"))
            .and_then(|v| v.get("value"))
            .ok_or_else(|| fail("verification", "Missing PDF geometry object."))?;
    }
    Err(fail("limit", "PDF geometry reference depth exceeds 32."))
}
fn inherited<'a>(
    page: &'a Value,
    key: &str,
    objects: &'a Map<String, Value>,
) -> Result<Option<&'a Value>> {
    let mut current = page;
    let mut seen = BTreeSet::new();
    for _ in 0..64 {
        if let Some(reference) = current.as_str() {
            if !seen.insert(reference) {
                return Err(fail("verification", "Cyclic PDF page ancestry."));
            }
        }
        let dict = resolve(current, objects)?
            .as_object()
            .ok_or_else(|| fail("verification", "Invalid PDF page dictionary."))?;
        if let Some(value) = dict.get(key) {
            let value = resolve(value, objects)?;
            if !value.is_null() {
                return Ok(Some(value));
            }
        }
        match dict.get("/Parent") {
            Some(parent) if !parent.is_null() => current = parent,
            Some(_) => return Ok(None),
            None => return Ok(None),
        }
    }
    Err(fail("limit", "PDF page ancestry exceeds 64."))
}
fn rect(value: &Value, objects: &Map<String, Value>) -> Result<[f64; 4]> {
    let values = resolve(value, objects)?
        .as_array()
        .filter(|v| v.len() == 4)
        .ok_or_else(|| fail("unsupported", "Invalid PDF page box."))?;
    let mut result = [0.0; 4];
    for (index, value) in values.iter().enumerate() {
        result[index] = resolve(value, objects)?
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| fail("unsupported", "Invalid PDF page coordinate."))?;
    }
    let normalized = [
        result[0].min(result[2]),
        result[1].min(result[3]),
        result[0].max(result[2]),
        result[1].max(result[3]),
    ];
    let width = normalized[2] - normalized[0];
    let height = normalized[3] - normalized[1];
    if !(width > 0.0 && height > 0.0 && width <= 1000000.0 && height <= 1000000.0) {
        return Err(fail("unsupported", "Unsupported PDF page dimensions."));
    }
    Ok(normalized)
}
#[derive(Debug, Serialize)]
pub struct PageGeometry {
    pub position: u32,
    pub media_box: [f64; 4],
    pub crop_box: [f64; 4],
    pub effective_crop_box: [f64; 4],
    pub bleed_box: [f64; 4],
    pub trim_box: [f64; 4],
    pub art_box: [f64; 4],
    pub rotation: u32,
    pub user_unit: f64,
}
#[derive(Debug, Serialize)]
pub struct PageInspection {
    pub sha256: String,
    pub pages: Vec<PageGeometry>,
}
fn geometry(document: &Value, cancel: &Cancellation) -> Result<Vec<PageGeometry>> {
    let pages = document
        .get("pages")
        .and_then(Value::as_array)
        .filter(|p| !p.is_empty() && p.len() <= 1000)
        .ok_or_else(|| fail("limit", "PDF page list exceeds its limit or is empty."))?;
    if document
        .get("qpdf")
        .and_then(Value::as_array)
        .and_then(|q| q.first())
        .and_then(|h| h.get("jsonversion"))
        .and_then(Value::as_u64)
        != Some(2)
    {
        return Err(fail(
            "verification",
            "Unsupported PDF geometry JSON version.",
        ));
    }
    let objects = document
        .get("qpdf")
        .and_then(Value::as_array)
        .filter(|q| q.len() == 2)
        .and_then(|q| q[1].as_object())
        .filter(|o| o.len() <= 100000)
        .ok_or_else(|| fail("verification", "Invalid PDF object table."))?;
    let mut result = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        cancel.check()?;
        if page.get("pageposfrom1").and_then(Value::as_u64) != Some(index as u64 + 1) {
            return Err(fail("verification", "PDF page order is inconsistent."));
        }
        let object = page
            .get("object")
            .filter(|v| v.as_str().is_some_and(pdf_graph::reference))
            .ok_or_else(|| fail("verification", "Invalid PDF page reference."))?;
        let dictionary = resolve(object, objects)?
            .as_object()
            .ok_or_else(|| fail("verification", "Invalid PDF page object."))?;
        if dictionary
            .get("/Type")
            .map(|v| resolve(v, objects))
            .transpose()?
            .and_then(Value::as_str)
            != Some("/Page")
        {
            return Err(fail(
                "verification",
                "Page list points to a non-page object.",
            ));
        }
        let media = rect(
            inherited(object, "/MediaBox", objects)?
                .ok_or_else(|| fail("unsupported", "PDF page has no media box."))?,
            objects,
        )?;
        let crop = match inherited(object, "/CropBox", objects)? {
            Some(value) => rect(value, objects)?,
            None => media,
        };
        let effective = [
            media[0].max(crop[0]),
            media[1].max(crop[1]),
            media[2].min(crop[2]),
            media[3].min(crop[3]),
        ];
        if effective[2] <= effective[0] || effective[3] <= effective[1] {
            return Err(fail(
                "unsupported",
                "PDF crop does not intersect its media box.",
            ));
        }
        let optional_box = |key: &str| -> Result<[f64; 4]> {
            dictionary.get(key).map_or(Ok(crop), |v| {
                let value = resolve(v, objects)?;
                if value.is_null() {
                    Ok(crop)
                } else {
                    rect(value, objects)
                }
            })
        };
        let rotation = match inherited(object, "/Rotate", objects)? {
            None => 0,
            Some(value) => {
                let n = value
                    .as_f64()
                    .filter(|n| {
                        n.is_finite()
                            && n.fract() == 0.0
                            && *n >= f64::from(i32::MIN)
                            && *n <= f64::from(i32::MAX)
                    })
                    .ok_or_else(|| fail("unsupported", "Invalid PDF rotation."))?
                    as i32;
                if n % 90 != 0 {
                    return Err(fail("unsupported", "PDF rotation must be a right angle."));
                }
                n.rem_euclid(360) as u32
            }
        };
        let user_unit = match dictionary.get("/UserUnit") {
            None => 1.0,
            Some(value) if resolve(value, objects)?.is_null() => 1.0,
            Some(value) => resolve(value, objects)?
                .as_f64()
                .filter(|v| v.is_finite() && *v > 0.0 && *v <= 75000.0)
                .ok_or_else(|| fail("unsupported", "Invalid PDF user unit."))?,
        };
        result.push(PageGeometry {
            position: index as u32 + 1,
            media_box: media,
            crop_box: crop,
            effective_crop_box: effective,
            bleed_box: optional_box("/BleedBox")?,
            trim_box: optional_box("/TrimBox")?,
            art_box: optional_box("/ArtBox")?,
            rotation,
            user_unit,
        });
    }
    Ok(result)
}
pub fn inspect(input: &Path, directory: &Path, cancel: &Cancellation) -> Result<PageInspection> {
    pdf_inspect::verify_pack(directory, cancel)?;
    let mut source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024)?;
    let mut command = pdf_inspect::command(directory)?;
    command
        .args([
            "--json",
            "--json-key=pages",
            "--json-key=qpdf",
            "--json-stream-data=none",
        ])
        .arg(source.snapshot.path());
    let bytes = native_process::run(command, cancel, Duration::from_secs(120), 64 * 1024 * 1024)?;
    let pages = geometry(&serde_json::from_slice(&bytes)?, cancel)?;
    source.check(input)?;
    let result = PageInspection {
        sha256: source.hash,
        pages,
    };
    if serde_json::to_vec(&result)?.len() > 900000 {
        return Err(fail(
            "limit",
            "PDF geometry preview exceeds the response limit.",
        ));
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_geometry_and_local_boxes_have_distinct_rules() {
        let document = serde_json::json!({"pages":[{"object":"3 0 R","pageposfrom1":1}],"qpdf":[{"jsonversion":2}, {"obj:3 0 R":{"value":{"/Type":"/Page","/Parent":"2 0 R","/UserUnit":2,"/CropBox":null}},"obj:2 0 R":{"value":{"/MediaBox":[10,20,210,320],"/CropBox":[0,0,300,400],"/Rotate":-90,"/BleedBox":[1,1,2,2]}}}]});
        let result = geometry(&document, &Cancellation::default()).unwrap();
        let page = &result[0];
        assert_eq!(page.media_box, [10.0, 20.0, 210.0, 320.0]);
        assert_eq!(page.effective_crop_box, page.media_box);
        assert_eq!(page.bleed_box, page.crop_box);
        assert_eq!(page.rotation, 270);
        assert_eq!(page.user_unit, 2.0);
        let mut cycle = document;
        cycle["qpdf"][1]["obj:2 0 R"]["value"]
            .as_object_mut()
            .unwrap()
            .remove("/MediaBox");
        cycle["qpdf"][1]["obj:2 0 R"]["value"]["/Parent"] = "3 0 R".into();
        assert!(geometry(&cycle, &Cancellation::default()).is_err());
    }
}
