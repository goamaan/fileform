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

mod exif_color;
mod image_color;
mod image_crop;
mod image_fit;
mod image_input;
mod image_resize;
mod jpeg_input;
mod media_audio;
pub use media_audio::SampleRange;
mod media_pack;
mod media_probe;
mod media_timeline;
mod media_video;
pub use media_video::{VideoEncoding, VideoFit};
mod native_process;
pub use image_crop::PixelCrop;
mod image_orientation;
mod image_preview;
mod jpeg_output;
mod tiff_input;
mod tiff_output;
pub use jpeg_output::Background;
mod json_table_reader;
mod png_metadata;
mod png_pipeline;
mod table_reader;
use json_table_reader::JsonTableReader;
use table_reader::TableReader;
pub const MAX_INPUT: u64 = 8 * 1024 * 1024;
const MAX_OUTPUT: u64 = 128 * 1024 * 1024;
const MAX_ROWS: u64 = 100_000;

#[derive(Clone, Default)]
pub struct Cancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
    fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(fail("cancelled", "Cancelled. No new output was saved."))
        } else {
            Ok(())
        }
    }
}
struct CancellableReader<R> {
    inner: R,
    cancellation: Cancellation,
}
impl<R: Read> Read for CancellableReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::other("Cancelled"));
        }
        self.inner.read(buffer)
    }
}

impl<R: Seek> Seek for CancellableReader<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::other("Cancelled"));
        }
        self.inner.seek(position)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    InspectAudioTimeline {
        input: PathBuf,
        directory: PathBuf,
    },
    FitVideo {
        input: PathBuf,
        output: PathBuf,
        directory: PathBuf,
        options: VideoFit,
        expected_source_sha256: Option<String>,
    },
    FitAudio {
        input: PathBuf,
        output: PathBuf,
        directory: PathBuf,
        max_bytes: u64,
        minimum_bitrate: Option<u32>,
        expected_source_sha256: Option<String>,
    },
    ConvertVideo {
        input: PathBuf,
        output: PathBuf,
        directory: PathBuf,
        options: VideoEncoding,
        expected_source_sha256: Option<String>,
    },
    RemuxVideo {
        input: PathBuf,
        output: PathBuf,
        directory: PathBuf,
        expected_source_sha256: Option<String>,
    },
    TrimAudio {
        input: PathBuf,
        output: PathBuf,
        directory: PathBuf,
        samples: SampleRange,
        expected_source_sha256: Option<String>,
    },
    ConvertAudio {
        input: PathBuf,
        output: PathBuf,
        directory: PathBuf,
        expected_source_sha256: Option<String>,
    },
    InspectMedia {
        input: PathBuf,
        directory: PathBuf,
    },
    VerifyMediaPack {
        directory: PathBuf,
    },
    ConvertImage {
        input: PathBuf,
        output: PathBuf,
        background: Option<Background>,
        quality: Option<u8>,
        crop: Option<PixelCrop>,
        max_dimension: Option<u32>,
        max_bytes: Option<u64>,
        minimum_quality: Option<u8>,
        expected_source_sha256: Option<String>,
    },
    InspectImage {
        input: PathBuf,
        preview: Option<bool>,
    },
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
    AudioTimeline(media_timeline::AudioTimeline),
    SavedVideo(media_video::VideoReceipt),
    SavedAudio(media_audio::AudioReceipt),
    MediaInspection(media_probe::MediaInspection),
    MediaPackVerification(media_pack::MediaPackVerification),
    ImageInspection(ImageInspection),
    SavedImage(ImageReceipt),
    Inspection(Inspection),
    Saved(Receipt),
}

