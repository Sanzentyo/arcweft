//! Fail-closed construction of the immutable registered callable catalog.
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    callable_source::HirCallableSignatureSource,
    model::HirTopLevelDecl,
    project::HirProject,
    symbol::{CallableDeclarationOwner, ProjectSymbolTable, ProjectSymbolTargetId},
};
use arcweft_lang_syntax::{
    ast::items::{ExternModMember, ExternModSource},
    ast::pattern::Pattern,
    types::{FnParam, FnParamGroup, FnParamKind},
};

use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    env::{FunctionSignature, TypeCheckEnv},
    nominal::{CheckedTypeReferenceCache, NominalResolutionIndex},
    registration::{AcceptedNominalWorld, AcceptedNominalWorldStamp, EnvironmentManifestDigest},
    types::{GenericTypeOwnerId, TypeKind},
};

use super::digest::CanonicalEncoder;
use super::limits::CatalogBuildWork;
use super::nominal_signature::ProjectSignatureResolver;
use super::{
    CallableArgumentPolicy, CallableAuthorityRank, CallableBuildLimitError, CallableCandidateId,
    CallableCatalogBuildError, CallableDocumentation, CallableEffectSchema, CallableGroupIndex,
    CallableGroupKind, CallableLimits, CallableLookupKey, CallableName, CallableOverloadIndex,
    CallableParameter, CallableParameterDocumentation, CallableParameterGroup,
    CallableParameterIndex, CallableParameterPassing, CallableParameterPresence,
    CallableParameterSource, CallableParameterType, CallablePath, CallablePathError,
    CallableProviderId, CallableRecord, CallableSignatureSchema, CallableSource, CallableValidator,
    CatalogCallableEntry, DocumentationProvenance, EnvironmentCallableCatalog,
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
    rust_extern_aliases: Vec<RustExternAliasSeed>,
    environment_publications: Vec<EnvironmentCallablePublication>,
    nominal_resolutions: NominalResolutionIndex,
    nominal_cache: CheckedTypeReferenceCache,
    work: CatalogBuildWork,
}

struct ProjectParameterPublication {
    groups: Vec<CallableParameterGroup>,
    documentation: Vec<CallableParameterDocumentation>,
    sources: Vec<CallableParameterSource>,
}

