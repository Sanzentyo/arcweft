//! Typed runtime carrier for a partially applied project function.
//!
//! A project continuation is deliberately not represented by a callable
//! label, a registry entry, or an ordinary `RuntimeFunctionValue`.  The
//! checked callable owner supplies the lineage and the complete ABI, while
//! this module owns the once-evaluated prefix values and validates that they
//! remain paired with that ABI.

use super::{RuntimeValue, RuntimeValueShape};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{RuntimePlan, RuntimePlanTypeProjection, RuntimePlanValueTypeError};
use crate::runtime_id::RuntimePlanTypeId;
use arcweft_id::runtime_program::RuntimeProjectContinuationLineageId;
use serde::{Deserializer, Serialize, Serializer, de::Error as _, ser::Error as _};
use thiserror::Error;

/// The exact checked ABI retained by one project-call continuation.
///
/// `prefix_types` is closed-group/ABI order and has the same length as the
/// continuation's `prefix_values`.  `function_type` is the final closed
/// function type, not a type inferred from the values at runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectContinuationAbi {
    lineage: RuntimeProjectContinuationLineageId,
    function_type: RuntimeSemanticTypeId,
    prefix_types: Box<[RuntimeSemanticTypeId]>,
}

impl RuntimeProjectContinuationAbi {
    /// Constructs an ABI after checking every referenced plan type and the
    /// final function-family requirement.
    pub fn try_new(
        plan: &RuntimePlan,
        lineage: RuntimeProjectContinuationLineageId,
        function_type: RuntimeSemanticTypeId,
        prefix_types: impl IntoIterator<Item = RuntimeSemanticTypeId>,
    ) -> Result<Self, RuntimeProjectContinuationAbiError> {
        let prefix_types = prefix_types.into_iter().collect::<Box<[_]>>();
        resolve_type_schema(plan, function_type, TypeRole::Function)?;
        for &ty in &prefix_types {
            resolve_type_schema(plan, ty, TypeRole::Prefix)?;
        }
        Ok(Self {
            lineage,
            function_type,
            prefix_types,
        })
    }

    /// Constructs an ABI from a snapshot's already decoded nonzero type
    /// ordinals.  Plan membership and value compatibility are checked when a
    /// snapshot is admitted to its generation-pinned program.
    pub(crate) fn from_snapshot_parts(
        lineage: RuntimeProjectContinuationLineageId,
        function_type: RuntimeSemanticTypeId,
        prefix_types: Box<[RuntimeSemanticTypeId]>,
    ) -> Result<Self, RuntimeProjectContinuationAbiError> {
        Ok(Self {
            lineage,
            function_type,
            prefix_types,
        })
    }

    /// Constructs the plan-local ABI after semantic construction has already
    /// rewritten every type identity.  Unlike [`Self::try_new`], this helper
    /// deliberately does not need a complete `RuntimePlan`: the aggregate
    /// builder performs membership and projection validation while lowering
    /// the enclosing project-call plan.  Keeping this constructor private to
    /// the crate prevents untyped callers from manufacturing a continuation
    /// ABI at a runtime boundary.
    pub(crate) fn from_admitted_parts(
        lineage: RuntimeProjectContinuationLineageId,
        function_type: RuntimeSemanticTypeId,
        prefix_types: Box<[RuntimeSemanticTypeId]>,
    ) -> Self {
        Self {
            lineage,
            function_type,
            prefix_types,
        }
    }

    #[must_use]
    pub const fn lineage(&self) -> RuntimeProjectContinuationLineageId {
        self.lineage
    }

    #[must_use]
    pub const fn function_type(&self) -> RuntimeSemanticTypeId {
        self.function_type
    }

    #[must_use]
    pub fn prefix_types(&self) -> &[RuntimeSemanticTypeId] {
        &self.prefix_types
    }
}

/// A once-evaluated prefix paired with its exact project-call ABI.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectContinuation {
    abi: RuntimeProjectContinuationAbi,
    prefix_values: Box<[RuntimeValue]>,
}

