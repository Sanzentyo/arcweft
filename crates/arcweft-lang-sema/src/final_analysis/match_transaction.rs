//! Transaction-local accounting for checked Match coverage.

use super::transcript_writer::{TranscriptByteCounter, TranscriptHasher, TranscriptWriteError};
use crate::{semantic_coordinate::StableSemanticCoordinate, types::SemanticTypeDigest};
use thiserror::Error;

/// One bounded resource family shared by transcript and coverage construction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedMatchLimitKind {
    Arms,
    MatrixRows,
    OrAlternatives,
    PatternNodes,
    ExpressionNodes,
    Depth,
    SequencePartitions,
    Specializations,
    UnreachableRows,
    WitnessNodes,
    TranscriptBytes,
}

impl CheckedMatchLimitKind {
    pub const COUNT: usize = 11;

    pub const ALL: [Self; Self::COUNT] = [
        Self::Arms,
        Self::MatrixRows,
        Self::OrAlternatives,
        Self::PatternNodes,
        Self::ExpressionNodes,
        Self::Depth,
        Self::SequencePartitions,
        Self::Specializations,
        Self::UnreachableRows,
        Self::WitnessNodes,
        Self::TranscriptBytes,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Arms => 0,
            Self::MatrixRows => 1,
            Self::OrAlternatives => 2,
            Self::PatternNodes => 3,
            Self::ExpressionNodes => 4,
            Self::Depth => 5,
            Self::SequencePartitions => 6,
            Self::Specializations => 7,
            Self::UnreachableRows => 8,
            Self::WitnessNodes => 9,
            Self::TranscriptBytes => 10,
        }
    }
}

/// Complete checked-`u64` admission policy for one Match transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedMatchLimits([u64; CheckedMatchLimitKind::COUNT]);

impl CheckedMatchLimits {
    pub const PRODUCTION: Self = Self::uniform(65_536)
        .with_limit(CheckedMatchLimitKind::Arms, 4_096)
        .with_limit(CheckedMatchLimitKind::Depth, 256)
        .with_limit(CheckedMatchLimitKind::Specializations, 262_144)
        .with_limit(CheckedMatchLimitKind::UnreachableRows, 4_096)
        .with_limit(CheckedMatchLimitKind::TranscriptBytes, 16 * 1024 * 1024);

    pub const fn uniform(maximum: u64) -> Self {
        Self([maximum; CheckedMatchLimitKind::COUNT])
    }

    #[must_use]
    pub const fn with_limit(mut self, kind: CheckedMatchLimitKind, limit: u64) -> Self {
        self.0[kind.index()] = limit;
        self
    }

    pub const fn limit(self, kind: CheckedMatchLimitKind) -> u64 {
        self.0[kind.index()]
    }

    pub const fn max_transcript_bytes(self) -> u64 {
        self.limit(CheckedMatchLimitKind::TranscriptBytes)
    }

    pub const fn max_depth(self) -> u64 {
        self.limit(CheckedMatchLimitKind::Depth)
    }
}

impl Default for CheckedMatchLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CheckedMatchWork([u64; CheckedMatchLimitKind::COUNT]);

impl CheckedMatchWork {
    pub(crate) const fn observed(self, kind: CheckedMatchLimitKind) -> u64 {
        self.0[kind.index()]
    }
}

#[derive(Debug)]
pub(crate) struct CheckedMatchBudget {
    limits: CheckedMatchLimits,
    work: CheckedMatchWork,
}

impl CheckedMatchBudget {
    pub(crate) const fn new(limits: CheckedMatchLimits) -> Self {
        Self {
            limits,
            work: CheckedMatchWork([0; CheckedMatchLimitKind::COUNT]),
        }
    }

    pub(crate) const fn work(&self) -> CheckedMatchWork {
        self.work
    }

    pub(crate) fn charge(
        &mut self,
        kind: CheckedMatchLimitKind,
        delta: u64,
    ) -> Result<(), CheckedMatchBuildError> {
        let attempted = self
            .work
            .observed(kind)
            .checked_add(delta)
            .ok_or(CheckedMatchBuildError::ArithmeticOverflow { kind })?;
        let limit = self.limits.limit(kind);
        if attempted > limit {
            return Err(CheckedMatchBuildError::LimitExceeded {
                kind,
                limit,
                attempted,
            });
        }
        self.work.0[kind.index()] = attempted;
        Ok(())
    }

