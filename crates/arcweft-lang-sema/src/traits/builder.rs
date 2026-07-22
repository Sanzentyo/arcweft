//! Trait and implementation collection from source-backed HIR declarations.

use super::{
    AssocEquality, AssociatedTypeAssignment, AssociatedTypeId, AssociatedTypeRequirement, ImplId,
    TraitCatalog, TraitDecl, TraitId, TraitImpl, TraitMethodBody, TraitMethodImpl,
    TraitMethodRequirement, TraitPredicateInput, TraitWitness, TraitWitnessId, as_impl_item,
    as_trait_item, collect_local_nominals, detached_generic_owner_from_range,
    generic_owner_for_range, generic_owner_for_signature, generic_type_scope, impl_head_label,
    impl_targets_overlap, impl_trait_name, local_type_name, method_signatures_compatible,
    nested_generic_type_scope, standard_iter, trait_bound_parts, trait_method_param_groups,
    type_kind_label,
};
use crate::diagnostics::{TraitDiagnostic, TypeCheckError};
use crate::nominal::{GenericTypeScope, SelfTypeScope};
use crate::types::{GenericTypeOwnerId, GenericTypeParameterId, TypeKind};
use arcweft_lang_hir::model::HirModule;
use arcweft_lang_syntax::ast::flow::{AuthoredExpr, Stmt};
use arcweft_lang_syntax::ast::items::{ImplItem, ImplMember, TraitItem, TraitMember};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::types::{
    AuthoredTypeRef, FnSignature, GenericParam, TypeRef, TypeRefNodePath, TypeRefNodeStep,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Builds a typed trait catalog through the same authored-type authority used
/// by ordinary checking.
pub(crate) fn collect_trait_catalog<R>(
    module: &HirModule,
    resolve: &mut R,
) -> (TraitCatalog, Vec<TypeCheckError>)
where
    R: FnMut(&CanonicalModulePath, &AuthoredTypeRef, &GenericTypeScope, SelfTypeScope) -> TypeKind,
{
    let mut builder = TraitCatalogBuilder::new(module, resolve);
    builder.collect_traits(module);
    builder.collect_impls(module);
    builder.finish()
}

/// Resolves function bounds through the normal nominal type authority before
/// selecting their trait declarations from the catalog.
pub(crate) fn trait_predicate_inputs_for_signature<R>(
    signature: &FnSignature,
    generic_scope: &GenericTypeScope,
    mut resolve: R,
) -> Vec<TraitPredicateInput>
where
    R: FnMut(&AuthoredTypeRef, &TypeRefNodePath) -> Option<TypeKind>,
{
    let mut inputs = Vec::new();
    for parameter in signature
        .generic_params()
        .iter()
        .filter_map(GenericParam::as_type_param)
    {
        let subject = TypeKind::GenericParam(
            generic_scope
                .binding(parameter.name())
                .expect("scope contains every declared generic type parameter")
                .id()
                .clone(),
        );
        for bound in parameter.bounds() {
            if let Some(input) = trait_predicate_input(subject.clone(), bound, &mut resolve) {
                inputs.push(input);
            }
        }
    }
    for clause in signature.where_clauses() {
        let Some(subject) = resolve(clause.subject(), &TypeRefNodePath::root()) else {
            continue;
        };
        for bound in clause.bounds() {
            if let Some(input) = trait_predicate_input(subject.clone(), bound, &mut resolve) {
                inputs.push(input);
            }
        }
    }
    inputs
}

fn trait_predicate_input(
    subject: TypeKind,
    bound: &AuthoredTypeRef,
    resolve: &mut impl FnMut(&AuthoredTypeRef, &TypeRefNodePath) -> Option<TypeKind>,
) -> Option<TraitPredicateInput> {
    if matches!(bound.value(), TypeRef::TraitBound(_)) {
        resolve(bound, &TypeRefNodePath::root())?;
    }
    let (trait_name, bindings) = trait_bound_parts(bound.value())?;
    let assoc_equalities = bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            let step = TypeRefNodeStep::AssociatedBinding(
                u16::try_from(index).expect("parser associated-binding cap fits u16"),
            );
            let path = bound
                .source()
                .nodes()
                .iter()
                .map(|(path, _)| path)
                .find(|path| path.steps() == [step])?;
            resolve(bound, path).map(|ty| AssocEquality::new(binding.name().as_str(), ty))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(TraitPredicateInput {
        subject,
        trait_name: trait_name.to_owned(),
        assoc_equalities,
    })
}

struct TraitCatalogBuilder<'a, R: ?Sized> {
    catalog: TraitCatalog,
    diagnostics: Vec<TypeCheckError>,
    local_nominals: HashSet<String>,
    next_assoc_id: usize,
    resolve: &'a mut R,
}

