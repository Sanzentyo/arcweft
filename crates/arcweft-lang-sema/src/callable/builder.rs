//! Fail-closed construction of the immutable registered callable catalog.
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    item::{
        HirCapabilityFunction, HirCapabilityMember, HirFlowItem, HirFunctionItem, HirImplFunction,
        HirImplMember, HirItem, HirItemPrefix, HirMethodParameter, HirMethodParameterGroup,
        HirParameter, HirParameterKind, HirPredicate, HirProof, HirTraitFunction, HirTraitMember,
        HirViewDeclaration,
    },
    module::HirModule,
    pattern::{HirPatternBinding, HirPatternKind},
    project::HirProjectView,
    source_index::{
        HirCallableParameterSourcePart, HirCallableSourceOwner, HirCallableSourceRole,
        HirFlowParameterSourcePart, HirFlowReturnSourcePart, HirFlowSourceRole, HirItemSourceRole,
        HirSourcePresence, HirSourceQuery, HirSourceSite,
    },
    symbol::{CallableSymbol, ProjectSymbolTable, ProjectSymbolTargetId},
};

use crate::{
    effect_row::EffectRow,
    effects::{EffectId, EffectSet},
    env::{FunctionSignature, TypeCheckEnv},
    nominal::{CheckedTypeReferenceCache, NominalResolutionIndex},
    registration::{AcceptedNominalWorld, AcceptedNominalWorldStamp, EnvironmentManifestDigest},
    types::TypeKind,
};

use super::digest::CanonicalEncoder;
use super::limits::CatalogBuildWork;
use super::nominal_signature::ProjectSignatureResolver;
use super::{
    CallableAccess, CallableArgumentPolicy, CallableAuthorityRank, CallableBuildLimitError,
    CallableCandidateId, CallableCatalogBuildError, CallableDocumentation, CallableEffectSchema,
    CallableEvaluatedEffect, CallableGenericParameterIssuer, CallableGroupIndex, CallableGroupKind,
    CallableLimits, CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameter,
    CallableParameterAdmission, CallableParameterGroup, CallableParameterIndex,
    CallableParameterPassing, CallableParameterPresence, CallableParameterSource, CallablePath,
    CallablePathError, CallableProviderId, CallableRecord, CallableSignatureSchema, CallableSource,
    CallableValidator, CatalogCallableEntry, DocumentationProvenance, EnvironmentCallableCatalog,
    EnvironmentCallableId, EnvironmentCallableKind, EnvironmentCallableOwner,
    EnvironmentCallablePublication, EnvironmentCallablePublicationRecord,
    EnvironmentDeclarationOrdinal, EquivalentCallableSource, NonEmptyCallableSet,
    ProjectCallableCatalog, ProjectCallablePath, ProjectNameBinding, RegisteredCallableCatalog,
    RegisteredProjectModuleCallables, SignatureOrigin, SpreadArgumentPolicy, StandardEnvironmentId,
    UnknownNamedArgumentPolicy,
};

pub(crate) struct RegisteredCallableCatalogBuilder {
    nominal_world: AcceptedNominalWorldStamp,
    limits: CallableLimits,
    project_modules: Vec<RegisteredProjectModuleCallables>,
    project_records: Vec<Arc<CallableRecord>>,
    project_bindings: Vec<(ProjectCallablePath, ProjectNameBinding)>,
    environment_publications: Vec<EnvironmentCallablePublication>,
    nominal_resolutions: NominalResolutionIndex,
    nominal_cache: CheckedTypeReferenceCache,
    work: CatalogBuildWork,
}

struct ProjectParameterPublication {
    groups: Vec<CallableParameterGroup>,
    sources: Vec<CallableParameterSource>,
    extension_receiver: Option<super::CallableExtensionReceiver>,
}

enum FinalProjectCallable<'a> {
    Flow {
        item: &'a HirItem,
        callable: &'a HirFlowItem,
    },
    Function {
        item: &'a HirItem,
        callable: &'a HirFunctionItem,
    },
    Predicate {
        item: &'a HirItem,
        callable: &'a HirPredicate,
    },
    Proof {
        item: &'a HirItem,
        callable: &'a HirProof,
    },
    View {
        item: &'a HirItem,
        callable: &'a HirViewDeclaration,
    },
    ExternCapability {
        callable: &'a HirCapabilityFunction,
    },
    TraitMethod {
        item: &'a HirItem,
        callable: &'a HirTraitFunction,
    },
    ImplMethod {
        item: &'a HirItem,
        callable: &'a HirImplFunction,
    },
}

#[derive(Clone, Copy)]
enum FinalProjectParameter<'a> {
    Ordinary(&'a HirParameter),
    Receiver,
    TypedMethod(&'a HirParameter),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StandardEnvironmentMethodProjection {
    id: EnvironmentCallableId,
    schema: CallableSignatureSchema,
}

impl StandardEnvironmentMethodProjection {
    fn try_from_signature(
        receiver: &TypeKind,
        member: &CallableName,
        signature: &FunctionSignature,
        role: crate::env::StandardEnvironmentMethodRole,
        limits: &CallableLimits,
    ) -> Result<Self, super::CallablePublicationError> {
        let kind = if signature.checks_args() {
            EnvironmentCallableKind::Method
        } else {
            EnvironmentCallableKind::UntypedMethodFallback
        };
        let key = CallableLookupKey::Method(super::ReceiverMethodKey::new(
            receiver.clone(),
            member.clone(),
        ));
        let schema = signature.callable_schema(
            EffectRow::closed(EffectSet::new()),
            if signature.checks_args() {
                role.validator()
            } else {
                CallableValidator::Untyped
            },
            CallableGenericParameterIssuer::empty(),
            limits,
        )?;
        let id = EnvironmentCallableId::new(
            EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core),
            kind,
            key,
            CallableOverloadIndex::try_from_usize(0)
                .map_err(|_| super::CallablePublicationError::InvalidOverload)?,
        );
        Ok(Self { id, schema })
    }

    fn into_publication_record(
        self,
        ordinal: usize,
    ) -> Result<EnvironmentCallablePublicationRecord, super::CallablePublicationError> {
        EnvironmentCallablePublicationRecord::try_new(
            self.id.kind(),
            self.id.key().clone(),
            self.id.overload(),
            self.schema,
            CallableDocumentation::missing(),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(ordinal)
                .map_err(|_| super::CallablePublicationError::InvalidOverload)?,
        )
    }
}

