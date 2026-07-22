//! Typed trait / impl / associated-type catalog for Arcweft DSL semantics.
//!
//! Seq08.1 keeps conformance evidence in semantic analysis. Parser and HIR
//! preserve syntax; later runtime-plan cuts consume typed witnesses instead of
//! rediscovering trait relationships from strings.

mod builder;
mod catalog;
mod format;
mod standard_iter;

pub(crate) use builder::{collect_trait_catalog, trait_predicate_inputs_for_signature};

use crate::env::{FunctionParam, FunctionSignature};
use crate::nominal::{GenericTypeBinding, GenericTypeScope, TypeSourceEvidence};
use crate::types::{DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId, TypeKind};
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lang_syntax::ast::flow::{AuthoredExpr, Stmt};
use arcweft_lang_syntax::ast::items::{ImplItem, TraitItem};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::ast::pattern::Pattern;
use arcweft_lang_syntax::types::{
    AssociatedTypeBinding, AuthoredTypeRef, FnParam, FnSignature, GenericParam, TypeRef,
};
use format::type_kind_label;
use std::collections::{BTreeMap, HashMap, HashSet};

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
    inherent_methods: HashMap<(TypeKind, String), (ImplId, TraitMethodImpl)>,
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
    self_parameter: GenericTypeParameterId,
    param_groups: Vec<Vec<FunctionParam>>,
    return_type: TypeKind,
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

