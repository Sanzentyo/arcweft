//! Static literal semantic encoding over the existing borrowed logical views.
//! Persistence digests normalize floating zero; executable literals retain bits.

mod traversal;

use super::{
    RuntimeAgentAction, RuntimeAgentActionDispatch, RuntimeAgentCaptureTarget,
    RuntimeAgentCompareOp, RuntimeAgentPredicate, RuntimeAgentProbe, RuntimeAgentValue,
    RuntimeCommand, RuntimeEntityReference, RuntimeInt, RuntimeIterator, RuntimeOpaquePersistence,
    RuntimeOpaqueValueClass, RuntimeRange, RuntimeRangeIterator, RuntimeRecordView,
    RuntimeScalarView, RuntimeSeq, RuntimeSignedIntWidth, RuntimeTupleView, RuntimeUInt,
    RuntimeUnsignedIntWidth, RuntimeValue, RuntimeValueView,
};
use crate::plan::body_semantic::RuntimeBodySemanticError;
use crate::task::semantic::TaskSemanticEncoder;

impl RuntimeValue {
    /// Visits direct child-list widths from the same logical graph used by
    /// static encoding. Invalid runtime-only values remain semantic admission
    /// errors; this count pass never constructs a payload or digest.
    pub(crate) fn try_visit_static_literal_child_counts<E>(
        &self,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        use traversal::LiteralEvent as Event;
        for event in traversal::LiteralEvents::new(self) {
            match event {
                Event::Value(value) => match value {
                    RuntimeValueView::Tuple(value) => visitor(value.len())?,
                    RuntimeValueView::Sequence(value) => visitor(value.len())?,
                    RuntimeValueView::Record(value) => visitor(value.len())?,
                    RuntimeValueView::NominalRecord(value) => visitor(value.fields().len())?,
                    RuntimeValueView::Opaque(_) | RuntimeValueView::Agent(_) => visitor(1)?,
                    RuntimeValueView::Variant { payload, .. } => {
                        visitor(usize::from(payload.is_some()))?;
                    }
                    RuntimeValueView::Reduction(value) => {
                        visitor(value.commands().len())?;
                        visitor(1)?;
                    }
                    RuntimeValueView::RuntimeOnly(value) => match value {
                        RuntimeValue::MatrixF32(value) => visitor(value.values().len())?,
                        RuntimeValue::MatrixF64(value) => visitor(value.values().len())?,
                        RuntimeValue::TensorF32(value) => {
                            visitor(value.shape().dims().len())?;
                            visitor(value.values().len())?;
                        }
                        RuntimeValue::TensorF64(value) => {
                            visitor(value.shape().dims().len())?;
                            visitor(value.values().len())?;
                        }
                        RuntimeValue::Iterator(RuntimeIterator::Values { items }) => {
                            visitor(items.len())?;
                        }
                        RuntimeValue::Range(_)
                        | RuntimeValue::Iterator(
                            RuntimeIterator::Range(_) | RuntimeIterator::Witness { .. },
                        )
                        | RuntimeValue::NeedHandle(_)
                        | RuntimeValue::Callable(_)
                        | RuntimeValue::Unit
                        | RuntimeValue::Bool(_)
                        | RuntimeValue::Int(_)
                        | RuntimeValue::UInt(_)
                        | RuntimeValue::F32(_)
                        | RuntimeValue::F64(_)
                        | RuntimeValue::String(_)
                        | RuntimeValue::Color(_)
                        | RuntimeValue::Char(_)
                        | RuntimeValue::Duration(_)
                        | RuntimeValue::Progress(_)
                        | RuntimeValue::EntityRef(_)
                        | RuntimeValue::Tuple(_)
                        | RuntimeValue::Seq(_)
                        | RuntimeValue::Record(_)
                        | RuntimeValue::NominalRecord(_)
                        | RuntimeValue::Opaque(_)
                        | RuntimeValue::Reduction(_)
                        | RuntimeValue::Agent(_)
                        | RuntimeValue::Variant { .. } => {}
                    },
                    RuntimeValueView::Scalar(_) => {}
                },
                Event::Predicate(predicate) => match predicate {
                    RuntimeAgentPredicate::All { predicates }
                    | RuntimeAgentPredicate::Any { predicates } => visitor(predicates.len())?,
                    RuntimeAgentPredicate::Compare { .. } | RuntimeAgentPredicate::Not { .. } => {
                        visitor(1)?;
                    }
                    RuntimeAgentPredicate::Exists { .. }
                    | RuntimeAgentPredicate::ActionEnabled { .. }
                    | RuntimeAgentPredicate::DiagnosticsHasError => {}
                },
                Event::Agent(_) | Event::Child(_) | Event::Command(_) => {}
            }
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one iterative exhaustive logical literal visitor keeps all child traversal on its owning value algebra"
    )]
    pub(crate) fn encode_static_literal(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), RuntimeBodySemanticError> {
        let mut events = traversal::LiteralEvents::new(self);
        loop {
            encoder.status()?;
            let Some(event) = events.next() else {
                break;
            };
            match event {
                traversal::LiteralEvent::Child(field) => {
                    encoder.enter_element();
                    if let Some(field) = field {
                        encoder.ordinal(field.zero_based());
                    }
                }
                traversal::LiteralEvent::Command(command) => {
                    encoder.enter_element();
                    encoder.string(command.constructor().as_str());
                    encoder.string(command.target().as_str());
                }
                traversal::LiteralEvent::Agent(agent) => {
                    use RuntimeAgentValue as A;
                    match agent {
                        A::ActionTarget(value) => {
                            encoder.tag(0);
                            encoder.string(value.id().as_str());
                            encoder.string(value.target().as_str());
                            encoder.tag(match value.action() {
                                RuntimeAgentAction::AdvanceText => 0,
                                RuntimeAgentAction::SelectChoice => 1,
                                RuntimeAgentAction::Invoke => 2,
                                RuntimeAgentAction::Scroll => 3,
                                RuntimeAgentAction::PointerClick => 4,
                            });
                            encoder.tag(match value.dispatch() {
                                RuntimeAgentActionDispatch::Semantic => 0,
                                RuntimeAgentActionDispatch::Physical => 1,
                            });
                            encoder.tag(u8::from(value.enabled()));
                        }
                        A::CaptureTarget(target) => {
                            encoder.tag(1);
                            match target {
                                RuntimeAgentCaptureTarget::Viewport => encoder.tag(0),
                                RuntimeAgentCaptureTarget::Layer { target } => {
                                    encoder.tag(1);
                                    encoder.string(target.as_str());
                                }
                                RuntimeAgentCaptureTarget::Object { target } => {
                                    encoder.tag(2);
                                    encoder.string(target.as_str());
                                }
                            }
                        }
                        A::DebugStatePath(path) => {
                            encoder.tag(2);
                            encoder.string(path.as_str());
                        }
                        A::ObservationFieldPath(path) => {
                            encoder.tag(3);
                            encoder.string(path.as_str());
                        }
                        A::Probe(probe) => {
                            encoder.tag(4);
                            probe.encode_static_probe(encoder);
                        }
                        A::Diagnostics => encoder.tag(5),
                        A::Predicate(_) => {
                            encoder.tag(6);
                        }
                        A::ViewportPoint { x, y } => {
                            encoder.tag(7);
                            encoder.ordinal(*x);
                            encoder.ordinal(*y);
                        }
                        A::BinaryData(data) => {
                            encoder.tag(8);
                            encoder.string(data);
                        }
                        A::DataShape(_) => {
                            encoder.reject_owner();
                            return Err(RuntimeBodySemanticError::InvalidStaticLiteral);
                        }
                    }
                }
                traversal::LiteralEvent::Predicate(predicate) => {
                    use RuntimeAgentPredicate as P;
                    match predicate {
                        P::Compare { probe, op, .. } => {
                            encoder.tag(0);
                            probe.encode_static_probe(encoder);
                            encoder.tag(match op {
                                RuntimeAgentCompareOp::Eq => 0,
                                RuntimeAgentCompareOp::NotEq => 1,
                                RuntimeAgentCompareOp::Greater => 2,
                                RuntimeAgentCompareOp::GreaterOrEqual => 3,
                                RuntimeAgentCompareOp::Less => 4,
                                RuntimeAgentCompareOp::LessOrEqual => 5,
                            });
                        }
                        P::Exists { probe } => {
                            encoder.tag(1);
                            probe.encode_static_probe(encoder);
                        }
                        P::ActionEnabled { target } => {
                            encoder.tag(2);
                            encoder.string(target.as_str());
                        }
                        P::DiagnosticsHasError => encoder.tag(3),
                        P::All { predicates } | P::Any { predicates } => {
                            encoder.tag(if matches!(predicate, P::All { .. }) {
                                4
                            } else {
                                5
                            });
                            encoder.count(predicates.len());
                        }
                        P::Not { .. } => {
                            encoder.tag(6);
                        }
                    }
                }
                traversal::LiteralEvent::Value(value) => match value {
                    RuntimeValueView::Scalar(value) => value.encode_static_literal(encoder)?,
                    RuntimeValueView::Tuple(tuple) => {
                        encoder.tag(12);
                        encoder.count(tuple.len());
                    }
                    RuntimeValueView::Sequence(sequence) => {
                        encoder.tag(13);
                        encoder.count(sequence.len());
                    }
                    RuntimeValueView::Record(record) => {
                        encoder.tag(14);
                        encoder.count(record.len());
                    }
                    RuntimeValueView::NominalRecord(record) => {
                        encoder.tag(15);
                        encoder.digest(record.semantic_identity().as_bytes());
                        encoder.digest(record.layout().as_bytes());
                        encoder.count(record.fields().len());
                    }
                    RuntimeValueView::Opaque(value) => {
                        if value.persistence() == RuntimeOpaquePersistence::SnapshotOnly
                            || matches!(
                                value.value_class(),
                                RuntimeOpaqueValueClass::AffineHandle(_)
                            )
                        {
                            encoder.reject_owner();
                            return Err(RuntimeBodySemanticError::InvalidStaticLiteral);
                        }
                        encoder.tag(16);
                        encoder.string(value.producer().as_str());
                        encoder.digest(value.semantic_identity().as_bytes());
                        encoder.tag(value.value_class().semantic_tag());
                        encoder.tag(value.persistence().semantic_tag());
                    }
                    RuntimeValueView::Variant {
                        owner,
                        ordinal,
                        payload,
                        ..
                    } => {
                        encoder.tag(17);
                        match owner {
                            crate::pattern::RuntimeVariantIdentity::Builtin(owner) => {
                                encoder.tag(0);
                                encoder.tag(owner.semantic_tag());
                            }
                            crate::pattern::RuntimeVariantIdentity::Nominal {
                                semantic_identity,
                                ..
                            } => {
                                encoder.tag(1);
                                encoder.digest(semantic_identity.as_bytes());
                            }
                        }
                        encoder.ordinal(ordinal);
                        encoder.tag(u8::from(payload.is_some()));
                    }
                    RuntimeValueView::RuntimeOnly(value) => match value {
                        RuntimeValue::Range(range) => {
                            encoder.tag(18);
                            range.encode_static_literal(encoder);
                        }
                        RuntimeValue::MatrixF32(value) => {
                            encoder.tag(20);
                            encoder.count(value.rows());
                            encoder.count(value.cols());
                            encoder.count(value.values().len());
                            for element in value.values() {
                                encoder.enter_element();
                                encoder.ordinal(element.to_bits());
                            }
                        }
                        RuntimeValue::MatrixF64(value) => {
                            encoder.tag(21);
                            encoder.count(value.rows());
                            encoder.count(value.cols());
                            encoder.count(value.values().len());
                            for element in value.values() {
                                encoder.enter_element();
                                encoder.scalar_u64(element.to_bits());
                            }
                        }
                        RuntimeValue::TensorF32(value) => {
                            encoder.tag(22);
                            encoder.count(value.shape().dims().len());
                            for dim in value.shape().dims() {
                                encoder.enter_element();
                                encoder.count(*dim);
                            }
                            encoder.count(value.values().len());
                            for element in value.values() {
                                encoder.enter_element();
                                encoder.ordinal(element.to_bits());
                            }
                        }
                        RuntimeValue::TensorF64(value) => {
                            encoder.tag(23);
                            encoder.count(value.shape().dims().len());
                            for dim in value.shape().dims() {
                                encoder.enter_element();
                                encoder.count(*dim);
                            }
                            encoder.count(value.values().len());
                            for element in value.values() {
                                encoder.enter_element();
                                encoder.scalar_u64(element.to_bits());
                            }
                        }
                        RuntimeValue::Iterator(RuntimeIterator::Values { items }) => {
                            encoder.tag(25);
                            encoder.count(items.len());
                        }
                        RuntimeValue::Iterator(RuntimeIterator::Range(range)) => {
                            encoder.tag(26);
                            range.encode_static_literal(encoder);
                        }
                        RuntimeValue::Iterator(RuntimeIterator::Witness { .. })
                        | RuntimeValue::NeedHandle(_)
                        | RuntimeValue::Callable(_)
                        | RuntimeValue::Unit
                        | RuntimeValue::Bool(_)
                        | RuntimeValue::Int(_)
                        | RuntimeValue::UInt(_)
                        | RuntimeValue::F32(_)
                        | RuntimeValue::F64(_)
                        | RuntimeValue::String(_)
                        | RuntimeValue::Color(_)
                        | RuntimeValue::Char(_)
                        | RuntimeValue::Duration(_)
                        | RuntimeValue::Progress(_)
                        | RuntimeValue::EntityRef(_)
                        | RuntimeValue::Tuple(_)
                        | RuntimeValue::Seq(_)
                        | RuntimeValue::Record(_)
                        | RuntimeValue::NominalRecord(_)
                        | RuntimeValue::Opaque(_)
                        | RuntimeValue::Reduction(_)
                        | RuntimeValue::Agent(_)
                        | RuntimeValue::Variant { .. } => {
                            encoder.reject_owner();
                            return Err(RuntimeBodySemanticError::InvalidStaticLiteral);
                        }
                    },
                    RuntimeValueView::Reduction(value) => {
                        encoder.tag(19);
                        encoder.string(value.owner().producer().as_str());
                        encoder.digest(value.owner().semantic_identity().as_bytes());
                        encoder.count(value.commands().len());
                    }
                    RuntimeValueView::Agent(_) => {
                        encoder.tag(24);
                    }
                },
            }
        }
        encoder.status().map_err(Into::into)
    }
}

