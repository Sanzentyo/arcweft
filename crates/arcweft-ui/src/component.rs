//! Typed UI component descriptors and registry data.

use crate::UiError;
use arcweft_id::PublicId;
use std::collections::BTreeMap;

/// Stable component identifier resolved at bundle/load time.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentId(pub u32);

/// Stable identifier for a component call/property schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentSchemaId(pub u32);

/// Stable identifier for an Arcweft-authored UI program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UiProgramId(pub u32);

/// Stable identifier for a host-registered Rust component implementation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustComponentId(pub u32);

/// Implementation family for a UI component descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentImplementation {
    /// Component implementation is registered by the host Rust embedding.
    Rust(RustComponentId),
    /// Component implementation is an Arcweft UI program from the bundle.
    Arcweft(UiProgramId),
}

/// Bundle/load-time component metadata shared by Rust and Arcweft components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentDescriptor {
    public_id: Option<PublicId>,
    schema: ComponentSchemaId,
    state_schema_hash: u64,
    implementation: ComponentImplementation,
}

/// Deterministic component registry keyed by dense `ComponentId`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentRegistry {
    descriptors: Vec<ComponentDescriptor>,
    public_ids: BTreeMap<PublicId, ComponentId>,
}

impl ComponentDescriptor {
    pub const fn new(
        public_id: Option<PublicId>,
        schema: ComponentSchemaId,
        state_schema_hash: u64,
        implementation: ComponentImplementation,
    ) -> Self {
        Self {
            public_id,
            schema,
            state_schema_hash,
            implementation,
        }
    }

    pub const fn public_id(&self) -> Option<&PublicId> {
        self.public_id.as_ref()
    }

    pub const fn schema(&self) -> ComponentSchemaId {
        self.schema
    }

    pub const fn state_schema_hash(&self) -> u64 {
        self.state_schema_hash
    }

    pub const fn implementation(&self) -> &ComponentImplementation {
        &self.implementation
    }
}

impl ComponentRegistry {
    pub fn register(&mut self, descriptor: ComponentDescriptor) -> Result<ComponentId, UiError> {
        if let Some(public_id) = descriptor.public_id()
            && self.public_ids.contains_key(public_id)
        {
            return Err(UiError::DuplicateComponentPublicId(public_id.clone()));
        }

        let index = u32::try_from(self.descriptors.len()).map_err(|_| UiError::CapacityExceeded)?;
        let id = ComponentId(index);
        if let Some(public_id) = descriptor.public_id().cloned() {
            self.public_ids.insert(public_id, id);
        }
        self.descriptors.push(descriptor);
        Ok(id)
    }

    pub fn get(&self, id: ComponentId) -> Option<&ComponentDescriptor> {
        self.descriptors.get(id.0 as usize)
    }

    pub fn resolve_public_id(&self, public_id: &PublicId) -> Option<ComponentId> {
        self.public_ids.get(public_id).copied()
    }

    pub fn as_slice(&self) -> &[ComponentDescriptor] {
        &self.descriptors
    }
}
