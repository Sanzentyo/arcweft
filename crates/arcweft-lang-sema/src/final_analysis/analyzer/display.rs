//! The one declaration-owned selection authority for standard `DisplayText`.
//!
//! A trait reference is a conformance identity, not a value type. We accept
//! the standard unqualified path only when the project symbol table has no
//! binding at that exact source site. In particular a project trait with the
//! same name cannot masquerade as the standard trait.

use arcweft_lang_hir::{
    identity::ItemId,
    item::{
        HirFunctionBody, HirImplMember, HirItemKind, HirMethodParameter, HirMethodReceiverKind,
    },
    leaf::{HirName, HirPathRoot, HirPathSegment},
    source_index::{HirCallableSourceOwner, HirSourceQuery, HirTypeSourceRole},
    symbol::{
        CallableDeclarationKey, ImplMethodDeclarationId, ImplMethodKind,
        ProjectHirSymbolLookupError, ProjectSymbolResolutionError,
    },
    type_ref::HirTypeKind,
};

use crate::{
    callable::CheckedCallableCatalog,
    checked_rich_text::{CheckedDisplayConformance, CheckedDisplayWitness},
    final_analysis::FinalSemanticAnalysisError,
    types::{TypeGenericUseCollector, TypeKind, TypeParameterSubstitutions},
};

use super::{Analyzer, CheckedSuspensionRole, statements::source_span};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DisplayConformanceRejection {
    #[error("the standard trait reference is hidden by a project symbol or is ambiguous")]
    TraitIdentity,
    #[error("the impl target or method type could not be resolved")]
    UnresolvedType,
    #[error("a DisplayText impl must have exactly one body-bearing display_text method")]
    MethodInventory,
    #[error("display_text must have one owned self and one DisplayContext parameter")]
    Parameters,
    #[error("display_text must return Result<Content, DisplayError>")]
    Result,
    #[error("display_text may not declare method generics, predicates, or attached content")]
    MethodModifiers,
    #[error("the DisplayText implementation has unsupported predicates or generic bounds")]
    ImplPredicates,
    #[error("display_text must have an empty inferred effect row and may not suspend")]
    Effects,
    #[error("the impl method is missing its accepted declaration identity")]
    MethodIdentity,
}

#[derive(Clone, Debug)]
struct DisplayImplTemplate {
    authority: crate::callable::CheckedCallableAuthorityLease,
    target: TypeKind,
    required_type_arguments: usize,
    implementation: ItemId,
    method_ordinal: u16,
    method_declaration: ImplMethodDeclarationId,
}

/// A standard trait signature and all validated project impl templates for
/// one accepted HIR/symbol generation. Every selected witness closes a
/// template against one exact final value type.
#[derive(Clone, Debug, Default)]
pub(in crate::final_analysis) struct DisplayConformanceCatalog {
    templates: Vec<DisplayImplTemplate>,
}

