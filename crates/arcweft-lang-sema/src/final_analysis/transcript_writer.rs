//! Shared checked byte writer for private semantic transcripts.

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum TranscriptWriteError {
    #[error("semantic transcript byte accounting overflow")]
    ArithmeticOverflow,
    #[error("semantic transcript byte limit {limit} exceeded by attempt {attempted}")]
    LimitExceeded { limit: u64, attempted: u64 },
}

pub(crate) trait TranscriptByteCounter {
    type Error;

    fn charge_transcript_bytes(&mut self, delta: u64) -> Result<(), Self::Error>;
}

/// Checked byte counter for semantic digest domains without a larger work
/// transaction. The caller supplies an exact, precomputed byte ceiling.
pub(crate) struct CheckedTranscriptByteBudget {
    used: u64,
    limit: u64,
}

impl CheckedTranscriptByteBudget {
    pub(crate) const fn exact(limit: u64) -> Self {
        Self { used: 0, limit }
    }
}

impl TranscriptByteCounter for CheckedTranscriptByteBudget {
    type Error = TranscriptWriteError;

    fn charge_transcript_bytes(&mut self, delta: u64) -> Result<(), Self::Error> {
        let attempted = self
            .used
            .checked_add(delta)
            .ok_or(TranscriptWriteError::ArithmeticOverflow)?;
        if attempted > self.limit {
            return Err(TranscriptWriteError::LimitExceeded {
                limit: self.limit,
                attempted,
            });
        }
        self.used = attempted;
        Ok(())
    }
}

pub(crate) struct TranscriptHasher<'a, C: TranscriptByteCounter + ?Sized> {
    hasher: blake3::Hasher,
    budget: &'a mut C,
}

impl<'a, C: TranscriptByteCounter + ?Sized> TranscriptHasher<'a, C> {
    pub(crate) fn new(budget: &'a mut C) -> Self {
        Self {
            hasher: blake3::Hasher::new(),
            budget,
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) -> Result<(), C::Error>
    where
        C::Error: From<TranscriptWriteError>,
    {
        let delta = u64::try_from(bytes.len())
            .map_err(|_| C::Error::from(TranscriptWriteError::ArithmeticOverflow))?;
        self.budget.charge_transcript_bytes(delta)?;
        self.hasher.update(bytes);
        Ok(())
    }

    pub(crate) fn finalize(self) -> [u8; 32] {
        *self.hasher.finalize().as_bytes()
    }
}
