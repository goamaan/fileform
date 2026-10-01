// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutputCollision {
    #[default]
    Fail,
    Rename,
}
fn renamed(destination: &Path, index: u32, directory: bool) -> Result<PathBuf> {
    if index == 0 {
        return Ok(destination.into());
    }
    let name = if directory {
        destination.file_name()
    } else {
        destination.file_stem()
    }
    .ok_or_else(|| fail("invalid_request", "Choose an output name."))?;
    let mut name = name.to_os_string();
    name.push(format!(" ({index})"));
    if !directory {
        if let Some(extension) = destination.extension() {
            name.push(".");
            name.push(extension);
        }
    }
    Ok(destination.with_file_name(name))
}
pub(crate) fn candidates(
    destination: &Path,
    directory: bool,
    policy: OutputCollision,
) -> impl Iterator<Item = Result<PathBuf>> + '_ {
    (0..if policy == OutputCollision::Rename {
        1000
    } else {
        1
    })
        .map(move |index| renamed(destination, index, directory))
}
pub(crate) fn collision(error: &std::io::Error, destination: &Path) -> bool {
    error.kind() == std::io::ErrorKind::AlreadyExists
        || std::fs::symlink_metadata(destination).is_ok()
}
/// Retry only atomic publication of the same verified staged file. No processing
/// or source snapshot is restarted when another process takes a candidate name.
pub(crate) fn publish_file(
    mut file: tempfile::NamedTempFile,
    destination: &Path,
    policy: OutputCollision,
    cancel: &Cancellation,
) -> Result<PathBuf> {
    for candidate in candidates(destination, false, policy) {
        cancel.check()?;
        let candidate = candidate?;
        match file.persist_noclobber(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(error) => {
                if !collision(&error.error, &candidate) {
                    return Err(error.error.into());
                }
                file = error.file;
            }
        }
    }
    Err(fail(
        "collision",
        "No free output name within the collision policy.",
    ))
}
pub(crate) fn publish_path(
    mut file: tempfile::TempPath,
    destination: &Path,
    policy: OutputCollision,
    cancel: &Cancellation,
) -> Result<PathBuf> {
    for candidate in candidates(destination, false, policy) {
        cancel.check()?;
        let candidate = candidate?;
        match file.persist_noclobber(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) => {
                if !collision(&error.error, &candidate) {
                    return Err(error.error.into());
                }
                file = error.path;
            }
        }
    }
    Err(fail(
        "collision",
        "No free output name within the collision policy.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renames_same_verified_file_without_replacing_data() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("report.pdf");
        let mut staged = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        use std::io::Write;
        staged.write_all(b"verified").unwrap();
        std::fs::write(&output, b"keep").unwrap();
        std::fs::write(root.path().join("report (1).pdf"), b"late arrival").unwrap();
        let saved = publish_file(
            staged,
            &output,
            OutputCollision::Rename,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(saved, root.path().join("report (2).pdf"));
        assert_eq!(std::fs::read(output).unwrap(), b"keep");
        assert_eq!(
            std::fs::read(root.path().join("report (1).pdf")).unwrap(),
            b"late arrival"
        );
        assert_eq!(std::fs::read(saved).unwrap(), b"verified");
        assert_eq!(
            renamed(Path::new("images.v2"), 1, true).unwrap(),
            Path::new("images.v2 (1)")
        );
        let staged = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        let path = staged.path().to_path_buf();
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(publish_file(
            staged,
            &root.path().join("cancelled.pdf"),
            OutputCollision::Rename,
            &cancel
        )
        .is_err());
        assert!(!path.exists());
    }
}
