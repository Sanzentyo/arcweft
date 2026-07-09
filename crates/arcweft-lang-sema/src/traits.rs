//! Typed trait / impl / associated-type catalog for Arcweft DSL semantics.
//!
//! Seq08.1 keeps conformance evidence in semantic analysis. Parser and HIR
//! preserve syntax; later runtime-plan cuts consume typed witnesses instead of
//! rediscovering trait relationships from strings.

mod format;
mod standard_iter;

use crate::diagnostics::{TraitDiagnostic, TypeCheckError};
use crate::types::TypeKind;
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::flow::{AuthoredExpr, Stmt};
use arcweft_lang_syntax::ast::items::{ImplItem, ImplMember, TraitItem, TraitMember};
use arcweft_lang_syntax::types::{
    AssocTypeBinding, FnParam, FnSignature, GenericParam, TypeRef, parse_type_ref,
};
use format::{label_has_generic, type_head, type_kind_label};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Stable sema id for a trait declaration in one checked module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitId(usize);

/// Stable sema id for an impl declaration in one checked module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImplId(usize);

/// Stable sema id for an associated type requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssociatedTypeId(usize);

/// Stable sema id for proof that one type implements one trait.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitWitnessId(usize);

impl TraitId {
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

impl ImplId {
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

impl AssociatedTypeId {
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

impl TraitWitnessId {
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

impl TraitMethodBody {
    pub fn new(statements: &[Stmt], value: Option<&AuthoredExpr>) -> Option<Self> {
        (!statements.is_empty() || value.is_some()).then(|| Self {
            statements: statements.to_vec(),
            value: value.cloned(),
        })
    }

    pub fn statements(&self) -> &[Stmt] {
        &self.statements
    }

    pub const fn value(&self) -> Option<&AuthoredExpr> {
        self.value.as_ref()
    }

    pub const fn is_present(&self) -> bool {
        !self.statements.is_empty() || self.value.is_some()
    }
}

/// Complete trait and impl catalog built for one HIR module.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TraitCatalog {
    traits: Vec<TraitDecl>,
    impls: Vec<TraitImpl>,
    witnesses: Vec<TraitWitness>,
    by_name: BTreeMap<String, TraitId>,
    exact_impls: HashMap<(TraitId, TypeKind), ImplId>,
    inherent_methods: HashMap<(TypeKind, String), TraitMethodImpl>,
}

/// Trait declaration after member normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitDecl {
    id: TraitId,
    name: String,
    supertraits: Vec<TraitId>,
    associated_types: Vec<AssociatedTypeRequirement>,
    methods: Vec<TraitMethodRequirement>,
}

/// Required associated type declared by a trait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeRequirement {
    id: AssociatedTypeId,
    trait_id: TraitId,
    name: String,
}

/// Required method declared by a trait or inherited from a supertrait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodRequirement {
    trait_id: TraitId,
    name: String,
    signature: FnSignature,
}

/// Inherent or trait impl after semantic normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitImpl {
    id: ImplId,
    trait_id: Option<TraitId>,
    target: TypeKind,
    associated_types: BTreeMap<String, AssociatedTypeAssignment>,
    methods: BTreeMap<String, TraitMethodImpl>,
    witness: Option<TraitWitnessId>,
}

/// Predicate saying `subject: Trait<Assoc = Type>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitPredicate {
    subject: TypeKind,
    trait_id: TraitId,
    assoc_equalities: Vec<AssocEquality>,
}

/// Associated type equality inside a trait bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssocEquality {
    name: String,
    ty: TypeKind,
}

/// Associated type assignment inside a trait impl.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeAssignment {
    name: String,
    value: TypeKind,
}

/// Method implementation or requirement projection preserved for lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodImpl {
    trait_id: Option<TraitId>,
    signature: FnSignature,
    return_type: TypeKind,
    body: Option<TraitMethodBody>,
}

/// Syntax body retained for runtime lowering of executable impl methods.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodBody {
    statements: Vec<Stmt>,
    value: Option<AuthoredExpr>,
}

/// Typed conformance witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitWitness {
    id: TraitWitnessId,
    impl_id: ImplId,
    trait_id: TraitId,
    self_ty: TypeKind,
}

/// Concrete method selected through one trait witness.
#[derive(Clone, Debug)]
pub struct TraitMethodForWitness<'a> {
    impl_id: ImplId,
    trait_id: TraitId,
    witness: TraitWitnessId,
    self_ty: &'a TypeKind,
    method: &'a TraitMethodImpl,
}

/// Method lookup result through the trait catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraitMethodResolution {
    Missing,
    Inherent(TraitMethodImpl),
    Unique {
        witness: Option<TraitWitnessId>,
        trait_id: TraitId,
        method: TraitMethodImpl,
    },
    Ambiguous(Vec<TraitMethodCandidate>),
}

/// Candidate shown by an ambiguous method diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodCandidate {
    pub trait_id: TraitId,
    pub trait_name: String,
    pub witness: Option<TraitWitnessId>,
    pub method_name: String,
}

/// Projection resolution result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionResolution {
    Resolved(TypeKind),
    Deferred(TypeKind),
}

/// Projection resolution failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    UnknownAssociatedType { subject: TypeKind, assoc: String },
    Ambiguous { subject: TypeKind, assoc: String },
}

