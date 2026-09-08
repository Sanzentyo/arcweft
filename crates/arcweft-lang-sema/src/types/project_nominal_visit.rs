//! Exhaustive traversal of project-nominal occurrences in semantic types.

use super::{ProjectNominalType, ScopedTypeView, TypeKind};

/// Visits every project nominal and all nested type arguments in source-owned
/// semantic order. The caller supplies only the operation performed at an
/// accepted nominal occurrence; recursion remains owned by the `TypeKind`
/// algebra so new constructors cannot silently escape the C2 inventory.
pub(crate) fn visit_project_nominals<E>(
    ty: ScopedTypeView<'_>,
    visitor: &mut impl FnMut(ScopedTypeView<'_>, &ProjectNominalType) -> Result<(), E>,
) -> Result<(), E> {
    let scope = ty.scope();
    match ty.value() {
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
        | TypeKind::StatementIngress(_)
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
        | TypeKind::CompileTimeCallable(_)
        | TypeKind::CompileTimeScalar(_)
        | TypeKind::CompileTimeEnum(_)
        | TypeKind::CompileTimeFx(_)
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
        | TypeKind::Projection { subject: inner, .. }
        | TypeKind::MetaType(inner) => {
            visit_project_nominals(ScopedTypeView::sealed(inner, scope), visitor)
        }
        TypeKind::FixedVector(vector) => {
            visit_project_nominals(ScopedTypeView::sealed(vector.component(), scope), visitor)
        }
        TypeKind::IteratorState { item, .. } | TypeKind::Array { item, .. } => {
            visit_project_nominals(ScopedTypeView::sealed(item, scope), visitor)
        }
        TypeKind::Ref(entity) => entity.value().map_or(Ok(()), |value| {
            visit_project_nominals(ScopedTypeView::sealed(value, scope), visitor)
        }),
        TypeKind::Map { key, value, .. } => {
            visit_project_nominals(ScopedTypeView::sealed(key, scope), visitor)?;
            visit_project_nominals(ScopedTypeView::sealed(value, scope), visitor)
        }
        TypeKind::Stream { item, error }
        | TypeKind::Parser { item, error }
        | TypeKind::Result { ok: item, error } => {
            visit_project_nominals(ScopedTypeView::sealed(item, scope), visitor)?;
            visit_project_nominals(ScopedTypeView::sealed(error, scope), visitor)
        }
        TypeKind::Function {
            binder,
            params,
            return_type,
            ..
        } => {
            let nested_scope = scope.with_binder(*binder);
            for parameter in params {
                visit_project_nominals(ScopedTypeView::sealed(parameter, &nested_scope), visitor)?;
            }
            visit_project_nominals(ScopedTypeView::sealed(return_type, &nested_scope), visitor)
        }
        TypeKind::ProjectNominal(nominal) => {
            visitor(ty, nominal)?;
            for argument in nominal.arguments() {
                visit_project_nominals(ScopedTypeView::sealed(argument, scope), visitor)?;
            }
            Ok(())
        }
        TypeKind::AcceptedNominal(nominal) => {
            for argument in nominal.arguments() {
                visit_project_nominals(ScopedTypeView::sealed(argument, scope), visitor)?;
            }
            Ok(())
        }
        TypeKind::OpenNominal(nominal) => {
            for argument in nominal.arguments() {
                visit_project_nominals(ScopedTypeView::sealed(argument, scope), visitor)?;
            }
            Ok(())
        }
        TypeKind::Tuple(items) | TypeKind::Choice(items) => {
            for item in items {
                visit_project_nominals(ScopedTypeView::sealed(item, scope), visitor)?;
            }
            Ok(())
        }
        TypeKind::VariantPayload(payload) => payload.visit_types(&mut |field| {
            visit_project_nominals(ScopedTypeView::sealed(field, scope), visitor)
        }),
    }
}
