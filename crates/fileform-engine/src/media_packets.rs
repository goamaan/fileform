// SPDX-License-Identifier: Apache-2.0
use crate::{
    fail, media_pack, media_probe, media_timeline::TimeBase, native_process, Cancellation, Result,
    Source,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command, time::Duration};
const MAX_PACKETS: usize = 100000;
#[derive(Debug, Deserialize)]
pub(crate) struct Packet {
    pub pts: i64,
    pub dts: Option<i64>,
    pub duration: i64,
    pub data_hash: String,
}
fn packets<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Packet>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<Packet>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("at most 100000 media packets")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(packet) = seq.next_element()? {
                if values.len() == MAX_PACKETS {
                    return Err(serde::de::Error::custom("Packet count limit exceeded"));
                }
                values.push(packet);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Bounded)
}
#[derive(Deserialize)]
struct Report {
    #[serde(deserialize_with = "packets")]
    packets: Vec<Packet>,
}
fn validate(packets: &[Packet], cancellation: &Cancellation) -> Result<()> {
    if packets.is_empty() {
        return Err(fail("unsupported", "No complete media packets were found."));
    }
    for packet in packets {
        cancellation.check()?;
        if !(-i64::MAX / 4..=i64::MAX / 4).contains(&packet.pts)
            || packet
                .dts
                .is_some_and(|n| !(-i64::MAX / 4..=i64::MAX / 4).contains(&n))
            || !(1..=i64::MAX / 4).contains(&packet.duration)
            || packet.data_hash.len() != 71
            || !packet.data_hash.starts_with("SHA256:")
            || !packet.data_hash.as_bytes()[7..]
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            return Err(fail(
                "unsupported",
                "Media packet timestamps or content hashes are incomplete or invalid.",
            ));
        }
    }
    Ok(())
}
pub(crate) fn read(
    input: &Path,
    directory: &Path,
    index: u32,
    cancellation: &Cancellation,
) -> Result<Vec<Packet>> {
    media_pack::verify(directory, cancellation)?;
    let mut command = Command::new(directory.canonicalize()?.join(if cfg!(windows) {
        "bin/ffprobe.exe"
    } else {
        "bin/ffprobe"
    }));
    command.env_clear();
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    command
        .args([
            "-v",
            "error",
            "-max_alloc",
            "268435456",
            "-threads",
            "2",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac",
            "-select_streams",
            &index.to_string(),
            "-show_packets",
            "-show_data_hash",
            "sha256",
            "-show_entries",
            "packet=pts,dts,duration,data_hash",
            "-of",
            "json=compact=1",
        ])
        .arg(input);
    let bytes = native_process::run(
        command,
        cancellation,
        Duration::from_secs(120),
        32 * 1024 * 1024,
    )?;
    let report: Report = serde_json::from_slice(&bytes).map_err(|_| {
        fail(
            "unsupported",
            "Invalid packet report or more than 100000 packets.",
        )
    })?;
    validate(&report.packets, cancellation)?;
    Ok(report.packets)
}
#[derive(Debug, Serialize)]
pub struct PacketInspection {
    pub sha256: String,
    pub stream_index: u32,
    pub packet_count: usize,
    pub time_base: TimeBase,
    pub first_pts: i64,
    pub last_end_pts: i64,
    pub maximum_packet_ticks: i64,
    pub has_reordered_packets: bool,
    pub packet_sequence_sha256: String,
}
pub fn inspect(
    input: &Path,
    directory: &Path,
    index: u32,
    cancellation: &Cancellation,
) -> Result<PacketInspection> {
    let mut source = Source::open_with_limit(input, cancellation.clone(), 2 * 1024 * 1024 * 1024)?;
    let info = media_probe::inspect(source.snapshot.path(), directory, cancellation)?;
    let stream = info
        .streams
        .iter()
        .find(|s| s.index == index && matches!(s.codec_type.as_str(), "audio" | "video"))
        .ok_or_else(|| {
            fail(
                "invalid_request",
                "Choose an existing audio or video stream index.",
            )
        })?;
    let time_base = TimeBase::parse(stream.time_base.as_deref())?;
    let packets = read(source.snapshot.path(), directory, index, cancellation)?;
    let mut hash = Sha256::new();
    for packet in &packets {
        cancellation.check()?;
        hash.update(packet.pts.to_le_bytes());
        hash.update([u8::from(packet.dts.is_some())]);
        hash.update(packet.dts.unwrap_or(0).to_le_bytes());
        hash.update(packet.duration.to_le_bytes());
        hash.update(packet.data_hash.as_bytes());
    }
    let digest = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    source.check(input)?;
    Ok(PacketInspection {
        sha256: source.hash,
        stream_index: index,
        packet_count: packets.len(),
        time_base,
        first_pts: packets.iter().map(|p| p.pts).min().unwrap(),
        last_end_pts: packets.iter().map(|p| p.pts + p.duration).max().unwrap(),
        maximum_packet_ticks: packets.iter().map(|p| p.duration).max().unwrap(),
        has_reordered_packets: packets.iter().any(|p| p.dts != Some(p.pts)),
        packet_sequence_sha256: digest,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packet_evidence_requires_timestamps_durations_and_hashes() {
        let mut packets = vec![Packet {
            pts: -1024,
            dts: Some(-1024),
            duration: 1024,
            data_hash: format!("SHA256:{}", "a".repeat(64)),
        }];
        assert!(validate(&packets, &Cancellation::default()).is_ok());
        packets[0].data_hash = "SHA256:bad".into();
        assert!(validate(&packets, &Cancellation::default()).is_err());
        packets[0].data_hash = format!("SHA256:{}", "a".repeat(64));
        packets[0].duration = 0;
        assert!(validate(&packets, &Cancellation::default()).is_err());
    }
    #[test]
    fn packet_list_is_bounded_during_parsing() {
        let packet = format!(
            r#"{{"pts":0,"duration":1,"data_hash":"SHA256:{}"}}"#,
            "a".repeat(64)
        );
        let text = format!(
            r#"{{"packets":[{}]}}"#,
            vec![packet; MAX_PACKETS + 1].join(",")
        );
        assert!(serde_json::from_str::<Report>(&text)
            .err()
            .unwrap()
            .to_string()
            .contains("Packet count limit"));
    }
}
