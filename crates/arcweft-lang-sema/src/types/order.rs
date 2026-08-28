use core::cmp::Ordering;

use crate::effect_row::{EffectRow, EffectRowTail};

use super::{
    EntityKind, EntityType, HandleState, IteratorStateKind, MapKind, StageActorHandleType,
    TypeKind, VariantPayloadShape, VariantPayloadType,
};

impl TypeKind {
    /// Compares semantic types structurally for deterministic in-memory indexes.
    ///
    /// This is deliberately not an `Ord` implementation: type equality remains
    /// the semantic contract, while the ordering only canonicalizes semantic
    /// choices and otherwise unordered in-memory inventories.
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive structural comparator keeps every TypeKind payload ordering adjacent to its variant tag"
    )]
    pub(crate) fn stable_ordering(&self, other: &Self) -> Ordering {
        type_kind_tag(self)
            .cmp(&type_kind_tag(other))
            .then_with(|| match (self, other) {
                (Self::Range(left), Self::Range(right))
                | (Self::Probe(left), Self::Probe(right))
                | (Self::Vec(left), Self::Vec(right))
                | (Self::Slice(left), Self::Slice(right))
                | (Self::Seq(left), Self::Seq(right))
                | (Self::Option(left), Self::Option(right))
                | (Self::ThreadHandle(left), Self::ThreadHandle(right))
                | (Self::Shared(left), Self::Shared(right))
                | (Self::Need(left), Self::Need(right)) => left.stable_ordering(right),
                (
                    Self::IteratorState {
                        family: left_family,
                        item: left_item,
                    },
                    Self::IteratorState {
                        family: right_family,
                        item: right_item,
                    },
                ) => iterator_state_tag(*left_family)
                    .cmp(&iterator_state_tag(*right_family))
                    .then_with(|| left_item.stable_ordering(right_item)),
                (Self::Ref(left), Self::Ref(right)) => entity_type_ordering(left, right),
                (Self::StageApi(left), Self::StageApi(right)) => left.as_str().cmp(right.as_str()),
                (Self::StageActorHandle(left), Self::StageActorHandle(right)) => {
                    stage_actor_handle_ordering(left, right)
                }
                (
                    Self::Array {
                        item: left_item,
                        len: left_len,
                    },
                    Self::Array {
                        item: right_item,
                        len: right_len,
                    },
                ) => left_item
                    .stable_ordering(right_item)
                    .then_with(|| left_len.cmp(right_len)),
                (
                    Self::Map {
                        kind: left_kind,
                        key: left_key,
                        value: left_value,
                    },
                    Self::Map {
                        kind: right_kind,
                        key: right_key,
                        value: right_value,
                    },
                ) => map_kind_tag(*left_kind)
                    .cmp(&map_kind_tag(*right_kind))
                    .then_with(|| left_key.stable_ordering(right_key))
                    .then_with(|| left_value.stable_ordering(right_value)),
                (
                    Self::BorrowRef {
                        kind: left_kind,
                        lifetime: left_lifetime,
                        inner: left_inner,
                    },
                    Self::BorrowRef {
                        kind: right_kind,
                        lifetime: right_lifetime,
                        inner: right_inner,
                    },
                ) => left_kind
                    .cmp(right_kind)
                    .then_with(|| left_lifetime.cmp(right_lifetime))
                    .then_with(|| left_inner.stable_ordering(right_inner)),
                (
                    Self::Stream {
                        item: left_item,
                        error: left_error,
                    },
                    Self::Stream {
                        item: right_item,
                        error: right_error,
                    },
                ) => left_item
                    .stable_ordering(right_item)
                    .then_with(|| left_error.stable_ordering(right_error)),
                (
                    Self::Parser {
                        item: left_item,
                        error: left_error,
                    },
                    Self::Parser {
                        item: right_item,
                        error: right_error,
                    },
                ) => left_item
                    .stable_ordering(right_item)
                    .then_with(|| left_error.stable_ordering(right_error)),
                (
                    Self::Result {
                        ok: left_ok,
                        error: left_error,
                    },
                    Self::Result {
                        ok: right_ok,
                        error: right_error,
                    },
                ) => left_ok
                    .stable_ordering(right_ok)
                    .then_with(|| left_error.stable_ordering(right_error)),
                (
                    Self::Handle {
                        name: left_name,
                        lifetime: left_lifetime,
                        state: left_state,
                        must_drop: left_must_drop,
                    },
                    Self::Handle {
                        name: right_name,
                        lifetime: right_lifetime,
                        state: right_state,
                        must_drop: right_must_drop,
                    },
                ) => left_name
                    .cmp(right_name)
                    .then_with(|| left_lifetime.cmp(right_lifetime))
                    .then_with(|| {
                        handle_state_tag(*left_state).cmp(&handle_state_tag(*right_state))
                    })
                    .then_with(|| left_must_drop.cmp(right_must_drop)),
                (
                    Self::Function {
                        params: left_params,
                        return_type: left_return,
                        effects: left_effects,
                    },
                    Self::Function {
                        params: right_params,
                        return_type: right_return,
                        effects: right_effects,
                    },
                ) => type_slice_ordering(left_params, right_params)
                    .then_with(|| left_return.stable_ordering(right_return))
                    .then_with(|| effect_row_ordering(left_effects, right_effects)),
                (Self::GenericParam(left), Self::GenericParam(right)) => left.cmp(right),
                (Self::ProjectNominal(left), Self::ProjectNominal(right)) => left
                    .declaration()
                    .cmp(right.declaration())
                    .then_with(|| type_slice_ordering(left.arguments(), right.arguments())),
                (Self::AcceptedNominal(left), Self::AcceptedNominal(right)) => left
                    .declaration()
                    .cmp(right.declaration())
                    .then_with(|| type_slice_ordering(left.arguments(), right.arguments())),
                (Self::OpenNominal(left), Self::OpenNominal(right)) => left
                    .rule()
                    .cmp(right.rule())
                    .then_with(|| left.path().cmp(right.path()))
                    .then_with(|| type_slice_ordering(left.arguments(), right.arguments())),
                (Self::Error(left), Self::Error(right)) => left.cmp(right),
                (Self::Named(left), Self::Named(right)) => left.cmp(right),
                (
                    Self::Projection {
                        subject: left_subject,
                        trait_name: left_trait,
                        assoc: left_assoc,
                    },
                    Self::Projection {
                        subject: right_subject,
                        trait_name: right_trait,
                        assoc: right_assoc,
                    },
                ) => left_subject
                    .stable_ordering(right_subject)
                    .then_with(|| left_trait.cmp(right_trait))
                    .then_with(|| left_assoc.cmp(right_assoc)),
                (Self::CharacterPatch(left), Self::CharacterPatch(right)) => {
                    entity_kind_ordering(left, right)
                }
                (Self::CharacterDialogue(left), Self::CharacterDialogue(right)) => left.cmp(right),
                (Self::DialogueLine(left), Self::DialogueLine(right)) => {
                    left.stable_ordering(right)
                }
                (Self::CharacterNominal(left), Self::CharacterNominal(right)) => left.cmp(right),
                (Self::AgentBuiltin(left), Self::AgentBuiltin(right)) => left.cmp(right),
                (Self::Tuple(left), Self::Tuple(right))
                | (Self::Choice(left), Self::Choice(right)) => type_slice_ordering(left, right),
                (Self::VariantPayload(left), Self::VariantPayload(right)) => {
                    variant_payload_ordering(left, right)
                }
                _ => Ordering::Equal,
            })
    }
}

