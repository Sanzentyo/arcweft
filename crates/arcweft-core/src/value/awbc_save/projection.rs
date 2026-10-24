//! Save projection over already-admitted borrowed logical values.

use super::{
    AwbcRuntimeFieldSnapshot, AwbcRuntimeOpaqueSnapshot, AwbcRuntimeSeqSnapshot,
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeProgramOwner,
};
use crate::value::{
    RuntimeAgentView, RuntimeCallableView, RuntimeDialogueContentValue, RuntimeIteratorView,
    RuntimeNeedView, RuntimeNominalRecordView, RuntimeOpaqueValue, RuntimeRecordView,
    RuntimeReductionView, RuntimeRetainedValueAdmission, RuntimeScalarView, RuntimeSequenceView,
    RuntimeValueView, RuntimeVariantView,
};

impl AwbcRuntimeValueSnapshot {
    /// Captures a domain-owned opaque carrier after complete borrowed admission,
    /// without first copying it into a temporary `RuntimeValue` wrapper.
    pub fn from_unrestricted_opaque(
        value: &RuntimeOpaqueValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let mut admission = Self::projection_admission(owner);
        admission.validate_opaque(value)?;
        Self::opaque_from_live(value, owner).map(Self::Opaque)
    }

    /// Captures the owning Content codec directly from its borrowed fields,
    /// including callbacks, without materializing a second live envelope.
    pub fn from_unrestricted_dialogue_content(
        value: &RuntimeDialogueContentValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let mut admission = Self::projection_admission(owner);
        admission.validate_dialogue_content(value)?;
        Self::from_admitted_view(value.view(), owner)
    }

    fn projection_admission(owner: Option<&RuntimeProgramOwner>) -> RuntimeRetainedValueAdmission {
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
        match owner {
            Some(owner) => RuntimeRetainedValueAdmission::for_program(owner.clone(), limits),
            None => RuntimeRetainedValueAdmission::standalone(limits),
        }
    }

