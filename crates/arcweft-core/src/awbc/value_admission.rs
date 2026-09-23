//! Persistent value admission through the program's original executable rows.

use thiserror::Error;

use super::schema::{
    AwbcAgentTypeShape, AwbcProgram, AwbcRecordField, AwbcRuntimeType,
    AwbcRuntimeTypeShape as Type, AwbcStringId, AwbcTypeId, AwbcVariantIdentity,
};
use crate::entry::schema::{
    value_budget::ValidationWork,
    value_encoding::{self, ValueAdmission, ValueValidation},
};
use crate::entry::{
    RuntimeNominalRecordShape, RuntimeSchemaError, RuntimeSchemaLimits, RuntimeValueDigest,
};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeVariantIdentity};
use crate::plan::RuntimeAgentTypeProjection;
use crate::value::{
    RuntimeReductionProducer, RuntimeScalarView as Scalar, RuntimeUnsignedIntWidth, RuntimeValue,
    RuntimeValueView as View,
};

/// Failure to admit a finite persistent value under an executable program type.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AwbcValueAdmissionError {
    #[error("AWBC program has no runtime type {ty:?}")]
    UnknownType { ty: AwbcTypeId },
    #[error("runtime value does not satisfy AWBC type {ty:?}: {source}")]
    Value {
        ty: AwbcTypeId,
        source: RuntimeSchemaError,
    },
}

impl AwbcProgram {
    /// Validates a live value through this program's exact type table.
    pub fn validate_live_value(
        &self,
        ty: AwbcTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), AwbcValueAdmissionError> {
        self.runtime_types
            .get(ty.index())
            .ok_or(AwbcValueAdmissionError::UnknownType { ty })?;
        let mut validation = AwbcValueValidation {
            program: self,
            work: ValidationWork::new(limits),
        };
        value_encoding::validate_live(value, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| AwbcValueAdmissionError::Value { ty, source })
    }

    /// Validates a decoded private snapshot candidate before publication.
    pub fn validate_snapshot_value(
        &self,
        ty: AwbcTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), AwbcValueAdmissionError> {
        self.runtime_types
            .get(ty.index())
            .ok_or(AwbcValueAdmissionError::UnknownType { ty })?;
        let mut validation = AwbcValueValidation {
            program: self,
            work: ValidationWork::new(limits),
        };
        value_encoding::validate_snapshot(value, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| AwbcValueAdmissionError::Value { ty, source })
    }

    /// Validates and hashes a persistent value against this program's type rows.
    /// Nominal recursion follows only the finite value's descendants. Diagnostic
    /// checked-type expansion is not used and no source schema is reconstructed.
    pub fn accepts_value(
        &self,
        ty: AwbcTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, AwbcValueAdmissionError> {
        self.runtime_types
            .get(ty.index())
            .ok_or(AwbcValueAdmissionError::UnknownType { ty })?;
        let mut validation = AwbcValueValidation {
            program: self,
            work: ValidationWork::new(limits),
        };
        value_encoding::validate_and_hash(value, limits, &mut validation, Expected::Type(ty))
            .map_err(|source| AwbcValueAdmissionError::Value { ty, source })
    }
}

#[derive(Clone, Copy)]
enum Expected {
    Type(AwbcTypeId),
    Byte,
    MapEntry { key: AwbcTypeId, value: AwbcTypeId },
    Admitted,
}

enum Children<'a> {
    None,
    Any,
    Single(Expected),
    Repeated(Expected),
    Tuple(&'a [AwbcTypeId]),
    Record(&'a [AwbcRecordField]),
    MapEntry { key: AwbcTypeId, value: AwbcTypeId },
    Reduction(AwbcTypeId),
}

struct Alternatives<'a>(std::slice::Iter<'a, AwbcTypeId>);

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

struct AwbcValueValidation<'a> {
    program: &'a AwbcProgram,
    work: ValidationWork,
}