impl RuntimeScalarView<'_> {
    fn encode_static_literal(
        self,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), RuntimeBodySemanticError> {
        match self {
            Self::Unit => encoder.tag(0),
            Self::Bool(value) => {
                encoder.tag(1);
                encoder.tag(u8::from(value));
            }
            Self::Int(value) => {
                encoder.tag(2);
                value.encode_static_literal(encoder);
            }
            Self::UInt(value) => {
                encoder.tag(3);
                value.encode_static_literal(encoder);
            }
            Self::F32(value) => {
                encoder.tag(4);
                encoder.ordinal(value.to_bits());
            }
            Self::F64(value) => {
                encoder.tag(5);
                encoder.scalar_u64(value.to_bits());
            }
            Self::String(value) => {
                encoder.tag(6);
                encoder.string(value);
            }
            Self::Color(value) => {
                encoder.tag(7);
                for channel in value.rgba8() {
                    encoder.tag(channel);
                }
            }
            Self::Char(value) => {
                encoder.tag(8);
                encoder.ordinal(u32::from(value));
            }
            Self::Duration(value) => {
                encoder.tag(9);
                encoder.scalar_u64(value.as_nanos());
            }
            Self::Progress(value) => {
                encoder.tag(10);
                encoder.ordinal(value.ratio().to_bits());
                encoder.tag(u8::from(value.label().is_some()));
                if let Some(label) = value.label() {
                    encoder.string(label);
                }
            }
            Self::EntityRef(value) => {
                encoder.tag(11);
                value.encode_body_identity(encoder);
            }
        }
        encoder.status().map_err(Into::into)
    }
}

