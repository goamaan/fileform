// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use fileform_engine::{Background, PixelCrop, Request};
use std::path::PathBuf;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let request = match args.as_slice() {
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
                "Usage: fileform-native inspect FILE | inspect-image FILE.png | convert-image INPUT.png OUTPUT.{{png,jpg}} [--background white|black] [--quality 1-100] [--crop x,y,width,height] [--max-dimension pixels] | convert-table INPUT OUTPUT.{{json,csv,tsv}}"
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
    for option in options.as_chunks::<2>().0 {
        match option[0].to_str() {
            Some("--background") if background.is_none() => {
                background = Some(match option[1].to_str() {
                    Some("white") => Background::White,
                    Some("black") => Background::Black,
                    _ => return Err("Background must be white or black."),
                })
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
        expected_source_sha256: None,
    })
}