fn stage_actor_handle_ordering(
    left: &StageActorHandleType,
    right: &StageActorHandleType,
) -> Ordering {
    match (left, right) {
        (StageActorHandleType::Any, StageActorHandleType::Any) => Ordering::Equal,
        (StageActorHandleType::Any, StageActorHandleType::Exact(_)) => Ordering::Less,
        (StageActorHandleType::Exact(_), StageActorHandleType::Any) => Ordering::Greater,
        (StageActorHandleType::Exact(left), StageActorHandleType::Exact(right)) => {
            left.as_str().cmp(right.as_str())
        }
    }
}

fn type_slice_ordering(left: &[TypeKind], right: &[TypeKind]) -> Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.stable_ordering(right))
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn variant_payload_ordering(left: &VariantPayloadType, right: &VariantPayloadType) -> Ordering {
    left.owner_family()
        .canonical_tag()
        .cmp(&right.owner_family().canonical_tag())
        .then_with(|| left.owner_type().cmp(&right.owner_type()))
        .then_with(|| left.case_ordinal().cmp(&right.case_ordinal()))
        .then_with(|| left.case().cmp(&right.case()))
        .then_with(|| variant_payload_shape_ordering(left.shape(), right.shape()))
}

fn variant_payload_shape_ordering(
    left: &VariantPayloadShape,
    right: &VariantPayloadShape,
) -> Ordering {
    match (left, right) {
        (VariantPayloadShape::Unit, VariantPayloadShape::Unit) => Ordering::Equal,
        (VariantPayloadShape::Unit, _) => Ordering::Less,
        (_, VariantPayloadShape::Unit) => Ordering::Greater,
        (VariantPayloadShape::Tuple(left), VariantPayloadShape::Tuple(right)) => left
            .iter()
            .zip(right)
            .map(|(left, right)| {
                left.ordinal()
                    .cmp(&right.ordinal())
                    .then_with(|| left.semantic_id().cmp(&right.semantic_id()))
                    .then_with(|| left.ty().stable_ordering(right.ty()))
            })
            .find(|ordering| *ordering != Ordering::Equal)
            .unwrap_or_else(|| left.len().cmp(&right.len())),
        (VariantPayloadShape::Record(left), VariantPayloadShape::Record(right)) => left
            .iter()
            .zip(right)
            .map(|(left, right)| {
                left.ordinal()
                    .cmp(&right.ordinal())
                    .then_with(|| left.semantic_id().cmp(&right.semantic_id()))
                    .then_with(|| left.ty().stable_ordering(right.ty()))
            })
            .find(|ordering| *ordering != Ordering::Equal)
            .unwrap_or_else(|| left.len().cmp(&right.len())),
        (VariantPayloadShape::Tuple(_), VariantPayloadShape::Record(_)) => Ordering::Less,
        (VariantPayloadShape::Record(_), VariantPayloadShape::Tuple(_)) => Ordering::Greater,
    }
}

