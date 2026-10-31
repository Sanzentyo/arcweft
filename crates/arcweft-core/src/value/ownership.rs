//! Structural ownership classification for executable runtime values.
//!
//! This module owns the generic classification used by structured execution,
//! AWBC, snapshots, and every affine opaque-handle leaf. New value containers
//! and affine leaf classes must extend the exhaustive traversals below rather
//! than introduce a side table.

use super::{RuntimeCallableValue, RuntimeHandleKind, RuntimeIterator, RuntimeSeq, RuntimeValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An identity operation whose successful result carries a checked Copy proof.
/// Rejection leaves the caller's owning argument packet untouched.
pub(crate) fn evaluate_ownership_intrinsic(
    intrinsic: super::RuntimeIntrinsic,
    args: &mut Vec<RuntimeValue>,
) -> Result<Option<RuntimeValue>, super::RuntimeEvalError> {
    if intrinsic != super::RuntimeIntrinsic::CoreRequireUnrestricted {
        return Ok(None);
    }
    let [value] = args.as_slice() else {
        return Err(super::RuntimeEvalError::UnsupportedPure {
            name: intrinsic.as_label().to_owned(),
            reason: "expected exactly one owned value".to_owned(),
        });
    };
    if !RuntimeValueOwnershipRequirement::Unrestricted.accepts(value.ownership()) {
        return Err(RuntimeValueOwnershipAdmissionError::AffineValue.into());
    }
    Ok(args.pop())
}

#[allow(dead_code, reason = "the canonical snapshot consumer lands in G1.2-D")]
mod binary;
mod path;
mod resources;
mod slot;

pub use path::{
    MAX_RUNTIME_VALUE_PATH_SEGMENTS, RuntimeValuePath, RuntimeValuePathError,
    RuntimeValuePathSegment,
};
pub use slot::RuntimeOwnedSlotId;

/// Whether a runtime value may be duplicated without transferring authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeValueOwnership {
    /// The complete transitive value graph may be copied.
    Unrestricted,
    /// The transitive graph contains at least one single-owner leaf.
    Affine,
}

/// The required custody of a complete incoming or retained value graph.
/// Its carrier type alone need not determine whether it can be copied.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeValueOwnershipRequirement {
    /// Transfers custody without granting duplication of affine descendants.
    #[default]
    Owned,
    /// Requires every descendant to permit duplication.
    Unrestricted,
}

/// A resource or custody failure found before constructing or copying a value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeValueOwnershipAdmissionError {
    /// The shared resource allowance could not admit the complete graph.
    #[error(transparent)]
    Resource(Box<crate::entry::RuntimeSchemaError>),
    /// The graph contains an affine node in an unrestricted storage role.
    #[error("an unrestricted value graph is required")]
    AffineValue,
}

impl RuntimeValueOwnershipRequirement {
    /// Checks an already established transitive ownership classification.
    #[must_use]
    pub const fn accepts(self, ownership: RuntimeValueOwnership) -> bool {
        match self {
            Self::Owned => true,
            Self::Unrestricted => ownership.permits_copy(),
        }
    }

    /// One resource traversal checks every node before ownership is admitted.
    pub(crate) fn admit(
        self,
        value: super::RuntimeValueView<'_>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
        depth: usize,
    ) -> Result<(), RuntimeValueOwnershipAdmissionError> {
        let mut affine = false;
        let mut observe = |value: super::RuntimeValueView<'_>| {
            affine |= !self.accepts(value.intrinsic_ownership());
            Ok(())
        };
        resources
            .validate_view(value, depth, Some(&mut observe))
            .map_err(|cause| RuntimeValueOwnershipAdmissionError::Resource(Box::new(cause)))?;
        if affine {
            return Err(RuntimeValueOwnershipAdmissionError::AffineValue);
        }
        Ok(())
    }
}

