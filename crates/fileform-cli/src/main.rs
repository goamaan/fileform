// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use fileform_engine::{
    Background, MediaInterval, MediaTime, PdfComposition, PdfRenderBox, PdfSplit, PdfTextExport,
    PixelCrop, Request, SampleRange, VideoEncoding, VideoFit, VideoTrimOptions,
};
use std::path::PathBuf;
fn main() {
    let raw: Vec<_> = std::env::args_os().skip(1).collect();
    let (args, collision) = collision_argument(raw);
    let (args, audio_stream) = audio_stream_argument(args);
    let mut request = match args.as_slice() {
        [command, input, output, directory, renderer, tail @ ..]
            if command == "optimize-pdf-images" =>
        {
            let mut options = fileform_engine::PdfImageOptimization {
                collision: fileform_engine::OutputCollision::Fail,
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                quality: 0.8,
                minimum_quality: 0.5,
                max_dimension: None,
                max_bytes: None,
                dry_run: false,
                allow_lossy: true,
            };
            let mut index = 0;
            let mut seen = std::collections::BTreeSet::new();
            while index < tail.len() {
                let flag = tail[index].to_str().unwrap_or("");
                if !seen.insert(flag) {
                    eprintln!("Repeated PDF optimization option.");
                    std::process::exit(2);
                }
                if flag == "--dry-run" {
                    options.dry_run = true;
                    index += 1;
                    continue;
                }
                let value = tail
                    .get(index + 1)
                    .and_then(|v| v.to_str())
                    .unwrap_or_else(|| {
                        eprintln!("PDF optimization option needs a value.");
                        std::process::exit(2)
                    });
                match flag {
                    "--quality" => options.quality = value.parse().unwrap_or(f64::NAN),
                    "--minimum-quality" => {
                        options.minimum_quality = value.parse().unwrap_or(f64::NAN)
                    }
                    "--max-dimension" => options.max_dimension = Some(value.parse().unwrap_or(0)),
                    "--max-bytes" => options.max_bytes = Some(value.parse().unwrap_or(0)),
                    _ => {
                        eprintln!("Unknown PDF optimization option.");
                        std::process::exit(2)
                    }
                }
                index += 2;
            }
            Request::OptimizePdfImages(options)
        }
        [command, output, directory, inputs @ ..] if command == "extract-pdf-images" => {
            let (inputs, page_ranges, _) = page_arguments(inputs, false);
            Request::ExtractPdfImages(fileform_engine::PdfImageExtraction {
                collision: fileform_engine::OutputCollision::Fail,
                inputs,
                output: Some(output.into()),
                directory: directory.into(),
                pages: None,
                page_ranges,
                dry_run: false,
            })
        }
        [command, directory, inputs @ ..] if command == "plan-pdf-images" => {
            let (inputs, page_ranges, _) = page_arguments(inputs, false);
            Request::ExtractPdfImages(fileform_engine::PdfImageExtraction {
                collision: fileform_engine::OutputCollision::Fail,
                inputs,
                output: None,
                directory: directory.into(),
                pages: None,
                page_ranges,
                dry_run: true,
            })
        }
        [command, output, directory, renderer, format, dpi, inputs @ ..]
            if command == "export-pdf-images" =>
        {
            let format = match format.to_str() {
                Some("png") => fileform_engine::PdfRasterFormat::Png,
                Some("jpeg") => fileform_engine::PdfRasterFormat::Jpeg,
                _ => {
                    eprintln!("Choose png or jpeg.");
                    std::process::exit(2)
                }
            };
            let dpi = dpi
                .to_str()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose 36–600 DPI.");
                    std::process::exit(2)
                });
            let (inputs, page_ranges, quality) = page_arguments(inputs, true);
            Request::ExportPdfImages(fileform_engine::PdfRasterBatch {
                collision: fileform_engine::OutputCollision::Fail,
                inputs,
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                pages: None,
                page_ranges,
                dpi,
                format,
                quality,
                allow_rasterization: true,
            })
        }
        [command, input, output, directory, renderer, page, dpi, tail @ ..]
            if command == "export-pdf-jpeg" =>
        {
            let page_index = page
                .to_str()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose a zero-based page index.");
                    std::process::exit(2)
                });
            let dpi = dpi
                .to_str()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose 36–600 DPI.");
                    std::process::exit(2)
                });
            let quality = match tail {
                [] => None,
                [value] => Some(
                    value
                        .to_str()
                        .and_then(|v| v.parse::<u8>().ok())
                        .unwrap_or_else(|| {
                            eprintln!("Choose JPEG quality from 5 to 100.");
                            std::process::exit(2)
                        }),
                ),
                _ => {
                    eprintln!("Too many JPEG options.");
                    std::process::exit(2)
                }
            };
            Request::ExportPdfJpeg(fileform_engine::PdfJpegExport {
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                page_index,
                dpi,
                quality,
            })
        }
        [command, input, output, directory, renderer, page, dpi] if command == "export-pdf-png" => {
            let page_index = page
                .to_str()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose a zero-based page index.");
                    std::process::exit(2)
                });
            let dpi = dpi
                .to_str()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose 36–600 DPI.");
                    std::process::exit(2)
                });
            Request::ExportPdfPng(fileform_engine::PdfPngExport {
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                page_index,
                dpi,
                quality: None,
            })
        }
        [command, input, directory, dpi] if command == "plan-pdf-raster" => {
            let dpi = dpi
                .to_str()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose an integer DPI from 36 to 600.");
                    std::process::exit(2)
                });
            Request::PlanPdfRaster {
                input: input.into(),
                directory: directory.into(),
                dpi,
            }
        }
        [command, input, output, directory, language]
            if command == "ocr-image" && language == "eng" =>
        {
            Request::OcrImage {
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                language: fileform_engine::OcrLanguage::Eng,
            }
        }
        [command, directory] if command == "verify-ocr-pack" => Request::VerifyOcrPack {
            directory: directory.into(),
        },
        [command, input, output, directory, renderer, options @ ..]
            if command == "export-pdf-text" =>
        {
            let (allow_missing_text, ocr) = match options {
                [] => (false, None),
                [flag] if flag == "--allow-missing-text" => (true, None),
                [flag, directory, language] if flag == "--ocr" && language == "eng" => (
                    false,
                    Some(fileform_engine::PdfOcrOptions {
                        directory: directory.into(),
                        language: fileform_engine::OcrLanguage::Eng,
                    }),
                ),
                _ => {
                    eprintln!("Expected --allow-missing-text or --ocr OCR_PACK eng.");
                    std::process::exit(2)
                }
            };
            Request::ExportPdfText(PdfTextExport {
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                allow_missing_text,
                ocr,
            })
        }
        [command, output, directory, renderer, inputs @ ..] if command == "split-pdf" => {
            let (inputs, selection, dry_run) = split_arguments(inputs);
            Request::SplitPdf(PdfSplit {
                collision: fileform_engine::OutputCollision::Fail,
                inputs,
                selection,
                dry_run,
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                groups: None,
                allow_document_changes: true,
            })
        }
        [command, output, directory, renderer, inputs @ ..] if command == "merge-pdf" => {
            let (inputs, page_ranges, _) = page_arguments(inputs, false);
            Request::ComposePdf(PdfComposition {
                collision: fileform_engine::OutputCollision::Fail,
                inputs,
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                pages: None,
                page_ranges,
                allow_document_changes: true,
            })
        }
        [command, input, output, directory, renderer, options @ ..]
            if command == "compress-pdf" =>
        {
            let max_bytes = match options {
                [] => None,
                [flag, value] if flag == "--max-bytes" => Some(
                    value
                        .to_str()
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or_else(|| {
                            eprintln!("Choose a positive byte limit.");
                            std::process::exit(2)
                        }),
                ),
                _ => {
                    eprintln!("Expected --max-bytes BYTES or no compression option.");
                    std::process::exit(2)
                }
            };
            Request::OptimizePdf {
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                renderer_directory: renderer.into(),
                max_bytes,
            }
        }
        [command, input, output, directory, page] if command == "extract-pdf-text" => {
            let page_index = page
                .to_str()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose a zero-based PDF page index.");
                    std::process::exit(2)
                });
            Request::ExtractPdfText {
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                page_index,
            }
        }
        [command, input, output, directory, page, options @ ..] if command == "render-pdf-page" => {
            let page_index = page
                .to_str()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Choose a zero-based PDF page index.");
                    std::process::exit(2)
                });
            let page_box = match options {
                [] => PdfRenderBox::Crop,
                [flag] if flag == "--media-box" => PdfRenderBox::Media,
                _ => {
                    eprintln!("Expected --media-box or no render option.");
                    std::process::exit(2)
                }
            };
            Request::RenderPdfPage {
                input: input.into(),
                output: output.into(),
                directory: directory.into(),
                page_index,
                page_box: Some(page_box),
                max_dimension: None,
            }
        }
        [command, input, directory] if command == "inspect-pdf-pages" => Request::InspectPdfPages {
            input: PathBuf::from(input),
            directory: PathBuf::from(directory),
        },
        [command, input, directory] if command == "inspect-pdf-graph" => Request::InspectPdfGraph {
            input: PathBuf::from(input),
            directory: PathBuf::from(directory),
        },
        [command, directory] if command == "verify-pdf-pack" => Request::VerifyPdfPack {
            directory: PathBuf::from(directory),
        },
        [command, input, directory] if command == "inspect-pdf" => Request::InspectPdf {
            input: PathBuf::from(input),
            directory: PathBuf::from(directory),
        },

        [command, input, output, directory, time] if command == "poster" => {
            let time = MediaTime::decimal(time.to_str().unwrap_or("")).unwrap_or_else(|e| {
                eprintln!("{}", e.message);
                std::process::exit(2)
            });
            Request::MediaPoster {
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                time,
                max_dimension: None,
            }
        }
        [command, input, directory] if command == "waveform" => Request::MediaWaveform {
            audio_stream: None,
            input: PathBuf::from(input),
            directory: PathBuf::from(directory),
            bins: None,
        },

        [command, input, output, directory, start, end] if command == "copy-audio-trim" => {
            let time = |v: &std::ffi::OsString| {
                MediaTime::decimal(v.to_str().unwrap_or("")).unwrap_or_else(|e| {
                    eprintln!("{}", e.message);
                    std::process::exit(2)
                })
            };
            Request::CopyAudioTrim {
                audio_stream: None,
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                interval: MediaInterval {
                    start: time(start),
                    end: time(end),
                },
                expected_source_sha256: None,
            }
        }
        [command, input, directory, index] if command == "inspect-media-packets" => {
            let stream_index = index
                .to_str()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Stream index must be a non-negative integer.");
                    std::process::exit(2)
                });
            Request::InspectMediaPackets {
                input: PathBuf::from(input),
                directory: PathBuf::from(directory),
                stream_index,
            }
        }
        [command, input, output, directory, start, end, flags @ ..]
            if command == "copy-video-trim" =>
        {
            let mute_audio = match flags {
                [] => false,
                [flag] if flag == "--mute-audio" => true,
                _ => {
                    eprintln!("Use --mute-audio or no extra flags.");
                    std::process::exit(2)
                }
            };
            let time = |v: &std::ffi::OsString| {
                MediaTime::decimal(v.to_str().unwrap_or("")).unwrap_or_else(|e| {
                    eprintln!("{}", e.message);
                    std::process::exit(2)
                })
            };
            Request::CopyVideoTrim {
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                options: VideoTrimOptions {
                    audio_stream: None,
                    interval: MediaInterval {
                        start: time(start),
                        end: time(end),
                    },
                    mute_audio,
                },
                expected_source_sha256: None,
            }
        }
        [command, input, output, directory, start, end, flags @ ..] if command == "trim-video" => {
            let mute_audio = match flags {
                [] => false,
                [flag] if flag == "--mute-audio" => true,
                _ => {
                    eprintln!("Use --mute-audio or no extra flags.");
                    std::process::exit(2)
                }
            };
            let time = |v: &std::ffi::OsString| {
                MediaTime::decimal(v.to_str().unwrap_or("")).unwrap_or_else(|e| {
                    eprintln!("{}", e.message);
                    std::process::exit(2)
                })
            };
            Request::TrimVideo {
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                options: VideoTrimOptions {
                    audio_stream: None,
                    interval: MediaInterval {
                        start: time(start),
                        end: time(end),
                    },
                    mute_audio,
                },
                expected_source_sha256: None,
            }
        }
        [command, input, directory] if command == "inspect-video-timeline" => {
            Request::InspectVideoTimeline {
                input: PathBuf::from(input),
                directory: PathBuf::from(directory),
            }
        }
        [command, input, output, directory, start, end] if command == "trim-audio-time" => {
            let time = |value: &std::ffi::OsString| {
                MediaTime::decimal(value.to_str().unwrap_or("")).unwrap_or_else(|e| {
                    eprintln!("{}", e.message);
                    std::process::exit(2)
                })
            };
            Request::TrimAudioTime {
                audio_stream: None,
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                interval: MediaInterval {
                    start: time(start),
                    end: time(end),
                },
                expected_source_sha256: None,
            }
        }
        [command, input, directory] if command == "inspect-audio-timeline" => {
            Request::InspectAudioTimeline {
                audio_stream: None,
                input: PathBuf::from(input),
                directory: PathBuf::from(directory),
            }
        }
        [command, input, output, directory, limit] if command == "fit-video" => {
            let max_bytes = limit
                .to_str()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Byte limit must be a positive integer.");
                    std::process::exit(2)
                });
            Request::FitVideo {
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                options: VideoFit {
                    audio_stream: None,
                    max_bytes,
                    minimum_bitrate: None,
                    max_dimension: None,
                },
                expected_source_sha256: None,
            }
        }
        [command, input, output, directory, limit] if command == "fit-audio" => {
            let bytes = limit
                .to_str()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or_else(|| {
                    eprintln!("Byte limit must be a positive integer.");
                    std::process::exit(2)
                });
            Request::FitAudio {
                audio_stream: None,
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                max_bytes: bytes,
                minimum_bitrate: None,
                expected_source_sha256: None,
            }
        }
        [command, input, output, directory, options @ ..] if command == "convert-video" => {
            let maximum = match options {
                [] => None,
                [flag, value] if flag == "--max-dimension" => Some(
                    value
                        .to_str()
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or_else(|| {
                            eprintln!("Video dimension must be an integer.");
                            std::process::exit(2)
                        }),
                ),
                _ => {
                    eprintln!("Use --max-dimension PIXELS.");
                    std::process::exit(2)
                }
            };
            Request::ConvertVideo {
                audio_stream: None,
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                options: VideoEncoding {
                    max_dimension: maximum,
                    bitrate: None,
                },
                expected_source_sha256: None,
            }
        }
        [command, input, output, directory] if command == "remux-video" => Request::RemuxVideo {
            audio_stream: None,
            input: PathBuf::from(input),
            output: PathBuf::from(output),
            directory: PathBuf::from(directory),
            expected_source_sha256: None,
        },
        [command, input, output, directory, start, end] if command == "trim-audio" => {
            let parse = |value: &std::ffi::OsString| {
                value
                    .to_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or_else(|| {
                        eprintln!("Trim boundaries must be non-negative integer samples.");
                        std::process::exit(2)
                    })
            };
            Request::TrimAudio {
                audio_stream: None,
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                samples: SampleRange {
                    start: parse(start),
                    end: parse(end),
                },
                expected_source_sha256: None,
            }
        }
        [command, input, output, directory] if command == "convert-audio" => {
            Request::ConvertAudio {
                audio_stream: None,
                input: PathBuf::from(input),
                output: PathBuf::from(output),
                directory: PathBuf::from(directory),
                expected_source_sha256: None,
            }
        }
        [command, input, directory] if command == "inspect-media" => Request::InspectMedia {
            input: PathBuf::from(input),
            directory: PathBuf::from(directory),
        },
        [command, directory] if command == "verify-media-pack" => Request::VerifyMediaPack {
            directory: PathBuf::from(directory),
        },
        [command, rest @ ..] if command == "convert-image" => {
            image_request(rest).unwrap_or_else(|message| {
                eprintln!("{message}");
                std::process::exit(2);
            })
        }
        [command, input] if command == "inspect-image" => Request::InspectImage {
            input: PathBuf::from(input),
            preview: None,
        },
        [command, input] if command == "inspect" => Request::Inspect {
            input: PathBuf::from(input),
        },
        [command, input, output] if command == "convert-table" => Request::ConvertTable {
            input: PathBuf::from(input),
            output: PathBuf::from(output),
            expected_source_sha256: None,
        },
        _ => {
            eprintln!(
                "Usage: fileform-native optimize-pdf-images INPUT OUTPUT.pdf PDF_PACK RENDER_PACK [--quality 0.8] [--minimum-quality 0.5] [--max-dimension PIXELS] [--max-bytes BYTES] [--dry-run] | extract-pdf-images FOLDER PDF_PACK INPUT... [--pages RANGES] [--collision fail|rename] | plan-pdf-images PDF_PACK INPUT... [--pages RANGES] | export-pdf-images FOLDER PDF_PACK RENDER_PACK png|jpeg DPI INPUT... [--pages RANGES] [--quality 5-100] [--collision fail|rename] | export-pdf-jpeg INPUT OUTPUT.jpg PDF_PACK RENDER_PACK PAGE DPI [QUALITY] | export-pdf-png INPUT OUTPUT.png PDF_PACK RENDER_PACK PAGE DPI | plan-pdf-raster INPUT PDF_PACK DPI | ocr-image INPUT OUTPUT.txt OCR_PACK eng | verify-ocr-pack PACK | export-pdf-text INPUT OUTPUT.txt PDF_PACK RENDER_PACK [--allow-missing-text | --ocr OCR_PACK eng] | split-pdf OUTPUT_FOLDER PDF_PACK RENDER_PACK INPUT... [--every COUNT | --ranges GROUPS | --after POSITIONS] [--dry-run] | merge-pdf OUTPUT.pdf PDF_PACK RENDER_PACK INPUT... [--pages RANGES] [--collision fail|rename] | compress-pdf INPUT OUTPUT.pdf PDF_PACK RENDER_PACK [--max-bytes BYTES] | extract-pdf-text INPUT OUTPUT.txt RENDER_PACK PAGE_INDEX | render-pdf-page INPUT OUTPUT.png RENDER_PACK PAGE_INDEX [--media-box] | inspect-pdf-pages FILE PACK_DIRECTORY | inspect-pdf-graph FILE PACK_DIRECTORY | inspect-pdf FILE PACK_DIRECTORY | verify-pdf-pack PACK_DIRECTORY | poster INPUT OUTPUT.png PACK_DIRECTORY SECONDS | waveform FILE PACK_DIRECTORY | copy-video-trim INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY START_SECONDS END_SECONDS [--mute-audio] | copy-audio-trim INPUT OUTPUT.m4a PACK_DIRECTORY START_SECONDS END_SECONDS | inspect-media-packets FILE PACK_DIRECTORY STREAM_INDEX | trim-video INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY START_SECONDS END_SECONDS [--mute-audio] | inspect-video-timeline FILE PACK_DIRECTORY | trim-audio-time INPUT OUTPUT.{{wav,flac,m4a}} PACK_DIRECTORY START_SECONDS END_SECONDS | inspect-audio-timeline FILE PACK_DIRECTORY | fit-video INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY BYTES | fit-audio INPUT OUTPUT.{{wav,flac,m4a,mp3}} PACK_DIRECTORY BYTES | convert-video INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY [--max-dimension PIXELS] | remux-video INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY | trim-audio INPUT OUTPUT.{{wav,flac,m4a}} PACK_DIRECTORY START_SAMPLE END_SAMPLE | convert-audio INPUT OUTPUT.{{wav,flac,m4a,mp3}} PACK_DIRECTORY | inspect-media FILE PACK_DIRECTORY | verify-media-pack DIRECTORY | inspect FILE | inspect-image FILE | convert-image INPUT OUTPUT.{{png,jpg,tiff}} [--background white|black] [--quality 1-100] [--crop x,y,width,height] [--max-dimension pixels] [--max-bytes bytes] [--minimum-quality 1-100] | convert-table INPUT OUTPUT.{{json,csv,tsv}}"
            );
            std::process::exit(2);
        }
    };
    if let Some(index) = audio_stream {
        match &mut request {
            Request::MediaWaveform { audio_stream, .. }
            | Request::CopyAudioTrim { audio_stream, .. }
            | Request::TrimAudioTime { audio_stream, .. }
            | Request::InspectAudioTimeline { audio_stream, .. }
            | Request::FitAudio { audio_stream, .. }
            | Request::TrimAudio { audio_stream, .. }
            | Request::ConvertAudio { audio_stream, .. }
            | Request::ConvertVideo { audio_stream, .. }
            | Request::RemuxVideo { audio_stream, .. } => *audio_stream = Some(index),
            Request::CopyVideoTrim { options, .. } | Request::TrimVideo { options, .. } => {
                options.audio_stream = Some(index)
            }
            Request::FitVideo { options, .. } => options.audio_stream = Some(index),
            _ => argument_error("This route does not yet accept audio track selection."),
        }
    }
    if let Some(policy) = collision {
        match &mut request {
            Request::OptimizePdfImages(options) => options.collision = policy,
            Request::ExtractPdfImages(options) => options.collision = policy,
            Request::ExportPdfImages(options) => options.collision = policy,
            Request::SplitPdf(options) => options.collision = policy,
            Request::ComposePdf(options) => options.collision = policy,
            _ => argument_error("This route does not yet accept a collision policy."),
        }
    }
    match fileform_engine::execute(request) {
        Ok(result) => println!(
            "{}",
            serde_json::to_string(&result).expect("serializable receipt")
        ),
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::to_string(&error).expect("serializable error")
            );
            std::process::exit(1);
        }
    }
}

