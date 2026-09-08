//! Generation-bound terminal facts for the once-only pipe binding.

use arcweft_lang_hir::identity::ExprId;

use crate::semantic_coordinate::{CheckedSemanticPath, SemanticCoordinateEncodingError};
use crate::types::SemanticTypeDigest;

const CHECKED_PIPE_BINDING_IDENTITY_DOMAIN: &[u8] = b"arcweft.lang.checked-pipe-binding.v1\0";
const CHECKED_PIPE_EVALUATION_CONTRACT_TAG: u8 = 0;

/// Opaque semantic identity of one accepted once-only pipe binding.
///
/// The tuple field is private and this type intentionally has no Serde
/// implementation. Only the owner-bound seal may issue an identity; its
/// bytes are exposed after sealing for transcript and runtime consumers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedPipeBindingIdentity([u8; 32]);

impl CheckedPipeBindingIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn issue(
        owner: &CheckedSemanticPath,
        value_type: SemanticTypeDigest,
    ) -> Result<Self, SemanticCoordinateEncodingError> {
        let coordinate = owner.canonical_bytes()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHECKED_PIPE_BINDING_IDENTITY_DOMAIN);
        hasher.update(&coordinate);
        hasher.update(value_type.as_bytes());
        hasher.update(&[CHECKED_PIPE_EVALUATION_CONTRACT_TAG]);
        Ok(Self(*hasher.finalize().as_bytes()))
    }
}

/// One source-order occurrence of a `^` placeholder bound by a pipe.
///
/// `lookup_expression` is retained only as generation-local validation
/// evidence. Stable identity and transcript consumers use the accepted
/// coordinate and source-order ordinal.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedPipeLeftOccurrence {
    lookup_expression: ExprId,
    coordinate: CheckedSemanticPath,
    ordinal: u32,
}

impl CheckedPipeLeftOccurrence {
    pub(crate) const fn new(
        lookup_expression: ExprId,
        coordinate: CheckedSemanticPath,
        ordinal: u32,
    ) -> Self {
        Self {
            lookup_expression,
            coordinate,
            ordinal,
        }
    }

    pub(in crate::final_analysis) const fn lookup_expression(&self) -> ExprId {
        self.lookup_expression
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

/// Final checked once-only pipe binding and its source-order `^` uses.
///
/// The two lookup expressions are validation-only HIR evidence. The accepted
/// owner coordinate, binding identity, value type, and occurrence rows are
/// the semantic payload consumed by transcript and downstream authorities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPipe {
    lookup_left: ExprId,
    lookup_right: ExprId,
    coordinate: CheckedSemanticPath,
    binding_identity: CheckedPipeBindingIdentity,
    value_type: SemanticTypeDigest,
    occurrences: Box<[CheckedPipeLeftOccurrence]>,
}

impl CheckedPipe {
    pub(crate) fn seal_owner_bound(
        lookup_left: ExprId,
        lookup_right: ExprId,
        coordinate: CheckedSemanticPath,
        value_type: SemanticTypeDigest,
        occurrences: impl Into<Box<[CheckedPipeLeftOccurrence]>>,
    ) -> Result<Self, SemanticCoordinateEncodingError> {
        let binding_identity = CheckedPipeBindingIdentity::issue(&coordinate, value_type)?;
        Ok(Self {
            lookup_left,
            lookup_right,
            coordinate,
            binding_identity,
            value_type,
            occurrences: occurrences.into(),
        })
    }

    pub(in crate::final_analysis) const fn lookup_left(&self) -> ExprId {
        self.lookup_left
    }

    pub(in crate::final_analysis) const fn lookup_right(&self) -> ExprId {
        self.lookup_right
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }

    pub const fn binding_identity(&self) -> CheckedPipeBindingIdentity {
        self.binding_identity
    }

    pub const fn identity(&self) -> CheckedPipeBindingIdentity {
        self.binding_identity()
    }

    pub const fn value_type(&self) -> SemanticTypeDigest {
        self.value_type
    }

    pub(in crate::final_analysis) const fn occurrences(&self) -> &[CheckedPipeLeftOccurrence] {
        &self.occurrences
    }
}

/// Final checked meaning of one `^` placeholder.
///
/// The binding identity and occurrence ordinal identify the owning pipe;
/// there is deliberately no raw parent `ExprId` in this terminal payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedPipeLeft {
    binding_identity: CheckedPipeBindingIdentity,
    occurrence_ordinal: u32,
    value_type: SemanticTypeDigest,
}

impl CheckedPipeLeft {
    pub(crate) const fn new(
        binding_identity: CheckedPipeBindingIdentity,
        occurrence_ordinal: u32,
        value_type: SemanticTypeDigest,
    ) -> Self {
        Self {
            binding_identity,
            occurrence_ordinal,
            value_type,
        }
    }

    pub const fn binding_identity(&self) -> CheckedPipeBindingIdentity {
        self.binding_identity
    }

    pub const fn identity(&self) -> CheckedPipeBindingIdentity {
        self.binding_identity()
    }

    pub const fn occurrence_ordinal(&self) -> u32 {
        self.occurrence_ordinal
    }

    pub const fn value_type(&self) -> SemanticTypeDigest {
        self.value_type
    }
}