/// One typed affine line handle with its canonical path in the containing
/// runtime value graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeAffineLineHandle {
    kind: RuntimeHandleKind,
    token: crate::runtime_id::RuntimeLineHandleToken,
    path: RuntimeValuePath,
}

impl RuntimeAffineLineHandle {
    pub(crate) const fn kind(&self) -> RuntimeHandleKind {
        self.kind
    }

    pub(crate) const fn token(&self) -> &crate::runtime_id::RuntimeLineHandleToken {
        &self.token
    }

    pub(crate) const fn path(&self) -> &RuntimeValuePath {
        &self.path
    }
}

/// Closed resource families observed by the single live/inert custody walk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAffineHandle {
    Line(RuntimeAffineLineHandle),
    Presentation {
        token: crate::runtime_id::RuntimePresentationHandleToken,
        path: RuntimeValuePath,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeAffineHandleError {
    #[error(transparent)]
    Schema(Box<crate::entry::RuntimeSchemaError>),
    #[error(transparent)]
    Path(#[from] RuntimeValuePathError),
    #[error(transparent)]
    Token(#[from] crate::runtime_id::RuntimeLineHandleTokenDecodeError),
    #[error(transparent)]
    PresentationToken(#[from] crate::runtime_id::RuntimePresentationHandleTokenDecodeError),
    #[error(
        "presentation resource {kind:?} requires its presentation ledger, not a dialogue ledger"
    )]
    PresentationLedgerRequired {
        kind: arcweft_interaction_model::presentation::PresentationResourceKind,
    },
    #[error(transparent)]
    RecordField(#[from] super::RuntimeRecordFieldIdError),
    #[error("runtime value structural ordinal exceeds the canonical u32 path coordinate")]
    StructuralOrdinalOverflow,
}

impl From<crate::entry::RuntimeSchemaError> for RuntimeAffineHandleError {
    fn from(error: crate::entry::RuntimeSchemaError) -> Self {
        Self::Schema(Box::new(error))
    }
}

/// A bare value cannot transfer the ledger obligations of a live resource handle.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDetachedValueError {
    #[error(transparent)]
    HandleGraph(#[from] RuntimeAffineHandleError),
    #[error("line handle {token:?} at {path:?} requires its owning ledger")]
    LineHandleCustodyRequired {
        token: crate::runtime_id::RuntimeLineHandleToken,
        path: RuntimeValuePath,
    },
    #[error("presentation handle {token:?} at {path:?} requires its owning ledger")]
    PresentationHandleCustodyRequired {
        token: crate::runtime_id::RuntimePresentationHandleToken,
        path: RuntimeValuePath,
    },
    #[error("Need {correlation:?} at {path:?} requires its retained producer context")]
    NeedProducerCustodyRequired {
        correlation: crate::task::TaskCorrelation,
        path: RuntimeValuePath,
    },
}

#[derive(Default)]
struct RuntimeValueResourceGraph {
    handles: Vec<RuntimeAffineHandle>,
    owned_need: Option<(crate::task::TaskCorrelation, RuntimeValuePath)>,
}

impl RuntimeValueResourceGraph {
    fn into_line_handles(self) -> Result<Vec<RuntimeAffineLineHandle>, RuntimeAffineHandleError> {
        self.handles
            .into_iter()
            .map(|handle| match handle {
                RuntimeAffineHandle::Line(handle) => Ok(handle),
                RuntimeAffineHandle::Presentation { token, .. } => {
                    Err(RuntimeAffineHandleError::PresentationLedgerRequired {
                        kind: token.resource(),
                    })
                }
            })
            .collect()
    }
}

impl RuntimeValueOwnership {
    /// Joins two transitive ownership classifications.
    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unrestricted, Self::Unrestricted) => Self::Unrestricted,
            _ => Self::Affine,
        }
    }

    /// Returns whether language-level copying is permitted.
    #[must_use]
    pub const fn permits_copy(self) -> bool {
        matches!(self, Self::Unrestricted)
    }
}

