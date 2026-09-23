//! Complete producer-owned policy schemas; layouts depend on active `RichText`.

use super::super::CharacterDialogueValueError;
use arcweft_core::{
    entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
        RuntimeSchemaLimits, RuntimeTypeSchema,
    },
    pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId, RuntimeVariantIdentity},
};
#[derive(Clone, Copy)]
pub(super) enum DialogueRuntimeVariantOwner {
    Voice,
    InlineFailure,
    InlineFallback,
    FallbackStyle,
}

impl DialogueRuntimeVariantOwner {
    const fn public_id(self) -> &'static str {
        match self {
            Self::Voice => "arcweft.dialogue.CharacterDialogueVoice",
            Self::InlineFailure => "arcweft.dialogue.InlineFailurePolicy",
            Self::InlineFallback => "arcweft.dialogue.InlineFallback",
            Self::FallbackStyle => "arcweft.dialogue.FallbackStylePolicy",
        }
    }

    const fn semantic_digest(self) -> [u8; 32] {
        match self {
            // SHA-256 of the versioned canonical owner labels. The bytes are
            // frozen schema identity, not source or display spellings.
            Self::Voice => [
                0x76, 0x53, 0x13, 0x17, 0x90, 0x11, 0xc8, 0xe7, 0x34, 0x93, 0xbe, 0xbe, 0x4e, 0xc0,
                0x4d, 0x05, 0x6a, 0xd3, 0xd5, 0xcd, 0x6a, 0xbd, 0xd3, 0x94, 0x9b, 0x0f, 0x8a, 0x36,
                0x9e, 0x3c, 0x6a, 0x4d,
            ],
            Self::InlineFailure => [
                0x5c, 0xfa, 0x09, 0xb9, 0xb5, 0x88, 0x19, 0x62, 0xe9, 0xdd, 0xe3, 0x22, 0xfb, 0xe5,
                0x50, 0xa8, 0x7a, 0x2b, 0xcc, 0x6f, 0xe1, 0x98, 0xc0, 0xe4, 0xd8, 0x51, 0x51, 0xb2,
                0x24, 0x8d, 0xed, 0x77,
            ],
            Self::InlineFallback => [
                0xb0, 0x2c, 0xfa, 0x28, 0x38, 0xd6, 0xf8, 0x30, 0x9e, 0x47, 0xad, 0xab, 0x77, 0xf1,
                0x24, 0xea, 0x90, 0x2a, 0xb6, 0xea, 0xa4, 0xa6, 0xf9, 0x88, 0xfa, 0xec, 0x56, 0x58,
                0x28, 0x50, 0x69, 0xc5,
            ],
            Self::FallbackStyle => [
                0x89, 0xa6, 0x0b, 0xba, 0xba, 0x9b, 0x88, 0xe0, 0x84, 0x03, 0x27, 0x37, 0xd8, 0x0e,
                0x27, 0xa3, 0xf4, 0xdd, 0x5a, 0x63, 0xeb, 0x3b, 0xec, 0x51, 0xcc, 0x7d, 0x5b, 0xd9,
                0x09, 0x82, 0xc1, 0xe2,
            ],
        }
    }

    fn schema_identity(self) -> RuntimeNominalSchemaIdentity {
        RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::try_new(self.public_id()).expect("valid fixed policy ID"),
            RuntimeSemanticTypeId::from_bytes(self.semantic_digest()),
        )
    }
}
/// Derived policy headers, with no duplicate executable type catalog.
pub(super) struct DialoguePolicyTypes {
    owners: [RuntimeVariantIdentity; 4],
}

impl DialoguePolicyTypes {
    pub(super) fn try_new(
        rich_text: RuntimeOpaqueTypeOwner,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, CharacterDialogueValueError> {
        use DialogueRuntimeVariantOwner as Owner;
        use RuntimeTypeSchema as Schema;
        let reference = |owner: Owner| Schema::NominalRef(owner.schema_identity());
        let style = Schema::Choice(
            vec![
                Schema::EntityReference,
                Schema::ExactOpaque {
                    owner: rich_text,
                    arguments: Box::new([]),
                },
            ]
            .into(),
        );
        let case = |ordinal, name: &str, payload| {
            RuntimeNominalSchemaCase::new(ordinal, name.to_owned(), payload)
        };
        let definitions = [
            (
                Owner::Voice,
                vec![case(0, "Auto", None), case(1, "Id", Some(Schema::String))],
            ),
            (
                Owner::InlineFailure,
                vec![
                    case(0, "FailLine", None),
                    case(1, "Discard", None),
                    case(2, "Fallback", Some(reference(Owner::InlineFallback))),
                ],
            ),
            (
                Owner::InlineFallback,
                vec![
                    case(
                        0,
                        "Text",
                        Some(Schema::Tuple(
                            vec![Schema::String, reference(Owner::FallbackStyle)].into(),
                        )),
                    ),
                    case(1, "ExprSource", Some(reference(Owner::FallbackStyle))),
                    case(2, "CallSource", Some(reference(Owner::FallbackStyle))),
                    case(3, "ValuePlain", None),
                ],
            ),
            (
                Owner::FallbackStyle,
                vec![
                    case(0, "Plain", None),
                    case(1, "InheritSurrounding", None),
                    case(2, "Apply", Some(Schema::Seq(Box::new(style)))),
                ],
            ),
        ]
        .map(|(owner, cases)| {
            RuntimeNominalSchemaDefinition::new(
                owner.schema_identity(),
                Box::<[Schema]>::default(),
                RuntimeNominalSchemaBody::Variant {
                    cases: cases.into(),
                },
            )
        });
        let graph = RuntimeNominalSchemaGraph::try_new(Vec::from(definitions), limits)?;
        let owners = [
            Owner::Voice,
            Owner::InlineFailure,
            Owner::InlineFallback,
            Owner::FallbackStyle,
        ]
        .into_iter()
        .map(|owner| {
            let identity = owner.schema_identity();
            Ok(RuntimeVariantIdentity::Nominal {
                nominal: identity.nominal().clone(),
                semantic_identity: identity.semantic_identity(),
                layout: graph.try_layout_hash(identity.semantic_identity())?,
            })
        })
        .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?;
        Ok(Self {
            owners: owners.try_into().expect("four complete policy definitions"),
        })
    }

    pub(super) fn identity(&self, owner: DialogueRuntimeVariantOwner) -> &RuntimeVariantIdentity {
        let index = match owner {
            DialogueRuntimeVariantOwner::Voice => 0,
            DialogueRuntimeVariantOwner::InlineFailure => 1,
            DialogueRuntimeVariantOwner::InlineFallback => 2,
            DialogueRuntimeVariantOwner::FallbackStyle => 3,
        };
        &self.owners[index]
    }
}
