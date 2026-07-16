//! Process-local View registry slots and implementation descriptors.

use super::{RustViewId, ViewId, ViewProgramId, ViewSchemaId};
use std::collections::BTreeMap;
use thiserror::Error;

/// Opaque process-local slot in one [`ViewRegistry`].
///
/// Registry slots are deliberately not serializable. Stable external state
/// must use [`ViewId`] instead.
///
/// ```compile_fail
/// use arcweft_view::ViewRegistryId;
///
/// fn serialize_dense_slot(id: ViewRegistryId) {
///     let _ = serde_json::to_string(&id);
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewRegistryId(u32);

/// Implementation family for one registered View.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewImplementation {
    Rust(RustViewId),
    Arcweft { program: ViewProgramId },
}

/// Validated registry metadata for one host or Arcweft View.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewDescriptor {
    id: Option<ViewId>,
    schema: ViewSchemaId,
    implementation: ViewImplementation,
}

/// Deterministic registry with non-reused process-local slots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewRegistry {
    slots: Vec<Option<ViewDescriptor>>,
    public: BTreeMap<ViewId, ViewRegistryId>,
}

/// Failure to register, resolve, or retire a View implementation.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewRegistryError {
    #[error("duplicate public View identity {0:?}")]
    DuplicateViewId(ViewId),
    #[error("View registry capacity is exhausted")]
    CapacityExceeded,
    #[error("registry slot {0:?} is not live")]
    Vacant(ViewRegistryId),
    #[error("only an Arcweft entry can be retired by program replacement")]
    NotArcweft,
    #[error("Arcweft program identity does not match the live descriptor")]
    ProgramMismatch,
}

impl ViewRegistryId {
    pub(crate) fn try_from_index(index: usize) -> Result<Self, ViewRegistryError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| ViewRegistryError::CapacityExceeded)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ViewDescriptor {
    pub const fn anonymous_rust(schema: ViewSchemaId, rust: RustViewId) -> Self {
        Self {
            id: None,
            schema,
            implementation: ViewImplementation::Rust(rust),
        }
    }

    pub const fn public_rust(id: ViewId, schema: ViewSchemaId, rust: RustViewId) -> Self {
        Self {
            id: Some(id),
            schema,
            implementation: ViewImplementation::Rust(rust),
        }
    }

    pub(crate) const fn arcweft(id: ViewId, schema: ViewSchemaId, program: ViewProgramId) -> Self {
        Self {
            id: Some(id),
            schema,
            implementation: ViewImplementation::Arcweft { program },
        }
    }

    pub const fn id(&self) -> Option<&ViewId> {
        self.id.as_ref()
    }

    pub const fn schema(&self) -> ViewSchemaId {
        self.schema
    }

    pub const fn implementation(&self) -> &ViewImplementation {
        &self.implementation
    }
}

impl ViewRegistry {
    pub fn register(
        &mut self,
        descriptor: ViewDescriptor,
    ) -> Result<ViewRegistryId, ViewRegistryError> {
        if let Some(id) = descriptor.id()
            && self.public.contains_key(id)
        {
            return Err(ViewRegistryError::DuplicateViewId(id.clone()));
        }

        let registry_id = ViewRegistryId::try_from_index(self.slots.len())?;
        let public_id = descriptor.id().cloned();

        self.slots.push(Some(descriptor));
        if let Some(public_id) = public_id {
            self.public.insert(public_id, registry_id);
        }
        Ok(registry_id)
    }

    pub fn get(&self, id: ViewRegistryId) -> Option<&ViewDescriptor> {
        self.slots.get(id.index())?.as_ref()
    }

    pub fn resolve(&self, id: &ViewId) -> Option<ViewRegistryId> {
        self.public.get(id).copied()
    }

    /// Registers an Arcweft implementation by stable owner and program identity.
    ///
    /// The process-local slot remains an implementation detail of the registry.
    pub fn register_arcweft(
        &mut self,
        id: ViewId,
        schema: ViewSchemaId,
        program: ViewProgramId,
    ) -> Result<ViewRegistryId, ViewRegistryError> {
        self.register(ViewDescriptor::arcweft(id, schema, program))
    }