impl RuntimeValue {
    /// Checks a transfer that carries a value without an execution-resource
    /// owner. External Need identities remain valid; issued resource handles must
    /// travel with their ledger instead of becoming detached values.
    pub fn validate_detached_custody(&self) -> Result<(), RuntimeDetachedValueError> {
        let mut graph = RuntimeValueResourceGraph::default();
        self.collect_resources(&RuntimeValuePath::root(), &mut graph)?;
        if let Some(handle) = graph.handles.into_iter().next() {
            return Err(match handle {
                RuntimeAffineHandle::Line(handle) => {
                    RuntimeDetachedValueError::LineHandleCustodyRequired {
                        token: handle.token,
                        path: handle.path,
                    }
                }
                RuntimeAffineHandle::Presentation { token, path, .. } => {
                    RuntimeDetachedValueError::PresentationHandleCustodyRequired { token, path }
                }
            });
        }
        if let Some((correlation, path)) = graph.owned_need {
            return Err(RuntimeDetachedValueError::NeedProducerCustodyRequired {
                correlation,
                path,
            });
        }
        Ok(())
    }

    /// Computes ownership from the complete executable value graph.
    ///
    /// The exhaustive recursive traversal is the sole authority and makes a
    /// future value variant a compile-time obligation here.
    #[must_use]
    pub fn ownership(&self) -> RuntimeValueOwnership {
        match self {
            Self::Unit
            | Self::Bool(_)
            | Self::Int(_)
            | Self::UInt(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::MatrixF32(_)
            | Self::MatrixF64(_)
            | Self::TensorF32(_)
            | Self::TensorF64(_)
            | Self::String(_)
            | Self::Color(_)
            | Self::Char(_)
            | Self::Duration(_)
            | Self::Progress(_)
            | Self::Range(_)
            | Self::EntityRef(_)
            | Self::NeedHandle(_) => self.view().intrinsic_ownership(),
            Self::Iterator(iterator) => iterator.ownership(),
            Self::Tuple(values) => values_ownership(values),
            Self::Seq(sequence) => sequence.ownership(),
            Self::Record(fields) => fields
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, field| {
                    ownership.join(field.value().ownership())
                }),
            Self::NominalRecord(record) => values_ownership(record.fields()),
            Self::Opaque(value) => self
                .view()
                .intrinsic_ownership()
                .join(value.payload().ownership()),
            Self::Reduction(value) => value
                .commands()
                .iter()
                .fold(value.state().ownership(), |ownership, command| {
                    ownership.join(command.payload().0.ownership())
                }),
            Self::Agent(value) => value.ownership(),
            Self::Callable(callable) => callable.ownership(),
            Self::Variant(crate::value::RuntimeVariantValue { payload, .. }) => payload
                .as_deref()
                .map_or(RuntimeValueOwnership::Unrestricted, RuntimeValue::ownership),
        }
    }

    pub(crate) fn affine_line_handles(
        &self,
    ) -> Result<Vec<RuntimeAffineLineHandle>, RuntimeAffineHandleError> {
        let mut handles = RuntimeValueResourceGraph::default();
        self.collect_resources(&RuntimeValuePath::root(), &mut handles)?;
        handles.into_line_handles()
    }

    pub(crate) fn affine_handles(
        &self,
    ) -> Result<Vec<RuntimeAffineHandle>, RuntimeAffineHandleError> {
        self.view().affine_handles()
    }

    fn collect_resources(
        &self,
        path: &RuntimeValuePath,
        graph: &mut RuntimeValueResourceGraph,
    ) -> Result<(), RuntimeAffineHandleError> {
        self.view().collect_resources(path, graph)
    }
}
impl RuntimeCallableValue {
    /// Computes ownership from the exact retained value set.
    #[must_use]
    pub fn ownership(&self) -> RuntimeValueOwnership {
        values_ownership(self.retained())
    }
}

