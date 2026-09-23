// SPDX-License-Identifier: Apache-2.0
use crate::{native_pack, Cancellation, Result};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
#[derive(Debug, Serialize)]
pub struct OcrPackVerification {
    pub version: String,
    pub architecture: String,
    pub executables: BTreeMap<String, String>,
    pub assets: BTreeMap<String, String>,
    pub recognition_languages: Vec<&'static str>,
    pub automatic_language_detection: bool,
}
pub fn verify(directory: &Path, cancel: &Cancellation) -> Result<OcrPackVerification> {
    let pack = native_pack::verify_with_assets(
        directory,
        cancel,
        "app.fileform.ocr",
        &["tesseract"],
        &["tessdata/eng.traineddata", "tessdata/osd.traineddata"],
        true,
    )?;
    Ok(OcrPackVerification {
        version: pack.version,
        architecture: pack.architecture,
        executables: pack.executables,
        assets: pack.assets,
        recognition_languages: vec!["eng"],
        automatic_language_detection: false,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn models_are_required_hashed_and_confined_to_the_pack() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("bin")).unwrap();
        std::fs::create_dir(root.path().join("tessdata")).unwrap();
        let executable = root.path().join(if cfg!(windows) {
            "bin/tesseract.exe"
        } else {
            "bin/tesseract"
        });
        std::fs::write(&executable, b"fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let hash = Sha256::digest(b"fixture")
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        let mut manifest = serde_json::json!({"schemaVersion":1,"id":"app.fileform.ocr","version":"test","architecture":std::env::consts::ARCH,"networkProtocols":false,"executables":{"tesseract":hash},"assets":{"tessdata/eng.traineddata":hash,"tessdata/osd.traineddata":hash}});
        let path = root.path().join("manifest.json");
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let eng = root.path().join("tessdata/eng.traineddata");
        let osd = root.path().join("tessdata/osd.traineddata");
        assert!(verify(root.path(), &Cancellation::default()).is_err());
        std::fs::write(&eng, b"fixture").unwrap();
        std::fs::write(&osd, b"fixture").unwrap();
        assert_eq!(
            verify(root.path(), &Cancellation::default())
                .unwrap()
                .assets
                .len(),
            2
        );
        std::fs::write(&eng, b"modified").unwrap();
        assert!(verify(root.path(), &Cancellation::default()).is_err());
        std::fs::write(&eng, b"fixture").unwrap();
        manifest["networkProtocols"] = true.into();
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(verify(root.path(), &Cancellation::default()).is_err());
        manifest["networkProtocols"] = false.into();
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let signal = Cancellation::default();
        signal.cancel();
        assert!(verify(root.path(), &signal).is_err());
        #[cfg(unix)]
        {
            let outside = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(outside.path(), b"fixture").unwrap();
            std::fs::remove_file(&eng).unwrap();
            std::os::unix::fs::symlink(outside.path(), &eng).unwrap();
            assert!(verify(root.path(), &Cancellation::default()).is_err());
        }
    }
}
