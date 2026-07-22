//! Source-backed nominal resolution for accepted project callable signatures.
use arcweft_lang_hir::{
    callable_source::HirCallableSignatureSource,
    model::HirModule,
    project::HirProject,
    symbol::{ProjectSymbolTable, nominal::SourceBackedTypeRef},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    types::{AuthoredTypeRef, FnSignature, GenericParam, TypeRef},
};

use crate::{
    checker::{NominalTypeContext, signature::function_signature_from_resolved},
    env::FunctionSignature,
    nominal::{
        CheckedTypeReferenceCache, GenericTypeBinding, GenericTypeScope, NominalResolutionIndex,
        NominalResolutionLimits, ResolvedTypeRefOutcome, SelfTypeScope, TypeResolutionInput,
        TypeSourceEvidence,
    },
    registration::AcceptedNominalWorld,
    types::{GenericTypeOwnerId, GenericTypeParameterId, TypeKind},
};

use super::CallableCatalogBuildError;

pub(super) struct ResolvedProjectSignature {
    pub(super) parameter_types: Vec<Vec<TypeKind>>,
    pub(super) return_type: TypeKind,
}

pub(super) struct ProjectSignatureResolver<'a> {
    project: &'a HirProject,
    symbols: &'a ProjectSymbolTable,
    nominal_world: &'a AcceptedNominalWorld,
    resolutions: &'a mut NominalResolutionIndex,
    cache: &'a mut CheckedTypeReferenceCache,
}

impl<'a> ProjectSignatureResolver<'a> {
    pub(super) fn new(
        project: &'a HirProject,
        symbols: &'a ProjectSymbolTable,
        nominal_world: &'a AcceptedNominalWorld,
        resolutions: &'a mut NominalResolutionIndex,
        cache: &'a mut CheckedTypeReferenceCache,
    ) -> Self {
        Self {
            project,
            symbols,
            nominal_world,
            resolutions,
            cache,
        }
    }

    pub(super) fn resolve_project_signature(
        &mut self,
        source: &HirCallableSignatureSource,
    ) -> Result<ResolvedProjectSignature, CallableCatalogBuildError> {
        let hir = self.project.module(source.module()).ok_or_else(|| {
            CallableCatalogBuildError::MissingProjectModuleSource {
                module: source.module().clone(),
            }
        })?;
        self.resolve_signature_types(
            source.module(),
            hir,
            source.signature(),
            &GenericTypeOwnerId::Callable(source.declaration().clone()),
        )
    }

    pub(super) fn resolve_function_signature(
        &mut self,
        module: &CanonicalModulePath,
        hir: &HirModule,
        signature: &FnSignature,
        owner: &GenericTypeOwnerId,
    ) -> Result<FunctionSignature, CallableCatalogBuildError> {
        let resolved = self.resolve_signature_types(module, hir, signature, owner)?;
        Ok(function_signature_from_resolved(
            signature,
            &resolved.parameter_types,
            resolved.return_type,
            NominalTypeContext::empty(),
        ))
    }