impl RuntimeSeq {
    /// Computes ownership in canonical logical storage order.
    #[must_use]
    pub fn ownership(&self) -> RuntimeValueOwnership {
        match self {
            Self::Values(values) => values_ownership(values),
            Self::Dense(_) => RuntimeValueOwnership::Unrestricted,
            Self::TupleColumns(columns) => columns
                .columns()
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, column| {
                    ownership.join(column.ownership())
                }),
            Self::RecordColumns(records) => records
                .fields()
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |ownership, field| {
                    ownership.join(field.values().ownership())
                }),
        }
    }
}

impl RuntimeIterator {
    pub(crate) fn ownership(&self) -> RuntimeValueOwnership {
        match self {
            Self::Values { items } => values_ownership(items),
            Self::Range(_) => RuntimeValueOwnership::Unrestricted,
            Self::Witness(witness) => witness.state().ownership(),
        }
    }
}

fn values_ownership<'a>(
    values: impl IntoIterator<Item = &'a RuntimeValue>,
) -> RuntimeValueOwnership {
    values
        .into_iter()
        .fold(RuntimeValueOwnership::Unrestricted, |ownership, value| {
            ownership.join(value.ownership())
        })
}

#[cfg(test)]
mod presentation_custody_tests {
    use super::*;
    use crate::pattern::{
        RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    };
    use crate::runtime_id::{
        ExecutionInstanceId, RuntimePersistentFiberId, RuntimePresentationHandleToken,
    };
    use crate::value::{
        AwbcRuntimeValueSnapshot, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
    };
    use arcweft_interaction_model::presentation::PresentationResourceKind;
    use std::num::NonZeroU64;

