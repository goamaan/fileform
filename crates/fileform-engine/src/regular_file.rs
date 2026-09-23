// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Result};
use std::{fs::File, path::Path};

pub(crate) fn open(path: &Path) -> Result<File> {
    #[cfg(windows)]
    if path.components().any(|part| {
        matches!(part,
        std::path::Component::Prefix(prefix) if matches!(prefix.kind(),
            std::path::Prefix::DeviceNS(_) | std::path::Prefix::Verbatim(_)))
    }) {
        return Err(fail(
            "invalid_input",
            "Choose a regular file, not a device or pipe.",
        ));
    }
    if !std::fs::metadata(path)?.is_file() {
        return Err(fail("invalid_input", "Choose a regular file."));
    }
    #[cfg(unix)]
    let file = {
        use rustix::fs::{open, Mode, OFlags};
        // NONBLOCK is ignored for ordinary files, but a FIFO substituted after
        // metadata validation cannot block before the handle-type check below.
        File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?,
        )
    };
    #[cfg(not(unix))]
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(fail("invalid_input", "Choose a regular file."));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_work_and_directories_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        std::fs::write(&path, b"source").unwrap();
        assert_eq!(open(&path).unwrap().metadata().unwrap().len(), 6);
        assert!(open(dir.path()).is_err());
    }
    #[test]
    #[cfg(unix)]
    fn regular_symlinks_remain_supported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        let link = dir.path().join("link");
        std::fs::write(&path, b"source").unwrap();
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert_eq!(open(&link).unwrap().metadata().unwrap().len(), 6);
    }
    #[test]
    #[cfg(windows)]
    fn device_namespaces_are_rejected_before_opening() {
        for path in [
            r"\\.\pipe\fileform-test",
            r"\\?\GLOBALROOT\Device\NamedPipe\fileform-test",
        ] {
            assert_eq!(open(Path::new(path)).unwrap_err().code, "invalid_input");
        }
    }
}