/// Resolved conformance evidence for one subject type and trait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitConformance {
    witness: Option<TraitWitnessId>,
    trait_id: TraitId,
    impl_id: Option<ImplId>,
    self_ty: TypeKind,
    associated_types: BTreeMap<String, TypeKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraitConformanceResolution {
    Missing,
    Unique(TraitConformance),
    Ambiguous(Vec<TraitConformance>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntoIteratorResolution {
    source_ty: TypeKind,
    item_ty: TypeKind,
    into_iter_ty: TypeKind,
    kind: IntoIteratorResolutionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntoIteratorResolutionKind {
    Explicit {
        into_iterator: TraitConformance,
        iterator: TraitConformance,
    },
    IteratorIdentity {
        iterator: TraitConformance,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntoIteratorResolutionError {
    MissingIntoIterator {
        source: Box<TypeKind>,
    },
    AmbiguousIntoIterator {
        source: Box<TypeKind>,
        candidates: Vec<TraitConformance>,
    },
    MissingIteratorForIntoIter {
        source: Box<TypeKind>,
        into_iter: Box<TypeKind>,
        item: Box<TypeKind>,
    },
    AmbiguousIteratorForIntoIter {
        source: Box<TypeKind>,
        into_iter: Box<TypeKind>,
        candidates: Vec<TraitConformance>,
    },
}

impl TraitCatalog {
    pub fn traits(&self) -> &[TraitDecl] {
        &self.traits
    }

    pub fn impls(&self) -> &[TraitImpl] {
        &self.impls
    }

    pub fn witnesses(&self) -> &[TraitWitness] {
        &self.witnesses
    }

    pub fn trait_impl(&self, id: ImplId) -> Option<&TraitImpl> {
        self.impls.get(id.index())
    }

    pub fn witness(&self, id: TraitWitnessId) -> Option<&TraitWitness> {
        self.witnesses.get(id.index())
    }

    pub fn trait_id(&self, name: &str) -> Option<TraitId> {
        self.by_name.get(name).copied()
    }

    pub fn trait_decl(&self, id: TraitId) -> Option<&TraitDecl> {
        self.traits.get(id.index())
    }

    pub fn trait_name(&self, id: TraitId) -> Option<&str> {
        self.trait_decl(id).map(TraitDecl::name)
    }

    pub fn witness_method(
        &self,
        witness: TraitWitnessId,
        method_name: &str,
    ) -> Option<TraitMethodForWitness<'_>> {
        let witness_decl = self.witness(witness)?;
        let impl_decl = self.trait_impl(witness_decl.impl_id)?;
        let method = impl_decl.methods.get(method_name)?;
        Some(TraitMethodForWitness {
            impl_id: impl_decl.id,
            trait_id: witness_decl.trait_id,
            witness,
            self_ty: &witness_decl.self_ty,
            method,
        })
    }

    pub fn resolve_into_iterator(
        &self,
        source: &TypeKind,
        predicates: &[TraitPredicate],
    ) -> Result<IntoIteratorResolution, IntoIteratorResolutionError> {
        let into_iterator = match self.resolve_trait_conformance_by_name(
            source,
            standard_iter::INTO_ITERATOR,
            &[],
            predicates,
        ) {
            TraitConformanceResolution::Missing => {
                return self.resolve_iterator_identity_into_iterator(source, predicates);
            }
            TraitConformanceResolution::Ambiguous(candidates) => {
                return Err(IntoIteratorResolutionError::AmbiguousIntoIterator {
                    source: Box::new(source.clone()),
                    candidates,
                });
            }
            TraitConformanceResolution::Unique(conformance) => conformance,
        };
        let item_ty = into_iterator
            .associated_type(standard_iter::ITEM)
            .cloned()
            .unwrap_or_else(|| TypeKind::Named("_".to_owned()));
        let into_iter_ty = into_iterator
            .associated_type(standard_iter::INTO_ITER)
            .cloned()
            .unwrap_or_else(|| TypeKind::Named("_".to_owned()));
        let item_eq = [AssocEquality::new(standard_iter::ITEM, item_ty.clone())];
        let iterator = match self.resolve_trait_conformance_by_name(
            &into_iter_ty,
            standard_iter::ITERATOR,
            &item_eq,
            predicates,
        ) {
            TraitConformanceResolution::Missing => {
                return Err(IntoIteratorResolutionError::MissingIteratorForIntoIter {
                    source: Box::new(source.clone()),
                    into_iter: Box::new(into_iter_ty),
                    item: Box::new(item_ty),
                });
            }
            TraitConformanceResolution::Ambiguous(candidates) => {
                return Err(IntoIteratorResolutionError::AmbiguousIteratorForIntoIter {
                    source: Box::new(source.clone()),
                    into_iter: Box::new(into_iter_ty),
                    candidates,
                });
            }
            TraitConformanceResolution::Unique(conformance) => conformance,
        };
        Ok(IntoIteratorResolution {
            source_ty: source.clone(),
            item_ty,
            into_iter_ty,
            kind: IntoIteratorResolutionKind::Explicit {
                into_iterator,
                iterator,
            },
        })
    }

    fn resolve_iterator_identity_into_iterator(
        &self,
        source: &TypeKind,
        predicates: &[TraitPredicate],
    ) -> Result<IntoIteratorResolution, IntoIteratorResolutionError> {
        let iterator = match self.resolve_trait_conformance_by_name(
            source,
            standard_iter::ITERATOR,
            &[],
            predicates,
        ) {
            TraitConformanceResolution::Missing => {
                return Err(IntoIteratorResolutionError::MissingIntoIterator {
                    source: Box::new(source.clone()),
                });
            }
            TraitConformanceResolution::Ambiguous(candidates) => {
                return Err(IntoIteratorResolutionError::AmbiguousIteratorForIntoIter {
                    source: Box::new(source.clone()),
                    into_iter: Box::new(source.clone()),
                    candidates,
                });
            }
            TraitConformanceResolution::Unique(conformance) => conformance,
        };
        let item_ty = iterator
            .associated_type(standard_iter::ITEM)
            .cloned()
            .unwrap_or_else(|| TypeKind::Named("_".to_owned()));
        Ok(IntoIteratorResolution {
            source_ty: source.clone(),
            item_ty,
            into_iter_ty: source.clone(),
            kind: IntoIteratorResolutionKind::IteratorIdentity { iterator },
        })
    }

    pub fn resolve_trait_conformance_by_name(
        &self,
        subject: &TypeKind,
        trait_name: &str,
        assoc_equalities: &[AssocEquality],
        predicates: &[TraitPredicate],
    ) -> TraitConformanceResolution {
        let Some(trait_id) = self.trait_id(trait_name) else {
            return TraitConformanceResolution::Missing;
        };
        let mut candidates = self
            .impls
            .iter()
            .filter(|impl_decl| impl_decl.trait_id == Some(trait_id))
            .filter_map(|impl_decl| conformance_from_impl(impl_decl, subject, assoc_equalities))
            .collect::<Vec<_>>();
        candidates.extend(predicates.iter().filter_map(|predicate| {
            conformance_from_predicate(predicate, subject, trait_id, assoc_equalities)
        }));
        match candidates.as_slice() {
            [] => TraitConformanceResolution::Missing,
            [candidate] => TraitConformanceResolution::Unique(candidate.clone()),
            _ => TraitConformanceResolution::Ambiguous(candidates),
        }
    }

    pub fn predicates_for_signature(&self, signature: &FnSignature) -> Vec<TraitPredicate> {
        let generic_bounds = signature
            .generic_params()
            .iter()
            .flat_map(|param| match param {
                GenericParam::Lifetime(_) => Vec::new(),
                GenericParam::Type(param) => param
                    .bounds()
                    .iter()
                    .filter_map(|bound| {
                        self.predicate_from_bound(
                            TypeKind::GenericParam(param.name().to_owned()),
                            bound,
                        )
                    })
                    .collect::<Vec<_>>(),
            });
        let where_bounds = signature.where_clauses().iter().flat_map(|clause| {
            let subject = trait_type_ref_kind(clause.subject(), &HashSet::new());
            clause
                .bounds()
                .iter()
                .filter_map(move |bound| self.predicate_from_bound(subject.clone(), bound))
                .collect::<Vec<_>>()
        });
        generic_bounds.chain(where_bounds).collect()
    }

    pub fn resolve_method(
        &self,
        receiver: &TypeKind,
        method_name: &str,
        predicates: &[TraitPredicate],
    ) -> TraitMethodResolution {
        if let Some(method) = self
            .inherent_methods
            .get(&(receiver.clone(), method_name.to_owned()))
            .cloned()
        {
            return TraitMethodResolution::Inherent(method);
        }

        let mut candidates = Vec::new();
        for witness in &self.witnesses {
            if &witness.self_ty != receiver {
                continue;
            }
            let Some(impl_decl) = self.impls.get(witness.impl_id.index()) else {
                continue;
            };
            let Some(method) = impl_decl.methods.get(method_name) else {
                continue;
            };
            candidates.push((Some(witness.id), witness.trait_id, method.clone()));
        }

        for predicate in predicates
            .iter()
            .filter(|predicate| predicate.subject() == receiver)
        {
            for requirement in self.inherited_methods(predicate.trait_id()) {
                if requirement.name != method_name {
                    continue;
                }
                let return_type = requirement
                    .signature
                    .return_type()
                    .map_or(TypeKind::Unit, |ty| {
                        substitute_trait_self(ty, receiver, predicate.assoc_equalities())
                    });
                candidates.push((
                    None,
                    requirement.trait_id,
                    TraitMethodImpl {
                        trait_id: Some(requirement.trait_id),
                        signature: requirement.signature.clone(),
                        return_type,
                        body: None,
                    },
                ));
            }
        }

        match candidates.as_slice() {
            [] => TraitMethodResolution::Missing,
            [(witness, trait_id, method)] => TraitMethodResolution::Unique {
                witness: *witness,
                trait_id: *trait_id,
                method: method.clone(),
            },
            _ => TraitMethodResolution::Ambiguous(
                candidates
                    .into_iter()
                    .map(|(witness, trait_id, method)| TraitMethodCandidate {
                        trait_id,
                        trait_name: self
                            .trait_name(trait_id)
                            .unwrap_or("<unknown-trait>")
                            .to_owned(),
                        witness,
                        method_name: method.signature.name().to_owned(),
                    })
                    .collect(),
            ),
        }
    }

    pub fn resolve_projection(
        &self,
        subject: &TypeKind,
        assoc: &str,
        predicates: &[TraitPredicate],
    ) -> Result<ProjectionResolution, ProjectionError> {
        let mut matches = Vec::new();
        for witness in &self.witnesses {
            if &witness.self_ty != subject {
                continue;
            }
            let Some(impl_decl) = self.impls.get(witness.impl_id.index()) else {
                continue;
            };
            if let Some(assignment) = impl_decl.associated_types.get(assoc) {
                matches.push(ProjectionResolution::Resolved(assignment.value.clone()));
            }
        }
        for predicate in predicates
            .iter()
            .filter(|predicate| predicate.subject() == subject)
        {
            let Some(trait_decl) = self.trait_decl(predicate.trait_id()) else {
                continue;
            };
            if !self.trait_has_assoc(trait_decl.id, assoc) {
                continue;
            }
            if let Some(equality) = predicate
                .assoc_equalities()
                .iter()
                .find(|equality| equality.name() == assoc)
            {
                matches.push(ProjectionResolution::Resolved(equality.ty().clone()));
            } else {
                matches.push(ProjectionResolution::Deferred(TypeKind::Projection {
                    subject: Box::new(subject.clone()),
                    trait_name: Some(trait_decl.name.clone()),
                    assoc: assoc.to_owned(),
                }));
            }
        }
        match matches.as_slice() {
            [] => Err(ProjectionError::UnknownAssociatedType {
                subject: subject.clone(),
                assoc: assoc.to_owned(),
            }),
            [resolution] => Ok(resolution.clone()),
            _ => Err(ProjectionError::Ambiguous {
                subject: subject.clone(),
                assoc: assoc.to_owned(),
            }),
        }
    }

    fn predicate_from_bound(&self, subject: TypeKind, bound: &TypeRef) -> Option<TraitPredicate> {
        let (name, bindings) = trait_bound_parts(bound)?;
        let trait_id = self.trait_id(name)?;
        Some(TraitPredicate::new(
            subject,
            trait_id,
            bindings
                .iter()
                .map(|binding| {
                    AssocEquality::new(
                        binding.name(),
                        trait_type_ref_kind(binding.value(), &HashSet::new()),
                    )
                })
                .collect::<Vec<_>>(),
        ))
    }

    fn trait_has_assoc(&self, trait_id: TraitId, assoc: &str) -> bool {
        self.inherited_associated_types(trait_id)
            .iter()
            .any(|requirement| requirement.name == assoc)
    }

    fn inherited_associated_types(&self, trait_id: TraitId) -> Vec<AssociatedTypeRequirement> {
        let mut visited = BTreeSet::new();
        let mut out = Vec::new();
        self.push_inherited_associated_types(trait_id, &mut visited, &mut out);
        out
    }

    fn push_inherited_associated_types(
        &self,
        trait_id: TraitId,
        visited: &mut BTreeSet<TraitId>,
        out: &mut Vec<AssociatedTypeRequirement>,
    ) {
        if !visited.insert(trait_id) {
            return;
        }
        let Some(trait_decl) = self.trait_decl(trait_id) else {
            return;
        };
        for supertrait in &trait_decl.supertraits {
            self.push_inherited_associated_types(*supertrait, visited, out);
        }
        out.extend(trait_decl.associated_types.iter().cloned());
    }

    fn inherited_methods(&self, trait_id: TraitId) -> Vec<TraitMethodRequirement> {
        let mut visited = BTreeSet::new();
        let mut out = Vec::new();
        self.push_inherited_methods(trait_id, &mut visited, &mut out);
        out
    }

    fn push_inherited_methods(
        &self,
        trait_id: TraitId,
        visited: &mut BTreeSet<TraitId>,
        out: &mut Vec<TraitMethodRequirement>,
    ) {
        if !visited.insert(trait_id) {
            return;
        }
        let Some(trait_decl) = self.trait_decl(trait_id) else {
            return;
        };
        for supertrait in &trait_decl.supertraits {
            self.push_inherited_methods(*supertrait, visited, out);
        }
        out.extend(trait_decl.methods.iter().cloned());
    }
}

impl TraitDecl {
    pub const fn id(&self) -> TraitId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl TraitPredicate {
    pub fn new(
        subject: TypeKind,
        trait_id: TraitId,
        assoc_equalities: impl IntoIterator<Item = AssocEquality>,
    ) -> Self {
        Self {
            subject,
            trait_id,
            assoc_equalities: assoc_equalities.into_iter().collect(),
        }
    }

    pub const fn subject(&self) -> &TypeKind {
        &self.subject
    }

    pub const fn trait_id(&self) -> TraitId {
        self.trait_id
    }

    pub fn assoc_equalities(&self) -> &[AssocEquality] {
        &self.assoc_equalities
    }
}

impl AssocEquality {
    pub fn new(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

impl TraitMethodImpl {
    pub const fn trait_id(&self) -> Option<TraitId> {
        self.trait_id
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }

    pub const fn return_type(&self) -> &TypeKind {
        &self.return_type
    }

    pub const fn body(&self) -> Option<&TraitMethodBody> {
        self.body.as_ref()
    }
}

impl<'a> TraitMethodForWitness<'a> {
    pub const fn impl_id(&self) -> ImplId {
        self.impl_id
    }

    pub const fn trait_id(&self) -> TraitId {
        self.trait_id
    }

    pub const fn witness(&self) -> TraitWitnessId {
        self.witness
    }

    pub const fn self_ty(&self) -> &'a TypeKind {
        self.self_ty
    }

    pub const fn signature(&self) -> &'a FnSignature {
        self.method.signature()
    }

    pub const fn return_type(&self) -> &'a TypeKind {
        self.method.return_type()
    }

    pub const fn body(&self) -> Option<&'a TraitMethodBody> {
        self.method.body()
    }
}

impl TraitConformance {
    pub const fn witness(&self) -> Option<TraitWitnessId> {
        self.witness
    }

    pub const fn trait_id(&self) -> TraitId {
        self.trait_id
    }

    pub const fn impl_id(&self) -> Option<ImplId> {
        self.impl_id
    }

    pub const fn self_ty(&self) -> &TypeKind {
        &self.self_ty
    }

    pub fn associated_type(&self, name: &str) -> Option<&TypeKind> {
        self.associated_types.get(name)
    }
}

impl IntoIteratorResolution {
    pub const fn source_ty(&self) -> &TypeKind {
        &self.source_ty
    }

    pub const fn item_ty(&self) -> &TypeKind {
        &self.item_ty
    }

    pub const fn into_iter_ty(&self) -> &TypeKind {
        &self.into_iter_ty
    }

    pub const fn kind(&self) -> &IntoIteratorResolutionKind {
        &self.kind
    }

    pub const fn into_iterator(&self) -> Option<&TraitConformance> {
        match &self.kind {
            IntoIteratorResolutionKind::Explicit { into_iterator, .. } => Some(into_iterator),
            IntoIteratorResolutionKind::IteratorIdentity { .. } => None,
        }
    }

    pub const fn iterator(&self) -> &TraitConformance {
        match &self.kind {
            IntoIteratorResolutionKind::Explicit { iterator, .. }
            | IntoIteratorResolutionKind::IteratorIdentity { iterator } => iterator,
        }
    }

    pub const fn is_iterator_identity(&self) -> bool {
        matches!(
            self.kind,
            IntoIteratorResolutionKind::IteratorIdentity { .. }
        )
    }
}

/// Builds a typed trait catalog and returns diagnostics for invalid declarations.
pub fn collect_trait_catalog(module: &HirModule) -> (TraitCatalog, Vec<TypeCheckError>) {
    let mut builder = TraitCatalogBuilder::new(module);
    builder.collect_traits(module);
    builder.collect_impls(module);
    builder.finish()
}

struct TraitCatalogBuilder {
    catalog: TraitCatalog,
    diagnostics: Vec<TypeCheckError>,
    local_nominals: HashSet<String>,
    next_assoc_id: usize,
}

impl TraitCatalogBuilder {
    fn new(module: &HirModule) -> Self {
        Self {
            catalog: TraitCatalog::default(),
            diagnostics: Vec::new(),
            local_nominals: collect_local_nominals(module),
            next_assoc_id: 0,
        }
    }

    fn collect_traits(&mut self, module: &HirModule) {
        standard_iter::install_standard_iterator_traits(&mut self.catalog, &mut self.next_assoc_id);
        for item in module.declarations().iter().filter_map(as_trait_item) {
            let id = TraitId::from_index(self.catalog.traits.len());
            if self.catalog.by_name.contains_key(item.name()) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::duplicate_trait(item.name()),
                ));
                continue;
            }
            self.catalog.by_name.insert(item.name().to_owned(), id);
            self.catalog.traits.push(TraitDecl {
                id,
                name: item.name().to_owned(),
                supertraits: Vec::new(),
                associated_types: Vec::new(),
                methods: Vec::new(),
            });
        }

        for item in module.declarations().iter().filter_map(as_trait_item) {
            let Some(id) = self.catalog.trait_id(item.name()) else {
                continue;
            };
            let supertraits = self.resolve_supertraits(item);
            let associated_types = self.collect_trait_associated_types(id, item);
            let methods = self.collect_trait_methods(id, item);
            if let Some(trait_decl) = self.catalog.traits.get_mut(id.index()) {
                trait_decl.supertraits = supertraits;
                trait_decl.associated_types = associated_types;
                trait_decl.methods = methods;
            }
        }
    }

    fn collect_impls(&mut self, module: &HirModule) {
        standard_iter::install_standard_iterator_impls(&mut self.catalog);
        for item in module.declarations().iter().filter_map(as_impl_item) {
            self.collect_impl(item);
        }
    }

    fn collect_impl(&mut self, item: &ImplItem) {
        if item.visibility().is_some() {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::pub_impl_unsupported(impl_head_label(item)),
            ));
        }

        let trait_id = item
            .trait_name()
            .and_then(|name| self.resolve_trait_name(name));
        if item.trait_name().is_some() && trait_id.is_none() {
            return;
        }

        let generic_params = impl_generic_names(item.generics());
        let target = parse_type_ref(item.target()).map_or_else(
            |_| TypeKind::Named(item.target().to_owned()),
            |ty| trait_type_ref_kind(&ty, &generic_params),
        );

        if let Some(trait_id) = trait_id
            && !self.impl_satisfies_orphan_rule(trait_id, &target)
        {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::orphan_impl(
                    self.catalog
                        .trait_name(trait_id)
                        .unwrap_or("<unknown-trait>"),
                    type_kind_label(&target),
                ),
            ));
        }

        let id = ImplId::from_index(self.catalog.impls.len());
        let mut impl_decl = TraitImpl {
            id,
            trait_id,
            target: target.clone(),
            associated_types: BTreeMap::new(),
            methods: BTreeMap::new(),
            witness: None,
        };

        self.collect_impl_members(item, &mut impl_decl, &generic_params);
        self.check_coherence(&impl_decl);
        if let Some(trait_id) = impl_decl.trait_id {
            self.validate_trait_impl(&impl_decl, trait_id);
            let witness = TraitWitnessId::from_index(self.catalog.witnesses.len());
            impl_decl.witness = Some(witness);
            self.catalog.witnesses.push(TraitWitness {
                id: witness,
                impl_id: id,
                trait_id,
                self_ty: target.clone(),
            });
            self.catalog.exact_impls.insert((trait_id, target), id);
        } else {
            self.register_inherent_methods(&impl_decl);
        }
        self.catalog.impls.push(impl_decl);
    }

    fn resolve_supertraits(&mut self, item: &TraitItem) -> Vec<TraitId> {
        item.supertraits()
            .iter()
            .filter_map(|name| self.resolve_trait_name(name))
            .collect()
    }

    fn collect_trait_associated_types(
        &mut self,
        trait_id: TraitId,
        item: &TraitItem,
    ) -> Vec<AssociatedTypeRequirement> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for member in item.members() {
            let TraitMember::AssociatedType {
                name,
                params,
                value,
            } = member
            else {
                continue;
            };
            if !seen.insert(name.clone()) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::duplicate_associated_type(item.name(), name),
                ));
                continue;
            }
            if !params.is_empty() {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::associated_type_constructor_unsupported(item.name(), name),
                ));
            }
            if value.is_some() {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::associated_type_default_unsupported(item.name(), name),
                ));
            }
            let id = AssociatedTypeId::from_index(self.next_assoc_id);
            self.next_assoc_id += 1;
            out.push(AssociatedTypeRequirement {
                id,
                trait_id,
                name: name.clone(),
            });
        }
        out
    }

    fn collect_trait_methods(
        &mut self,
        trait_id: TraitId,
        item: &TraitItem,
    ) -> Vec<TraitMethodRequirement> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for member in item.members() {
            let TraitMember::Function {
                signature, body, ..
            } = member
            else {
                if let TraitMember::Raw(raw) = member {
                    self.diagnostics.push(TypeCheckError::trait_diagnostic(
                        TraitDiagnostic::raw_trait_member(item.name(), raw),
                    ));
                }
                continue;
            };
            if !seen.insert(signature.name().to_owned()) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::duplicate_method(item.name(), signature.name()),
                ));
                continue;
            }
            if body.is_some() {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::trait_default_method_unsupported(
                        item.name(),
                        signature.name(),
                    ),
                ));
            }
            out.push(TraitMethodRequirement {
                trait_id,
                name: signature.name().to_owned(),
                signature: signature.clone(),
            });
        }
        out
    }

    fn collect_impl_members(
        &mut self,
        item: &ImplItem,
        impl_decl: &mut TraitImpl,
        generic_params: &HashSet<String>,
    ) {
        let mut assoc_seen = BTreeSet::new();
        let mut method_seen = BTreeSet::new();
        for member in item.members() {
            match member {
                ImplMember::AssociatedType {
                    name,
                    params,
                    value,
                } => {
                    if impl_decl.trait_id.is_none() {
                        self.diagnostics.push(TypeCheckError::trait_diagnostic(
                            TraitDiagnostic::associated_type_in_inherent_impl(name),
                        ));
                    }
                    if !params.is_empty() {
                        self.diagnostics.push(TypeCheckError::trait_diagnostic(
                            TraitDiagnostic::associated_type_constructor_unsupported(
                                item.trait_name().unwrap_or("<inherent>"),
                                name,
                            ),
                        ));
                    }
                    if !assoc_seen.insert(name.clone()) {
                        self.diagnostics.push(TypeCheckError::trait_diagnostic(
                            TraitDiagnostic::duplicate_associated_type_assignment(
                                item.trait_name().unwrap_or("<inherent>"),
                                name,
                            ),
                        ));
                    }
                    impl_decl.associated_types.insert(
                        name.clone(),
                        AssociatedTypeAssignment {
                            name: name.clone(),
                            value: trait_type_ref_kind(value, generic_params),
                        },
                    );
                }
                ImplMember::Function {
                    signature,
                    body_statements,
                    body_value,
                    ..
                } => {
                    if !method_seen.insert(signature.name().to_owned()) {
                        self.diagnostics.push(TypeCheckError::trait_diagnostic(
                            TraitDiagnostic::duplicate_method(
                                item.trait_name().unwrap_or("<inherent>"),
                                signature.name(),
                            ),
                        ));
                    }
                    let return_type = signature.return_type().map_or(TypeKind::Unit, |ty| {
                        substitute_self_type(ty, &impl_decl.target, impl_decl, generic_params)
                    });
                    impl_decl.methods.insert(
                        signature.name().to_owned(),
                        TraitMethodImpl {
                            trait_id: impl_decl.trait_id,
                            signature: signature.clone(),
                            return_type,
                            body: TraitMethodBody::new(body_statements, body_value.as_ref()),
                        },
                    );
                }
                ImplMember::Raw(raw) => {
                    self.diagnostics.push(TypeCheckError::trait_diagnostic(
                        TraitDiagnostic::raw_impl_member(impl_head_label(item), raw),
                    ));
                }
            }
        }
    }

    fn validate_trait_impl(&mut self, impl_decl: &TraitImpl, trait_id: TraitId) {
        let required_assoc = self.catalog.inherited_associated_types(trait_id);
        let required_methods = self.catalog.inherited_methods(trait_id);
        let required_assoc_names = required_assoc
            .iter()
            .map(|assoc| assoc.name.as_str())
            .collect::<BTreeSet<_>>();
        let required_method_names = required_methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<BTreeSet<_>>();
        let trait_name = self
            .catalog
            .trait_name(trait_id)
            .unwrap_or("<unknown-trait>");
        let target = type_kind_label(&impl_decl.target);

        for assoc in &required_assoc {
            if !impl_decl.associated_types.contains_key(&assoc.name) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::missing_associated_type(trait_name, &target, &assoc.name),
                ));
            }
        }
        for assignment in impl_decl.associated_types.keys() {
            if !required_assoc_names.contains(assignment.as_str()) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::unknown_associated_type(trait_name, assignment),
                ));
            }
        }

        for method in &required_methods {
            let Some(actual) = impl_decl.methods.get(&method.name) else {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::missing_required_method(trait_name, &target, &method.name),
                ));
                continue;
            };
            if !actual.body().is_some_and(TraitMethodBody::is_present) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::missing_required_method_body(
                        trait_name,
                        &target,
                        &method.name,
                    ),
                ));
            }
            if !signatures_compatible(&method.signature, &actual.signature, impl_decl) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::impl_method_signature_mismatch(trait_name, &method.name),
                ));
            }
        }
        for method in impl_decl.methods.keys() {
            if !required_method_names.contains(method.as_str()) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::unknown_trait_method(trait_name, method),
                ));
            }
        }
    }

    fn check_coherence(&mut self, impl_decl: &TraitImpl) {
        let Some(trait_id) = impl_decl.trait_id else {
            return;
        };
        if self
            .catalog
            .exact_impls
            .contains_key(&(trait_id, impl_decl.target.clone()))
        {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::duplicate_impl(
                    self.catalog
                        .trait_name(trait_id)
                        .unwrap_or("<unknown-trait>"),
                    type_kind_label(&impl_decl.target),
                ),
            ));
        }
        for existing in &self.catalog.impls {
            if existing.trait_id != Some(trait_id) {
                continue;
            }
            if impl_targets_overlap(&existing.target, &impl_decl.target) {
                self.diagnostics.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::overlapping_impl(
                        self.catalog
                            .trait_name(trait_id)
                            .unwrap_or("<unknown-trait>"),
                        type_kind_label(&existing.target),
                        type_kind_label(&impl_decl.target),
                    ),
                ));
            }
        }
    }

    fn register_inherent_methods(&mut self, impl_decl: &TraitImpl) {
        for (name, method) in &impl_decl.methods {
            self.catalog
                .inherent_methods
                .insert((impl_decl.target.clone(), name.clone()), method.clone());
        }
    }

    fn impl_satisfies_orphan_rule(&self, trait_id: TraitId, target: &TypeKind) -> bool {
        self.catalog.trait_decl(trait_id).is_some()
            || local_type_name(target).is_some_and(|name| self.local_nominals.contains(name))
    }

    fn resolve_trait_name(&mut self, name: &str) -> Option<TraitId> {
        self.catalog.trait_id(name).or_else(|| {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::unknown_trait(name),
            ));
            None
        })
    }

    fn finish(self) -> (TraitCatalog, Vec<TypeCheckError>) {
        (self.catalog, self.diagnostics)
    }
}