    #[test]
    fn presentation_custody_preserves_family_token_and_path_in_live_and_inert_storage() {
        for kind in PresentationResourceKind::ALL {
            let token = RuntimePresentationHandleToken::from_issued(
                crate::effect::RuntimeArtifactFingerprint::try_from_bytes([31; 32]).unwrap(),
                kind,
                ExecutionInstanceId::from_allocated(NonZeroU64::new(7).unwrap()),
                RuntimePersistentFiberId::from_allocated(4),
                NonZeroU64::new(9).unwrap(),
                crate::runtime_id::RuntimePresentationBankId::Root,
            );
            let owner = RuntimeOpaqueTypeOwner::exact_with(
                RuntimeOpaqueTypeProducerId::try_new("test.presentation.custody").unwrap(),
                RuntimeSemanticTypeId::from_bytes([kind.semantic_tag() + 1; 32]),
                RuntimeOpaqueValueClass::PresentationHandle,
                RuntimeOpaquePersistence::SnapshotOnly,
            );
            let value = RuntimeValue::Tuple(vec![owner.try_wrap(token.encode_payload()).unwrap()]);
            assert_eq!(value.ownership(), RuntimeValueOwnership::Affine);
            assert_eq!(
                value.try_canonical_bytes(1024),
                Err(crate::entry::RuntimeSchemaError::Encoding {
                    message: "opaque value class/persistence is not constant-admissible".to_owned(),
                })
            );
            let image = AwbcRuntimeValueSnapshot::from_standalone_value(&value).unwrap();
            let live = value.affine_handles().unwrap();
            assert_eq!(live, image.view().affine_handles().unwrap());
            let expected_path = RuntimeValuePath::root()
                .child(RuntimeValuePathSegment::TupleElement(0))
                .unwrap();
            assert_eq!(
                live,
                [RuntimeAffineHandle::Presentation {
                    token: token.clone(),
                    path: expected_path.clone()
                }]
            );
            assert_eq!(
                value.validate_detached_custody(),
                Err(
                    RuntimeDetachedValueError::PresentationHandleCustodyRequired {
                        token: token.clone(),
                        path: expected_path
                    }
                )
            );
            assert_eq!(
                value.affine_line_handles(),
                Err(RuntimeAffineHandleError::PresentationLedgerRequired { kind })
            );
            assert_eq!(
                image.view().affine_line_handles(),
                Err(RuntimeAffineHandleError::PresentationLedgerRequired { kind })
            );
            let program = crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::new(
                crate::awbc::schema::AwbcProgram::default(),
            ));
            assert_eq!(
                image.into_unrestricted_runtime_value_for_program(&program),
                Err(crate::value::AwbcRuntimeValueSnapshotError::Resource(
                    crate::entry::RuntimeSchemaError::PresentationHandleCustody {
                        path: "$[0]".into(),
                        token,
                        duplicate: false,
                    }
                ))
            );
        }
    }

    #[test]
    fn presentation_owner_rejects_malformed_live_and_decoded_payloads_before_publication() {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            RuntimeOpaqueTypeProducerId::try_new("test.presentation.custody").unwrap(),
            RuntimeSemanticTypeId::from_bytes([11; 32]),
            RuntimeOpaqueValueClass::PresentationHandle,
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        assert_eq!(
            owner.try_wrap(RuntimeValue::Unit),
            Err(crate::value::RuntimeOpaqueValueError::PresentationToken(
                crate::runtime_id::RuntimePresentationHandleTokenDecodeError::Shape,
            ))
        );
        let forged = crate::value::RuntimeOpaqueValue::new_exact(&owner, RuntimeValue::Unit);
        let decoded = serde_json::from_value::<crate::value::RuntimeOpaqueValue>(
            serde_json::to_value(&forged).unwrap(),
        )
        .unwrap();
        assert!(!owner.accepts_opaque_value(&decoded));
        assert!(
            !crate::pattern::RuntimeCheckedType::Opaque { owner }
                .accepts_value(&RuntimeValue::Opaque(decoded))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimeVariantIdentity;
    use crate::value::TupleSeq;

    #[test]
    fn unrestricted_guard_returns_the_same_owned_allocation() {
        let value = String::from("one owning allocation");
        let address = value.as_ptr();
        let mut args = vec![RuntimeValue::String(value)];
        let Some(RuntimeValue::String(value)) = evaluate_ownership_intrinsic(
            super::super::RuntimeIntrinsic::CoreRequireUnrestricted,
            &mut args,
        )
        .unwrap() else {
            panic!("owned String result");
        };
        assert_eq!(value.as_ptr(), address);
        assert!(args.is_empty());
    }

    #[test]
    fn unrestricted_guard_rejects_actual_affine_custody_without_consuming_the_packet() {
        let issued = crate::tests::program_custody::issued_program_handle();
        let mut args = vec![RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(true),
            issued.value,
        ])];
        let rejected = evaluate_ownership_intrinsic(
            super::super::RuntimeIntrinsic::CoreRequireUnrestricted,
            &mut args,
        )
        .unwrap_err();
        assert_eq!(
            rejected,
            super::super::RuntimeEvalError::ValueOwnership(
                RuntimeValueOwnershipAdmissionError::AffineValue
            )
        );
        assert_eq!(args.len(), 1);
        let handles = args[0].affine_line_handles().unwrap();
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].token(), &issued.token);
    }

    #[test]
    fn join_is_affine_if_either_side_is_affine() {
        assert_eq!(
            RuntimeValueOwnership::Unrestricted.join(RuntimeValueOwnership::Unrestricted),
            RuntimeValueOwnership::Unrestricted
        );
        assert_eq!(
            RuntimeValueOwnership::Unrestricted.join(RuntimeValueOwnership::Affine),
            RuntimeValueOwnership::Affine
        );
        assert_eq!(
            RuntimeValueOwnership::Affine.join(RuntimeValueOwnership::Unrestricted),
            RuntimeValueOwnership::Affine
        );
        assert_eq!(
            RuntimeValueOwnership::Affine.join(RuntimeValueOwnership::Affine),
            RuntimeValueOwnership::Affine
        );
        assert!(RuntimeValueOwnership::Unrestricted.permits_copy());
        assert!(!RuntimeValueOwnership::Affine.permits_copy());
    }

    #[test]
    fn current_nested_value_graph_is_recursively_unrestricted() {
        let value = RuntimeValue::Tuple(vec![
            RuntimeValue::try_record(vec![(
                "payload".to_owned(),
                RuntimeValue::Variant(crate::value::RuntimeVariantValue {
                    owner: RuntimeVariantIdentity::Builtin(
                        crate::pattern::RuntimeBuiltinVariantIdentity::Result,
                    ),
                    ordinal: 0,
                    name: "Ok".to_owned(),
                    payload: Some(Box::new(RuntimeValue::Seq(RuntimeSeq::values(vec![
                        RuntimeValue::String("value".to_owned()),
                    ])))),
                    type_instantiation: None,
                }),
            )])
            .unwrap(),
            RuntimeValue::Bool(true),
        ]);

        assert_eq!(value.ownership(), RuntimeValueOwnership::Unrestricted);
    }

    #[test]
    fn need_handle_makes_its_entire_value_graph_affine() {
        let handle = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.profile"));
        assert_eq!(handle.ownership(), RuntimeValueOwnership::Affine);
        assert_eq!(
            RuntimeValue::Tuple(vec![RuntimeValue::Bool(true), handle]).ownership(),
            RuntimeValueOwnership::Affine
        );
    }

    #[test]
    fn columnar_sequence_ownership_uses_stored_column_order() {
        let tuple = RuntimeSeq::TupleColumns(
            TupleSeq::new(
                1,
                vec![
                    RuntimeSeq::values(vec![RuntimeValue::u32(1)]),
                    RuntimeSeq::values(vec![RuntimeValue::String("x".to_owned())]),
                ],
            )
            .unwrap(),
        );
        let record = RuntimeSeq::record_columns(1, vec![("field".to_owned(), tuple)]).unwrap();

        assert_eq!(record.ownership(), RuntimeValueOwnership::Unrestricted);
    }

    #[test]
    fn live_and_inert_custody_preserve_the_same_physical_column_path() {
        let issued = crate::tests::program_custody::issued_program_handle();
        let owner = crate::task::RuntimeProgramOwner::Plan(std::sync::Arc::clone(&issued.plan));
        let column = RuntimeSeq::TupleColumns(
            TupleSeq::new(1, vec![RuntimeSeq::values(vec![issued.value])]).unwrap(),
        );
        let value = RuntimeValue::Tuple(vec![RuntimeValue::Seq(
            RuntimeSeq::record_columns(1, vec![("handle".into(), column)]).unwrap(),
        )]);
        let image =
            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner)
                .unwrap();
        let live = value.affine_line_handles().unwrap();
        let inert = image.view().affine_line_handles().unwrap();
        assert_eq!(live, inert);
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].token(), &issued.token);
        let expected = RuntimeValuePath::root()
            .child(RuntimeValuePathSegment::TupleElement(0))
            .unwrap()
            .child(RuntimeValuePathSegment::RecordColumn(
                crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
            ))
            .unwrap()
            .child(RuntimeValuePathSegment::TupleColumn(0))
            .unwrap()
            .child(RuntimeValuePathSegment::SequenceElement(0))
            .unwrap();
        assert_eq!(live[0].path(), &expected);
        assert_eq!(
            image.affine_line_handle_tokens().unwrap(),
            vec![issued.token]
        );
    }

    #[test]
    fn ownership_wire_names_are_stable() {
        assert_eq!(
            serde_json::to_string(&RuntimeValueOwnership::Unrestricted).unwrap(),
            "\"unrestricted\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeValueOwnership::Affine).unwrap(),
            "\"affine\""
        );
    }
}
