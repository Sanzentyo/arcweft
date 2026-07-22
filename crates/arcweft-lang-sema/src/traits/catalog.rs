//! Trait catalog lookup, conformance, projection, and iterator selection.

use super::{
    AssocEquality, AssociatedTypeRequirement, ImplId, IntoIteratorResolution,
    IntoIteratorResolutionError, IntoIteratorResolutionKind, ProjectionError, ProjectionResolution,
    TraitCatalog, TraitConformanceResolution, TraitDecl, TraitId, TraitImpl, TraitMethodCandidate,
    TraitMethodForWitness, TraitMethodImpl, TraitMethodRequirement, TraitMethodResolution,
    TraitPredicate, TraitPredicateInput, TraitWitness, TraitWitnessId, conformance_from_impl,
    conformance_from_predicate, instantiate_trait_requirement_params,
    instantiate_trait_requirement_type, standard_iter,
};
use crate::types::TypeKind;
use std::collections::BTreeSet;

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

    pub(crate) fn predicates_from_inputs(
        &self,
        inputs: impl IntoIterator<Item = TraitPredicateInput>,
    ) -> Vec<TraitPredicate> {
        inputs
            .into_iter()
            .filter_map(|input| {
                self.trait_id(&input.trait_name).map(|trait_id| {
                    TraitPredicate::new(input.subject, trait_id, input.assoc_equalities)
                })
            })
            .collect()
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
            return TraitMethodResolution::Inherent {
                implementation: method.0,
                method: method.1,
            };
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
                let return_type = instantiate_trait_requirement_type(
                    &requirement.return_type,
                    &requirement.self_parameter,
                    receiver,
                    predicate.assoc_equalities(),
                );
                let param_groups = instantiate_trait_requirement_params(
                    &requirement.param_groups,
                    &requirement.self_parameter,
                    receiver,
                    predicate.assoc_equalities(),
                );
                candidates.push((
                    None,
                    requirement.trait_id,
                    TraitMethodImpl {
                        trait_id: Some(requirement.trait_id),
                        signature: requirement.signature.clone(),
                        param_groups,
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

    /// Resolves every associated-type projection carried by a method result.
    ///
    /// Trait selection and projection are catalog-owned semantic operations.
    /// Call resolvers and the checker consume this one result instead of
    /// recursively reinterpreting projection-bearing types independently.
    pub fn resolve_type_projections(
        &self,
        ty: TypeKind,
        predicates: &[TraitPredicate],
    ) -> Result<TypeKind, ProjectionError> {
        match ty {
            TypeKind::Projection { subject, assoc, .. } => {
                match self.resolve_projection(&subject, &assoc, predicates)? {
                    ProjectionResolution::Resolved(ty) | ProjectionResolution::Deferred(ty) => {
                        Ok(ty)
                    }
                }
            }
            TypeKind::Vec(inner) => Ok(TypeKind::Vec(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::Seq(inner) => Ok(TypeKind::Seq(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::Range(inner) => Ok(TypeKind::Range(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::Slice(inner) => Ok(TypeKind::Slice(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::Option(inner) => Ok(TypeKind::Option(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::Probe(inner) => Ok(TypeKind::Probe(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::ThreadHandle(inner) => Ok(TypeKind::ThreadHandle(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::Shared(inner) => Ok(TypeKind::Shared(
                self.resolve_projection_box(*inner, predicates)?,
            )),
            TypeKind::BorrowRef {
                kind,
                lifetime,
                inner,
            } => Ok(TypeKind::BorrowRef {
                kind,
                lifetime,
                inner: Box::new(self.resolve_type_projections(*inner, predicates)?),
            }),
            TypeKind::IteratorState { family, item } => Ok(TypeKind::IteratorState {
                family,
                item: Box::new(self.resolve_type_projections(*item, predicates)?),
            }),
            TypeKind::Need { ready, error } => Ok(TypeKind::Need {
                ready: Box::new(self.resolve_type_projections(*ready, predicates)?),
                error: Box::new(self.resolve_type_projections(*error, predicates)?),
            }),
            TypeKind::Stream { item, error } => Ok(TypeKind::Stream {
                item: Box::new(self.resolve_type_projections(*item, predicates)?),
                error: Box::new(self.resolve_type_projections(*error, predicates)?),
            }),
            TypeKind::Source { item, error } => Ok(TypeKind::Source {
                item: Box::new(self.resolve_type_projections(*item, predicates)?),
                error: Box::new(self.resolve_type_projections(*error, predicates)?),
            }),
            TypeKind::Result { ok, error } => Ok(TypeKind::Result {
                ok: Box::new(self.resolve_type_projections(*ok, predicates)?),
                error: Box::new(self.resolve_type_projections(*error, predicates)?),
            }),
            TypeKind::Map { kind, key, value } => Ok(TypeKind::Map {
                kind,
                key: Box::new(self.resolve_type_projections(*key, predicates)?),
                value: Box::new(self.resolve_type_projections(*value, predicates)?),
            }),
            TypeKind::Array { item, len } => Ok(TypeKind::Array {
                item: Box::new(self.resolve_type_projections(*item, predicates)?),
                len,
            }),
            TypeKind::Function {
                params,
                return_type,
                effects,
            } => Ok(TypeKind::function_with_effects(
                params
                    .into_iter()
                    .map(|param| self.resolve_type_projections(param, predicates))
                    .collect::<Result<Vec<_>, _>>()?,
                self.resolve_type_projections(*return_type, predicates)?,
                effects,
            )),
            TypeKind::ProjectNominal(nominal) => {
                self.resolve_project_nominal_projections(&nominal, predicates)
            }
            TypeKind::AcceptedNominal(nominal) => {
                self.resolve_accepted_nominal_projections(&nominal, predicates)
            }
            TypeKind::OpenNominal(nominal) => {
                self.resolve_open_nominal_projections(&nominal, predicates)
            }
            TypeKind::Tuple(items) => Ok(TypeKind::Tuple(
                self.resolve_projection_items(items, predicates)?,
            )),
            TypeKind::Choice(items) => Ok(TypeKind::Choice(
                self.resolve_projection_items(items, predicates)?,
            )),
            other => Ok(other),
        }
    }

    fn resolve_projection_box(
        &self,
        inner: TypeKind,
        predicates: &[TraitPredicate],
    ) -> Result<Box<TypeKind>, ProjectionError> {
        self.resolve_type_projections(inner, predicates)
            .map(Box::new)
    }

    fn resolve_projection_items(
        &self,
        items: Vec<TypeKind>,
        predicates: &[TraitPredicate],
    ) -> Result<Vec<TypeKind>, ProjectionError> {
        items
            .into_iter()
            .map(|item| self.resolve_type_projections(item, predicates))
            .collect()
    }

    fn resolve_project_nominal_projections(
        &self,
        nominal: &crate::types::ProjectNominalType,
        predicates: &[TraitPredicate],
    ) -> Result<TypeKind, ProjectionError> {
        Ok(TypeKind::ProjectNominal(
            crate::types::ProjectNominalType::new(
                nominal.declaration().clone(),
                self.resolve_projection_items(nominal.arguments().to_vec(), predicates)?,
            ),
        ))
    }

    fn resolve_accepted_nominal_projections(
        &self,
        nominal: &crate::types::AcceptedNominalType,
        predicates: &[TraitPredicate],
    ) -> Result<TypeKind, ProjectionError> {
        Ok(TypeKind::AcceptedNominal(
            crate::types::AcceptedNominalType::new(
                nominal.declaration().clone(),
                self.resolve_projection_items(nominal.arguments().to_vec(), predicates)?,
            ),
        ))
    }

    fn resolve_open_nominal_projections(
        &self,
        nominal: &crate::types::OpenNominalType,
        predicates: &[TraitPredicate],
    ) -> Result<TypeKind, ProjectionError> {
        Ok(TypeKind::OpenNominal(crate::types::OpenNominalType::new(
            nominal.rule().clone(),
            nominal.path().clone(),
            self.resolve_projection_items(nominal.arguments().to_vec(), predicates)?,
        )))
    }

    fn trait_has_assoc(&self, trait_id: TraitId, assoc: &str) -> bool {
        self.inherited_associated_types(trait_id)
            .iter()
            .any(|requirement| requirement.name == assoc)
    }

    pub(super) fn inherited_associated_types(
        &self,
        trait_id: TraitId,
    ) -> Vec<AssociatedTypeRequirement> {
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

    pub(super) fn inherited_methods(&self, trait_id: TraitId) -> Vec<TraitMethodRequirement> {
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