fn conformance_from_impl(
    impl_decl: &TraitImpl,
    subject: &TypeKind,
    assoc_equalities: &[AssocEquality],
) -> Option<TraitConformance> {
    let substitutions = match_type_pattern(&impl_decl.target, subject)?;
    let associated_types = impl_decl
        .associated_types
        .iter()
        .map(|(name, assignment)| {
            (
                name.clone(),
                substitute_type(&assignment.value, &substitutions),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assoc_equalities_match(&associated_types, assoc_equalities).then(|| TraitConformance {
        witness: impl_decl.witness,
        trait_id: impl_decl.trait_id.expect("trait impl has trait id"),
        impl_id: Some(impl_decl.id),
        self_ty: subject.clone(),
        associated_types,
    })
}

fn conformance_from_predicate(
    predicate: &TraitPredicate,
    subject: &TypeKind,
    trait_id: TraitId,
    assoc_equalities: &[AssocEquality],
) -> Option<TraitConformance> {
    if predicate.trait_id() != trait_id || predicate.subject() != subject {
        return None;
    }
    let associated_types = predicate
        .assoc_equalities()
        .iter()
        .map(|equality| (equality.name().to_owned(), equality.ty().clone()))
        .collect::<BTreeMap<_, _>>();
    assoc_equalities_match(&associated_types, assoc_equalities).then(|| TraitConformance {
        witness: None,
        trait_id,
        impl_id: None,
        self_ty: subject.clone(),
        associated_types,
    })
}

fn assoc_equalities_match(
    associated_types: &BTreeMap<String, TypeKind>,
    required: &[AssocEquality],
) -> bool {
    required.iter().all(|equality| {
        associated_types
            .get(equality.name())
            .is_some_and(|actual| actual == equality.ty())
    })
}

fn match_type_pattern(pattern: &TypeKind, actual: &TypeKind) -> Option<HashMap<String, TypeKind>> {
    let mut substitutions = HashMap::new();
    match_type_pattern_into(pattern, actual, &mut substitutions).then_some(substitutions)
}

fn match_type_pattern_into(
    pattern: &TypeKind,
    actual: &TypeKind,
    substitutions: &mut HashMap<String, TypeKind>,
) -> bool {
    if let TypeKind::GenericParam(name) = pattern {
        return match_generic_param_pattern(name, actual, substitutions);
    }
    if let Some((lhs, rhs)) = unary_type_pattern(pattern, actual) {
        return match_type_pattern_into(lhs, rhs, substitutions);
    }
    if let Some(((lhs_first, lhs_second), (rhs_first, rhs_second))) =
        binary_type_pattern(pattern, actual)
    {
        return match_type_pattern_into(lhs_first, rhs_first, substitutions)
            && match_type_pattern_into(lhs_second, rhs_second, substitutions);
    }
    if let Some(((lhs_key, lhs_value), (rhs_key, rhs_value))) = map_type_pattern(pattern, actual) {
        return match_type_pattern_into(lhs_key, rhs_key, substitutions)
            && match_type_pattern_into(lhs_value, rhs_value, substitutions);
    }
    if let Some((lhs, rhs)) = borrow_ref_type_pattern(pattern, actual) {
        return match_type_pattern_into(lhs, rhs, substitutions);
    }
    if let Some((lhs, rhs)) = iterator_state_type_pattern(pattern, actual) {
        return match_type_pattern_into(lhs, rhs, substitutions);
    }
    if let (TypeKind::Named(lhs), TypeKind::Named(rhs)) = (pattern, actual) {
        return match_named_pattern(lhs, rhs, substitutions);
    }
    pattern == actual
}

fn match_generic_param_pattern(
    name: &str,
    actual: &TypeKind,
    substitutions: &mut HashMap<String, TypeKind>,
) -> bool {
    if let Some(existing) = substitutions.get(name) {
        existing == actual
    } else {
        substitutions.insert(name.to_owned(), actual.clone());
        true
    }
}

fn unary_type_pattern<'a>(
    pattern: &'a TypeKind,
    actual: &'a TypeKind,
) -> Option<(&'a TypeKind, &'a TypeKind)> {
    match (pattern, actual) {
        (TypeKind::Vec(lhs), TypeKind::Vec(rhs))
        | (TypeKind::Seq(lhs), TypeKind::Seq(rhs))
        | (TypeKind::Slice(lhs), TypeKind::Slice(rhs))
        | (TypeKind::Range(lhs), TypeKind::Range(rhs))
        | (TypeKind::Option(lhs), TypeKind::Option(rhs))
        | (TypeKind::ThreadHandle(lhs), TypeKind::ThreadHandle(rhs))
        | (TypeKind::Shared(lhs), TypeKind::Shared(rhs))
        | (TypeKind::Array { item: lhs, .. }, TypeKind::Array { item: rhs, .. }) => {
            Some((lhs, rhs))
        }
        _ => None,
    }
}

type BinaryTypePattern<'a> = ((&'a TypeKind, &'a TypeKind), (&'a TypeKind, &'a TypeKind));

fn binary_type_pattern<'a>(
    pattern: &'a TypeKind,
    actual: &'a TypeKind,
) -> Option<BinaryTypePattern<'a>> {
    match (pattern, actual) {
        (
            TypeKind::Need {
                ready: lhs_ready,
                error: lhs_error,
            },
            TypeKind::Need {
                ready: rhs_ready,
                error: rhs_error,
            },
        )
        | (
            TypeKind::Stream {
                item: lhs_ready,
                error: lhs_error,
            },
            TypeKind::Stream {
                item: rhs_ready,
                error: rhs_error,
            },
        )
        | (
            TypeKind::Source {
                item: lhs_ready,
                error: lhs_error,
            },
            TypeKind::Source {
                item: rhs_ready,
                error: rhs_error,
            },
        )
        | (
            TypeKind::Result {
                ok: lhs_ready,
                error: lhs_error,
            },
            TypeKind::Result {
                ok: rhs_ready,
                error: rhs_error,
            },
        ) => Some(((lhs_ready, lhs_error), (rhs_ready, rhs_error))),
        _ => None,
    }
}

