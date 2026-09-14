// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use fileform_engine::Request;
use std::io::{self, Read, Write};
fn main() {
    let mut data = Vec::new();
    let read = io::stdin().take(1_048_577).read_to_end(&mut data);
    let reply = if read.is_err() || data.len() > 1_048_576 {
        serde_json::json!({"version":1,"ok":false,"error":{"code":"invalid_request","message":"Request exceeds 1 MiB or cannot be read."}})
    } else {
        match serde_json::from_slice::<Request>(&data) {
            Ok(request) => match fileform_engine::execute(request) {
                Ok(result) => serde_json::json!({"version":1,"ok":true,"result":result}),
                Err(error) => serde_json::json!({"version":1,"ok":false,"error":error}),
            },
            Err(_) => {
                serde_json::json!({"version":1,"ok":false,"error":{"code":"invalid_request","message":"Send one supported JSON request."}})
            }
        }
    };
    let ok = reply["ok"] == true;
    let mut stdout = io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &reply).is_err() || stdout.write_all(b"\n").is_err() {
        std::process::exit(2);
    }
    if !ok {
        std::process::exit(1);
    }
}
