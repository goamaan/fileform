// SPDX-License-Identifier: Apache-2.0
use crate::{Cancellation, Result};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
#[derive(Debug, Serialize)]
pub struct MediaPackVerification {
    pub version: String,
    pub architecture: String,
    pub supports_mp3: bool,
    pub executables: BTreeMap<String, String>,
}
pub fn verify(directory: &Path, cancellation: &Cancellation) -> Result<MediaPackVerification> {
    let pack = crate::native_pack::verify(
        directory,
        cancellation,
        "app.fileform.media",
        &["ffmpeg", "ffprobe"],
        true,
    )?;
    Ok(MediaPackVerification {
        version: pack.version,
        architecture: pack.architecture,
        supports_mp3: pack.audio_encoders.iter().any(|s| s == "libmp3lame"),
        executables: pack.executables,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    const MANIFEST_LIMIT: u64 = 64 * 1024;
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("bin")).unwrap();
        let mut hashes = BTreeMap::new();
        for name in ["ffmpeg", "ffprobe"] {
            let suffix = if cfg!(windows) { ".exe" } else { "" };
            let path = dir.path().join(format!("bin/{name}{suffix}"));
            std::fs::write(&path, name).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            hashes.insert(
                name,
                Sha256::digest(name.as_bytes())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            );
        }
        let manifest = serde_json::json!({"schemaVersion":1,"id":"app.fileform.media","version":"test","architecture":std::env::consts::ARCH,"networkProtocols":false,"audioEncoders":["libmp3lame"],"executables":hashes});
        std::fs::write(
            dir.path().join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        dir
    }
    #[test]
    fn verifies_both_platform_binaries_and_rejects_tampering() {
        let dir = fixture();
        let result = verify(dir.path(), &Cancellation::default()).unwrap();
        assert!(result.supports_mp3);
        assert_eq!(result.executables.len(), 2);
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        std::fs::write(dir.path().join(format!("bin/ffprobe{suffix}")), b"tampered").unwrap();
        assert_eq!(
            verify(dir.path(), &Cancellation::default())
                .unwrap_err()
                .code,
            "engine_unavailable"
        );
    }
    #[test]
    fn rejects_network_incompatible_and_oversized_manifests() {
        for (field, value) in [
            ("networkProtocols", serde_json::json!(true)),
            ("architecture", serde_json::json!("wrong")),
            ("schemaVersion", serde_json::json!(2)),
        ] {
            let dir = fixture();
            let path = dir.path().join("manifest.json");
            let mut value_map: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value_map[field] = value;
            std::fs::write(path, serde_json::to_vec(&value_map).unwrap()).unwrap();
            assert!(verify(dir.path(), &Cancellation::default()).is_err());
        }
        let dir = fixture();
        std::fs::write(
            dir.path().join("manifest.json"),
            vec![b' '; MANIFEST_LIMIT as usize + 1],
        )
        .unwrap();
        assert!(verify(dir.path(), &Cancellation::default()).is_err());
    }
    #[test]
    fn cancellation_prevents_pack_access() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert_eq!(
            verify(Path::new("missing"), &cancellation)
                .unwrap_err()
                .code,
            "cancelled"
        );
    }
    #[cfg(unix)]
    #[test]
    fn rejects_binary_symlink_outside_pack() {
        let dir = fixture();
        let external = tempfile::NamedTempFile::new().unwrap();
        std::fs::remove_file(dir.path().join("bin/ffprobe")).unwrap();
        std::os::unix::fs::symlink(external.path(), dir.path().join("bin/ffprobe")).unwrap();
        assert!(verify(dir.path(), &Cancellation::default()).is_err());
    }
}
