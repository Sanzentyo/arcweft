//! Complete producer-owned type graph for CharacterDialogue policies.

use super::super::CharacterDialogueValueError;
use arcweft_core::{
    entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
        RuntimeSchemaLimits, RuntimeTypeSchema,
    },
    pattern::{
        RuntimeCheckedType, RuntimeCheckedVariantCase, RuntimeOpaqueTypeOwner,
        RuntimeSemanticTypeId, RuntimeVariantIdentity,
    },
    plan::{
        RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
        RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const POLICY_VARIANT_COUNT: usize = 4;

/// One of the closed nominal policy owners emitted by the CharacterDialogue
/// producer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialoguePolicyVariantOwner {
    /// CharacterDialogue's stored voice policy.
    Voice,
    /// The action taken when inline text evaluation fails.
    InlineFailure,
    /// The source used to recover from inline text evaluation failure.
    InlineFallback,
    /// The style applied to fallback text.
    FallbackStyle,
}

impl CharacterDialoguePolicyVariantOwner {
    /// Language-visible policy owner order. Voice keeps its existing source
    /// environment spelling and is therefore not part of this alias set.
    #[must_use]
    pub fn language_policy_owners() -> impl ExactSizeIterator<Item = Self> {
        [
            Self::InlineFailure,
            Self::InlineFallback,
            Self::FallbackStyle,
        ]
        .into_iter()
    }

    /// Source-level type name, where this policy owner is addressable by the
    /// language. Runtime schema identity remains owned by this graph.
    pub const fn language_type_name(self) -> &'static str {
        match self {
            Self::Voice => "DialogueVoice",
            Self::InlineFailure => "InlineFailure",
            Self::InlineFallback => "InlineFallback",
            Self::FallbackStyle => "FallbackStyle",
        }
    }

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
            // SHA-256 of the versioned canonical owner labels. These bytes are
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

    const fn index(self) -> usize {
        match self {
            Self::Voice => 0,
            Self::InlineFailure => 1,
            Self::InlineFallback => 2,
            Self::FallbackStyle => 3,
        }
    }

    fn schema_identity(self) -> RuntimeNominalSchemaIdentity {
        RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::try_new(self.public_id()).expect("valid fixed policy ID"),
            RuntimeSemanticTypeId::from_bytes(self.semantic_digest()),
        )
    }

    const fn error_field(self) -> &'static str {
        match self {
            Self::Voice => "voice_policy",
            Self::InlineFailure => "inline_failure_policy",
            Self::InlineFallback => "inline_fallback_policy",
            Self::FallbackStyle => "fallback_style_policy",
        }
    }
}

/// The complete dialogue-owned nominal proof and its runtime-plan projection.
///
/// The same case specification emits the persistent nominal graph, type seeds,
/// variant-domain seeds, checked owner identities, and the runtime validation
/// contract. Runtime-plan can admit these batches directly without rebuilding
/// policy schemas from case names or display values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialoguePolicyTypeGraph {
    rich_text_owner: RuntimeOpaqueTypeOwner,
    schema_graph: Arc<RuntimeNominalSchemaGraph>,
    type_seeds: Box<[RuntimePlanTypeSeed]>,
    variant_domain_seeds: Box<[RuntimeVariantDomainSeed]>,
    identities: [RuntimeVariantIdentity; POLICY_VARIANT_COUNT],
    checked_variants: [RuntimeCheckedType; POLICY_VARIANT_COUNT],
}

