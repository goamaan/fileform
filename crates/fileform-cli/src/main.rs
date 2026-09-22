// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use fileform_engine::{
    Background, MediaInterval, MediaTime, PixelCrop, Request, SampleRange, VideoEncoding, VideoFit,
};
use std::path::PathBuf;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let request = match args.as_slice() {
        [command, input, output, directory, start, end] if command == "trim-audio-time" => {
            let time = |value: &std::ffi::OsString| {
                MediaTime::decimal(value.to_str().unwrap_or("")).unwrap_or_else(|e| {
                    eprintln!("{}", e.message);
                    std::process::exit(2)
                })
            };
            Request::TrimAudioTime {
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
                "Usage: fileform-native trim-audio-time INPUT OUTPUT.{{wav,flac}} PACK_DIRECTORY START_SECONDS END_SECONDS | inspect-audio-timeline FILE PACK_DIRECTORY | fit-video INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY BYTES | fit-audio INPUT OUTPUT.{{wav,flac,m4a,mp3}} PACK_DIRECTORY BYTES | convert-video INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY [--max-dimension PIXELS] | remux-video INPUT OUTPUT.{{mp4,mov}} PACK_DIRECTORY | trim-audio INPUT OUTPUT.{{wav,flac}} PACK_DIRECTORY START_SAMPLE END_SAMPLE | convert-audio INPUT OUTPUT.{{wav,flac,m4a,mp3}} PACK_DIRECTORY | inspect-media FILE PACK_DIRECTORY | verify-media-pack DIRECTORY | inspect FILE | inspect-image FILE | convert-image INPUT OUTPUT.{{png,jpg,tiff}} [--background white|black] [--quality 1-100] [--crop x,y,width,height] [--max-dimension pixels] [--max-bytes bytes] [--minimum-quality 1-100] | convert-table INPUT OUTPUT.{{json,csv,tsv}}"
            );
            std::process::exit(2);
        }
    };
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