impl RegisteredCallableCatalogBuilder {
    pub(crate) fn for_nominal_world(world: &AcceptedNominalWorld, limits: CallableLimits) -> Self {
        Self {
            nominal_world: world.stamp(),
            limits,
            project_modules: Vec::new(),
            project_records: Vec::new(),
            project_bindings: Vec::new(),
            environment_publications: Vec::new(),
            nominal_resolutions: NominalResolutionIndex::production(),
            nominal_cache: CheckedTypeReferenceCache::default(),
            work: CatalogBuildWork::new(limits.max_catalog_build_work()),
        }
    }

    pub(crate) fn add_project(
        &mut self,
        project: HirProjectView<'_>,
        symbols: &ProjectSymbolTable,
        nominal_world: &AcceptedNominalWorld,
    ) -> Result<(), CallableCatalogBuildError> {
        if project.package() != symbols.world().package() {
            return Err(CallableCatalogBuildError::ProjectWorldPackageMismatch {
                expected: project.package().clone(),
                actual: symbols.world().package().clone(),
            });
        }
        let module_count = project.modules().len();
        if module_count > self.limits.max_project_modules() {
            return Err(super::CallableBuildLimitError::Modules {
                actual: module_count,
                limit: self.limits.max_project_modules(),
            }
            .into());
        }
        for (module_path, module) in project.modules() {
            self.work.charge(1)?;
            let module_symbols = symbols
                .callable_symbols()
                .filter(|symbol| symbol.declaration().module() == module_path)
                .collect::<Vec<_>>();
            let mut declarations = Vec::with_capacity(module_symbols.len());
            for symbol in module_symbols {
                let ordinal = self.project_records.len();
                let record = Arc::new(self.project_record(
                    symbol,
                    module,
                    ordinal,
                    project,
                    symbols,
                    nominal_world,
                )?);
                declarations.push(symbol.declaration().clone());
                self.project_records.push(record);
                let record_count = self
                    .environment_publications
                    .iter()
                    .try_fold(self.project_records.len(), |count, publication| {
                        count.checked_add(publication.records().len())
                    });
                if record_count.is_none_or(|count| count > self.limits.max_catalog_records()) {
                    return Err(super::CallableBuildLimitError::Records {
                        actual: record_count.unwrap_or(usize::MAX),
                        limit: self.limits.max_catalog_records(),
                    }
                    .into());
                }
            }
            self.project_modules
                .push(RegisteredProjectModuleCallables::new(
                    module_path.clone(),
                    module.provenance().source_identity().clone(),
                    declarations,
                ));
        }
        Ok(())
    }

    fn project_record(
        &mut self,
        symbol: &CallableSymbol,
        module: &HirModule,
        ordinal: usize,
        project: HirProjectView<'_>,
        symbols: &ProjectSymbolTable,
        nominal_world: &AcceptedNominalWorld,
    ) -> Result<CallableRecord, CallableCatalogBuildError> {
        self.work.charge(1)?;
        let path_segment_count = match symbol.declaration() {
            arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(flow) => {
                flow.public_id().as_str().split('.').count()
            }
            declaration => declaration.module().segments().len().saturating_add(1),
        };
        self.work.charge(
            u64::try_from(path_segment_count)
                .map_err(|_| CallableCatalogBuildError::WorkOverflow)?,
        )?;
        let resolved = ProjectSignatureResolver::new(
            project,
            symbols,
            nominal_world,
            &mut self.nominal_resolutions,
            &mut self.nominal_cache,
        )
        .resolve_project_signature(symbol)?;
        let callable = final_project_callable(module, symbol)?;
        let parameters = project_parameters(
            module,
            symbol,
            &callable,
            &resolved.parameter_types,
            &self.limits,
            &mut self.work,
        )?;
        let effects = if matches!(callable, FinalProjectCallable::ExternCapability { .. }) {
            let declared = effect_set(module, symbol, &callable, &mut self.work)?;
            CallableEffectSchema::fixed(EffectRow::closed(declared))
        } else {
            CallableEffectSchema::project(symbol.declaration().clone())
        };
        let validator = match symbol.owner() {
            arcweft_lang_hir::symbol::CallableDeclarationOwner::TraitRequirement => {
                CallableValidator::Method(super::CallableMethodRole::TraitRequirement)
            }
            arcweft_lang_hir::symbol::CallableDeclarationOwner::TraitImplementation => {
                CallableValidator::Method(super::CallableMethodRole::TraitImplementation)
            }
            arcweft_lang_hir::symbol::CallableDeclarationOwner::InherentMethod => {
                CallableValidator::Method(super::CallableMethodRole::Inherent)
            }
            _ => CallableValidator::Ordinary,
        };
        let mut schema = CallableSignatureSchema::try_new(
            parameters.groups,
            resolved.return_type,
            effects,
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                if callable.has_rest_parameter() {
                    SpreadArgumentPolicy::TypedRest
                } else {
                    SpreadArgumentPolicy::FixedLiteralOnly
                },
            ),
            validator,
            resolved.generic_issuer,
            &self.limits,
        )?;
        if let Some(receiver) = parameters.extension_receiver {
            schema = schema.with_extension_receiver(receiver)?;
        }
        let host_call_contract = resolved.host_call_contract;
        let schema = Arc::new(schema);
        let documentation = project_documentation(symbol, callable.prefix().documentation())?;
        let signature_span = project_signature_span(module, symbol, &callable)?;
        let name_span = project_name_span(module, symbol, &callable)?;
        let identity_span = project_identity_span(module, symbol, &callable)?;
        if &identity_span != symbol.name_span() {
            return Err(identity_mismatch(symbol));
        }
        let result_span = project_result_span(module, symbol, &callable)?;
        let callable_source = CallableSource::try_new(
            Some(symbol.declaration().clone()),
            Some(signature_span),
            name_span,
            result_span,
            parameters.sources,
        )
        .map_err(|_| identity_mismatch(symbol))?;
        CallableRecord::try_new(
            CallableCandidateId::Project(symbol.declaration().clone()),
            project_lookup_key(symbol, &schema)?,
            CallableAuthorityRank::Project,
            CallableProviderId::Project(symbol.declaration().package().clone()),
            project_access(symbol),
            schema,
            documentation,
            Some(callable_source),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(ordinal)
                .map_err(|_| identity_mismatch(symbol))?,
        )
        .and_then(|record| record.with_host_call_contract(host_call_contract))
        .map_err(CallableCatalogBuildError::InvalidRecord)
    }

    /// Publishes every source-visible project spelling after registration has
    /// assigned exact semantic types to non-callable external owners.
    pub(crate) fn add_project_bindings(
        &mut self,
        project: HirProjectView<'_>,
        symbols: &ProjectSymbolTable,
        mut non_callable_type: impl FnMut(&ProjectSymbolTargetId) -> Option<TypeKind>,
    ) -> Result<(), CallableCatalogBuildError> {
        for (module, binding_path, target) in symbols.scope_bindings() {
            let segment_count = binding_path.segments().len();
            self.work.charge(1)?;
            self.work.charge(
                u64::try_from(segment_count)
                    .map_err(|_| CallableCatalogBuildError::WorkOverflow)?,
            )?;
            let segments = binding_path.segments().iter().map(|segment| {
                CallableName::try_new(segment.as_str())
                    .expect("ProjectSymbolSegment grammar is a strict subset of CallableName")
            });
            let callable_path = match CallablePath::try_new_with_limits(segments, &self.limits) {
                Ok(path) => path,
                Err(CallablePathError::TooManySegments { actual, limit }) => {
                    return Err(CallableBuildLimitError::PathSegments { actual, limit }.into());
                }
                Err(CallablePathError::Empty) => {
                    unreachable!("ProjectSymbolPath is non-empty by construction")
                }
            };
            let path =
                ProjectCallablePath::new(project.package().clone(), module.clone(), callable_path);
            let binding = match target {
                ProjectSymbolTargetId::Callable(declaration) => {
                    ProjectNameBinding::Callable(declaration.clone())
                }
                ProjectSymbolTargetId::StructuralCallable(_) => continue,
                ProjectSymbolTargetId::External(_)
                | ProjectSymbolTargetId::Nominal(_)
                | ProjectSymbolTargetId::Retained(_)
                | ProjectSymbolTargetId::Module(_) => ProjectNameBinding::NonCallable {
                    path: path.clone(),
                    ty: non_callable_type(target).ok_or_else(|| {
                        CallableCatalogBuildError::MissingProjectBindingType {
                            target: Box::new(target.clone()),
                        }
                    })?,
                },
            };
            self.project_bindings.push((path, binding));
        }
        Ok(())
    }

    pub(crate) fn add_environment(
        &mut self,
        publication: EnvironmentCallablePublication,
    ) -> Result<(), CallableCatalogBuildError> {
        if publication.nominal_world() != &self.nominal_world {
            return Err(CallableCatalogBuildError::PublicationWorldMismatch {
                expected: Box::new(self.nominal_world.clone()),
                actual: Box::new(publication.nominal_world().clone()),
            });
        }
        let environment_count = self
            .environment_publications
            .iter()
            .map(|publication| publication.records().len())
            .try_fold(publication.records().len(), usize::checked_add);
        let record_count = environment_count
            .and_then(|count| count.checked_add(self.project_records.len()))
            .unwrap_or(usize::MAX);
        if record_count > self.limits.max_catalog_records() {
            return Err(super::CallableBuildLimitError::Records {
                actual: record_count,
                limit: self.limits.max_catalog_records(),
            }
            .into());
        }
        self.work.charge(1)?;
        self.work
            .charge(u64::try_from(publication.records().len()).unwrap_or(u64::MAX))?;
        self.environment_publications.push(publication);
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<RegisteredCallableCatalog, CallableCatalogBuildError> {
        let environment =
            finish_environment(self.environment_publications, &self.limits, &mut self.work)?;
        let project = finish_project(
            self.project_modules,
            self.project_records,
            self.project_bindings,
            &mut self.work,
        )?;
        Ok(RegisteredCallableCatalog::new(
            self.nominal_world,
            project,
            environment,
            self.nominal_resolutions,
        ))
    }
}