fn map_type_pattern<'a>(
    pattern: &'a TypeKind,
    actual: &'a TypeKind,
) -> Option<BinaryTypePattern<'a>> {
    match (pattern, actual) {
        (
            TypeKind::Map {
                kind: lhs_kind,
                key: lhs_key,
                value: lhs_value,
            },
            TypeKind::Map {
                kind: rhs_kind,
                key: rhs_key,
                value: rhs_value,
            },
        ) if lhs_kind == rhs_kind => Some(((lhs_key, lhs_value), (rhs_key, rhs_value))),
        _ => None,
    }
}

fn borrow_ref_type_pattern<'a>(
    pattern: &'a TypeKind,
    actual: &'a TypeKind,
) -> Option<(&'a TypeKind, &'a TypeKind)> {
    match (pattern, actual) {
        (
            TypeKind::BorrowRef {
                lifetime: lhs_lifetime,
                inner: lhs_inner,
            },
            TypeKind::BorrowRef {
                lifetime: rhs_lifetime,
                inner: rhs_inner,
            },
        ) if lhs_lifetime == rhs_lifetime => Some((lhs_inner, rhs_inner)),
        _ => None,
    }
}

fn iterator_state_type_pattern<'a>(
    pattern: &'a TypeKind,
    actual: &'a TypeKind,
) -> Option<(&'a TypeKind, &'a TypeKind)> {
    match (pattern, actual) {
        (
            TypeKind::IteratorState {
                family: lhs_family,
                item: lhs_item,
            },
            TypeKind::IteratorState {
                family: rhs_family,
                item: rhs_item,
            },
        ) if lhs_family == rhs_family => Some((lhs_item, rhs_item)),
        _ => None,
    }
}

