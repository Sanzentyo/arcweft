//! Recursive open-type classification used by overload selection.

use super::TypeKind;

impl TypeKind {
    /// Reports whether this type contains the non-escaping line-execution
    /// operation. A `DialogueLine<R>` may be consumed by line lowering, but it
    /// cannot become a runtime value through a container, binding, capture,
    /// callable boundary, or host payload.
    pub(crate) fn contains_dialogue_line_operation(&self) -> bool {
        match self {
            Self::DialogueLine(_) => true,
            Self::Ref(entity) => entity
                .value()
                .is_some_and(Self::contains_dialogue_line_operation),
            Self::Range(inner)
            | Self::Probe(inner)
            | Self::Vec(inner)
            | Self::Slice(inner)
            | Self::Seq(inner)
            | Self::Option(inner)
            | Self::ThreadHandle(inner)
            | Self::Shared(inner)
            | Self::BorrowRef { inner, .. } => inner.contains_dialogue_line_operation(),
            Self::IteratorState { item, .. } | Self::Array { item, .. } => {
                item.contains_dialogue_line_operation()
            }
            Self::Map { key, value, .. } => {
                key.contains_dialogue_line_operation() || value.contains_dialogue_line_operation()
            }
            Self::Need { ready, error } => {
                ready.contains_dialogue_line_operation() || error.contains_dialogue_line_operation()
            }
            Self::Stream { item, error } => {
                item.contains_dialogue_line_operation() || error.contains_dialogue_line_operation()
            }
            Self::Result { ok, error } => {
                ok.contains_dialogue_line_operation() || error.contains_dialogue_line_operation()
            }
            Self::Function {
                params,
                return_type,
                ..
            } => {
                params.iter().any(Self::contains_dialogue_line_operation)
                    || return_type.contains_dialogue_line_operation()
            }
            Self::ProjectNominal(nominal) => arguments_contain_dialogue_line(nominal.arguments()),
            Self::AcceptedNominal(nominal) => arguments_contain_dialogue_line(nominal.arguments()),
            Self::OpenNominal(nominal) => arguments_contain_dialogue_line(nominal.arguments()),
            Self::Projection { subject, .. } => subject.contains_dialogue_line_operation(),
            Self::Tuple(items) | Self::Choice(items) => {
                items.iter().any(Self::contains_dialogue_line_operation)
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
            | Self::AgentBuiltin(_)
            | Self::GenericParam(_)
            | Self::Handle { .. }
            | Self::CharacterPatch(_)
            | Self::FocusPatch
            | Self::CharacterDialogue(_)
            | Self::CharacterNominal(_)
            | Self::Named(_)
            | Self::Error(_)
            | Self::Unit
            | Self::Never => false,
        }
    }

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
            | Self::DialogueLine(inner)
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
            Self::Stream { item, error } => {
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
            | Self::AgentBuiltin(_)
            | Self::GenericParam(_)
            | Self::Handle { .. }
            | Self::CharacterPatch(_)
            | Self::FocusPatch
            | Self::CharacterDialogue(_)
            | Self::CharacterNominal(_)
            | Self::Named(_)
            | Self::Unit
            | Self::Never => false,
        }
    }
}

fn arguments_contain_nominal_poison(arguments: &[TypeKind]) -> bool {
    arguments.iter().any(TypeKind::contains_nominal_poison)
}

fn arguments_contain_dialogue_line(arguments: &[TypeKind]) -> bool {
    arguments
        .iter()
        .any(TypeKind::contains_dialogue_line_operation)
}
