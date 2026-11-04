//! Declaration linkage for checked external calls. Authored aliases and runtime
//! display labels are not implementation identities.

use serde::{Deserialize, Serialize};

use crate::entry::{RuntimeCallableId, RuntimeIdentityError};
use crate::plan::RuntimeCallableParameterCoordinate;
pub use arcweft_manifest_model::StandardEnvironmentId;

/// One source operand's accepted formal destination and expanded ABI range.
/// Missing coordinates are explicit for operands that span several formals;
/// consumers must not infer a named parameter from its value type.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeExternalCallOperand {
    pub parameter: Option<RuntimeCallableParameterCoordinate>,
    pub abi_position: u32,
    pub expansion_width: u32,
}

#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum RuntimeExternalCallOperandError {
    #[error("external operands do not cover one contiguous ABI range")]
    InvalidRange,
    #[error("external operand {operand} differs from the admitted call argument")]
    ArgumentMismatch { operand: usize },
    #[error("external operand count differs: expected {expected}, found {actual}")]
    ArgumentCount { expected: usize, actual: usize },
}

impl RuntimeExternalCallOperand {
    pub(crate) const fn validation_scratch_element_bytes() -> usize {
        size_of::<(u32, u32)>()
    }

    pub(crate) fn preflight_layout(
        operands: impl ExactSizeIterator<Item = Self>,
    ) -> Result<(), RuntimeExternalCallOperandError> {
        let mut ranges = operands
            .map(|row| (row.abi_position, row.expansion_width))
            .collect::<Vec<_>>();
        // Empty spreads at a boundary precede its nonempty range. Unstable
        // sorting needs no second scratch allocation and leaves source rows intact.
        ranges.sort_unstable();
        let mut next = 0_u32;
        for (position, width) in ranges {
            if position != next {
                return Err(RuntimeExternalCallOperandError::InvalidRange);
            }
            next = next
                .checked_add(width)
                .ok_or(RuntimeExternalCallOperandError::InvalidRange)?;
        }
        Ok(())
    }
}

/// Registration authority, independent of a source binding or alias.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeExternalCallableProvider {
    Standard(StandardEnvironmentId),
    Adapter(String),
}

/// Complete declaration lookup key within its registration authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeExternalCallableExport {
    Function(Box<[String]>),
    Method { receiver: [u8; 32], name: String },
    RustFunction(Box<[String]>),
}

/// Selected external declaration, preserving provider, kind and overload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "DeclarationFields")]
pub struct RuntimeExternalCallableDeclaration {
    provider: RuntimeExternalCallableProvider,
    export: RuntimeExternalCallableExport,
    overload: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclarationFields {
    provider: RuntimeExternalCallableProvider,
    export: RuntimeExternalCallableExport,
    overload: u16,
}

impl TryFrom<DeclarationFields> for RuntimeExternalCallableDeclaration {
    type Error = RuntimeIdentityError;

    fn try_from(fields: DeclarationFields) -> Result<Self, Self::Error> {
        Self::try_new(fields.provider, fields.export, fields.overload)
    }
}

impl RuntimeExternalCallableDeclaration {
    pub fn try_new(
        provider: RuntimeExternalCallableProvider,
        export: RuntimeExternalCallableExport,
        overload: u16,
    ) -> Result<Self, RuntimeIdentityError> {
        if let RuntimeExternalCallableProvider::Adapter(package) = &provider {
            validate_component("external callable provider", package)?;
        }
        match &export {
            RuntimeExternalCallableExport::Function(path)
            | RuntimeExternalCallableExport::RustFunction(path) => {
                if path.is_empty() {
                    return Err(RuntimeIdentityError::Empty {
                        kind: "external callable path",
                    });
                }
                for name in path {
                    validate_component("external callable path segment", name)?;
                }
            }
            RuntimeExternalCallableExport::Method { name, .. } => {
                validate_component("external callable method", name)?;
            }
        }
        Ok(Self {
            provider,
            export,
            overload,
        })
    }

    pub const fn provider(&self) -> &RuntimeExternalCallableProvider {
        &self.provider
    }

    pub const fn export(&self) -> &RuntimeExternalCallableExport {
        &self.export
    }

    pub const fn overload(&self) -> u16 {
        self.overload
    }
}

fn validate_component(kind: &'static str, value: &str) -> Result<(), RuntimeIdentityError> {
    if value.is_empty() {
        return Err(RuntimeIdentityError::Empty { kind });
    }
    if let Some((byte, _)) = value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(RuntimeIdentityError::Control { kind, byte });
    }
    Ok(())
}