impl RuntimeProjectContinuation {
    /// Creates a continuation and validates the complete prefix against the
    /// owning plan's type table.  Callers must pass values in ABI order; this
    /// constructor never infers or reconstructs a type schema from values.
    pub fn try_new(
        plan: &RuntimePlan,
        abi: RuntimeProjectContinuationAbi,
        prefix_values: impl IntoIterator<Item = RuntimeValue>,
    ) -> Result<Self, RuntimeProjectContinuationError> {
        let prefix_values = prefix_values.into_iter().collect::<Box<[_]>>();
        let continuation = Self { abi, prefix_values };
        continuation.validate_against_plan(plan)?;
        Ok(continuation)
    }

    /// Reconstructs a value from an AWBC session snapshot. The snapshot
    /// admission layer performs generation-pinned semantic type validation
    /// after this shape-preserving conversion; this method only enforces the
    /// local ABI cardinality invariant.
    pub(crate) fn from_snapshot_parts(
        abi: RuntimeProjectContinuationAbi,
        prefix_values: Box<[RuntimeValue]>,
    ) -> Result<Self, RuntimeProjectContinuationError> {
        if prefix_values.len() != abi.prefix_types.len() {
            return Err(RuntimeProjectContinuationError::PrefixArity {
                expected: abi.prefix_types.len(),
                actual: prefix_values.len(),
            });
        }
        Ok(Self { abi, prefix_values })
    }

    /// Revalidates a restored continuation against the exact owning plan.
    pub(crate) fn validate_against_plan(
        &self,
        plan: &RuntimePlan,
    ) -> Result<(), RuntimeProjectContinuationError> {
        resolve_type_schema(plan, self.abi.function_type, TypeRole::Function)?;
        let prefix_types = self
            .abi
            .prefix_types
            .iter()
            .copied()
            .map(|ty| resolve_type_schema(plan, ty, TypeRole::Prefix))
            .collect::<Result<Vec<_>, _>>()?;
        if self.prefix_values.len() != self.abi.prefix_types.len() {
            return Err(RuntimeProjectContinuationError::PrefixArity {
                expected: self.abi.prefix_types.len(),
                actual: self.prefix_values.len(),
            });
        }
        for (position, (value, &expected_plan)) in
            self.prefix_values.iter().zip(&prefix_types).enumerate()
        {
            if !plan.value_matches_type(expected_plan, value)? {
                return Err(RuntimeProjectContinuationError::PrefixTypeMismatch {
                    position,
                    expected: self.abi.prefix_types[position],
                    actual: value.shape(),
                });
            }
        }
        Ok(())
    }

    /// Confirms that this value is the exact callee expected by a checked
    /// continuation application. Equality is structural over the sealed ABI;
    /// no declaration or type is looked up from the lineage.
    pub(crate) fn validate_expected_abi(
        &self,
        expected: &RuntimeProjectContinuationAbi,
    ) -> Result<(), RuntimeProjectContinuationError> {
        if self.lineage() != expected.lineage() {
            return Err(RuntimeProjectContinuationError::LineageMismatch {
                expected: expected.lineage(),
                actual: self.lineage(),
            });
        }
        if self.function_type() != expected.function_type() {
            return Err(RuntimeProjectContinuationError::FunctionTypeMismatch {
                expected: expected.function_type(),
                actual: self.function_type(),
            });
        }
        if self.prefix_types() != expected.prefix_types() {
            return Err(RuntimeProjectContinuationError::PrefixSchemaMismatch);
        }
        Ok(())
    }

