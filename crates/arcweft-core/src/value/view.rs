//! Borrowed logical values over ordinary, dense, and columnar storage.
//!
//! These views own no runtime payload and are not another value carrier.
//! Consumers inspect the same logical fields without cloning a sequence row
//! before its work and scalar limits have been checked.

mod agent;
mod dense_numeric;
mod effects;
mod iterator;
mod need;
mod nominal;
mod sequence;
pub(crate) use agent::{
    RuntimeAgentKind, RuntimeAgentView, RuntimePredicateKind, RuntimePredicateListView,
    RuntimePredicateView,
};
pub(crate) use dense_numeric::RuntimeDenseNumericView;
pub(crate) use effects::{
    RuntimeCallableView, RuntimeCommandListView, RuntimeCommandView, RuntimeOpaqueView,
    RuntimeReductionView,
};
pub(crate) use iterator::{RuntimeIteratorKind, RuntimeIteratorView};
pub(crate) use need::RuntimeNeedView;
pub(crate) use nominal::{RuntimeNominalRecordView, RuntimeVariantView};
pub(crate) use sequence::RuntimeSequenceView;

use crate::pattern::RuntimeVariantIdentity;
use crate::time::LogicalDuration;

use super::ownership::RuntimeValueOwnership;
use super::{
    DenseSeq, Progress, RecordSeqField, RuntimeColor, RuntimeEntityReference, RuntimeInt,
    RuntimeRecordFieldId, RuntimeRecordValue, RuntimeSeq, RuntimeUInt, RuntimeValue,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RuntimeScalarView<'a> {
    Unit,
    Bool(bool),
    Int(RuntimeInt),
    UInt(RuntimeUInt),
    F32(f32),
    F64(f64),
    String(&'a str),
    Color(RuntimeColor),
    Char(char),
    Duration(LogicalDuration),
    Progress { ratio: f32, label: Option<&'a str> },
    EntityRef(&'a RuntimeEntityReference),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeValueView<'a> {
    Scalar(RuntimeScalarView<'a>),
    DenseNumeric(RuntimeDenseNumericView<'a>),
    Tuple(RuntimeTupleView<'a>),
    Record(RuntimeRecordView<'a>),
    Sequence(RuntimeSequenceView<'a>),
    NominalRecord(RuntimeNominalRecordView<'a>),
    Opaque(RuntimeOpaqueView<'a>),
    Reduction(RuntimeReductionView<'a>),
    Agent(RuntimeAgentView<'a>),
    Variant(RuntimeVariantView<'a>),
    Range(&'a super::RuntimeRange),
    Iterator(RuntimeIteratorView<'a>),
    Need(RuntimeNeedView<'a>),
    Callable(RuntimeCallableView<'a>),
}

/// A live-issued logical view. Inspection is shared with snapshot admission,
/// while copying and custody checks require this live-only capability.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LiveRuntimeValueView<'a>(RuntimeValueView<'a>);

impl<'a> LiveRuntimeValueView<'a> {
    pub(crate) fn sequence(self) -> Option<&'a RuntimeSeq> {
        match self.0 {
            RuntimeValueView::Sequence(value) => value.live_value(),
            _ => None,
        }
    }
    pub(crate) const fn inspect(self) -> RuntimeValueView<'a> {
        self.0
    }

    /// Projects one logical child while retaining the parent's live provenance.
    pub(crate) fn child_at(self, index: usize) -> Option<Self> {
        let value = match self.0 {
            RuntimeValueView::Tuple(values) => values.get(index)?,
            RuntimeValueView::Record(fields) => fields.get(index)?.2,
            RuntimeValueView::Sequence(values) => values.value_view(index)?,
            RuntimeValueView::NominalRecord(record) => record.fields().get(index)?,
            RuntimeValueView::Variant(variant) if index == 0 => variant.payload()?,
            _ => return None,
        };
        Some(self.child(value))
    }

    /// Descendants are issued only from this live parent; raw inspected views
    /// cannot independently acquire a live capability.
    fn child(self, value: RuntimeValueView<'a>) -> Self {
        Self(value)
    }

    /// Materializes a second logical value only after recursive Copy proof.
    /// Columnar rows are projected through borrowed children before any
    /// physical row is constructed.
    pub(crate) fn copy_unrestricted(self) -> Option<RuntimeValue> {
        if !self.ownership().permits_copy() {
            return None;
        }
        Some(match self.0 {
            RuntimeValueView::Scalar(scalar) => match scalar {
                RuntimeScalarView::Unit => RuntimeValue::Unit,
                RuntimeScalarView::Bool(value) => RuntimeValue::Bool(value),
                RuntimeScalarView::Int(value) => RuntimeValue::Int(value),
                RuntimeScalarView::UInt(value) => RuntimeValue::UInt(value),
                RuntimeScalarView::F32(value) => RuntimeValue::F32(value),
                RuntimeScalarView::F64(value) => RuntimeValue::F64(value),
                RuntimeScalarView::String(value) => RuntimeValue::String(value.to_owned()),
                RuntimeScalarView::Color(value) => RuntimeValue::Color(value),
                RuntimeScalarView::Char(value) => RuntimeValue::Char(value),
                RuntimeScalarView::Duration(value) => RuntimeValue::Duration(value),
                RuntimeScalarView::Progress { ratio, label } => {
                    let value = Progress::new(ratio)
                        .expect("live-issued Progress retains its validated ratio");
                    RuntimeValue::Progress(match label {
                        Some(label) => value.with_label(label),
                        None => value,
                    })
                }
                RuntimeScalarView::EntityRef(value) => RuntimeValue::EntityRef(value.clone()),
            },
            RuntimeValueView::Tuple(values) => RuntimeValue::Tuple(
                (0..values.len())
                    .map(|index| self.child(values.get(index)?).copy_unrestricted())
                    .collect::<Option<Vec<_>>>()?,
            ),
            RuntimeValueView::Record(values) => RuntimeValue::Record(
                RuntimeRecordValue::try_new(
                    (0..values.len())
                        .map(|index| {
                            let (_, name, value) = values.get(index)?;
                            Some((name.to_owned(), self.child(value).copy_unrestricted()?))
                        })
                        .collect::<Option<Vec<_>>>()?,
                )
                .expect("an admitted record view retains valid field names"),
            ),
            RuntimeValueView::Sequence(values) => RuntimeValue::Seq(
                values
                    .live_value()
                    .expect("live sequence capability retains live storage")
                    .clone(),
            ),
            RuntimeValueView::NominalRecord(value) => RuntimeValue::NominalRecord(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
            RuntimeValueView::Opaque(value) => RuntimeValue::Opaque(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
            RuntimeValueView::Reduction(value) => RuntimeValue::Reduction(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
            RuntimeValueView::Agent(value) => RuntimeValue::Agent(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
            RuntimeValueView::Variant(value) => RuntimeValue::Variant(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
            RuntimeValueView::DenseNumeric(value) => value
                .live_value()
                .expect("a live numeric capability retains live storage")
                .clone(),
            RuntimeValueView::Range(value) => RuntimeValue::Range(value.clone()),
            RuntimeValueView::Iterator(value) => RuntimeValue::Iterator(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
            RuntimeValueView::Need(value) => RuntimeValue::NeedHandle(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
            RuntimeValueView::Callable(value) => RuntimeValue::Callable(
                value
                    .live_value()
                    .expect("live capability retains its original storage")
                    .clone(),
            ),
        })
    }

    /// Checks a logical borrowed row for one handle without materializing a
    /// second live value from columnar storage.
    pub(crate) fn contains_line_handle(
        self,
        token: &crate::runtime_id::RuntimeLineHandleToken,
    ) -> Result<bool, super::ownership::RuntimeAffineLineHandleError> {
        match self.0 {
            RuntimeValueView::Scalar(_) | RuntimeValueView::DenseNumeric(_) => Ok(false),
            RuntimeValueView::Tuple(values) => {
                for index in 0..values.len() {
                    if let Some(value) = values.get(index)
                        && self.child(value).contains_line_handle(token)?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RuntimeValueView::Record(values) => {
                for index in 0..values.len() {
                    if let Some((_, _, value)) = values.get(index)
                        && self.child(value).contains_line_handle(token)?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RuntimeValueView::Sequence(values) => {
                for index in 0..values.len().expect("live sequence has a platform length") {
                    if let Some(value) = values.value_view(index)
                        && self.child(value).contains_line_handle(token)?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RuntimeValueView::NominalRecord(record) => {
                record.fields().iter().try_fold(false, |found, value| {
                    Ok(found || self.child(value).contains_line_handle(token)?)
                })
            }
            RuntimeValueView::Opaque(value) => match value.value_class() {
                super::RuntimeOpaqueValueClass::AffineHandle(_) => Ok(
                    &crate::runtime_id::RuntimeLineHandleToken::try_decode_payload(
                        value
                            .live_value()
                            .expect("live opaque capability retains live storage")
                            .payload(),
                    )? == token,
                ),
                super::RuntimeOpaqueValueClass::Plain => {
                    self.child(value.payload()).contains_line_handle(token)
                }
            },
            RuntimeValueView::Reduction(value) => {
                if self.child(value.state()).contains_line_handle(token)? {
                    return Ok(true);
                }
                for command in value.commands().iter() {
                    if self.child(command.payload()).contains_line_handle(token)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RuntimeValueView::Agent(value) => {
                for (_, nested) in value
                    .live_value()
                    .expect("live Agent capability retains live storage")
                    .nested_runtime_values_with_depth()
                {
                    if nested.live_view().contains_line_handle(token)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RuntimeValueView::Variant(variant) => variant.payload().map_or(Ok(false), |value| {
                self.child(value).contains_line_handle(token)
            }),
            RuntimeValueView::Range(_) => Ok(false),
            RuntimeValueView::Need(value) => {
                for value in value.request_values() {
                    if self.child(value).contains_line_handle(token)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RuntimeValueView::Iterator(value) => match value.kind() {
                RuntimeIteratorKind::Values(items) => {
                    for value in items.iter() {
                        if self.child(value).contains_line_handle(token)? {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                RuntimeIteratorKind::Range(_) => Ok(false),
                RuntimeIteratorKind::Witness(witness) => {
                    self.child(witness.state()).contains_line_handle(token)
                }
            },
            RuntimeValueView::Callable(value) => {
                for value in value.retained().iter() {
                    if self.child(value).contains_line_handle(token)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// Ownership of a borrowed logical row, including columnar children.
    pub(crate) fn ownership(self) -> RuntimeValueOwnership {
        let join = |current: RuntimeValueOwnership, value: RuntimeValueView<'_>| {
            current.join(self.child(value).ownership())
        };
        match self.0 {
            RuntimeValueView::Scalar(_)
            | RuntimeValueView::DenseNumeric(_)
            | RuntimeValueView::Range(_)
            | RuntimeValueView::Need(_) => self.0.intrinsic_ownership(),
            RuntimeValueView::Tuple(items) => (0..items.len())
                .filter_map(|index| items.get(index))
                .fold(RuntimeValueOwnership::Unrestricted, join),
            RuntimeValueView::Record(fields) => (0..fields.len())
                .filter_map(|index| fields.get(index))
                .fold(
                    RuntimeValueOwnership::Unrestricted,
                    |current, (_, _, value)| join(current, value),
                ),
            RuntimeValueView::Sequence(sequence) => sequence
                .live_value()
                .expect("live sequence retains live storage")
                .ownership(),
            RuntimeValueView::NominalRecord(record) => record
                .fields()
                .iter()
                .fold(RuntimeValueOwnership::Unrestricted, |current, value| {
                    current.join(self.child(value).ownership())
                }),
            RuntimeValueView::Opaque(value) => self
                .0
                .intrinsic_ownership()
                .join(self.child(value.payload()).ownership()),
            RuntimeValueView::Reduction(value) => value
                .commands()
                .iter()
                .fold(self.child(value.state()).ownership(), |current, command| {
                    current.join(self.child(command.payload()).ownership())
                }),
            RuntimeValueView::Agent(value) => value
                .live_value()
                .expect("live Agent retains live storage")
                .ownership(),
            RuntimeValueView::Variant(variant) => variant
                .payload()
                .map_or(RuntimeValueOwnership::Unrestricted, |value| {
                    self.child(value).ownership()
                }),
            RuntimeValueView::Iterator(value) => value
                .live_value()
                .expect("live iterator retains live storage")
                .ownership(),
            RuntimeValueView::Callable(value) => value
                .live_value()
                .expect("live callable retains live storage")
                .ownership(),
        }
    }
}

impl<'a> RuntimeValueView<'a> {
    /// Intrinsic custody of this logical node, independent of its descendants.
    /// The graph visitor joins descendants for both live and inert values.
    pub(crate) fn intrinsic_ownership(self) -> RuntimeValueOwnership {
        match self {
            Self::Need(_) => RuntimeValueOwnership::Affine,
            Self::Opaque(value) => value.value_class().intrinsic_ownership(),
            Self::Scalar(_)
            | Self::DenseNumeric(_)
            | Self::Tuple(_)
            | Self::Record(_)
            | Self::Sequence(_)
            | Self::NominalRecord(_)
            | Self::Reduction(_)
            | Self::Agent(_)
            | Self::Variant(_)
            | Self::Range(_)
            | Self::Iterator(_)
            | Self::Callable(_) => RuntimeValueOwnership::Unrestricted,
        }
    }

    pub(crate) fn builtin_variant_case(
        self,
    ) -> Option<(
        crate::pattern::RuntimeBuiltinVariantCaseIdentity,
        Option<Self>,
    )> {
        let Self::Variant(variant) = self else {
            return None;
        };
        let RuntimeVariantIdentity::Builtin(owner) = variant.owner() else {
            return None;
        };
        let case = owner.case_at(variant.ordinal())?;
        if case.name() != variant.name() || variant.type_instantiation().is_some() {
            return None;
        }
        let payload = match (case.payload_arity(), variant.payload()) {
            (None, None) => None,
            (Some(arity), Some(Self::Tuple(fields))) if fields.len() == arity => fields.get(0),
            _ => return None,
        };
        Some((case.identity(), payload))
    }
    /// Borrows the construction witness for a header with scoped nominal identity.
    pub(crate) fn construction_instantiation(
        self,
    ) -> Option<(
        crate::pattern::RuntimeSemanticTypeId,
        &'a crate::program_types::RuntimeFunctionEffectInstantiation,
    )> {
        match self {
            Self::NominalRecord(record) => {
                Some((record.semantic_identity(), record.type_instantiation()?))
            }
            Self::Variant(variant) => match variant.owner() {
                RuntimeVariantIdentity::Nominal {
                    semantic_identity, ..
                } => Some((*semantic_identity, variant.type_instantiation()?)),
                RuntimeVariantIdentity::Builtin(_) => None,
            },
            _ => None,
        }
    }
    pub(crate) fn type_name(self) -> &'static str {
        match self {
            Self::Scalar(RuntimeScalarView::Unit) => "unit",
            Self::Scalar(RuntimeScalarView::Bool(_)) => "bool",
            Self::Scalar(RuntimeScalarView::Int(_)) => "signed integer",
            Self::Scalar(RuntimeScalarView::UInt(_)) => "unsigned integer",
            Self::Scalar(RuntimeScalarView::F32(_)) => "f32",
            Self::Scalar(RuntimeScalarView::F64(_)) => "f64",
            Self::Scalar(RuntimeScalarView::String(_)) => "string",
            Self::Scalar(RuntimeScalarView::Color(_)) => "color",
            Self::Scalar(RuntimeScalarView::Char(_)) => "char",
            Self::Scalar(RuntimeScalarView::Duration(_)) => "duration",
            Self::Scalar(RuntimeScalarView::Progress { .. }) => "progress",
            Self::Scalar(RuntimeScalarView::EntityRef(_)) => "entity reference",
            Self::Tuple(_) => "tuple",
            Self::Record(_) => "record",
            Self::Sequence(_) => "sequence",
            Self::NominalRecord(_) => "nominal record",
            Self::Opaque(_) => "opaque value",
            Self::Reduction(_) => "Reduction value",
            Self::Agent(_) => "Agent value",
            Self::Variant(_) => "variant",
            Self::DenseNumeric(value) => match value.ty() {
                crate::math::RuntimeDenseNumericType::Matrix(
                    crate::math::RuntimeFloatWidth::F32,
                ) => "f32 matrix",
                crate::math::RuntimeDenseNumericType::Matrix(
                    crate::math::RuntimeFloatWidth::F64,
                ) => "f64 matrix",
                crate::math::RuntimeDenseNumericType::Tensor(
                    crate::math::RuntimeFloatWidth::F32,
                ) => "f32 tensor",
                crate::math::RuntimeDenseNumericType::Tensor(
                    crate::math::RuntimeFloatWidth::F64,
                ) => "f64 tensor",
            },
            Self::Range(_) => "range",
            Self::Iterator(_) => "iterator",
            Self::Need(_) => "Need value",
            Self::Callable(_) => "function",
        }
    }

    /// Tests one node of the closed `AgentValue` algebra. Container descendants
    /// must satisfy the same rule; the caller owns traversal and its allowance.
    pub(crate) fn is_agent_value_node(self) -> bool {
        use RuntimeScalarView as Scalar;
        match self {
            Self::Scalar(
                Scalar::Unit
                | Scalar::Bool(_)
                | Scalar::String(_)
                | Scalar::EntityRef(_)
                | Scalar::Int(RuntimeInt::I64(_))
                | Scalar::UInt(RuntimeUInt::U64(_)),
            )
            | Self::Sequence(_)
            | Self::Record(_) => true,
            Self::Scalar(Scalar::F64(value)) => value.is_finite(),
            Self::Scalar(_)
            | Self::Tuple(_)
            | Self::NominalRecord(_)
            | Self::Opaque(_)
            | Self::Reduction(_)
            | Self::Agent(_)
            | Self::Variant(_)
            | Self::DenseNumeric(_)
            | Self::Range(_)
            | Self::Iterator(_)
            | Self::Need(_)
            | Self::Callable(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeValueSource<'a> {
    Live(&'a RuntimeValue),
    Snapshot(&'a super::AwbcRuntimeValueSnapshot),
}
impl<'a> RuntimeValueSource<'a> {
    pub(crate) fn view(self) -> RuntimeValueView<'a> {
        match self {
            Self::Live(value) => value.view(),
            Self::Snapshot(value) => value.view(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeTupleView<'a> {
    Values(&'a [RuntimeValue]),
    Deque(&'a std::collections::VecDeque<RuntimeValue>),
    Snapshots(&'a [super::AwbcRuntimeValueSnapshot]),
    NominalFields(&'a [super::awbc_save::AwbcRuntimeNominalFieldSnapshot]),
    SnapshotColumns {
        columns: &'a [super::awbc_save::AwbcRuntimeSeqSnapshot],
        row: usize,
    },
    Borrowed(&'a [&'a RuntimeValue]),
    Views(&'a [RuntimeValueView<'a>]),
    Columns {
        columns: &'a [RuntimeSeq],
        row: usize,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeRecordView<'a> {
    Values(&'a RuntimeRecordValue),
    Snapshots(&'a [super::awbc_save::AwbcRuntimeFieldSnapshot]),
    SnapshotColumns {
        fields: &'a [super::awbc_save::AwbcRuntimeRecordSeqFieldSnapshot],
        row: usize,
    },
    Columns {
        fields: &'a [RecordSeqField],
        row: usize,
    },
}

impl RuntimeValue {
    pub(crate) fn live_view(&self) -> LiveRuntimeValueView<'_> {
        LiveRuntimeValueView(self.view())
    }

    pub(crate) fn view(&self) -> RuntimeValueView<'_> {
        use RuntimeScalarView as Scalar;
        use RuntimeValueView as View;
        match self {
            Self::Unit => View::Scalar(Scalar::Unit),
            Self::Bool(value) => View::Scalar(Scalar::Bool(*value)),
            Self::Int(value) => View::Scalar(Scalar::Int(*value)),
            Self::UInt(value) => View::Scalar(Scalar::UInt(*value)),
            Self::F32(value) => View::Scalar(Scalar::F32(*value)),
            Self::F64(value) => View::Scalar(Scalar::F64(*value)),
            Self::String(value) => View::Scalar(Scalar::String(value)),
            Self::Color(value) => View::Scalar(Scalar::Color(*value)),
            Self::Char(value) => View::Scalar(Scalar::Char(*value)),
            Self::Duration(value) => View::Scalar(Scalar::Duration(*value)),
            Self::Progress(value) => View::Scalar(Scalar::Progress {
                ratio: value.ratio(),
                label: value.label(),
            }),
            Self::EntityRef(value) => View::Scalar(Scalar::EntityRef(value)),
            Self::Tuple(values) => View::Tuple(RuntimeTupleView::Values(values)),
            Self::Record(values) => View::Record(RuntimeRecordView::Values(values)),
            Self::Seq(values) => View::Sequence(RuntimeSequenceView::Live(values)),
            Self::NominalRecord(value) => {
                View::NominalRecord(RuntimeNominalRecordView::Live(value))
            }
            Self::Opaque(value) => View::Opaque(RuntimeOpaqueView::Live(value)),
            Self::Reduction(value) => View::Reduction(RuntimeReductionView::Live(value)),
            Self::Agent(value) => View::Agent(RuntimeAgentView::Live(value)),
            Self::Variant(variant) => View::Variant(RuntimeVariantView::live(variant)),
            Self::MatrixF32(_) | Self::MatrixF64(_) | Self::TensorF32(_) | Self::TensorF64(_) => {
                View::DenseNumeric(RuntimeDenseNumericView::live(self))
            }
            Self::Range(value) => View::Range(value),
            Self::Iterator(value) => View::Iterator(RuntimeIteratorView::Live(value)),
            Self::NeedHandle(value) => View::Need(RuntimeNeedView::Live(value)),
            Self::Callable(value) => View::Callable(RuntimeCallableView::Live(value)),
        }
    }
}

impl super::AwbcRuntimeValueSnapshot {
    pub(crate) fn view(&self) -> RuntimeValueView<'_> {
        use RuntimeScalarView as Scalar;
        use RuntimeValueView as View;
        match self {
            Self::Unit => View::Scalar(Scalar::Unit),
            Self::Bool(v) => View::Scalar(Scalar::Bool(*v)),
            Self::Int(v) => View::Scalar(Scalar::Int(*v)),
            Self::UInt(v) => View::Scalar(Scalar::UInt(*v)),
            Self::F32(v) => View::Scalar(Scalar::F32(f32::from_bits(*v))),
            Self::F64(v) => View::Scalar(Scalar::F64(f64::from_bits(*v))),
            Self::String(v) => View::Scalar(Scalar::String(v)),
            Self::Color(v) => View::Scalar(Scalar::Color(*v)),
            Self::Char(v) => View::Scalar(Scalar::Char(*v)),
            Self::Duration(v) => View::Scalar(Scalar::Duration(*v)),
            Self::Progress { ratio, label } => View::Scalar(Scalar::Progress {
                ratio: *ratio,
                label: label.as_deref(),
            }),
            Self::EntityRef(v) => View::Scalar(Scalar::EntityRef(v)),
            Self::DenseNumeric(v) => View::DenseNumeric(RuntimeDenseNumericView::snapshot(v)),
            Self::Range(v) => View::Range(v),
            Self::Iterator(v) => View::Iterator(RuntimeIteratorView::Snapshot(v)),
            Self::Tuple(v) => View::Tuple(RuntimeTupleView::Snapshots(v)),
            Self::Record(v) => View::Record(RuntimeRecordView::Snapshots(v)),
            Self::Seq(v) => View::Sequence(RuntimeSequenceView::Snapshot(v)),
            Self::NominalRecord(v) => View::NominalRecord(RuntimeNominalRecordView::Snapshot(v)),
            Self::Opaque(v) => View::Opaque(RuntimeOpaqueView::Snapshot(v)),
            Self::Reduction(v) => View::Reduction(RuntimeReductionView::Snapshot(v)),
            Self::Agent(v) => View::Agent(RuntimeAgentView::Snapshot(v)),
            Self::Callable(v) => View::Callable(RuntimeCallableView::Snapshot(v)),
            Self::NeedHandle(v) => View::Need(RuntimeNeedView::Snapshot(v)),
            Self::Variant {
                owner,
                ordinal,
                name,
                type_instantiation,
                payload,
            } => View::Variant(RuntimeVariantView::snapshot(
                owner,
                *ordinal,
                name,
                type_instantiation.as_ref(),
                payload.as_deref(),
            )),
        }
    }
}

impl RuntimeSeq {
    pub(crate) fn live_value_view(&self, index: usize) -> Option<LiveRuntimeValueView<'_>> {
        self.value_view(index).map(LiveRuntimeValueView)
    }

    pub(crate) fn value_view(&self, index: usize) -> Option<RuntimeValueView<'_>> {
        if index >= self.len() {
            return None;
        }
        Some(match self {
            Self::Values(values) => values[index].view(),
            Self::Dense(values) => RuntimeValueView::Scalar(values.scalar_view(index)?),
            Self::TupleColumns(values) => RuntimeValueView::Tuple(RuntimeTupleView::Columns {
                columns: values.columns(),
                row: index,
            }),
            Self::RecordColumns(values) => RuntimeValueView::Record(RuntimeRecordView::Columns {
                fields: values.fields(),
                row: index,
            }),
        })
    }
}

impl DenseSeq {
    pub(super) fn scalar_view(&self, index: usize) -> Option<RuntimeScalarView<'_>> {
        use RuntimeScalarView as Scalar;
        Some(match self {
            Self::Units(length) => {
                if index < *length {
                    Scalar::Unit
                } else {
                    return None;
                }
            }
            Self::I8(values) => Scalar::Int(RuntimeInt::I8(*values.as_slice().get(index)?)),
            Self::I16(values) => Scalar::Int(RuntimeInt::I16(*values.as_slice().get(index)?)),
            Self::I32(values) => Scalar::Int(RuntimeInt::I32(*values.as_slice().get(index)?)),
            Self::I64(values) => Scalar::Int(RuntimeInt::I64(*values.as_slice().get(index)?)),
            Self::I128(values) => Scalar::Int(RuntimeInt::I128(*values.as_slice().get(index)?)),
            Self::ISize(values) => {
                Scalar::Int(RuntimeInt::ISize(values.as_slice().get(index)?.get()))
            }
            Self::U8(values) | Self::Bytes(values) => {
                Scalar::UInt(RuntimeUInt::U8(*values.as_slice().get(index)?))
            }
            Self::U16(values) => Scalar::UInt(RuntimeUInt::U16(*values.as_slice().get(index)?)),
            Self::U32(values) => Scalar::UInt(RuntimeUInt::U32(*values.as_slice().get(index)?)),
            Self::U64(values) => Scalar::UInt(RuntimeUInt::U64(*values.as_slice().get(index)?)),
            Self::U128(values) => Scalar::UInt(RuntimeUInt::U128(*values.as_slice().get(index)?)),
            Self::USize(values) => {
                Scalar::UInt(RuntimeUInt::USize(values.as_slice().get(index)?.get()))
            }
            Self::F32(values) => Scalar::F32(*values.as_slice().get(index)?),
            Self::F64(values) => Scalar::F64(*values.as_slice().get(index)?),
            Self::Bool(values) => Scalar::Bool(*values.as_slice().get(index)?),
            Self::Chars(values) => Scalar::Char(*values.as_slice().get(index)?),
            Self::Durations(values) => Scalar::Duration(*values.as_slice().get(index)?),
            Self::Strings(values) => Scalar::String(values.as_slice().get(index)?),
            Self::EntityRefs(values) => Scalar::EntityRef(values.as_slice().get(index)?),
        })
    }
}

impl<'a> RuntimeTupleView<'a> {
    pub(crate) fn iter(self) -> impl ExactSizeIterator<Item = RuntimeValueView<'a>> {
        (0..self.len())
            .map(move |index| self.get(index).expect("value list owns each indexed child"))
    }
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Deque(values) => values.len(),
            Self::Snapshots(values) => values.len(),
            Self::NominalFields(values) => values.len(),
            Self::SnapshotColumns { columns, .. } => columns.len(),
            Self::Borrowed(values) => values.len(),
            Self::Views(values) => values.len(),
            Self::Columns { columns, .. } => columns.len(),
        }
    }

    pub(crate) fn get(self, index: usize) -> Option<RuntimeValueView<'a>> {
        match self {
            Self::Values(values) => values.get(index).map(RuntimeValue::view),
            Self::Deque(values) => values.get(index).map(RuntimeValue::view),
            Self::Snapshots(values) => values.get(index).map(super::AwbcRuntimeValueSnapshot::view),
            Self::NominalFields(values) => values.get(index).map(|field| field.value.view()),
            Self::SnapshotColumns { columns, row } => {
                RuntimeSequenceView::Snapshot(columns.get(index)?).value_view(row)
            }
            Self::Borrowed(values) => values.get(index).map(|value| value.view()),
            Self::Views(values) => values.get(index).copied(),
            Self::Columns { columns, row } => columns.get(index)?.value_view(row),
        }
    }
}

impl<'a> RuntimeRecordView<'a> {
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Snapshots(values) => values.len(),
            Self::SnapshotColumns { fields, .. } => fields.len(),
            Self::Columns { fields, .. } => fields.len(),
        }
    }

    pub(crate) fn get(
        self,
        index: usize,
    ) -> Option<(RuntimeRecordFieldId, &'a str, RuntimeValueView<'a>)> {
        match self {
            Self::Values(values) => values
                .get(index)
                .map(|field| (field.field(), field.name(), field.value().view())),
            Self::Snapshots(values) => values
                .get(index)
                .map(|field| (field.field, field.name.as_str(), field.value.view())),
            Self::SnapshotColumns { fields, row } => {
                let field = fields.get(index)?;
                Some((
                    field.field,
                    field.name.as_str(),
                    RuntimeSequenceView::Snapshot(&field.values).value_view(row)?,
                ))
            }
            Self::Columns { fields, row } => {
                let field = fields.get(index)?;
                Some((field.field(), field.name(), field.values().value_view(row)?))
            }
        }
    }
}

pub(crate) fn runtime_value_views_equal(
    left: RuntimeValueView<'_>,
    right: RuntimeValueView<'_>,
) -> bool {
    match (left, right) {
        (RuntimeValueView::Scalar(left), RuntimeValueView::Scalar(right)) => left == right,
        (RuntimeValueView::Tuple(left), RuntimeValueView::Tuple(right)) => {
            left.len() == right.len()
                && (0..left.len()).all(|index| match (left.get(index), right.get(index)) {
                    (Some(left), Some(right)) => runtime_value_views_equal(left, right),
                    _ => false,
                })
        }
        (RuntimeValueView::Record(left), RuntimeValueView::Record(right)) => {
            left.len() == right.len()
                && (0..left.len()).all(|index| match (left.get(index), right.get(index)) {
                    (Some((left_id, left_name, left)), Some((right_id, right_name, right))) => {
                        left_id == right_id
                            && left_name == right_name
                            && runtime_value_views_equal(left, right)
                    }
                    _ => false,
                })
        }
        (RuntimeValueView::Sequence(left), RuntimeValueView::Sequence(right)) => left
            .len()
            .ok()
            .zip(right.len().ok())
            .is_some_and(|(left_length, right_length)| {
                left_length == right_length
                    && (0..left_length).all(|index| {
                        match (left.value_view(index), right.value_view(index)) {
                            (Some(left), Some(right)) => runtime_value_views_equal(left, right),
                            _ => false,
                        }
                    })
            }),
        (RuntimeValueView::NominalRecord(left), RuntimeValueView::NominalRecord(right)) => {
            left.type_id() == right.type_id()
                && left.semantic_identity() == right.semantic_identity()
                && left.layout() == right.layout()
                && left.type_instantiation() == right.type_instantiation()
                && runtime_value_views_equal(
                    RuntimeValueView::Tuple(left.fields()),
                    RuntimeValueView::Tuple(right.fields()),
                )
        }
        (RuntimeValueView::Opaque(left), RuntimeValueView::Opaque(right)) => {
            match (left.live_value(), right.live_value()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
        }
        (RuntimeValueView::Reduction(left), RuntimeValueView::Reduction(right)) => {
            match (left.live_value(), right.live_value()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
        }
        (RuntimeValueView::Agent(left), RuntimeValueView::Agent(right)) => {
            match (left.live_value(), right.live_value()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
        }
        (RuntimeValueView::Variant(left), RuntimeValueView::Variant(right)) => {
            left.owner() == right.owner()
                && left.type_instantiation() == right.type_instantiation()
                && left.ordinal() == right.ordinal()
                && left.name() == right.name()
                && match (left.payload(), right.payload()) {
                    (Some(left), Some(right)) => runtime_value_views_equal(left, right),
                    (None, None) => true,
                    _ => false,
                }
        }
        (RuntimeValueView::DenseNumeric(left), RuntimeValueView::DenseNumeric(right)) => {
            left.ty() == right.ty()
                && left.rank() == right.rank()
                && left.len() == right.len()
                && (0..left.rank()).all(|index| {
                    match (left.dimension(index), right.dimension(index)) {
                        (Ok(left), Ok(right)) => left == right,
                        _ => false,
                    }
                })
                && left.values().eq(right.values())
        }
        (RuntimeValueView::Range(left), RuntimeValueView::Range(right)) => left == right,
        (RuntimeValueView::Iterator(left), RuntimeValueView::Iterator(right)) => {
            match (left.live_value(), right.live_value()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
        }
        (RuntimeValueView::Need(left), RuntimeValueView::Need(right)) => {
            match (left.live_value(), right.live_value()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
        }
        (RuntimeValueView::Callable(left), RuntimeValueView::Callable(right)) => {
            match (left.live_value(), right.live_value()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
        }
        _ => false,
    }
}