    fn resolve_signature_types(
        &mut self,
        module: &CanonicalModulePath,
        hir: &HirModule,
        signature: &FnSignature,
        owner: &GenericTypeOwnerId,
    ) -> Result<ResolvedProjectSignature, CallableCatalogBuildError> {
        let generics = Self::generic_scope(hir, signature.generic_params(), owner)?;
        for parameter in signature
            .generic_params()
            .iter()
            .filter_map(GenericParam::as_type_param)
        {
            for bound in parameter.bounds() {
                self.resolve_trait_bound_types(module, hir, bound, &generics)?;
            }
        }
        for predicate in signature.where_clauses() {
            self.resolve_authored(module, hir, predicate.subject(), &generics)?;
            for bound in predicate.bounds() {
                self.resolve_trait_bound_types(module, hir, bound, &generics)?;
            }
        }
        let parameter_types = signature
            .param_groups()
            .iter()
            .map(|group| {
                group
                    .params()
                    .iter()
                    .map(|parameter| {
                        parameter.ty().map_or(Ok(TypeKind::Unit), |authored| {
                            self.resolve_authored(module, hir, authored, &generics)
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let return_type = signature
            .return_type()
            .map_or(Ok(TypeKind::Unit), |authored| {
                self.resolve_authored(module, hir, authored, &generics)
            })?;
        Ok(ResolvedProjectSignature {
            parameter_types,
            return_type,
        })
    }

    fn resolve_trait_bound_types(
        &mut self,
        module: &CanonicalModulePath,
        hir: &HirModule,
        bound: &AuthoredTypeRef,
        generics: &GenericTypeScope,
    ) -> Result<(), CallableCatalogBuildError> {
        if matches!(bound.value(), TypeRef::TraitBound(_)) {
            self.resolve_authored(module, hir, bound, generics)?;
        }
        Ok(())
    }

    fn generic_scope(
        hir: &HirModule,
        parameters: &[GenericParam],
        owner: &GenericTypeOwnerId,
    ) -> Result<GenericTypeScope, CallableCatalogBuildError> {
        let bindings = parameters
            .iter()
            .filter_map(GenericParam::as_type_param)
            .enumerate()
            .map(|(ordinal, parameter)| {
                let source = hir
                    .source_span(parameter.name_range())
                    .or_else(|| hir.source_span(parameter.range()))
                    .ok_or_else(|| CallableCatalogBuildError::MissingProjectModuleSource {
                        module: hir.module_path().clone(),
                    })?;
                let ordinal =
                    u16::try_from(ordinal).map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
                Ok(GenericTypeBinding::new(
                    GenericTypeParameterId::new(owner.clone(), ordinal),
                    parameter.name().clone(),
                    TypeSourceEvidence::accepted(parameter.name_range(), source),
                ))
            })
            .collect::<Result<Vec<_>, CallableCatalogBuildError>>()?;
        GenericTypeScope::try_new(bindings).map_err(|error| {
            CallableCatalogBuildError::InvalidProjectSignatureSource {
                span: error
                    .duplicate()
                    .project()
                    .expect("accepted callable generic bindings retain project source")
                    .clone(),
            }
        })
    }

    fn resolve_authored(
        &mut self,
        module: &CanonicalModulePath,
        hir: &HirModule,
        authored: &AuthoredTypeRef,
        generics: &GenericTypeScope,
    ) -> Result<TypeKind, CallableCatalogBuildError> {
        let document = hir.source_document().ok_or_else(|| {
            CallableCatalogBuildError::MissingProjectModuleSource {
                module: module.clone(),
            }
        })?;
        let source = hir
            .source_span(*authored.root_source().whole())
            .ok_or_else(|| CallableCatalogBuildError::MissingProjectModuleSource {
                module: module.clone(),
            })?;
        let source_backed =
            SourceBackedTypeRef::try_bind(authored.clone(), document, document.identity())
                .map_err(
                    |reason| CallableCatalogBuildError::ProjectSignatureSourceBinding {
                        span: source.clone(),
                        reason: Box::new(reason),
                    },
                )?;
        let input = TypeResolutionInput::accepted(
            &source_backed,
            module,
            self.symbols,
            self.nominal_world,
            generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .map_err(
            |reason| CallableCatalogBuildError::ProjectSignatureResolutionInput {
                span: source.clone(),
                reason: Box::new(reason),
            },
        )?;
        let report = self.cache.resolve(&input).map_err(|reason| {
            CallableCatalogBuildError::ProjectSignatureResolutionInput {
                span: source.clone(),
                reason: Box::new(reason),
            }
        })?;
        let resolved = match report.outcome() {
            ResolvedTypeRefOutcome::Complete(product) => product.recovered().clone(),
            ResolvedTypeRefOutcome::Poisoned(poisoned) => poisoned.product().recovered().clone(),
            ResolvedTypeRefOutcome::Detached(_) => {
                return Err(CallableCatalogBuildError::DetachedProjectSignatureType {
                    span: source,
                });
            }
        };
        self.resolutions
            .record(source.clone(), report.as_ref().clone())
            .map_err(
                |reason| CallableCatalogBuildError::ProjectSignatureResolutionIndex {
                    span: source,
                    reason: Box::new(reason),
                },
            )?;
        Ok(resolved)
    }
}