impl DisplayConformanceCatalog {
    pub(super) fn build(
        analyzer: &Analyzer<'_, '_, '_>,
        callables: &CheckedCallableCatalog,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut templates = Vec::new();
        for module in analyzer.modules.values().copied() {
            for (owner, item) in module.items() {
                let HirItemKind::Impl(implementation) = item.kind() else {
                    continue;
                };
                let Some(trait_ref) = implementation.trait_ref() else {
                    continue;
                };
                let trait_node = module
                    .resolve_type(trait_ref)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirTypeKind::Path(path) = trait_node.kind() else {
                    continue;
                };
                if path.root() != HirPathRoot::ImplicitCrate
                    || !matches!(path.segments(), [HirPathSegment::Identifier(name)] if name.as_str() == "DisplayText")
                {
                    continue;
                }
                let trait_source = source_span(
                    module,
                    HirSourceQuery::Type {
                        owner: trait_ref,
                        role: HirTypeSourceRole::Whole,
                    },
                )?;
                match analyzer.symbols.resolve_hir_symbol_target(
                    module.key().path(),
                    path,
                    trait_source,
                ) {
                    Err(ProjectHirSymbolLookupError::Symbol(
                        ProjectSymbolResolutionError::Unknown { .. },
                    )) => {}
                    _ => return Err(invalid(owner, DisplayConformanceRejection::TraitIdentity)),
                }
                if !implementation.where_predicates().is_empty()
                    || implementation
                        .generic_parameters()
                        .iter()
                        .any(|parameter| !parameter.bounds().is_empty())
                {
                    return Err(invalid(owner, DisplayConformanceRejection::ImplPredicates));
                }
                let target = analyzer
                    .types
                    .get(&implementation.target())
                    .cloned()
                    .ok_or_else(|| invalid(owner, DisplayConformanceRejection::UnresolvedType))?;
                if !matches!(target, TypeKind::ProjectNominal(_))
                    || implementation.generic_parameters().iter().any(|parameter| {
                        matches!(
                            parameter,
                            arcweft_lang_hir::item::HirGenericParameter::Lifetime { .. }
                        )
                    })
                {
                    return Err(invalid(owner, DisplayConformanceRejection::ImplPredicates));
                }
                let required_type_arguments = implementation.generic_parameters().len();
                let uses = TypeGenericUseCollector::collect(&target)
                    .map_err(|_| invalid(owner, DisplayConformanceRejection::ImplPredicates))?;
                if uses.types().len() != required_type_arguments
                    || uses
                        .types()
                        .iter()
                        .enumerate()
                        .any(|(ordinal, parameter)| usize::from(parameter.ordinal()) != ordinal)
                {
                    return Err(invalid(owner, DisplayConformanceRejection::ImplPredicates));
                }
                let [HirImplMember::Function(method)] = implementation.members() else {
                    return Err(invalid(owner, DisplayConformanceRejection::MethodInventory));
                };
                let ordinal = 0;
                if method.name().resolved().map(HirName::as_str) != Some("display_text")
                    || !matches!(method.body(), Some(HirFunctionBody::Block { .. }))
                {
                    return Err(invalid(owner, DisplayConformanceRejection::MethodInventory));
                }
                if !method.generic_parameters().is_empty()
                    || !method.where_predicates().is_empty()
                    || method.attached_content().is_some()
                {
                    return Err(invalid(owner, DisplayConformanceRejection::MethodModifiers));
                }
                let [group] = method.parameter_groups() else {
                    return Err(invalid(owner, DisplayConformanceRejection::Parameters));
                };
                let [
                    HirMethodParameter::Receiver(receiver),
                    HirMethodParameter::Typed(context),
                ] = group.parameters()
                else {
                    return Err(invalid(owner, DisplayConformanceRejection::Parameters));
                };
                if receiver.kind() != HirMethodReceiverKind::Owned
                    || context.default().is_some()
                    || analyzer.types.get(&context.ty()) != Some(&TypeKind::DisplayContext)
                {
                    return Err(invalid(owner, DisplayConformanceRejection::Parameters));
                }
                let Some(return_type) = method.return_type() else {
                    return Err(invalid(owner, DisplayConformanceRejection::Result));
                };
                let content = analyzer
                    .catalogs
                    .world
                    .environment()
                    .typecheck_env()
                    .standard_dialogue_content_type()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let expected = TypeKind::Result {
                    ok: Box::new(content),
                    error: Box::new(TypeKind::DisplayError),
                };
                if analyzer.types.get(&return_type) != Some(&expected) {
                    return Err(invalid(owner, DisplayConformanceRejection::Result));
                }
                let mut declarations = analyzer.symbols.callable_symbols().filter_map(|symbol| {
                    (symbol.source_item() == owner
                        && symbol.source_owner()
                            == HirCallableSourceOwner::ImplFunction { member: ordinal })
                    .then(|| symbol.declaration())
                });
                let Some(CallableDeclarationKey::ImplMethod(declaration)) = declarations.next()
                else {
                    return Err(invalid(owner, DisplayConformanceRejection::MethodIdentity));
                };
                if declarations.next().is_some() || declaration.kind() != ImplMethodKind::Trait {
                    return Err(invalid(owner, DisplayConformanceRejection::MethodIdentity));
                }
                let facts = callables
                    .project_callable(&CallableDeclarationKey::ImplMethod(declaration.clone()))
                    .map_err(|_| invalid(owner, DisplayConformanceRejection::MethodIdentity))?;
                if !facts.actual_row().is_some_and(|row| row.is_empty())
                    || facts.suspension() != CheckedSuspensionRole::NonSuspending
                {
                    return Err(invalid(owner, DisplayConformanceRejection::Effects));
                }
                if let Some(first) = templates
                    .iter()
                    .find(|existing: &&DisplayImplTemplate| existing.target == target)
                {
                    return Err(FinalSemanticAnalysisError::DuplicateDisplayTextImpl {
                        target: Box::new(target),
                        first: first.implementation,
                        second: owner,
                    });
                }
                templates.push(DisplayImplTemplate {
                    authority: callables.authority_lease(),
                    target,
                    required_type_arguments,
                    implementation: owner,
                    method_ordinal: ordinal,
                    method_declaration: declaration.clone(),
                });
            }
        }
        Ok(Self { templates })
    }

