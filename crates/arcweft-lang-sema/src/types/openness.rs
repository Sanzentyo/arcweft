//! Recursive open-type classification used by overload selection.

use crate::effect_row::EffectRowTail;

use super::TypeKind;

impl TypeKind {
    /// Reports whether a resolved type still contains authoritative nominal
    /// recovery poison.
    ///
    /// Consumers use this to suppress only diagnostics whose comparison would
    /// depend on a type node that has already failed nominal resolution.
    pub(crate) fn contains_nominal_poison(&self) -> bool {
        match self {
            Self::Error(_) => true,
            Self::Ref(entity) => entity.value().is_some_and(Self::contains_nominal_poison),
            Self::Range(inner)
            | Self::Probe(inner)
            | Self::Vec(inner)
            | Self::Slice(inner)
            | Self::Seq(inner)
            | Self::Option(inner)
            | Self::ThreadHandle(inner)
            | Self::Shared(inner)
            | Self::BorrowRef { inner, .. } => inner.contains_nominal_poison(),
            Self::IteratorState { item, .. } => item.contains_nominal_poison(),
            Self::Array { item, len } => {
                matches!(len, super::ArrayLength::Error(_)) || item.contains_nominal_poison()
            }
            Self::Map { key, value, .. } => {
                key.contains_nominal_poison() || value.contains_nominal_poison()
            }
            Self::Need { ready, error } => {
                ready.contains_nominal_poison() || error.contains_nominal_poison()
            }
            Self::Stream { item, error } | Self::Source { item, error } => {
                item.contains_nominal_poison() || error.contains_nominal_poison()
            }
            Self::Result { ok, error } => {
                ok.contains_nominal_poison() || error.contains_nominal_poison()
            }
            Self::Function {
                params,
                return_type,
                ..
            } => {
                params.iter().any(Self::contains_nominal_poison)
                    || return_type.contains_nominal_poison()
            }
            Self::ProjectNominal(nominal) => arguments_contain_nominal_poison(nominal.arguments()),
            Self::AcceptedNominal(nominal) => arguments_contain_nominal_poison(nominal.arguments()),
            Self::OpenNominal(nominal) => arguments_contain_nominal_poison(nominal.arguments()),
            Self::Projection { subject, .. } => subject.contains_nominal_poison(),
            Self::Tuple(items) | Self::Choice(items) => {
                items.iter().any(Self::contains_nominal_poison)
            }
            Self::Bool
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64
            | Self::I128
            | Self::ISize
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::U128
            | Self::USize
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::TextCluster
            | Self::Duration
            | Self::DisplayText
            | Self::DebugStatePath
            | Self::ObservationFieldPath
            | Self::Predicate
            | Self::Observation
            | Self::ObservedObject
            | Self::AgentBBox
            | Self::ActionName
            | Self::ActionTarget
            | Self::ActionResult
            | Self::AgentValue
            | Self::DataFormat
            | Self::DataShape
            | Self::AgentEntityMetadata
            | Self::AgentSourceAnchor
            | Self::AgentProjectGraphNeighborhood
            | Self::AgentProjectGraphSymbol
            | Self::AgentProjectGraphEdge
            | Self::CaptureTarget
            | Self::CaptureRef
            | Self::AgentResource
            | Self::AgentResourceBody
            | Self::RagContextPack
            | Self::GenericParam(_)
            | Self::Handle { .. }
            | Self::Speaker(_)
            | Self::SpeakerPreset(_)
            | Self::CharacterPatch(_)
            | Self::FocusPatch
            | Self::CharacterNominal(_)
            | Self::Named(_)
            | Self::Unit
            | Self::Never => false,
        }
    }

    /// Reports whether overload matching sees an unresolved or wildcard component.
    pub(crate) fn has_open_components(&self) -> bool {
        match self {
            Self::Named(name) => name == "_",
            Self::GenericParam(_) | Self::Projection { .. } | Self::Error(_) => true,
            Self::Ref(entity) => entity.value().is_some_and(Self::has_open_components),
            Self::Range(inner)
            | Self::Probe(inner)
            | Self::Vec(inner)
            | Self::Slice(inner)
            | Self::Seq(inner)
            | Self::Option(inner)
            | Self::ThreadHandle(inner)
            | Self::Shared(inner)
            | Self::BorrowRef { inner, .. } => inner.has_open_components(),
            Self::IteratorState { item, .. } => item.has_open_components(),
            Self::Array { item, len } => len.has_open_components() || item.has_open_components(),
            Self::Map { key, value, .. } => {
                key.has_open_components() || value.has_open_components()
            }
            Self::Need { ready, error } => {
                ready.has_open_components() || error.has_open_components()
            }
            Self::Stream { item, error } | Self::Source { item, error } => {
                item.has_open_components() || error.has_open_components()
            }
            Self::Result { ok, error } => ok.has_open_components() || error.has_open_components(),
            Self::Function {
                params,
                return_type,
                effects,
            } => {
                params.iter().any(Self::has_open_components)
                    || return_type.has_open_components()
                    || !matches!(effects.tail(), EffectRowTail::Closed)
            }
            Self::ProjectNominal(nominal) => {
                nominal.arguments().iter().any(Self::has_open_components)
            }
            Self::AcceptedNominal(nominal) => {
                nominal.arguments().iter().any(Self::has_open_components)
            }
            Self::OpenNominal(nominal) => nominal.arguments().iter().any(Self::has_open_components),
            Self::Tuple(items) | Self::Choice(items) => items.iter().any(Self::has_open_components),
            Self::Bool
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64
            | Self::I128
            | Self::ISize
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::U128
            | Self::USize
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::TextCluster
            | Self::Duration
            | Self::DisplayText
            | Self::DebugStatePath
            | Self::ObservationFieldPath
            | Self::Predicate
            | Self::Observation
            | Self::ObservedObject
            | Self::AgentBBox
            | Self::ActionName
            | Self::ActionTarget
            | Self::ActionResult
            | Self::AgentValue
            | Self::DataFormat
            | Self::DataShape
            | Self::AgentEntityMetadata
            | Self::AgentSourceAnchor
            | Self::AgentProjectGraphNeighborhood
            | Self::AgentProjectGraphSymbol
            | Self::AgentProjectGraphEdge
            | Self::CaptureTarget
            | Self::CaptureRef
            | Self::AgentResource
            | Self::AgentResourceBody
            | Self::RagContextPack
            | Self::Handle { .. }
            | Self::Speaker(_)
            | Self::SpeakerPreset(_)
            | Self::CharacterPatch(_)
            | Self::FocusPatch
            | Self::CharacterNominal(_)
            | Self::Unit
            | Self::Never => false,
        }
    }
}

fn arguments_contain_nominal_poison(arguments: &[TypeKind]) -> bool {
    arguments.iter().any(TypeKind::contains_nominal_poison)
}