/// Checked semantic identity and its selected declaration linkage. The checked
/// identity still binds the complete signature and accepted catalog generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "CallableFields")]
pub struct RuntimeExternalCallable {
    identity: RuntimeCallableId,
    declaration: RuntimeExternalCallableDeclaration,
    operands: Box<[RuntimeExternalCallOperand]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallableFields {
    identity: RuntimeCallableId,
    declaration: RuntimeExternalCallableDeclaration,
    operands: Box<[RuntimeExternalCallOperand]>,
}

impl TryFrom<CallableFields> for RuntimeExternalCallable {
    type Error = RuntimeExternalCallOperandError;

    fn try_from(fields: CallableFields) -> Result<Self, Self::Error> {
        Self::new(fields.identity, fields.declaration).with_operands(fields.operands)
    }
}

impl RuntimeExternalCallable {
    pub fn new(
        identity: RuntimeCallableId,
        declaration: RuntimeExternalCallableDeclaration,
    ) -> Self {
        Self {
            identity,
            declaration,
            operands: Box::new([]),
        }
    }

    /// Binds accepted source-order operand destinations to this invocation.
    /// An empty declaration is executable only with an empty argument list.
    pub fn with_operands(
        mut self,
        operands: Box<[RuntimeExternalCallOperand]>,
    ) -> Result<Self, RuntimeExternalCallOperandError> {
        RuntimeExternalCallOperand::preflight_layout(operands.iter().copied())?;
        self.operands = operands;
        Ok(self)
    }

    #[must_use]
    pub fn operands(&self) -> &[RuntimeExternalCallOperand] {
        &self.operands
    }

    /// Checks the invocation against the original expression's operand groups.
    pub fn validate_arguments(
        &self,
        count: usize,
        mut argument: impl FnMut(usize) -> Option<(u32, u32)>,
    ) -> Result<(), RuntimeExternalCallOperandError> {
        if self.operands.len() != count {
            return Err(RuntimeExternalCallOperandError::ArgumentCount {
                expected: self.operands.len(),
                actual: count,
            });
        }
        for (index, row) in self.operands.iter().enumerate() {
            if argument(index) != Some((row.abi_position, row.expansion_width)) {
                return Err(RuntimeExternalCallOperandError::ArgumentMismatch { operand: index });
            }
        }
        Ok(())
    }

    pub const fn identity(&self) -> &RuntimeCallableId {
        &self.identity
    }

    pub const fn declaration(&self) -> &RuntimeExternalCallableDeclaration {
        &self.declaration
    }