impl FinalProjectCallable<'_> {
    fn prefix(&self) -> &HirItemPrefix {
        match self {
            Self::Flow { item, .. }
            | Self::Function { item, .. }
            | Self::Predicate { item, .. }
            | Self::Proof { item, .. }
            | Self::View { item, .. }
            | Self::TraitMethod { item, .. }
            | Self::ImplMethod { item, .. } => item.prefix(),
            Self::ExternCapability { callable } => callable.prefix(),
        }
    }

    fn parameter_groups(&self) -> Vec<Vec<FinalProjectParameter<'_>>> {
        match self {
            Self::Flow { callable, .. } => vec![
                callable
                    .parameters()
                    .iter()
                    .map(FinalProjectParameter::Ordinary)
                    .collect(),
            ],
            Self::Function { callable, .. } => callable
                .parameter_groups()
                .iter()
                .map(|group| {
                    group
                        .parameters()
                        .iter()
                        .map(FinalProjectParameter::Ordinary)
                        .collect()
                })
                .collect(),
            Self::Predicate { callable, .. } => vec![
                callable
                    .parameters()
                    .iter()
                    .map(FinalProjectParameter::Ordinary)
                    .collect(),
            ],
            Self::Proof { callable, .. } => vec![
                callable
                    .parameters()
                    .iter()
                    .map(FinalProjectParameter::Ordinary)
                    .collect(),
            ],
            Self::View { callable, .. } => vec![
                callable
                    .parameters()
                    .iter()
                    .map(FinalProjectParameter::Ordinary)
                    .collect(),
            ],
            Self::ExternCapability { callable } => callable
                .parameter_groups()
                .iter()
                .map(|group| {
                    group
                        .parameters()
                        .iter()
                        .map(FinalProjectParameter::Ordinary)
                        .collect()
                })
                .collect(),
            Self::TraitMethod { callable, .. } => callable
                .parameter_groups()
                .iter()
                .map(method_parameters)
                .collect(),
            Self::ImplMethod { callable, .. } => callable
                .parameter_groups()
                .iter()
                .map(method_parameters)
                .collect(),
        }
    }

    fn has_rest_parameter(&self) -> bool {
        self.parameter_groups()
            .into_iter()
            .flatten()
            .any(|parameter| {
                matches!(
                    parameter,
                    FinalProjectParameter::Ordinary(parameter)
                        | FinalProjectParameter::TypedMethod(parameter)
                        if parameter.kind() == HirParameterKind::RestPositional
                )
            })
    }
}

fn method_parameters(group: &HirMethodParameterGroup) -> Vec<FinalProjectParameter<'_>> {
    group
        .parameters()
        .iter()
        .map(|parameter| match parameter {
            HirMethodParameter::Receiver(_) => FinalProjectParameter::Receiver,
            HirMethodParameter::Typed(parameter) => FinalProjectParameter::TypedMethod(parameter),
        })
        .collect()
}

