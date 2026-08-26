//! Accepted-world registry for typed `CharacterDialogue` custom patch fields.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceSpan;
use arcweft_view::ViewId;
use thiserror::Error;

use crate::{callable::CallableName, registration::AcceptedNominalWorldStamp, types::TypeKind};

/// Typed coordinate of a field admitted by the CharacterDialogue schema.
///
/// The coordinate is shared by schema construction and final patch rows. It
/// is deliberately not reconstructed from an authored parameter name after
/// call checking.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueFieldCoordinate {
    Voice,
    Look,
    Stage,
    Portrait,
    Focus,
    Cleanup,
    View,
    SourceLocale,
    Hooks,
    Style,
    RichText,
    InlineFailure,
    Custom(CharacterDialogueCustomFieldId),
}

impl CharacterDialogueFieldCoordinate {
    pub const fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }
}

/// One source binding that selects a stable custom-field identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueCustomFieldBinding {
    module: Option<CanonicalModulePath>,
    name: String,
}

/// Complete typed descriptor for one `CharacterDialogue` custom coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueCustomFieldDescriptor {
    id: CharacterDialogueCustomFieldId,
    bindings: Box<[CharacterDialogueCustomFieldBinding]>,
    value_type: TypeKind,
    runtime_nominal_type: Option<RuntimeNominalTypeId>,
    runtime_layout: TypeLayoutHash,
    clearable: bool,
    accepted_views: BTreeSet<ViewId>,
    declaration: SourceSpan,
}

/// Immutable custom-field inventory owned by one exact accepted nominal world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueCustomFieldRegistry {
    world: AcceptedNominalWorldStamp,
    semantic_digest: [u8; 32],
    by_id: BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomFieldDescriptor>,
    bindings: BTreeMap<CharacterDialogueCustomFieldBinding, CharacterDialogueCustomFieldId>,
}

#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueCustomFieldRegistryError {
    #[error("CharacterDialogue custom field `{0}` has no source binding")]
    MissingBinding(CharacterDialogueCustomFieldId),
    #[error("duplicate CharacterDialogue custom field `{0}`")]
    DuplicateId(CharacterDialogueCustomFieldId),
    #[error("duplicate CharacterDialogue custom-field binding `{0}`")]
    DuplicateBinding(String),
    #[error("reserved CharacterDialogue field `{0}` cannot be a custom binding")]
    ReservedBinding(String),
    #[error("CharacterDialogue custom-field binding `{0}` is not a callable parameter name")]
    InvalidBinding(String),
}

impl CharacterDialogueCustomFieldBinding {
    #[must_use]
    pub fn global(name: impl Into<String>) -> Self {
        Self {
            module: None,
            name: name.into(),
        }
    }

    #[must_use]
    pub fn in_module(module: CanonicalModulePath, name: impl Into<String>) -> Self {
        Self {
            module: Some(module),
            name: name.into(),
        }
    }

    pub const fn module(&self) -> Option<&CanonicalModulePath> {
        self.module.as_ref()
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl CharacterDialogueCustomFieldDescriptor {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        id: CharacterDialogueCustomFieldId,
        bindings: impl Into<Box<[CharacterDialogueCustomFieldBinding]>>,
        value_type: TypeKind,
        runtime_nominal_type: Option<RuntimeNominalTypeId>,
        runtime_layout: TypeLayoutHash,
        clearable: bool,
        accepted_views: BTreeSet<ViewId>,
        declaration: SourceSpan,
    ) -> Self {
        Self {
            id,
            bindings: bindings.into(),
            value_type,
            runtime_nominal_type,
            runtime_layout,
            clearable,
            accepted_views,
            declaration,
        }
    }

    pub const fn id(&self) -> &CharacterDialogueCustomFieldId {
        &self.id
    }

    pub const fn bindings(&self) -> &[CharacterDialogueCustomFieldBinding] {
        &self.bindings
    }

    pub const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }

    pub const fn runtime_nominal_type(&self) -> Option<&RuntimeNominalTypeId> {
        self.runtime_nominal_type.as_ref()
    }

    pub const fn runtime_layout(&self) -> TypeLayoutHash {
        self.runtime_layout
    }

    pub const fn clearable(&self) -> bool {
        self.clearable
    }

    pub const fn accepted_views(&self) -> &BTreeSet<ViewId> {
        &self.accepted_views
    }

    pub const fn declaration(&self) -> &SourceSpan {
        &self.declaration
    }
}

impl CharacterDialogueCustomFieldRegistry {
    #[must_use]
    pub fn empty(world: AcceptedNominalWorldStamp) -> Self {
        Self {
            world,
            semantic_digest: empty_registry_digest(),
            by_id: BTreeMap::new(),
            bindings: BTreeMap::new(),
        }
    }

