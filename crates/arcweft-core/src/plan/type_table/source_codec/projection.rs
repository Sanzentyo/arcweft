//! Source fields of the existing generic type projection algebra.

use super::{Reader, RuntimeTypeSourceError, Writer};
use crate::entry::{RuntimeMapKind, RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId,
};
use crate::plan::type_scope::source_codec::RuntimeTypeScopeRef;
use crate::plan::{
    RuntimeAgentOperationalType, RuntimeAgentTypeProjection, RuntimeArrayLength,
    RuntimeBoundConstReference, RuntimeBoundTypeReference, RuntimeFunctionTypeContract,
    RuntimePlanRecordField, RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimeTypeScope,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::value::{
    RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeSignedIntWidth,
    RuntimeUnsignedIntWidth,
};

type Projection = RuntimePlanTypeProjection<RuntimePlanTypeId>;

impl Writer<'_> {
    fn references(&mut self, items: &[RuntimePlanTypeId]) -> Result<(), RuntimeTypeSourceError> {
        self.counts.array::<RuntimePlanTypeId>(items.len())?;
        self.count(items.len())?;
        for item in items {
            self.reference(*item)?;
        }
        Ok(())
    }

    pub(super) fn projection(
        &mut self,
        projection: &Projection,
        scope: &RuntimeTypeScope,
    ) -> Result<(), RuntimeTypeSourceError> {
        use RuntimePlanTypeProjection as P;
        self.counts.scope_admission(scope.binders().len())?;
        self.tag(projection.executable_semantic_kind())?;
        match projection {
            P::BoundType(reference) => {
                scope.validate_type(*reference)?;
                self.bytes(&reference.depth().to_le_bytes())?;
                self.bytes(&reference.slot().to_le_bytes())?;
            }
            P::Signed(width) => self.tag(width.semantic_tag())?,
            P::Unsigned(width) => self.tag(width.semantic_tag())?,
            P::DenseNumeric(dense) => self.bytes(&dense.semantic_parts())?,
            P::Range(item)
            | P::Iterator(item)
            | P::Need(item)
            | P::ThreadHandle(item)
            | P::Shared(item)
            | P::Reference(item) => self.reference(*item)?,
            P::Sequence { kind, item } => {
                self.tag(kind.semantic_tag())?;
                self.reference(*item)?;
            }
            P::Array { item, length } => {
                self.reference(*item)?;
                scope.validate_length(*length)?;
                match length {
                    RuntimeArrayLength::Constant(length) => {
                        self.tag(0)?;
                        self.bytes(&length.to_le_bytes())?;
                    }
                    RuntimeArrayLength::Bound(reference) => {
                        self.tag(1)?;
                        self.bytes(&reference.depth().to_le_bytes())?;
                        self.bytes(&reference.slot().to_le_bytes())?;
                    }
                }
            }
            P::Map { kind, key, value } => {
                self.tag(kind.semantic_tag())?;
                self.reference(*key)?;
                self.reference(*value)?;
            }
            P::Stream { item, error } => {
                self.reference(*item)?;
                self.reference(*error)?;
            }
            P::Result {
                value,
                error,
                value_payload,
                error_payload,
            } => {
                for item in [value, error, value_payload, error_payload] {
                    self.reference(*item)?;
                }
            }
            P::Option { item, some_payload } => {
                self.reference(*item)?;
                self.reference(*some_payload)?;
            }
            P::BuiltinVariant { owner, cases } => {
                self.tag(owner.semantic_tag())?;
                self.counts
                    .array::<Option<RuntimePlanTypeId>>(cases.len())?;
                self.count(cases.len())?;
                for case in cases {
                    self.tag(u8::from(case.is_some()))?;
                    if let Some(ty) = case {
                        self.reference(*ty)?;
                    }
                }
            }
            P::Function {
                contract,
                parameters,
                result,
            } => {
                if !contract.binder().is_empty() {
                    self.counts.scope_admission(1)?;
                }
                let prepared = if let Some(meter) = &mut self.meter {
                    contract.prepare_source_encoding(scope, self.counts.limits.function(), meter)?
                } else {
                    contract.prepare_source_in_frame(scope, self.counts.limits.function())?
                };
                self.count(prepared.encoded_bytes())?;
                self.extent(prepared.encoded_bytes())?;
                self.counts.storage(prepared.storage_bytes())?;
                self.counts.elements(prepared.elements())?;
                if let Some(output) = &mut self.output {
                    prepared.append_to(output)?;
                }
                self.references(parameters)?;
                self.reference(*result)?;
            }
            P::Nominal {
                nominal,
                layout,
                arguments,
            } => {
                self.string(nominal.as_str())?;
                self.bytes(layout.as_bytes())?;
                self.references(arguments)?;
            }
            P::Tuple(items) | P::Choice(items) => self.references(items)?,
            P::Record(fields) => {
                self.counts
                    .array::<RuntimePlanRecordField<RuntimePlanTypeId>>(fields.len())?;
                self.count(fields.len())?;
                for field in fields {
                    self.string(field.diagnostic_name())?;
                    self.reference(*field.ty())?;
                }
            }
            P::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                arguments,
            } => {
                self.string(producer.as_str())?;
                self.tag(admission.encoded())?;
                self.tag(value_class.semantic_tag())?;
                self.tag(persistence.semantic_tag())?;
                self.references(arguments)?;
            }
            P::Agent(agent) => {
                self.tag(agent.operational_type().semantic_tag())?;
                match agent {
                    RuntimeAgentTypeProjection::Probe(item)
                    | RuntimeAgentTypeProjection::DataShape(item) => self.reference(*item)?,
                    _ => {}
                }
            }
            P::Never
            | P::Unit
            | P::Bool
            | P::F32
            | P::F64
            | P::String
            | P::Color
            | P::Char
            | P::Bytes
            | P::Duration
            | P::Progress
            | P::EntityReference
            | P::AgentValue => {}
        }
        Ok(())
    }
}