fn final_project_callable<'a>(
    module: &'a HirModule,
    symbol: &CallableSymbol,
) -> Result<FinalProjectCallable<'a>, CallableCatalogBuildError> {
    if symbol.source_snapshot() != module.snapshot_id() {
        return Err(identity_mismatch(symbol));
    }
    let item = module
        .resolve_item(symbol.source_item())
        .map_err(|_| identity_mismatch(symbol))?;
    match (symbol.source_owner(), item.kind()) {
        (HirCallableSourceOwner::Item, arcweft_lang_hir::item::HirItemKind::Flow(callable)) => {
            Ok(FinalProjectCallable::Flow { item, callable })
        }
        (HirCallableSourceOwner::Item, arcweft_lang_hir::item::HirItemKind::Function(callable)) => {
            Ok(FinalProjectCallable::Function { item, callable })
        }
        (
            HirCallableSourceOwner::Item,
            arcweft_lang_hir::item::HirItemKind::Predicate(callable),
        ) => Ok(FinalProjectCallable::Predicate { item, callable }),
        (HirCallableSourceOwner::Item, arcweft_lang_hir::item::HirItemKind::Proof(callable)) => {
            Ok(FinalProjectCallable::Proof { item, callable })
        }
        (HirCallableSourceOwner::ViewItem, arcweft_lang_hir::item::HirItemKind::View(callable)) => {
            Ok(FinalProjectCallable::View { item, callable })
        }
        (
            HirCallableSourceOwner::ExternCapabilityFunction { member },
            arcweft_lang_hir::item::HirItemKind::ExternCapability(capability),
        ) => match capability.members().get(usize::from(member)) {
            Some(HirCapabilityMember::Function(callable)) => {
                Ok(FinalProjectCallable::ExternCapability { callable })
            }
            _ => Err(identity_mismatch(symbol)),
        },
        (
            HirCallableSourceOwner::TraitFunction { member },
            arcweft_lang_hir::item::HirItemKind::Trait(trait_item),
        ) => match trait_item.members().get(usize::from(member)) {
            Some(HirTraitMember::Function(callable)) => {
                Ok(FinalProjectCallable::TraitMethod { item, callable })
            }
            _ => Err(identity_mismatch(symbol)),
        },
        (
            HirCallableSourceOwner::ImplFunction { member },
            arcweft_lang_hir::item::HirItemKind::Impl(impl_item),
        ) => match impl_item.members().get(usize::from(member)) {
            Some(HirImplMember::Function(callable)) => {
                Ok(FinalProjectCallable::ImplMethod { item, callable })
            }
            _ => Err(identity_mismatch(symbol)),
        },
        _ => Err(identity_mismatch(symbol)),
    }
}

fn callable_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    role: HirCallableSourceRole,
) -> Result<Option<arcweft_source::SourceSpan>, CallableCatalogBuildError> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item {
                owner: symbol.source_item(),
                role: HirItemSourceRole::Callable(role),
            },
        )
        .map_err(|_| identity_mismatch(symbol))?;
    Ok(match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => None,
    })
}

fn required_callable_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    role: HirCallableSourceRole,
) -> Result<arcweft_source::SourceSpan, CallableCatalogBuildError> {
    callable_span(module, symbol, role)?.ok_or_else(|| identity_mismatch(symbol))
}

fn flow_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    role: HirFlowSourceRole,
) -> Result<Option<arcweft_source::SourceSpan>, CallableCatalogBuildError> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item {
                owner: symbol.source_item(),
                role: HirItemSourceRole::Flow(role),
            },
        )
        .map_err(|_| identity_mismatch(symbol))?;
    Ok(match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => None,
    })
}

fn required_flow_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    role: HirFlowSourceRole,
) -> Result<arcweft_source::SourceSpan, CallableCatalogBuildError> {
    flow_span(module, symbol, role)?.ok_or_else(|| identity_mismatch(symbol))
}

fn project_signature_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    callable: &FinalProjectCallable<'_>,
) -> Result<arcweft_source::SourceSpan, CallableCatalogBuildError> {
    match callable {
        FinalProjectCallable::Flow { .. } => {
            required_flow_span(module, symbol, HirFlowSourceRole::Whole)
        }
        _ => required_callable_span(
            module,
            symbol,
            HirCallableSourceRole::Signature {
                owner: symbol.source_owner(),
            },
        ),
    }
}

fn project_name_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    callable: &FinalProjectCallable<'_>,
) -> Result<Option<arcweft_source::SourceSpan>, CallableCatalogBuildError> {
    match callable {
        FinalProjectCallable::Flow { callable, .. } => callable
            .identity()
            .name()
            .map(|_| required_flow_span(module, symbol, HirFlowSourceRole::Name))
            .transpose(),
        _ => required_callable_span(
            module,
            symbol,
            HirCallableSourceRole::Name {
                owner: symbol.source_owner(),
            },
        )
        .map(Some),
    }
}

fn project_identity_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    callable: &FinalProjectCallable<'_>,
) -> Result<arcweft_source::SourceSpan, CallableCatalogBuildError> {
    match callable {
        FinalProjectCallable::Flow { callable, .. } => required_flow_span(
            module,
            symbol,
            if callable.identity().name().is_some() {
                HirFlowSourceRole::Name
            } else {
                HirFlowSourceRole::PublicId
            },
        ),
        _ => project_name_span(module, symbol, callable)?.ok_or_else(|| identity_mismatch(symbol)),
    }
}

fn project_result_span(
    module: &HirModule,
    symbol: &CallableSymbol,
    callable: &FinalProjectCallable<'_>,
) -> Result<Option<arcweft_source::SourceSpan>, CallableCatalogBuildError> {
    match callable {
        FinalProjectCallable::Flow { callable, .. } => callable
            .result()
            .authored_type()
            .map(|_| {
                required_flow_span(
                    module,
                    symbol,
                    HirFlowSourceRole::Return {
                        part: HirFlowReturnSourcePart::Whole,
                    },
                )
            })
            .transpose(),
        _ => callable_span(
            module,
            symbol,
            HirCallableSourceRole::Result {
                owner: symbol.source_owner(),
            },
        ),
    }
}

fn effect_set(
    module: &HirModule,
    symbol: &CallableSymbol,
    callable: &FinalProjectCallable<'_>,
    work: &mut CatalogBuildWork,
) -> Result<EffectSet, CallableCatalogBuildError> {
    let FinalProjectCallable::ExternCapability { callable } = callable else {
        return Ok(EffectSet::new());
    };
    callable
        .effects()
        .iter()
        .map(|effect| {
            work.charge(1)?;
            EffectId::try_from_hir_expression(module, *effect)
                .map(|(effect, _)| effect)
                .map_err(|_| identity_mismatch(symbol))
        })
        .collect()
}

