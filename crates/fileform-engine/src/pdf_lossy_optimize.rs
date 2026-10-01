// SPDX-License-Identifier: Apache-2.0
use crate::{
    digest, fail, native_process, pdf_graph, pdf_image_graph::ImageGraph, pdf_inspect,
    pdf_lossy_images, pdf_pages, pdf_render, pdf_text, Cancellation, Result, Source,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageOptimization {
    #[serde(default)]
    pub collision: crate::OutputCollision,
    pub input: PathBuf,
    pub output: PathBuf,
    pub directory: PathBuf,
    pub renderer_directory: PathBuf,
    #[serde(default = "initial_quality")]
    pub quality: f64,
    #[serde(default = "floor_quality")]
    pub minimum_quality: f64,
    pub max_dimension: Option<u32>,
    pub max_bytes: Option<u64>,
    #[serde(default)]
    pub dry_run: bool,
    pub allow_lossy: bool,
}
fn initial_quality() -> f64 {
    0.8
}
fn floor_quality() -> f64 {
    0.5
}
#[derive(Debug, Serialize)]
pub struct OptimizationImage {
    pub object_number: u32,
    pub generation: u16,
    pub original_width: Option<u32>,
    pub original_height: Option<u32>,
    pub output_width: Option<u32>,
    pub output_height: Option<u32>,
    pub skip_reason: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ImageOptimizationReceipt {
    pub status: &'static str,
    pub output: Option<PathBuf>,
    pub input_bytes: u64,
    pub output_bytes: Option<u64>,
    pub source_sha256: String,
    pub output_sha256: Option<String>,
    pub pages: u32,
    pub candidates: Vec<OptimizationImage>,
    pub attempted_quality: Vec<f64>,
    pub selected_quality: Option<f64>,
    pub warnings: Vec<&'static str>,
}
fn schedule(initial: f64, floor: f64, fit: bool, eligible: bool) -> Vec<f64> {
    if !fit || !eligible || initial == floor {
        return vec![initial];
    }
    (0..6)
        .map(|i| {
            if i == 5 {
                floor
            } else {
                initial - (initial - floor) * f64::from(i) / 5.0
            }
        })
        .collect()
}
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        out.push(char::from(ALPHABET[(a >> 2) as usize]));
        out.push(char::from(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(ALPHABET[(c & 63) as usize])
        } else {
            '='
        });
    }
    out
}
fn text_proof(
    input: &Path,
    renderer: &Path,
    geometry: &[pdf_pages::PageGeometry],
    deadline: Instant,
    cancel: &Cancellation,
) -> Result<Vec<String>> {
    let mut result = Vec::with_capacity(geometry.len());
    for geometry in geometry {
        let page = geometry.position - 1;
        let (mut render, _) = pdf_render::command(renderer, cancel)?;
        render
            .arg(input)
            .arg(page.to_string())
            .args(["64", "media"])
            .args(geometry.media_box.iter().map(|v| v.to_string()))
            .arg((geometry.rotation / 90).to_string());
        let raster = native_process::run(render, cancel, remaining(deadline)?, 64 * 64 * 3 + 64)?;
        pdf_render::raster(&raster, 64)?;
        let (mut command, _) = pdf_render::command(renderer, cancel)?;
        command.arg(input).arg(page.to_string()).arg("text");
        let bytes = native_process::run(command, cancel, remaining(deadline)?, 4_000_032)?;
        use sha2::{Digest, Sha256};
        result.push(
            Sha256::digest(pdf_text::decode(&bytes)?.as_bytes())
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect(),
        );
    }
    Ok(result)
}
fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|v| !v.is_zero())
        .map(|v| v.min(Duration::from_secs(60)))
        .ok_or_else(|| {
            fail(
                "limit",
                "PDF image optimization exceeded its ten-minute budget. No output was saved.",
            )
        })
}
pub fn optimize(
    options: &ImageOptimization,
    cancel: &Cancellation,
) -> Result<ImageOptimizationReceipt> {
    if !options.allow_lossy {
        return Err(fail(
            "unsupported",
            "PDF image optimization requires acceptance of lossy image recompression.",
        ));
    }
    if !options.quality.is_finite()
        || !options.minimum_quality.is_finite()
        || !(0.05..=1.0).contains(&options.quality)
        || !(0.05..=options.quality).contains(&options.minimum_quality)
        || options
            .max_dimension
            .is_some_and(|v| !(1..=16384).contains(&v))
        || options
            .max_bytes
            .is_some_and(|v| v == 0 || v > 512 * 1024 * 1024)
        || !options
            .output
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("pdf"))
    {
        return Err(fail(
            "invalid_request",
            "Choose PDF output, quality/floor from 0.05 to 1, and valid pixel/byte limits.",
        ));
    }
    if options.collision == crate::OutputCollision::Fail && options.output.try_exists()? {
        return Err(fail("collision", "The output already exists."));
    }
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut source = Source::open_with_limit(&options.input, cancel.clone(), 512 * 1024 * 1024)?;
    let input_bytes = source.snapshot.as_file().metadata()?.len();
    let info = pdf_inspect::inspect(source.snapshot.path(), &options.directory, cancel)?;
    let original =
        pdf_graph::document_snapshot(source.snapshot.path(), &options.directory, cancel)?;
    let fingerprint = pdf_graph::fingerprint(&original, cancel)?;
    if !fingerprint.special_preservation_keys.is_empty() {
        return Err(fail("unsupported","Image optimization does not support signed, encrypted, interactive, annotated, outlined, tagged or optional-content PDFs."));
    }
    let geometry = pdf_pages::inspect(source.snapshot.path(), &options.directory, cancel)?.pages;
    if geometry.len() != info.pages as usize {
        return Err(fail("verification", "PDF page inspections disagree."));
    }
    let before_text = text_proof(
        source.snapshot.path(),
        &options.renderer_directory,
        &geometry,
        deadline,
        cancel,
    )?;
    let inventory = ImageGraph::load(source.snapshot.path(), &options.directory, cancel)?;
    let images = inventory.discover(0, &(0..info.pages).collect::<Vec<_>>(), cancel)?;
    let masks = inventory.optimization_masks()?;
    let mut candidates = Vec::with_capacity(images.len());
    let mut cumulative = 0u64;
    let working = tempfile::tempdir()?;
    for image in &images {
        let original_candidate = &image.candidate;
        let mut reason = original_candidate.skip_reason.clone();
        let dict = inventory.optimization_dict(&image.reference)?;
        if reason.is_none()
            && (image.soft_mask.is_some()
                || masks.contains(&image.reference)
                || dict
                    .get("/SMask")
                    .is_some_and(|v| v.as_str() != Some("/None")))
        {
            reason = Some("Masked images and images used as masks remain unchanged.".into());
        }
        if reason.is_none() && dict.contains_key("/SMaskInData") {
            reason = Some("Embedded mask semantics remain unchanged.".into());
        }
        let (mut width, mut height) = (original_candidate.width, original_candidate.height);
        if reason.is_none() {
            cumulative += u64::from(width.unwrap_or(0)) * u64::from(height.unwrap_or(0)) * 4;
            if cumulative > 512 * 1024 * 1024 {
                return Err(fail(
                    "limit",
                    "Cumulative decoded image samples exceed 512 MiB.",
                ));
            }
            if let Some(maximum) = options.max_dimension {
                let (w, h) = (
                    width.expect("validated width"),
                    height.expect("validated height"),
                );
                let edge = w.max(h);
                if edge > maximum {
                    width =
                        Some((u64::from(w) * u64::from(maximum) / u64::from(edge)).max(1) as u32);
                    height =
                        Some((u64::from(h) * u64::from(maximum) / u64::from(edge)).max(1) as u32);
                }
            }
            match pdf_lossy_images::decode(
                source.snapshot.path(),
                &options.directory,
                image,
                cancel,
            ) {
                Ok(pixels) => drop(pixels),
                Err(error) if error.code == "unsupported" => {
                    reason = Some(error.message);
                    width = original_candidate.width;
                    height = original_candidate.height;
                }
                Err(error) => return Err(error),
            }
        }
        candidates.push(OptimizationImage {
            object_number: original_candidate.object_number,
            generation: original_candidate.generation,
            original_width: original_candidate.width,
            original_height: original_candidate.height,
            output_width: width,
            output_height: height,
            skip_reason: reason,
        });
    }
    let eligible = candidates.iter().any(|v| v.skip_reason.is_none());
    let mut receipt=ImageOptimizationReceipt{status:"planned",output:None,input_bytes,output_bytes:None,source_sha256:source.hash.clone(),output_sha256:None,pages:info.pages,candidates,attempted_quality:vec![],selected_quality:None,
        warnings:vec!["Only eligible resource image objects are recompressed. All other reachable document objects and streams must match the expected graph. Images may change; page geometry, text and document metadata are retained.","Unsupported image objects remain unchanged; inspect candidate reasons. PDF version may increase to 1.5."]};
    if serde_json::to_vec(&receipt)?.len() > 900_000 {
        return Err(fail(
            "limit",
            "PDF optimization plan exceeds the response limit.",
        ));
    }
    source.check(&options.input)?;
    if options.dry_run {
        return Ok(receipt);
    }
    let parent = options
        .output
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let identity = same_file::Handle::from_path(parent)?;
    for quality in schedule(
        options.quality,
        options.minimum_quality,
        options.max_bytes.is_some(),
        eligible,
    )
    .into_iter()
    {
        cancel.check()?;
        remaining(deadline)?;
        receipt.attempted_quality.push(quality);
        let attempt_dir = tempfile::tempdir_in(working.path())?;
        let mut expected = original.clone();
        let mut updates = Map::new();
        let mut encoded_bytes = 0u64;
        for (index, (image, candidate)) in images.iter().zip(&receipt.candidates).enumerate() {
            if candidate.skip_reason.is_some() {
                continue;
            }
            let destination = attempt_dir.path().join(format!("image-{index}.jpg"));
            // Integer encoder settings round upward, never below the requested floor.
            let setting = (quality * 100.0).ceil().clamp(5.0, 100.0) as u8;
            pdf_lossy_images::encode(
                source.snapshot.path(),
                &options.directory,
                image,
                &destination,
                (
                    candidate.output_width.expect("width"),
                    candidate.output_height.expect("height"),
                ),
                setting,
                cancel,
            )?;
            encoded_bytes += std::fs::metadata(&destination)?.len();
            if encoded_bytes > 128 * 1024 * 1024 {
                return Err(fail("limit", "Replacement JPEG bytes exceed 128 MiB."));
            }
            let mut bytes = Vec::new();
            std::fs::File::open(&destination)?
                .take(128 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            let mut dict = inventory.optimization_dict(&image.reference)?;
            dict.remove("/Length");
            dict.remove("/DecodeParms");
            dict.insert("/Width".into(), Value::from(candidate.output_width));
            dict.insert("/Height".into(), Value::from(candidate.output_height));
            dict.insert("/Filter".into(), Value::from("/DCTDecode"));
            let key = format!("obj:{}", image.reference);
            updates.insert(
                key.clone(),
                serde_json::json!({"stream":{"dict":dict,"datafile":destination}}),
            );
            expected["qpdf"][1][&key] =
                serde_json::json!({"stream":{"dict":dict,"data":base64(&bytes)}});
        }
        let expected = pdf_graph::fingerprint(&expected, cancel)?.graph_sha256;
        let mut job = tempfile::NamedTempFile::new_in(attempt_dir.path())?;
        serde_json::to_writer(
            job.as_file_mut(),
            &serde_json::json!({"qpdf":[{"jsonversion":2},updates]}),
        )?;
        job.flush()?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        let mut command = pdf_inspect::command(&options.directory)?;
        command.args([
            "--compress-streams=y",
            "--decode-level=generalized",
            "--recompress-flate",
            "--compression-level=9",
            "--object-streams=generate",
        ]);
        if eligible {
            let mut argument = std::ffi::OsString::from("--update-from-json=");
            argument.push(job.path());
            command.arg(argument);
        }
        command.arg(source.snapshot.path()).arg(temporary.path());
        native_process::run_with_output_limit(
            command,
            cancel,
            Duration::from_secs(120),
            64 * 1024,
            Some((temporary.path(), 512 * 1024 * 1024)),
        )?;
        let after = pdf_inspect::inspect(temporary.path(), &options.directory, cancel)?;
        let actual = pdf_graph::document_snapshot(temporary.path(), &options.directory, cancel)?;
        if after.pages != info.pages
            || pdf_graph::fingerprint(&actual, cancel)?.graph_sha256 != expected
            || pdf_pages::inspect(temporary.path(), &options.directory, cancel)?.pages != geometry
            || text_proof(
                temporary.path(),
                &options.renderer_directory,
                &geometry,
                deadline,
                cancel,
            )? != before_text
        {
            return Err(fail("verification","PDF did not preserve its exact expected graph, page geometry and text. No output was saved."));
        }
        let (bytes, sha256) = digest(temporary.as_file_mut(), cancel, 512 * 1024 * 1024)?;
        source.check(&options.input)?;
        if options.max_bytes.is_some_and(|limit| bytes > limit) {
            continue;
        }
        receipt.selected_quality = Some(quality);
        if options.max_bytes.is_none() && bytes >= input_bytes {
            receipt.status = "not_smaller";
            return Ok(receipt);
        }
        if identity != same_file::Handle::from_path(parent)? {
            return Err(fail("output_changed", "Output folder changed."));
        }
        temporary.as_file().sync_all()?;
        cancel.check()?;
        let saved_output = crate::output_collision::publish_file(
            temporary,
            &options.output,
            options.collision,
            cancel,
        )?;
        receipt.status = "saved";
        receipt.output = Some(saved_output);
        receipt.output_bytes = Some(bytes);
        receipt.output_sha256 = Some(sha256);
        return Ok(receipt);
    }
    receipt.status = "target_unmet";
    let mut error=fail("target_unmet",format!("The complete PDF did not fit the byte limit at the requested quality floor after {} verified attempts. No output was saved.",receipt.attempted_quality.len()));
    error.pdf_optimization = Some(serde_json::to_value(&receipt)?);
    Err(error)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoding_and_quality_floor_are_exact() {
        for (data, value) in [
            (b"".as_slice(), ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(data), value);
        }
        let qualities = schedule(0.83, 0.505, true, true);
        assert_eq!(qualities.len(), 6);
        assert_eq!(*qualities.last().unwrap(), 0.505);
        assert!(qualities.windows(2).all(|v| v[0] > v[1]));
        assert_eq!(schedule(0.8, 0.5, false, true), vec![0.8]);
        assert_eq!(schedule(0.8, 0.5, true, false), vec![0.8]);
    }
}
