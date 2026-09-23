// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use serde::Deserialize;
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
    network_protocols: Option<bool>,
    executables: BTreeMap<String, String>,
    #[serde(default)]
    libraries: BTreeMap<String, String>,
    #[serde(default)]
    audio_encoders: Vec<String>,
    #[serde(default)]
    assets: BTreeMap<String, String>,
}
#[derive(Debug)]
pub(crate) struct VerifiedPack {
    pub version: String,
    pub architecture: String,
    pub audio_encoders: Vec<String>,
    pub executables: BTreeMap<String, String>,
    pub assets: BTreeMap<String, String>,
}
fn invalid() -> crate::Failure {
    fail(
        "engine_unavailable",
        "The native tool pack is missing, incompatible, or failed integrity verification.",
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
    if !std::fs::metadata(&path)?.is_file() {
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
pub(crate) fn verify(
    directory: &Path,
    cancellation: &Cancellation,
    id: &str,
    names: &[&str],
    require_offline: bool,
) -> Result<VerifiedPack> {
    verify_with_libraries(directory, cancellation, id, names, &[], require_offline)
}
pub(crate) fn verify_with_libraries(
    directory: &Path,
    cancellation: &Cancellation,
    id: &str,
    names: &[&str],
    libraries: &[&str],
    require_offline: bool,
) -> Result<VerifiedPack> {
    verify_contents(
        directory,
        cancellation,
        id,
        names,
        libraries,
        &[],
        require_offline,
    )
}
pub(crate) fn verify_with_assets(
    directory: &Path,
    cancellation: &Cancellation,
    id: &str,
    names: &[&str],
    assets: &[&str],
    require_offline: bool,
) -> Result<VerifiedPack> {
    verify_contents(
        directory,
        cancellation,
        id,
        names,
        &[],
        assets,
        require_offline,
    )
}
fn verify_contents(
    directory: &Path,
    cancellation: &Cancellation,
    id: &str,
    names: &[&str],
    libraries: &[&str],
    assets: &[&str],
    require_offline: bool,
) -> Result<VerifiedPack> {
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
        || manifest.id != id
        || manifest.network_protocols == Some(true)
        || (require_offline && manifest.network_protocols != Some(false))
        || manifest.version.is_empty()
        || manifest.version.len() > 128
        || architecture(&manifest.architecture) != std::env::consts::ARCH
    {
        return Err(invalid());
    }
    let mut verified = BTreeMap::new();
    for &name in names {
        cancellation.check()?;
        let expected = manifest.executables.get(name).ok_or_else(invalid)?;
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        let actual = verify_file(
            &root,
            &format!("bin/{name}{suffix}"),
            expected,
            cancellation,
            true,
            BINARY_LIMIT,
        )?;
        verified.insert(name.into(), actual);
    }
    for &name in libraries {
        let expected = manifest.libraries.get(name).ok_or_else(invalid)?;
        verify_file(
            &root,
            &format!("bin/{name}"),
            expected,
            cancellation,
            false,
            BINARY_LIMIT,
        )?;
    }
    let mut verified_assets = BTreeMap::new();
    for &relative in assets {
        if relative.contains([':', '\\'])
            || !Path::new(relative)
                .components()
                .all(|v| matches!(v, std::path::Component::Normal(_)))
        {
            return Err(invalid());
        }
        let expected = manifest.assets.get(relative).ok_or_else(invalid)?;
        let actual = verify_file(
            &root,
            relative,
            expected,
            cancellation,
            false,
            64 * 1024 * 1024,
        )?;
        verified_assets.insert(relative.to_owned(), actual);
    }
    cancellation.check()?;
    Ok(VerifiedPack {
        version: manifest.version,
        architecture: std::env::consts::ARCH.into(),
        audio_encoders: manifest.audio_encoders,
        executables: verified,
        assets: verified_assets,
    })
}

fn verify_file(
    root: &Path,
    relative: &str,
    expected: &str,
    cancellation: &Cancellation,
    executable: bool,
    maximum: u64,
) -> Result<String> {
    #[cfg(not(unix))]
    let _ = executable;
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    let mut file = contained_file(root, relative)?;
    let metadata = file.metadata()?;
    if metadata.len() == 0 || metadata.len() > maximum {
        return Err(invalid());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if executable && metadata.permissions().mode() & 0o111 == 0 {
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
        if total > maximum {
            return Err(invalid());
        }
        hash.update(&buffer[..count]);
    }
    let actual = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if total != metadata.len() || actual != expected {
        return Err(invalid());
    }
    Ok(actual)
}
