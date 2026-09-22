// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use fileform_engine::{Cancellation, Request};
use std::io::{self, BufRead, Read, Write};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SupervisedRequest {
    request: Request,
    cancel_on_disconnect: bool,
}
fn main() {
    let mut data = Vec::new();
    // Bare requests retain one-shot CLI semantics. Desktop requests opt into
    // cancellation when their supervising control pipe closes or becomes invalid.
    let mut input = io::BufReader::new(io::stdin());
    let read = input.by_ref().take(1_048_577).read_until(b'\n', &mut data);
    let cancellation = Cancellation::default();
    let reply = if read.is_err() || data.len() > 1_048_576 {
        serde_json::json!({"version":1,"ok":false,"error":{"code":"invalid_request","message":"Request exceeds 1 MiB or cannot be read."}})
    } else {
        let parsed = serde_json::from_slice::<SupervisedRequest>(&data)
            .map(|envelope| (envelope.request, envelope.cancel_on_disconnect))
            .or_else(|_| serde_json::from_slice::<Request>(&data).map(|request| (request, false)));
        match parsed {
            Ok((request, cancel_on_disconnect)) => {
                let signal = cancellation.clone();
                std::thread::spawn(move || {
                    let mut control = Vec::new();
                    let read = input.take(8).read_until(b'\n', &mut control);
                    if cancel_on_disconnect
                        || (read.is_ok() && (control == b"cancel\n" || control == b"cancel\r\n"))
                    {
                        signal.cancel();
                    }
                });
                match fileform_engine::execute_with_cancellation(request, cancellation) {
                    Ok(result) => serde_json::json!({"version":1,"ok":true,"result":result}),
                    Err(error) => serde_json::json!({"version":1,"ok":false,"error":error}),
                }
            }
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
