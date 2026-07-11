//! Backend-neutral provider ABI and bounded typed output storage.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    capability::{FxRendererInterface, FxRendererInterfaceSet, FxTarget},
    identity::{FxAbiHash, FxId, FxInstanceId, FxSemanticHash},
    plan::ResolvedFxOperation,
    state::{FxGraphChildPath, FxSampleContext},
    value::FxRuntimeValue,
};

/// Provider implementation tier. All tiers expose the same typed ABI.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FxProviderKind {
    Builtin,
    Rust,
    Wasm,
}

/// Bounded output and provider-state contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxProviderLimits {
    pub max_operations: u32,
    pub max_values_per_operation: u16,
    pub max_state_values: u16,
}

/// Registration descriptor shared by builtin, Rust, and WASM implementations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxProviderDescriptor {
    pub id: FxId,
    pub abi_hash: FxAbiHash,
    pub semantic_hash: Option<FxSemanticHash>,
    pub kind: FxProviderKind,
    pub interfaces: FxRendererInterfaceSet,
    pub limits: FxProviderLimits,
}

/// Typed provider request with no GPU, encoder, glyph, or pixel callback.
#[derive(Clone, Copy, Debug)]
pub struct FxProviderRequest<'a> {
    pub instance: FxInstanceId,
    pub definition: &'a FxId,
    pub child_path: &'a FxGraphChildPath,
    pub target: FxTarget,
    pub parameters: &'a [FxRuntimeValue],
    pub context: FxSampleContext,
}

/// Bounded Arcweft-owned output storage passed to providers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FxProviderOutput {
    limits: FxProviderLimits,
    operations: Vec<ResolvedFxOperation>,
}

/// Descriptor registry used before implementation-specific dispatch.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FxProviderRegistry {
    descriptors: BTreeMap<FxId, FxProviderDescriptor>,
}

/// Common provider interface. Implementations may only return typed plan operations.
pub trait FxProvider {
    fn descriptor(&self) -> &FxProviderDescriptor;

    fn evaluate(
        &self,
        request: FxProviderRequest<'_>,
        output: &mut FxProviderOutput,
    ) -> Result<(), FxProviderError>;
}

/// Provider registration, resolution, or bounded-output failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxProviderError {
    #[error("duplicate Fx provider ID `{id}`")]
    DuplicateId { id: FxId },
    #[error("Fx provider `{id}` ABI does not match the definition")]
    AbiMismatch { id: FxId },
    #[error("Fx provider `{id}` semantic hash does not match the definition")]
    SemanticMismatch { id: FxId },
    #[error("Fx provider `{id}` is not available in this host tier")]
    Unavailable { id: FxId },
    #[error("Fx provider `{id}` does not expose renderer interface {interface:?}")]
    UnsupportedInterface {
        id: FxId,
        interface: FxRendererInterface,
    },
    #[error("Fx provider output exceeds its {limit}-operation limit")]
    OutputBudgetExceeded { limit: u32 },
    #[error(
        "Fx provider operation has {actual} values, exceeding its per-operation limit of {limit}"
    )]
    ValueBudgetExceeded { actual: usize, limit: u16 },
}

impl Default for FxProviderLimits {
    fn default() -> Self {
        Self {
            max_operations: 4_096,
            max_values_per_operation: 64,
            max_state_values: 256,
        }
    }
}

impl FxProviderOutput {
    pub fn new(limits: FxProviderLimits) -> Self {
        Self {
            limits,
            operations: Vec::new(),
        }
    }

    pub fn try_push(&mut self, operation: ResolvedFxOperation) -> Result<(), FxProviderError> {
        if self.operations.len() >= self.limits.max_operations as usize {
            return Err(FxProviderError::OutputBudgetExceeded {
                limit: self.limits.max_operations,
            });
        }
        let value_count = match &operation {
            ResolvedFxOperation::Transform(_) => 0,
            ResolvedFxOperation::Values(operation) => operation.values.len(),
        };
        if value_count > usize::from(self.limits.max_values_per_operation) {
            return Err(FxProviderError::ValueBudgetExceeded {
                actual: value_count,
                limit: self.limits.max_values_per_operation,
            });
        }
        self.operations.push(operation);
        Ok(())
    }

    pub fn operations(&self) -> &[ResolvedFxOperation] {
        &self.operations
    }

    pub fn into_operations(self) -> Vec<ResolvedFxOperation> {
        self.operations
    }
}

impl FxProviderRegistry {
    pub fn register(&mut self, descriptor: FxProviderDescriptor) -> Result<(), FxProviderError> {
        if self.descriptors.contains_key(&descriptor.id) {
            return Err(FxProviderError::DuplicateId { id: descriptor.id });
        }
        self.descriptors.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    pub fn resolve(
        &self,
        id: &FxId,
        abi_hash: FxAbiHash,
        semantic_hash: FxSemanticHash,
        available_kinds: &BTreeSet<FxProviderKind>,
        required_interfaces: &FxRendererInterfaceSet,
    ) -> Result<&FxProviderDescriptor, FxProviderError> {
        let descriptor = self
            .descriptors
            .get(id)
            .ok_or_else(|| FxProviderError::Unavailable { id: id.clone() })?;
        if descriptor.abi_hash != abi_hash {
            return Err(FxProviderError::AbiMismatch { id: id.clone() });
        }
        if descriptor
            .semantic_hash
            .is_some_and(|hash| hash != semantic_hash)
        {
            return Err(FxProviderError::SemanticMismatch { id: id.clone() });
        }
        if !available_kinds.contains(&descriptor.kind) {
            return Err(FxProviderError::Unavailable { id: id.clone() });
        }
        if let Some(interface) = required_interfaces
            .iter()
            .find(|interface| !descriptor.interfaces.contains(*interface))
        {
            return Err(FxProviderError::UnsupportedInterface {
                id: id.clone(),
                interface,
            });
        }
        Ok(descriptor)
    }

    pub fn descriptors(&self) -> impl ExactSizeIterator<Item = &FxProviderDescriptor> {
        self.descriptors.values()
    }
}