#[derive(Debug, Serialize)]
pub struct ImageInspection {
    pub sha256: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub has_icc: bool,
    pub has_exif: bool,
    pub has_color_metadata: bool,
    pub has_hdr_metadata: bool,
    pub decoded_rgba_sha256: String,
    pub orientation: u8,
    pub display_width: u32,
    pub display_height: u32,
    pub oriented_rgba_sha256: String,
    pub icc_srgb_rgba_sha256: Option<String>,
    pub srgb_rgba_sha256: Option<String>,
    pub color_interpretation: String,
    pub conversion_available: bool,
    pub preservation_pending: bool,
    pub preview: Option<image_preview::ImagePreview>,
}
#[derive(Debug, Serialize)]
pub struct ImageReceipt {
    pub output: PathBuf,
    pub attempts: u32,
    pub quality: Option<u8>,
    pub bytes: u64,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
}
fn inspect_image(input: &Path, cancellation: &Cancellation, preview: bool) -> Result<Response> {
    let (_, mut inspection, pixels) = prepare_image(input, cancellation)?;
    if preview && inspection.conversion_available {
        inspection.preview = Some(image_preview::make(&pixels, cancellation)?);
    }
    Ok(Response::ImageInspection(inspection))
}
fn prepare_image(
    input: &Path,
    cancellation: &Cancellation,
) -> Result<(Source, ImageInspection, image::RgbaImage)> {
    let mut source = Source::open_with_limit(input, cancellation.clone(), 512 * 1024 * 1024)?;
    let mut signature = [0u8; 2];
    source.snapshot_reader()?.read_exact(&mut signature)?;
    let decoded = if signature == [0xff, 0xd8] {
        jpeg_input::decode(io::BufReader::new(source.snapshot_reader()?))?
    } else if signature == *b"II" || signature == *b"MM" {
        tiff_input::decode(io::BufReader::new(source.snapshot_reader()?), cancellation)?
    } else {
        png_pipeline::decode(io::BufReader::new(source.snapshot_reader()?))?
    };
    cancellation.check()?;
    let decoded_hash = Sha256::digest(decoded.pixels.as_raw())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let (width, height) = decoded.pixels.dimensions();
    let mut oriented = image_orientation::apply(decoded.pixels, decoded.orientation, cancellation)?;
    let oriented_hash: String = Sha256::digest(oriented.as_raw())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let icc_srgb_rgba_sha256 = if let (false, false, Some(profile)) = (
        decoded.has_cicp || decoded.preservation_pending,
        decoded.has_hdr_metadata,
        decoded.icc_profile.as_ref(),
    ) {
        image_color::normalize_icc(&mut oriented, profile, decoded.source_gray, cancellation)?;
        Some(
            Sha256::digest(oriented.as_raw())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        )
    } else {
        None
    };
    let (srgb_rgba_sha256, color_interpretation) =
        if decoded.has_cicp || decoded.has_hdr_metadata || decoded.preservation_pending {
            (None, "extended_color_pending")
        } else if icc_srgb_rgba_sha256.is_some() {
            (
                icc_srgb_rgba_sha256.clone(),
                decoded.color_override.unwrap_or("icc"),
            )
        } else if decoded.srgb {
            (
                Some(oriented_hash.clone()),
                decoded.color_override.unwrap_or("srgb"),
            )
        } else if decoded.gamma.is_some() || decoded.chromaticities.is_some() {
            let profile = image_color::png_gamma_profile(decoded.gamma, decoded.chromaticities)?;
            image_color::normalize_icc(&mut oriented, &profile, false, cancellation)?;
            (
                Some(
                    Sha256::digest(oriented.as_raw())
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect(),
                ),
                "gamma_chromaticities",
            )
        } else {
            (Some(oriented_hash.clone()), "assumed_srgb")
        };
    let icc_srgb_rgba_sha256 = if decoded.has_icc {
        icc_srgb_rgba_sha256
    } else {
        None
    };
    source.check(input)?;
    let conversion_available = srgb_rgba_sha256.is_some();
    let inspection = ImageInspection {
        sha256: source.hash.clone(),
        bytes: source.input.metadata()?.len(),
        width,
        height,
        orientation: decoded.orientation,
        display_width: oriented.width(),
        display_height: oriented.height(),
        oriented_rgba_sha256: oriented_hash,
        icc_srgb_rgba_sha256,
        srgb_rgba_sha256,
        color_interpretation: color_interpretation.into(),
        has_alpha: decoded.has_alpha,
        has_icc: decoded.has_icc,
        has_exif: decoded.has_exif,
        has_color_metadata: decoded.has_color_metadata,
        has_hdr_metadata: decoded.has_hdr_metadata,
        decoded_rgba_sha256: decoded_hash,
        conversion_available,
        preservation_pending: decoded.preservation_pending,
        preview: None,
    };
    Ok((source, inspection, oriented))
}

