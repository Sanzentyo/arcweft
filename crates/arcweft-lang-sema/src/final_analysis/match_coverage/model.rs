//! Private Match matrix, constructor, domain, and witness-support algebra.

use super::super::canonical_literal::CanonicalCoverageLiteral;
use super::super::model::AcceptedProjectItemSemanticId;
use crate::{
    semantic_coordinate::{StablePatternCoordinate, StableSemanticCoordinate},
    types::{
        AcceptedVariantCaseSemanticId, AcceptedVariantPayloadFieldSemanticId, SemanticTypeDigest,
        TypeKind, VariantPayloadShape,
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedSequencePartitionWitness {
    Exact(u64),
    Interval {
        lower: u64,
        upper_exclusive: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedCoverageWitness {
    Unit,
    Bool(bool),
    Literal(CanonicalCoverageLiteral),
    Entity(AcceptedProjectItemSemanticId),
    Other {
        type_digest: SemanticTypeDigest,
    },
    Variant {
        case: AcceptedVariantCaseSemanticId,
        payload: CheckedVariantCoverageWitness,
    },
    Tuple(Box<[CheckedCoverageWitness]>),
    Record {
        owner: SemanticTypeDigest,
        fields: Box<[CheckedCoverageWitness]>,
    },
    Array(Box<[CheckedCoverageWitness]>),
    Sequence {
        partition: CheckedSequencePartitionWitness,
        visible_prefix: Box<[CheckedCoverageWitness]>,
    },
    Choice {
        ordinal: u32,
        alternative: SemanticTypeDigest,
        value: Box<CheckedCoverageWitness>,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedVariantCoverageWitness {
    Unit,
    Tuple(Box<[CheckedCoverageWitness]>),
    Record(Box<[CheckedVariantRecordCoverageWitnessField]>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CheckedVariantRecordCoverageWitnessField {
    pub(super) semantic_id: AcceptedVariantPayloadFieldSemanticId,
    pub(super) value: CheckedCoverageWitness,
}

pub(super) type Matrix = Vec<PatternVector>;
pub(super) type PatternVector = Vec<DeconstructedPattern>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DeconstructedPattern {
    pub(super) kind: DeconstructedPatternKind,
    pub(super) coordinate: StablePatternCoordinate,
    pub(super) semantic_coordinate: StableSemanticCoordinate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DeconstructedPatternKind {
    Wildcard,
    Constructor {
        constructor: CoverageConstructorId,
        fields: Box<[DeconstructedPattern]>,
    },
    Or(Box<[DeconstructedPattern]>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum CoverageConstructorId {
    Unit,
    Bool(bool),
    Variant {
        owner: SemanticTypeDigest,
        case: AcceptedVariantCaseSemanticId,
        ordinal: u32,
    },
    Tuple {
        owner: SemanticTypeDigest,
    },
    Record {
        owner: SemanticTypeDigest,
    },
    Array {
        owner: SemanticTypeDigest,
        length: u64,
    },
    Sequence {
        owner: SemanticTypeDigest,
        partition: SequencePartition,
    },
    Choice {
        owner: SemanticTypeDigest,
        ordinal: u32,
        alternative: SemanticTypeDigest,
    },
    Literal(CanonicalCoverageLiteral),
    Entity {
        owner: SemanticTypeDigest,
        item: AcceptedProjectItemSemanticId,
    },
    Other(SemanticTypeDigest),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum SequencePartition {
    Exact(u64),
    Interval {
        lower: u64,
        upper_exclusive: Option<u64>,
    },
}

impl SequencePartition {
    pub(super) const fn lower(self) -> u64 {
        match self {
            Self::Exact(value) => value,
            Self::Interval { lower, .. } => lower,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CoverageConstructor {
    pub(super) identity: CoverageConstructorId,
    pub(super) field_types: Box<[TypeKind]>,
    pub(super) variant_payload: Option<VariantPayloadShape>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CoverageTypeDomain {
    Empty,
    Constructors(Box<[CoverageConstructor]>),
}

pub(super) struct ExpandedPattern {
    pub(super) pattern: DeconstructedPattern,
    pub(super) alternative: Option<StablePatternCoordinate>,
}

impl DeconstructedPattern {
    pub(super) fn wildcard(
        coordinate: StablePatternCoordinate,
        semantic_coordinate: StableSemanticCoordinate,
    ) -> Self {
        Self {
            kind: DeconstructedPatternKind::Wildcard,
            coordinate,
            semantic_coordinate,
        }
    }
}

impl CoverageConstructor {
    pub(super) fn nullary(identity: CoverageConstructorId) -> Self {
        Self {
            identity,
            field_types: Box::new([]),
            variant_payload: None,
        }
    }
}