/// Source-resolved predicate input awaiting catalog trait selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TraitPredicateInput {
    subject: TypeKind,
    trait_name: String,
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
    param_groups: Vec<Vec<FunctionParam>>,
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
    Inherent {
        implementation: ImplId,
        method: TraitMethodImpl,
    },
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

    /// Projects this selected method into the semantic function signature used
    /// by the shared callable schema.
    pub(crate) fn call_signature(&self, return_type: TypeKind) -> FunctionSignature {
        let return_type =
            self.param_groups
                .iter()
                .skip(1)
                .rev()
                .fold(return_type, |return_type, group| {
                    TypeKind::function(group.iter().map(|param| param.ty().clone()), return_type)
                });
        let params = self.param_groups.first().cloned().unwrap_or_default();
        let remaining_param_groups = self
            .param_groups
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>();
        FunctionSignature::new(return_type, params)
            .with_remaining_param_groups(remaining_param_groups)
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

impl TraitWitness {
    pub const fn impl_id(&self) -> ImplId {
        self.impl_id
    }

    pub const fn trait_id(&self) -> TraitId {
        self.trait_id
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

#[derive(Default)]
struct TypePatternSubstitutions {
    generics: HashMap<GenericTypeParameterId, TypeKind>,
}

fn match_type_pattern(pattern: &TypeKind, actual: &TypeKind) -> Option<TypePatternSubstitutions> {
    let mut substitutions = TypePatternSubstitutions::default();
    match_type_pattern_into(pattern, actual, &mut substitutions).then_some(substitutions)
}

fn match_type_pattern_into(
    pattern: &TypeKind,
    actual: &TypeKind,
    substitutions: &mut TypePatternSubstitutions,
) -> bool {
    if let TypeKind::GenericParam(name) = pattern {
        return match_generic_param_pattern(name, actual, substitutions);
    }
    match (pattern, actual) {
        (
            TypeKind::Array {
                item: pattern_item,
                len: pattern_len,
            },
            TypeKind::Array {
                item: actual_item,
                len: actual_len,
            },
        ) => {
            return pattern_len == actual_len
                && match_type_pattern_into(pattern_item, actual_item, substitutions);
        }
        (
            TypeKind::Function {
                params: pattern_params,
                return_type: pattern_return,
                effects: pattern_effects,
            },
            TypeKind::Function {
                params: actual_params,
                return_type: actual_return,
                effects: actual_effects,
            },
        ) => {
            return pattern_effects == actual_effects
                && match_type_pattern_sequence(pattern_params, actual_params, substitutions)
                && match_type_pattern_into(pattern_return, actual_return, substitutions);
        }
        (TypeKind::ProjectNominal(pattern), TypeKind::ProjectNominal(actual)) => {
            return pattern.declaration() == actual.declaration()
                && match_type_pattern_sequence(
                    pattern.arguments(),
                    actual.arguments(),
                    substitutions,
                );
        }
        (TypeKind::AcceptedNominal(pattern), TypeKind::AcceptedNominal(actual)) => {
            return pattern.declaration() == actual.declaration()
                && match_type_pattern_sequence(
                    pattern.arguments(),
                    actual.arguments(),
                    substitutions,
                );
        }
        (TypeKind::OpenNominal(pattern), TypeKind::OpenNominal(actual)) => {
            return pattern.rule() == actual.rule()
                && pattern.path() == actual.path()
                && match_type_pattern_sequence(
                    pattern.arguments(),
                    actual.arguments(),
                    substitutions,
                );
        }
        (
            TypeKind::Projection {
                subject: pattern_subject,
                trait_name: pattern_trait,
                assoc: pattern_assoc,
            },
            TypeKind::Projection {
                subject: actual_subject,
                trait_name: actual_trait,
                assoc: actual_assoc,
            },
        ) => {
            return pattern_trait == actual_trait
                && pattern_assoc == actual_assoc
                && match_type_pattern_into(pattern_subject, actual_subject, substitutions);
        }
        (TypeKind::Tuple(pattern), TypeKind::Tuple(actual))
        | (TypeKind::Choice(pattern), TypeKind::Choice(actual)) => {
            return match_type_pattern_sequence(pattern, actual, substitutions);
        }
        _ => {}
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
    pattern == actual
}

fn match_type_pattern_sequence(
    pattern: &[TypeKind],
    actual: &[TypeKind],
    substitutions: &mut TypePatternSubstitutions,
) -> bool {
    pattern.len() == actual.len()
        && pattern
            .iter()
            .zip(actual)
            .all(|(pattern, actual)| match_type_pattern_into(pattern, actual, substitutions))
}

fn match_generic_param_pattern(
    parameter: &GenericTypeParameterId,
    actual: &TypeKind,
    substitutions: &mut TypePatternSubstitutions,
) -> bool {
    if let Some(existing) = substitutions.generics.get(parameter) {
        existing == actual
    } else {
        substitutions
            .generics
            .insert(parameter.clone(), actual.clone());
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
        | (TypeKind::Probe(lhs), TypeKind::Probe(rhs))
        | (TypeKind::ThreadHandle(lhs), TypeKind::ThreadHandle(rhs))
        | (TypeKind::Shared(lhs), TypeKind::Shared(rhs)) => Some((lhs, rhs)),
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
                kind: lhs_kind,
                lifetime: lhs_lifetime,
                inner: lhs_inner,
            },
            TypeKind::BorrowRef {
                kind: rhs_kind,
                lifetime: rhs_lifetime,
                inner: rhs_inner,
            },
        ) if lhs_kind == rhs_kind && lhs_lifetime == rhs_lifetime => Some((lhs_inner, rhs_inner)),
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

fn substitute_type(ty: &TypeKind, substitutions: &TypePatternSubstitutions) -> TypeKind {
    if let Some(nominal) = substitute_nominal_type(ty, substitutions) {
        return nominal;
    }

    match ty {
        TypeKind::GenericParam(name) => substitutions
            .generics
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
        TypeKind::BorrowRef {
            kind,
            lifetime,
            inner,
        } => TypeKind::BorrowRef {
            kind: *kind,
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
        TypeKind::Probe(inner) => TypeKind::Probe(Box::new(substitute_type(inner, substitutions))),
        TypeKind::Function {
            params,
            return_type,
            effects,
        } => substitute_function_type(params, return_type, effects, substitutions),
        TypeKind::ProjectNominal(_) | TypeKind::AcceptedNominal(_) | TypeKind::OpenNominal(_) => {
            unreachable!("nominal substitutions return before the structural match")
        }
        TypeKind::Projection {
            subject,
            trait_name,
            assoc,
        } => TypeKind::Projection {
            subject: Box::new(substitute_type(subject, substitutions)),
            trait_name: trait_name.clone(),
            assoc: assoc.clone(),
        },
        TypeKind::Tuple(items) => TypeKind::Tuple(
            items
                .iter()
                .map(|item| substitute_type(item, substitutions))
                .collect(),
        ),
        TypeKind::Choice(items) => TypeKind::Choice(
            items
                .iter()
                .map(|item| substitute_type(item, substitutions))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn substitute_function_type(
    params: &[TypeKind],
    return_type: &TypeKind,
    effects: &crate::effect_row::EffectRow,
    substitutions: &TypePatternSubstitutions,
) -> TypeKind {
    TypeKind::function_with_effects(
        params
            .iter()
            .map(|param| substitute_type(param, substitutions)),
        substitute_type(return_type, substitutions),
        effects.clone(),
    )
}

fn substitute_nominal_type(
    ty: &TypeKind,
    substitutions: &TypePatternSubstitutions,
) -> Option<TypeKind> {
    let arguments = |items: &[TypeKind]| {
        items
            .iter()
            .map(|argument| substitute_type(argument, substitutions))
            .collect::<Vec<_>>()
    };
    match ty {
        TypeKind::ProjectNominal(nominal) => Some(TypeKind::ProjectNominal(
            crate::types::ProjectNominalType::new(
                nominal.declaration().clone(),
                arguments(nominal.arguments()),
            ),
        )),
        TypeKind::AcceptedNominal(nominal) => Some(TypeKind::AcceptedNominal(
            crate::types::AcceptedNominalType::new(
                nominal.declaration().clone(),
                arguments(nominal.arguments()),
            ),
        )),
        TypeKind::OpenNominal(nominal) => {
            Some(TypeKind::OpenNominal(crate::types::OpenNominalType::new(
                nominal.rule().clone(),
                nominal.path().clone(),
                arguments(nominal.arguments()),
            )))
        }
        _ => None,
    }
}

fn instantiate_trait_requirement_type(
    ty: &TypeKind,
    self_parameter: &GenericTypeParameterId,
    receiver: &TypeKind,
    assoc_equalities: &[AssocEquality],
) -> TypeKind {
    let mut substitutions = TypePatternSubstitutions::default();
    substitutions
        .generics
        .insert(self_parameter.clone(), receiver.clone());
    let substituted = substitute_type(ty, &substitutions);
    resolve_predicate_associated_types(substituted, receiver, assoc_equalities)
}

fn instantiate_trait_requirement_params(
    groups: &[Vec<FunctionParam>],
    self_parameter: &GenericTypeParameterId,
    receiver: &TypeKind,
    assoc_equalities: &[AssocEquality],
) -> Vec<Vec<FunctionParam>> {
    groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|param| {
                    FunctionParam::new(
                        param.name().map(str::to_owned),
                        instantiate_trait_requirement_type(
                            param.ty(),
                            self_parameter,
                            receiver,
                            assoc_equalities,
                        ),
                        param.kind(),
                        param.has_default(),
                        [],
                    )
                })
                .collect()
        })
        .collect()
}

fn resolve_predicate_associated_types(
    ty: TypeKind,
    receiver: &TypeKind,
    assoc_equalities: &[AssocEquality],
) -> TypeKind {
    let resolve = |ty| resolve_predicate_associated_types(ty, receiver, assoc_equalities);
    match ty {
        TypeKind::Projection {
            subject,
            trait_name,
            assoc,
        } => resolve_predicate_projection(*subject, trait_name, assoc, receiver, assoc_equalities),
        TypeKind::Vec(inner) => TypeKind::Vec(Box::new(resolve(*inner))),
        TypeKind::Seq(inner) => TypeKind::Seq(Box::new(resolve(*inner))),
        TypeKind::Slice(inner) => TypeKind::Slice(Box::new(resolve(*inner))),
        TypeKind::Range(inner) => TypeKind::Range(Box::new(resolve(*inner))),
        TypeKind::Option(inner) => TypeKind::Option(Box::new(resolve(*inner))),
        TypeKind::Probe(inner) => TypeKind::Probe(Box::new(resolve(*inner))),
        TypeKind::ThreadHandle(inner) => TypeKind::ThreadHandle(Box::new(resolve(*inner))),
        TypeKind::Shared(inner) => TypeKind::Shared(Box::new(resolve(*inner))),
        TypeKind::BorrowRef {
            kind,
            lifetime,
            inner,
        } => TypeKind::BorrowRef {
            kind,
            lifetime,
            inner: Box::new(resolve(*inner)),
        },
        TypeKind::IteratorState { family, item } => TypeKind::IteratorState {
            family,
            item: Box::new(resolve(*item)),
        },
        TypeKind::Need { ready, error } => TypeKind::Need {
            ready: Box::new(resolve(*ready)),
            error: Box::new(resolve(*error)),
        },
        TypeKind::Stream { item, error } => TypeKind::Stream {
            item: Box::new(resolve(*item)),
            error: Box::new(resolve(*error)),
        },
        TypeKind::Source { item, error } => TypeKind::Source {
            item: Box::new(resolve(*item)),
            error: Box::new(resolve(*error)),
        },
        TypeKind::Result { ok, error } => TypeKind::Result {
            ok: Box::new(resolve(*ok)),
            error: Box::new(resolve(*error)),
        },
        TypeKind::Map { kind, key, value } => TypeKind::Map {
            kind,
            key: Box::new(resolve(*key)),
            value: Box::new(resolve(*value)),
        },
        TypeKind::Array { item, len } => TypeKind::Array {
            item: Box::new(resolve(*item)),
            len,
        },
        TypeKind::Function {
            params,
            return_type,
            effects,
        } => TypeKind::function_with_effects(
            params.into_iter().map(&resolve),
            resolve(*return_type),
            effects,
        ),
        TypeKind::ProjectNominal(nominal) => {
            TypeKind::ProjectNominal(crate::types::ProjectNominalType::new(
                nominal.declaration().clone(),
                nominal
                    .arguments()
                    .iter()
                    .cloned()
                    .map(&resolve)
                    .collect::<Vec<_>>(),
            ))
        }
        TypeKind::AcceptedNominal(nominal) => {
            TypeKind::AcceptedNominal(crate::types::AcceptedNominalType::new(
                nominal.declaration().clone(),
                nominal
                    .arguments()
                    .iter()
                    .cloned()
                    .map(&resolve)
                    .collect::<Vec<_>>(),
            ))
        }
        TypeKind::OpenNominal(nominal) => {
            TypeKind::OpenNominal(crate::types::OpenNominalType::new(
                nominal.rule().clone(),
                nominal.path().clone(),
                nominal
                    .arguments()
                    .iter()
                    .cloned()
                    .map(&resolve)
                    .collect::<Vec<_>>(),
            ))
        }
        TypeKind::Tuple(items) => TypeKind::Tuple(items.into_iter().map(&resolve).collect()),
        TypeKind::Choice(items) => TypeKind::Choice(items.into_iter().map(resolve).collect()),
        other => other,
    }
}

fn resolve_predicate_projection(
    subject: TypeKind,
    trait_name: Option<String>,
    assoc: String,
    receiver: &TypeKind,
    assoc_equalities: &[AssocEquality],
) -> TypeKind {
    let subject = resolve_predicate_associated_types(subject, receiver, assoc_equalities);
    assoc_equalities
        .iter()
        .find(|equality| subject == *receiver && equality.name() == assoc)
        .map_or(
            TypeKind::Projection {
                subject: Box::new(subject),
                trait_name,
                assoc,
            },
            |equality| equality.ty().clone(),
        )
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
            _ => None,
        })
        .collect()
}

fn generic_owner_for_range(
    module: &HirModule,
    declaration_module: &CanonicalModulePath,
    range: TextRange,
) -> GenericTypeOwnerId {
    module
        .project_source_span(declaration_module, range)
        .map_or_else(
            || detached_generic_owner_from_range(range),
            GenericTypeOwnerId::AcceptedSource,
        )
}

fn generic_owner_for_signature(
    module: &HirModule,
    declaration_module: &CanonicalModulePath,
    signature: &FnSignature,
    fallback: TextRange,
) -> GenericTypeOwnerId {
    let range = signature
        .generic_params()
        .first()
        .map_or(fallback, |parameter| match parameter {
            GenericParam::Lifetime(lifetime) => lifetime.range(),
            GenericParam::Type(parameter) => parameter.range(),
        });
    generic_owner_for_range(module, declaration_module, range)
}

fn generic_type_scope(
    module: &HirModule,
    declaration_module: &CanonicalModulePath,
    generics: &[GenericParam],
    owner: &GenericTypeOwnerId,
) -> GenericTypeScope {
    let bindings = generics
        .iter()
        .filter_map(GenericParam::as_type_param)
        .enumerate()
        .map(|(ordinal, parameter)| {
            let source = module
                .project_source_span(declaration_module, parameter.name_range())
                .map_or_else(
                    || TypeSourceEvidence::detached(parameter.name_range()),
                    |project| TypeSourceEvidence::accepted(parameter.name_range(), project),
                );
            GenericTypeBinding::new(
                GenericTypeParameterId::new(
                    owner.clone(),
                    u16::try_from(ordinal).expect("syntax generic-parameter limit fits u16"),
                ),
                parameter.name().clone(),
                source,
            )
        });
    GenericTypeScope::try_new(bindings)
        .expect("syntax owner must not declare duplicate generic type parameters")
}

fn nested_generic_type_scope(
    module: &HirModule,
    declaration_module: &CanonicalModulePath,
    generics: &[GenericParam],
    owner: &GenericTypeOwnerId,
    parent: &GenericTypeScope,
) -> GenericTypeScope {
    let child = generic_type_scope(module, declaration_module, generics, owner);
    let mut bindings = child.bindings().cloned().collect::<Vec<_>>();
    bindings.extend(
        parent
            .bindings()
            .filter(|binding| child.binding(binding.name()).is_none())
            .cloned(),
    );
    GenericTypeScope::try_new(bindings)
        .expect("child generic names shadow parent bindings before scope construction")
}

pub(crate) fn detached_generic_owner_from_range(range: TextRange) -> GenericTypeOwnerId {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in range
        .start()
        .to_le_bytes()
        .into_iter()
        .chain(range.end().to_le_bytes())
    {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(value))
}

fn method_signatures_compatible(
    required: &TraitMethodRequirement,
    actual: &TraitMethodImpl,
    impl_decl: &TraitImpl,
) -> bool {
    if required.signature.name() != actual.signature.name()
        || required.param_groups.len() != actual.param_groups.len()
    {
        return false;
    }
    let assoc_equalities = impl_decl
        .associated_types
        .values()
        .map(|assignment| AssocEquality::new(&assignment.name, assignment.value.clone()))
        .collect::<Vec<_>>();
    let required_groups = instantiate_trait_requirement_params(
        &required.param_groups,
        &required.self_parameter,
        &impl_decl.target,
        &assoc_equalities,
    );
    let mut substitutions = TypePatternSubstitutions::default();
    for (required_group, actual_group) in required_groups.iter().zip(&actual.param_groups) {
        if required_group.len() != actual_group.len() {
            return false;
        }
        for (required_param, actual_param) in required_group.iter().zip(actual_group) {
            if required_param.kind() != actual_param.kind()
                || required_param.has_default() != actual_param.has_default()
                || !match_type_pattern_into(
                    required_param.ty(),
                    actual_param.ty(),
                    &mut substitutions,
                )
            {
                return false;
            }
        }
    }
    let required_return = instantiate_trait_requirement_type(
        &required.return_type,
        &required.self_parameter,
        &impl_decl.target,
        &assoc_equalities,
    );
    match_type_pattern_into(&required_return, &actual.return_type, &mut substitutions)
}

pub(super) fn trait_method_param_groups(
    signature: &FnSignature,
    mut resolve: impl FnMut(&AuthoredTypeRef) -> Option<TypeKind>,
) -> Option<Vec<Vec<FunctionParam>>> {
    let mut groups = Vec::with_capacity(signature.param_groups().len());
    for group in signature.param_groups() {
        let mut params = Vec::with_capacity(group.params().len());
        for param in group
            .params()
            .iter()
            .filter(|param| !is_trait_receiver_param(param))
        {
            let ty = param.ty().map_or(Some(TypeKind::Unit), &mut resolve)?;
            params.push(trait_method_param(param, ty));
        }
        groups.push(params);
    }
    Some(groups)
}

fn trait_method_param(param: &FnParam, ty: TypeKind) -> FunctionParam {
    let name = match param.pattern() {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            name.as_str()
        }
        _ => "_",
    };
    if param.is_rest() {
        FunctionParam::rest(name, ty)
    } else if param.default().is_some() {
        FunctionParam::defaulted(name, ty)
    } else {
        FunctionParam::required(name, ty)
    }
}

fn is_trait_receiver_param(param: &FnParam) -> bool {
    param.receiver_kind().is_some()
}

fn trait_bound_parts(bound: &TypeRef) -> Option<(&str, &[AssociatedTypeBinding])> {
    match bound {
        TypeRef::Path(path) => crate::types::direct_type_name(path).map(|name| (name, &[][..])),
        TypeRef::TraitBound(bound) => {
            crate::types::direct_type_name(bound.path()).map(|name| (name, bound.associated()))
        }
        TypeRef::Generic { base, .. } => {
            crate::types::direct_type_name(base).map(|name| (name, &[][..]))
        }
        _ => None,
    }
}

fn impl_head_label(item: &ImplItem) -> String {
    impl_trait_name(item).map_or_else(
        || format!("impl {}", item.target().value().canonical_label()),
        |trait_name| {
            format!(
                "impl {trait_name} for {}",
                item.target().value().canonical_label()
            )
        },
    )
}

fn impl_trait_name(item: &ImplItem) -> Option<&str> {
    item.trait_ref()
        .and_then(|reference| trait_bound_parts(reference.value()))
        .map(|(name, _)| name)
}

fn local_type_name(ty: &TypeKind) -> Option<&str> {
    match ty {
        TypeKind::Named(name) => Some(name),
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
