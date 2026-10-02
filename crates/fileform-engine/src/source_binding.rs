// SPDX-License-Identifier: Apache-2.0
use crate::{fail, Result, Source};
pub(crate) fn single(source: &Source, expected: Option<&str>) -> Result<()> {
    if let Some(expected) = expected {
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        {
            return Err(fail("invalid_request", "Invalid inspected source hash."));
        }
        if source.hash != expected {
            return Err(fail(
                "source_changed",
                "The inspected file changed. Add it again.",
            ));
        }
    }
    Ok(())
}
pub(crate) fn multiple(sources: &[Source], expected: Option<&[String]>) -> Result<()> {
    if let Some(expected) = expected {
        if sources.len() != expected.len() {
            return Err(fail(
                "invalid_request",
                "Bind every selected source hash in input order.",
            ));
        }
        for (source, hash) in sources.iter().zip(expected) {
            single(source, Some(hash))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_validate_identity_and_order() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, b"first").unwrap();
        std::fs::write(&b, b"second").unwrap();
        let sources = vec![Source::open(&a).unwrap(), Source::open(&b).unwrap()];
        let hashes = sources.iter().map(|v| v.hash.clone()).collect::<Vec<_>>();
        multiple(&sources, Some(&hashes)).unwrap();
        multiple(&sources, None).unwrap();
        assert_eq!(
            multiple(&sources, Some(&hashes[..1])).unwrap_err().code,
            "invalid_request"
        );
        let mut reversed = hashes.clone();
        reversed.reverse();
        assert_eq!(
            multiple(&sources, Some(&reversed)).unwrap_err().code,
            "source_changed"
        );
        assert_eq!(
            single(&sources[0], Some("bad")).unwrap_err().code,
            "invalid_request"
        );
        assert_eq!(
            single(&sources[0], Some(&hashes[0].to_uppercase()))
                .unwrap_err()
                .code,
            "invalid_request"
        );
    }
}
