//! Stateful implementation of the single recursive nominal resolver.

mod resolution;
mod state;
mod support;
mod traversal;

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::symbol::nominal::{
    ProjectNominalBody, ProjectNominalDeclarationId, SourceBackedTypeRef,
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    types::{
        AuthoredTypeRef, TypePath, TypeRef, TypeRefNodePath, TypeRefNodeStep, TypeRefSourceMap,
    },
};
use arcweft_source::SourceSpan;

use crate::types::{EntityKind, GenericTypeParameterId, TypeKind, TypePoisonId};

use super::super::{
    AliasExpansionFact, BuiltinTypeConstructor, DetachedNominalEvidence, DetachedNominalReason,
    DetachedTypeRef, ExternalNominalResolution, GenericTypeScope, NominalDiagnosticRelated,
    NominalRelatedMessage, NominalResolutionLimitKind, NominalTypeDiagnostic,
    NominalTypeDiagnosticKind, PoisonedTypeRef, ResolvedAliasReference, ResolvedOpenNominal,
    ResolvedTypeNode, ResolvedTypeProduct, ResolvedTypeRefOutcome, SelfTypeScope,
    StructuralTypeNodeKind, TypeArgumentExpectation, TypeArityExpectation, TypeArityTarget,
    TypeNameResolution, TypePoisonOrigin, TypePoisonRecord, TypeResolutionFailure,
    TypeResolutionInput, TypeResolutionInputError, TypeResolutionReport, TypeResolutionWorld,
    TypeSourceEvidence,
};
use support::{
    ProjectNameLookup, ProjectSelection, builtin, canonical_cycle, canonical_poisons,
    collect_recovery_poisons, diagnostic_kind, diagnostic_ordering, direct_name, direct_segment,
    evidence_from_project, open_expectation, related_ordering,
};

pub(super) fn resolve_type_ref(
    input: &TypeResolutionInput<'_>,
) -> Result<TypeResolutionReport, TypeResolutionInputError> {
    Resolver::new(input).resolve()
}

struct Resolver<'input, 'world> {
    input: &'input TypeResolutionInput<'world>,
    nodes: Vec<ResolvedTypeNode>,
    aliases: Vec<AliasExpansionFact>,
    diagnostics: Vec<NominalTypeDiagnostic>,
    poisons: Vec<TypePoisonRecord>,
    unavailable: Vec<TypeRefNodePath>,
    reserved_poison_indices: BTreeSet<u32>,
    next_poison_index: u32,
    type_nodes: u64,
    alias_nodes: u64,
    work: u64,
    global_halt: Option<TypePoisonId>,
    alias_stack: Vec<ProjectNominalDeclarationId>,
}

struct SourceContext<'a> {
    authored: &'a AuthoredTypeRef,
    project: Option<&'a TypeRefSourceMap<SourceSpan>>,
    module: Option<&'a CanonicalModulePath>,
    generics: GenericContext<'a>,
    alias_target: bool,
}

enum GenericContext<'a> {
    Input(&'a GenericTypeScope),
    Alias(&'a BTreeMap<ModuleSegment, AliasBinding>),
}

struct AliasBinding {
    id: GenericTypeParameterId,
    value: TypeKind,
}

struct NodeValue {
    ty: Option<TypeKind>,
    const_int: Option<usize>,
    entity_family: Option<EntityKind>,
    causes: Vec<TypePoisonId>,
}

struct NameResult {
    value: NodeValue,
    outcome: TypeNameResolution,
}

impl NodeValue {
    fn typed(ty: TypeKind, causes: impl IntoIterator<Item = TypePoisonId>) -> Self {
        Self {
            ty: Some(ty),
            const_int: None,
            entity_family: None,
            causes: canonical_poisons(causes),
        }
    }

    fn constant(value: usize) -> Self {
        Self {
            ty: None,
            const_int: Some(value),
            entity_family: None,
            causes: Vec::new(),
        }
    }

    fn entity_family(value: EntityKind) -> Self {
        Self {
            ty: None,
            const_int: None,
            entity_family: Some(value),
            causes: Vec::new(),
        }
    }

    fn error(poison: TypePoisonId, causes: impl IntoIterator<Item = TypePoisonId>) -> Self {
        Self::typed(TypeKind::Error(poison), causes.into_iter().chain([poison]))
    }

    fn recovered_or(&self, fallback: TypeKind) -> TypeKind {
        self.ty.clone().unwrap_or(fallback)
    }
}

impl SourceContext<'_> {
    fn child_path(&self, parent: &TypeRefNodePath, step: TypeRefNodeStep) -> TypeRefNodePath {
        self.authored
            .source()
            .nodes()
            .iter()
            .find_map(|(candidate, _)| {
                let steps = candidate.steps();
                (steps.len() == parent.steps().len() + 1
                    && steps.starts_with(parent.steps())
                    && steps.last() == Some(&step))
                .then(|| candidate.clone())
            })
            .expect("validated authored source maps contain every typed child path")
    }

    fn evidence(&self, path: &TypeRefNodePath, head: bool) -> TypeSourceEvidence {
        let local = self
            .authored
            .source_at(path)
            .expect("validated authored source maps contain the visited node");
        let local = if head {
            local.head().map_or(*local.whole(), |head| *head.range())
        } else {
            *local.whole()
        };
        let project = self.project.map(|spans| {
            let source = spans
                .source_at(path)
                .expect("bound project maps preserve every typed node path");
            if head {
                source
                    .head()
                    .map_or_else(|| source.whole().clone(), |head| head.range().clone())
            } else {
                source.whole().clone()
            }
        });
        TypeSourceEvidence::new(local, project)
    }

    fn terminal_evidence(&self, path: &TypeRefNodePath) -> Option<TypeSourceEvidence> {
        let local = self.authored.source_at(path)?.head()?.terminal().copied()?;
        let project = self.project.map(|spans| {
            spans
                .source_at(path)
                .expect("bound project maps preserve every typed node path")
                .head()
                .and_then(|head| head.terminal())
                .expect("bound project path heads preserve their terminal segment")
                .clone()
        });
        Some(TypeSourceEvidence::new(local, project))
    }

    fn reference_path(&self, path: &TypeRefNodePath) -> Option<TypePath> {
        match self.authored.value_at(path)? {
            TypeRef::Path(path) | TypeRef::Generic { base: path, .. } => Some(path.clone()),
            TypeRef::TraitBound(bound) => Some(bound.path().clone()),
            _ => None,
        }
    }

    fn generic(&self, path: &TypePath) -> Option<(GenericTypeParameterId, TypeKind)> {
        let name = direct_segment(path)?.try_as_module_segment().ok()?;
        match &self.generics {
            GenericContext::Input(scope) => scope.binding(&name).map(|binding| {
                (
                    binding.id().clone(),
                    TypeKind::GenericParam(binding.id().clone()),
                )
            }),
            GenericContext::Alias(bindings) => bindings
                .get(&name)
                .map(|binding| (binding.id.clone(), binding.value.clone())),
        }
    }
}

