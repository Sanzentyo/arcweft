//! Source-backed nominal resolution owned by the normal semantic checker.

use std::collections::HashMap;

use arcweft_lang_hir::symbol::nominal::SourceBackedTypeRef;
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        module_path::CanonicalModulePath,
        pattern::{Pattern, VariantPatternPayload},
    },
    types::{
        AuthoredTypeRef, FnParam, FnSignature, GenericParam, TypeRef, TypeRefNodePath,
        TypeRefNodeStep,
    },
};

use crate::{
    diagnostics::TypeCheckError,
    nominal::{
        GenericTypeBinding, GenericTypeScope, NominalResolutionLimits, ResolvedTypeProduct,
        SelfTypeScope, TypeResolutionInput, TypeResolutionReport, TypeSourceEvidence,
        resolve_type_ref,
    },
    traits::detached_generic_owner_from_range,
    types::{GenericTypeOwnerId, TypeKind},
};

use super::TypeChecker;

impl TypeChecker<'_> {
    pub(super) fn resolve_authored_type_in_module(
        &mut self,
        module: &CanonicalModulePath,
        authored: &AuthoredTypeRef,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> TypeKind {
        let previous_module = self.current_module.replace(module.clone());
        let resolved = self.resolve_authored_type(authored, generics, self_scope);
        self.current_module = previous_module;
        resolved
    }

    pub(super) fn generic_scope_for_signature(
        &mut self,
        signature: &FnSignature,
        owner: &GenericTypeOwnerId,
    ) -> GenericTypeScope {
        self.generic_scope_for_parameters(signature.generic_params(), owner)
    }

    pub(super) fn generic_scope_for_parameters(
        &mut self,
        parameters: &[GenericParam],
        owner: &GenericTypeOwnerId,
    ) -> GenericTypeScope {
        let bindings = parameters
            .iter()
            .filter_map(GenericParam::as_type_param)
            .enumerate()
            .filter_map(|(ordinal, parameter)| {
                let ordinal = u16::try_from(ordinal).ok()?;
                let source = self.type_source_evidence(parameter.name_range());
                Some(GenericTypeBinding::new(
                    crate::types::GenericTypeParameterId::new(owner.clone(), ordinal),
                    parameter.name().clone(),
                    source,
                ))
            })
            .collect::<Vec<_>>();

        match GenericTypeScope::try_new(bindings) {
            Ok(scope) => scope,
            Err(error) => {
                self.errors.push(TypeCheckError::new(format!(
                    "duplicate generic type parameter `{}` at {:?}",
                    error.name(),
                    error.duplicate().local()
                )));
                GenericTypeScope::empty()
            }
        }
    }

    pub(super) fn nested_generic_scope_for_signature(
        &mut self,
        signature: &FnSignature,
        owner: &GenericTypeOwnerId,
        parent: &GenericTypeScope,
    ) -> GenericTypeScope {
        let child = self.generic_scope_for_signature(signature, owner);
        let mut bindings = child.bindings().cloned().collect::<Vec<_>>();
        bindings.extend(
            parent
                .bindings()
                .filter(|parent_binding| child.binding(parent_binding.name()).is_none())
                .cloned(),
        );
        GenericTypeScope::try_new(bindings)
            .expect("child generic names shadow parent bindings before scope construction")
    }

    pub(super) fn resolve_generic_parameter_bounds(
        &mut self,
        parameters: &[GenericParam],
        generics: &GenericTypeScope,
        self_scope: &SelfTypeScope,
    ) {
        for parameter in parameters.iter().filter_map(GenericParam::as_type_param) {
            for bound in parameter.bounds() {
                self.resolve_trait_bound_types(bound, generics, self_scope.clone());
            }
        }
    }

    pub(super) fn resolve_trait_bound_types(
        &mut self,
        bound: &AuthoredTypeRef,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) {
        if matches!(bound.value(), TypeRef::TraitBound(_)) {
            self.resolve_authored_type(bound, generics, self_scope);
        }
    }

    pub(super) fn generic_owner_for_range(&self, range: TextRange) -> GenericTypeOwnerId {
        self.source_span_for_current_range(range).map_or_else(
            || detached_generic_owner_from_range(range),
            GenericTypeOwnerId::AcceptedSource,
        )
    }

    pub(super) fn generic_owner_for_signature(
        &self,
        signature: &FnSignature,
        fallback: TextRange,
    ) -> GenericTypeOwnerId {
        let range =
            signature
                .generic_params()
                .first()
                .map_or(fallback, |parameter| match parameter {
                    GenericParam::Lifetime(lifetime) => lifetime.range(),
                    GenericParam::Type(parameter) => parameter.range(),
                });
        self.generic_owner_for_range(range)
    }

    pub(super) fn project_nominal_owner_for_name(
        &self,
        name_range: TextRange,
    ) -> Option<GenericTypeOwnerId> {
        let source = self.source_span_for_current_range(name_range)?;
        self.project_symbols?
            .nominal_symbols()
            .find(|declaration| declaration.source().name() == &source)
            .map(|declaration| GenericTypeOwnerId::Nominal(declaration.id().clone()))
    }

    pub(super) fn resolve_authored_type(
        &mut self,
        authored: &AuthoredTypeRef,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> TypeKind {
        self.resolve_authored_type_report(authored, generics, self_scope)
            .outcome()
            .product()
            .recovered()
            .clone()
    }

    pub(super) fn resolve_authored_type_node(
        &mut self,
        authored: &AuthoredTypeRef,
        node: &TypeRefNodePath,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> Option<TypeKind> {
        let report = self.resolve_authored_type_report(authored, generics, self_scope);
        report
            .outcome()
            .product()
            .nodes()
            .iter()
            .find(|resolved| resolved.node() == node)
            .and_then(|resolved| resolved.recovered().cloned())
    }

    fn resolve_authored_type_report(
        &mut self,
        authored: &AuthoredTypeRef,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> TypeResolutionReport {
        let accepted = self
            .current_module
            .as_ref()
            .zip(self.project_symbols)
            .zip(self.registered_environment)
            .and_then(|((module, symbols), environment)| {
                self.checked_module
                    .project_source_document(module)
                    .map(|document| (module, symbols, environment, document))
            });

        let report = if let Some((module, symbols, environment, document)) = accepted {
            let source_backed =
                SourceBackedTypeRef::try_bind(authored.clone(), document, document.identity())
                    .expect(
                        "source-bound HIR type ranges remain valid for their accepted document",
                    );
            let root = source_backed
                .spans()
                .source_at(&TypeRefNodePath::root())
                .expect("accepted type source maps contain their root")
                .whole()
                .clone();
            let already_recorded = self.nominal_resolutions.report(&root).is_some();
            let input = TypeResolutionInput::accepted(
                &source_backed,
                module,
                symbols,
                environment.nominal_world(),
                generics,
                self_scope,
                NominalResolutionLimits::PRODUCTION,
            )
            .expect("registered type checking receives one validated world and source revision");
            let report = self
                .nominal_resolution_cache
                .resolve(&input)
                .expect("validated production nominal limits and registered owner integrity");
            if !already_recorded {
                self.errors.extend(
                    report
                        .diagnostics()
                        .iter()
                        .cloned()
                        .map(TypeCheckError::nominal),
                );
                if let Err(error) = self
                    .nominal_resolutions
                    .record(root.clone(), report.as_ref().clone())
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "nominal resolution index rejected an accepted fact: {error:?}"
                    )));
                }
            }
            if self.checked_anonymous_choice_roots.insert(root) {
                self.record_anonymous_choice_duplicates(
                    authored.value(),
                    report.outcome().product(),
                );
            }
            return report.as_ref().clone();
        } else {
            resolve_type_ref(&TypeResolutionInput::detached(
                authored,
                self.current_module.as_ref(),
                self.env,
                generics,
                self_scope,
                NominalResolutionLimits::PRODUCTION,
            ))
            .expect("compiled detached nominal limits are valid")
        };

        self.errors.extend(
            report
                .diagnostics()
                .iter()
                .cloned()
                .map(TypeCheckError::nominal),
        );
        self.record_anonymous_choice_duplicates(authored.value(), report.outcome().product());
        report
    }

    fn record_anonymous_choice_duplicates(
        &mut self,
        authored: &TypeRef,
        product: &ResolvedTypeProduct,
    ) {
        let mut path = Vec::new();
        collect_anonymous_choice_duplicates(authored, product, &mut path, &mut self.errors);
    }

    pub(super) fn resolve_active_authored_type(&mut self, authored: &AuthoredTypeRef) -> TypeKind {
        let generics = self.active_generic_scope.clone();
        let self_scope = self.active_self_scope.clone();
        self.resolve_authored_type(authored, &generics, self_scope)
    }

    pub(super) fn resolve_pattern_type_annotations(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Typed { ty, .. } => {
                self.resolve_active_authored_type(ty);
            }
            Pattern::Tuple(items)
            | Pattern::BracketSeq { items, .. }
            | Pattern::Variant {
                payload: Some(VariantPatternPayload::Tuple(items)),
                ..
            } => {
                for item in items {
                    self.resolve_pattern_type_annotations(item);
                }
            }
            Pattern::Record { fields, .. }
            | Pattern::Variant {
                payload: Some(VariantPatternPayload::Record { fields, .. }),
                ..
            } => {
                for field in fields {
                    self.resolve_pattern_type_annotations(field.pattern());
                }
            }
            Pattern::Whole { pattern, .. } => self.resolve_pattern_type_annotations(pattern),
            Pattern::Ident(_)
            | Pattern::MutIdent(_)
            | Pattern::Literal(_)
            | Pattern::Entity(_)
            | Pattern::Variant { payload: None, .. }
            | Pattern::Discard
            | Pattern::Raw(_) => {}
        }
    }

    pub(super) fn resolve_detached_authored_type(
        &mut self,
        authored: &AuthoredTypeRef,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> TypeKind {
        let report = resolve_type_ref(&TypeResolutionInput::detached(
            authored,
            self.current_module.as_ref(),
            self.env,
            generics,
            self_scope,
            NominalResolutionLimits::PRODUCTION,
        ))
        .expect("compiled detached nominal limits are valid");
        self.errors.extend(
            report
                .diagnostics()
                .iter()
                .cloned()
                .map(TypeCheckError::nominal),
        );
        report.outcome().product().recovered().clone()
    }

    pub(super) fn resolve_function_param_type(
        &mut self,
        param: &FnParam,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> TypeKind {
        param.ty().map_or(TypeKind::Unit, |authored| {
            self.resolve_authored_type(authored, generics, self_scope)
        })
    }

    pub(super) fn resolve_function_param_binding_type(
        &mut self,
        param: &FnParam,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> TypeKind {
        let ty = self.resolve_function_param_type(param, generics, self_scope);
        if param.is_rest() {
            TypeKind::Vec(Box::new(ty))
        } else {
            ty
        }
    }

    pub(super) fn resolve_function_signature(
        &mut self,
        signature: &FnSignature,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
    ) -> crate::env::FunctionSignature {
        self.resolve_generic_parameter_bounds(signature.generic_params(), generics, &self_scope);
        let parameter_types = signature
            .param_groups()
            .iter()
            .map(|group| {
                group
                    .params()
                    .iter()
                    .map(|parameter| {
                        self.resolve_function_param_type(parameter, generics, self_scope.clone())
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let return_type = signature.return_type().map_or(TypeKind::Unit, |authored| {
            self.resolve_authored_type(authored, generics, self_scope)
        });
        super::function_signature_from_resolved(
            signature,
            &parameter_types,
            return_type,
            super::NominalTypeContext::new(
                &self.nominal_fields,
                &self.nominal_variant_payloads,
                &self.project_nominal_shapes,
                self.env,
            ),
        )
    }

    fn type_source_evidence(&self, range: TextRange) -> TypeSourceEvidence {
        self.source_span_for_current_range(range).map_or_else(
            || TypeSourceEvidence::detached(range),
            |source| TypeSourceEvidence::accepted(range, source),
        )
    }
}

fn collect_anonymous_choice_duplicates(
    authored: &TypeRef,
    product: &ResolvedTypeProduct,
    path: &mut Vec<TypeRefNodeStep>,
    errors: &mut Vec<TypeCheckError>,
) {
    match authored {
        TypeRef::Choice(alternatives) => {
            collect_choice_duplicates(alternatives, product, path, errors);
        }
        TypeRef::Tuple(items) => {
            for (index, item) in items.iter().enumerate() {
                path.push(TypeRefNodeStep::TupleItem(
                    u16::try_from(index).expect("parser caps tuple items"),
                ));
                collect_anonymous_choice_duplicates(item, product, path, errors);
                path.pop();
            }
        }
        TypeRef::Function {
            params,
            return_type,
            ..
        } => {
            for (index, parameter) in params.iter().enumerate() {
                path.push(TypeRefNodeStep::FunctionParameter(
                    u16::try_from(index).expect("parser caps function parameters"),
                ));
                collect_anonymous_choice_duplicates(parameter, product, path, errors);
                path.pop();
            }
            path.push(TypeRefNodeStep::FunctionReturn);
            collect_anonymous_choice_duplicates(return_type, product, path, errors);
            path.pop();
        }
        TypeRef::Generic { args, .. } => {
            for (index, argument) in args.iter().enumerate() {
                path.push(TypeRefNodeStep::GenericArgument(
                    u16::try_from(index).expect("parser caps generic arguments"),
                ));
                collect_anonymous_choice_duplicates(argument, product, path, errors);
                path.pop();
            }
        }
        TypeRef::TraitBound(bound) => {
            for (index, argument) in bound.args().iter().enumerate() {
                path.push(TypeRefNodeStep::TraitArgument(
                    u16::try_from(index).expect("parser caps trait arguments"),
                ));
                collect_anonymous_choice_duplicates(argument, product, path, errors);
                path.pop();
            }
            for (index, binding) in bound.associated().iter().enumerate() {
                path.push(TypeRefNodeStep::AssociatedBinding(
                    u16::try_from(index).expect("parser caps associated bindings"),
                ));
                collect_anonymous_choice_duplicates(binding.value(), product, path, errors);
                path.pop();
            }
        }
        TypeRef::Projection { subject, .. } => {
            path.push(TypeRefNodeStep::ProjectionSubject);
            collect_anonymous_choice_duplicates(subject, product, path, errors);
            path.pop();
        }
        TypeRef::Reference(reference) => {
            path.push(TypeRefNodeStep::ReferenceReferent);
            collect_anonymous_choice_duplicates(reference.referent(), product, path, errors);
            path.pop();
        }
        TypeRef::Slice(item) => {
            path.push(TypeRefNodeStep::SliceItem);
            collect_anonymous_choice_duplicates(item, product, path, errors);
            path.pop();
        }
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => {}
    }
}

fn collect_choice_duplicates(
    alternatives: &[TypeRef],
    product: &ResolvedTypeProduct,
    path: &mut Vec<TypeRefNodeStep>,
    errors: &mut Vec<TypeCheckError>,
) {
    let mut normalized = HashMap::<TypeKind, String>::new();
    for (index, alternative) in alternatives.iter().enumerate() {
        path.push(TypeRefNodeStep::ChoiceAlternative(
            u16::try_from(index).expect("parser caps choice alternatives"),
        ));
        collect_anonymous_choice_duplicates(alternative, product, path, errors);
        let resolved = product
            .nodes()
            .iter()
            .find(|node| node.node().steps() == path.as_slice())
            .and_then(|node| node.recovered());
        if let Some(resolved) = resolved
            && !resolved.contains_nominal_poison()
        {
            let source_label = super::helpers::type_ref_label(alternative);
            if let Some(previous) = normalized.insert(resolved.clone(), source_label.clone()) {
                let message = if previous == source_label {
                    format!("duplicate alternative `{source_label}` in anonymous sum")
                } else {
                    format!(
                        "anonymous sum alternatives `{previous}` and `{source_label}` erase to the same type `{}`",
                        super::helpers::type_kind_label(resolved)
                    )
                };
                errors.push(TypeCheckError::new(message));
            }
        }
        path.pop();
    }
}