    pub(super) fn from_admitted_view(
        value: RuntimeValueView<'_>,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        use RuntimeValueView as View;
        Ok(match value {
            View::Scalar(value) => Self::from_scalar_view(value),
            View::Tuple(values) => Self::Tuple(
                values
                    .iter()
                    .map(|value| Self::from_admitted_view(value, owner))
                    .collect::<Result<_, _>>()?,
            ),
            View::Record(fields) => Self::Record(Self::record_from_admitted_view(fields, owner)?),
            View::Sequence(value) => Self::Seq(Self::sequence_from_admitted_view(value, owner)?),
            View::NominalRecord(RuntimeNominalRecordView::Live(value)) => {
                Self::NominalRecord(Self::nominal_from_live(value, owner)?)
            }
            View::Opaque(value) => Self::Opaque(AwbcRuntimeOpaqueSnapshot {
                producer: value.producer().clone(),
                semantic_identity: value.semantic_identity(),
                value_class: value.value_class(),
                persistence: value.persistence(),
                payload: Box::new(Self::from_admitted_view(value.payload(), owner)?),
            }),
            View::Reduction(RuntimeReductionView::Live(value)) => {
                Self::Reduction(Self::reduction_from_live(value, owner)?)
            }
            View::Agent(RuntimeAgentView::Live(value)) => {
                Self::Agent(Self::agent_from_live(value, owner)?)
            }
            View::Callable(RuntimeCallableView::Live(value)) => {
                Self::Callable(Self::callable_from_live(value, owner)?)
            }
            View::Iterator(RuntimeIteratorView::Live(value)) => {
                Self::Iterator(Self::iterator_from_live(value, owner)?)
            }
            View::Need(RuntimeNeedView::Live(value)) => Self::NeedHandle(
                crate::task::RuntimeNeedHandleSaveSnapshot::from_live_admitted(value, owner)?,
            ),
            View::DenseNumeric(value) => Self::DenseNumeric(
                crate::math::RuntimeDenseNumericPayload::try_from(
                    value
                        .live_value()
                        .ok_or_else(Self::inert_projection_error)?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            ),
            View::Range(value) => Self::Range(value.clone()),
            View::Variant(value) => Self::variant_from_admitted_view(value, owner)?,
            View::NominalRecord(RuntimeNominalRecordView::Snapshot(_))
            | View::Reduction(RuntimeReductionView::Snapshot(_))
            | View::Agent(RuntimeAgentView::Snapshot(_))
            | View::Callable(RuntimeCallableView::Snapshot(_))
            | View::Iterator(RuntimeIteratorView::Snapshot(_))
            | View::Need(RuntimeNeedView::Snapshot(_)) => {
                return Err(Self::inert_projection_error());
            }
        })
    }

    fn record_from_admitted_view(
        fields: RuntimeRecordView<'_>,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Vec<AwbcRuntimeFieldSnapshot>, AwbcRuntimeValueSnapshotError> {
        (0..fields.len())
            .map(|index| {
                let (field, name, value) =
                    fields.get(index).expect("admitted record has every field");
                Ok(AwbcRuntimeFieldSnapshot {
                    field,
                    name: name.to_owned(),
                    value: Self::from_admitted_view(value, owner)?,
                })
            })
            .collect()
    }

    fn sequence_from_admitted_view(
        value: RuntimeSequenceView<'_>,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeSeqSnapshot, AwbcRuntimeValueSnapshotError> {
        match value {
            RuntimeSequenceView::Live(value) => Self::sequence_from_live(value, owner),
            RuntimeSequenceView::Bytes(value) => Ok(AwbcRuntimeSeqSnapshot::Dense(
                crate::value::DenseSeq::bytes(value.to_vec()),
            )),
            value @ (RuntimeSequenceView::DialogueBindings(_)
            | RuntimeSequenceView::DialogueEffects(_)) => Ok(AwbcRuntimeSeqSnapshot::Values(
                (0..value.len()?)
                    .map(|index| {
                        Self::from_admitted_view(
                            value
                                .value_view(index)
                                .expect("admitted dialogue sequence has every row"),
                            owner,
                        )
                    })
                    .collect::<Result<_, _>>()?,
            )),
            RuntimeSequenceView::Snapshot(_) | RuntimeSequenceView::Views(_) => {
                Err(Self::inert_projection_error())
            }
        }
    }

    fn variant_from_admitted_view(
        value: RuntimeVariantView<'_>,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        if matches!(
            value.owner(),
            crate::value::RuntimeVariantIdentity::Builtin(_)
        ) && RuntimeValueView::Variant(value)
            .builtin_variant_case()
            .is_none()
        {
            return Err(AwbcRuntimeValueSnapshotError::new(
                "invalid builtin variant snapshot",
            ));
        }
        Ok(Self::Variant {
            owner: value.owner().clone(),
            ordinal: value.ordinal(),
            name: value.name().to_owned(),
            type_instantiation: value.type_instantiation().cloned(),
            payload: value
                .payload()
                .map(|value| Self::from_admitted_view(value, owner))
                .transpose()?
                .map(Box::new),
        })
    }

    fn inert_projection_error() -> AwbcRuntimeValueSnapshotError {
        AwbcRuntimeValueSnapshotError::new("live save projection cannot activate an inert carrier")
    }

    fn from_scalar_view(value: RuntimeScalarView<'_>) -> Self {
        use RuntimeScalarView as Scalar;
        match value {
            Scalar::Unit => Self::Unit,
            Scalar::Bool(value) => Self::Bool(value),
            Scalar::Int(value) => Self::Int(value),
            Scalar::UInt(value) => Self::UInt(value),
            Scalar::F32(value) => Self::F32(value.to_bits()),
            Scalar::F64(value) => Self::F64(value.to_bits()),
            Scalar::String(value) => Self::String(value.to_owned()),
            Scalar::Color(value) => Self::Color(value),
            Scalar::Char(value) => Self::Char(value),
            Scalar::Duration(value) => Self::Duration(value),
            Scalar::Progress { ratio, label } => Self::Progress {
                ratio,
                label: label.map(str::to_owned),
            },
            Scalar::EntityRef(value) => Self::EntityRef(value.clone()),
        }
    }
}