fn project_parameters(
    module: &HirModule,
    symbol: &CallableSymbol,
    callable: &FinalProjectCallable<'_>,
    resolved_groups: &[Vec<TypeKind>],
    limits: &CallableLimits,
    work: &mut CatalogBuildWork,
) -> Result<ProjectParameterPublication, CallableCatalogBuildError> {
    let parameter_groups = callable.parameter_groups();
    let mut groups = Vec::with_capacity(parameter_groups.len());
    let mut sources = Vec::new();
    let mut extension_receiver = None;
    for (group_index, group) in parameter_groups.iter().enumerate() {
        work.charge(1)?;
        let group_id = CallableGroupIndex::try_from_usize(group_index)
            .map_err(|_| identity_mismatch(symbol))?;
        let group_source_index =
            u16::try_from(group_index).map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
        let mut parameters = Vec::with_capacity(group.len());
        for (parameter_index, parameter) in group.iter().enumerate() {
            work.charge(1)?;
            let parameter_id = CallableParameterIndex::try_from_usize(parameter_index)
                .map_err(|_| identity_mismatch(symbol))?;
            let parameter_source_index = u16::try_from(parameter_index)
                .map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
            let parameter_source = project_parameter_source(
                module,
                symbol,
                callable,
                group_id,
                parameter_id,
                group_source_index,
                parameter_source_index,
            )
            .map_err(|_| identity_mismatch(symbol))?;
            sources.push(parameter_source.clone());
            if parameter
                .typed()
                .is_some_and(|parameter| parameter.kind() == HirParameterKind::ExtensionReceiver)
                && extension_receiver
                    .replace(super::CallableExtensionReceiver::new(
                        group_id,
                        parameter_id,
                    ))
                    .is_some()
            {
                return Err(CallableCatalogBuildError::InvalidSchema(
                    super::CallableSchemaError::DuplicateExtensionReceiver,
                ));
            }
            parameters.push(
                CallableParameter::try_new(
                    parameter_id,
                    parameter_name(module, *parameter, symbol)?,
                    CallableParameterAdmission::checked(
                        resolved_groups
                            .get(group_index)
                            .and_then(|group| group.get(parameter_index))
                            .cloned()
                            .ok_or_else(|| identity_mismatch(symbol))?,
                    ),
                    parameter_passing(module, *parameter, symbol)?,
                    if parameter.default().is_some() {
                        CallableParameterPresence::Defaulted
                    } else {
                        CallableParameterPresence::Required
                    },
                    None,
                    Some(parameter_source),
                )
                .map_err(CallableCatalogBuildError::InvalidSchema)?,
            );
        }
        groups.push(CallableParameterGroup::try_new(
            group_id,
            if group_index == 0 {
                CallableGroupKind::Initial
            } else {
                CallableGroupKind::Curried
            },
            parameters,
            limits,
        )?);
    }
    Ok(ProjectParameterPublication {
        groups,
        sources,
        extension_receiver,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "one typed parameter source retains both semantic and source ordinals"
)]
fn project_parameter_source(
    module: &HirModule,
    symbol: &CallableSymbol,
    callable: &FinalProjectCallable<'_>,
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
    source_group: u16,
    source_parameter: u16,
) -> Result<CallableParameterSource, CallableCatalogBuildError> {
    if matches!(callable, FinalProjectCallable::Flow { .. }) {
        if source_group != 0 {
            return Err(identity_mismatch(symbol));
        }
        let role = |part| HirFlowSourceRole::Parameter {
            ordinal: source_parameter,
            part,
        };
        return CallableParameterSource::try_new(
            group,
            parameter,
            required_flow_span(module, symbol, role(HirFlowParameterSourcePart::Whole))?,
            flow_span(module, symbol, role(HirFlowParameterSourcePart::Pattern))?,
            flow_span(module, symbol, role(HirFlowParameterSourcePart::Type))?,
            None,
        )
        .map_err(|_| identity_mismatch(symbol));
    }
    let role = |part| HirCallableSourceRole::Parameter {
        owner: symbol.source_owner(),
        group: source_group,
        parameter: source_parameter,
        part,
    };
    CallableParameterSource::try_new(
        group,
        parameter,
        required_callable_span(module, symbol, role(HirCallableParameterSourcePart::Whole))?,
        callable_span(module, symbol, role(HirCallableParameterSourcePart::Name))?,
        callable_span(module, symbol, role(HirCallableParameterSourcePart::Type))?,
        callable_span(
            module,
            symbol,
            role(HirCallableParameterSourcePart::Default),
        )?,
    )
    .map_err(|_| identity_mismatch(symbol))
}

fn callable_path(symbol: &CallableSymbol) -> Result<CallablePath, CallableCatalogBuildError> {
    let segments = match symbol.declaration() {
        arcweft_lang_hir::symbol::CallableDeclarationKey::Existing(declaration) => declaration
            .owner_path()
            .iter()
            .map(|segment| CallableName::try_new(segment.as_str()))
            .chain(std::iter::once(CallableName::try_new(declaration.name())))
            .collect::<Result<Vec<_>, _>>(),
        arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(declaration) => declaration
            .public_id()
            .as_str()
            .split('.')
            .map(CallableName::try_new)
            .collect::<Result<Vec<_>, _>>(),
        arcweft_lang_hir::symbol::CallableDeclarationKey::TraitRequirement(_)
        | arcweft_lang_hir::symbol::CallableDeclarationKey::ImplMethod(_) => {
            return Err(identity_mismatch(symbol));
        }
    }
    .map_err(|_| identity_mismatch(symbol))?;
    CallablePath::try_new(segments).map_err(|_| identity_mismatch(symbol))
}

fn project_lookup_key(
    symbol: &CallableSymbol,
    schema: &CallableSignatureSchema,
) -> Result<CallableLookupKey, CallableCatalogBuildError> {
    if !symbol.owner().is_method() {
        return callable_path(symbol).map(CallableLookupKey::Free);
    }
    let receiver = schema
        .groups()
        .first()
        .and_then(|group| group.parameters().first())
        .and_then(|parameter| parameter.declared_type().cloned())
        .ok_or_else(|| identity_mismatch(symbol))?;
    let method = CallableName::try_new(symbol.declaration().name())
        .map_err(|_| identity_mismatch(symbol))?;
    Ok(CallableLookupKey::Method(super::ReceiverMethodKey::new(
        receiver, method,
    )))
}

fn project_access(symbol: &CallableSymbol) -> CallableAccess {
    match symbol.declaration() {
        arcweft_lang_hir::symbol::CallableDeclarationKey::Existing(_) => CallableAccess::Direct {
            declaration_visibility: symbol.visibility(),
        },
        arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(_) => CallableAccess::Structural,
        arcweft_lang_hir::symbol::CallableDeclarationKey::TraitRequirement(requirement) => {
            CallableAccess::TraitRequirement {
                trait_declaration: requirement.trait_declaration().clone(),
                trait_visibility: symbol.visibility(),
            }
        }
        arcweft_lang_hir::symbol::CallableDeclarationKey::ImplMethod(method) => {
            match method.kind() {
                arcweft_lang_hir::symbol::ImplMethodKind::Trait => {
                    CallableAccess::TraitImplementation
                }
                arcweft_lang_hir::symbol::ImplMethodKind::Inherent => {
                    CallableAccess::InherentMethod {
                        owner_module: method.implementation().module().clone(),
                    }
                }
            }
        }
    }
}

fn parameter_name(
    module: &HirModule,
    parameter: FinalProjectParameter<'_>,
    symbol: &CallableSymbol,
) -> Result<Option<CallableName>, CallableCatalogBuildError> {
    if matches!(parameter, FinalProjectParameter::Receiver) {
        return CallableName::try_new("self")
            .map(Some)
            .map_err(|_| identity_mismatch(symbol));
    }
    let parameter = parameter
        .typed()
        .expect("non-receiver final project parameter is typed");
    let pattern = module
        .resolve_pattern(parameter.pattern())
        .map_err(|_| identity_mismatch(symbol))?;
    let name = match pattern.kind() {
        HirPatternKind::Binding(HirPatternBinding::Bound { name, .. })
        | HirPatternKind::MutableBinding(HirPatternBinding::Bound { name, .. }) => Some(name),
        _ => None,
    };
    name.map(|name| CallableName::try_new(name.as_str()))
        .transpose()
        .map_err(|_| identity_mismatch(symbol))
}

fn parameter_passing(
    module: &HirModule,
    parameter: FinalProjectParameter<'_>,
    symbol: &CallableSymbol,
) -> Result<CallableParameterPassing, CallableCatalogBuildError> {
    if matches!(parameter, FinalProjectParameter::Receiver) {
        return Ok(CallableParameterPassing::PositionalOnly);
    }
    let typed = parameter
        .typed()
        .expect("non-receiver final project parameter is typed");
    match typed.kind() {
        HirParameterKind::ExtensionReceiver => {
            return Ok(CallableParameterPassing::PositionalOnly);
        }
        HirParameterKind::RestPositional => {
            return Ok(CallableParameterPassing::RestPositional);
        }
        HirParameterKind::Fixed => {}
    }
    Ok(if parameter_name(module, parameter, symbol)?.is_some() {
        CallableParameterPassing::PositionalOrNamed
    } else {
        CallableParameterPassing::PositionalOnly
    })
}

impl<'a> FinalProjectParameter<'a> {
    const fn typed(self) -> Option<&'a HirParameter> {
        match self {
            Self::Ordinary(parameter) | Self::TypedMethod(parameter) => Some(parameter),
            Self::Receiver => None,
        }
    }

    const fn default(self) -> Option<arcweft_lang_hir::identity::ExprId> {
        match self.typed() {
            Some(parameter) => parameter.default(),
            None => None,
        }
    }
}