impl RuntimeEntityReference {
    pub(crate) fn encode_body_identity(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Project { family, public_id } => {
                encoder.tag(0);
                encoder.tag(family.semantic_tag());
                encoder.string(public_id.as_str());
            }
            Self::DialogueLine(line) => {
                encoder.tag(1);
                encoder.string(&line.canonical_label());
            }
            Self::CharacterLook { character, look } => {
                encoder.tag(2);
                encoder.string(character.as_str());
                encoder.string(look.as_str());
            }
        }
    }
}

impl RuntimeInt {
    fn encode_static_literal(self, encoder: &mut TaskSemanticEncoder<'_>) {
        let tag = match self {
            Self::I8(_) => 0,
            Self::I16(_) => 1,
            Self::I32(_) => 2,
            Self::I64(_) => 3,
            Self::I128(_) => 4,
            Self::ISize(_) => 5,
        };
        encoder.tag(tag);
        encoder.scalar_u128(u128::from_le_bytes(self.as_i128().to_le_bytes()));
    }
}
impl RuntimeUInt {
    fn encode_static_literal(self, encoder: &mut TaskSemanticEncoder<'_>) {
        let tag = match self {
            Self::U8(_) => 0,
            Self::U16(_) => 1,
            Self::U32(_) => 2,
            Self::U64(_) => 3,
            Self::U128(_) => 4,
            Self::USize(_) => 5,
        };
        encoder.tag(tag);
        encoder.scalar_u128(self.as_u128());
    }
}
impl RuntimeRange {
    fn encode_static_literal(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Int {
                start,
                end,
                inclusive,
            } => {
                encoder.tag(0);
                for bound in [start, end] {
                    encoder.tag(u8::from(bound.is_some()));
                    if let Some(bound) = bound {
                        bound.encode_static_literal(encoder);
                    }
                }
                encoder.tag(u8::from(*inclusive));
            }
            Self::UInt {
                start,
                end,
                inclusive,
            } => {
                encoder.tag(1);
                for bound in [start, end] {
                    encoder.tag(u8::from(bound.is_some()));
                    if let Some(bound) = bound {
                        bound.encode_static_literal(encoder);
                    }
                }
                encoder.tag(u8::from(*inclusive));
            }
        }
    }
}