fn match_named_pattern(
    pattern: &str,
    actual: &str,
    substitutions: &mut HashMap<String, TypeKind>,
) -> bool {
    if pattern == actual {
        return true;
    }
    let Some((pattern_base, pattern_arg)) = split_one_generic_arg(pattern) else {
        return false;
    };
    let Some((actual_base, actual_arg)) = split_one_generic_arg(actual) else {
        return false;
    };
    if pattern_base != actual_base {
        return false;
    }
    if let Some(existing) = substitutions.get(pattern_arg) {
        existing == &TypeKind::Named(actual_arg.to_owned())
    } else {
        substitutions.insert(
            pattern_arg.to_owned(),
            TypeKind::Named(actual_arg.to_owned()),
        );
        true
    }
}

fn split_one_generic_arg(value: &str) -> Option<(&str, &str)> {
    let (base, rest) = value.split_once('<')?;
    let arg = rest.strip_suffix('>')?.trim();
    (!arg.contains(',')).then_some((base.trim(), arg))
}

fn substitute_type(ty: &TypeKind, substitutions: &HashMap<String, TypeKind>) -> TypeKind {
    match ty {
        TypeKind::GenericParam(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        TypeKind::Vec(inner) => TypeKind::Vec(Box::new(substitute_type(inner, substitutions))),
        TypeKind::Seq(inner) => TypeKind::Seq(Box::new(substitute_type(inner, substitutions))),
        TypeKind::Slice(inner) => TypeKind::Slice(Box::new(substitute_type(inner, substitutions))),
        TypeKind::Range(inner) => TypeKind::Range(Box::new(substitute_type(inner, substitutions))),
        TypeKind::Option(inner) => {
            TypeKind::Option(Box::new(substitute_type(inner, substitutions)))
        }
        TypeKind::ThreadHandle(inner) => {
            TypeKind::ThreadHandle(Box::new(substitute_type(inner, substitutions)))
        }
        TypeKind::Shared(inner) => {
            TypeKind::Shared(Box::new(substitute_type(inner, substitutions)))
        }
        TypeKind::BorrowRef { lifetime, inner } => TypeKind::BorrowRef {
            lifetime: lifetime.clone(),
            inner: Box::new(substitute_type(inner, substitutions)),
        },
        TypeKind::IteratorState { family, item } => TypeKind::IteratorState {
            family: *family,
            item: Box::new(substitute_type(item, substitutions)),
        },
        TypeKind::Need { ready, error } => TypeKind::Need {
            ready: Box::new(substitute_type(ready, substitutions)),
            error: Box::new(substitute_type(error, substitutions)),
        },
        TypeKind::Stream { item, error } => TypeKind::Stream {
            item: Box::new(substitute_type(item, substitutions)),
            error: Box::new(substitute_type(error, substitutions)),
        },
        TypeKind::Source { item, error } => TypeKind::Source {
            item: Box::new(substitute_type(item, substitutions)),
            error: Box::new(substitute_type(error, substitutions)),
        },
        TypeKind::Result { ok, error } => TypeKind::Result {
            ok: Box::new(substitute_type(ok, substitutions)),
            error: Box::new(substitute_type(error, substitutions)),
        },
        TypeKind::Map { kind, key, value } => TypeKind::Map {
            kind: *kind,
            key: Box::new(substitute_type(key, substitutions)),
            value: Box::new(substitute_type(value, substitutions)),
        },
        TypeKind::Array { item, len } => TypeKind::Array {
            item: Box::new(substitute_type(item, substitutions)),
            len: len.clone(),
        },
        other => other.clone(),
    }
}

fn as_trait_item(decl: &HirTopLevelDecl) -> Option<&TraitItem> {
    match decl {
        HirTopLevelDecl::Trait(item) => Some(item),
        _ => None,
    }
}

fn as_impl_item(decl: &HirTopLevelDecl) -> Option<&ImplItem> {
    match decl {
        HirTopLevelDecl::Impl(item) => Some(item),
        _ => None,
    }
}

fn collect_local_nominals(module: &HirModule) -> HashSet<String> {
    module
        .declarations()
        .iter()
        .filter_map(|decl| match decl {
            HirTopLevelDecl::Struct(item) => Some(item.name().to_owned()),
            HirTopLevelDecl::Enum(item) => Some(item.name().to_owned()),
            HirTopLevelDecl::TypeAlias(item) => Some(item.name().to_owned()),
            HirTopLevelDecl::State(item) => Some(item.name().to_owned()),
            _ => None,
        })
        .collect()
}

fn impl_generic_names(generics: Option<&str>) -> HashSet<String> {
    generics
        .into_iter()
        .flat_map(|source| source.split(','))
        .filter_map(|param| {
            param
                .trim()
                .split_once(':')
                .map_or(Some(param.trim()), |(name, _)| Some(name.trim()))
        })
        .filter(|name| !name.is_empty() && !name.starts_with('\''))
        .map(ToOwned::to_owned)
        .collect()
}

fn signatures_compatible(
    required: &FnSignature,
    actual: &FnSignature,
    impl_decl: &TraitImpl,
) -> bool {
    if required.name() != actual.name()
        || required.param_groups().len() != actual.param_groups().len()
    {
        return false;
    }
    let generic_params = HashSet::new();
    for (required_group, actual_group) in required.param_groups().iter().zip(actual.param_groups())
    {
        if required_group.params().len() != actual_group.params().len() {
            return false;
        }
        for (required_param, actual_param) in
            required_group.params().iter().zip(actual_group.params())
        {
            if function_param_type(
                required_param,
                &impl_decl.target,
                impl_decl,
                &generic_params,
            ) != function_param_type(actual_param, &impl_decl.target, impl_decl, &generic_params)
            {
                return false;
            }
        }
    }
    let expected = required.return_type().map_or(TypeKind::Unit, |ty| {
        substitute_self_type(ty, &impl_decl.target, impl_decl, &generic_params)
    });
    let actual = actual.return_type().map_or(TypeKind::Unit, |ty| {
        substitute_self_type(ty, &impl_decl.target, impl_decl, &generic_params)
    });
    expected == actual
}

fn function_param_type(
    param: &FnParam,
    self_ty: &TypeKind,
    impl_decl: &TraitImpl,
    generic_params: &HashSet<String>,
) -> TypeKind {
    substitute_self_type(param.ty(), self_ty, impl_decl, generic_params)
}

fn substitute_self_type(
    ty: &TypeRef,
    self_ty: &TypeKind,
    impl_decl: &TraitImpl,
    generic_params: &HashSet<String>,
) -> TypeKind {
    match ty {
        TypeRef::Path(path) if path == "Self" => self_ty.clone(),
        TypeRef::Projection { subject, assoc } if matches!(subject.as_ref(), TypeRef::Path(path) if path == "Self") => {
            impl_decl.associated_types.get(assoc).map_or_else(
                || trait_type_ref_kind(ty, generic_params),
                |assignment| assignment.value.clone(),
            )
        }
        TypeRef::Generic { base, args } => {
            let arg_types = args
                .iter()
                .map(|arg| substitute_self_type(arg, self_ty, impl_decl, generic_params))
                .collect::<Vec<_>>();
            generic_type_kind(base, &arg_types)
        }
        TypeRef::Choice(alternatives) => TypeKind::Choice(
            alternatives
                .iter()
                .map(|alternative| {
                    substitute_self_type(alternative, self_ty, impl_decl, generic_params)
                })
                .collect(),
        ),
        TypeRef::Ref { inner, .. } => TypeKind::BorrowRef {
            lifetime: None,
            inner: Box::new(substitute_self_type(
                inner,
                self_ty,
                impl_decl,
                generic_params,
            )),
        },
        TypeRef::Slice(inner) => TypeKind::Slice(Box::new(substitute_self_type(
            inner,
            self_ty,
            impl_decl,
            generic_params,
        ))),
        _ => trait_type_ref_kind(ty, generic_params),
    }
}

fn substitute_trait_self(
    ty: &TypeRef,
    self_ty: &TypeKind,
    assoc_equalities: &[AssocEquality],
) -> TypeKind {
    match ty {
        TypeRef::Path(path) if path == "Self" => self_ty.clone(),
        TypeRef::Projection { subject, assoc } if matches!(subject.as_ref(), TypeRef::Path(path) if path == "Self") => {
            assoc_equalities
                .iter()
                .find(|equality| equality.name() == assoc)
                .map_or_else(
                    || TypeKind::Projection {
                        subject: Box::new(self_ty.clone()),
                        trait_name: None,
                        assoc: assoc.clone(),
                    },
                    |equality| equality.ty().clone(),
                )
        }
        _ => trait_type_ref_kind(ty, &HashSet::new()),
    }
}

fn trait_type_ref_kind(ty: &TypeRef, generic_params: &HashSet<String>) -> TypeKind {
    match ty {
        TypeRef::Never => TypeKind::Never,
        TypeRef::ConstInt(value) => TypeKind::Named(value.to_string()),
        TypeRef::Path(path) if path == "Self" || generic_params.contains(path) => {
            TypeKind::GenericParam(path.clone())
        }
        TypeRef::Path(path) => primitive_or_named(path),
        TypeRef::Tuple(items) => TypeKind::Tuple(
            items
                .iter()
                .map(|item| trait_type_ref_kind(item, generic_params))
                .collect(),
        ),
        TypeRef::Function {
            params,
            return_type,
        } => TypeKind::function(
            params
                .iter()
                .map(|param| trait_type_ref_kind(param, generic_params)),
            trait_type_ref_kind(return_type, generic_params),
        ),
        TypeRef::Projection { subject, assoc } => TypeKind::Projection {
            subject: Box::new(trait_type_ref_kind(subject, generic_params)),
            trait_name: None,
            assoc: assoc.clone(),
        },
        TypeRef::Generic { base, args } => {
            let arg_types = args
                .iter()
                .map(|arg| trait_type_ref_kind(arg, generic_params))
                .collect::<Vec<_>>();
            generic_type_kind(base, &arg_types)
        }
        TypeRef::TraitBound(bound) => primitive_or_named(bound.path()),
        TypeRef::Choice(alternatives) => TypeKind::Choice(
            alternatives
                .iter()
                .map(|alternative| trait_type_ref_kind(alternative, generic_params))
                .collect(),
        ),
        TypeRef::Ref { inner, .. } => TypeKind::BorrowRef {
            lifetime: None,
            inner: Box::new(trait_type_ref_kind(inner, generic_params)),
        },
        TypeRef::Slice(inner) => {
            TypeKind::Slice(Box::new(trait_type_ref_kind(inner, generic_params)))
        }
    }
}

fn generic_type_kind(base: &str, args: &[TypeKind]) -> TypeKind {
    match (base, args) {
        ("Vec", [item]) => TypeKind::Vec(Box::new(item.clone())),
        ("Seq", [item]) => TypeKind::Seq(Box::new(item.clone())),
        ("Range", [item]) => TypeKind::Range(Box::new(item.clone())),
        ("Option", [item]) => TypeKind::Option(Box::new(item.clone())),
        ("Result", [ok, error]) => TypeKind::Result {
            ok: Box::new(ok.clone()),
            error: Box::new(error.clone()),
        },
        _ => TypeKind::Named(format!(
            "{base}<{}>",
            args.iter()
                .map(type_kind_label)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn primitive_or_named(path: &str) -> TypeKind {
    TypeKind::primitive_name(path).unwrap_or_else(|| {
        if path.chars().next().is_some_and(char::is_uppercase) && path.chars().count() == 1 {
            TypeKind::GenericParam(path.to_owned())
        } else {
            TypeKind::Named(path.to_owned())
        }
    })
}

fn trait_bound_parts(bound: &TypeRef) -> Option<(&str, &[AssocTypeBinding])> {
    match bound {
        TypeRef::Path(path) => Some((path.as_str(), &[])),
        TypeRef::TraitBound(bound) => Some((bound.path(), bound.assoc_bindings())),
        TypeRef::Generic { base, .. } => Some((base.as_str(), &[])),
        _ => None,
    }
}

fn impl_head_label(item: &ImplItem) -> String {
    item.trait_name().map_or_else(
        || format!("impl {}", item.target()),
        |trait_name| format!("impl {trait_name} for {}", item.target()),
    )
}

fn local_type_name(ty: &TypeKind) -> Option<&str> {
    match ty {
        TypeKind::Named(name) | TypeKind::GenericParam(name) => Some(type_head(name)),
        TypeKind::Vec(inner)
        | TypeKind::Seq(inner)
        | TypeKind::Range(inner)
        | TypeKind::Slice(inner)
        | TypeKind::Option(inner) => local_type_name(inner),
        _ => None,
    }
}

fn impl_targets_overlap(left: &TypeKind, right: &TypeKind) -> bool {
    if left == right || is_generic_wildcard(left) || is_generic_wildcard(right) {
        return true;
    }
    match (left, right) {
        (TypeKind::Named(left), TypeKind::Named(right)) => {
            type_head(left) == type_head(right)
                && (label_has_generic(left) || label_has_generic(right))
        }
        (TypeKind::Vec(left), TypeKind::Vec(right))
        | (TypeKind::Seq(left), TypeKind::Seq(right))
        | (TypeKind::Range(left), TypeKind::Range(right))
        | (TypeKind::Slice(left), TypeKind::Slice(right))
        | (TypeKind::Option(left), TypeKind::Option(right)) => impl_targets_overlap(left, right),
        _ => false,
    }
}

fn is_generic_wildcard(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::GenericParam(_)) || matches!(ty, TypeKind::Named(name) if name == "_")
}
