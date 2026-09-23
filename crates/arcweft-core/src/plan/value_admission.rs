//! Value admission against the existing plan tables, including recursive domains.
//!
//! The building and sealed phases borrow the same authorities. No checked-type
//! tree or additional nominal catalog is materialized for executable acceptance.

use thiserror::Error;

use crate::entry::schema::{
    value_budget::ValidationWork,
    value_encoding::{self, ValueAdmission, ValueValidation},
};
use crate::entry::{
    RuntimeNominalSchemaIdentity, RuntimeSchemaError, RuntimeSchemaLimits, RuntimeValueDigest,
};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner};
use crate::runtime_id::RuntimePlanTypeId;
use crate::value::{
    RuntimeIterator, RuntimeRange, RuntimeRangeIterator, RuntimeReductionProducer,
    RuntimeScalarView as Scalar, RuntimeUnsignedIntWidth, RuntimeValue, RuntimeValueView as View,
};

use super::{
    RuntimeAgentTypeProjection, RuntimeNominalRecordDomain, RuntimeNominalRecordDomainField,
    RuntimePlan, RuntimePlanRecordField, RuntimePlanTypeDeclaration,
    RuntimePlanTypeProjection as Type, RuntimeVariantDomain,
    nominal_record_domains::RuntimeNominalRecordDomainTableBuilder,
    type_table::RuntimePlanTypeTableBuilder, variant_domains::RuntimeVariantDomainTableBuilder,
};

/// A value cannot be admitted under a plan-local type and the selected limits.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanValueAdmissionError {
    #[error("runtime plan has no type {ty}")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("runtime value does not satisfy plan type {ty}: {source}")]
    Value {
        ty: RuntimePlanTypeId,
        source: RuntimeSchemaError,
    },
}

impl RuntimePlan {
    /// Validates a live value against this plan's exact type/domain tables,
    /// including recursive nominal descendants, without persistence encoding.
    pub fn validate_live_value(
        &self,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimePlanValueAdmissionError> {
        let mut validation =
            PlanValueValidation::new(PlanValueAuthority::Sealed(self), ty, limits)?;
        value_encoding::validate_live(value, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| RuntimePlanValueAdmissionError::Value { ty, source })
    }

    /// Validates a decoded private snapshot candidate before publication.
    pub fn validate_snapshot_value(
        &self,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimePlanValueAdmissionError> {
        let mut validation =
            PlanValueValidation::new(PlanValueAuthority::Sealed(self), ty, limits)?;
        value_encoding::validate_snapshot(value, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| RuntimePlanValueAdmissionError::Value { ty, source })
    }

    /// Validates the complete finite value through the plan's type and nominal
    /// domain tables, and hashes its canonical persistent representation.
    /// Recursive type edges are followed only when the value has a descendant.
    pub fn accepts_value(
        &self,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, RuntimePlanValueAdmissionError> {
        let mut validation =
            PlanValueValidation::new(PlanValueAuthority::Sealed(self), ty, limits)?;
        value_encoding::validate_and_hash(value, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| RuntimePlanValueAdmissionError::Value { ty, source })
    }
}

/// Borrowed phase context; every lookup uses the original admitted tables.
pub(super) enum PlanValueAuthority<'a> {
    Sealed(&'a RuntimePlan),
    Building {
        types: &'a RuntimePlanTypeTableBuilder,
        records: &'a RuntimeNominalRecordDomainTableBuilder,
        variants: &'a RuntimeVariantDomainTableBuilder,
    },
}

impl<'a> PlanValueAuthority<'a> {
    fn get(&self, ty: RuntimePlanTypeId) -> Option<&'a RuntimePlanTypeDeclaration> {
        match self {
            Self::Sealed(plan) => plan.type_table().get(ty),
            Self::Building { types, .. } => types.get(ty),
        }
    }

    fn declaration(&self, ty: RuntimePlanTypeId) -> &'a RuntimePlanTypeDeclaration {
        self.get(ty)
            .expect("admission established every type/domain reference")
    }

    fn record(&self, ty: RuntimePlanTypeId) -> Option<&'a RuntimeNominalRecordDomain> {
        match self {
            Self::Sealed(plan) => plan.nominal_record_domains().get(ty),
            Self::Building { records, .. } => records.get(ty),
        }
    }