    /// Appends one checked current-group prefix to this continuation.  The
    /// result ABI must retain the old prefix schema exactly and extend it with
    /// the current group's types.  The original value is not mutated, so a
    /// call site can publish at most one newly constructed continuation.
    pub fn try_extend(
        &self,
        plan: &RuntimePlan,
        result_abi: RuntimeProjectContinuationAbi,
        current_values: impl IntoIterator<Item = RuntimeValue>,
    ) -> Result<Self, RuntimeProjectContinuationError> {
        self.validate_against_plan(plan)?;
        let current_values = current_values.into_iter().collect::<Vec<_>>();
        let old_prefix = self.abi.prefix_types();
        let result_prefix = result_abi.prefix_types();
        if result_prefix.len() < old_prefix.len()
            || result_prefix[..old_prefix.len()] != *old_prefix
        {
            return Err(RuntimeProjectContinuationError::PrefixSchemaMismatch);
        }
        let expected_current = &result_prefix[old_prefix.len()..];
        if current_values.len() != expected_current.len() {
            return Err(RuntimeProjectContinuationError::CurrentArity {
                expected: expected_current.len(),
                actual: current_values.len(),
            });
        }
        for (position, (value, &expected)) in
            current_values.iter().zip(expected_current).enumerate()
        {
            let expected_plan_type = resolve_type_schema(plan, expected, TypeRole::Prefix)?;
            if !plan.value_matches_type(expected_plan_type, value)? {
                return Err(RuntimeProjectContinuationError::CurrentTypeMismatch {
                    position: old_prefix.len() + position,
                    expected,
                    actual: value.shape(),
                });
            }
        }
        let mut values = self.prefix_values.to_vec();
        values.extend(current_values);
        Self::try_new(plan, result_abi, values)
    }

    #[must_use]
    pub const fn abi(&self) -> &RuntimeProjectContinuationAbi {
        &self.abi
    }

    #[must_use]
    pub const fn lineage(&self) -> RuntimeProjectContinuationLineageId {
        self.abi.lineage
    }

    #[must_use]
    pub const fn function_type(&self) -> RuntimeSemanticTypeId {
        self.abi.function_type
    }

    #[must_use]
    pub fn prefix_types(&self) -> &[RuntimeSemanticTypeId] {
        &self.abi.prefix_types
    }

    #[must_use]
    pub fn prefix_values(&self) -> &[RuntimeValue] {
        &self.prefix_values
    }
}

impl Serialize for RuntimeProjectContinuation {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(S::Error::custom(
            "project continuations cannot cross a generic persistence boundary; use the AWBC session-save DTO",
        ))
    }
}

impl<'de> serde::Deserialize<'de> for RuntimeProjectContinuation {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(D::Error::custom(
            "project continuations cannot cross a generic persistence boundary; use the AWBC session-save DTO",
        ))
    }
}

/// Failure to validate one continuation ABI or prefix.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProjectContinuationError {
    #[error(transparent)]
    Abi(#[from] RuntimeProjectContinuationAbiError),
    #[error("project continuation prefix has {actual} values; expected {expected}")]
    PrefixArity { expected: usize, actual: usize },
    #[error(
        "project continuation prefix value {position} has shape {actual:?}; expected semantic type {expected:?}"
    )]
    PrefixTypeMismatch {
        position: usize,
        expected: RuntimeSemanticTypeId,
        actual: RuntimeValueShape,
    },
    #[error("project continuation result ABI does not retain the existing prefix schema")]
    PrefixSchemaMismatch,
    #[error(
        "project continuation function type mismatch: expected {expected:?}, received {actual:?}"
    )]
    FunctionTypeMismatch {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error("project continuation lineage mismatch: expected {expected}, received {actual}")]
    LineageMismatch {
        expected: RuntimeProjectContinuationLineageId,
        actual: RuntimeProjectContinuationLineageId,
    },
    #[error("project continuation current group has {actual} values; expected {expected}")]
    CurrentArity { expected: usize, actual: usize },
    #[error(
        "project continuation current value {position} has shape {actual:?}; expected semantic type {expected:?}"
    )]
    CurrentTypeMismatch {
        position: usize,
        expected: RuntimeSemanticTypeId,
        actual: RuntimeValueShape,
    },
    #[error(transparent)]
    ValueType(#[from] RuntimePlanValueTypeError),
}

