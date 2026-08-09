use arcweft_lang_hir::{
    leaf::{HirPath, HirPathRoot, HirPathSegment},
    symbol::{
        ExternalSymbol,
        nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationId},
    },
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModulePathRoot},
    types::TypePath,
};
use arcweft_source::SourceSpan;

use crate::{
    env::nominal::{
        OpenNominalArity, OpenNominalEnvironment, OpenNominalPattern, OpenNominalRule,
        OpenNominalScope,
    },
    types::TypePoisonId,
};

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

pub(super) fn direct_name(path: &HirPath) -> Option<&str> {
    if path.root() != HirPathRoot::ImplicitCrate {
        return None;
    }
    let [segment] = path.segments() else {
        return None;
    };
    Some(match segment {
        HirPathSegment::Identifier(name) => name.as_str(),
        HirPathSegment::ProjectSymbol(name) => name.as_str(),
    })
}

pub(super) fn hir_path_matches_type_path(actual: &HirPath, expected: &TypePath) -> bool {
    let root_matches = matches!(
        (actual.root(), expected.root()),
        (HirPathRoot::ImplicitCrate, ModulePathRoot::ImplicitCrate)
            | (HirPathRoot::Crate, ModulePathRoot::Crate)
            | (HirPathRoot::SelfModule, ModulePathRoot::SelfModule)
    ) || matches!(
        (actual.root(), expected.root()),
        (HirPathRoot::Super { depth: actual }, ModulePathRoot::Super(expected)) if actual == expected
    );
    root_matches
        && actual.segments().len() == expected.segments().len()
        && actual
            .segments()
            .iter()
            .zip(expected.segments())
            .all(|(actual, expected)| {
                let actual = match actual {
                    HirPathSegment::Identifier(name) => name.as_str(),
                    HirPathSegment::ProjectSymbol(name) => name.as_str(),
                };
                actual == expected.as_str()
            })
}

pub(super) fn open_rule_matches_hir(
    rule: &OpenNominalRule,
    environment: OpenNominalEnvironment,
    current_module: &CanonicalModulePath,
    path: &HirPath,
    arity: u16,
) -> bool {
    scope_matches(rule.scope(), environment, current_module)
        && pattern_matches(rule.pattern(), path)
        && rule.arity().contains(arity)
}

fn scope_matches(
    scope: &OpenNominalScope,
    environment: OpenNominalEnvironment,
    current_module: &CanonicalModulePath,
) -> bool {
    match scope {
        OpenNominalScope::AcceptedWorld => environment == OpenNominalEnvironment::Accepted,
        OpenNominalScope::DetachedOnly => environment == OpenNominalEnvironment::Detached,
        OpenNominalScope::ExactModule(module) => current_module == module,
        OpenNominalScope::ModuleSubtree(root) => {
            current_module.segments().starts_with(root.segments())
        }
    }
}

fn pattern_matches(pattern: &OpenNominalPattern, path: &HirPath) -> bool {
    match pattern {
        OpenNominalPattern::Exact(expected) => hir_path_matches_type_path(path, expected),
        OpenNominalPattern::Namespace {
            prefix,
            min_tail_segments,
            max_tail_segments,
        } => hir_path_tail_length(prefix, path).is_some_and(|tail| {
            usize::from(*min_tail_segments) <= tail && tail <= usize::from(*max_tail_segments)
        }),
    }
}

fn hir_path_tail_length(prefix: &TypePath, path: &HirPath) -> Option<usize> {
    let root_matches = matches!(
        (path.root(), prefix.root()),
        (HirPathRoot::ImplicitCrate, ModulePathRoot::ImplicitCrate)
            | (HirPathRoot::Crate, ModulePathRoot::Crate)
            | (HirPathRoot::SelfModule, ModulePathRoot::SelfModule)
    ) || matches!(
        (path.root(), prefix.root()),
        (HirPathRoot::Super { depth: actual }, ModulePathRoot::Super(expected)) if actual == expected
    );
    if !root_matches || path.segments().len() < prefix.segments().len() {
        return None;
    }
    let prefix_matches = path
        .segments()
        .iter()
        .zip(prefix.segments())
        .all(|(actual, expected)| {
            let actual = match actual {
                HirPathSegment::Identifier(name) => name.as_str(),
                HirPathSegment::ProjectSymbol(name) => name.as_str(),
            };
            actual == expected.as_str()
        });
    prefix_matches.then(|| path.segments().len() - prefix.segments().len())
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
    TypeSourceEvidence::accepted(source.range(), source)
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