    pub(crate) fn observe_depth(&mut self, depth: u64) -> Result<(), CheckedMatchBuildError> {
        let limit = self.limits.limit(CheckedMatchLimitKind::Depth);
        if depth > limit {
            return Err(CheckedMatchBuildError::LimitExceeded {
                kind: CheckedMatchLimitKind::Depth,
                limit,
                attempted: depth,
            });
        }
        let index = CheckedMatchLimitKind::Depth.index();
        self.work.0[index] = self.work.0[index].max(depth);
        Ok(())
    }

    /// Admits transaction-local canonical storage without recording a
    /// transcript write. The retained bytes are written (and charged) later
    /// through `TranscriptHasher`; keeping this check separate prevents
    /// materialization from inflating the exact transcript byte counter.
    pub(super) fn admit_transcript_allocation(
        &self,
        total: u64,
    ) -> Result<(), CheckedMatchBuildError> {
        let limit = self.limits.limit(CheckedMatchLimitKind::TranscriptBytes);
        if total > limit {
            return Err(CheckedMatchBuildError::LimitExceeded {
                kind: CheckedMatchLimitKind::TranscriptBytes,
                limit,
                attempted: total,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum CheckedMatchBuildError {
    #[error("checked Match owner evidence is missing at {coordinate:?}")]
    MissingExactOwner {
        coordinate: StableSemanticCoordinate,
    },
    #[error("checked Match contains recovered or poisoned semantic evidence at {coordinate:?}")]
    PoisonedSemanticNode {
        coordinate: StableSemanticCoordinate,
    },
    #[error("checked Match contains a duplicate semantic path at {coordinate:?}")]
    DuplicateSemanticPath {
        coordinate: StableSemanticCoordinate,
    },
    #[error("checked Match row is inconsistent with its exact owner at {coordinate:?}")]
    InvalidCheckedRow {
        coordinate: StableSemanticCoordinate,
    },
    #[error("Match coverage has no exact domain authority for type {type_digest:?}")]
    UnsupportedDomain { type_digest: SemanticTypeDigest },
    #[error("checked Match {kind:?} limit {limit} exceeded by attempt {attempted}")]
    LimitExceeded {
        kind: CheckedMatchLimitKind,
        limit: u64,
        attempted: u64,
    },
    #[error("checked Match {kind:?} accounting overflow")]
    ArithmeticOverflow { kind: CheckedMatchLimitKind },
    #[error("checked Match construction was cancelled")]
    Cancelled,
}

impl From<TranscriptWriteError> for CheckedMatchBuildError {
    fn from(error: TranscriptWriteError) -> Self {
        match error {
            TranscriptWriteError::ArithmeticOverflow => Self::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::TranscriptBytes,
            },
            TranscriptWriteError::LimitExceeded { limit, attempted } => Self::LimitExceeded {
                kind: CheckedMatchLimitKind::TranscriptBytes,
                limit,
                attempted,
            },
        }
    }
}

impl TranscriptByteCounter for CheckedMatchBudget {
    type Error = CheckedMatchBuildError;

    fn charge_transcript_bytes(&mut self, delta: u64) -> Result<(), Self::Error> {
        self.charge(CheckedMatchLimitKind::TranscriptBytes, delta)
    }
}

pub(super) type CoverageTranscriptHasher<'a> = TranscriptHasher<'a, CheckedMatchBudget>;

pub(super) fn checked_len(
    value: usize,
    kind: CheckedMatchLimitKind,
) -> Result<u64, CheckedMatchBuildError> {
    u64::try_from(value).map_err(|_| CheckedMatchBuildError::ArithmeticOverflow { kind })
}

pub(super) fn checked_depth_successor(depth: u64) -> Result<u64, CheckedMatchBuildError> {
    depth
        .checked_add(1)
        .ok_or(CheckedMatchBuildError::ArithmeticOverflow {
            kind: CheckedMatchLimitKind::Depth,
        })
}
