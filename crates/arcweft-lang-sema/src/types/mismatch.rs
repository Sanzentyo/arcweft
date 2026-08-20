//! Exhaustive deterministic structural mismatch traversal.

use arcweft_character::id::{CharacterId, CharacterPartId};

use super::{CharacterNominalFamily, TypeKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeMismatch {
    path: Vec<TypeMismatchPathSegment>,
    reason: TypeMismatchReason,
    expected: TypeKind,
    actual: TypeKind,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeMismatchPathSegment {
    IteratorFamily,
    RangeItem,
    IteratorItem,
    EntityKind,
    EntityPayloadPresence,
    EntityPayload,
    ProbeItem,
    VectorItem,
    ArrayLength,
    ArrayItem,
    SliceItem,
    SequenceItem,
    MapFamily,
    MapKey,
    MapValue,
    BorrowKind,
    BorrowLifetime,
    BorrowInner,
    NeedItem,
    StreamItem,
    StreamError,
    ResultOk,
    ResultError,
    OptionItem,
    HandleName,
    HandleLifetime,
    HandleState,
    HandleMustDrop,
    ThreadResult,
    SharedInner,
    FunctionArity,
    FunctionEffects,
    FunctionParameter(usize),
    FunctionReturn,
    GenericIdentity,
    ProjectNominalDeclaration,
    ProjectNominalArgument(usize),
    AcceptedNominalDeclaration,
    AcceptedNominalArgument(usize),
    OpenNominalRule,
    OpenNominalPath,
    OpenNominalArgument(usize),
    TypePoison,
    ProjectionTrait,
    ProjectionAssociation,
    ProjectionSubject,
    CharacterDialogueCharacter,
    DialogueLineResult,
    CharacterPatchKind,
    CharacterFamily,
    CharacterOwner,
    CharacterVariantPart,
    NamedName,
    AgentBuiltin,
    TupleArity,
    TupleElement(usize),
    ChoiceArity,
    ChoiceAlternative(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeMismatchReason {
    OuterConstructor,
    NonTypeParameter,
    PayloadPresence,
    Arity {
        expected: usize,
        actual: usize,
    },
    CharacterFamily {
        expected: CharacterNominalFamily,
        actual: CharacterNominalFamily,
    },
    CharacterOwner {
        expected: CharacterId,
        actual: CharacterId,
    },
    CharacterVariantPart {
        expected: CharacterPartId,
        actual: CharacterPartId,
    },
}

impl TypeMismatch {
    pub fn path(&self) -> &[TypeMismatchPathSegment] {
        &self.path
    }

    pub const fn reason(&self) -> &TypeMismatchReason {
        &self.reason
    }

    pub const fn expected(&self) -> &TypeKind {
        &self.expected
    }

    pub const fn actual(&self) -> &TypeKind {
        &self.actual
    }

    fn at(
        expected: &TypeKind,
        actual: &TypeKind,
        segment: TypeMismatchPathSegment,
        reason: TypeMismatchReason,
    ) -> Self {
        Self {
            path: vec![segment],
            reason,
            expected: expected.clone(),
            actual: actual.clone(),
        }
    }

    fn outer(expected: &TypeKind, actual: &TypeKind) -> Self {
        Self {
            path: Vec::new(),
            reason: TypeMismatchReason::OuterConstructor,
            expected: expected.clone(),
            actual: actual.clone(),
        }
    }

    fn prepend(mut self, segment: TypeMismatchPathSegment) -> Self {
        self.path.insert(0, segment);
        self
    }
}

impl TypeKind {
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match keeps every TypeKind variant and recursive mismatch path auditable"
    )]
    pub fn first_mismatch(&self, actual: &Self) -> Option<TypeMismatch> {
        if core::mem::discriminant(self) != core::mem::discriminant(actual) {
            return Some(TypeMismatch::outer(self, actual));
        }

        match self {
            Self::Bool => {
                let Self::Bool = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::I8 => {
                let Self::I8 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::I16 => {
                let Self::I16 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::I32 => {
                let Self::I32 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::I64 => {
                let Self::I64 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::I128 => {
                let Self::I128 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::ISize => {
                let Self::ISize = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::U8 => {
                let Self::U8 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::U16 => {
                let Self::U16 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::U32 => {
                let Self::U32 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::U64 => {
                let Self::U64 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::U128 => {
                let Self::U128 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::USize => {
                let Self::USize = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::F32 => {
                let Self::F32 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::F64 => {
                let Self::F64 = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::String => {
                let Self::String = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Char => {
                let Self::Char = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Bytes => {
                let Self::Bytes = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::TextCluster => {
                let Self::TextCluster = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Duration => {
                let Self::Duration = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Progress => {
                let Self::Progress = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Range(expected) => {
                let Self::Range(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::RangeItem))
            }
            Self::IteratorState {
                family: expected_family,
                item: expected,
            } => {
                let Self::IteratorState {
                    family: actual_family,
                    item: actual_item,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                if expected_family == actual_family {
                    expected
                        .first_mismatch(actual_item)
                        .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::IteratorItem))
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::IteratorFamily,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                }
            }
            Self::DisplayText => {
                let Self::DisplayText = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::DebugStatePath => {
                let Self::DebugStatePath = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::ObservationFieldPath => {
                let Self::ObservationFieldPath = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Ref(expected) => {
                let Self::Ref(actual_entity) = actual else {
                    unreachable!("equal discriminants")
                };
                if expected.kind() != actual_entity.kind() {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::EntityKind,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else if expected.value().is_some() != actual_entity.value().is_some() {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::EntityPayloadPresence,
                        TypeMismatchReason::PayloadPresence,
                    ))
                } else {
                    expected
                        .value()
                        .zip(actual_entity.value())
                        .and_then(|(expected, actual)| expected.first_mismatch(actual))
                        .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::EntityPayload))
                }
            }
            Self::Probe(expected) => {
                let Self::Probe(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::ProbeItem))
            }
            Self::Predicate => {
                let Self::Predicate = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Observation => {
                let Self::Observation = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::ObservedObject => {
                let Self::ObservedObject = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentBBox => {
                let Self::AgentBBox = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::ActionName => {
                let Self::ActionName = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::ActionTarget => {
                let Self::ActionTarget = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::ActionResult => {
                let Self::ActionResult = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentValue => {
                let Self::AgentValue = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::DataFormat => {
                let Self::DataFormat = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::DataShape => {
                let Self::DataShape = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentEntityMetadata => {
                let Self::AgentEntityMetadata = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentSourceAnchor => {
                let Self::AgentSourceAnchor = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentProjectGraphNeighborhood => {
                let Self::AgentProjectGraphNeighborhood = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentProjectGraphSymbol => {
                let Self::AgentProjectGraphSymbol = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentProjectGraphEdge => {
                let Self::AgentProjectGraphEdge = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::CaptureTarget => {
                let Self::CaptureTarget = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::CaptureRef => {
                let Self::CaptureRef = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentResource => {
                let Self::AgentResource = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::AgentResourceBody => {
                let Self::AgentResourceBody = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::RagContextPack => {
                let Self::RagContextPack = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Vec(expected) => {
                let Self::Vec(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::VectorItem))
            }
            Self::Array {
                item: expected,
                len: expected_len,
            } => {
                let Self::Array {
                    item: actual_item,
                    len: actual_len,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                if expected_len.accepts(actual_len) {
                    expected
                        .first_mismatch(actual_item)
                        .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::ArrayItem))
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::ArrayLength,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                }
            }
            Self::Slice(expected) => {
                let Self::Slice(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::SliceItem))
            }
            Self::Seq(expected) => {
                let Self::Seq(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::SequenceItem))
            }
            Self::Map {
                kind: expected_kind,
                key: expected_key,
                value: expected_value,
            } => {
                let Self::Map {
                    kind: actual_kind,
                    key: actual_key,
                    value: actual_value,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                if expected_kind == actual_kind {
                    expected_key
                        .first_mismatch(actual_key)
                        .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::MapKey))
                        .or_else(|| {
                            expected_value
                                .first_mismatch(actual_value)
                                .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::MapValue))
                        })
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::MapFamily,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                }
            }
            Self::BorrowRef {
                kind: expected_kind,
                lifetime: expected_lifetime,
                inner: expected,
            } => {
                let Self::BorrowRef {
                    kind: actual_kind,
                    lifetime: actual_lifetime,
                    inner: actual_inner,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                if expected_kind != actual_kind {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::BorrowKind,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else if expected_lifetime == actual_lifetime {
                    expected
                        .first_mismatch(actual_inner)
                        .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::BorrowInner))
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::BorrowLifetime,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                }
            }
            Self::Need(expected_item) => {
                let Self::Need(actual_item) = actual else {
                    unreachable!("equal discriminants")
                };
                expected_item
                    .first_mismatch(actual_item)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::NeedItem))
            }
            Self::Stream {
                item: expected_item,
                error: expected_error,
            } => {
                let Self::Stream {
                    item: actual_item,
                    error: actual_error,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                expected_item
                    .first_mismatch(actual_item)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::StreamItem))
                    .or_else(|| {
                        expected_error
                            .first_mismatch(actual_error)
                            .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::StreamError))
                    })
            }
            Self::Result {
                ok: expected_ok,
                error: expected_error,
            } => {
                let Self::Result {
                    ok: actual_ok,
                    error: actual_error,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                expected_ok
                    .first_mismatch(actual_ok)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::ResultOk))
                    .or_else(|| {
                        expected_error
                            .first_mismatch(actual_error)
                            .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::ResultError))
                    })
            }
            Self::Option(expected) => {
                let Self::Option(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::OptionItem))
            }
            Self::Handle {
                name: expected_name,
                lifetime: expected_lifetime,
                state: expected_state,
                must_drop: expected_must_drop,
            } => {
                let Self::Handle {
                    name: actual_name,
                    lifetime: actual_lifetime,
                    state: actual_state,
                    must_drop: actual_must_drop,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                if expected_name != actual_name {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::HandleName,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else if expected_lifetime != actual_lifetime {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::HandleLifetime,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else if expected_state != actual_state {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::HandleState,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else if expected_must_drop != actual_must_drop {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::HandleMustDrop,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else {
                    None
                }
            }
            Self::ThreadHandle(expected) => {
                let Self::ThreadHandle(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::ThreadResult))
            }
            Self::Shared(expected) => {
                let Self::Shared(actual) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::SharedInner))
            }
            Self::Function {
                params: expected_params,
                return_type: expected_return,
                effects: expected_effects,
            } => {
                let Self::Function {
                    params: actual_params,
                    return_type: actual_return,
                    effects: actual_effects,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                if expected_params.len() != actual_params.len() {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::FunctionArity,
                        TypeMismatchReason::Arity {
                            expected: expected_params.len(),
                            actual: actual_params.len(),
                        },
                    ))
                } else if expected_effects != actual_effects {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::FunctionEffects,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else {
                    expected_params
                        .iter()
                        .zip(actual_params)
                        .enumerate()
                        .find_map(|(index, (expected, actual))| {
                            expected.first_mismatch(actual).map(|mismatch| {
                                mismatch.prepend(TypeMismatchPathSegment::FunctionParameter(index))
                            })
                        })
                        .or_else(|| {
                            expected_return
                                .first_mismatch(actual_return)
                                .map(|mismatch| {
                                    mismatch.prepend(TypeMismatchPathSegment::FunctionReturn)
                                })
                        })
                }
            }
            Self::GenericParam(expected_id) => {
                let Self::GenericParam(actual_id) = actual else {
                    unreachable!("equal discriminants")
                };
                (expected_id != actual_id).then(|| {
                    TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::GenericIdentity,
                        TypeMismatchReason::NonTypeParameter,
                    )
                })
            }
            Self::ProjectNominal(expected) => {
                let Self::ProjectNominal(actual_nominal) = actual else {
                    unreachable!("equal discriminants")
                };
                if expected.declaration() == actual_nominal.declaration() {
                    nominal_arguments_mismatch(
                        self,
                        actual,
                        expected.arguments(),
                        actual_nominal.arguments(),
                        TypeMismatchPathSegment::ProjectNominalArgument,
                    )
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::ProjectNominalDeclaration,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                }
            }
            Self::AcceptedNominal(expected) => {
                let Self::AcceptedNominal(actual_nominal) = actual else {
                    unreachable!("equal discriminants")
                };
                if expected.declaration() == actual_nominal.declaration() {
                    nominal_arguments_mismatch(
                        self,
                        actual,
                        expected.arguments(),
                        actual_nominal.arguments(),
                        TypeMismatchPathSegment::AcceptedNominalArgument,
                    )
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::AcceptedNominalDeclaration,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                }
            }
            Self::OpenNominal(expected) => {
                let Self::OpenNominal(actual_nominal) = actual else {
                    unreachable!("equal discriminants")
                };
                if expected.rule() != actual_nominal.rule() {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::OpenNominalRule,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else if expected.path() != actual_nominal.path() {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::OpenNominalPath,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else {
                    nominal_arguments_mismatch(
                        self,
                        actual,
                        expected.arguments(),
                        actual_nominal.arguments(),
                        TypeMismatchPathSegment::OpenNominalArgument,
                    )
                }
            }
            Self::Error(expected_poison) => {
                let Self::Error(actual_poison) = actual else {
                    unreachable!("equal discriminants")
                };
                (expected_poison != actual_poison).then(|| {
                    TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::TypePoison,
                        TypeMismatchReason::NonTypeParameter,
                    )
                })
            }
            Self::Projection {
                subject: expected_subject,
                trait_name: expected_trait,
                assoc: expected_assoc,
            } => {
                let Self::Projection {
                    subject: actual_subject,
                    trait_name: actual_trait,
                    assoc: actual_assoc,
                } = actual
                else {
                    unreachable!("equal discriminants")
                };
                if expected_trait != actual_trait {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::ProjectionTrait,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else if expected_assoc != actual_assoc {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::ProjectionAssociation,
                        TypeMismatchReason::NonTypeParameter,
                    ))
                } else {
                    expected_subject
                        .first_mismatch(actual_subject)
                        .map(|mismatch| {
                            mismatch.prepend(TypeMismatchPathSegment::ProjectionSubject)
                        })
                }
            }
            Self::CharacterDialogue(expected) => {
                let Self::CharacterDialogue(actual_dialogue) = actual else {
                    unreachable!("equal discriminants")
                };
                (expected != actual_dialogue).then(|| {
                    TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::CharacterDialogueCharacter,
                        TypeMismatchReason::NonTypeParameter,
                    )
                })
            }
            Self::DialogueLine(expected) => {
                let Self::DialogueLine(actual_result) = actual else {
                    unreachable!("equal discriminants")
                };
                expected
                    .first_mismatch(actual_result)
                    .map(|mismatch| mismatch.prepend(TypeMismatchPathSegment::DialogueLineResult))
            }
            Self::CharacterPatch(expected) => {
                let Self::CharacterPatch(actual_kind) = actual else {
                    unreachable!("equal discriminants")
                };
                (expected != actual_kind).then(|| {
                    TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::CharacterPatchKind,
                        TypeMismatchReason::NonTypeParameter,
                    )
                })
            }
            Self::FocusPatch => {
                let Self::FocusPatch = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::CharacterNominal(expected) => {
                let Self::CharacterNominal(actual_nominal) = actual else {
                    unreachable!("equal discriminants")
                };
                if expected.family() != actual_nominal.family() {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::CharacterFamily,
                        TypeMismatchReason::CharacterFamily {
                            expected: expected.family(),
                            actual: actual_nominal.family(),
                        },
                    ))
                } else if expected.character() != actual_nominal.character() {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::CharacterOwner,
                        TypeMismatchReason::CharacterOwner {
                            expected: expected.character().clone(),
                            actual: actual_nominal.character().clone(),
                        },
                    ))
                } else if let (Some(expected_part), Some(actual_part)) =
                    (expected.part(), actual_nominal.part())
                {
                    (expected_part != actual_part).then(|| {
                        TypeMismatch::at(
                            self,
                            actual,
                            TypeMismatchPathSegment::CharacterVariantPart,
                            TypeMismatchReason::CharacterVariantPart {
                                expected: expected_part.clone(),
                                actual: actual_part.clone(),
                            },
                        )
                    })
                } else {
                    None
                }
            }
            Self::Named(expected_name) => {
                let Self::Named(actual_name) = actual else {
                    unreachable!("equal discriminants")
                };
                (expected_name != actual_name).then(|| {
                    TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::NamedName,
                        TypeMismatchReason::NonTypeParameter,
                    )
                })
            }
            Self::AgentBuiltin(expected_builtin) => {
                let Self::AgentBuiltin(actual_builtin) = actual else {
                    unreachable!("equal discriminants")
                };
                (expected_builtin != actual_builtin).then(|| {
                    TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::AgentBuiltin,
                        TypeMismatchReason::NonTypeParameter,
                    )
                })
            }
            Self::Tuple(expected_items) => {
                let Self::Tuple(actual_items) = actual else {
                    unreachable!("equal discriminants")
                };
                if expected_items.len() == actual_items.len() {
                    expected_items
                        .iter()
                        .zip(actual_items)
                        .enumerate()
                        .find_map(|(index, (expected, actual))| {
                            expected.first_mismatch(actual).map(|mismatch| {
                                mismatch.prepend(TypeMismatchPathSegment::TupleElement(index))
                            })
                        })
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::TupleArity,
                        TypeMismatchReason::Arity {
                            expected: expected_items.len(),
                            actual: actual_items.len(),
                        },
                    ))
                }
            }
            Self::Choice(expected_items) => {
                let Self::Choice(actual_items) = actual else {
                    unreachable!("equal discriminants")
                };
                if expected_items.len() == actual_items.len() {
                    expected_items
                        .iter()
                        .zip(actual_items)
                        .enumerate()
                        .find_map(|(index, (expected, actual))| {
                            expected.first_mismatch(actual).map(|mismatch| {
                                mismatch.prepend(TypeMismatchPathSegment::ChoiceAlternative(index))
                            })
                        })
                } else {
                    Some(TypeMismatch::at(
                        self,
                        actual,
                        TypeMismatchPathSegment::ChoiceArity,
                        TypeMismatchReason::Arity {
                            expected: expected_items.len(),
                            actual: actual_items.len(),
                        },
                    ))
                }
            }
            Self::ViewValue => {
                let Self::ViewValue = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Unit => {
                let Self::Unit = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
            Self::Never => {
                let Self::Never = actual else {
                    unreachable!("equal discriminants")
                };
                None
            }
        }
    }
}

fn nominal_arguments_mismatch(
    expected_type: &TypeKind,
    actual_type: &TypeKind,
    expected: &[TypeKind],
    actual: &[TypeKind],
    path: fn(usize) -> TypeMismatchPathSegment,
) -> Option<TypeMismatch> {
    if expected.len() != actual.len() {
        return Some(TypeMismatch::at(
            expected_type,
            actual_type,
            path(expected.len().min(actual.len())),
            TypeMismatchReason::Arity {
                expected: expected.len(),
                actual: actual.len(),
            },
        ));
    }

    expected
        .iter()
        .zip(actual)
        .enumerate()
        .find_map(|(index, (expected, actual))| {
            expected
                .first_mismatch(actual)
                .map(|mismatch| mismatch.prepend(path(index)))
        })
}