    fn variant(&self, ty: RuntimePlanTypeId) -> Option<&'a RuntimeVariantDomain> {
        match self {
            Self::Sealed(plan) => plan.variant_domains().get(ty),
            Self::Building { variants, .. } => variants.get(ty),
        }
    }

    pub(super) fn validate_literal(
        self,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimePlanValueAdmissionError> {
        let mut validation = PlanValueValidation::new(self, ty, limits)?;
        value_encoding::validate_literal(value, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| RuntimePlanValueAdmissionError::Value { ty, source })
    }
}

#[derive(Clone, Copy)]
enum Expected {
    Type(RuntimePlanTypeId),
    Byte,
    MapEntry {
        key: RuntimePlanTypeId,
        value: RuntimePlanTypeId,
    },
    Admitted,
}

enum Children<'a> {
    None,
    Any,
    Single(Expected),
    Repeated(Expected),
    Tuple(&'a [RuntimePlanTypeId]),
    Record(&'a [RuntimePlanRecordField<RuntimePlanTypeId>]),
    NominalRecord(&'a [RuntimeNominalRecordDomainField]),
    MapEntry {
        key: RuntimePlanTypeId,
        value: RuntimePlanTypeId,
    },
    Reduction(RuntimePlanTypeId),
}

struct Alternatives<'a>(std::slice::Iter<'a, RuntimePlanTypeId>);

impl Iterator for Alternatives<'_> {
    type Item = Expected;
    fn next(&mut self) -> Option<Expected> {
        self.0.next().copied().map(Expected::Type)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl ExactSizeIterator for Alternatives<'_> {}

struct PlanValueValidation<'a> {
    authority: PlanValueAuthority<'a>,
    work: ValidationWork,
}

impl<'a> PlanValueValidation<'a> {
    fn new(
        authority: PlanValueAuthority<'a>,
        ty: RuntimePlanTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimePlanValueAdmissionError> {
        authority
            .get(ty)
            .ok_or(RuntimePlanValueAdmissionError::UnknownType { ty })?;
        Ok(Self {
            authority,
            work: ValidationWork::new(limits),
        })
    }

    fn mismatch(value: View<'_>) -> RuntimeSchemaError {
        RuntimeSchemaError::Type {
            path: "$".to_owned(),
            expected: "admitted plan type",
            actual: value.type_name(),
        }
    }

    fn arity(expected: usize, actual: usize) -> Result<(), RuntimeSchemaError> {
        if expected == actual {
            Ok(())
        } else {
            Err(RuntimeSchemaError::Arity {
                path: "$".to_owned(),
                expected,
                actual,
            })
        }
    }

    fn variant(
        &self,
        ty: RuntimePlanTypeId,
        value: View<'_>,
    ) -> Result<Children<'a>, RuntimeSchemaError> {
        let View::Variant {
            owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Err(Self::mismatch(value));
        };
        let declaration = self.authority.declaration(ty);
        let case = declaration
            .select_variant_case(ty, self.authority.variant(ty), ordinal)
            .map_err(|_| RuntimeSchemaError::UnknownVariant {
                path: "$".to_owned(),
                variant: name.to_owned(),
            })?;
        if case.owner() != owner || case.name() != name {
            return Err(Self::mismatch(value));
        }
        match (case.payload(), payload.is_some()) {
            (None, false) => Ok(Children::None),
            (Some(ty), true) => Ok(Children::Single(Expected::Type(ty))),
            _ => Err(RuntimeSchemaError::VariantPayload {
                path: "$".to_owned(),
            }),
        }
    }

    fn nominal(
        &self,
        ty: RuntimePlanTypeId,
        nominal: &crate::entry::RuntimeNominalTypeId,
        layout: crate::entry::TypeLayoutHash,
        value: View<'_>,
    ) -> Result<Children<'a>, RuntimeSchemaError> {
        if self.authority.variant(ty).is_some() {
            return self.variant(ty, value);
        }
        let domain =
            self.authority
                .record(ty)
                .ok_or_else(|| RuntimeSchemaError::UnresolvedNominal {
                    identity: RuntimeNominalSchemaIdentity::new(
                        nominal.clone(),
                        self.authority.declaration(ty).semantic_identity(),
                    ),
                })?;
        let View::NominalRecord(record) = value else {
            return Err(Self::mismatch(value));
        };
        if record.type_id() != nominal {
            return Err(RuntimeSchemaError::NominalIdentity {
                path: "$".to_owned(),
                expected: nominal.as_str().to_owned(),
                actual: record.type_id().as_str().to_owned(),
            });
        }
        let semantic_identity = self.authority.declaration(ty).semantic_identity();
        if record.semantic_identity() != semantic_identity {
            return Err(RuntimeSchemaError::NominalSemanticIdentity {
                path: "$".to_owned(),
                expected: semantic_identity,
                actual: record.semantic_identity(),
            });
        }
        if record.layout() != layout {
            return Err(RuntimeSchemaError::NominalLayout {
                path: "$".to_owned(),
            });
        }
        Self::arity(domain.fields().len(), record.fields().len())?;
        Ok(Children::NominalRecord(domain.fields()))
    }

    fn opaque(
        &self,
        ty: RuntimePlanTypeId,
        owner: &RuntimeOpaqueTypeOwner,
        arguments: &[RuntimePlanTypeId],
        value: View<'_>,
    ) -> Result<Children<'a>, RuntimeSchemaError> {
        if self.authority.variant(ty).is_some() {
            return self.variant(ty, value);
        }
        match value {
            View::Opaque(actual) if owner.accepts_opaque_value(actual) => Ok(Children::Any),
            View::Reduction(actual)
                if owner.admission() == RuntimeOpaqueTypeAdmission::ExactIdentity
                    && RuntimeReductionProducer::accepts(owner.producer())
                    && actual.owner() == owner =>
            {
                let [state] = arguments else {
                    return Err(RuntimeSchemaError::OpaqueOwner {
                        path: "$".to_owned(),
                    });
                };
                Ok(Children::Reduction(*state))
            }
            _ => Err(RuntimeSchemaError::OpaqueOwner {
                path: "$".to_owned(),
            }),
        }
    }

    fn range(&self, item: RuntimePlanTypeId, range: &RuntimeRange) -> bool {
        match (self.authority.declaration(item).projection(), range) {
            (Type::Signed(width), RuntimeRange::Int { start, end, .. }) => {
                (start.is_some() || end.is_some())
                    && start.iter().chain(end).all(|value| value.width() == *width)
            }
            (Type::Unsigned(width), RuntimeRange::UInt { start, end, .. }) => {
                (start.is_some() || end.is_some())
                    && start.iter().chain(end).all(|value| value.width() == *width)
            }
            _ => false,
        }
    }

    fn iterator(
        &self,
        item: RuntimePlanTypeId,
        iterator: &RuntimeIterator,
    ) -> Option<Children<'a>> {
        match iterator {
            RuntimeIterator::Values { items, index } if *index <= items.len() => {
                Some(Children::Repeated(Expected::Type(item)))
            }
            RuntimeIterator::Range(range) => {
                let matches = match (self.authority.declaration(item).projection(), range) {
                    (Type::Signed(expected), RuntimeRangeIterator::Int { width, .. }) => {
                        expected == width
                    }
                    (Type::Unsigned(expected), RuntimeRangeIterator::UInt { width, .. }) => {
                        expected == width
                    }
                    _ => false,
                };
                matches.then_some(Children::None)
            }
            RuntimeIterator::Values { .. } | RuntimeIterator::Witness { .. } => None,
        }
    }

    fn check(
        &self,
        ty: RuntimePlanTypeId,
        value: View<'_>,
    ) -> Result<Children<'a>, RuntimeSchemaError> {
        let declaration = self.authority.declaration(ty);
        match (declaration.projection(), value) {
            (Type::Unit, View::Scalar(Scalar::Unit))
            | (Type::Bool, View::Scalar(Scalar::Bool(_)))
            | (Type::F32, View::Scalar(Scalar::F32(_)))
            | (Type::F64, View::Scalar(Scalar::F64(_)))
            | (Type::String, View::Scalar(Scalar::String(_)))
            | (Type::Char, View::Scalar(Scalar::Char(_)))
            | (Type::Duration, View::Scalar(Scalar::Duration(_)))
            | (Type::Progress, View::Scalar(Scalar::Progress(_)))
            | (Type::EntityReference, View::Scalar(Scalar::EntityRef(_))) => Ok(Children::None),
            (Type::Signed(width), View::Scalar(Scalar::Int(actual)))
                if actual.width() == *width =>
            {
                Ok(Children::None)
            }
            (Type::Unsigned(width), View::Scalar(Scalar::UInt(actual)))
                if actual.width() == *width =>
            {
                Ok(Children::None)
            }
            (Type::AgentValue, value) if value.is_agent_value_node() => {
                Ok(Children::Repeated(Expected::Type(ty)))
            }
            (Type::Bytes { .. }, View::Sequence(_)) => Ok(Children::Repeated(Expected::Byte)),
            (Type::Sequence { item, .. }, View::Sequence(_)) => {
                Ok(Children::Repeated(Expected::Type(*item)))
            }
            (Type::Map { key, value, .. }, View::Sequence(_)) => {
                Ok(Children::Repeated(Expected::MapEntry {
                    key: *key,
                    value: *value,
                }))
            }
            (Type::Array { item, length }, View::Sequence(actual)) => {
                if u64::try_from(actual.len()) != Ok(*length) {
                    return Err(RuntimeSchemaError::ArrayLength {
                        path: "$".to_owned(),
                        expected: *length,
                        actual: actual.len(),
                    });
                }
                Ok(Children::Repeated(Expected::Type(*item)))
            }
            (Type::Tuple(items), View::Tuple(actual)) => {
                Self::arity(items.len(), actual.len())?;
                Ok(Children::Tuple(items))
            }
            (Type::Record(fields), View::Record(actual)) => {
                Self::arity(fields.len(), actual.len())?;
                for (index, field) in fields.iter().enumerate() {
                    let (id, name, _) = actual.get(index).expect("record arity was checked");
                    if id.zero_based() as usize != index || name != field.diagnostic_name() {
                        return Err(RuntimeSchemaError::RecordField {
                            path: "$".to_owned(),
                            ordinal: index,
                        });
                    }
                }
                Ok(Children::Record(fields))
            }
            (
                Type::Nominal {
                    nominal, layout, ..
                },
                _,
            ) => self.nominal(ty, nominal, *layout, value),
            (Type::Option { .. } | Type::Result { .. } | Type::BuiltinVariant { .. }, _) => {
                self.variant(ty, value)
            }
            (
                Type::Opaque {
                    producer,
                    admission,
                    value_class,
                    persistence,
                    arguments,
                },
                _,
            ) => self.opaque(
                ty,
                &RuntimeOpaqueTypeOwner::with_admission(
                    producer.clone(),
                    declaration.semantic_identity(),
                    *admission,
                    *value_class,
                    *persistence,
                ),
                arguments,
                value,
            ),
            (Type::Range(item), View::RuntimeOnly(RuntimeValue::Range(range)))
                if self.range(*item, range) =>
            {
                Ok(Children::None)
            }
            (Type::Iterator(item), View::RuntimeOnly(RuntimeValue::Iterator(iterator))) => self
                .iterator(*item, iterator)
                .ok_or_else(|| Self::mismatch(value)),
            (
                Type::Agent(RuntimeAgentTypeProjection::DataShape(_)),
                View::Agent(crate::value::RuntimeAgentValue::DataShape(shape)),
            ) if matches!(&self.authority, PlanValueAuthority::Sealed(plan) if shape.matches_plan(plan, declaration.semantic_identity())) => {
                Ok(Children::None)
            }
            (Type::Agent(expected), View::Agent(actual))
                if !matches!(
                    expected,
                    RuntimeAgentTypeProjection::Probe(_) | RuntimeAgentTypeProjection::DataShape(_)
                ) && expected.operational_type() == actual.operational_type() =>
            {
                Ok(Children::Any)
            }
            (Type::Agent(expected), View::Record(_))
                if expected.operational_type().accepts_protocol_record() =>
            {
                Ok(Children::Any)
            }
            _ => Err(Self::mismatch(value)),
        }
    }
}

