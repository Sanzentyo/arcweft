//! Bounded validation of one explicitly known file-backed source document.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use arcweft_source::{
    MAX_REGISTRATION_SOURCE_BYTES, SourceDocument, SourceDocumentError, SourceDocumentIdentity,
    SourceName,
};
use thiserror::Error;

/// Failure to read or validate one explicit accepted source path.
#[derive(Debug, Error)]
pub enum ExactFileDocumentError {
    #[error("failed to read exact source file `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("exact source file `{path}` has {observed} bytes, exceeding {maximum}")]
    SourceBytes {
        path: PathBuf,
        observed: u64,
        maximum: u64,
    },
    #[error("exact source file `{path}` is not valid UTF-8")]
    Utf8 { path: PathBuf },
    #[error("failed to construct exact source document: {0}")]
    Document(#[from] SourceDocumentError),
    #[error("exact source file content no longer matches its accepted identity")]
    IdentityMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}

/// Reads only `path`, rebinds it to the accepted logical ID, and compares its exact identity.
#[allow(
    clippy::result_large_err,
    reason = "the exact validator preserves both complete source identities in its typed mismatch error"
)]
pub fn validate_exact_file_document(
    path: &Path,
    expected: &SourceDocumentIdentity,
) -> Result<Arc<SourceDocument>, ExactFileDocumentError> {
    let bytes = std::fs::read(path).map_err(|source| ExactFileDocumentError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > MAX_REGISTRATION_SOURCE_BYTES {
        return Err(ExactFileDocumentError::SourceBytes {
            path: path.to_path_buf(),
            observed,
            maximum: MAX_REGISTRATION_SOURCE_BYTES,
        });
    }
    let text = String::from_utf8(bytes).map_err(|_| ExactFileDocumentError::Utf8 {
        path: path.to_path_buf(),
    })?;
    let document = Arc::new(SourceDocument::try_new(
        expected.id().clone(),
        SourceName::path(path.display().to_string()),
        text,
    )?);
    if document.identity() != expected {
        return Err(ExactFileDocumentError::IdentityMismatch {
            expected: expected.clone(),
            actual: document.identity().clone(),
        });
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::{ExactFileDocumentError, validate_exact_file_document};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    #[test]
    fn exact_file_validation_uses_the_accepted_logical_id_and_detects_changes() {
        let path = std::env::temp_dir().join(format!(
            "arcweft-exact-source-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock follows epoch")
                .as_nanos()
        ));
        std::fs::write(&path, "accepted").expect("fixture writes");
        let accepted = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://fixture/source.arcw").expect("logical id"),
            SourceName::path("logical/source.arcw"),
            "accepted",
        )
        .expect("accepted document");

        let validated = validate_exact_file_document(&path, accepted.identity())
            .expect("unchanged file validates");
        assert_eq!(validated.identity(), accepted.identity());
        assert_eq!(
            validated.identity().id().as_str(),
            "arcweft-project://fixture/source.arcw"
        );

        std::fs::write(&path, "changed").expect("fixture changes");
        assert!(matches!(
            validate_exact_file_document(&path, accepted.identity()),
            Err(ExactFileDocumentError::IdentityMismatch { .. })
        ));
        std::fs::remove_file(path).expect("fixture removes");
    }
}