impl RuntimeAgentProbe {
    fn encode_static_probe(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Signal { target } => {
                encoder.tag(0);
                encoder.string(target.as_str());
            }
            Self::Metric { target } => {
                encoder.tag(1);
                encoder.string(target.as_str());
            }
            Self::StatePath { path } => {
                encoder.tag(2);
                encoder.string(path.as_str());
            }
            Self::ObservationField { path } => {
                encoder.tag(3);
                encoder.string(path.as_str());
            }
        }
    }
}

#[cfg(test)]
mod tests;

impl RuntimeRangeIterator {
    fn encode_static_literal(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Int {
                width,
                current,
                end,
                inclusive,
                done,
            } => {
                encoder.tag(0);
                encoder.tag(match width {
                    RuntimeSignedIntWidth::I8 => 0,
                    RuntimeSignedIntWidth::I16 => 1,
                    RuntimeSignedIntWidth::I32 => 2,
                    RuntimeSignedIntWidth::I64 => 3,
                    RuntimeSignedIntWidth::I128 => 4,
                    RuntimeSignedIntWidth::ISize => 5,
                });
                encoder.scalar_u128(u128::from_le_bytes(current.to_le_bytes()));
                encoder.scalar_u128(u128::from_le_bytes(end.to_le_bytes()));
                encoder.tag(u8::from(*inclusive));
                encoder.tag(u8::from(*done));
            }
            Self::UInt {
                width,
                current,
                end,
                inclusive,
                done,
            } => {
                encoder.tag(1);
                encoder.tag(match width {
                    RuntimeUnsignedIntWidth::U8 => 0,
                    RuntimeUnsignedIntWidth::U16 => 1,
                    RuntimeUnsignedIntWidth::U32 => 2,
                    RuntimeUnsignedIntWidth::U64 => 3,
                    RuntimeUnsignedIntWidth::U128 => 4,
                    RuntimeUnsignedIntWidth::USize => 5,
                });
                encoder.scalar_u128(*current);
                encoder.scalar_u128(*end);
                encoder.tag(u8::from(*inclusive));
                encoder.tag(u8::from(*done));
            }
        }
    }
}
