// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Cancellation, Result};
use std::path::{Path, PathBuf};
pub(crate) struct DirectoryTransaction {
    _root: tempfile::TempDir,
    payload: PathBuf,
    destination: PathBuf,
    parent: PathBuf,
    parent_identity: same_file::Handle,
}
impl DirectoryTransaction {
    pub(crate) fn new(destination: &Path) -> Result<Self> {
        if destination.file_name().is_none() {
            return Err(fail("invalid_request", "Choose a new output folder."));
        }
        if std::fs::symlink_metadata(destination).is_ok() {
            return Err(fail("collision", "The output already exists."));
        }
        let parent = destination
            .parent()
            .filter(|v| !v.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let parent_identity = same_file::Handle::from_path(&parent)?;
        let root = tempfile::tempdir_in(&parent)?;
        let payload = root.path().join("parts");
        #[cfg(unix)]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(not(unix))]
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&payload)?;
        Ok(Self {
            _root: root,
            payload,
            destination: destination.into(),
            parent,
            parent_identity,
        })
    }
    pub(crate) fn path(&self) -> &Path {
        &self.payload
    }
    /// Caller has already verified/synced every part and rechecked every source.
    pub(crate) fn commit(self, cancel: &Cancellation) -> Result<()> {
        if self.parent_identity != same_file::Handle::from_path(&self.parent)? {
            return Err(fail("output_changed", "Output folder changed."));
        }
        cancel.check()?;
        // tempfile 3.27.0 uses RENAME_EXCL on macOS and MoveFileExW without
        // REPLACE_EXISTING on Windows. Unsupported directory moves fail closed.
        // The enclosing TempDir owns recursive cleanup on error, not TempPath.
        let mut payload = tempfile::TempPath::try_from_path(&self.payload)?;
        payload.disable_cleanup(true);
        payload.persist_noclobber(&self.destination).map_err(|e| {
            fail(
                if e.error.kind() == std::io::ErrorKind::AlreadyExists
                    || std::fs::symlink_metadata(&self.destination).is_ok()
                {
                    "collision"
                } else {
                    "io"
                },
                e.error.to_string(),
            )
        })?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_publication_is_exclusive_even_for_late_empty_folder_collisions() {
        let root = tempfile::tempdir().unwrap();
        for kind in 0..3 {
            let destination = root.path().join(format!("target-{kind}"));
            let transaction = DirectoryTransaction::new(&destination).unwrap();
            let staging = transaction._root.path().to_path_buf();
            std::fs::write(transaction.path().join("part.pdf"), b"verified").unwrap();
            if kind == 0 {
                std::fs::write(&destination, b"existing").unwrap();
            } else {
                std::fs::create_dir(&destination).unwrap();
                if kind == 2 {
                    std::fs::write(destination.join("existing"), b"keep").unwrap();
                }
            }
            assert!(transaction.commit(&Cancellation::default()).is_err());
            assert!(!staging.exists());
            if kind == 0 {
                assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
            } else {
                assert!(!destination.join("part.pdf").exists());
            }
        }
        let destination = root.path().join("complete");
        let transaction = DirectoryTransaction::new(&destination).unwrap();
        std::fs::write(transaction.path().join("part.pdf"), b"verified").unwrap();
        transaction.commit(&Cancellation::default()).unwrap();
        assert_eq!(
            std::fs::read(destination.join("part.pdf")).unwrap(),
            b"verified"
        );
        let destination = root.path().join("cancelled");
        let transaction = DirectoryTransaction::new(&destination).unwrap();
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(transaction.commit(&cancel).is_err());
        assert!(!destination.exists());
    }
}