fn image_request(args: &[std::ffi::OsString]) -> Result<Request, &'static str> {
    let [input, output, options @ ..] = args else {
        return Err("convert-image requires input and output paths.");
    };
    if options.len() % 2 != 0 {
        return Err("Each image option requires a value.");
    }
    let mut background = None;
    let mut quality = None;
    let mut crop = None;
    let mut max_dimension = None;
    let mut max_bytes = None;
    let mut minimum_quality = None;
    for option in options.as_chunks::<2>().0 {
        match option[0].to_str() {
            Some("--background") if background.is_none() => {
                background = Some(match option[1].to_str() {
                    Some("white") => Background::White,
                    Some("black") => Background::Black,
                    _ => return Err("Background must be white or black."),
                })
            }
            Some("--max-bytes") if max_bytes.is_none() => {
                max_bytes = Some(
                    option[1]
                        .to_str()
                        .and_then(|s| s.parse::<u64>().ok())
                        .filter(|v| *v > 0)
                        .ok_or("Byte limit must be a positive integer.")?,
                )
            }
            Some("--minimum-quality") if minimum_quality.is_none() => {
                minimum_quality = Some(
                    option[1]
                        .to_str()
                        .and_then(|s| s.parse::<u8>().ok())
                        .filter(|v| (1..=100).contains(v))
                        .ok_or("Minimum quality must be from 1 to 100.")?,
                )
            }
            Some("--max-dimension") if max_dimension.is_none() => {
                max_dimension = Some(
                    option[1]
                        .to_str()
                        .and_then(|s| s.parse::<u32>().ok())
                        .filter(|v| *v > 0)
                        .ok_or("Maximum dimension must be a positive integer.")?,
                )
            }
            Some("--crop") if crop.is_none() => {
                let values = option[1]
                    .to_str()
                    .ok_or("Crop must be x,y,width,height in pixels.")?
                    .split(',')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "Crop must use non-negative integer pixels.")?;
                let [x, y, width, height] = values.as_slice() else {
                    return Err("Crop must be x,y,width,height in pixels.");
                };
                crop = Some(PixelCrop {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                });
            }
            Some("--quality") if quality.is_none() => {
                quality = Some(
                    option[1]
                        .to_str()
                        .and_then(|s| s.parse::<u8>().ok())
                        .filter(|v| (1..=100).contains(v))
                        .ok_or("Quality must be an integer from 1 to 100.")?,
                )
            }
            _ => return Err("Unknown or repeated image option."),
        }
    }
    Ok(Request::ConvertImage {
        input: PathBuf::from(input),
        output: PathBuf::from(output),
        background,
        quality,
        crop,
        max_dimension,
        max_bytes,
        minimum_quality,
        expected_source_sha256: None,
    })
}