    pub(in crate::final_analysis) fn for_interpolation(
        &self,
        ty: &TypeKind,
        content: &TypeKind,
    ) -> Result<Option<CheckedDisplayWitness>, FinalSemanticAnalysisError> {
        if let Some(builtin) = CheckedDisplayWitness::for_interpolation(ty, content) {
            return Ok(Some(builtin));
        }
        self.project(ty)
            .map(|value| value.map(|value| CheckedDisplayWitness::Project(Box::new(value))))
    }

    pub(in crate::final_analysis) fn for_fmt_primary(
        &self,
        ty: &TypeKind,
        content: &TypeKind,
    ) -> Result<Option<CheckedDisplayWitness>, FinalSemanticAnalysisError> {
        if let Some(builtin) = CheckedDisplayWitness::for_fmt_primary(ty, content) {
            return Ok(Some(builtin));
        }
        if let TypeKind::Option(inner) = ty {
            return self.project(inner).map(|value| {
                value.map(|value| CheckedDisplayWitness::OptionProject(Box::new(value)))
            });
        }
        self.project(ty)
            .map(|value| value.map(|value| CheckedDisplayWitness::Project(Box::new(value))))
    }

    fn project(
        &self,
        target: &TypeKind,
    ) -> Result<Option<CheckedDisplayConformance>, FinalSemanticAnalysisError> {
        let mut selected = None;
        for template in &self.templates {
            let mut substitutions = TypeParameterSubstitutions::default();
            if !substitutions.observe(&template.target, target)
                || substitutions.apply_resolved(&template.target).as_ref() != Some(target)
                || substitutions.bindings().count() != template.required_type_arguments
            {
                continue;
            }
            let type_arguments = substitutions
                .bindings()
                .map(|(parameter, ty)| (parameter.clone(), ty.clone()))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let conformance = CheckedDisplayConformance::new(
                template.authority.clone(),
                target.clone(),
                template.implementation,
                template.method_ordinal,
                template.method_declaration.clone(),
                type_arguments,
            );
            if let Some(first) = selected.replace(conformance) {
                return Err(FinalSemanticAnalysisError::DuplicateDisplayTextImpl {
                    target: Box::new(target.clone()),
                    first: first.implementation(),
                    second: template.implementation,
                });
            }
        }
        Ok(selected)
    }
}

fn invalid(
    implementation: ItemId,
    reason: DisplayConformanceRejection,
) -> FinalSemanticAnalysisError {
    FinalSemanticAnalysisError::InvalidDisplayTextImpl {
        implementation,
        reason,
    }
}
