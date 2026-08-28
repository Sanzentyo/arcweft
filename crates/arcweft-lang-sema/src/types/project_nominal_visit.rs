//! Exhaustive traversal of project-nominal occurrences in semantic types.

use super::{ProjectNominalType, TypeKind};

/// Visits every project nominal and all nested type arguments in source-owned
/// semantic order. The caller supplies only the operation performed at an
/// accepted nominal occurrence; recursion remains owned by the `TypeKind`
/// algebra so new constructors cannot silently escape the C2 inventory.
pub(crate) fn visit_project_nominals<E>(
    ty: &TypeKind,
    visitor: &mut impl FnMut(&ProjectNominalType) -> Result<(), E>,
) -> Result<(), E> {
    match ty {
        TypeKind::Bool
        | TypeKind::I8
        | TypeKind::I16
        | TypeKind::I32
        | TypeKind::I64
        | TypeKind::I128
        | TypeKind::ISize
        | TypeKind::U8
        | TypeKind::U16
        | TypeKind::U32
        | TypeKind::U64
        | TypeKind::U128
        | TypeKind::USize
        | TypeKind::F32
        | TypeKind::F64
        | TypeKind::String
        | TypeKind::Char
        | TypeKind::Bytes
        | TypeKind::TextCluster
        | TypeKind::Duration
        | TypeKind::Progress
        | TypeKind::StageApi(_)
        | TypeKind::LineContext
        | TypeKind::StageActorHandle(_)
        | TypeKind::CueHandle
        | TypeKind::VoiceHandle
        | TypeKind::DisplayText
        | TypeKind::DebugStatePath
        | TypeKind::ObservationFieldPath
        | TypeKind::Predicate
        | TypeKind::Observation
        | TypeKind::ObservedObject
        | TypeKind::AgentBBox
        | TypeKind::ActionName
        | TypeKind::ActionTarget
        | TypeKind::ActionResult
        | TypeKind::AgentValue
        | TypeKind::DataFormat
        | TypeKind::DataShape
        | TypeKind::AgentEntityMetadata
        | TypeKind::AgentSourceAnchor
        | TypeKind::AgentProjectGraphNeighborhood
        | TypeKind::AgentProjectGraphSymbol
        | TypeKind::AgentProjectGraphEdge
        | TypeKind::CaptureTarget
        | TypeKind::CaptureRef
        | TypeKind::AgentResource
        | TypeKind::AgentResourceBody
        | TypeKind::RagContextPack
        | TypeKind::AgentBuiltin(_)
        | TypeKind::Handle { .. }
        | TypeKind::GenericParam(_)
        | TypeKind::Error(_)
        | TypeKind::CharacterPatch(_)
        | TypeKind::FocusPatch
        | TypeKind::CharacterDialogue(_)
        | TypeKind::ViewValue
        | TypeKind::CharacterNominal(_)
        | TypeKind::Named(_)
        | TypeKind::Unit
        | TypeKind::Never => Ok(()),
        TypeKind::Range(inner)
        | TypeKind::Probe(inner)
        | TypeKind::Vec(inner)
        | TypeKind::Slice(inner)
        | TypeKind::Seq(inner)
        | TypeKind::Need(inner)
        | TypeKind::Option(inner)
        | TypeKind::ThreadHandle(inner)
        | TypeKind::Shared(inner)
        | TypeKind::DialogueLine(inner)
        | TypeKind::BorrowRef { inner, .. }
        | TypeKind::Projection { subject: inner, .. } => visit_project_nominals(inner, visitor),
        TypeKind::IteratorState { item, .. } | TypeKind::Array { item, .. } => {
            visit_project_nominals(item, visitor)
        }
        TypeKind::Ref(entity) => entity
            .value()
            .map_or(Ok(()), |value| visit_project_nominals(value, visitor)),
        TypeKind::Map { key, value, .. } => {
            visit_project_nominals(key, visitor)?;
            visit_project_nominals(value, visitor)
        }
        TypeKind::Stream { item, error }
        | TypeKind::Parser { item, error }
        | TypeKind::Result { ok: item, error } => {
            visit_project_nominals(item, visitor)?;
            visit_project_nominals(error, visitor)
        }
        TypeKind::Function {
            params,
            return_type,
            ..
        } => {
            for parameter in params {
                visit_project_nominals(parameter, visitor)?;
            }
            visit_project_nominals(return_type, visitor)
        }
        TypeKind::ProjectNominal(nominal) => {
            visitor(nominal)?;
            for argument in nominal.arguments() {
                visit_project_nominals(argument, visitor)?;
            }
            Ok(())
        }
        TypeKind::AcceptedNominal(nominal) => {
            for argument in nominal.arguments() {
                visit_project_nominals(argument, visitor)?;
            }
            Ok(())
        }
        TypeKind::OpenNominal(nominal) => {
            for argument in nominal.arguments() {
                visit_project_nominals(argument, visitor)?;
            }
            Ok(())
        }
        TypeKind::Tuple(items) | TypeKind::Choice(items) => {
            for item in items {
                visit_project_nominals(item, visitor)?;
            }
            Ok(())
        }
        TypeKind::VariantPayload(payload) => payload
            .shape()
            .visit_types(&mut |field| visit_project_nominals(field, visitor)),
    }
}