struct ImageOptions {
    background: Option<Background>,
    quality: Option<u8>,
    crop: Option<PixelCrop>,
    max_dimension: Option<u32>,
    max_bytes: Option<u64>,
    minimum_quality: Option<u8>,
}
fn convert_image(
    input: &Path,
    output: &Path,
    expected_hash: Option<&str>,
    cancellation: &Cancellation,
    options: ImageOptions,
) -> Result<Response> {
    let ImageOptions {
        background,
        quality,
        crop,
        max_dimension,
        max_bytes,
        minimum_quality,
    } = options;
    if max_dimension == Some(0) {
        return Err(fail(
            "invalid_request",
            "Maximum image dimension must be positive.",
        ));
    }
    let output_kind = match output
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "png",
        Some("jpg" | "jpeg") => "jpeg",
        Some("tif" | "tiff") => "tiff",
        _ => {
            return Err(fail(
                "invalid_output",
                "Choose a .png, .jpg, .jpeg, .tif or .tiff output filename.",
            ))
        }
    };
    let jpeg = output_kind == "jpeg";
    if quality.is_some_and(|value| !(1..=100).contains(&value)) {
        return Err(fail(
            "invalid_request",
            "JPEG quality must be between 1 and 100.",
        ));
    }
    if !jpeg && (quality.is_some() || background.is_some()) {
        return Err(fail(
            "invalid_request",
            "Quality and background options apply to JPEG output.",
        ));
    }
    let qualities = image_fit::qualities(quality.unwrap_or(85), minimum_quality, max_bytes, jpeg)?;
    if output.try_exists()? {
        return Err(fail(
            "collision",
            "The output already exists. Choose another filename.",
        ));
    }
    let (mut source, inspection, pixels) = prepare_image(input, cancellation)?;
    if !inspection.conversion_available {
        return Err(fail(
            "unsupported",
            "This image requires an extended-color preservation workflow.",
        ));
    }
    if expected_hash.is_some_and(|hash| hash != source.hash) {
        return Err(fail(
            "source_changed",
            "The inspected image changed. Add it again.",
        ));
    }
    if jpeg && inspection.has_alpha && background.is_none() {
        return Err(fail(
            "invalid_request",
            "Choose a white or black background for transparent JPEG output.",
        ));
    }
    let pixels = if let Some(crop) = crop {
        image_crop::apply(pixels, crop, cancellation)?
    } else {
        pixels
    };
    let pixels = if let Some(maximum) = max_dimension {
        image_resize::limit(pixels, maximum, cancellation)?
    } else {
        pixels
    };
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = same_file::Handle::from_path(parent)?;
    for (attempt, candidate_quality) in qualities.into_iter().enumerate() {
        let mut temporary = NamedTempFile::new_in(parent)?;
        let mut jpeg_profile = None;
        let mut tiff_profile = None;
        {
            let mut writer = LimitedWriter {
                inner: io::BufWriter::new(temporary.as_file_mut()),
                cancellation: cancellation.clone(),
                bytes: 0,
                maximum_bytes: 512 * 1024 * 1024,
            };
            if jpeg {
                jpeg_profile = Some(jpeg_output::encode(
                    &mut writer,
                    &pixels,
                    background.unwrap_or(Background::White),
                    candidate_quality,
                    cancellation,
                )?);
            } else if output_kind == "tiff" {
                tiff_profile = Some(tiff_output::encode(&mut writer, &pixels, cancellation)?);
            } else {
                let mut encoder = png::Encoder::new(writer, pixels.width(), pixels.height());
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
                let mut writer = encoder
                    .write_header()
                    .map_err(|e| fail("encoding", e.to_string()))?;
                writer
                    .write_image_data(pixels.as_raw())
                    .map_err(|e| fail("encoding", e.to_string()))?;
                writer
                    .finish()
                    .map_err(|e| fail("encoding", e.to_string()))?;
            }
        }
        cancellation.check()?;
        temporary.as_file_mut().seek(SeekFrom::Start(0))?;
        if let Some(profile) = jpeg_profile {
            jpeg_output::verify(
                io::BufReader::new(CancellableReader {
                    inner: temporary.as_file_mut(),
                    cancellation: cancellation.clone(),
                }),
                pixels.width(),
                pixels.height(),
                &profile,
                cancellation,
            )?;
        } else if let Some(profile) = tiff_profile {
            tiff_output::verify(
                CancellableReader {
                    inner: temporary.as_file_mut(),
                    cancellation: cancellation.clone(),
                },
                &pixels,
                &profile,
                cancellation,
            )?;
        } else {
            let checked = png_pipeline::decode(io::BufReader::new(CancellableReader {
                inner: temporary.as_file_mut(),
                cancellation: cancellation.clone(),
            }))?;
            if checked.pixels != pixels || !checked.srgb || checked.orientation != 1 {
                return Err(fail(
                    "verification",
                    "Saved PNG pixels or color metadata differ from the rendered image.",
                ));
            }
        }
        let (bytes, sha256) = digest(temporary.as_file_mut(), cancellation, 512 * 1024 * 1024)?;
        if max_bytes.is_some_and(|limit| bytes > limit) {
            continue;
        }
        source.check(input)?;
        if directory != same_file::Handle::from_path(parent)? {
            return Err(fail("output_changed", "The output folder changed."));
        }
        temporary.as_file().sync_all()?;
        cancellation.check()?;
        temporary.persist_noclobber(output).map_err(|e| {
            fail(
                if e.error.kind() == io::ErrorKind::AlreadyExists {
                    "collision"
                } else {
                    "io"
                },
                e.error.to_string(),
            )
        })?;
        return Ok(Response::SavedImage(ImageReceipt {
            attempts: (attempt + 1) as u32,
            quality: jpeg.then_some(candidate_quality),
            output: output.to_path_buf(),
            bytes,
            sha256,
            width: pixels.width(),
            height: pixels.height(),
        }));
    }
    Err(fail("target_unmet","The complete image could not fit the byte limit within the chosen quality floor. No output was saved."))
}

