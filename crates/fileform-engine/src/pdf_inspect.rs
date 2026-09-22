// SPDX-License-Identifier: Apache-2.0
use crate::{fail, native_pack, native_process, Cancellation, Result, Source};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path, process::Command, time::Duration};
#[derive(Debug, Serialize)]
pub struct PdfPackVerification {
    pub version: String,
    pub architecture: String,
    pub executables: BTreeMap<String, String>,
}
pub fn verify_pack(directory: &Path, cancel: &Cancellation) -> Result<PdfPackVerification> {
    let pack = native_pack::verify(directory, cancel, "app.fileform.pdf", &["qpdf"], false)?;
    Ok(PdfPackVerification {
        version: pack.version,
        architecture: pack.architecture,
        executables: pack.executables,
    })
}
#[derive(Debug, Serialize)]
pub struct PdfInspection {
    pub sha256: String,
    pub bytes: u64,
    pub pages: u32,
    pub qpdf_check_passed: bool,
}
pub(crate) fn command(directory: &Path) -> Result<Command> {
    let mut command = Command::new(directory.canonicalize()?.join(if cfg!(windows) {
        "bin/qpdf.exe"
    } else {
        "bin/qpdf"
    }));
    command.env_clear();
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    Ok(command)
}
pub fn inspect(input: &Path, directory: &Path, cancel: &Cancellation) -> Result<PdfInspection> {
    verify_pack(directory, cancel)?;
    let mut source = Source::open_with_limit(input, cancel.clone(), 512 * 1024 * 1024)?;
    let mut count = command(directory)?;
    count.arg("--show-npages").arg(source.snapshot.path());
    let count = native_process::run(count, cancel, Duration::from_secs(60), 4096)?;
    let pages = std::str::from_utf8(&count)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|n| (1..=1000).contains(n))
        .ok_or_else(|| fail("unsupported", "Choose a PDF with 1–1000 readable pages."))?;
    let mut check = command(directory)?;
    check.arg("--check").arg(source.snapshot.path());
    native_process::run(check, cancel, Duration::from_secs(60), 64 * 1024)?;
    source.check(input)?;
    Ok(PdfInspection {
        sha256: source.hash,
        bytes: source.input.metadata()?.len(),
        pages,
        qpdf_check_passed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn pdf_pack_checks_identity_and_binary_integrity() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("bin")).unwrap();
        let binary = dir.path().join(if cfg!(windows) {
            "bin/qpdf.exe"
        } else {
            "bin/qpdf"
        });
        std::fs::write(&binary, b"fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let hash: String = Sha256::digest(b"fixture")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let mut manifest = serde_json::json!({"schemaVersion":1,"id":"app.fileform.pdf","version":"test","architecture":std::env::consts::ARCH,"executables":{"qpdf":hash}});
        let path = dir.path().join("manifest.json");
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert_eq!(
            verify_pack(dir.path(), &Cancellation::default())
                .unwrap()
                .executables
                .len(),
            1
        );
        manifest["id"] = "app.fileform.media".into();
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(verify_pack(dir.path(), &Cancellation::default()).is_err());
        manifest["id"] = "app.fileform.pdf".into();
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        std::fs::write(binary, b"modified").unwrap();
        assert!(verify_pack(dir.path(), &Cancellation::default()).is_err());
    }
}
