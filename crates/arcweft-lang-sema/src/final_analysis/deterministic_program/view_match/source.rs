//! Retained data is checked against the original immutable semantic owner.

use arcweft_core::pattern::{
    MatchSourceData, MatchSourceEncodingError, MatchSourceSink, MatchSourceWriter,
};

use super::CompilerLocalViewMatchCatalogError as Error;

impl From<MatchSourceEncodingError> for Error {
    fn from(_: MatchSourceEncodingError) -> Self {
        Self::CoordinateLength
    }
}

pub(super) fn validate(
    checked: &crate::final_analysis::CheckedMatch,
    retained: &MatchSourceData,
    site: arcweft_view::ViewMatchSiteId,
) -> Result<(), Error> {
    if !retained.has_canonical_arm_coordinates()
        || arcweft_view::ViewMatchSiteId::for_source_coordinate(retained.coordinate())? != site
    {
        return Err(Error::RetainedSourceMismatch);
    }
    // This ceiling comes from the original checked transcript. Candidate data
    // cannot choose a larger allowance, and no candidate bytes are copied.
    let mut transcript = MatchSourceWriter::new(TranscriptMatch {
        hasher: blake3::Hasher::new(),
        remaining: checked.transcript().byte_len(),
    });
    let encoded = retained.write_semantic(&mut transcript);
    let transcript = transcript.finish(encoded)?;
    if transcript.remaining != 0
        || transcript.hasher.finalize().as_bytes() != checked.semantic_digest().as_bytes()
    {
        return Err(Error::RetainedSourceMismatch);
    }
    Ok(())
}

struct TranscriptMatch {
    hasher: blake3::Hasher,
    remaining: u64,
}

impl MatchSourceSink for TranscriptMatch {
    type Error = Error;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let count = u64::try_from(bytes.len()).map_err(|_| Error::CoordinateLength)?;
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or(Error::RetainedSourceMismatch)?;
        self.hasher.update(bytes);
        Ok(())
    }
}
