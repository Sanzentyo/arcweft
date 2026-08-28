//! Exact registered Character look identity retained by Stage expressions.

use arcweft_character::{
    id::{CharacterId, CharacterLookId},
    manifest::{CharacterManifest, CharacterPartSelection},
};
use arcweft_lang_hir::leaf::HirName;

use crate::types::{CharacterNominalType, SemanticTypeDigest, TypeKind};

const CHARACTER_LOOK_SEMANTIC_DOMAIN: &[u8] = b"arcweft.lang.accepted-character-look.v1\0";

/// Opaque semantic identity of one exact look row from the registered
/// Character manifest generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedCharacterLookSemanticId([u8; 32]);

impl AcceptedCharacterLookSemanticId {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact manifest-backed Stage look selected by a short-variant expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStageLook {
    character_nominal: SemanticTypeDigest,
    character: CharacterId,
    look_id: CharacterLookId,
    look: AcceptedCharacterLookSemanticId,
    diagnostic_name: HirName,
}

impl CheckedStageLook {
    pub(in crate::final_analysis) fn try_from_registered_manifest(
        nominal: &CharacterNominalType,
        manifest: &CharacterManifest,
        diagnostic_name: HirName,
    ) -> Option<Self> {
        let CharacterNominalType::Look { character } = nominal else {
            return None;
        };
        if manifest.character() != character || manifest.validate().is_err() {
            return None;
        }
        let look_id = CharacterLookId::try_new(diagnostic_name.as_str()).ok()?;
        let look = manifest.look(&look_id)?;
        let semantic_look =
            accepted_character_look_semantic_id(character, look.id(), look.selections())?;
        Some(Self {
            character_nominal: TypeKind::CharacterNominal(nominal.clone())
                .semantic_identity_digest(),
            character: character.clone(),
            look_id,
            look: semantic_look,
            diagnostic_name,
        })
    }

    /// Returns the semantic identity of the exact Character nominal type.
    pub const fn character_nominal(&self) -> SemanticTypeDigest {
        self.character_nominal
    }

    /// Returns the exact registered Character identity.
    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    pub(crate) const fn look(&self) -> AcceptedCharacterLookSemanticId {
        self.look
    }

    /// Returns the exact manifest-owned runtime look identity. Downstream
    /// projection consumes this typed id and never reconstructs it from the
    /// diagnostic spelling.
    pub const fn look_id(&self) -> &CharacterLookId {
        &self.look_id
    }

    /// Returns the authored spelling retained only for diagnostics.
    pub const fn diagnostic_name(&self) -> &HirName {
        &self.diagnostic_name
    }

    pub(crate) fn matches_type(&self, ty: &TypeKind) -> bool {
        matches!(
            ty,
            TypeKind::CharacterNominal(CharacterNominalType::Look { character })
                if character == &self.character
        ) && ty.semantic_identity_digest() == self.character_nominal
    }
}

fn accepted_character_look_semantic_id(
    character: &CharacterId,
    look: &CharacterLookId,
    selections: &[CharacterPartSelection],
) -> Option<AcceptedCharacterLookSemanticId> {
    let mut selections = selections.iter().collect::<Vec<_>>();
    selections.sort_by_key(|selection| selection.part());

    let mut hasher = blake3::Hasher::new();
    hasher.update(CHARACTER_LOOK_SEMANTIC_DOMAIN);
    update_bytes(&mut hasher, character.as_str())?;
    update_bytes(&mut hasher, look.as_str())?;
    hasher.update(&u64::try_from(selections.len()).ok()?.to_le_bytes());
    for selection in selections {
        update_bytes(&mut hasher, selection.part().as_str())?;
        update_bytes(&mut hasher, selection.variant().as_str())?;
    }
    Some(AcceptedCharacterLookSemanticId(hasher.finalize().into()))
}

fn update_bytes(hasher: &mut blake3::Hasher, value: &str) -> Option<()> {
    hasher.update(&u64::try_from(value.len()).ok()?.to_le_bytes());
    hasher.update(value.as_bytes());
    Some(())
}

#[cfg(test)]
#[path = "stage_look_tests.rs"]
mod tests;