/// Failure to construct a continuation ABI before it is paired with values.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProjectContinuationAbiError {
    #[error("project continuation references unknown function semantic type {ty:?}")]
    UnknownFunctionType { ty: RuntimeSemanticTypeId },
    #[error("project continuation references unknown prefix semantic type {ty:?}")]
    UnknownPrefixType { ty: RuntimeSemanticTypeId },
    #[error("project continuation function semantic type {ty:?} is not a function type")]
    FunctionTypeNotFunction { ty: RuntimeSemanticTypeId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeRole {
    Function,
    Prefix,
}

fn resolve_type_schema(
    plan: &RuntimePlan,
    ty: RuntimeSemanticTypeId,
    role: TypeRole,
) -> Result<RuntimePlanTypeId, RuntimeProjectContinuationAbiError> {
    let Some(plan_ty) = plan.type_table().id_for_semantic(ty) else {
        return Err(match role {
            TypeRole::Function => RuntimeProjectContinuationAbiError::UnknownFunctionType { ty },
            TypeRole::Prefix => RuntimeProjectContinuationAbiError::UnknownPrefixType { ty },
        });
    };
    let Some(declaration) = plan.type_table().get(plan_ty) else {
        return Err(match role {
            TypeRole::Function => RuntimeProjectContinuationAbiError::UnknownFunctionType { ty },
            TypeRole::Prefix => RuntimeProjectContinuationAbiError::UnknownPrefixType { ty },
        });
    };
    if matches!(role, TypeRole::Function)
        && !matches!(
            declaration.projection(),
            RuntimePlanTypeProjection::Function { .. }
        )
    {
        return Err(RuntimeProjectContinuationAbiError::FunctionTypeNotFunction { ty });
    }
    Ok(plan_ty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{AwbcRuntimeValueSnapshot, RuntimeValue};

    fn type_id(raw: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([raw; 32])
    }

    fn continuation() -> RuntimeProjectContinuation {
        let abi = RuntimeProjectContinuationAbi::from_snapshot_parts(
            RuntimeProjectContinuationLineageId::from_checked_digest([7; 32]),
            type_id(1),
            Box::new([type_id(2)]),
        )
        .expect("snapshot ABI shape is valid");
        RuntimeProjectContinuation::from_snapshot_parts(abi, Box::new([RuntimeValue::Unit]))
            .expect("snapshot prefix cardinality is valid")
    }

    #[test]
    fn generic_runtime_value_serde_rejects_project_continuation() {
        let value = RuntimeValue::ProjectContinuation(continuation());
        assert!(serde_json::to_value(value).is_err());
    }

    #[test]
    fn awbc_snapshot_round_trips_project_continuation_abi_and_values() {
        let value = RuntimeValue::ProjectContinuation(continuation());
        let snapshot = AwbcRuntimeValueSnapshot::from_runtime_value(&value)
            .expect("project continuation enters the AWBC save DTO");
        let encoded = serde_json::to_vec(&snapshot).expect("snapshot serializes");
        let decoded: AwbcRuntimeValueSnapshot =
            serde_json::from_slice(&encoded).expect("snapshot deserializes");
        let restored = decoded
            .into_runtime_value()
            .expect("project continuation restores");
        assert_eq!(restored, value);
    }

    #[test]
    fn snapshot_rejects_project_continuation_prefix_arity_mismatch() {
        let abi = RuntimeProjectContinuationAbi::from_snapshot_parts(
            RuntimeProjectContinuationLineageId::from_checked_digest([9; 32]),
            type_id(1),
            Box::new([type_id(2)]),
        )
        .expect("snapshot ABI shape is valid");
        assert!(matches!(
            RuntimeProjectContinuation::from_snapshot_parts(abi, Box::new([])),
            Err(RuntimeProjectContinuationError::PrefixArity { .. })
        ));
    }
}
