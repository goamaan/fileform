// SPDX-License-Identifier: Apache-2.0
use crate::{fail, native_process, pdf_graph, pdf_inspect, Cancellation, Result};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::BufReader,
    path::Path,
    time::Duration,
};

#[derive(Clone, Debug, Serialize)]
pub struct ImageCandidate {
    pub source_index: usize,
    pub object_number: u32,
    pub generation: u16,
    pub resource_pages: Vec<u32>,
    pub resource_paths: Vec<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub filters: Vec<String>,
    pub color_space: Option<String>,
    pub encoding_outcome: Option<&'static str>,
    pub alpha_handling: Option<&'static str>,
    pub skip_reason: Option<String>,
    pub artifact_name: Option<String>,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
}
pub(crate) struct ImageObject {
    pub candidate: ImageCandidate,
    pub reference: String,
    pub channels: usize,
    pub soft_mask: Option<String>,
}
pub(crate) struct ImageGraph {
    root: Value,
}
fn invalid(message: &str) -> crate::Failure {
    fail("verification", message)
}
pub(crate) fn reference(value: &str) -> Result<(u32, u16)> {
    if !pdf_graph::reference(value) {
        return Err(invalid("Invalid PDF image reference."));
    }
    let mut parts = value.split(' ');
    Ok((
        parts
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| invalid("Invalid image object number."))?,
        parts
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| invalid("Invalid image object generation."))?,
    ))
}
impl ImageGraph {
    pub(crate) fn load(input: &Path, directory: &Path, cancel: &Cancellation) -> Result<Self> {
        pdf_inspect::verify_pack(directory, cancel)?;
        let mut temporary = tempfile::NamedTempFile::new()?;
        let mut command = pdf_inspect::command(directory)?;
        command
            .args([
                "--json",
                "--json-key=qpdf",
                "--json-key=pages",
                "--json-key=encrypt",
                "--json-stream-data=none",
            ])
            .arg(input);
        native_process::run_to_file(
            command,
            cancel,
            Duration::from_secs(60),
            &mut temporary,
            32 * 1024 * 1024,
        )?;
        let root: Value = serde_json::from_reader(BufReader::new(temporary.as_file_mut()))?;
        Self::from_value(root)
    }
    fn from_value(root: Value) -> Result<Self> {
        if root
            .get("encrypt")
            .and_then(|v| v.get("encrypted"))
            .and_then(Value::as_bool)
            != Some(false)
            || root
                .get("qpdf")
                .and_then(Value::as_array)
                .is_none_or(|v| v.len() != 2)
            || root
                .get("pages")
                .and_then(Value::as_array)
                .is_none_or(|v| v.is_empty() || v.len() > 1000)
        {
            return Err(fail(
                "unsupported",
                "Image extraction needs an unencrypted PDF with 1–1000 pages.",
            ));
        }
        let result = Self { root };
        if result.objects()?.len() > 100000 {
            return Err(fail("limit", "PDF image inventory exceeds 100000 objects."));
        }
        if result.root["qpdf"][0]["jsonversion"] != 2 {
            return Err(invalid("Unsupported qpdf object inventory."));
        }
        Ok(result)
    }
    fn objects(&self) -> Result<&Map<String, Value>> {
        self.root["qpdf"][1]
            .as_object()
            .ok_or_else(|| invalid("Invalid PDF object inventory."))
    }
    fn object(&self, reference: &str) -> Result<&Value> {
        self.objects()?
            .get(&format!("obj:{reference}"))
            .ok_or_else(|| invalid("Missing PDF resource object."))
    }
    fn reference_dict(&self, reference: &str) -> Result<&Map<String, Value>> {
        let object = self.object(reference)?;
        self.dict(
            object
                .get("stream")
                .and_then(|v| v.get("dict"))
                .or_else(|| object.get("value")),
        )?
        .ok_or_else(|| invalid("Missing PDF object dictionary."))
    }
    fn resolve<'a>(&'a self, mut value: Option<&'a Value>) -> Result<Option<&'a Value>> {
        let mut seen = BTreeSet::new();
        for _ in 0..33 {
            let Some(reference) = value
                .and_then(Value::as_str)
                .filter(|v| pdf_graph::reference(v))
            else {
                return Ok(value);
            };
            if !seen.insert(reference) {
                return Err(invalid("Cyclic PDF image metadata reference."));
            }
            let object = self.object(reference)?;
            if let Some(stream) = object.get("stream") {
                return stream
                    .get("dict")
                    .map(Some)
                    .ok_or_else(|| invalid("Missing PDF stream dictionary."));
            }
            value = object.get("value");
            if value.is_none() {
                return Err(invalid("Missing PDF indirect value."));
            }
        }
        Err(fail(
            "limit",
            "PDF image metadata reference depth exceeds 32.",
        ))
    }
    fn dict<'a>(&'a self, value: Option<&'a Value>) -> Result<Option<&'a Map<String, Value>>> {
        match self.resolve(value)? {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_object()
                .map(Some)
                .ok_or_else(|| invalid("Invalid PDF resource dictionary.")),
        }
    }
    fn number(&self, value: Option<&Value>) -> Result<Option<u32>> {
        Ok(self
            .resolve(value)?
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= f64::from(i32::MAX) && v.fract() == 0.0)
            .map(|v| v as u32))
    }
    fn text<'a>(&'a self, value: Option<&'a Value>) -> Result<Option<&'a str>> {
        Ok(self.resolve(value)?.and_then(Value::as_str))
    }
    fn filters(&self, dict: &Map<String, Value>) -> Result<Vec<String>> {
        let value = self.resolve(dict.get("/Filter"))?;
        let names = match value {
            None | Some(Value::Null) => return Ok(vec![]),
            Some(Value::String(v)) => vec![v.as_str()],
            Some(Value::Array(v)) if v.len() <= 16 => {
                let mut names = vec![];
                for v in v {
                    names.push(
                        self.text(Some(v))?
                            .unwrap_or("unsupported-filter-definition"),
                    );
                }
                names
            }
            _ => return Ok(vec!["unsupported-filter-definition".into()]),
        };
        Ok(names
            .into_iter()
            .map(|v| {
                if v.len() <= 128 {
                    v.to_owned()
                } else {
                    "unsupported-filter-definition".into()
                }
            })
            .collect())
    }
    fn samples_supported(&self, dict: &Map<String, Value>) -> Result<bool> {
        let filters = self.filters(dict)?;
        if !filters.iter().all(|v| {
            matches!(
                v.as_str(),
                "/FlateDecode" | "/ASCII85Decode" | "/ASCIIHexDecode" | "/RunLengthDecode"
            )
        }) {
            return Ok(false);
        }
        Ok(match self.resolve(dict.get("/DecodeParms"))? {
            None | Some(Value::Null) => true,
            Some(Value::Array(v)) => v.len() == filters.len() && v.iter().all(Value::is_null),
            Some(Value::Object(v)) => v.is_empty(),
            _ => false,
        })
    }
    fn candidate(
        &self,
        source_index: usize,
        reference: &str,
        page: u32,
        path: String,
    ) -> Result<ImageObject> {
        let (object_number, generation) = self::reference(reference)?;
        let dict = self.reference_dict(reference)?;
        let width = self.number(dict.get("/Width"))?;
        let height = self.number(dict.get("/Height"))?;
        let filters = self.filters(dict)?;
        let space = self.text(dict.get("/ColorSpace"))?;
        let color_space = space.map(|v| {
            if v.len() <= 128 {
                v.to_owned()
            } else {
                "unsupported-color-space".into()
            }
        });
        let channels = if space == Some("/DeviceRGB") { 3 } else { 1 };
        let jpeg = filters == ["/DCTDecode"];
        let mut reason = if self.object(reference)?.get("stream").is_none() {
            Some("Image object is not a stream.")
        } else if width.is_none_or(|v| v == 0) || height.is_none_or(|v| v == 0) {
            Some("Invalid intrinsic dimensions.")
        } else if width.is_some_and(|v| v > 16384)
            || height.is_some_and(|v| v > 16384)
            || u64::from(width.unwrap_or(0)) * u64::from(height.unwrap_or(0)) > 64_000_000
        {
            Some("Image exceeds 16384 pixels per edge or 64 million pixels.")
        } else if self
            .resolve(dict.get("/ImageMask"))?
            .and_then(Value::as_bool)
            == Some(true)
        {
            Some("Stencil image masks are unsupported.")
        } else if self.number(dict.get("/BitsPerComponent"))? != Some(8) {
            Some("Only 8-bit image samples are supported.")
        } else if !matches!(space, Some("/DeviceRGB" | "/DeviceGray")) {
            Some("Unsupported color space; only DeviceRGB and DeviceGray are supported.")
        } else if dict.contains_key("/Decode") {
            Some("Custom Decode arrays are unsupported.")
        } else if dict.contains_key("/Mask") {
            Some("Explicit image or color-key masks are unsupported.")
        } else if dict.contains_key("/Alternates") || dict.contains_key("/OPI") {
            Some("Alternate image representations are unsupported.")
        } else {
            None
        };
        let mut soft_mask = None;
        if reason.is_none() {
            if let Some(mask) = dict.get("/SMask") {
                if mask.as_str() != Some("/None") {
                    if jpeg {
                        reason =
                            Some("JPEG with a soft mask cannot preserve encoded bytes faithfully.");
                    } else if let Some(reference) =
                        mask.as_str().filter(|v| pdf_graph::reference(v))
                    {
                        let md = self
                            .dict(Some(mask))?
                            .ok_or_else(|| invalid("Missing soft-mask dictionary."))?;
                        if self.text(md.get("/Subtype"))? != Some("/Image")
                            || self.number(md.get("/Width"))? != width
                            || self.number(md.get("/Height"))? != height
                            || self.number(md.get("/BitsPerComponent"))? != Some(8)
                            || self.text(md.get("/ColorSpace"))? != Some("/DeviceGray")
                        {
                            reason = Some("Soft mask must be a same-size 8-bit DeviceGray image.");
                        } else if ["/Matte", "/Decode", "/Mask", "/SMask", "/ImageMask"]
                            .iter()
                            .any(|key| md.contains_key(*key))
                        {
                            reason = Some(
                                "Soft-mask Matte, Decode or nested-mask semantics are unsupported.",
                            );
                        } else if !self.samples_supported(md)? {
                            reason =
                                Some("Soft-mask filters or predictor parameters are unsupported.");
                        } else {
                            soft_mask = Some(reference.to_owned());
                        }
                    } else {
                        reason = Some("Unsupported soft-mask reference.");
                    }
                }
            }
        }
        if reason.is_none() {
            if jpeg {
                if self
                    .resolve(dict.get("/DecodeParms"))?
                    .is_some_and(|v| !v.is_null())
                {
                    reason = Some("JPEG decode parameters are unsupported.");
                }
            } else if !self.samples_supported(dict)? {
                reason = Some("Unsupported image filters or predictor parameters.");
            }
        }
        Ok(ImageObject {
            candidate: ImageCandidate {
                source_index,
                object_number,
                generation,
                resource_pages: vec![page],
                resource_paths: vec![path],
                width,
                height,
                filters,
                color_space,
                encoding_outcome: reason.is_none().then_some(if jpeg {
                    "preserved_encoded_bytes"
                } else {
                    "reconstructed_pixels"
                }),
                alpha_handling: reason.is_none().then_some(if soft_mask.is_some() {
                    "straight alpha from soft mask; hidden RGB preserved"
                } else {
                    "opaque"
                }),
                skip_reason: reason.map(str::to_owned),
                artifact_name: None,
                bytes: None,
                sha256: None,
            },
            reference: reference.into(),
            channels,
            soft_mask,
        })
    }
    pub(crate) fn discover(
        &self,
        source_index: usize,
        pages: &[u32],
        cancel: &Cancellation,
    ) -> Result<Vec<ImageObject>> {
        let page_objects = self.root["pages"]
            .as_array()
            .ok_or_else(|| invalid("Missing PDF pages."))?;
        let mut walk = Discovery {
            graph: self,
            source_index,
            cancel,
            images: vec![],
            indices: BTreeMap::new(),
            visits: 0,
            provenance: 0,
        };
        for &page in pages {
            cancel.check()?;
            let reference = page_objects
                .get(page as usize)
                .and_then(|v| v.get("object"))
                .and_then(Value::as_str)
                .ok_or_else(|| fail("invalid_request", "A selected PDF page does not exist."))?;
            self::reference(reference)?;
            let mut current = reference;
            let mut ancestors = BTreeSet::new();
            let mut resources = None;
            for _ in 0..64 {
                let reference = current;
                if !ancestors.insert(reference.to_owned()) {
                    return Err(invalid("Cyclic PDF page ancestry."));
                }
                let dict = self.reference_dict(current)?;
                if let Some(value) = dict.get("/Resources") {
                    resources = Some(value);
                    break;
                }
                if let Some(parent) = dict.get("/Parent").filter(|v| !v.is_null()) {
                    current = parent
                        .as_str()
                        .filter(|v| pdf_graph::reference(v))
                        .ok_or_else(|| invalid("Invalid PDF page ancestry reference."))?;
                } else {
                    break;
                }
                if ancestors.len() == 64 {
                    return Err(fail("limit", "PDF page ancestry exceeds 64."));
                }
            }
            walk.resources(resources, page, &[], &BTreeSet::new())?;
        }
        Ok(walk.images)
    }
}
struct Discovery<'a> {
    graph: &'a ImageGraph,
    source_index: usize,
    cancel: &'a Cancellation,
    images: Vec<ImageObject>,
    indices: BTreeMap<String, usize>,
    visits: usize,
    provenance: usize,
}
impl Discovery<'_> {
    fn resources(
        &mut self,
        value: Option<&Value>,
        page: u32,
        path: &[String],
        forms: &BTreeSet<String>,
    ) -> Result<()> {
        self.cancel.check()?;
        if path.len() > 32 {
            return Err(fail("limit", "PDF Form resource nesting exceeds 32."));
        }
        let Some(resources) = self.graph.dict(value)? else {
            return Ok(());
        };
        let Some(xobjects) = self.graph.dict(resources.get("/XObject"))? else {
            return Ok(());
        };
        for (name, reference) in xobjects {
            self.cancel.check()?;
            self.visits += 1;
            if self.visits > 100000 {
                return Err(fail(
                    "limit",
                    "PDF resource traversal exceeds 100000 references.",
                ));
            }
            let reference = reference
                .as_str()
                .filter(|v| pdf_graph::reference(v))
                .ok_or_else(|| fail("unsupported", "PDF XObjects require indirect references."))?;
            if name.len() > 256 {
                return Err(fail("limit", "PDF resource name exceeds 256 bytes."));
            }
            let dict = self.graph.reference_dict(reference)?;
            match self.graph.text(dict.get("/Subtype"))? {
                Some("/Form") => {
                    if forms.contains(reference) {
                        continue;
                    }
                    let mut nested = forms.clone();
                    nested.insert(reference.to_owned());
                    let mut path = path.to_vec();
                    path.push(name.to_owned());
                    self.resources(dict.get("/Resources").or(value), page, &path, &nested)?;
                }
                Some("/Image") => {
                    let location = format!(
                        "page {}: {}",
                        page + 1,
                        path.iter()
                            .chain(std::iter::once(name))
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" → ")
                    );
                    self.provenance += location.len();
                    if location.len() > 4096 || self.provenance > 4 * 1024 * 1024 {
                        return Err(fail(
                            "limit",
                            "PDF image provenance exceeds its metadata limit.",
                        ));
                    }
                    if let Some(&index) = self.indices.get(reference) {
                        let candidate = &mut self.images[index].candidate;
                        if !candidate.resource_pages.contains(&page) {
                            candidate.resource_pages.push(page);
                        }
                        if !candidate.resource_paths.contains(&location) {
                            candidate.resource_paths.push(location);
                        }
                    } else {
                        if self.images.len() == 1000 {
                            return Err(fail(
                                "limit",
                                "Extract at most 1000 unique image objects.",
                            ));
                        }
                        self.indices.insert(reference.into(), self.images.len());
                        self.images.push(self.graph.candidate(
                            self.source_index,
                            reference,
                            page,
                            location,
                        )?);
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}