impl<'a> AwbcValueValidation<'a> {
    fn row(&self, ty: AwbcTypeId) -> Result<&'a AwbcRuntimeType, RuntimeSchemaError> {
        self.program
            .runtime_types
            .get(ty.index())
            .ok_or_else(|| RuntimeSchemaError::Encoding {
                message: format!("AWBC type reference {} is out of bounds", ty.0),
            })
    }

    fn string(&self, id: AwbcStringId) -> Result<&'a str, RuntimeSchemaError> {
        self.program
            .strings
            .get(id.index())
            .map(String::as_str)
            .ok_or_else(|| RuntimeSchemaError::Encoding {
                message: format!("AWBC string reference {} is out of bounds", id.0),
            })
    }

    fn mismatch(value: View<'_>) -> RuntimeSchemaError {
        RuntimeSchemaError::Type {
            path: "$".to_owned(),
            expected: "admitted AWBC type",
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

    fn fields(
        &self,
        ty: AwbcTypeId,
        shape: RuntimeNominalRecordShape,
        fields: &[AwbcRecordField],
    ) -> Result<(), RuntimeSchemaError> {
        self.work.collection(fields.len())?;
        self.program
            .validate_record_fields(ty, shape, fields)
            .map_err(|error| RuntimeSchemaError::Encoding {
                message: error.to_string(),
            })
    }

    fn variant(
        &self,
        ty: AwbcTypeId,
        row: &AwbcRuntimeType,
        value: View<'_>,
    ) -> Result<Children<'a>, RuntimeSchemaError> {
        let Type::Variant {
            owner: expected_owner,
            arguments,
            cases,
        } = row.shape()
        else {
            unreachable!("the selected row is a variant")
        };
        self.work.collection(cases.len())?;
        self.work.collection(arguments.len())?;
        self.program
            .validate_variant_fields(ty, expected_owner, arguments, cases)
            .map_err(|error| RuntimeSchemaError::Encoding {
                message: error.to_string(),
            })?;
        let View::Variant {
            owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Err(Self::mismatch(value));
        };
        let owner_matches = match (expected_owner, owner) {
            (AwbcVariantIdentity::Builtin(expected), RuntimeVariantIdentity::Builtin(actual)) => {
                expected == actual
            }
            (
                AwbcVariantIdentity::Nominal { public_id, layout },
                RuntimeVariantIdentity::Nominal {
                    nominal,
                    semantic_identity,
                    layout: actual_layout,
                },
            ) => {
                self.string(*public_id)? == nominal.as_str()
                    && row.semantic_identity() == *semantic_identity
                    && layout == actual_layout.as_bytes()
            }
            _ => false,
        };
        if !owner_matches {
            return Err(Self::mismatch(value));
        }
        let case = usize::try_from(ordinal)
            .ok()
            .and_then(|index| cases.get(index))
            .ok_or_else(|| RuntimeSchemaError::UnknownVariant {
                path: "$".to_owned(),
                variant: name.to_owned(),
            })?;
        if self.string(case.name)? != name {
            return Err(RuntimeSchemaError::UnknownVariant {
                path: "$".to_owned(),
                variant: name.to_owned(),
            });
        }
        match (case.payload, payload.is_some()) {
            (None, false) => Ok(Children::None),
            (Some(ty), true) => Ok(Children::Single(Expected::Type(ty))),
            _ => Err(RuntimeSchemaError::VariantPayload {
                path: "$".to_owned(),
            }),
        }
    }

    fn check(&self, ty: AwbcTypeId, value: View<'_>) -> Result<Children<'a>, RuntimeSchemaError> {
        let row = self.row(ty)?;
        match (row.shape(), value) {
            (Type::Unit, View::Scalar(Scalar::Unit))
            | (Type::Bool, View::Scalar(Scalar::Bool(_)))
            | (Type::F32, View::Scalar(Scalar::F32(_)))
            | (Type::F64, View::Scalar(Scalar::F64(_)))
            | (Type::String, View::Scalar(Scalar::String(_)))
            | (Type::Char, View::Scalar(Scalar::Char(_)))
            | (Type::Duration, View::Scalar(Scalar::Duration(_)))
            | (Type::Progress, View::Scalar(Scalar::Progress(_)))
            | (Type::EntityRef, View::Scalar(Scalar::EntityRef(_))) => Ok(Children::None),
            (Type::Int(kind), View::Scalar(Scalar::Int(actual)))
                if actual.width() == (*kind).into() =>
            {
                Ok(Children::None)
            }
            (Type::UInt(kind), View::Scalar(Scalar::UInt(actual)))
                if actual.width() == (*kind).into() =>
            {
                Ok(Children::None)
            }
            (Type::AgentValue, value) if value.is_agent_value_node() => {
                Ok(Children::Repeated(Expected::Type(ty)))
            }
            (Type::Bytes { .. }, View::Sequence(_)) => Ok(Children::Repeated(Expected::Byte)),
            (Type::Sequence(item), View::Sequence(_)) => {
                Ok(Children::Repeated(Expected::Type(*item)))
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
            (Type::Map { key, value, .. }, View::Sequence(_)) => {
                Ok(Children::Repeated(Expected::MapEntry {
                    key: *key,
                    value: *value,
                }))
            }
            (Type::Tuple(items), View::Tuple(actual)) => {
                Self::arity(items.len(), actual.len())?;
                Ok(Children::Tuple(items))
            }
            (Type::Record { fields, .. }, View::Record(actual)) => {
                self.fields(ty, RuntimeNominalRecordShape::Record, fields)?;
                Self::arity(fields.len(), actual.len())?;
                for (index, field) in fields.iter().enumerate() {
                    let (id, name, _) = actual.get(index).expect("record arity was checked");
                    if id != field.field
                        || field.name.map(|id| self.string(id)).transpose()? != Some(name)
                    {
                        return Err(RuntimeSchemaError::RecordField {
                            path: "$".to_owned(),
                            ordinal: index,
                        });
                    }
                }
                Ok(Children::Record(fields))
            }
            (
                Type::NominalRecord {
                    public_id,
                    layout,
                    shape,
                    fields,
                    ..
                },
                View::NominalRecord(actual),
            ) => {
                self.fields(ty, *shape, fields)?;
                if self.string(*public_id)? != actual.type_id().as_str() {
                    return Err(RuntimeSchemaError::NominalIdentity {
                        path: "$".to_owned(),
                        expected: self.string(*public_id)?.to_owned(),
                        actual: actual.type_id().as_str().to_owned(),
                    });
                }
                if actual.semantic_identity() != row.semantic_identity() {
                    return Err(RuntimeSchemaError::NominalSemanticIdentity {
                        path: "$".to_owned(),
                        expected: row.semantic_identity(),
                        actual: actual.semantic_identity(),
                    });
                }
                if layout != actual.layout().as_bytes() {
                    return Err(RuntimeSchemaError::NominalLayout {
                        path: "$".to_owned(),
                    });
                }
                Self::arity(fields.len(), actual.fields().len())?;
                Ok(Children::Record(fields))
            }
            (Type::Variant { .. }, _) => self.variant(ty, row, value),
            (Type::Opaque { arguments, .. }, _) => {
                let owner = row
                    .try_opaque_owner(&self.program.strings)
                    .map_err(|error| RuntimeSchemaError::Encoding {
                        message: error.to_string(),
                    })?
                    .expect("the selected row is opaque");
                match value {
                    View::Opaque(actual) if owner.accepts_opaque_value(actual) => Ok(Children::Any),
                    View::Reduction(actual)
                        if owner.admission() == RuntimeOpaqueTypeAdmission::ExactIdentity
                            && RuntimeReductionProducer::accepts(owner.producer())
                            && actual.owner() == &owner =>
                    {
                        let [state] = arguments.as_slice() else {
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
            (
                Type::Agent(AwbcAgentTypeShape::DataShape(_)),
                View::Agent(crate::value::RuntimeAgentValue::DataShape(shape)),
            ) if shape.matches_awbc(self.program, row.semantic_identity()) => Ok(Children::None),
            (Type::Agent(AwbcAgentTypeShape::Leaf(expected)), View::Agent(actual))
                if RuntimeAgentTypeProjection::<AwbcTypeId>::try_leaf(*expected).is_some()
                    && *expected == actual.operational_type() =>
            {
                Ok(Children::Any)
            }
            (Type::Agent(AwbcAgentTypeShape::Leaf(expected)), View::Record(_))
                if expected.accepts_protocol_record() =>
            {
                Ok(Children::Any)
            }
            (Type::Dynamic, _) => Ok(Children::Any),
            _ => Err(Self::mismatch(value)),
        }
    }
}

#[cfg(test)]
mod tests;

impl<'a> ValueValidation for AwbcValueValidation<'a> {
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
                if let Type::Choice(alternatives) = self.row(ty)?.shape() {
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
            Children::Record(fields) => fields.get(index).map(|field| Expected::Type(field.ty)),
            Children::MapEntry { key, .. } if index == 0 => Some(Expected::Type(*key)),
            Children::MapEntry { value, .. } if index == 1 => Some(Expected::Type(*value)),
            Children::Reduction(state) if index == 0 => Some(Expected::Type(*state)),
            Children::Any | Children::Reduction(_) => Some(Expected::Admitted),
            Children::None | Children::Single(_) | Children::MapEntry { .. } => None,
        };
        child.ok_or_else(|| RuntimeSchemaError::Encoding {
            message: "validated AWBC value has an unexpected child".to_owned(),
        })
    }
}
