//! Checked Character presentation evidence carried by runtime plans.

use crate::CharacterDialogueContractIdentity;
use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterPresentationCatalogGeneration, CharacterPresentationLocalePolicyDigest,
        CharacterPresentationSemanticDigest,
    },
};
use arcweft_core::entry::TypeLayoutHash;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Typed evidence identifying the Character selected by dialogue execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterPresentationTargetEvidence {
    Exact(CharacterId),
    RuntimeCharacterDialogue {
        contract: CharacterDialogueContractIdentity,
        layout: TypeLayoutHash,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CharacterPresentationTargetEvidenceWire {
    Exact {
        character: CharacterId,
    },
    RuntimeCharacterDialogue {
        contract: CharacterDialogueContractIdentity,
        layout: TypeLayoutHash,
    },
}

impl Serialize for CharacterPresentationTargetEvidence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Exact(character) => CharacterPresentationTargetEvidenceWire::Exact {
                character: character.clone(),
            },
            Self::RuntimeCharacterDialogue { contract, layout } => {
                CharacterPresentationTargetEvidenceWire::RuntimeCharacterDialogue {
                    contract: *contract,
                    layout: *layout,
                }
            }
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CharacterPresentationTargetEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match CharacterPresentationTargetEvidenceWire::deserialize(deserializer)? {
                CharacterPresentationTargetEvidenceWire::Exact { character } => {
                    Self::Exact(character)
                }
                CharacterPresentationTargetEvidenceWire::RuntimeCharacterDialogue {
                    contract,
                    layout,
                } => Self::RuntimeCharacterDialogue { contract, layout },
            },
        )
    }
}

/// Runtime-plan Character presentation target bound to accepted catalog identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckedCharacterPresentationPlan {
    target: CharacterPresentationTargetEvidence,
    semantic_digest: CharacterPresentationSemanticDigest,
    locale_policy_digest: CharacterPresentationLocalePolicyDigest,
}

/// Failure to verify a checked Character presentation plan at an artifact or
/// runtime boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedCharacterPresentationPlanError {
    #[error("Character presentation semantic digest is stale")]
    StaleSemanticDigest {
        expected: CharacterPresentationSemanticDigest,
        actual: CharacterPresentationSemanticDigest,
    },
    #[error("Character presentation locale-policy digest is stale")]
    StaleLocalePolicyDigest {
        expected: CharacterPresentationLocalePolicyDigest,
        actual: CharacterPresentationLocalePolicyDigest,
    },
    #[error("runtime CharacterDialogue contract does not match the checked target")]
    CharacterContractMismatch,
    #[error("runtime CharacterDialogue layout does not match the checked target")]
    CharacterLayoutMismatch,
    #[error("Character `{character}` is absent from the accepted presentation catalog")]
    UnknownCharacter { character: CharacterId },
}

impl CheckedCharacterPresentationPlan {
    /// Binds typed target evidence to one already accepted catalog generation.
    ///
    /// The nominal target and generation have no unchecked textual or numeric
    /// state. Artifact decoding and runtime membership/contract verification
    /// occur before this final constructor is called.
    pub fn try_new(
        target: CharacterPresentationTargetEvidence,
        generation: CharacterPresentationCatalogGeneration,
    ) -> Result<Self, CheckedCharacterPresentationPlanError> {
        Ok(Self {
            target,
            semantic_digest: generation.semantic_digest(),
            locale_policy_digest: generation.locale_policy_digest(),
        })
    }

    #[must_use]
    pub const fn target(&self) -> &CharacterPresentationTargetEvidence {
        &self.target
    }

    #[must_use]
    pub const fn semantic_digest(&self) -> CharacterPresentationSemanticDigest {
        self.semantic_digest
    }

    #[must_use]
    pub const fn locale_policy_digest(&self) -> CharacterPresentationLocalePolicyDigest {
        self.locale_policy_digest
    }
}

#[cfg(test)]
mod tests {
    use super::{CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan};
    use crate::CharacterDialogueContractIdentity;
    use arcweft_character::{
        id::CharacterId,
        presentation_name::{
            CharacterPresentationCatalogGeneration, CharacterPresentationCatalogRevision,
            CharacterPresentationLocalePolicyDigest, CharacterPresentationSemanticDigest,
        },
    };
    use arcweft_core::entry::{RuntimeValueDigest, TypeLayoutHash};

    #[test]
    fn exact_target_binds_both_catalog_digests() {
        let semantic = CharacterPresentationSemanticDigest::from_bytes([1; 32]);
        let policy = CharacterPresentationLocalePolicyDigest::from_bytes([2; 32]);
        let generation = CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            semantic,
            policy,
        );
        let target = CharacterPresentationTargetEvidence::Exact(
            CharacterId::try_new("character.alice").unwrap(),
        );

        let plan = CheckedCharacterPresentationPlan::try_new(target.clone(), generation).unwrap();
        assert_eq!(plan.target(), &target);
        assert_eq!(plan.semantic_digest(), semantic);
        assert_eq!(plan.locale_policy_digest(), policy);
    }

    #[test]
    fn runtime_target_retains_nominal_contract_and_layout_only() {
        let contract = CharacterDialogueContractIdentity::new(
            RuntimeValueDigest::from_bytes([3; 32]),
            RuntimeValueDigest::from_bytes([4; 32]),
            RuntimeValueDigest::from_bytes([5; 32]),
            RuntimeValueDigest::from_bytes([6; 32]),
        );
        let layout = TypeLayoutHash::from_bytes([7; 32]);
        let target =
            CharacterPresentationTargetEvidence::RuntimeCharacterDialogue { contract, layout };
        let generation = CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            CharacterPresentationSemanticDigest::from_bytes([8; 32]),
            CharacterPresentationLocalePolicyDigest::from_bytes([9; 32]),
        );

        let plan = CheckedCharacterPresentationPlan::try_new(target.clone(), generation).unwrap();
        assert_eq!(plan.target(), &target);
    }
}