fn project_documentation(
    symbol: &CallableSymbol,
    documentation: Option<&arcweft_lang_hir::item::HirDocumentation>,
) -> Result<CallableDocumentation, CallableCatalogBuildError> {
    let Some(documentation) = documentation else {
        return Ok(CallableDocumentation::missing());
    };
    let markdown = documentation.markdown();
    let (summary, details) = markdown.split_once('\n').map_or_else(
        || (Some(Arc::<str>::from(markdown)), None),
        |(summary, details)| {
            (
                (!summary.is_empty()).then(|| Arc::<str>::from(summary)),
                (!details.trim().is_empty()).then(|| Arc::<str>::from(details.trim())),
            )
        },
    );
    CallableDocumentation::try_new(
        summary,
        details,
        Vec::new(),
        DocumentationProvenance::ProjectSource {
            declaration: symbol.declaration().clone(),
        },
    )
    .map_err(|_| identity_mismatch(symbol))
}

fn identity_mismatch(symbol: &CallableSymbol) -> CallableCatalogBuildError {
    CallableCatalogBuildError::ProjectIdentityMismatch {
        declaration: symbol.declaration().clone(),
    }
}

fn finish_project(
    modules: Vec<RegisteredProjectModuleCallables>,
    records: Vec<Arc<CallableRecord>>,
    bindings: Vec<(ProjectCallablePath, ProjectNameBinding)>,
    work: &mut CatalogBuildWork,
) -> Result<ProjectCallableCatalog, CallableCatalogBuildError> {
    let mut by_declaration = HashMap::new();
    for record in records {
        work.charge(1)?;
        let CallableCandidateId::Project(declaration) = record.id() else {
            return Err(CallableCatalogBuildError::InvalidRecord(
                super::CallableCatalogError::IdKeyMismatch,
            ));
        };
        let declaration = declaration.clone();
        if by_declaration.insert(declaration.clone(), record).is_some() {
            return Err(CallableCatalogBuildError::DuplicateTypedId {
                id: Box::new(CallableCandidateId::Project(declaration)),
            });
        }
    }
    let mut by_path: HashMap<ProjectCallablePath, ProjectNameBinding> = HashMap::new();
    for (path, binding) in bindings {
        work.charge(1)?;
        if let Some(first) = by_path.get_mut(&path) {
            *first = merge_project_value_binding(&path, first.clone(), binding)?;
        } else {
            by_path.insert(path, binding);
        }
    }
    Ok(ProjectCallableCatalog::new(
        modules,
        by_declaration,
        by_path,
    ))
}

