//! Stateful implementation of the single recursive final-HIR nominal resolver.

mod resolution;
mod state;
mod support;
mod traversal;

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    identity::TypeId,
    leaf::HirPath,
    source_index::{HirSourceSite, HirTypeSourceRole},
    symbol::nominal::ProjectNominalDeclarationId,
    type_ref::HirTypeKind,
};
use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::SourceRange;

use crate::types::{EntityKind, GenericTypeParameterId, TypeKind, TypePoisonId};

use super::super::{
    AliasExpansionFact, AssociatedTypeScope, BuiltinTypeConstructor, DetachedNominalEvidence,
    DetachedNominalReason, DetachedTypeRef, ExternalNominalResolution, GenericTypeScope,
    NominalDiagnosticRelated, NominalRelatedMessage, NominalResolutionLimitKind,
    NominalTypeDiagnostic, NominalTypeDiagnosticKind, PoisonedTypeRef, ResolvedAliasReference,
    ResolvedOpenNominal, ResolvedTypeNode, ResolvedTypeProduct, ResolvedTypeRefOutcome,
    SelfTypeScope, StructuralTypeNodeKind, TypeArgumentExpectation, TypeArgumentKind,
    TypeArityExpectation, TypeArityTarget, TypeNameResolution, TypePoisonOrigin, TypePoisonRecord,
    TypeResolutionFailure, TypeResolutionInput, TypeResolutionInputError, TypeResolutionModule,
    TypeResolutionReport, TypeResolutionWorld, TypeSourceEvidence,
};
use support::{
    ProjectNameLookup, ProjectSelection, canonical_cycle, canonical_poisons, diagnostic_kind,
    diagnostic_ordering, direct_name, evidence_from_project, hir_path_matches_type_path,
    open_expectation, open_rule_matches_hir, related_ordering,
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
    unavailable: Vec<TypeId>,
    reserved_poison_indices: BTreeSet<u32>,
    next_poison_index: u32,
    type_nodes: u64,
    alias_nodes: u64,
    work: u64,
    global_halt: Option<TypePoisonId>,
    alias_stack: Vec<ProjectNominalDeclarationId>,
}

struct SourceContext<'a> {
    module: TypeResolutionModule<'a>,
    generics: GenericContext<'a>,
    associated: Option<AssociatedTypeScope>,
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

    fn argument_kind(&self) -> Option<TypeArgumentKind> {
        if let Some(family) = &self.entity_family {
            return Some(TypeArgumentKind::EntityFamily(family.clone()));
        }
        if let Some(value) = self.const_int {
            return Some(TypeArgumentKind::ConstInt(value));
        }
        match &self.ty {
            Some(TypeKind::Error(_)) | None => None,
            Some(ty) => Some(TypeArgumentKind::Type(ty.clone())),
        }
    }
}

impl SourceContext<'_> {
    fn evidence(&self, owner: TypeId, head: bool) -> TypeSourceEvidence {
        let role = if head {
            self.head_role(owner)
        } else {
            HirTypeSourceRole::Whole
        };
        self.evidence_for(owner, role)
            .unwrap_or_else(|| self.required_whole(owner))
    }

    fn terminal_evidence(&self, owner: TypeId) -> Option<TypeSourceEvidence> {
        let ty = self
            .module
            .resolve_type(owner)
            .expect("validated final-HIR type identity remains live");
        let role = match ty.kind() {
            HirTypeKind::Path(path) => HirTypeSourceRole::PathSegment {
                ordinal: u32::try_from(path.segments().len().saturating_sub(1)).ok()?,
            },
            HirTypeKind::Generic(_) => HirTypeSourceRole::GenericBase,
            HirTypeKind::TraitBound(_) => HirTypeSourceRole::TraitBase,
            HirTypeKind::Projection(_) => HirTypeSourceRole::ProjectionName,
            _ => return None,
        };
        self.evidence_for(owner, role)
    }

    fn reference_path(&self, owner: TypeId) -> Option<HirPath> {
        let ty = self.module.resolve_type(owner).ok()?;
        match ty.kind() {
            HirTypeKind::Path(path) => Some(path.clone()),
            HirTypeKind::Generic(generic) => Some(generic.base().clone()),
            HirTypeKind::TraitBound(bound) => Some(bound.base().clone()),
            _ => None,
        }
    }

    fn generic(&self, path: &HirPath) -> Option<(GenericTypeParameterId, TypeKind)> {
        let name = direct_name(path)?;
        let name = ModuleSegment::new(name).ok()?;
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

    fn head_role(&self, owner: TypeId) -> HirTypeSourceRole {
        match self
            .module
            .resolve_type(owner)
            .expect("validated final-HIR type identity remains live")
            .kind()
        {
            HirTypeKind::ConstInt(_) => HirTypeSourceRole::ConstInteger,
            HirTypeKind::Path(path) => HirTypeSourceRole::PathSegment {
                ordinal: u32::try_from(path.segments().len().saturating_sub(1))
                    .expect("HIR path segment limits fit u32"),
            },
            HirTypeKind::Generic(_) => HirTypeSourceRole::GenericBase,
            HirTypeKind::TraitBound(_) => HirTypeSourceRole::TraitBase,
            HirTypeKind::Projection(_) => HirTypeSourceRole::ProjectionName,
            HirTypeKind::Never => HirTypeSourceRole::NeverMarker,
            _ => HirTypeSourceRole::Whole,
        }
    }

    fn evidence_for(&self, owner: TypeId, role: HirTypeSourceRole) -> Option<TypeSourceEvidence> {
        match self.module.type_source_site(owner, role)? {
            HirSourceSite::Span(span) => {
                Some(TypeSourceEvidence::accepted(span.range(), span.clone()))
            }
            HirSourceSite::Insertion(insertion) => Some(TypeSourceEvidence::detached(
                SourceRange::new(insertion.offset(), insertion.offset()),
            )),
        }
    }

    fn required_whole(&self, owner: TypeId) -> TypeSourceEvidence {
        self.evidence_for(owner, HirTypeSourceRole::Whole)
            .expect("every final-HIR type owner retains its required whole source role")
    }
}

impl<'input, 'world> Resolver<'input, 'world> {
    fn new(input: &'input TypeResolutionInput<'world>) -> Self {
        let mut reserved_poison_indices = BTreeSet::new();
        if let SelfTypeScope::Poisoned(poison) = input.self_scope() {
            reserved_poison_indices.insert(poison.index());
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
        let root_context = SourceContext {
            module: self.input.module(),
            generics: GenericContext::Input(self.input.generics()),
            associated: self.input.associated().cloned(),
            alias_target: false,
        };
        let root = self.input.root();
        let value = self.resolve_node(&root_context, root, 1)?;
        let recovered = value.ty.unwrap_or_else(|| {
            value
                .causes
                .first()
                .copied()
                .map_or(TypeKind::Unit, TypeKind::Error)
        });
        let product = ResolvedTypeProduct::new(root, recovered, self.nodes, self.aliases);
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
