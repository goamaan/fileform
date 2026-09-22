// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

const MANIFEST_LIMIT: u64 = 64 * 1024;
const BINARY_LIMIT: u64 = 512 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    id: String,
    version: String,
    architecture: String,
    network_protocols: bool,
    executables: BTreeMap<String, String>,
    #[serde(default)]
    audio_encoders: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct MediaPackVerification {
    pub version: String,
    pub architecture: String,
    pub supports_mp3: bool,
    pub executables: BTreeMap<String, String>,
}
fn invalid() -> crate::Failure {
    fail(
        "engine_unavailable",
        "The media pack is missing, incompatible, or failed integrity verification.",
    )
}
fn architecture(value: &str) -> &str {
    match value {
        "arm64" => "aarch64",
        "amd64" => "x86_64",
        other => other,
    }
}
fn contained_file(root: &Path, relative: &str) -> Result<File> {
    let path = root.join(relative).canonicalize().map_err(|_| invalid())?;
    if !path.starts_with(root) {
        return Err(invalid());
    }
    let file = File::open(path).map_err(|_| invalid())?;
    if !file.metadata()?.is_file() {
        return Err(invalid());
    }
    Ok(file)
}
/// Checks a pack against its manifest. The manifest must itself come from the
/// trusted application distribution; this is not publisher authentication.
pub fn verify(directory: &Path, cancellation: &Cancellation) -> Result<MediaPackVerification> {
    cancellation.check()?;
    let root = directory.canonicalize().map_err(|_| invalid())?;
    let manifest_file = contained_file(&root, "manifest.json")?;
    let mut bytes = Vec::new();
    manifest_file
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err(invalid());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if manifest.schema_version != 1
        || manifest.id != "app.fileform.media"
        || manifest.network_protocols
        || manifest.version.is_empty()
        || manifest.version.len() > 128
        || architecture(&manifest.architecture) != std::env::consts::ARCH
    {
        return Err(invalid());
    }
    let mut verified = BTreeMap::new();
    for name in ["ffmpeg", "ffprobe"] {
        cancellation.check()?;
        let expected = manifest.executables.get(name).ok_or_else(invalid)?;
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid());
        }
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        let mut file = contained_file(&root, &format!("bin/{name}{suffix}"))?;
        let metadata = file.metadata()?;
        if metadata.len() == 0 || metadata.len() > BINARY_LIMIT {
            return Err(invalid());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(invalid());
            }
        }
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        let mut total = 0u64;
        loop {
            cancellation.check()?;
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > BINARY_LIMIT {
                return Err(invalid());
            }
            hash.update(&buffer[..count]);
        }
        let actual = hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if total != metadata.len() || actual != *expected {
            return Err(invalid());
        }
        verified.insert(name.into(), actual);
    }
    cancellation.check()?;
    Ok(MediaPackVerification {
        version: manifest.version,
        architecture: std::env::consts::ARCH.into(),
        supports_mp3: manifest
            .audio_encoders
            .iter()
            .any(|name| name == "libmp3lame"),
        executables: verified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
