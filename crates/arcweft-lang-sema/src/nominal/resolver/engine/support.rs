use std::collections::BTreeSet;

use arcweft_lang_hir::symbol::{
    ExternalSymbol,
    nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationId},
};
use arcweft_lang_syntax::{
    ast::{common::TextRange, module_path::ModulePathRoot},
    types::{TypePath, TypeRef},
};
use arcweft_source::SourceSpan;

use crate::{env::nominal::OpenNominalArity, types::TypePoisonId};

use super::{
    NameResult, NominalDiagnosticRelated, NominalTypeDiagnostic, NominalTypeDiagnosticKind,
    TypeArityExpectation, TypeResolutionFailure, TypeSourceEvidence,
};

pub(super) enum ProjectSelection {
    Nominal(Box<ProjectNominalDeclaration>),
    External(Box<ExternalSymbol>),
}

pub(super) enum ProjectNameLookup {
    Absent,
    Selected(ProjectSelection),
    Failed(Box<NameResult>),
}

pub(super) fn direct_name(path: &TypePath) -> Option<&str> {
    direct_segment(path).map(arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment::as_str)
}

pub(super) fn direct_segment(
    path: &TypePath,
) -> Option<&arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment> {
    (path.root() == ModulePathRoot::ImplicitCrate && path.segments().len() == 1)
        .then(|| &path.segments()[0])
}

pub(super) fn open_expectation(arity: OpenNominalArity) -> TypeArityExpectation {
    match arity {
        OpenNominalArity::Exact(exact) => TypeArityExpectation::Exact(exact),
        OpenNominalArity::Inclusive { minimum, maximum } => {
            TypeArityExpectation::Inclusive { minimum, maximum }
        }
    }
}

pub(super) fn diagnostic_kind(failure: &TypeResolutionFailure) -> NominalTypeDiagnosticKind {
    match failure {
        TypeResolutionFailure::Unknown { path } => {
            NominalTypeDiagnosticKind::Unknown { path: path.clone() }
        }
        TypeResolutionFailure::Ambiguous { path, candidates } => {
            NominalTypeDiagnosticKind::Ambiguous {
                path: path.clone(),
                candidates: candidates.clone(),
            }
        }
        TypeResolutionFailure::Inaccessible { path, candidates } => {
            NominalTypeDiagnosticKind::Inaccessible {
                path: path.clone(),
                candidates: candidates.clone(),
            }
        }
        TypeResolutionFailure::WrongKind { path, actual } => NominalTypeDiagnosticKind::WrongKind {
            path: path.clone(),
            actual: actual.clone(),
        },
        TypeResolutionFailure::WrongArgumentKind {
            target,
            argument,
            expected,
            actual,
        } => NominalTypeDiagnosticKind::WrongArgumentKind {
            target: target.clone(),
            argument: *argument,
            expected: *expected,
            actual: actual.clone(),
        },
        TypeResolutionFailure::WrongArity {
            target,
            expected,
            actual,
        } => NominalTypeDiagnosticKind::WrongArity {
            target: target.clone(),
            expected: *expected,
            actual: *actual,
        },
        TypeResolutionFailure::CyclicAlias { cycle } => NominalTypeDiagnosticKind::CyclicAlias {
            cycle: cycle.clone(),
        },
        TypeResolutionFailure::SelfUnavailable => NominalTypeDiagnosticKind::SelfUnavailable,
        TypeResolutionFailure::Limit {
            kind,
            observed,
            maximum,
        } => NominalTypeDiagnosticKind::Limit {
            kind: *kind,
            observed: *observed,
            maximum: *maximum,
        },
        TypeResolutionFailure::WorkOverflow { attempted, maximum } => {
            NominalTypeDiagnosticKind::WorkOverflow {
                attempted: *attempted,
                maximum: *maximum,
            }
        }
    }
}

pub(super) fn collect_recovery_poisons(value: &TypeRef, output: &mut BTreeSet<u32>) {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            TypeRef::Recovery(recovery) => {
                output.insert(recovery.index());
            }
            TypeRef::Tuple(items) | TypeRef::Choice(items) => {
                pending.extend(items.iter().rev());
            }
            TypeRef::Function {
                params,
                return_type,
                ..
            } => {
                pending.push(return_type);
                pending.extend(params.iter().rev());
            }
            TypeRef::Generic { args, .. } => pending.extend(args.iter().rev()),
            TypeRef::TraitBound(bound) => {
                pending.extend(
                    bound
                        .associated()
                        .iter()
                        .rev()
                        .map(arcweft_lang_syntax::types::AssociatedTypeBinding::value),
                );
                pending.extend(bound.args().iter().rev());
            }
            TypeRef::Projection { subject, .. } | TypeRef::Slice(subject) => {
                pending.push(subject);
            }
            TypeRef::Reference(reference) => pending.push(reference.referent()),
            TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) => {}
        }
    }
}

pub(super) fn canonical_cycle(
    mut cycle: Vec<ProjectNominalDeclarationId>,
) -> Box<[ProjectNominalDeclarationId]> {
    if let Some((position, _)) = cycle.iter().enumerate().min_by_key(|(_, id)| *id) {
        cycle.rotate_left(position);
    }
    cycle.into_boxed_slice()
}

pub(super) fn canonical_poisons(
    poisons: impl IntoIterator<Item = TypePoisonId>,
) -> Vec<TypePoisonId> {
    let mut poisons = poisons.into_iter().collect::<Vec<_>>();
    poisons.sort_unstable();
    poisons.dedup();
    poisons
}

pub(super) fn evidence_from_project(source: SourceSpan) -> TypeSourceEvidence {
    TypeSourceEvidence::accepted(
        TextRange::new(source.range().start(), source.range().end()),
        source,
    )
}

pub(super) fn diagnostic_ordering(
    left: &NominalTypeDiagnostic,
    right: &NominalTypeDiagnostic,
) -> core::cmp::Ordering {
    evidence_ordering(left.primary(), right.primary()).then_with(|| left.kind().cmp(right.kind()))
}

pub(super) fn related_ordering(
    left: &NominalDiagnosticRelated,
    right: &NominalDiagnosticRelated,
) -> core::cmp::Ordering {
    evidence_ordering(left.source(), right.source())
        .then_with(|| left.message().cmp(&right.message()))
}

fn evidence_ordering(left: &TypeSourceEvidence, right: &TypeSourceEvidence) -> core::cmp::Ordering {
    left.project()
        .cmp(&right.project())
        .then_with(|| left.local().cmp(&right.local()))
}