fn split_arguments(
    args: &[std::ffi::OsString],
) -> (
    Vec<PathBuf>,
    Option<fileform_engine::PdfSplitSelection>,
    bool,
) {
    let mut inputs = Vec::new();
    let mut selection = None;
    let mut dry_run = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].to_str() {
            Some("--dry-run") if !dry_run => {
                dry_run = true;
                index += 1;
            }
            Some(flag @ ("--ranges" | "--every" | "--after")) if selection.is_none() => {
                let value = args
                    .get(index + 1)
                    .and_then(|v| v.to_str())
                    .unwrap_or_else(|| argument_error("Split option needs a value."));
                selection = Some(match flag {
                    "--ranges" => fileform_engine::PdfSplitSelection::Ranges {
                        ranges: value.into(),
                    },
                    "--every" => fileform_engine::PdfSplitSelection::Every {
                        count: value.parse().unwrap_or_else(|_| {
                            argument_error("Choose a positive split interval.")
                        }),
                    },
                    _ => fileform_engine::PdfSplitSelection::After {
                        positions: value
                            .split(',')
                            .map(|v| {
                                v.trim().parse().unwrap_or_else(|_| {
                                    argument_error("Use comma-separated split positions.")
                                })
                            })
                            .collect(),
                    },
                });
                index += 2;
            }
            Some("--") => {
                inputs.extend(args[index + 1..].iter().map(PathBuf::from));
                break;
            }
            Some(flag) if flag.starts_with("--") => {
                argument_error("Unknown, repeated or conflicting split option.")
            }
            _ => {
                inputs.push(PathBuf::from(&args[index]));
                index += 1;
            }
        }
    }
    (inputs, selection, dry_run)
}
fn argument_error(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}

