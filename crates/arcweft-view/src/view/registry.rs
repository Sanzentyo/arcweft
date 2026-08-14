//! Process-local View registry slots and implementation descriptors.

use super::{
    AcceptedViewProgramRevision, ProjectedRuntimeViewId, RustViewId, ViewId, ViewIdentityError,
    ViewProgramId, ViewSchemaId,
};
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
    Arcweft {
        program: ViewProgramId,
        revision: AcceptedViewProgramRevision,
    },
}

/// Digest of every live public View implementation in one registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewRegistryRuntimeDigest([u8; 32]);

impl ViewRegistryRuntimeDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// String-bearing coordinate in the version-one View registry transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewRegistryStringField {
    ViewId,
    ViewProgramId,
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
    #[error("Arcweft implementation identity does not match the live descriptor")]
    ProgramMismatch,
}

/// Failure to compute the canonical runtime View-registry digest.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewRegistryRuntimeDigestError {
    #[error(transparent)]
    Registry(#[from] ViewRegistryError),
    #[error(transparent)]
    Identity(#[from] ViewIdentityError),
    #[error("View registry contains {observed} live public rows; maximum is {maximum}")]
    EntryLimit { observed: usize, maximum: usize },
    #[error("public View {view} points to vacant registry slot {slot:?}")]
    DanglingPublicIndex { view: ViewId, slot: ViewRegistryId },
    #[error("public View map key {key} differs from descriptor identity {descriptor}")]
    PublicIdMismatch { key: ViewId, descriptor: ViewId },
    #[error("public View map key {view} points to an anonymous descriptor")]
    AnonymousPublicEntry { view: ViewId },
    #[error("View identity field {field:?} has {bytes} UTF-8 bytes; maximum is {maximum}")]
    StringLength {
        view: ViewId,
        field: ViewRegistryStringField,
        bytes: usize,
        maximum: u32,
    },
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

    pub(crate) const fn arcweft(
        id: ViewId,
        schema: ViewSchemaId,
        program: ViewProgramId,
        revision: AcceptedViewProgramRevision,
    ) -> Self {
        Self {
            id: Some(id),
            schema,
            implementation: ViewImplementation::Arcweft { program, revision },
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
    pub const MAX_RUNTIME_DIGEST_PUBLIC_ROWS: usize = 65_536;

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
        revision: AcceptedViewProgramRevision,
    ) -> Result<ViewRegistryId, ViewRegistryError> {
        self.register(ViewDescriptor::arcweft(id, schema, program, revision))
    }

    /// Retires one Arcweft implementation by stable owner and expected program.
    ///
    /// Retired process-local slots become permanent tombstones.
    pub fn retire_arcweft(
        &mut self,
        id: &ViewId,
        expected_program: &ViewProgramId,
        expected_revision: AcceptedViewProgramRevision,
    ) -> Result<(), ViewRegistryError> {
        self.retire_arcweft_slot(id, expected_program, expected_revision)
            .map(drop)
    }

    fn retire_arcweft_slot(
        &mut self,
        id: &ViewId,
        expected_program: &ViewProgramId,
        expected_revision: AcceptedViewProgramRevision,
    ) -> Result<ViewRegistryId, ViewRegistryError> {
        let registry_id = self.resolve(id).ok_or(ViewRegistryError::NotArcweft)?;
        let descriptor = self
            .get(registry_id)
            .ok_or(ViewRegistryError::Vacant(registry_id))?;
        match descriptor.implementation() {
            ViewImplementation::Arcweft { program, revision }
                if program == expected_program && *revision == expected_revision => {}
            ViewImplementation::Arcweft { .. } => {
                return Err(ViewRegistryError::ProgramMismatch);
            }
            ViewImplementation::Rust(_) => return Err(ViewRegistryError::NotArcweft),
        }

        self.public.remove(id);
        self.slots[registry_id.index()] = None;
        Ok(registry_id)
    }

    /// Computes the canonical version-one digest of every live public View.
    pub fn runtime_digest_v1(
        &self,
    ) -> Result<ViewRegistryRuntimeDigest, ViewRegistryRuntimeDigestError> {
        if self.public.len() > Self::MAX_RUNTIME_DIGEST_PUBLIC_ROWS {
            return Err(ViewRegistryRuntimeDigestError::EntryLimit {
                observed: self.public.len(),
                maximum: Self::MAX_RUNTIME_DIGEST_PUBLIC_ROWS,
            });
        }

        for (view, slot) in &self.public {
            let descriptor = self.get(*slot).ok_or_else(|| {
                ViewRegistryRuntimeDigestError::DanglingPublicIndex {
                    view: view.clone(),
                    slot: *slot,
                }
            })?;
            let descriptor_id = descriptor.id().ok_or_else(|| {
                ViewRegistryRuntimeDigestError::AnonymousPublicEntry { view: view.clone() }
            })?;
            if view != descriptor_id {
                return Err(ViewRegistryRuntimeDigestError::PublicIdMismatch {
                    key: view.clone(),
                    descriptor: descriptor_id.clone(),
                });
            }
            validate_string(view, ViewRegistryStringField::ViewId, view.as_str())?;
            if let ViewImplementation::Arcweft { program, .. } = descriptor.implementation() {
                validate_string(
                    view,
                    ViewRegistryStringField::ViewProgramId,
                    program.as_str(),
                )?;
            }
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.view-registry.runtime.v1\0");
        hasher.update(&1_u32.to_le_bytes());
        hasher.update(
            &u32::try_from(self.public.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for (view, slot) in &self.public {
            let descriptor = self.get(*slot).ok_or_else(|| {
                ViewRegistryRuntimeDigestError::DanglingPublicIndex {
                    view: view.clone(),
                    slot: *slot,
                }
            })?;
            hash_string(&mut hasher, view.as_str());
            hash_projected_runtime_view_id(&mut hasher, view.projected_runtime_id_v1());
            hasher.update(&descriptor.schema().0.to_le_bytes());
            match descriptor.implementation() {
                ViewImplementation::Rust(rust) => {
                    hasher.update(&[0x00]);
                    hasher.update(&rust.0.to_le_bytes());
                }
                ViewImplementation::Arcweft { program, revision } => {
                    hasher.update(&[0x01]);
                    hash_string(&mut hasher, program.as_str());
                    hasher.update(revision.as_bytes());
                }
            }
        }
        Ok(ViewRegistryRuntimeDigest(*hasher.finalize().as_bytes()))
    }
}

fn validate_string(
    view: &ViewId,
    field: ViewRegistryStringField,
    value: &str,
) -> Result<(), ViewRegistryRuntimeDigestError> {
    if value.len() > u32::MAX as usize {
        return Err(ViewRegistryRuntimeDigestError::StringLength {
            view: view.clone(),
            field,
            bytes: value.len(),
            maximum: u32::MAX,
        });
    }
    Ok(())
}

fn hash_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_projected_runtime_view_id(hasher: &mut blake3::Hasher, id: ProjectedRuntimeViewId) {
    hasher.update(id.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptedViewProgramRevision, RustViewId, ViewDescriptor, ViewId, ViewImplementation,
        ViewProgramId, ViewRegistry, ViewRegistryError, ViewSchemaId,
    };

    fn view_id(value: &str) -> ViewId {
        ViewId::try_new(value).unwrap()
    }

    fn program_id(value: &str) -> ViewProgramId {
        ViewProgramId::try_new(value).unwrap()
    }

    fn revision(byte: u8) -> AcceptedViewProgramRevision {
        AcceptedViewProgramRevision::try_from_bytes([byte; 32]).unwrap()
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
            registry.register_arcweft(
                id.clone(),
                ViewSchemaId(2),
                program_id("view-program.main"),
                revision(1),
            ),
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
            .register_arcweft(id.clone(), ViewSchemaId(1), program.clone(), revision(1))
            .unwrap();
        let retired = registry.resolve(&id).unwrap();

        assert_eq!(registry.retire_arcweft(&id, &program, revision(1)), Ok(()));
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

    #[test]
    fn runtime_digest_uses_the_complete_little_endian_public_row_grammar() {
        let mut registry = ViewRegistry::default();
        registry
            .register(ViewDescriptor::public_rust(
                view_id("view.rust"),
                ViewSchemaId(0x0102_0304_0506_0708),
                RustViewId(0x0a0b_0c0d),
            ))
            .unwrap();
        registry
            .register_arcweft(
                view_id("view.arcweft"),
                ViewSchemaId(11),
                program_id("view-program.main"),
                revision(0x5a),
            )
            .unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(b"arcweft.view-registry.runtime.v1\0");
        expected.extend_from_slice(&1_u32.to_le_bytes());
        expected.extend_from_slice(&2_u32.to_le_bytes());
        for (view, schema, implementation) in [
            (
                view_id("view.arcweft"),
                ViewSchemaId(11),
                ViewImplementation::Arcweft {
                    program: program_id("view-program.main"),
                    revision: revision(0x5a),
                },
            ),
            (
                view_id("view.rust"),
                ViewSchemaId(0x0102_0304_0506_0708),
                ViewImplementation::Rust(RustViewId(0x0a0b_0c0d)),
            ),
        ] {
            expected.extend_from_slice(
                &u32::try_from(view.as_str().len())
                    .expect("test View identity length")
                    .to_le_bytes(),
            );
            expected.extend_from_slice(view.as_str().as_bytes());
            expected.extend_from_slice(view.projected_runtime_id_v1().as_bytes());
            expected.extend_from_slice(&schema.0.to_le_bytes());
            match implementation {
                ViewImplementation::Rust(rust) => {
                    expected.push(0x00);
                    expected.extend_from_slice(&rust.0.to_le_bytes());
                }
                ViewImplementation::Arcweft { program, revision } => {
                    expected.push(0x01);
                    expected.extend_from_slice(
                        &u32::try_from(program.as_str().len())
                            .expect("test View program identity length")
                            .to_le_bytes(),
                    );
                    expected.extend_from_slice(program.as_str().as_bytes());
                    expected.extend_from_slice(revision.as_bytes());
                }
            }
        }

        assert_eq!(
            registry.runtime_digest_v1().unwrap().as_bytes(),
            blake3::hash(&expected).as_bytes()
        );
    }

    #[test]
    fn runtime_digest_is_public_id_sorted_and_ignores_anonymous_slots() {
        let mut first = ViewRegistry::default();
        first
            .register(ViewDescriptor::anonymous_rust(
                ViewSchemaId(99),
                RustViewId(99),
            ))
            .unwrap();
        first
            .register_arcweft(
                view_id("view.zeta"),
                ViewSchemaId(2),
                program_id("view-program.main"),
                revision(1),
            )
            .unwrap();
        first
            .register(ViewDescriptor::public_rust(
                view_id("view.alpha"),
                ViewSchemaId(1),
                RustViewId(1),
            ))
            .unwrap();

        let mut second = ViewRegistry::default();
        second
            .register(ViewDescriptor::public_rust(
                view_id("view.alpha"),
                ViewSchemaId(1),
                RustViewId(1),
            ))
            .unwrap();
        second
            .register_arcweft(
                view_id("view.zeta"),
                ViewSchemaId(2),
                program_id("view-program.main"),
                revision(1),
            )
            .unwrap();

        assert_eq!(
            first.runtime_digest_v1().unwrap(),
            second.runtime_digest_v1().unwrap()
        );
    }

    #[test]
    fn runtime_digest_excludes_retired_tombstones() {
        let mut with_tombstone = ViewRegistry::default();
        let retired_id = view_id("view.retired");
        let program = program_id("view-program.main");
        with_tombstone
            .register_arcweft(
                retired_id.clone(),
                ViewSchemaId(1),
                program.clone(),
                revision(1),
            )
            .unwrap();
        with_tombstone
            .register(ViewDescriptor::public_rust(
                view_id("view.live"),
                ViewSchemaId(2),
                RustViewId(2),
            ))
            .unwrap();
        with_tombstone
            .retire_arcweft(&retired_id, &program, revision(1))
            .unwrap();

        let mut without_retired = ViewRegistry::default();
        without_retired
            .register(ViewDescriptor::public_rust(
                view_id("view.live"),
                ViewSchemaId(2),
                RustViewId(2),
            ))
            .unwrap();

        assert_eq!(
            with_tombstone.runtime_digest_v1().unwrap(),
            without_retired.runtime_digest_v1().unwrap()
        );
    }

    #[test]
    fn runtime_digest_changes_for_every_arcweft_implementation_identity_field() {
        let digest = |program: &str, revision_byte| {
            let mut registry = ViewRegistry::default();
            registry
                .register_arcweft(
                    view_id("view.arcweft"),
                    ViewSchemaId(1),
                    program_id(program),
                    revision(revision_byte),
                )
                .unwrap();
            registry.runtime_digest_v1().unwrap()
        };

        assert_ne!(
            digest("view-program.first", 1),
            digest("view-program.second", 1)
        );
        assert_ne!(
            digest("view-program.first", 1),
            digest("view-program.first", 2)
        );
    }

    #[test]
    fn runtime_digest_rejects_more_than_the_public_row_limit() {
        let mut registry = ViewRegistry::default();
        for index in 0..=ViewRegistry::MAX_RUNTIME_DIGEST_PUBLIC_ROWS {
            registry
                .register(ViewDescriptor::public_rust(
                    view_id(&format!("view.limit.{index}")),
                    ViewSchemaId(1),
                    RustViewId(1),
                ))
                .unwrap();
        }

        assert!(matches!(
            registry.runtime_digest_v1(),
            Err(super::ViewRegistryRuntimeDigestError::EntryLimit {
                observed: 65_537,
                maximum: 65_536,
            })
        ));
    }
}