struct ImplMemberContext<'a> {
    module: &'a HirModule,
    declaration_module: &'a CanonicalModulePath,
    item: &'a ImplItem,
    generic_scope: &'a GenericTypeScope,
}

impl<'a, R> TraitCatalogBuilder<'a, R>
where
    R: FnMut(&CanonicalModulePath, &AuthoredTypeRef, &GenericTypeScope, SelfTypeScope) -> TypeKind
        + ?Sized,
{
    fn new(module: &HirModule, resolve: &'a mut R) -> Self {
        Self {
            catalog: TraitCatalog::default(),
            diagnostics: Vec::new(),
            local_nominals: collect_local_nominals(module),
            next_assoc_id: 0,
            resolve,
        }
    }

    fn collect_traits(&mut self, module: &HirModule) {
        standard_iter::install_standard_iterator_traits(&mut self.catalog, &mut self.next_assoc_id);
        for (_, item) in module
            .declarations_with_modules()
            .filter_map(|(owner, declaration)| as_trait_item(declaration).map(|item| (owner, item)))
        {
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

        for (declaration_module, item) in module
            .declarations_with_modules()
            .filter_map(|(owner, declaration)| as_trait_item(declaration).map(|item| (owner, item)))
        {
            let Some(id) = self.catalog.trait_id(item.name()) else {
                continue;
            };
            let supertraits = self.resolve_supertraits(module, declaration_module, item);
            let associated_types =
                self.collect_trait_associated_types(module, declaration_module, id, item);
            let methods = self.collect_trait_methods(module, declaration_module, id, item);
            if let Some(trait_decl) = self.catalog.traits.get_mut(id.index()) {
                trait_decl.supertraits = supertraits;
                trait_decl.associated_types = associated_types;
                trait_decl.methods = methods;
            }
        }
    }

    fn collect_impls(&mut self, module: &HirModule) {
        standard_iter::install_standard_iterator_impls(&mut self.catalog);
        for (declaration_module, item) in module
            .declarations_with_modules()
            .filter_map(|(owner, declaration)| as_impl_item(declaration).map(|item| (owner, item)))
        {
            self.collect_impl(module, declaration_module, item);
        }
    }

    fn collect_impl(
        &mut self,
        module: &HirModule,
        declaration_module: &CanonicalModulePath,
        item: &ImplItem,
    ) {
        if item.visibility().is_some() {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::pub_impl_unsupported(impl_head_label(item)),
            ));
        }

        let trait_name = item
            .trait_ref()
            .and_then(|reference| trait_bound_parts(reference.value()))
            .map(|(name, _)| name);
        let trait_id = trait_name.and_then(|name| self.resolve_trait_name(name));
        if item.trait_ref().is_some() && trait_id.is_none() {
            return;
        }

        let owner = module
            .project_source_span(declaration_module, *item.range())
            .map_or_else(
                || detached_generic_owner_from_range(*item.range()),
                GenericTypeOwnerId::AcceptedSource,
            );
        let generic_scope = generic_type_scope(module, declaration_module, item.generics(), &owner);
        let target = (self.resolve)(
            declaration_module,
            item.target(),
            &generic_scope,
            SelfTypeScope::Absent,
        );
        let self_scope = SelfTypeScope::Known(target.clone());
        if let Some(trait_ref) = item.trait_ref() {
            self.resolve_trait_bound_types(
                declaration_module,
                trait_ref,
                &generic_scope,
                self_scope.clone(),
            );
        }
        for clause in item.where_clauses() {
            (self.resolve)(
                declaration_module,
                clause.subject(),
                &generic_scope,
                self_scope.clone(),
            );
            for bound in clause.bounds() {
                self.resolve_trait_bound_types(
                    declaration_module,
                    bound,
                    &generic_scope,
                    self_scope.clone(),
                );
            }
        }

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

        self.collect_impl_members(
            module,
            declaration_module,
            item,
            &mut impl_decl,
            &generic_scope,
        );
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

    fn resolve_supertraits(
        &mut self,
        module: &HirModule,
        declaration_module: &CanonicalModulePath,
        item: &TraitItem,
    ) -> Vec<TraitId> {
        let self_parameter = GenericTypeParameterId::new(
            generic_owner_for_range(module, declaration_module, *item.range()),
            u16::MAX,
        );
        let generics = GenericTypeScope::empty();
        for bound in item.supertraits() {
            self.resolve_trait_bound_types(
                declaration_module,
                bound,
                &generics,
                SelfTypeScope::Known(TypeKind::GenericParam(self_parameter.clone())),
            );
        }
        item.supertraits()
            .iter()
            .filter_map(|bound| {
                trait_bound_parts(bound.value()).and_then(|(name, _)| self.resolve_trait_name(name))
            })
            .collect()
    }

    fn collect_trait_associated_types(
        &mut self,
        module: &HirModule,
        declaration_module: &CanonicalModulePath,
        trait_id: TraitId,
        item: &TraitItem,
    ) -> Vec<AssociatedTypeRequirement> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        let self_parameter = GenericTypeParameterId::new(
            generic_owner_for_range(module, declaration_module, *item.range()),
            u16::MAX,
        );
        let generics = GenericTypeScope::empty();
        let self_scope = SelfTypeScope::Known(TypeKind::GenericParam(self_parameter));
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
            if let Some(value) = value {
                (self.resolve)(declaration_module, value, &generics, self_scope.clone());
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
        module: &HirModule,
        declaration_module: &CanonicalModulePath,
        trait_id: TraitId,
        item: &TraitItem,
    ) -> Vec<TraitMethodRequirement> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        let self_owner = generic_owner_for_range(module, declaration_module, *item.range());
        let self_parameter = GenericTypeParameterId::new(self_owner, u16::MAX);
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
            let method_owner =
                generic_owner_for_signature(module, declaration_module, signature, *item.range());
            let generic_scope = generic_type_scope(
                module,
                declaration_module,
                signature.generic_params(),
                &method_owner,
            );
            let self_scope = SelfTypeScope::Known(TypeKind::GenericParam(self_parameter.clone()));
            for parameter in signature
                .generic_params()
                .iter()
                .filter_map(GenericParam::as_type_param)
            {
                for bound in parameter.bounds() {
                    self.resolve_trait_bound_types(
                        declaration_module,
                        bound,
                        &generic_scope,
                        self_scope.clone(),
                    );
                }
            }
            for clause in signature.where_clauses() {
                (self.resolve)(
                    declaration_module,
                    clause.subject(),
                    &generic_scope,
                    self_scope.clone(),
                );
                for bound in clause.bounds() {
                    self.resolve_trait_bound_types(
                        declaration_module,
                        bound,
                        &generic_scope,
                        self_scope.clone(),
                    );
                }
            }
            let Some(param_groups) = trait_method_param_groups(signature, |ty| {
                Some((self.resolve)(
                    declaration_module,
                    ty,
                    &generic_scope,
                    self_scope.clone(),
                ))
            }) else {
                continue;
            };
            let return_type = signature.return_type().map_or(TypeKind::Unit, |ty| {
                (self.resolve)(declaration_module, ty, &generic_scope, self_scope.clone())
            });
            out.push(TraitMethodRequirement {
                trait_id,
                name: signature.name().to_owned(),
                signature: signature.clone(),
                self_parameter: self_parameter.clone(),
                param_groups,
                return_type,
            });
        }
        out
    }

    fn collect_impl_members(
        &mut self,
        module: &HirModule,
        declaration_module: &CanonicalModulePath,
        item: &ImplItem,
        impl_decl: &mut TraitImpl,
        generic_scope: &GenericTypeScope,
    ) {
        let context = ImplMemberContext {
            module,
            declaration_module,
            item,
            generic_scope,
        };
        let mut assoc_seen = BTreeSet::new();
        let mut method_seen = BTreeSet::new();
        for member in item.members() {
            match member {
                ImplMember::AssociatedType {
                    name,
                    params,
                    value,
                } => self.collect_associated_type_member(
                    &context,
                    impl_decl,
                    &mut assoc_seen,
                    name,
                    params,
                    value,
                ),
                ImplMember::Function {
                    signature,
                    body_statements,
                    body_value,
                    ..
                } => self.collect_method_member(
                    &context,
                    impl_decl,
                    &mut method_seen,
                    signature,
                    body_statements,
                    body_value.as_deref(),
                ),
                ImplMember::Raw(raw) => {
                    self.diagnostics.push(TypeCheckError::trait_diagnostic(
                        TraitDiagnostic::raw_impl_member(impl_head_label(item), raw),
                    ));
                }
            }
        }
    }

    fn collect_associated_type_member(
        &mut self,
        context: &ImplMemberContext<'_>,
        impl_decl: &mut TraitImpl,
        seen: &mut BTreeSet<String>,
        name: &str,
        params: &[String],
        value: &AuthoredTypeRef,
    ) {
        if impl_decl.trait_id.is_none() {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::associated_type_in_inherent_impl(name),
            ));
        }
        let owner = impl_trait_name(context.item).unwrap_or("<inherent>");
        if !params.is_empty() {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::associated_type_constructor_unsupported(owner, name),
            ));
        }
        if !seen.insert(name.to_owned()) {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::duplicate_associated_type_assignment(owner, name),
            ));
        }
        let value = (self.resolve)(
            context.declaration_module,
            value,
            context.generic_scope,
            SelfTypeScope::Known(impl_decl.target.clone()),
        );
        impl_decl.associated_types.insert(
            name.to_owned(),
            AssociatedTypeAssignment {
                name: name.to_owned(),
                value,
            },
        );
    }

    fn collect_method_member(
        &mut self,
        context: &ImplMemberContext<'_>,
        impl_decl: &mut TraitImpl,
        seen: &mut BTreeSet<String>,
        signature: &FnSignature,
        body_statements: &[Stmt],
        body_value: Option<&AuthoredExpr>,
    ) {
        if !seen.insert(signature.name().to_owned()) {
            self.diagnostics.push(TypeCheckError::trait_diagnostic(
                TraitDiagnostic::duplicate_method(
                    impl_trait_name(context.item).unwrap_or("<inherent>"),
                    signature.name(),
                ),
            ));
        }
        let method_owner = generic_owner_for_signature(
            context.module,
            context.declaration_module,
            signature,
            *context.item.range(),
        );
        let method_generics = nested_generic_type_scope(
            context.module,
            context.declaration_module,
            signature.generic_params(),
            &method_owner,
            context.generic_scope,
        );
        let self_scope = SelfTypeScope::Known(impl_decl.target.clone());
        self.resolve_method_bounds(
            context.declaration_module,
            signature,
            &method_generics,
            &self_scope,
        );
        let Some(param_groups) = trait_method_param_groups(signature, |ty| {
            Some((self.resolve)(
                context.declaration_module,
                ty,
                &method_generics,
                self_scope.clone(),
            ))
        }) else {
            return;
        };
        let return_type = signature.return_type().map_or(TypeKind::Unit, |ty| {
            (self.resolve)(context.declaration_module, ty, &method_generics, self_scope)
        });
        impl_decl.methods.insert(
            signature.name().to_owned(),
            TraitMethodImpl {
                trait_id: impl_decl.trait_id,
                signature: signature.clone(),
                param_groups,
                return_type,
                body: TraitMethodBody::new(body_statements, body_value),
            },
        );
    }

    fn resolve_method_bounds(
        &mut self,
        declaration_module: &CanonicalModulePath,
        signature: &FnSignature,
        generic_scope: &GenericTypeScope,
        self_scope: &SelfTypeScope,
    ) {
        for parameter in signature
            .generic_params()
            .iter()
            .filter_map(GenericParam::as_type_param)
        {
            for bound in parameter.bounds() {
                self.resolve_trait_bound_types(
                    declaration_module,
                    bound,
                    generic_scope,
                    self_scope.clone(),
                );
            }
        }
        for clause in signature.where_clauses() {
            (self.resolve)(
                declaration_module,
                clause.subject(),
                generic_scope,
                self_scope.clone(),
            );
            for bound in clause.bounds() {
                self.resolve_trait_bound_types(
                    declaration_module,
                    bound,
                    generic_scope,
                    self_scope.clone(),
                );
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
            if !method_signatures_compatible(method, actual, impl_decl) {
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
            self.catalog.inherent_methods.insert(
                (impl_decl.target.clone(), name.clone()),
                (impl_decl.id, method.clone()),
            );
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

    fn resolve_trait_bound_types(
        &mut self,
        declaration_module: &CanonicalModulePath,
        bound: &AuthoredTypeRef,
        generic_scope: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) {
        if matches!(bound.value(), TypeRef::TraitBound(_)) {
            (self.resolve)(declaration_module, bound, generic_scope, self_scope);
        }
    }

    fn finish(self) -> (TraitCatalog, Vec<TypeCheckError>) {
        (self.catalog, self.diagnostics)
    }
}