fn page_arguments(
    args: &[std::ffi::OsString],
    allow_quality: bool,
) -> (Vec<PathBuf>, Option<String>, Option<u8>) {
    let mut inputs = Vec::new();
    let mut ranges = None;
    let mut quality = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].to_str() {
            Some("--pages") if ranges.is_none() => {
                ranges = Some(
                    args.get(index + 1)
                        .and_then(|v| v.to_str())
                        .unwrap_or_else(|| argument_error("Page selection needs a value."))
                        .into(),
                );
                index += 2;
            }
            Some("--quality") if allow_quality && quality.is_none() => {
                quality = Some(
                    args.get(index + 1)
                        .and_then(|v| v.to_str())
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| argument_error("Choose JPEG quality from 5 to 100.")),
                );
                index += 2;
            }
            Some("--") => {
                inputs.extend(args[index + 1..].iter().map(PathBuf::from));
                break;
            }
            Some(flag) if flag.starts_with("--") => {
                argument_error("Unknown or repeated page option.")
            }
            _ => {
                inputs.push(PathBuf::from(&args[index]));
                index += 1;
            }
        }
    }
    (inputs, ranges, quality)
}

fn collision_argument(
    args: Vec<std::ffi::OsString>,
) -> (
    Vec<std::ffi::OsString>,
    Option<fileform_engine::OutputCollision>,
) {
    let mut cleaned = Vec::new();
    let mut collision = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--" {
            cleaned.extend_from_slice(&args[index..]);
            break;
        }
        if args[index] == "--collision" {
            if collision.is_some() {
                argument_error("Repeated collision policy.");
            }
            collision = Some(match args.get(index + 1).and_then(|v| v.to_str()) {
                Some("fail") => fileform_engine::OutputCollision::Fail,
                Some("rename") => fileform_engine::OutputCollision::Rename,
                _ => argument_error("Choose --collision fail or rename."),
            });
            index += 2;
        } else {
            cleaned.push(args[index].clone());
            index += 1;
        }
    }
    (cleaned, collision)
}

fn audio_stream_argument(args: Vec<std::ffi::OsString>) -> (Vec<std::ffi::OsString>, Option<u32>) {
    let mut cleaned = Vec::new();
    let mut selected = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--" {
            cleaned.extend_from_slice(&args[index..]);
            break;
        }
        if args[index] == "--audio-stream" {
            if selected.is_some() {
                argument_error("Repeated audio track selection.");
            }
            selected = Some(
                args.get(index + 1)
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| argument_error("Choose an existing audio stream index.")),
            );
            index += 2;
        } else {
            cleaned.push(args[index].clone());
            index += 1;
        }
    }
    (cleaned, selected)
}