fn merge_project_value_binding(
    path: &ProjectCallablePath,
    first: ProjectNameBinding,
    second: ProjectNameBinding,
) -> Result<ProjectNameBinding, CallableCatalogBuildError> {
    if first == second {
        return Ok(first);
    }
    match (first, second) {
        (ProjectNameBinding::Callable(left), ProjectNameBinding::Callable(right)) => {
            Ok(ambiguous_project_callables([left, right]))
        }
        (
            ProjectNameBinding::AmbiguousCallables { declarations },
            ProjectNameBinding::Callable(declaration),
        )
        | (
            ProjectNameBinding::Callable(declaration),
            ProjectNameBinding::AmbiguousCallables { declarations },
        ) => Ok(ambiguous_project_callables(
            declarations.iter().cloned().chain([declaration]),
        )),
        (
            ProjectNameBinding::AmbiguousCallables { declarations: left },
            ProjectNameBinding::AmbiguousCallables {
                declarations: right,
            },
        ) => Ok(ambiguous_project_callables(
            left.iter().cloned().chain(right.iter().cloned()),
        )),
        (value @ ProjectNameBinding::Callable(_), ProjectNameBinding::NonCallable { .. })
        | (
            value @ ProjectNameBinding::AmbiguousCallables { .. },
            ProjectNameBinding::NonCallable { .. },
        )
        | (value @ ProjectNameBinding::Environment(_), ProjectNameBinding::NonCallable { .. })
        | (ProjectNameBinding::NonCallable { .. }, value @ ProjectNameBinding::Callable(_))
        | (
            ProjectNameBinding::NonCallable { .. },
            value @ ProjectNameBinding::AmbiguousCallables { .. },
        )
        | (ProjectNameBinding::NonCallable { .. }, value @ ProjectNameBinding::Environment(_)) => {
            Ok(value)
        }
        (first, second) => Err(CallableCatalogBuildError::ProjectBindingCollision {
            path: Box::new(path.clone()),
            first: Box::new(first),
            second: Box::new(second),
        }),
    }
}

fn ambiguous_project_callables(
    declarations: impl IntoIterator<Item = arcweft_lang_hir::symbol::CallableDeclarationKey>,
) -> ProjectNameBinding {
    let mut declarations = declarations.into_iter().collect::<Vec<_>>();
    declarations.sort();
    declarations.dedup();
    match declarations.as_slice() {
        [declaration] => ProjectNameBinding::Callable(declaration.clone()),
        _ => ProjectNameBinding::AmbiguousCallables {
            declarations: declarations.into(),
        },
    }
}

fn finish_environment(
    publications: Vec<EnvironmentCallablePublication>,
    limits: &CallableLimits,
    work: &mut CatalogBuildWork,
) -> Result<EnvironmentCallableCatalog, CallableCatalogBuildError> {
    let mut records = Vec::new();
    let mut by_id = HashMap::new();
    for publication in publications {
        let publication_digest = publication.digest();
        for publication_record in publication.records() {
            let id = EnvironmentCallableId::new(
                publication.owner().clone(),
                publication_record.kind(),
                publication_record.key().clone(),
                publication_record.overload(),
            );
            let candidate = CallableCandidateId::Environment(id.clone());
            if by_id.contains_key(&id) {
                return Err(CallableCatalogBuildError::DuplicateTypedId {
                    id: Box::new(candidate),
                });
            }
            let record = Arc::new(CallableRecord::try_new(
                candidate,
                publication_record.key().clone(),
                publication.owner().authority(),
                publication.owner().provider(),
                CallableAccess::Environment,
                Arc::new(publication_record.schema().clone()),
                publication_record.documentation().clone(),
                publication_record.source().cloned(),
                publication_record.rust().cloned(),
                Some(publication_digest),
                publication_record.declaration_order(),
            )?);
            by_id.insert(id, Arc::clone(&record));
            work.charge(1)?;
            records.push(record);
        }
    }

    validate_environment_groups(&records, work)?;
    let mut sets: HashMap<CallableLookupKey, Vec<Arc<CallableRecord>>> = HashMap::new();
    for record in records {
        sets.entry(record.key().clone()).or_default().push(record);
    }
    let mut free = HashMap::new();
    let mut methods = HashMap::new();
    for (key, records) in sets {
        let entries = coalesce_records(records, limits, work)?;
        let set = NonEmptyCallableSet::try_new(entries, limits)?;
        match key {
            CallableLookupKey::Free(path) => {
                work.charge(1)?;
                free.insert(path, set);
            }
            CallableLookupKey::Method(method) => {
                work.charge(1)?;
                methods.insert(method, set);
            }
        }
    }
    Ok(EnvironmentCallableCatalog::new(free, methods, by_id))
}

fn validate_environment_groups(
    records: &[Arc<CallableRecord>],
    work: &mut CatalogBuildWork,
) -> Result<(), CallableCatalogBuildError> {
    let mut by_key_provider: HashMap<
        (CallableLookupKey, CallableProviderId),
        Vec<&Arc<CallableRecord>>,
    > = HashMap::new();
    let mut providers: HashMap<
        CallableLookupKey,
        Vec<(CallableAuthorityRank, CallableProviderId)>,
    > = HashMap::new();
    for record in records {
        let candidates = providers.entry(record.key().clone()).or_default();
        if !candidates
            .iter()
            .any(|(_, provider)| provider == record.provider())
        {
            for (rank, first) in candidates.iter() {
                work.charge(1)?;
                if *rank == record.authority() && first != record.provider() {
                    return Err(CallableCatalogBuildError::SameRankCollision {
                        key: Box::new(record.key().clone()),
                        rank: record.authority(),
                        first: Box::new(first.clone()),
                        second: Box::new(record.provider().clone()),
                    });
                }
            }
            candidates.push((record.authority(), record.provider().clone()));
        }
        by_key_provider
            .entry((record.key().clone(), record.provider().clone()))
            .or_default()
            .push(record);
    }
    for ((key, provider), mut group) in by_key_provider {
        group.sort_by_key(|record| environment_overload(record));
        let mut seen = HashSet::new();
        for (expected, record) in group.into_iter().enumerate() {
            let actual = environment_overload(record);
            if !seen.insert(actual) {
                return Err(CallableCatalogBuildError::DuplicateProviderOverload {
                    key: Box::new(key),
                    provider: Box::new(provider),
                    overload: actual,
                });
            }
            let expected = CallableOverloadIndex::try_from_usize(expected)
                .map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
            if actual != expected {
                return Err(CallableCatalogBuildError::NonContiguousOverloads {
                    key: Box::new(key),
                    provider: Box::new(provider),
                    expected,
                    actual,
                });
            }
        }
    }
    Ok(())
}

fn environment_overload(record: &CallableRecord) -> CallableOverloadIndex {
    let CallableCandidateId::Environment(id) = record.id() else {
        unreachable!("environment catalog contains only environment records")
    };
    id.overload()
}