struct RustExternAliasSeed {
    path: ProjectCallablePath,
    package: String,
    export: CallableName,
    signature: FunctionSignature,
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
                CallableValidator::Ordinary
            } else {
                CallableValidator::Untyped
            },
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

    pub(crate) const fn id(&self) -> &EnvironmentCallableId {
        &self.id
    }

    pub(crate) const fn schema(&self) -> &CallableSignatureSchema {
        &self.schema
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
            rust_extern_aliases: Vec::new(),
            environment_publications: Vec::new(),
            nominal_resolutions: NominalResolutionIndex::production(),
            nominal_cache: CheckedTypeReferenceCache::default(),
            work: CatalogBuildWork::new(limits.max_catalog_build_work()),
        }
    }

    pub(crate) fn add_project(
        &mut self,
        project: &HirProject,
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
        for (module, hir) in project.modules() {
            self.work.charge(1)?;
            let source = project.source(module).ok_or_else(|| {
                CallableCatalogBuildError::MissingProjectModuleSource {
                    module: module.clone(),
                }
            })?;
            let sources = project
                .module_callable_signature_sources(module)
                .ok_or_else(|| CallableCatalogBuildError::MissingProjectModuleSource {
                    module: module.clone(),
                })?;
            let mut declarations = Vec::with_capacity(sources.len());
            for source_record in sources {
                let ordinal = self.project_records.len();
                let record = Arc::new(self.project_record(
                    source_record,
                    ordinal,
                    project,
                    symbols,
                    nominal_world,
                )?);
                declarations.push(source_record.declaration().clone());
                if source_record.declaration().owner() == CallableDeclarationOwner::ExternCapability
                {
                    let CallableLookupKey::Free(path) = record.key() else {
                        return Err(CallableCatalogBuildError::InvalidRecord(
                            super::CallableCatalogError::IdKeyMismatch,
                        ));
                    };
                    self.project_bindings.push((
                        ProjectCallablePath::new(
                            project.package().clone(),
                            module.clone(),
                            path.clone(),
                        ),
                        ProjectNameBinding::Callable(source_record.declaration().clone()),
                    ));
                }
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
                    module.clone(),
                    source.clone(),
                    declarations,
                ));
            self.add_rust_extern_aliases(project, symbols, nominal_world, module, hir)?;
        }
        Ok(())
    }

    fn add_rust_extern_aliases(
        &mut self,
        project: &HirProject,
        symbols: &ProjectSymbolTable,
        nominal_world: &AcceptedNominalWorld,
        module: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
        hir: &arcweft_lang_hir::model::HirModule,
    ) -> Result<(), CallableCatalogBuildError> {
        for declaration in hir.declarations() {
            let HirTopLevelDecl::ExternMod(item) = declaration else {
                continue;
            };
            if item.abi() != "rust" {
                continue;
            }
            let Some(ExternModSource::Crate(package)) = item.source() else {
                continue;
            };
            for member in item.members() {
                let ExternModMember::Function(function) = member else {
                    continue;
                };
                self.work.charge(1)?;
                let export = CallableName::try_new(function.signature().name())
                    .map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
                let path = item
                    .path()
                    .segments()
                    .iter()
                    .map(|segment| CallableName::try_new(segment.as_str()))
                    .chain(std::iter::once(Ok(export.clone())))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
                let path = CallablePath::try_new(path)
                    .map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
                let declaration_source = hir.source_span(*item.range()).ok_or_else(|| {
                    CallableCatalogBuildError::MissingProjectModuleSource {
                        module: module.clone(),
                    }
                })?;
                if function
                    .signature()
                    .generic_params()
                    .iter()
                    .any(|parameter| parameter.as_type_param().is_some())
                {
                    return Err(CallableCatalogBuildError::InvalidProjectSignatureSource {
                        span: declaration_source,
                    });
                }
                self.rust_extern_aliases.push(RustExternAliasSeed {
                    path: ProjectCallablePath::new(project.package().clone(), module.clone(), path),
                    package: package.clone(),
                    export,
                    signature: ProjectSignatureResolver::new(
                        project,
                        symbols,
                        nominal_world,
                        &mut self.nominal_resolutions,
                        &mut self.nominal_cache,
                    )
                    .resolve_function_signature(
                        module,
                        hir,
                        function.signature(),
                        &GenericTypeOwnerId::AcceptedSource(declaration_source),
                    )?,
                });
            }
        }
        Ok(())
    }

    fn project_record(
        &mut self,
        source: &HirCallableSignatureSource,
        ordinal: usize,
        project: &HirProject,
        symbols: &ProjectSymbolTable,
        nominal_world: &AcceptedNominalWorld,
    ) -> Result<CallableRecord, CallableCatalogBuildError> {
        self.work.charge(1)?;
        let path_segment_count = source
            .path()
            .qualifiers()
            .len()
            .checked_add(1)
            .ok_or(CallableCatalogBuildError::WorkOverflow)?;
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
        .resolve_project_signature(source)?;
        let parameters = project_parameters(
            source,
            &resolved.parameter_types,
            &self.limits,
            &mut self.work,
        )?;
        let declared = EffectSet::from_labels(
            source
                .effects()
                .declared()
                .iter()
                .map(arcweft_lang_hir::callable_source::HirEffectName::as_str),
        )
        .map_err(|_| identity_mismatch(source))?;
        let effects = if source.declaration().owner() == CallableDeclarationOwner::ExternCapability
        {
            CallableEffectSchema::fixed(EffectRow::closed(declared))
        } else {
            CallableEffectSchema::project(source.declaration().clone(), EffectRow::closed(declared))
        };
        let schema = Arc::new(CallableSignatureSchema::try_new(
            parameters.groups,
            resolved.return_type,
            effects,
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                if source
                    .signature()
                    .param_groups()
                    .iter()
                    .flat_map(FnParamGroup::params)
                    .any(FnParam::is_rest)
                {
                    SpreadArgumentPolicy::TypedRest
                } else {
                    SpreadArgumentPolicy::Reject
                },
            ),
            CallableValidator::Ordinary,
            &self.limits,
        )?);
        let documentation = project_documentation(source, parameters.documentation)?;
        let callable_source = CallableSource::try_new(
            Some(source.declaration().clone()),
            Some(source.signature_span().clone()),
            Some(source.name_span().clone()),
            source.result_span().cloned(),
            parameters.sources,
        )
        .map_err(|_| identity_mismatch(source))?;
        CallableRecord::try_new(
            CallableCandidateId::Project(source.declaration().clone()),
            CallableLookupKey::Free(callable_path(source)?),
            CallableAuthorityRank::Project,
            CallableProviderId::Project(source.package().clone()),
            schema,
            documentation,
            Some(callable_source),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(ordinal)
                .map_err(|_| identity_mismatch(source))?,
        )
        .map_err(CallableCatalogBuildError::InvalidRecord)
    }

    /// Publishes every source-visible project spelling after registration has
    /// assigned exact semantic types to non-callable external owners.
    pub(crate) fn add_project_bindings(
        &mut self,
        project: &HirProject,
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
                ProjectSymbolTargetId::External(_)
                | ProjectSymbolTargetId::Nominal(_)
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
        bind_rust_extern_aliases(
            self.rust_extern_aliases,
            &environment,
            &mut self.project_bindings,
            &mut self.work,
        )?;
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

fn bind_rust_extern_aliases(
    aliases: Vec<RustExternAliasSeed>,
    environment: &EnvironmentCallableCatalog,
    bindings: &mut Vec<(ProjectCallablePath, ProjectNameBinding)>,
    work: &mut CatalogBuildWork,
) -> Result<(), CallableCatalogBuildError> {
    for alias in aliases {
        work.charge(1)?;
        let matching = environment
            .rust_exports(&alias.package, &alias.export)
            .into_iter()
            .filter(|record| record.schema().matches_function_signature(&alias.signature))
            .collect::<Vec<_>>();
        match matching.as_slice() {
            [] => {}
            [record] => {
                let CallableCandidateId::Environment(id) = record.id() else {
                    return Err(CallableCatalogBuildError::InvalidRecord(
                        super::CallableCatalogError::IdKeyMismatch,
                    ));
                };
                bindings.push((alias.path, ProjectNameBinding::Environment(id.clone())));
            }
            candidates => {
                return Err(CallableCatalogBuildError::AmbiguousRustExternBinding {
                    path: alias.path,
                    package: alias.package.clone(),
                    export: alias.export.clone(),
                    candidates: candidates.len(),
                });
            }
        }
    }
    Ok(())
}

fn project_parameters(
    source: &HirCallableSignatureSource,
    resolved_groups: &[Vec<TypeKind>],
    limits: &CallableLimits,
    work: &mut CatalogBuildWork,
) -> Result<ProjectParameterPublication, CallableCatalogBuildError> {
    let mut groups = Vec::with_capacity(source.signature().param_groups().len());
    let mut documentation = Vec::new();
    let mut sources = Vec::new();
    for (group_index, group) in source.signature().param_groups().iter().enumerate() {
        work.charge(1)?;
        let group_id = CallableGroupIndex::try_from_usize(group_index)
            .map_err(|_| identity_mismatch(source))?;
        let mut parameters = Vec::with_capacity(group.params().len());
        for (parameter_index, parameter) in group.params().iter().enumerate() {
            work.charge(1)?;
            let parameter_id = CallableParameterIndex::try_from_usize(parameter_index)
                .map_err(|_| identity_mismatch(source))?;
            let source_parameter = source
                .parameter_spans()
                .iter()
                .find(|candidate| {
                    usize::from(candidate.group()) == group_index
                        && usize::from(candidate.parameter()) == parameter_index
                })
                .ok_or_else(|| identity_mismatch(source))?;
            let parameter_source = CallableParameterSource::try_new(
                group_id,
                parameter_id,
                source_parameter.whole().clone(),
                source_parameter.name().cloned(),
                source_parameter.ty().cloned(),
                source_parameter.default().cloned(),
            )
            .map_err(|_| identity_mismatch(source))?;
            sources.push(parameter_source.clone());
            let parameter_documentation = parameter.doc().map(|doc| Arc::<str>::from(doc.text()));
            if let Some(parameter_documentation) = &parameter_documentation {
                work.charge(1)?;
                documentation.push(
                    CallableParameterDocumentation::try_new(
                        group_id,
                        parameter_id,
                        Arc::clone(parameter_documentation),
                    )
                    .map_err(|_| identity_mismatch(source))?,
                );
            }
            parameters.push(
                CallableParameter::try_new(
                    parameter_id,
                    parameter_name(parameter).map_err(|_| identity_mismatch(source))?,
                    CallableParameterType::Exact(
                        resolved_groups
                            .get(group_index)
                            .and_then(|group| group.get(parameter_index))
                            .cloned()
                            .ok_or_else(|| identity_mismatch(source))?,
                    ),
                    parameter_passing(parameter),
                    if parameter.default().is_some() {
                        CallableParameterPresence::Defaulted
                    } else {
                        CallableParameterPresence::Required
                    },
                    parameter_documentation,
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
        documentation,
        sources,
    })
}

fn callable_path(
    source: &HirCallableSignatureSource,
) -> Result<CallablePath, CallableCatalogBuildError> {
    let segments = source
        .path()
        .qualifiers()
        .iter()
        .map(|segment| CallableName::try_new(segment.as_str()))
        .chain(std::iter::once(CallableName::try_new(source.path().leaf())))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| identity_mismatch(source))?;
    CallablePath::try_new(segments).map_err(|_| identity_mismatch(source))
}

fn parameter_name(parameter: &FnParam) -> Result<Option<CallableName>, super::CallableScalarError> {
    let name = parameter
        .pattern()
        .simple_binding_name()
        .or_else(|| parameter.receiver_kind().is_some().then_some("self"));
    name.map(CallableName::try_new).transpose()
}

fn parameter_passing(parameter: &FnParam) -> CallableParameterPassing {
    if parameter.kind() == FnParamKind::Rest {
        CallableParameterPassing::RestPositional
    } else if matches!(
        parameter.pattern(),
        Pattern::Ident(_) | Pattern::MutIdent(_)
    ) {
        CallableParameterPassing::PositionalOrNamed
    } else {
        CallableParameterPassing::PositionalOnly
    }
}

fn project_documentation(
    source: &HirCallableSignatureSource,
    parameters: Vec<CallableParameterDocumentation>,
) -> Result<CallableDocumentation, CallableCatalogBuildError> {
    let Some(doc) = source.documentation() else {
        if parameters.is_empty() {
            return Ok(CallableDocumentation::missing());
        }
        return CallableDocumentation::try_new(
            None,
            None,
            parameters,
            DocumentationProvenance::ProjectSource {
                declaration: source.declaration().clone(),
            },
        )
        .map_err(|_| identity_mismatch(source));
    };
    let (summary, details) = doc.text().split_once('\n').map_or_else(
        || (Some(Arc::<str>::from(doc.text())), None),
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
        parameters,
        DocumentationProvenance::ProjectSource {
            declaration: source.declaration().clone(),
        },
    )
    .map_err(|_| identity_mismatch(source))
}

fn identity_mismatch(source: &HirCallableSignatureSource) -> CallableCatalogBuildError {
    CallableCatalogBuildError::ProjectIdentityMismatch {
        declaration: source.declaration().clone(),
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
    declarations: impl IntoIterator<Item = arcweft_lang_hir::symbol::CallableDeclarationId>,
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
    pub(crate) fn standard_method_projection(
        &self,
        receiver: &TypeKind,
        member: &CallableName,
        limits: &CallableLimits,
    ) -> Result<Option<StandardEnvironmentMethodProjection>, super::CallablePublicationError> {
        self.method_signature(receiver, member.as_str())
            .map(|signature| {
                StandardEnvironmentMethodProjection::try_from_signature(
                    receiver, member, signature, limits,
                )
            })
            .transpose()
    }

    pub(crate) fn standard_callable_publication(
        &self,
        nominal_world: AcceptedNominalWorldStamp,
        limits: &CallableLimits,
    ) -> Result<EnvironmentCallablePublication, super::CallablePublicationError> {
        let owner = EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core);
        let mut records = Vec::new();
        let mut functions = self
            .functions
            .iter()
            .map(|(path, result)| {
                (
                    path.clone(),
                    self.function_signatures
                        .get(path)
                        .cloned()
                        .unwrap_or_else(|| FunctionSignature::return_only(result.clone())),
                )
            })
            .collect::<Vec<_>>();
        functions.extend(
            self.function_signatures
                .iter()
                .filter(|(path, _)| !self.functions.contains_key(*path))
                .map(|(path, signature)| (path.clone(), signature.clone())),
        );
        functions.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (ordinal, (path, signature)) in functions.into_iter().enumerate() {
            let key = CallableLookupKey::Free(callable_path_from_storage(&path)?);
            records.push(environment_record_from_signature(
                EnvironmentCallableKind::Function,
                key,
                &signature,
                self.function_effects.get(&path).map(Vec::as_slice),
                ordinal,
                limits,
            )?);
        }
        let offset = records.len();
        let mut methods = self.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|((left_ty, left), _), ((right_ty, right), _)| {
            left.cmp(right)
                .then_with(|| left_ty.stable_ordering(right_ty))
        });
        for (ordinal, ((receiver, name), method)) in methods.into_iter().enumerate() {
            let member = CallableName::try_new(name.as_str())
                .map_err(|_| super::CallablePublicationError::InvalidOverload)?;
            let projection = StandardEnvironmentMethodProjection::try_from_signature(
                receiver,
                &member,
                &method.signature,
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

fn callable_path_from_storage(path: &str) -> Result<CallablePath, super::CallablePublicationError> {
    let segments = path
        .split('.')
        .map(CallableName::try_new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| super::CallablePublicationError::InvalidOverload)?;
    CallablePath::try_new(segments).map_err(|_| super::CallablePublicationError::InvalidOverload)
}

fn environment_record_from_signature(
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    signature: &FunctionSignature,
    effects: Option<&[crate::env::EffectCapability]>,
    ordinal: usize,
    limits: &CallableLimits,
) -> Result<EnvironmentCallablePublicationRecord, super::CallablePublicationError> {
    let validator = if signature.checks_args() {
        CallableValidator::Ordinary
    } else {
        CallableValidator::Untyped
    };
    let effects = EffectSet::from_labels(
        effects
            .unwrap_or_default()
            .iter()
            .map(crate::env::EffectCapability::as_str),
    )
    .map_err(|_| super::CallablePublicationError::InvalidOverload)?;
    let schema = signature.callable_schema(EffectRow::closed(effects), validator, limits)?;
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