fn delimiter(path: &Path) -> Result<Option<u8>> {
    match path
        .extension()
        .and_then(|x| x.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => Ok(Some(b',')),
        Some("tsv") => Ok(Some(b'\t')),
        Some("json") => Ok(None),
        _ => Err(fail(
            "unsupported",
            "This worker accepts CSV, TSV and flat JSON tables.",
        )),
    }
}
fn digest(
    file: &mut File,
    cancellation: &Cancellation,
    maximum_bytes: u64,
) -> Result<(u64, String)> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0u8; 65536];
    loop {
        cancellation.check()?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > maximum_bytes {
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
    cancellation: Cancellation,
    input: File,
    snapshot: NamedTempFile,
    identity: same_file::Handle,
    hash: String,
    maximum_bytes: u64,
}
impl Source {
    #[cfg(test)]
    fn open(path: &Path) -> Result<Self> {
        Self::open_cancellable(path, Cancellation::default())
    }
    fn open_cancellable(path: &Path, cancellation: Cancellation) -> Result<Self> {
        Self::open_with_limit(path, cancellation, MAX_INPUT)
    }
    fn open_with_limit(
        path: &Path,
        cancellation: Cancellation,
        maximum_bytes: u64,
    ) -> Result<Self> {
        cancellation.check()?;
        let mut input = File::open(path)?;
        let metadata = input.metadata()?;
        if !metadata.is_file() {
            return Err(fail("invalid_input", "Choose a regular file."));
        }
        if metadata.len() > maximum_bytes {
            return Err(fail(
                "limit",
                format!(
                    "Input exceeds the {} MiB limit.",
                    maximum_bytes / (1024 * 1024)
                ),
            ));
        }
        let identity = same_file::Handle::from_file(input.try_clone()?)?;
        let mut snapshot = NamedTempFile::new()?;
        let copied = io::copy(
            &mut CancellableReader {
                inner: (&mut input).take(maximum_bytes.saturating_add(1)),
                cancellation: cancellation.clone(),
            },
            &mut snapshot,
        )?;
        if copied > maximum_bytes {
            return Err(fail("limit", "Input grew beyond its size limit."));
        }
        let (_, hash) = digest(snapshot.as_file_mut(), &cancellation, maximum_bytes)?;
        let mut source = Self {
            cancellation,
            input,
            snapshot,
            identity,
            hash,
            maximum_bytes,
        };
        source.check(path)?;
        Ok(source)
    }
    fn check(&mut self, path: &Path) -> Result<()> {
        let identity = same_file::Handle::from_path(path)?;
        if self.identity != identity
            || digest(&mut self.input, &self.cancellation, self.maximum_bytes)?.1 != self.hash
        {
            return Err(fail(
                "source_changed",
                "The source changed. Add the file again.",
            ));
        }
        Ok(())
    }
    fn reader(
        &mut self,
        delimiter: Option<u8>,
    ) -> Result<InputReader<CancellableReader<&mut File>>> {
        let input = self.snapshot_reader()?;
        match delimiter {
            Some(separator) => Ok(InputReader::Delimited(TableReader::new(input, separator)?)),
            None => Ok(InputReader::Json(JsonTableReader::new(input)?)),
        }
    }
    fn snapshot_reader(&mut self) -> Result<CancellableReader<&mut File>> {
        self.cancellation.check()?;
        self.snapshot.as_file_mut().seek(SeekFrom::Start(0))?;
        Ok(CancellableReader {
            inner: self.snapshot.as_file_mut(),
            cancellation: self.cancellation.clone(),
        })
    }
}
enum InputReader<R: Read> {
    Delimited(TableReader<R>),
    Json(JsonTableReader<R>),
}
impl<R: Read> InputReader<R> {
    fn next_record(&mut self) -> Result<Option<Vec<String>>> {
        match self {
            Self::Delimited(reader) => reader.next_record(),
            Self::Json(reader) => reader.next_record(),
        }
    }
}
fn headers<R: Read>(reader: &mut InputReader<R>) -> Result<Vec<String>> {
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
    cancellation: Cancellation,
    inner: W,
    bytes: u64,
    maximum_bytes: u64,
}
impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::other("Cancelled"));
        }
        if self.bytes + bytes.len() as u64 > self.maximum_bytes {
            return Err(io::Error::other("Output exceeds its size limit."));
        }
        let n = self.inner.write(bytes)?;
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write + Seek> Seek for LimitedWriter<W> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::other("Cancelled"));
        }
        let position = self.inner.seek(position)?;
        if position > self.maximum_bytes {
            return Err(io::Error::other("Output seek exceeds its size limit."));
        }
        Ok(position)
    }
}