fn coalesce_records(
    mut records: Vec<Arc<CallableRecord>>,
    limits: &CallableLimits,
    work: &mut CatalogBuildWork,
) -> Result<Vec<CatalogCallableEntry>, CallableCatalogBuildError> {
    records.sort_by(|left, right| {
        authority_order(left.authority())
            .cmp(&authority_order(right.authority()))
            .then_with(|| environment_overload(left).cmp(&environment_overload(right)))
            .then_with(|| left.declaration_order().cmp(&right.declaration_order()))
    });
    let mut entries: Vec<(Arc<CallableRecord>, Vec<EquivalentCallableSource>)> = Vec::new();
    for record in records {
        let mut duplicate = None;
        if record.authority() == CallableAuthorityRank::Adapter {
            for (index, (primary, _)) in entries.iter().enumerate() {
                work.charge(1)?;
                if primary.authority() != CallableAuthorityRank::Standard {
                    continue;
                }
                let schema_work = u64::try_from(primary.schema().groups().len())
                    .ok()
                    .and_then(|group_count| 1_u64.checked_add(group_count))
                    .and_then(|work| {
                        u64::try_from(primary.schema().total_parameters())
                            .ok()
                            .and_then(|parameter_count| work.checked_add(parameter_count))
                    })
                    .ok_or(CallableCatalogBuildError::WorkOverflow)?;
                work.charge(schema_work)?;
                if primary.schema().semantic_eq(record.schema()) {
                    duplicate = Some(index);
                    break;
                }
            }
        }
        if let Some(index) = duplicate {
            let (_, equivalents) = &mut entries[index];
            equivalents.push(EquivalentCallableSource::new(
                record.id().clone(),
                environment_origin(&record),
                record.documentation().clone(),
                record.source().cloned(),
                record.rust().cloned(),
            ));
        } else {
            entries.push((record, Vec::new()));
        }
    }
    entries
        .into_iter()
        .map(|(primary, equivalents)| {
            CatalogCallableEntry::try_new(primary, equivalents, limits)
                .map_err(CallableCatalogBuildError::InvalidRecord)
        })
        .collect()
}

const fn authority_order(authority: CallableAuthorityRank) -> u8 {
    match authority {
        CallableAuthorityRank::Project => 0,
        CallableAuthorityRank::Standard => 1,
        CallableAuthorityRank::Adapter => 2,
    }
}

fn environment_origin(record: &CallableRecord) -> SignatureOrigin {
    let CallableCandidateId::Environment(id) = record.id() else {
        unreachable!("environment origin requires an environment record")
    };
    match id.owner() {
        EnvironmentCallableOwner::Standard(owner) => SignatureOrigin::Standard {
            owner: *owner,
            id: id.clone(),
        },
        EnvironmentCallableOwner::Adapter(package) => SignatureOrigin::Adapter {
            package: package.clone(),
            id: id.clone(),
        },
    }
}

impl TypeCheckEnv {
    pub(crate) fn standard_callable_publication(
        &self,
        nominal_world: AcceptedNominalWorldStamp,
        limits: &CallableLimits,
    ) -> Result<EnvironmentCallablePublication, super::CallablePublicationError> {
        let owner = EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core);
        let mut records = Vec::new();
        let mut functions = self.standard_functions().iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| left.path.cmp(&right.path));
        for (ordinal, function) in functions.into_iter().enumerate() {
            let key = CallableLookupKey::Free(function.path.clone());
            let signature = self.canonical_standard_callable_signature(function.signature.clone());
            records.push(environment_record_from_signature(
                EnvironmentCallableKind::Function,
                key,
                &signature,
                Some(function.effects.as_slice()),
                function.validator.clone(),
                function.evaluated_effect,
                ordinal,
                limits,
            )?);
        }
        let offset = records.len();
        let mut methods = self.standard_methods().iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.member
                .cmp(&right.member)
                .then_with(|| left.receiver.stable_ordering(&right.receiver))
        });
        for (ordinal, method) in methods.into_iter().enumerate() {
            let receiver = self.canonical_standard_callable_type(method.receiver.clone());
            let signature = self.canonical_standard_callable_signature(method.signature.clone());
            let projection = StandardEnvironmentMethodProjection::try_from_signature(
                &receiver,
                &method.member,
                &signature,
                method.role,
                limits,
            )?;
            records.push(projection.into_publication_record(offset + ordinal)?);
        }
        let manifest_digest = standard_manifest_digest(&records);
        EnvironmentCallablePublication::try_new_projected(
            owner,
            nominal_world,
            manifest_digest,
            records,
            limits,
        )
    }
}

fn standard_manifest_digest(
    records: &[EnvironmentCallablePublicationRecord],
) -> EnvironmentManifestDigest {
    const DOMAIN: &[u8] = b"arcweft.standard-environment-manifest.v1\0";

    let mut records = records.iter().collect::<Vec<_>>();
    records.sort_by_key(|record| record.declaration_order());
    let mut encoder = CanonicalEncoder::default();
    encoder.usize(records.len());
    for record in records {
        encoder.environment_kind(record.kind());
        encoder.lookup_key(record.key());
        encoder.usize(record.overload().get());
        encoder.bytes(record.schema().semantic_digest().as_bytes());
    }
    EnvironmentManifestDigest::from_bytes(encoder.finish(DOMAIN))
}

fn environment_record_from_signature(
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    signature: &FunctionSignature,
    effects: Option<&[crate::env::EffectCapability]>,
    validator: CallableValidator,
    evaluated_effect: Option<CallableEvaluatedEffect>,
    ordinal: usize,
    limits: &CallableLimits,
) -> Result<EnvironmentCallablePublicationRecord, super::CallablePublicationError> {
    let effects = EffectSet::from_labels(
        effects
            .unwrap_or_default()
            .iter()
            .map(crate::env::EffectCapability::as_str),
    )
    .map_err(|_| super::CallablePublicationError::InvalidOverload)?;
    let mut schema = signature.callable_schema(
        EffectRow::closed(effects),
        validator,
        CallableGenericParameterIssuer::empty(),
        limits,
    )?;
    if let Some(effect) = evaluated_effect {
        schema = schema.with_evaluated_effect(effect);
    }
    EnvironmentCallablePublicationRecord::try_new(
        kind,
        key,
        CallableOverloadIndex::try_from_usize(0)
            .map_err(|_| super::CallablePublicationError::InvalidOverload)?,
        schema,
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(ordinal)
            .map_err(|_| super::CallablePublicationError::InvalidOverload)?,
    )
}