impl<'input, 'world> Resolver<'input, 'world> {
    fn new(input: &'input TypeResolutionInput<'world>) -> Self {
        let mut reserved_poison_indices = BTreeSet::new();
        collect_recovery_poisons(
            input.authored().authored().value(),
            &mut reserved_poison_indices,
        );
        if let SelfTypeScope::Poisoned(poison) = input.self_scope() {
            reserved_poison_indices.insert(poison.index());
        }
        if let Some(symbols) = input.world().symbols() {
            for declaration in symbols.nominal_symbols() {
                if let ProjectNominalBody::TypeAlias { target } = declaration.body() {
                    collect_recovery_poisons(
                        target.authored().value(),
                        &mut reserved_poison_indices,
                    );
                }
            }
        }
        Self {
            input,
            nodes: Vec::new(),
            aliases: Vec::new(),
            diagnostics: Vec::new(),
            poisons: Vec::new(),
            unavailable: Vec::new(),
            reserved_poison_indices,
            next_poison_index: 0,
            type_nodes: 0,
            alias_nodes: 0,
            work: 0,
            global_halt: None,
            alias_stack: Vec::new(),
        }
    }

    fn resolve(mut self) -> Result<TypeResolutionReport, TypeResolutionInputError> {
        let authored = self.input.authored();
        let root_context = SourceContext {
            authored: authored.authored(),
            project: authored.source_backed().map(SourceBackedTypeRef::spans),
            module: self.input.current_module(),
            generics: GenericContext::Input(self.input.generics()),
            alias_target: false,
        };
        let root = TypeRefNodePath::root();
        let value = self.resolve_node(&root_context, authored.authored().value(), &root, 1)?;
        let recovered = value.ty.unwrap_or_else(|| {
            value
                .causes
                .first()
                .copied()
                .map_or(TypeKind::Unit, TypeKind::Error)
        });
        let product = ResolvedTypeProduct::new(recovered, self.nodes, self.aliases);
        let outcome = if self.unavailable.is_empty() {
            if value.causes.is_empty() {
                ResolvedTypeRefOutcome::Complete(product)
            } else {
                ResolvedTypeRefOutcome::Poisoned(PoisonedTypeRef::new(product, value.causes))
            }
        } else {
            ResolvedTypeRefOutcome::Detached(DetachedTypeRef::new(
                product,
                self.unavailable,
                value.causes,
            ))
        };

        self.diagnostics.sort_by(diagnostic_ordering);
        self.diagnostics.dedup_by(|left, right| {
            left.kind() == right.kind() && left.primary() == right.primary()
        });
        let maximum = usize::from(self.input.limits().diagnostics_per_type_reference());
        let omitted_diagnostics = self.diagnostics.len().saturating_sub(maximum) as u64;
        self.diagnostics.truncate(maximum);
        self.poisons.sort_by_key(TypePoisonRecord::id);
        self.poisons.dedup_by_key(|record| record.id());
        Ok(TypeResolutionReport::new(
            outcome,
            self.diagnostics,
            self.poisons,
            omitted_diagnostics,
            self.work,
        ))
    }
}