    /// Retires one Arcweft implementation by stable owner and expected program.
    ///
    /// Retired process-local slots become permanent tombstones.
    pub fn retire_arcweft(
        &mut self,
        id: &ViewId,
        expected_program: &ViewProgramId,
    ) -> Result<(), ViewRegistryError> {
        self.retire_arcweft_slot(id, expected_program).map(drop)
    }

    fn retire_arcweft_slot(
        &mut self,
        id: &ViewId,
        expected_program: &ViewProgramId,
    ) -> Result<ViewRegistryId, ViewRegistryError> {
        let registry_id = self.resolve(id).ok_or(ViewRegistryError::NotArcweft)?;
        let descriptor = self
            .get(registry_id)
            .ok_or(ViewRegistryError::Vacant(registry_id))?;
        match descriptor.implementation() {
            ViewImplementation::Arcweft { program } if program == expected_program => {}
            ViewImplementation::Arcweft { .. } => {
                return Err(ViewRegistryError::ProgramMismatch);
            }
            ViewImplementation::Rust(_) => return Err(ViewRegistryError::NotArcweft),
        }

        self.public.remove(id);
        self.slots[registry_id.index()] = None;
        Ok(registry_id)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RustViewId, ViewDescriptor, ViewId, ViewImplementation, ViewProgramId, ViewRegistry,
        ViewRegistryError, ViewSchemaId,
    };

    fn view_id(value: &str) -> ViewId {
        ViewId::try_new(value).unwrap()
    }

    fn program_id(value: &str) -> ViewProgramId {
        ViewProgramId::try_new(value).unwrap()
    }

    #[test]
    fn anonymous_and_public_rust_views_have_distinct_registry_capabilities() {
        let mut registry = ViewRegistry::default();
        let anonymous = registry
            .register(ViewDescriptor::anonymous_rust(
                ViewSchemaId(1),
                RustViewId(1),
            ))
            .unwrap();
        let public_id = view_id("view.host.public");
        let public = registry
            .register(ViewDescriptor::public_rust(
                public_id.clone(),
                ViewSchemaId(2),
                RustViewId(2),
            ))
            .unwrap();

        assert_ne!(anonymous, public);
        assert_eq!(registry.get(anonymous).unwrap().id(), None);
        assert_eq!(registry.resolve(&public_id), Some(public));
        assert_eq!(
            registry.get(public).unwrap().implementation(),
            &ViewImplementation::Rust(RustViewId(2))
        );
    }

    #[test]
    fn duplicate_public_registration_is_candidate_first() {
        let mut registry = ViewRegistry::default();
        let id = view_id("view.shared");
        registry
            .register(ViewDescriptor::public_rust(
                id.clone(),
                ViewSchemaId(1),
                RustViewId(1),
            ))
            .unwrap();
        let before = registry.clone();

        assert_eq!(
            registry
                .register_arcweft(id.clone(), ViewSchemaId(2), program_id("view-program.main"),),
            Err(ViewRegistryError::DuplicateViewId(id))
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn retired_arcweft_slots_are_tombstones_and_are_not_reused() {
        let mut registry = ViewRegistry::default();
        let id = view_id("view.arcweft.first");
        let program = program_id("view-program.main");
        registry
            .register_arcweft(id.clone(), ViewSchemaId(1), program.clone())
            .unwrap();
        let retired = registry.resolve(&id).unwrap();

        assert_eq!(registry.retire_arcweft(&id, &program), Ok(()));
        assert!(registry.get(retired).is_none());
        assert_eq!(registry.resolve(&id), None);

        let replacement = registry
            .register(ViewDescriptor::anonymous_rust(
                ViewSchemaId(2),
                RustViewId(2),
            ))
            .unwrap();
        assert_ne!(replacement, retired);
        assert!(registry.get(retired).is_none());
    }
}