pub fn execute(request: Request) -> Result<Response> {
    execute_with_cancellation(request, Cancellation::default())
}
pub fn execute_with_cancellation(request: Request, cancellation: Cancellation) -> Result<Response> {
    let result = execute_inner(request, &cancellation);
    if result.is_err() && cancellation.is_cancelled() {
        cancellation.check()?;
    }
    result
}
fn execute_inner(request: Request, cancellation: &Cancellation) -> Result<Response> {
    cancellation.check()?;
    if let Request::InspectAudioTimeline { input, directory } = &request {
        return media_timeline::inspect(input, directory, cancellation)
            .map(Response::AudioTimeline);
    }
    if let Request::FitVideo {
        input,
        output,
        directory,
        options,
        expected_source_sha256,
    } = &request
    {
        return media_video::fit(
            input,
            output,
            directory,
            *options,
            expected_source_sha256.as_deref(),
            cancellation,
        )
        .map(Response::SavedVideo);
    }
    if let Request::FitAudio {
        input,
        output,
        directory,
        max_bytes,
        minimum_bitrate,
        expected_source_sha256,
    } = &request
    {
        return media_audio::fit(
            input,
            output,
            directory,
            *max_bytes,
            *minimum_bitrate,
            expected_source_sha256.as_deref(),
            cancellation,
        )
        .map(Response::SavedAudio);
    }
    if let Request::ConvertVideo {
        input,
        output,
        directory,
        options,
        expected_source_sha256,
    } = &request
    {
        return media_video::transform(
            input,
            output,
            directory,
            expected_source_sha256.as_deref(),
            Some(*options),
            false,
            cancellation,
        )
        .map(Response::SavedVideo);
    }
    if let Request::RemuxVideo {
        input,
        output,
        directory,
        expected_source_sha256,
    } = &request
    {
        return media_video::transform(
            input,
            output,
            directory,
            expected_source_sha256.as_deref(),
            None,
            false,
            cancellation,
        )
        .map(Response::SavedVideo);
    }
    if let Request::TrimAudio {
        input,
        output,
        directory,
        samples,
        expected_source_sha256,
    } = &request
    {
        return media_audio::convert(
            input,
            output,
            directory,
            expected_source_sha256.as_deref(),
            Some(*samples),
            None,
            cancellation,
        )
        .map(Response::SavedAudio);
    }
    if let Request::ConvertAudio {
        input,
        output,
        directory,
        expected_source_sha256,
    } = &request
    {
        return media_audio::convert(
            input,
            output,
            directory,
            expected_source_sha256.as_deref(),
            None,
            None,
            cancellation,
        )
        .map(Response::SavedAudio);
    }
    if let Request::InspectMedia { input, directory } = &request {
        return media_probe::inspect(input, directory, cancellation).map(Response::MediaInspection);
    }
    if let Request::VerifyMediaPack { directory } = &request {
        return media_pack::verify(directory, cancellation).map(Response::MediaPackVerification);
    }
    if let Request::ConvertImage {
        input,
        output,
        expected_source_sha256,
        background,
        quality,
        crop,
        max_dimension,
        max_bytes,
        minimum_quality,
    } = &request
    {
        return convert_image(
            input,
            output,
            expected_source_sha256.as_deref(),
            cancellation,
            ImageOptions {
                background: *background,
                quality: *quality,
                crop: *crop,
                max_dimension: *max_dimension,
                max_bytes: *max_bytes,
                minimum_quality: *minimum_quality,
            },
        );
    }
    if let Request::InspectImage { input, preview } = &request {
        return inspect_image(input, cancellation, preview.unwrap_or(false));
    }
    let input = match &request {
        Request::InspectAudioTimeline { .. }
        | Request::FitVideo { .. }
        | Request::FitAudio { .. }
        | Request::ConvertVideo { .. }
        | Request::RemuxVideo { .. }
        | Request::TrimAudio { .. }
        | Request::ConvertAudio { .. }
        | Request::InspectMedia { .. }
        | Request::VerifyMediaPack { .. } => {
            unreachable!("handled above")
        }
        Request::ConvertImage { input, .. }
        | Request::InspectImage { input, .. }
        | Request::Inspect { input }
        | Request::ConvertTable { input, .. } => input,
    };
    let separator = delimiter(input)?;
    let mut source = Source::open_cancellable(input, cancellation.clone())?;
    match request {
        Request::InspectAudioTimeline { .. }
        | Request::FitVideo { .. }
        | Request::FitAudio { .. }
        | Request::ConvertVideo { .. }
        | Request::RemuxVideo { .. }
        | Request::TrimAudio { .. }
        | Request::ConvertAudio { .. }
        | Request::InspectMedia { .. }
        | Request::VerifyMediaPack { .. } => {
            unreachable!("handled above")
        }
        Request::ConvertImage { .. } | Request::InspectImage { .. } => {
            unreachable!("handled above")
        }
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
                outputs: if separator.is_none() {
                    vec!["csv", "tsv"]
                } else {
                    vec!["json", "csv", "tsv"]
                },
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
            if separator.is_none() && output_separator.is_none() {
                return Err(fail(
                    "invalid_output",
                    "Choose CSV or TSV for JSON input. JSON scalar types become text cells.",
                ));
            }
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
                    maximum_bytes: MAX_OUTPUT,
                    cancellation: cancellation.clone(),
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
            let (bytes, sha256) = digest(temporary.as_file_mut(), cancellation, MAX_OUTPUT)?;
            source.check(&input)?;
            if directory != same_file::Handle::from_path(parent)? {
                return Err(fail("output_changed", "The output folder changed."));
            }
            temporary.as_file().sync_all()?;
            cancellation.check()?;
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
        let quote = (index == 0 && cell.starts_with('\u{feff}'))
            || cell
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
    separator: Option<u8>,
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
    separator: Option<u8>,
    output: &mut File,
    expected_rows: u64,
) -> Result<()> {
    use serde::de::{self, SeqAccess, Visitor};
    struct Rows<'a> {
        reader: InputReader<CancellableReader<&'a mut File>>,
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
            verify_delimited(&mut source, Some(b','), output.as_file_mut(), b'\t', 1)
                .unwrap_err()
                .code,
            "verification"
        );
    }
    #[test]
    fn flat_json_preserves_numeric_lexemes_and_aligns_reordered_keys() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.json");
        let output = dir.path().join("result.csv");
        let text = "\u{feff}[{\"large\":9007199254740993,\"decimal\":-0.001200e+999,\"text\":\"日本\\n語\",\"flag\":true,\"empty\":null},{\"empty\":\"\",\"text\":\"001\",\"flag\":false,\"decimal\":-0,\"large\":123456789012345678901234567890}]";
        std::fs::write(&input, text).unwrap();
        let Response::Inspection(info) = execute(Request::Inspect {
            input: input.clone(),
        })
        .unwrap() else {
            panic!("inspection expected")
        };
        assert_eq!(info.rows, 2);
        assert_eq!(info.outputs, ["csv", "tsv"]);
        execute(Request::ConvertTable {
            input: input.clone(),
            output: output.clone(),
            expected_source_sha256: Some(info.sha256),
        })
        .unwrap();
        let mut reader = TableReader::new(File::open(output).unwrap(), b',').unwrap();
        assert_eq!(
            reader.next_record().unwrap().unwrap(),
            ["large", "decimal", "text", "flag", "empty"]
        );
        assert_eq!(
            reader.next_record().unwrap().unwrap(),
            ["9007199254740993", "-0.001200e+999", "日本\n語", "true", ""]
        );
        assert_eq!(
            reader.next_record().unwrap().unwrap(),
            ["123456789012345678901234567890", "-0", "001", "false", ""]
        );
        assert!(reader.next_record().unwrap().is_none());
        assert_eq!(std::fs::read_to_string(input).unwrap(), text);
    }
    #[test]
    fn json_bom_in_a_column_name_is_not_lost_as_file_encoding_marker() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.json");
        let output = dir.path().join("result.tsv");
        std::fs::write(
            &input,
            serde_json::to_vec(&serde_json::json!([{ "\u{feff}name": "value" }])).unwrap(),
        )
        .unwrap();
        execute(Request::ConvertTable {
            input,
            output: output.clone(),
            expected_source_sha256: None,
        })
        .unwrap();
        let mut reader = TableReader::new(File::open(output).unwrap(), b'\t').unwrap();
        assert_eq!(reader.next_record().unwrap().unwrap(), ["\u{feff}name"]);
    }
    #[test]
    fn invalid_json_tables_never_publish() {
        for text in [
            r#"[]"#,
            r#"[{}]"#,
            r#"[{"a":1,"a":2}]"#,
            r#"[{"a":1,"\u0061":2}]"#,
            r#"[{"a":1},{"b":2}]"#,
            r#"[{"a":[]}]"#,
            r#"[{"a":{}}]"#,
            r#"[{"a":01}]"#,
            r#"[{"a":1.}]"#,
            r#"[{"a":1e}]"#,
            r#"[{"a":+1}]"#,
            r#"[{"a":NaN}]"#,
            r#"[{"a":true},]"#,
            r#"[{"a":false,}]"#,
            r#"[{"a":null}] trailing"#,
            r#"[{"a":"\ud800"}]"#,
            r#"[{" ":1}]"#,
            r#"[{"a":1},{"a":2,"b":3}]"#,
            r#"[{"a":"unterminated}]"#,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let input = dir.path().join("source.json");
            let output = dir.path().join("result.tsv");
            std::fs::write(&input, text).unwrap();
            assert!(
                execute(Request::ConvertTable {
                    input: input.clone(),
                    output: output.clone(),
                    expected_source_sha256: None
                })
                .is_err(),
                "accepted {text}"
            );
            assert!(!output.exists());
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
            assert_eq!(std::fs::read_to_string(input).unwrap(), text);
        }
    }
    #[test]
    fn source_policy_applies_to_open_and_later_growth() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.bin");
        std::fs::write(&input, b"12345678").unwrap();
        assert!(Source::open_with_limit(&input, Cancellation::default(), 7).is_err());
        let mut source = Source::open_with_limit(&input, Cancellation::default(), 8).unwrap();
        std::fs::write(&input, b"123456789").unwrap();
        assert_eq!(source.check(&input).unwrap_err().code, "limit");
    }
    #[test]
    fn snapshot_seeks_are_independent_of_original_and_cancellable() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.bin");
        std::fs::write(&input, b"original").unwrap();
        let cancellation = Cancellation::default();
        let mut source = Source::open_with_limit(&input, cancellation.clone(), 8).unwrap();
        std::fs::write(&input, b"replaced").unwrap();
        let mut reader = source.snapshot_reader().unwrap();
        reader.seek(SeekFrom::Start(4)).unwrap();
        let mut suffix = String::new();
        reader.read_to_string(&mut suffix).unwrap();
        assert_eq!(suffix, "inal");
        cancellation.cancel();
        assert!(reader.seek(SeekFrom::Start(0)).is_err());
        assert!(reader.read(&mut [0u8; 1]).is_err());
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