    pub fn try_new(
        world: AcceptedNominalWorldStamp,
        descriptors: impl IntoIterator<Item = CharacterDialogueCustomFieldDescriptor>,
    ) -> Result<Self, CharacterDialogueCustomFieldRegistryError> {
        let mut registry = Self::empty(world);
        for descriptor in descriptors {
            if descriptor.bindings.is_empty() {
                return Err(CharacterDialogueCustomFieldRegistryError::MissingBinding(
                    descriptor.id,
                ));
            }
            let id = descriptor.id.clone();
            if registry.by_id.contains_key(&id) {
                return Err(CharacterDialogueCustomFieldRegistryError::DuplicateId(id));
            }
            for binding in &descriptor.bindings {
                if CallableName::try_new(binding.name()).is_err() {
                    return Err(CharacterDialogueCustomFieldRegistryError::InvalidBinding(
                        binding.name().to_owned(),
                    ));
                }
                if is_reserved(binding.name()) {
                    return Err(CharacterDialogueCustomFieldRegistryError::ReservedBinding(
                        binding.name().to_owned(),
                    ));
                }
                if registry
                    .bindings
                    .insert(binding.clone(), id.clone())
                    .is_some()
                {
                    return Err(CharacterDialogueCustomFieldRegistryError::DuplicateBinding(
                        binding.name().to_owned(),
                    ));
                }
            }
            registry.by_id.insert(id, descriptor);
        }
        registry.semantic_digest = registry_digest(&registry.by_id, &registry.bindings);
        Ok(registry)
    }

    pub const fn world(&self) -> &AcceptedNominalWorldStamp {
        &self.world
    }

    pub const fn semantic_digest(&self) -> &[u8; 32] {
        &self.semantic_digest
    }

    pub fn descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = &CharacterDialogueCustomFieldDescriptor> {
        self.by_id.values()
    }

    pub fn descriptor(
        &self,
        id: &CharacterDialogueCustomFieldId,
    ) -> Option<&CharacterDialogueCustomFieldDescriptor> {
        self.by_id.get(id)
    }

    pub fn resolve(
        &self,
        module: &CanonicalModulePath,
        name: &str,
    ) -> Option<&CharacterDialogueCustomFieldDescriptor> {
        let scoped = CharacterDialogueCustomFieldBinding::in_module(module.clone(), name);
        let global = CharacterDialogueCustomFieldBinding::global(name);
        self.bindings
            .get(&scoped)
            .or_else(|| self.bindings.get(&global))
            .and_then(|id| self.by_id.get(id))
    }

    /// Returns the deterministic source bindings visible from one module.
    ///
    /// A module-scoped binding shadows a global binding with the same source
    /// name. The returned order is the canonical source-name order used by
    /// callable schemas and tooling.
    pub fn visible_bindings(
        &self,
        module: &CanonicalModulePath,
    ) -> Vec<(&str, &CharacterDialogueCustomFieldDescriptor)> {
        let mut visible = BTreeMap::new();
        for (binding, id) in &self.bindings {
            if binding.module().is_none()
                && let Some(descriptor) = self.by_id.get(id)
            {
                visible.insert(binding.name(), descriptor);
            }
        }
        for (binding, id) in &self.bindings {
            if binding.module() == Some(module)
                && let Some(descriptor) = self.by_id.get(id)
            {
                visible.insert(binding.name(), descriptor);
            }
        }
        visible.into_iter().collect()
    }
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "id" | "text_key"
            | "voice"
            | "look"
            | "stage"
            | "portrait"
            | "focus"
            | "cleanup"
            | "view"
            | "source_locale"
            | "hooks"
            | "style"
            | "rich_text"
            | "inline_error"
            | "inline_error_policy"
            | "inline_fallback"
            | "character"
            | "character_id"
            | "content"
    )
}

fn empty_registry_digest() -> [u8; 32] {
    registry_digest(&BTreeMap::new(), &BTreeMap::new())
}

fn registry_digest(
    descriptors: &BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomFieldDescriptor>,
    bindings: &BTreeMap<CharacterDialogueCustomFieldBinding, CharacterDialogueCustomFieldId>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.character-dialogue-custom-fields.v1\0");
    hasher.update(
        &u32::try_from(descriptors.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for (id, descriptor) in descriptors {
        hash_string(&mut hasher, id.as_str());
        hasher.update(descriptor.value_type.semantic_identity_digest().as_bytes());
        match &descriptor.runtime_nominal_type {
            Some(nominal) => {
                hasher.update(&[1]);
                hash_string(&mut hasher, nominal.as_str());
            }
            None => {
                hasher.update(&[0]);
            }
        }
        hasher.update(descriptor.runtime_layout.as_bytes());
        hasher.update(&[u8::from(descriptor.clearable)]);
        hasher.update(
            &u32::try_from(descriptor.accepted_views.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for view in &descriptor.accepted_views {
            hash_string(&mut hasher, view.as_str());
        }
    }
    hasher.update(
        &u32::try_from(bindings.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for (binding, id) in bindings {
        match &binding.module {
            Some(module) => {
                hasher.update(&[1]);
                hasher.update(
                    &u32::try_from(module.segments().len())
                        .unwrap_or(u32::MAX)
                        .to_le_bytes(),
                );
                for segment in module.segments() {
                    hash_string(&mut hasher, segment.as_str());
                }
            }
            None => {
                hasher.update(&[0]);
            }
        }
        hash_string(&mut hasher, binding.name());
        hash_string(&mut hasher, id.as_str());
    }
    *hasher.finalize().as_bytes()
}

fn hash_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
    hasher.update(value.as_bytes());
}