impl CharacterDialoguePolicyTypeGraph {
    /// Builds the complete bounded graph for the exact active RichText owner.
    pub fn try_new(
        rich_text: RuntimeOpaqueTypeOwner,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, CharacterDialogueValueError> {
        let definitions = POLICY_VARIANTS
            .iter()
            .map(|variant| {
                RuntimeNominalSchemaDefinition::new(
                    variant.owner.schema_identity(),
                    Box::<[RuntimeTypeSchema]>::default(),
                    RuntimeNominalSchemaBody::Variant {
                        cases: variant
                            .cases
                            .iter()
                            .map(|case| {
                                RuntimeNominalSchemaCase::new(
                                    case.ordinal,
                                    case.name.to_owned(),
                                    case.payload.map(|payload| schema_for(payload, &rich_text)),
                                )
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    },
                )
            })
            .collect::<Vec<_>>();
        let schema_graph = Arc::new(RuntimeNominalSchemaGraph::try_new(definitions, limits)?);

        let identities = POLICY_VARIANTS
            .iter()
            .map(|variant| {
                let schema_identity = variant.owner.schema_identity();
                Ok(RuntimeVariantIdentity::Nominal {
                    nominal: schema_identity.nominal().clone(),
                    semantic_identity: schema_identity.semantic_identity(),
                    layout: schema_graph.try_layout_hash(schema_identity.semantic_identity())?,
                })
            })
            .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?
            .try_into()
            .expect("four complete policy definitions");

        let mut types = BTreeMap::new();
        for variant in &POLICY_VARIANTS {
            let identity = variant.owner.schema_identity();
            append_type_graph(
                RuntimeCheckedType::Nominal {
                    nominal: identity.nominal().clone(),
                    semantic_identity: identity.semantic_identity(),
                    layout: schema_graph.try_layout_hash(identity.semantic_identity())?,
                    arguments: Vec::new(),
                },
                &mut types,
            )?;
            for case in variant.cases {
                if let Some(payload) = case.payload {
                    append_type_graph(
                        checked_type_for_seed(payload, &rich_text, &schema_graph)?,
                        &mut types,
                    )?;
                }
            }
        }
        let type_seeds = types.into_values().collect::<Vec<_>>().into_boxed_slice();

        let variant_domain_seeds = POLICY_VARIANTS
            .iter()
            .map(|variant| {
                let identity = variant.owner.schema_identity();
                Ok(RuntimeVariantDomainSeed::new(
                    identity.semantic_identity(),
                    identity.nominal().clone(),
                    schema_graph.try_layout_hash(identity.semantic_identity())?,
                    variant
                        .cases
                        .iter()
                        .map(|case| {
                            let payload = case.payload.map(|payload| {
                                checked_type_for_seed(payload, &rich_text, &schema_graph)
                                    .map(|checked| seed_identity(&checked))
                            });
                            Ok(RuntimeVariantCaseSeed::new(case.name, payload.transpose()?))
                        })
                        .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?,
                ))
            })
            .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?
            .into_boxed_slice();

        let checked_variants = POLICY_VARIANTS
            .iter()
            .map(|variant| {
                checked_variant_for_program(
                    variant.owner,
                    &identities,
                    &rich_text,
                    &mut BTreeSet::new(),
                )
            })
            .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?
            .try_into()
            .expect("four complete policy definitions");

        Ok(Self {
            rich_text_owner: rich_text,
            schema_graph,
            type_seeds,
            variant_domain_seeds,
            identities,
            checked_variants,
        })
    }

    /// Returns the retained source schema graph for merging into a plan.
    #[must_use]
    pub const fn schema_graph(&self) -> &Arc<RuntimeNominalSchemaGraph> {
        &self.schema_graph
    }

    /// Exact accepted RichText role used to build this graph.
    #[must_use]
    pub const fn rich_text_owner(&self) -> &RuntimeOpaqueTypeOwner {
        &self.rich_text_owner
    }

    /// Returns the source-ordered case schema retained by this graph.
    #[must_use]
    pub fn cases(
        &self,
        owner: CharacterDialoguePolicyVariantOwner,
    ) -> &'static [CharacterDialoguePolicyCaseSpec] {
        variant_spec(owner).cases
    }

    /// Projects a case payload through the graph's checked seed vocabulary.
    /// Nested policy owners stay nominal, so structural carrier identities
    /// agree with the type seeds emitted by this same graph.
    pub fn checked_payload_type(
        &self,
        payload: CharacterDialoguePolicyTypeSchema,
    ) -> Result<RuntimeCheckedType, CharacterDialogueValueError> {
        checked_type_for_seed(payload, &self.rich_text_owner, &self.schema_graph)
    }

    /// Resolves the language-visible policy type alias to its exact graph
    /// owner. This is the runtime identity bridge for the source alias.
    #[must_use]
    pub fn owner_for_language_type(name: &str) -> Option<CharacterDialoguePolicyVariantOwner> {
        CharacterDialoguePolicyVariantOwner::language_policy_owners()
            .find(|owner| owner.language_type_name() == name)
    }

    /// Returns the exact semantic identity retained for one policy owner.
    #[must_use]
    pub fn semantic_identity(
        &self,
        owner: CharacterDialoguePolicyVariantOwner,
    ) -> RuntimeSemanticTypeId {
        let RuntimeVariantIdentity::Nominal {
            semantic_identity, ..
        } = self.identity(owner)
        else {
            unreachable!("CharacterDialogue policy owners are nominal variants")
        };
        *semantic_identity
    }

    /// Complete checked type for one graph-owned policy variant.
    #[must_use]
    pub fn checked_type(&self, owner: CharacterDialoguePolicyVariantOwner) -> &RuntimeCheckedType {
        &self.checked_variants[owner.index()]
    }

    /// Resolves an exact graph type identity back to its policy owner.
    #[must_use]
    pub fn owner_for_semantic_identity(
        &self,
        identity: RuntimeSemanticTypeId,
    ) -> Option<CharacterDialoguePolicyVariantOwner> {
        [
            CharacterDialoguePolicyVariantOwner::Voice,
            CharacterDialoguePolicyVariantOwner::InlineFailure,
            CharacterDialoguePolicyVariantOwner::InlineFallback,
            CharacterDialoguePolicyVariantOwner::FallbackStyle,
        ]
        .into_iter()
        .find(|owner| self.semantic_identity(*owner) == identity)
    }

    /// Clones the complete canonical type-seed batch for plan admission.
    #[must_use]
    pub fn type_seeds(&self) -> Box<[RuntimePlanTypeSeed]> {
        self.type_seeds.clone()
    }

    /// Clones the four nominal variant-domain seeds for plan admission.
    #[must_use]
    pub fn variant_domain_seeds(&self) -> Box<[RuntimeVariantDomainSeed]> {
        self.variant_domain_seeds.clone()
    }

    /// Returns the exact identity retained by this graph and used by the
    /// CharacterDialogue value encoder.
    #[must_use]
    pub fn identity(&self, owner: CharacterDialoguePolicyVariantOwner) -> &RuntimeVariantIdentity {
        &self.identities[owner.index()]
    }

    pub(super) fn case_spec(
        case: CharacterDialoguePolicyCase,
    ) -> (CharacterDialoguePolicyVariantOwner, u32, &'static str) {
        let owner = case.owner();
        let spec = variant_spec(owner)
            .cases
            .iter()
            .find(|spec| spec.case == case)
            .expect("policy case has a fixed source specification");
        (owner, spec.ordinal, spec.name)
    }

    pub(super) fn case_for_value(
        owner: CharacterDialoguePolicyVariantOwner,
        ordinal: u32,
        name: &str,
    ) -> Option<CharacterDialoguePolicyCase> {
        variant_spec(owner)
            .cases
            .iter()
            .find(|case| case.ordinal == ordinal && case.name == name)
            .map(|case| case.case)
    }

    pub(super) fn validate_program_types(
        &self,
        program: arcweft_core::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<(), CharacterDialogueValueError> {
        for variant in &POLICY_VARIANTS {
            let actual =
                program.checked_type(variant.owner.schema_identity().semantic_identity())?;
            if actual != self.checked_variants[variant.owner.index()] {
                return Err(CharacterDialogueValueError::Field {
                    field: variant.owner.error_field(),
                    reason: "active program policy owner, layout, or cases differ from the producer schema"
                        .to_owned(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CharacterDialoguePolicyCase {
    VoiceAuto,
    VoiceId,
    InlineFailureFailLine,
    InlineFailureDiscard,
    InlineFailureFallback,
    InlineFallbackText,
    InlineFallbackExprSource,
    InlineFallbackCallSource,
    InlineFallbackValuePlain,
    FallbackStylePlain,
    FallbackStyleInheritSurrounding,
    FallbackStyleApply,
}

impl CharacterDialoguePolicyCase {
    const fn owner(self) -> CharacterDialoguePolicyVariantOwner {
        match self {
            Self::VoiceAuto | Self::VoiceId => CharacterDialoguePolicyVariantOwner::Voice,
            Self::InlineFailureFailLine
            | Self::InlineFailureDiscard
            | Self::InlineFailureFallback => CharacterDialoguePolicyVariantOwner::InlineFailure,
            Self::InlineFallbackText
            | Self::InlineFallbackExprSource
            | Self::InlineFallbackCallSource
            | Self::InlineFallbackValuePlain => CharacterDialoguePolicyVariantOwner::InlineFallback,
            Self::FallbackStylePlain
            | Self::FallbackStyleInheritSurrounding
            | Self::FallbackStyleApply => CharacterDialoguePolicyVariantOwner::FallbackStyle,
        }
    }
}

/// Recursive payload shape described by the CharacterDialogue policy graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterDialoguePolicyTypeSchema {
    String,
    EntityReference,
    RichText,
    Nominal(CharacterDialoguePolicyVariantOwner),
    Sequence(&'static CharacterDialoguePolicyTypeSchema),
    Tuple(&'static [CharacterDialoguePolicyTypeSchema]),
    Choice(&'static [CharacterDialoguePolicyTypeSchema]),
}

/// One source-ordered case and optional payload from the canonical graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterDialoguePolicyCaseSpec {
    case: CharacterDialoguePolicyCase,
    ordinal: u32,
    name: &'static str,
    payload: Option<CharacterDialoguePolicyTypeSchema>,
}

impl CharacterDialoguePolicyCaseSpec {
    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Source spelling accepted by the language for this graph case.
    #[must_use]
    pub const fn language_name(self) -> &'static str {
        match self.case {
            CharacterDialoguePolicyCase::VoiceAuto => "auto",
            CharacterDialoguePolicyCase::VoiceId => "id",
            CharacterDialoguePolicyCase::InlineFailureFailLine => "fail",
            CharacterDialoguePolicyCase::InlineFailureDiscard => "discard",
            CharacterDialoguePolicyCase::InlineFailureFallback => "fallback",
            CharacterDialoguePolicyCase::InlineFallbackText => "text",
            CharacterDialoguePolicyCase::InlineFallbackExprSource => "expr_source",
            CharacterDialoguePolicyCase::InlineFallbackCallSource => "call_source",
            CharacterDialoguePolicyCase::InlineFallbackValuePlain => "value_plain",
            CharacterDialoguePolicyCase::FallbackStylePlain => "plain",
            CharacterDialoguePolicyCase::FallbackStyleInheritSurrounding => "inherit_surrounding",
            CharacterDialoguePolicyCase::FallbackStyleApply => "apply",
        }
    }

    #[must_use]
    pub const fn payload(self) -> Option<CharacterDialoguePolicyTypeSchema> {
        self.payload
    }
}

struct PolicyVariantSpec {
    owner: CharacterDialoguePolicyVariantOwner,
    cases: &'static [CharacterDialoguePolicyCaseSpec],
}

static INLINE_TEXT_TUPLE_ITEMS: [CharacterDialoguePolicyTypeSchema; 2] = [
    CharacterDialoguePolicyTypeSchema::String,
    CharacterDialoguePolicyTypeSchema::Nominal(CharacterDialoguePolicyVariantOwner::FallbackStyle),
];
static STYLE_CHOICE_ITEMS: [CharacterDialoguePolicyTypeSchema; 2] = [
    CharacterDialoguePolicyTypeSchema::EntityReference,
    CharacterDialoguePolicyTypeSchema::RichText,
];
static STYLE_CHOICE: CharacterDialoguePolicyTypeSchema =
    CharacterDialoguePolicyTypeSchema::Choice(&STYLE_CHOICE_ITEMS);
static INLINE_TEXT_PAYLOAD: CharacterDialoguePolicyTypeSchema =
    CharacterDialoguePolicyTypeSchema::Tuple(&INLINE_TEXT_TUPLE_ITEMS);
static FALLBACK_STYLE_SEQUENCE: CharacterDialoguePolicyTypeSchema =
    CharacterDialoguePolicyTypeSchema::Sequence(&STYLE_CHOICE);

static VOICE_CASES: [CharacterDialoguePolicyCaseSpec; 2] = [
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::VoiceAuto,
        ordinal: 0,
        name: "Auto",
        payload: None,
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::VoiceId,
        ordinal: 1,
        name: "Id",
        payload: Some(CharacterDialoguePolicyTypeSchema::String),
    },
];
static INLINE_FAILURE_CASES: [CharacterDialoguePolicyCaseSpec; 3] = [
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::InlineFailureFailLine,
        ordinal: 0,
        name: "FailLine",
        payload: None,
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::InlineFailureDiscard,
        ordinal: 1,
        name: "Discard",
        payload: None,
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::InlineFailureFallback,
        ordinal: 2,
        name: "Fallback",
        payload: Some(CharacterDialoguePolicyTypeSchema::Nominal(
            CharacterDialoguePolicyVariantOwner::InlineFallback,
        )),
    },
];
static INLINE_FALLBACK_CASES: [CharacterDialoguePolicyCaseSpec; 4] = [
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::InlineFallbackText,
        ordinal: 0,
        name: "Text",
        payload: Some(INLINE_TEXT_PAYLOAD),
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::InlineFallbackExprSource,
        ordinal: 1,
        name: "ExprSource",
        payload: Some(CharacterDialoguePolicyTypeSchema::Nominal(
            CharacterDialoguePolicyVariantOwner::FallbackStyle,
        )),
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::InlineFallbackCallSource,
        ordinal: 2,
        name: "CallSource",
        payload: Some(CharacterDialoguePolicyTypeSchema::Nominal(
            CharacterDialoguePolicyVariantOwner::FallbackStyle,
        )),
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::InlineFallbackValuePlain,
        ordinal: 3,
        name: "ValuePlain",
        payload: None,
    },
];
static FALLBACK_STYLE_CASES: [CharacterDialoguePolicyCaseSpec; 3] = [
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::FallbackStylePlain,
        ordinal: 0,
        name: "Plain",
        payload: None,
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::FallbackStyleInheritSurrounding,
        ordinal: 1,
        name: "InheritSurrounding",
        payload: None,
    },
    CharacterDialoguePolicyCaseSpec {
        case: CharacterDialoguePolicyCase::FallbackStyleApply,
        ordinal: 2,
        name: "Apply",
        payload: Some(FALLBACK_STYLE_SEQUENCE),
    },
];

static POLICY_VARIANTS: [PolicyVariantSpec; POLICY_VARIANT_COUNT] = [
    PolicyVariantSpec {
        owner: CharacterDialoguePolicyVariantOwner::Voice,
        cases: &VOICE_CASES,
    },
    PolicyVariantSpec {
        owner: CharacterDialoguePolicyVariantOwner::InlineFailure,
        cases: &INLINE_FAILURE_CASES,
    },
    PolicyVariantSpec {
        owner: CharacterDialoguePolicyVariantOwner::InlineFallback,
        cases: &INLINE_FALLBACK_CASES,
    },
    PolicyVariantSpec {
        owner: CharacterDialoguePolicyVariantOwner::FallbackStyle,
        cases: &FALLBACK_STYLE_CASES,
    },
];

fn schema_for(
    schema_type: CharacterDialoguePolicyTypeSchema,
    rich_text: &RuntimeOpaqueTypeOwner,
) -> RuntimeTypeSchema {
    match schema_type {
        CharacterDialoguePolicyTypeSchema::String => RuntimeTypeSchema::String,
        CharacterDialoguePolicyTypeSchema::EntityReference => RuntimeTypeSchema::EntityReference,
        CharacterDialoguePolicyTypeSchema::RichText => RuntimeTypeSchema::ExactOpaque {
            owner: rich_text.clone(),
            arguments: Box::new([]),
        },
        CharacterDialoguePolicyTypeSchema::Nominal(owner) => {
            RuntimeTypeSchema::NominalRef(owner.schema_identity())
        }
        CharacterDialoguePolicyTypeSchema::Sequence(item) => {
            RuntimeTypeSchema::Seq(Box::new(schema_for(*item, rich_text)))
        }
        CharacterDialoguePolicyTypeSchema::Tuple(items) => RuntimeTypeSchema::Tuple(
            items
                .iter()
                .copied()
                .map(|item| schema_for(item, rich_text))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        CharacterDialoguePolicyTypeSchema::Choice(items) => RuntimeTypeSchema::Choice(
            items
                .iter()
                .copied()
                .map(|item| schema_for(item, rich_text))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    }
}

fn checked_type_for_seed(
    schema_type: CharacterDialoguePolicyTypeSchema,
    rich_text: &RuntimeOpaqueTypeOwner,
    graph: &RuntimeNominalSchemaGraph,
) -> Result<RuntimeCheckedType, CharacterDialogueValueError> {
    Ok(match schema_type {
        CharacterDialoguePolicyTypeSchema::String => RuntimeCheckedType::String,
        CharacterDialoguePolicyTypeSchema::EntityReference => RuntimeCheckedType::EntityReference,
        CharacterDialoguePolicyTypeSchema::RichText => RuntimeCheckedType::Opaque {
            owner: rich_text.clone(),
        },
        CharacterDialoguePolicyTypeSchema::Nominal(owner) => {
            let identity = owner.schema_identity();
            RuntimeCheckedType::Nominal {
                nominal: identity.nominal().clone(),
                semantic_identity: identity.semantic_identity(),
                layout: graph.try_layout_hash(identity.semantic_identity())?,
                arguments: Vec::new(),
            }
        }
        CharacterDialoguePolicyTypeSchema::Sequence(item) => {
            RuntimeCheckedType::Sequence(Box::new(checked_type_for_seed(*item, rich_text, graph)?))
        }
        CharacterDialoguePolicyTypeSchema::Tuple(items) => RuntimeCheckedType::Tuple(
            items
                .iter()
                .copied()
                .map(|item| checked_type_for_seed(item, rich_text, graph))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CharacterDialoguePolicyTypeSchema::Choice(items) => RuntimeCheckedType::Choice(
            items
                .iter()
                .copied()
                .map(|item| checked_type_for_seed(item, rich_text, graph))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    })
}

fn checked_variant_for_program(
    owner: CharacterDialoguePolicyVariantOwner,
    identities: &[RuntimeVariantIdentity; POLICY_VARIANT_COUNT],
    rich_text: &RuntimeOpaqueTypeOwner,
    visiting: &mut BTreeSet<CharacterDialoguePolicyVariantOwner>,
) -> Result<RuntimeCheckedType, CharacterDialogueValueError> {
    if !visiting.insert(owner) {
        return Err(policy_error(
            "policy case payload graph contains a recursive nominal cycle",
        ));
    }
    let variant = variant_spec(owner);
    let cases = variant
        .cases
        .iter()
        .map(|case| {
            let payload = case
                .payload
                .map(|payload| {
                    checked_type_for_program(payload, identities, rich_text, visiting).map(Box::new)
                })
                .transpose()?;
            Ok(RuntimeCheckedVariantCase {
                name: case.name.to_owned(),
                payload,
            })
        })
        .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?;
    visiting.remove(&owner);
    Ok(RuntimeCheckedType::Variant {
        owner: identities[owner.index()].clone(),
        arguments: Vec::new(),
        cases,
    })
}

fn checked_type_for_program(
    schema_type: CharacterDialoguePolicyTypeSchema,
    identities: &[RuntimeVariantIdentity; POLICY_VARIANT_COUNT],
    rich_text: &RuntimeOpaqueTypeOwner,
    visiting: &mut BTreeSet<CharacterDialoguePolicyVariantOwner>,
) -> Result<RuntimeCheckedType, CharacterDialogueValueError> {
    Ok(match schema_type {
        CharacterDialoguePolicyTypeSchema::String => RuntimeCheckedType::String,
        CharacterDialoguePolicyTypeSchema::EntityReference => RuntimeCheckedType::EntityReference,
        CharacterDialoguePolicyTypeSchema::RichText => RuntimeCheckedType::Opaque {
            owner: rich_text.clone(),
        },
        CharacterDialoguePolicyTypeSchema::Nominal(owner) => {
            return checked_variant_for_program(owner, identities, rich_text, visiting);
        }
        CharacterDialoguePolicyTypeSchema::Sequence(item) => {
            RuntimeCheckedType::Sequence(Box::new(checked_type_for_program(
                *item, identities, rich_text, visiting,
            )?))
        }
        CharacterDialoguePolicyTypeSchema::Tuple(items) => RuntimeCheckedType::Tuple(
            items
                .iter()
                .copied()
                .map(|item| checked_type_for_program(item, identities, rich_text, visiting))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CharacterDialoguePolicyTypeSchema::Choice(items) => RuntimeCheckedType::Choice(
            items
                .iter()
                .copied()
                .map(|item| checked_type_for_program(item, identities, rich_text, visiting))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    })
}

fn append_type_graph(
    root: RuntimeCheckedType,
    seeds: &mut BTreeMap<RuntimeSemanticTypeId, RuntimePlanTypeSeed>,
) -> Result<(), CharacterDialogueValueError> {
    let mut pending = vec![root];
    while let Some(checked) = pending.pop() {
        let semantic_identity = seed_identity(&checked);
        let seed = RuntimePlanTypeSeed::new(semantic_identity, plan_projection(&checked)?);
        if let Some(previous) = seeds.get(&semantic_identity) {
            if previous != &seed {
                return Err(policy_error(
                    "two policy payload shapes share a conflicting semantic identity",
                ));
            }
        } else {
            seeds.insert(semantic_identity, seed);
        }
        pending.extend(seed_children(&checked));
    }
    Ok(())
}

fn seed_children(checked: &RuntimeCheckedType) -> Vec<RuntimeCheckedType> {
    match checked {
        RuntimeCheckedType::Sequence(item) => vec![(**item).clone()],
        RuntimeCheckedType::Tuple(items) | RuntimeCheckedType::Choice(items) => items.clone(),
        RuntimeCheckedType::Nominal { arguments, .. } => arguments.clone(),
        _ => Vec::new(),
    }
}

/// Preserves source-issued semantic identities at leaf owners; only structural
/// carriers derive their identity from the checked-type transcript.
fn seed_identity(checked: &RuntimeCheckedType) -> RuntimeSemanticTypeId {
    match checked {
        RuntimeCheckedType::Nominal {
            semantic_identity, ..
        } => *semantic_identity,
        RuntimeCheckedType::Opaque { owner } => owner.semantic_identity(),
        _ => checked.semantic_identity_digest(),
    }
}

fn plan_projection(
    checked: &RuntimeCheckedType,
) -> Result<RuntimePlanTypeProjection<RuntimeSemanticTypeId>, CharacterDialogueValueError> {
    use RuntimePlanTypeProjection as Projection;
    Ok(match checked {
        RuntimeCheckedType::String => Projection::String,
        RuntimeCheckedType::EntityReference => Projection::EntityReference,
        RuntimeCheckedType::Sequence(item) => Projection::Sequence {
            kind: RuntimePlanSequenceKind::Seq,
            item: seed_identity(item),
        },
        RuntimeCheckedType::Tuple(items) => Projection::Tuple(
            items
                .iter()
                .map(seed_identity)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        RuntimeCheckedType::Choice(items) => Projection::Choice(
            items
                .iter()
                .map(seed_identity)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        RuntimeCheckedType::Nominal {
            nominal,
            layout,
            arguments,
            ..
        } => Projection::Nominal {
            nominal: nominal.clone(),
            layout: *layout,
            arguments: arguments
                .iter()
                .map(seed_identity)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
        RuntimeCheckedType::Opaque { owner } => Projection::Opaque {
            producer: owner.producer().clone(),
            admission: owner.admission(),
            value_class: owner.value_class(),
            persistence: owner.persistence(),
            arguments: Box::new([]),
        },
        _ => {
            return Err(policy_error(
                "policy case specification contains an unsupported plan type",
            ));
        }
    })
}

fn variant_spec(owner: CharacterDialoguePolicyVariantOwner) -> &'static PolicyVariantSpec {
    POLICY_VARIANTS
        .get(owner.index())
        .expect("policy owner has a fixed case specification")
}

fn policy_error(reason: &'static str) -> CharacterDialogueValueError {
    CharacterDialogueValueError::Field {
        field: "policy_type_graph",
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::plan::RuntimePlanTypeProjection as Projection;

    fn rich_text_owner() -> RuntimeOpaqueTypeOwner {
        RuntimeOpaqueTypeOwner::exact(
            super::super::CharacterDialogueRuntimeSchema::opaque_type_producer(),
            RuntimeSemanticTypeId::from_bytes([0x5a; 32]),
        )
    }

    #[test]
    fn policy_graph_retains_all_owner_domains_and_structural_payload_types() {
        let rich_text = rich_text_owner();
        let policies = CharacterDialoguePolicyTypeGraph::try_new(
            rich_text.clone(),
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
        let graph = policies.schema_graph();
        assert_eq!(graph.definitions().len(), POLICY_VARIANT_COUNT);

        let type_seeds = policies.type_seeds();
        let domain_seeds = policies.variant_domain_seeds();
        assert_eq!(domain_seeds.len(), POLICY_VARIANT_COUNT);

        for variant in &POLICY_VARIANTS {
            let identity = variant.owner.schema_identity();
            let RuntimeVariantIdentity::Nominal {
                nominal,
                semantic_identity,
                layout,
            } = policies.identity(variant.owner)
            else {
                panic!("CharacterDialogue policies use nominal variants");
            };
            assert_eq!(nominal, identity.nominal());
            assert_eq!(*semantic_identity, identity.semantic_identity());
            assert_eq!(
                *layout,
                graph.try_layout_hash(identity.semantic_identity()).unwrap()
            );
            assert!(type_seeds.iter().any(|seed| {
                seed.semantic_identity() == identity.semantic_identity()
                    && matches!(seed.projection(), Projection::Nominal { nominal, layout: seed_layout, .. }
                        if nominal == identity.nominal() && seed_layout == layout)
            }));

            let domain = domain_seeds
                .iter()
                .find(|domain| domain.owner() == identity.semantic_identity())
                .expect("every policy owner has a domain seed");
            assert_eq!(domain.nominal(), identity.nominal());
            assert_eq!(domain.layout(), *layout);
            assert_eq!(
                domain
                    .cases()
                    .iter()
                    .map(RuntimeVariantCaseSeed::name)
                    .collect::<Vec<_>>(),
                variant
                    .cases
                    .iter()
                    .map(|case| case.name)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                domain
                    .cases()
                    .iter()
                    .map(RuntimeVariantCaseSeed::payload)
                    .collect::<Vec<_>>(),
                variant
                    .cases
                    .iter()
                    .map(|case| {
                        case.payload.map(|payload| {
                            let checked =
                                checked_type_for_seed(payload, &rich_text, graph).unwrap();
                            seed_identity(&checked)
                        })
                    })
                    .collect::<Vec<_>>()
            );
        }

        let fallback_style = CharacterDialoguePolicyVariantOwner::FallbackStyle.schema_identity();
        let fallback_layout = graph
            .try_layout_hash(fallback_style.semantic_identity())
            .unwrap();
        let fallback_style_type = RuntimeCheckedType::Nominal {
            nominal: fallback_style.nominal().clone(),
            semantic_identity: fallback_style.semantic_identity(),
            layout: fallback_layout,
            arguments: Vec::new(),
        };
        let text_payload =
            RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::String, fallback_style_type]);
        assert!(type_seeds.iter().any(|seed| {
            seed.semantic_identity() == seed_identity(&text_payload)
                && matches!(seed.projection(), Projection::Tuple(items)
                if items.as_ref() == [
                    RuntimeCheckedType::String.semantic_identity_digest(),
                    fallback_style.semantic_identity(),
                ])
        }));

        let style_choice = RuntimeCheckedType::Choice(vec![
            RuntimeCheckedType::EntityReference,
            RuntimeCheckedType::Opaque {
                owner: rich_text.clone(),
            },
        ]);
        let style_sequence = RuntimeCheckedType::Sequence(Box::new(style_choice.clone()));
        assert!(type_seeds.iter().any(|seed| {
            seed.semantic_identity() == style_choice.semantic_identity_digest()
                && matches!(seed.projection(), Projection::Choice(items)
                if items.as_ref() == &[
                    RuntimeCheckedType::EntityReference.semantic_identity_digest(),
                    rich_text.semantic_identity(),
                ])
        }));
        assert!(type_seeds.iter().any(|seed| {
            seed.semantic_identity() == rich_text.semantic_identity()
                && matches!(seed.projection(), Projection::Opaque { producer, .. }
                    if producer == rich_text.producer())
        }));
        assert!(type_seeds.iter().any(|seed| {
            seed.semantic_identity() == style_sequence.semantic_identity_digest()
                && matches!(seed.projection(), Projection::Sequence { item, .. }
                    if *item == style_choice.semantic_identity_digest())
        }));
    }
}
