// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use fileform_engine::Request;
use std::path::PathBuf;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let request = match args.as_slice() {
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
                "Usage: fileform-native inspect FILE | convert-table INPUT OUTPUT.{{json,csv,tsv}}"
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