impl Reader<'_, '_> {
    fn references(
        &mut self,
        materialize: bool,
    ) -> Result<Option<Box<[RuntimePlanTypeId]>>, RuntimeTypeSourceError> {
        let count = self.count()?;
        self.counts.array::<RuntimePlanTypeId>(count)?;
        let mut items = materialize.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let item = self.reference()?;
            if let Some(items) = &mut items {
                items.push(item);
            }
        }
        Ok(items.map(Vec::into_boxed_slice))
    }

    pub(super) fn projection(
        &mut self,
        materialize: bool,
        scope: RuntimeTypeScopeRef<'_>,
    ) -> Result<Option<Projection>, RuntimeTypeSourceError> {
        use RuntimePlanTypeProjection as P;
        self.counts.scope_admission(scope.binder_count())?;
        Ok(match self.tag()? {
            0 => {
                let reference =
                    RuntimeBoundTypeReference::from_coordinates(self.u32()?, self.u16()?);
                scope.bound_type(reference.depth(), reference.slot())?;
                materialize.then_some(P::BoundType(reference))
            }
            1 => materialize.then_some(P::Never),
            2 => materialize.then_some(P::Unit),
            3 => materialize.then_some(P::Bool),
            4 => {
                let tag = self.tag()?;
                let width = RuntimeSignedIntWidth::from_semantic_tag(tag).ok_or(
                    RuntimeTypeSourceError::Tag {
                        kind: "signed width",
                        tag,
                    },
                )?;
                materialize.then_some(P::Signed(width))
            }
            5 => {
                let tag = self.tag()?;
                let width = RuntimeUnsignedIntWidth::from_semantic_tag(tag).ok_or(
                    RuntimeTypeSourceError::Tag {
                        kind: "unsigned width",
                        tag,
                    },
                )?;
                materialize.then_some(P::Unsigned(width))
            }
            6 => materialize.then_some(P::F32),
            7 => materialize.then_some(P::F64),
            8 => materialize.then_some(P::String),
            9 => materialize.then_some(P::Color),
            10 => materialize.then_some(P::Char),
            11 => materialize.then_some(P::Bytes),
            12 => materialize.then_some(P::Duration),
            13 => materialize.then_some(P::Progress),
            14 => materialize.then_some(P::EntityReference),
            15 => materialize.then_some(P::AgentValue),
            16 => {
                let item = self.reference()?;
                materialize.then_some(P::Range(item))
            }
            17 => {
                let item = self.reference()?;
                materialize.then_some(P::Iterator(item))
            }
            18 => {
                let tag = self.tag()?;
                let kind = RuntimePlanSequenceKind::from_semantic_tag(tag).ok_or(
                    RuntimeTypeSourceError::Tag {
                        kind: "sequence",
                        tag,
                    },
                )?;
                let item = self.reference()?;
                materialize.then_some(P::Sequence { kind, item })
            }
            19 => {
                let item = self.reference()?;
                let length = match self.tag()? {
                    0 => RuntimeArrayLength::Constant(u64::from_le_bytes(
                        self.take(8)?.try_into().expect("eight-byte array length"),
                    )),
                    1 => {
                        let reference =
                            RuntimeBoundConstReference::from_coordinates(self.u32()?, self.u16()?);
                        scope.bound_const(reference.depth(), reference.slot())?;
                        RuntimeArrayLength::Bound(reference)
                    }
                    tag => {
                        return Err(RuntimeTypeSourceError::Tag {
                            kind: "array length",
                            tag,
                        });
                    }
                };
                materialize.then_some(P::Array { item, length })
            }
            20 => {
                let tag = self.tag()?;
                let kind = RuntimeMapKind::from_semantic_tag(tag)
                    .ok_or(RuntimeTypeSourceError::Tag { kind: "map", tag })?;
                let key = self.reference()?;
                let value = self.reference()?;
                materialize.then_some(P::Map { kind, key, value })
            }
            21 => {
                let item = self.reference()?;
                materialize.then_some(P::Need(item))
            }
            22 => {
                let item = self.reference()?;
                let error = self.reference()?;
                materialize.then_some(P::Stream { item, error })
            }
            23 => {
                let value = self.reference()?;
                let error = self.reference()?;
                let value_payload = self.reference()?;
                let error_payload = self.reference()?;
                materialize.then_some(P::Result {
                    value,
                    error,
                    value_payload,
                    error_payload,
                })
            }
            24 => {
                let item = self.reference()?;
                let some_payload = self.reference()?;
                materialize.then_some(P::Option { item, some_payload })
            }
            25 => {
                let tag = self.tag()?;
                let owner = RuntimeBuiltinVariantIdentity::from_wire_tag(tag).ok_or(
                    RuntimeTypeSourceError::Tag {
                        kind: "builtin variant",
                        tag,
                    },
                )?;
                let count = self.count()?;
                self.counts.array::<Option<RuntimePlanTypeId>>(count)?;
                let mut cases = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let case = if self.boolean()? {
                        Some(self.reference()?)
                    } else {
                        None
                    };
                    if let Some(cases) = &mut cases {
                        cases.push(case);
                    }
                }
                cases.map(|cases| P::BuiltinVariant {
                    owner,
                    cases: cases.into_boxed_slice(),
                })
            }
            26 => {
                let item = self.reference()?;
                materialize.then_some(P::ThreadHandle(item))
            }
            27 => {
                let item = self.reference()?;
                materialize.then_some(P::Shared(item))
            }
            28 => {
                let item = self.reference()?;
                materialize.then_some(P::Reference(item))
            }
            29 => {
                let length = self.count()?;
                let bytes = self.take(length)?;
                let prepared = RuntimeFunctionTypeContract::preflight_source_in_frame(
                    bytes,
                    scope,
                    self.counts.limits.function(),
                )?;
                if !prepared.binder().is_empty() {
                    self.counts.scope_admission(1)?;
                }
                self.counts.storage(prepared.storage_bytes())?;
                self.counts.elements(prepared.elements())?;
                let parameters = self.references(materialize)?;
                let result = self.reference()?;
                if let Some(parameters) = parameters {
                    Some(P::Function {
                        contract: prepared.materialize(self.meter)?,
                        parameters,
                        result,
                    })
                } else {
                    None
                }
            }
            30 => {
                let nominal = self.string()?;
                RuntimeNominalTypeId::validate_source(nominal)?;
                let layout = TypeLayoutHash::from_bytes(self.digest()?);
                let arguments = self.references(materialize)?;
                if let Some(arguments) = arguments {
                    Some(P::Nominal {
                        nominal: RuntimeNominalTypeId::try_new(nominal)?,
                        layout,
                        arguments,
                    })
                } else {
                    None
                }
            }
            31 => self.references(materialize)?.map(P::Tuple),
            32 => {
                let count = self.count()?;
                self.counts
                    .array::<RuntimePlanRecordField<RuntimePlanTypeId>>(count)?;
                let mut fields = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let name = self.string()?;
                    let ty = self.reference()?;
                    if let Some(fields) = &mut fields {
                        fields.push(RuntimePlanRecordField::new(name, ty));
                    }
                }
                fields.map(|fields| P::Record(fields.into_boxed_slice()))
            }
            33 => self.references(materialize)?.map(P::Choice),
            34 => {
                let producer = self.string()?;
                RuntimeNominalTypeId::validate_source(producer)?;
                let tag = self.tag()?;
                let admission = RuntimeOpaqueTypeAdmission::from_encoded(tag).ok_or(
                    RuntimeTypeSourceError::Tag {
                        kind: "opaque admission",
                        tag,
                    },
                )?;
                let tag = self.tag()?;
                let value_class = RuntimeOpaqueValueClass::from_semantic_tag(tag).ok_or(
                    RuntimeTypeSourceError::Tag {
                        kind: "opaque value class",
                        tag,
                    },
                )?;
                let tag = self.tag()?;
                let persistence = RuntimeOpaquePersistence::from_semantic_tag(tag).ok_or(
                    RuntimeTypeSourceError::Tag {
                        kind: "opaque persistence",
                        tag,
                    },
                )?;
                let arguments = self.references(materialize)?;
                if let Some(arguments) = arguments {
                    Some(P::Opaque {
                        producer: RuntimeOpaqueTypeProducerId::try_new(producer)?,
                        admission,
                        value_class,
                        persistence,
                        arguments,
                    })
                } else {
                    None
                }
            }
            35 => {
                let tag = self.tag()?;
                let kind = RuntimeAgentOperationalType::from_semantic_tag(tag)
                    .ok_or(RuntimeTypeSourceError::Tag { kind: "agent", tag })?;
                let agent = match kind {
                    RuntimeAgentOperationalType::Probe => {
                        RuntimeAgentTypeProjection::Probe(self.reference()?)
                    }
                    RuntimeAgentOperationalType::DataShape => {
                        RuntimeAgentTypeProjection::DataShape(self.reference()?)
                    }
                    kind => {
                        RuntimeAgentTypeProjection::try_leaf(kind).expect("non-generic Agent kind")
                    }
                };
                materialize.then_some(P::Agent(agent))
            }
            36 => {
                let parts = self.take(2)?.try_into().expect("two-byte dense type");
                let dense = crate::math::RuntimeDenseNumericType::from_semantic_parts(parts)
                    .ok_or(RuntimeTypeSourceError::DenseNumeric { parts })?;
                materialize.then_some(P::DenseNumeric(dense))
            }
            tag => {
                return Err(RuntimeTypeSourceError::Tag {
                    kind: "projection",
                    tag,
                });
            }
        })
    }
}