    pub(crate) fn encode_body_metadata(
        &self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        encoder.string(self.identity.as_str());
        match self.declaration.provider() {
            RuntimeExternalCallableProvider::Standard(provider) => {
                encoder.tag(0);
                encoder.tag(provider.canonical_tag());
            }
            RuntimeExternalCallableProvider::Adapter(package) => {
                encoder.tag(1);
                encoder.string(package);
            }
        }
        match self.declaration.export() {
            RuntimeExternalCallableExport::Function(path)
            | RuntimeExternalCallableExport::RustFunction(path) => {
                encoder.tag(
                    if matches!(
                        self.declaration.export(),
                        RuntimeExternalCallableExport::Function(_)
                    ) {
                        0
                    } else {
                        2
                    },
                );
                encoder.count(path.len());
                for segment in path {
                    encoder.enter_element();
                    encoder.string(segment);
                }
            }
            RuntimeExternalCallableExport::Method { receiver, name } => {
                encoder.tag(1);
                encoder.digest(receiver);
                encoder.string(name);
            }
        }
        encoder.ordinal(u32::from(self.declaration.overload()));
        encoder.count(self.operands.len());
        for row in &self.operands {
            encoder.enter_element();
            if let Some(parameter) = row.parameter {
                encoder.tag(1);
                encoder.ordinal(parameter.group);
                encoder.ordinal(parameter.parameter);
            } else {
                encoder.tag(0);
            }
            encoder.ordinal(row.abi_position);
            encoder.ordinal(row.expansion_width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

    fn declaration() -> RuntimeExternalCallable {
        RuntimeExternalCallable::new(
            RuntimeCallableId::from_checked_digest([0xb7; 32]),
            RuntimeExternalCallableDeclaration::try_new(
                RuntimeExternalCallableProvider::Standard(StandardEnvironmentId::Core),
                RuntimeExternalCallableExport::Function(Box::new(["fixture".into()])),
                0,
            )
            .unwrap(),
        )
    }

    fn operand(parameter: u32, position: u32, width: u32) -> RuntimeExternalCallOperand {
        RuntimeExternalCallOperand {
            parameter: Some(RuntimeCallableParameterCoordinate {
                group: 2,
                parameter,
            }),
            abi_position: position,
            expansion_width: width,
        }
    }

    #[test]
    fn external_operand_layout_preserves_source_order_and_exact_formal_coordinates() {
        let operands = Box::new([operand(7, 1, 2), operand(2, 0, 1), operand(9, 3, 0)]);
        let call = declaration().with_operands(operands.clone()).unwrap();
        assert_eq!(call.operands(), operands.as_slice());
        assert!(
            call.validate_arguments(3, |index| Some([(1, 2), (0, 1), (3, 0)][index]))
                .is_ok()
        );
        assert_eq!(
            call.validate_arguments(2, |_| None),
            Err(RuntimeExternalCallOperandError::ArgumentCount {
                expected: 3,
                actual: 2
            })
        );
        assert_eq!(
            call.validate_arguments(3, |index| Some([(1, 1), (0, 1), (3, 0)][index])),
            Err(RuntimeExternalCallOperandError::ArgumentMismatch { operand: 0 })
        );
        let bytes = serde_json::to_vec(&call).unwrap();
        assert_eq!(
            serde_json::from_slice::<RuntimeExternalCallable>(&bytes).unwrap(),
            call
        );
    }

    #[test]
    fn external_operand_layout_rejects_gaps_overlap_overflow_and_missing_serialized_rows() {
        for operands in [
            vec![operand(0, 1, 1)],
            vec![operand(0, 0, 2), operand(1, 1, 1)],
            vec![operand(0, 0, u32::MAX), operand(1, u32::MAX, 1)],
        ] {
            assert_eq!(
                declaration().with_operands(operands.into()),
                Err(RuntimeExternalCallOperandError::InvalidRange)
            );
        }
        let mut wire = serde_json::to_value(declaration()).unwrap();
        wire.as_object_mut().unwrap().remove("operands");
        assert!(serde_json::from_value::<RuntimeExternalCallable>(wire).is_err());
        let mut wire = serde_json::to_value(declaration()).unwrap();
        wire["operands"] = serde_json::to_value([operand(0, 1, 1)]).unwrap();
        assert!(serde_json::from_value::<RuntimeExternalCallable>(wire).is_err());
    }

    #[test]
    fn external_operand_coordinates_and_expansion_bind_the_executable_digest() {
        let digest = |row| {
            let call = declaration().with_operands(Box::new([row])).unwrap();
            let mut meter = TaskSemanticMeter::new(u64::MAX, u64::MAX);
            let mut encoder =
                TaskSemanticEncoder::new(b"arcweft.test.external-operands.v1\0", &mut meter);
            call.encode_body_metadata(&mut encoder);
            encoder.finish().unwrap()
        };
        let original = digest(operand(0, 0, 1));
        assert_ne!(original, digest(operand(1, 0, 1)));
        assert_ne!(original, digest(operand(0, 0, 2)));
        let mut other = operand(0, 0, 1);
        other.parameter.as_mut().unwrap().group = 3;
        assert_ne!(original, digest(other));
        other.parameter = None;
        assert_ne!(original, digest(other));
    }

    #[test]
    fn external_callable_semantic_metadata_binds_linkage_as_well_as_checked_identity() {
        let digest = |provider, export, overload| {
            let callable = RuntimeExternalCallable::new(
                RuntimeCallableId::from_checked_digest([0xa1; 32]),
                RuntimeExternalCallableDeclaration::try_new(provider, export, overload).unwrap(),
            );
            let mut meter = TaskSemanticMeter::new(u64::MAX, u64::MAX);
            let mut encoder =
                TaskSemanticEncoder::new(b"arcweft.test.external-call.v1\0", &mut meter);
            callable.encode_body_metadata(&mut encoder);
            encoder.finish().unwrap()
        };
        let provider =
            RuntimeExternalCallableProvider::Standard(StandardEnvironmentId::InferenceTensor);
        let export =
            RuntimeExternalCallableExport::Function(["infer".into(), "add_f32".into()].into());
        let original = digest(provider.clone(), export.clone(), 0);
        assert_ne!(
            original,
            digest(
                RuntimeExternalCallableProvider::Adapter("other".into()),
                export.clone(),
                0
            )
        );
        assert_ne!(original, digest(provider.clone(), export, 1));
        assert_ne!(
            original,
            digest(
                provider.clone(),
                RuntimeExternalCallableExport::Function(["infer.add_f32".into()].into()),
                0
            )
        );
        assert_ne!(
            original,
            digest(
                provider,
                RuntimeExternalCallableExport::RustFunction(
                    ["infer".into(), "add_f32".into()].into()
                ),
                0
            )
        );
    }
}
