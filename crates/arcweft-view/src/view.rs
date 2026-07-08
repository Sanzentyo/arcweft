//! Typed View view descriptors and registry data.

use crate::ViewError;
use arcweft_id::PublicId;
use std::collections::BTreeMap;

/// Stable view identifier resolved at bundle/load time.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewId(pub u32);

/// Stable identifier for a view call/property schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewSchemaId(pub u32);

/// Stable identifier for an Arcweft-authored View program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewProgramId(pub u32);

/// Stable identifier for a host-registered Rust view implementation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustViewId(pub u32);

/// Implementation family for a View view descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewImplementation {
    /// View implementation is registered by the host Rust embedding.
    Rust(RustViewId),
    /// View implementation is an Arcweft View program from the bundle.
    Arcweft(ViewProgramId),
}

/// Bundle/load-time view metadata shared by Rust and Arcweft views.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewDescriptor {
    public_id: Option<PublicId>,
    schema: ViewSchemaId,
    state_schema_hash: u64,
    implementation: ViewImplementation,
}

/// Deterministic view registry keyed by dense `ViewId`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewRegistry {
    descriptors: Vec<ViewDescriptor>,
    public_ids: BTreeMap<PublicId, ViewId>,
}

impl ViewDescriptor {
    pub const fn new(
        public_id: Option<PublicId>,
        schema: ViewSchemaId,
        state_schema_hash: u64,
        implementation: ViewImplementation,
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

    pub const fn schema(&self) -> ViewSchemaId {
        self.schema
    }

    pub const fn state_schema_hash(&self) -> u64 {
        self.state_schema_hash
    }

    pub const fn implementation(&self) -> &ViewImplementation {
        &self.implementation
    }
}

impl ViewRegistry {
    pub fn register(&mut self, descriptor: ViewDescriptor) -> Result<ViewId, ViewError> {
        if let Some(public_id) = descriptor.public_id()
            && self.public_ids.contains_key(public_id)
        {
            return Err(ViewError::DuplicateViewPublicId(public_id.clone()));
        }

        let index =
            u32::try_from(self.descriptors.len()).map_err(|_| ViewError::CapacityExceeded)?;
        let id = ViewId(index);
        if let Some(public_id) = descriptor.public_id().cloned() {
            self.public_ids.insert(public_id, id);
        }
        self.descriptors.push(descriptor);
        Ok(id)
    }

    pub fn get(&self, id: ViewId) -> Option<&ViewDescriptor> {
        self.descriptors.get(id.0 as usize)
    }

    pub fn resolve_public_id(&self, public_id: &PublicId) -> Option<ViewId> {
        self.public_ids.get(public_id).copied()
    }

    pub fn as_slice(&self) -> &[ViewDescriptor] {
        &self.descriptors
    }
}