impl<'a> ValueValidation for PlanValueValidation<'a> {
    type Expected = Expected;
    type Children = Children<'a>;
    type Alternatives = Alternatives<'a>;

    fn preflight(&mut self, expected: &Expected, depth: usize) -> Result<(), RuntimeSchemaError> {
        if !Self::is_admitted(expected) {
            self.work.charge(depth)?;
        }
        Ok(())
    }

    fn alternative(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.work.charge(depth)
    }

    fn is_admitted(expected: &Expected) -> bool {
        matches!(expected, Expected::Admitted)
    }

    fn admitted_children() -> Children<'a> {
        Children::Any
    }

    fn enter(
        &mut self,
        expected: Expected,
        value: View<'_>,
    ) -> Result<ValueAdmission<Children<'a>, Alternatives<'a>>, RuntimeSchemaError> {
        match expected {
            Expected::Admitted => Ok(ValueAdmission::Admitted),
            Expected::Byte => {
                if matches!(value, View::Scalar(Scalar::UInt(actual)) if actual.width() == RuntimeUnsignedIntWidth::U8)
                {
                    Ok(ValueAdmission::Children(Children::None))
                } else {
                    Err(Self::mismatch(value))
                }
            }
            Expected::MapEntry { key, value: mapped } => {
                let View::Tuple(actual) = value else {
                    return Err(Self::mismatch(value));
                };
                Self::arity(2, actual.len())?;
                Ok(ValueAdmission::Children(Children::MapEntry {
                    key,
                    value: mapped,
                }))
            }
            Expected::Type(ty) => {
                if let Type::Choice(alternatives) = self.authority.declaration(ty).projection() {
                    self.work.collection(alternatives.len())?;
                    Ok(ValueAdmission::Choice(Alternatives(alternatives.iter())))
                } else {
                    self.check(ty, value).map(ValueAdmission::Children)
                }
            }
        }
    }

    fn child(
        &mut self,
        children: &Children<'a>,
        index: usize,
    ) -> Result<Expected, RuntimeSchemaError> {
        let child = match children {
            Children::Single(expected) if index == 0 => Some(*expected),
            Children::Repeated(expected) => Some(*expected),
            Children::Tuple(items) => items.get(index).copied().map(Expected::Type),
            Children::Record(fields) => fields.get(index).map(|field| Expected::Type(*field.ty())),
            Children::NominalRecord(fields) => {
                fields.get(index).map(|field| Expected::Type(field.ty()))
            }
            Children::MapEntry { key, .. } if index == 0 => Some(Expected::Type(*key)),
            Children::MapEntry { value, .. } if index == 1 => Some(Expected::Type(*value)),
            Children::Reduction(state) if index == 0 => Some(Expected::Type(*state)),
            Children::Any | Children::Reduction(_) => Some(Expected::Admitted),
            Children::None | Children::Single(_) | Children::MapEntry { .. } => None,
        };
        child.ok_or_else(|| RuntimeSchemaError::Encoding {
            message: "validated plan value has an unexpected child".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests;