fn entity_type_ordering(left: &EntityType, right: &EntityType) -> Ordering {
    entity_kind_ordering(left.kind(), right.kind()).then_with(|| {
        match (left.value(), right.value()) {
            (Some(left), Some(right)) => left.stable_ordering(right),
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    })
}

fn entity_kind_ordering(left: &EntityKind, right: &EntityKind) -> Ordering {
    entity_kind_tag(left)
        .cmp(&entity_kind_tag(right))
        .then_with(|| match (left, right) {
            (EntityKind::Other(left), EntityKind::Other(right)) => left.cmp(right),
            _ => Ordering::Equal,
        })
}

fn effect_row_ordering(left: &EffectRow, right: &EffectRow) -> Ordering {
    left.concrete()
        .iter()
        .cmp(right.concrete().iter())
        .then_with(|| effect_row_tail_ordering(left.tail(), right.tail()))
}

fn effect_row_tail_ordering(left: EffectRowTail, right: EffectRowTail) -> Ordering {
    effect_row_tail_tag(left)
        .cmp(&effect_row_tail_tag(right))
        .then_with(|| match (left, right) {
            (EffectRowTail::Variable(left), EffectRowTail::Variable(right)) => {
                left.index().cmp(&right.index())
            }
            _ => Ordering::Equal,
        })
}

const fn effect_row_tail_tag(tail: EffectRowTail) -> u8 {
    match tail {
        EffectRowTail::Closed => 0,
        EffectRowTail::Variable(_) => 1,
        EffectRowTail::Unknown => 2,
    }
}

const fn iterator_state_tag(kind: IteratorStateKind) -> u8 {
    match kind {
        IteratorStateKind::Range => 0,
        IteratorStateKind::Seq => 1,
        IteratorStateKind::Stream => 2,
        IteratorStateKind::Vec => 3,
        IteratorStateKind::Array => 4,
        IteratorStateKind::Slice => 5,
    }
}

const fn map_kind_tag(kind: MapKind) -> u8 {
    match kind {
        MapKind::Ordered => 0,
        MapKind::Sorted => 1,
        MapKind::BTree => 2,
    }
}

const fn handle_state_tag(state: HandleState) -> u8 {
    match state {
        HandleState::Live => 0,
        HandleState::Dropped => 1,
        HandleState::Detached => 2,
        HandleState::MovedOut => 3,
    }
}

const fn entity_kind_tag(kind: &EntityKind) -> u8 {
    match kind {
        EntityKind::Agent => 0,
        EntityKind::Entry => 1,
        EntityKind::Flow => 2,
        EntityKind::Choice => 3,
        EntityKind::ChoiceOption => 4,
        EntityKind::Character => 5,
        EntityKind::View => 6,
        EntityKind::Action => 7,
        EntityKind::Activity => 8,
        EntityKind::DialogueLine => 9,
        EntityKind::Text => 10,
        EntityKind::Content => 11,
        EntityKind::Input => 12,
        EntityKind::Button => 13,
        EntityKind::Style => 14,
        EntityKind::Asset => 15,
        EntityKind::Image => 16,
        EntityKind::Animation => 17,
        EntityKind::Capture => 18,
        EntityKind::Hook => 19,
        EntityKind::Signal => 20,
        EntityKind::Metric => 21,
        EntityKind::Scene => 22,
        EntityKind::Test => 24,
        EntityKind::Bench => 25,
        EntityKind::Layer => 26,
        EntityKind::Voice => 27,
        EntityKind::Se => 28,
        EntityKind::Bgm => 29,
        EntityKind::AudioBus => 30,
        EntityKind::MixerSnapshot => 31,
        EntityKind::Ducking => 32,
        EntityKind::Motion => 33,
        EntityKind::Rig => 34,
        EntityKind::Slot => 35,
        EntityKind::Target => 36,
        EntityKind::Other(_) => 37,
    }
}

const fn type_kind_tag(kind: &TypeKind) -> u8 {
    match kind {
        TypeKind::Bool => 0,
        TypeKind::I8 => 1,
        TypeKind::I16 => 2,
        TypeKind::I32 => 3,
        TypeKind::I64 => 4,
        TypeKind::I128 => 5,
        TypeKind::ISize => 6,
        TypeKind::U8 => 7,
        TypeKind::U16 => 8,
        TypeKind::U32 => 9,
        TypeKind::U64 => 10,
        TypeKind::U128 => 11,
        TypeKind::USize => 12,
        TypeKind::F32 => 13,
        TypeKind::F64 => 14,
        TypeKind::String => 15,
        TypeKind::Char => 16,
        TypeKind::Bytes => 17,
        TypeKind::TextCluster => 18,
        TypeKind::Duration => 19,
        TypeKind::Range(_) => 20,
        TypeKind::IteratorState { .. } => 21,
        TypeKind::DisplayText => 22,
        TypeKind::DebugStatePath => 23,
        TypeKind::ObservationFieldPath => 24,
        TypeKind::Ref(_) => 25,
        TypeKind::Probe(_) => 26,
        TypeKind::Predicate => 27,
        TypeKind::Observation => 28,
        TypeKind::ObservedObject => 29,
        TypeKind::AgentBBox => 30,
        TypeKind::ActionName => 31,
        TypeKind::ActionTarget => 32,
        TypeKind::ActionResult => 33,
        TypeKind::AgentValue => 34,
        TypeKind::DataFormat => 35,
        TypeKind::DataShape => 36,
        TypeKind::AgentEntityMetadata => 37,
        TypeKind::AgentSourceAnchor => 38,
        TypeKind::AgentProjectGraphNeighborhood => 39,
        TypeKind::AgentProjectGraphSymbol => 40,
        TypeKind::AgentProjectGraphEdge => 41,
        TypeKind::CaptureTarget => 42,
        TypeKind::CaptureRef => 43,
        TypeKind::AgentResource => 44,
        TypeKind::AgentResourceBody => 45,
        TypeKind::RagContextPack => 46,
        TypeKind::Vec(_) => 47,
        TypeKind::Array { .. } => 48,
        TypeKind::Slice(_) => 49,
        TypeKind::Seq(_) => 50,
        TypeKind::Map { .. } => 51,
        TypeKind::BorrowRef { .. } => 52,
        TypeKind::Need(_) => 53,
        TypeKind::Stream { .. } => 54,
        TypeKind::Parser { .. } => 55,
        TypeKind::Result { .. } => 56,
        TypeKind::Option(_) => 57,
        TypeKind::Handle { .. } => 58,
        TypeKind::ThreadHandle(_) => 59,
        TypeKind::Shared(_) => 60,
        TypeKind::Function { .. } => 61,
        TypeKind::GenericParam(_) => 62,
        TypeKind::ProjectNominal(_) => 63,
        TypeKind::AcceptedNominal(_) => 64,
        TypeKind::OpenNominal(_) => 65,
        TypeKind::Error(_) => 66,
        TypeKind::Projection { .. } => 67,
        TypeKind::CharacterDialogue(_) => 68,
        TypeKind::DialogueLine(_) => 69,
        TypeKind::CharacterPatch(_) => 70,
        TypeKind::FocusPatch => 71,
        TypeKind::CharacterNominal(_) => 72,
        TypeKind::Named(_) => 73,
        TypeKind::Tuple(_) => 74,
        TypeKind::Choice(_) => 75,
        TypeKind::Unit => 76,
        TypeKind::Never => 77,
        TypeKind::AgentBuiltin(_) => 78,
        TypeKind::ViewValue => 79,
        TypeKind::Progress => 80,
        TypeKind::StageApi(_) => 81,
        TypeKind::LineContext => 82,
        TypeKind::StageActorHandle(_) => 83,
        TypeKind::CueHandle => 84,
        TypeKind::VoiceHandle => 85,
        TypeKind::VariantPayload(_) => 86,
    }
}
