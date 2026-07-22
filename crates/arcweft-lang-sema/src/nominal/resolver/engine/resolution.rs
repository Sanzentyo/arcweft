use std::collections::BTreeMap;

use arcweft_lang_hir::symbol::{
    ExternalSymbol, ProjectSymbolTable, ProjectTypeLookupError, ProjectTypeTarget,
    nominal::{ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationKind},
};
use arcweft_lang_syntax::{
    ast::symbol_path::SymbolPath,
    types::{TypePath, TypeRefNodePath},
};

use crate::{
    env::nominal::{
        AcceptedNominalOwnerId, AcceptedNominalRecord, AcceptedNominalSemantics,
        OpenNominalEnvironment, OpenNominalRule,
    },
    registration::{
        AcceptedNominalWorld, ExternalOwnerLookupError, RegisteredExternalOwner,
        RegisteredExternalOwnerKind,
    },
    types::{
        AcceptedNominalType, ArrayLength, GenericTypeOwnerId, GenericTypeParameterId, MapKind,
        OpenNominalType, ProjectNominalType, TypeKind, TypePoisonId,
    },
};

use super::{
    AliasBinding, AliasExpansionFact, BuiltinTypeConstructor, DetachedNominalEvidence,
    DetachedNominalReason, ExternalNominalResolution, GenericContext, NameResult, NodeValue,
    NominalDiagnosticRelated, NominalRelatedMessage, NominalResolutionLimitKind, ProjectNameLookup,
    ProjectSelection, ResolvedAliasReference, ResolvedOpenNominal, Resolver, SelfTypeScope,
    SourceContext, TypeArgumentExpectation, TypeArgumentKind, TypeArityExpectation,
    TypeArityTarget, TypeNameResolution, TypePoisonOrigin, TypeResolutionFailure,
    TypeResolutionInputError, TypeResolutionWorld, canonical_cycle, canonical_poisons,
    evidence_from_project, open_expectation,
};

enum ExternalRecordLookup {
    Record(AcceptedNominalRecord),
    BudgetExceeded(Box<NameResult>),
}

struct ExternalResolutionSite<'a, 'source> {
    context: &'a SourceContext<'source>,
    node: &'a TypeRefNodePath,
    symbols: &'a ProjectSymbolTable,
    environment: &'a AcceptedNominalWorld,
    external: &'a ExternalSymbol,
}

impl Resolver<'_, '_> {
    pub(super) fn resolve_name(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        path: &TypePath,
        arguments: Vec<(TypeRefNodePath, NodeValue)>,
        depth: u16,
    ) -> Result<NameResult, TypeResolutionInputError> {
        let actual = u16::try_from(arguments.len()).expect("parser generic-argument cap fits u16");
        let child_causes = arguments
            .iter()
            .flat_map(|(_, argument)| argument.causes.iter().copied())
            .collect::<Vec<_>>();

        if let Some(scoped) = self.resolve_scoped_name(context, node, path, actual, &child_causes) {
            return Ok(scoped);
        }

        if let Some(builtin) = BuiltinTypeConstructor::from_type_path(path) {
            let expected = TypeArityExpectation::Exact(builtin.arity());
            if !expected.contains(actual) {
                return Ok(self.failed_name(
                    context,
                    node,
                    TypeResolutionFailure::WrongArity {
                        target: TypeArityTarget::Builtin(builtin),
                        expected,
                        actual,
                    },
                    child_causes,
                    Vec::new(),
                ));
            }
            let value = self.apply_builtin(context, builtin, arguments, child_causes);
            return Ok(NameResult {
                value,
                outcome: TypeNameResolution::Builtin(builtin),
            });
        }

        if matches!(self.input.world(), TypeResolutionWorld::Accepted { .. }) {
            match self.lookup_project_name(context, node, path, &child_causes) {
                ProjectNameLookup::Absent => {}
                ProjectNameLookup::Failed(failed) => return Ok(*failed),
                ProjectNameLookup::Selected(ProjectSelection::Nominal(declaration)) => {
                    return self.resolve_project_nominal(
                        context,
                        node,
                        &declaration,
                        arguments,
                        child_causes,
                        depth,
                    );
                }
                ProjectNameLookup::Selected(ProjectSelection::External(external)) => {
                    let TypeResolutionWorld::Accepted {
                        symbols,
                        environment,
                    } = self.input.world()
                    else {
                        unreachable!("project lookup only selects externals in accepted worlds")
                    };
                    return self.resolve_external(
                        &ExternalResolutionSite {
                            context,
                            node,
                            symbols,
                            environment,
                            external: &external,
                        },
                        arguments,
                        child_causes,
                    );
                }
            }
        }

        Ok(self.resolve_environment_name(context, node, path, arguments, child_causes, actual))
    }

    fn resolve_scoped_name(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        path: &TypePath,
        actual: u16,
        child_causes: &[TypePoisonId],
    ) -> Option<NameResult> {
        if super::direct_name(path) == Some("Self") {
            let resolved = match self.input.self_scope() {
                SelfTypeScope::Known(ty) if actual == 0 => NameResult {
                    value: NodeValue::typed(ty.clone(), child_causes.iter().copied()),
                    outcome: TypeNameResolution::SelfType(ty.clone()),
                },
                SelfTypeScope::Poisoned(poison) if actual == 0 => {
                    let poison = *poison;
                    self.record_poison(
                        poison,
                        TypePoisonOrigin::UpstreamTypeDiagnostic,
                        context.evidence(node, true),
                        true,
                    );
                    NameResult {
                        value: NodeValue::error(poison, child_causes.iter().copied()),
                        outcome: TypeNameResolution::Poisoned(poison),
                    }
                }
                SelfTypeScope::Known(_) | SelfTypeScope::Poisoned(_) | SelfTypeScope::Absent => {
                    self.failed_name(
                        context,
                        node,
                        TypeResolutionFailure::SelfUnavailable,
                        child_causes.to_vec(),
                        Vec::new(),
                    )
                }
            };
            return Some(resolved);
        }
        let (id, ty) = context.generic(path)?;
        (actual == 0).then(|| NameResult {
            value: NodeValue::typed(ty, child_causes.iter().copied()),
            outcome: TypeNameResolution::Generic(id),
        })
    }

    fn lookup_project_name(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        path: &TypePath,
        child_causes: &[TypePoisonId],
    ) -> ProjectNameLookup {
        let TypeResolutionWorld::Accepted { symbols, .. } = self.input.world() else {
            return ProjectNameLookup::Absent;
        };
        let source = context
            .evidence(node, true)
            .project()
            .expect("accepted input has project source evidence")
            .clone();
        let current_module = context
            .module
            .expect("accepted input and alias targets have an owning module");
        let lookup = symbols.resolve_type_target(current_module, path, source);
        match lookup {
            Ok(ProjectTypeTarget::Nominal(declaration)) => self.charged_project_selection(
                context,
                node,
                child_causes,
                ProjectSelection::Nominal(Box::new(declaration.clone())),
            ),
            Ok(ProjectTypeTarget::External(external)) => self.charged_project_selection(
                context,
                node,
                child_causes,
                ProjectSelection::External(Box::new(external.clone())),
            ),
            Err(ProjectTypeLookupError::Unknown { .. }) => ProjectNameLookup::Absent,
            Err(ProjectTypeLookupError::Ambiguous { candidates, .. }) => self
                .project_lookup_failure(
                    context,
                    node,
                    child_causes,
                    candidates.len() as u64,
                    TypeResolutionFailure::Ambiguous {
                        path: path.clone(),
                        candidates: candidates.clone(),
                    },
                    Self::candidate_related(&candidates),
                ),
            Err(ProjectTypeLookupError::Inaccessible { candidates, .. }) => self
                .project_lookup_failure(
                    context,
                    node,
                    child_causes,
                    candidates.len() as u64,
                    TypeResolutionFailure::Inaccessible {
                        path: path.clone(),
                        candidates: candidates.clone(),
                    },
                    Self::candidate_related(&candidates),
                ),
            Err(ProjectTypeLookupError::WrongKind { actual, .. }) => self.project_lookup_failure(
                context,
                node,
                child_causes,
                1,
                TypeResolutionFailure::WrongKind {
                    path: path.clone(),
                    actual: actual.as_ref().clone(),
                },
                Self::candidate_related(core::slice::from_ref(actual.as_ref())),
            ),
            Err(ProjectTypeLookupError::InvalidPath { .. }) => {
                ProjectNameLookup::Failed(Box::new(self.failed_name(
                    context,
                    node,
                    TypeResolutionFailure::Unknown { path: path.clone() },
                    child_causes.to_vec(),
                    Vec::new(),
                )))
            }
        }
    }

    fn charged_project_selection(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        child_causes: &[TypePoisonId],
        selection: ProjectSelection,
    ) -> ProjectNameLookup {
        self.charge_name_work(1, context, node, child_causes.to_vec())
            .map_or(ProjectNameLookup::Selected(selection), |failed| {
                ProjectNameLookup::Failed(Box::new(failed))
            })
    }

    fn project_lookup_failure(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        child_causes: &[TypePoisonId],
        work: u64,
        failure: TypeResolutionFailure,
        related: Vec<NominalDiagnosticRelated>,
    ) -> ProjectNameLookup {
        if let Some(failed) = self.charge_name_work(work, context, node, child_causes.to_vec()) {
            return ProjectNameLookup::Failed(Box::new(failed));
        }
        ProjectNameLookup::Failed(Box::new(self.failed_name(
            context,
            node,
            failure,
            child_causes.to_vec(),
            related,
        )))
    }

    fn resolve_environment_name(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        path: &TypePath,
        arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
        actual: u16,
    ) -> NameResult {
        let catalog = self.input.world().environment().nominal_catalog();
        if let Some(record) = catalog.exact(path).cloned() {
            if let Some(failed) = self.charge_name_work(1, context, node, child_causes.clone()) {
                return failed;
            }
            return self.resolve_accepted(context, node, &record, arguments, child_causes);
        }
        self.resolve_open_or_unavailable(context, node, path, arguments, child_causes, actual)
    }

    fn resolve_open_or_unavailable(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        path: &TypePath,
        arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
        actual: u16,
    ) -> NameResult {
        let environment_kind = match self.input.world() {
            TypeResolutionWorld::Accepted { .. } => OpenNominalEnvironment::Accepted,
            TypeResolutionWorld::Detached { .. } => OpenNominalEnvironment::Detached,
        };
        let rules = self
            .input
            .world()
            .environment()
            .nominal_catalog()
            .open_rules()
            .cloned()
            .collect::<Vec<_>>();
        let mut wrong_arity_rule = None;
        for rule in rules {
            if let Some(failed) = self.charge_name_work(1, context, node, child_causes.clone()) {
                return failed;
            }
            if rule.matches(environment_kind, context.module, path, actual) {
                return self.resolve_open(context, &rule, path, arguments, child_causes);
            }
            if wrong_arity_rule.is_none()
                && rule.matches(
                    environment_kind,
                    context.module,
                    path,
                    rule.arity().minimum(),
                )
            {
                wrong_arity_rule = Some(rule);
            }
        }
        if let Some(rule) = wrong_arity_rule {
            return self.failed_name(
                context,
                node,
                TypeResolutionFailure::WrongArity {
                    target: TypeArityTarget::Open(rule.id().clone()),
                    expected: open_expectation(rule.arity()),
                    actual,
                },
                child_causes,
                rule.source()
                    .cloned()
                    .map(|source| {
                        vec![NominalDiagnosticRelated::new(
                            evidence_from_project(source),
                            NominalRelatedMessage::ExpectedArityDeclaration,
                        )]
                    })
                    .unwrap_or_default(),
            );
        }
        self.unavailable_name(context, node, path, child_causes)
    }

    fn unavailable_name(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        path: &TypePath,
        child_causes: Vec<TypePoisonId>,
    ) -> NameResult {
        if matches!(self.input.world(), TypeResolutionWorld::Accepted { .. }) {
            return self.failed_name(
                context,
                node,
                TypeResolutionFailure::Unknown { path: path.clone() },
                child_causes,
                Vec::new(),
            );
        }
        let source = context.evidence(node, true);
        let reason = if context.module.is_some() {
            DetachedNominalReason::ProjectWorldUnavailable
        } else {
            DetachedNominalReason::ModuleUnavailable
        };
        let evidence = DetachedNominalEvidence::new(path.clone(), source.clone(), reason);
        let poison = self.allocate_poison();
        self.record_poison(poison, TypePoisonOrigin::DetachedUnavailable, source, false);
        self.unavailable.push(node.clone());
        NameResult {
            value: NodeValue::error(poison, child_causes),
            outcome: TypeNameResolution::DetachedUnavailable(evidence),
        }
    }

    fn resolve_project_nominal(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        declaration: &ProjectNominalDeclaration,
        arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
        depth: u16,
    ) -> Result<NameResult, TypeResolutionInputError> {
        let actual = u16::try_from(arguments.len()).expect("parser cap");
        let expected = TypeArityExpectation::Exact(
            u16::try_from(declaration.type_parameters().len()).expect("project parameter cap"),
        );
        if !expected.contains(actual) {
            let source = declaration
                .source()
                .generics()
                .unwrap_or_else(|| declaration.source().whole())
                .clone();
            return Ok(self.failed_name(
                context,
                node,
                TypeResolutionFailure::WrongArity {
                    target: TypeArityTarget::Project(declaration.id().clone()),
                    expected,
                    actual,
                },
                child_causes,
                vec![NominalDiagnosticRelated::new(
                    evidence_from_project(source),
                    NominalRelatedMessage::ExpectedArityDeclaration,
                )],
            ));
        }
        let checked = arguments
            .into_iter()
            .map(|(path, value)| self.require_type(context, &path, value))
            .collect::<Vec<_>>();
        match declaration.id().kind() {
            ProjectNominalDeclarationKind::Struct | ProjectNominalDeclarationKind::Enum => {
                let nominal = ProjectNominalType::new(declaration.id().clone(), checked);
                Ok(NameResult {
                    value: NodeValue::typed(
                        TypeKind::ProjectNominal(nominal.clone()),
                        child_causes,
                    ),
                    outcome: TypeNameResolution::Project(nominal),
                })
            }
            ProjectNominalDeclarationKind::TypeAlias => {
                self.expand_alias(context, node, declaration, checked, child_causes, depth)
            }
        }
    }

    fn expand_alias(
        &mut self,
        use_context: &SourceContext<'_>,
        use_node: &TypeRefNodePath,
        declaration: &ProjectNominalDeclaration,
        arguments: Vec<TypeKind>,
        child_causes: Vec<TypePoisonId>,
        depth: u16,
    ) -> Result<NameResult, TypeResolutionInputError> {
        if self.alias_stack.len() >= usize::from(self.input.limits().alias_expansion_depth()) {
            return Ok(self.failed_name(
                use_context,
                use_node,
                TypeResolutionFailure::Limit {
                    kind: NominalResolutionLimitKind::AliasExpansionDepth,
                    observed: self.alias_stack.len() as u64 + 1,
                    maximum: u64::from(self.input.limits().alias_expansion_depth()),
                },
                child_causes,
                Vec::new(),
            ));
        }
        if let Some(position) = self
            .alias_stack
            .iter()
            .position(|candidate| candidate == declaration.id())
        {
            let cycle = canonical_cycle(self.alias_stack[position..].to_vec());
            let related = self.alias_cycle_related(&cycle);
            return Ok(self.failed_name(
                use_context,
                use_node,
                TypeResolutionFailure::CyclicAlias { cycle },
                child_causes,
                related,
            ));
        }

        let ProjectNominalBody::TypeAlias { target } = declaration.body() else {
            unreachable!("type-alias declaration IDs own type-alias bodies")
        };
        let mut bindings = BTreeMap::new();
        let mut substitution = Vec::with_capacity(arguments.len());
        for (parameter, argument) in declaration.type_parameters().iter().zip(&arguments) {
            if let Some(failed) =
                self.charge_name_work(1, use_context, use_node, child_causes.clone())
            {
                return Ok(failed);
            }
            let id = GenericTypeParameterId::new(
                GenericTypeOwnerId::Nominal(declaration.id().clone()),
                parameter.ordinal(),
            );
            substitution.push((id.clone(), argument.clone()));
            bindings.insert(
                parameter.name().clone(),
                AliasBinding {
                    id,
                    value: argument.clone(),
                },
            );
        }
        let target_context = SourceContext {
            authored: target.authored(),
            project: Some(target.spans()),
            module: Some(declaration.id().module()),
            generics: GenericContext::Alias(&bindings),
            alias_target: true,
        };
        let target_node = TypeRefNodePath::root();
        self.alias_stack.push(declaration.id().clone());
        let normalized = self.resolve_node(
            &target_context,
            target.authored().value(),
            &target_node,
            depth + 1,
        )?;
        self.alias_stack.pop();

        let target_source = target_context.evidence(&target_node, true);
        let use_source = use_context.evidence(use_node, true);
        let declaration_source = declaration.source().name().clone();
        let normalized_ty = normalized.recovered_or(TypeKind::Unit);
        self.aliases.push(AliasExpansionFact::new(
            declaration.id().clone(),
            arguments.clone(),
            substitution,
            normalized_ty.clone(),
            use_source.clone(),
            declaration_source.clone(),
            target_source.clone(),
        ));
        let reference = ResolvedAliasReference::new(
            declaration.id().clone(),
            arguments,
            normalized_ty.clone(),
            use_source,
            declaration_source,
            target_source,
        );
        Ok(NameResult {
            value: NodeValue::typed(
                normalized_ty,
                child_causes.into_iter().chain(normalized.causes),
            ),
            outcome: TypeNameResolution::Alias(reference),
        })
    }

    fn resolve_accepted(
        &mut self,
        context: &SourceContext<'_>,
        node: &TypeRefNodePath,
        record: &AcceptedNominalRecord,
        arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
    ) -> NameResult {
        let actual = u16::try_from(arguments.len()).expect("parser cap");
        let expected = TypeArityExpectation::Exact(record.arity());
        if !expected.contains(actual) {
            let related = record
                .source()
                .cloned()
                .map(|source| {
                    vec![NominalDiagnosticRelated::new(
                        evidence_from_project(source),
                        NominalRelatedMessage::ExpectedArityDeclaration,
                    )]
                })
                .unwrap_or_default();
            return self.failed_name(
                context,
                node,
                TypeResolutionFailure::WrongArity {
                    target: TypeArityTarget::Accepted(record.id().clone()),
                    expected,
                    actual,
                },
                child_causes,
                related,
            );
        }
        let checked = arguments
            .into_iter()
            .map(|(path, value)| self.require_type(context, &path, value))
            .collect::<Vec<_>>();
        let nominal = AcceptedNominalType::new(record.id().clone(), checked);
        let ty = record
            .try_instantiate(nominal.arguments().to_vec())
            .expect("accepted catalog records retain valid semantics and checked arity");
        NameResult {
            value: NodeValue::typed(ty, child_causes),
            outcome: TypeNameResolution::Accepted(nominal),
        }
    }

    fn resolve_open(
        &mut self,
        context: &SourceContext<'_>,
        rule: &OpenNominalRule,
        path: &TypePath,
        arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
    ) -> NameResult {
        let checked = arguments
            .into_iter()
            .map(|(path, value)| self.require_type(context, &path, value))
            .collect::<Vec<_>>();
        let fact = ResolvedOpenNominal::new(rule.id().clone(), path.clone(), checked.clone());
        let nominal = OpenNominalType::new(rule.id().clone(), path.clone(), checked);
        NameResult {
            value: NodeValue::typed(TypeKind::OpenNominal(nominal), child_causes),
            outcome: TypeNameResolution::Open(fact),
        }
    }

    fn resolve_external(
        &mut self,
        site: &ExternalResolutionSite<'_, '_>,
        arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
    ) -> Result<NameResult, TypeResolutionInputError> {
        let owner = Self::registered_external_owner(site.symbols, site.environment, site.external)?;
        let bound_accepted = match &owner {
            RegisteredExternalOwner::Environment(owner) => site
                .environment
                .environment_binding(owner.value_binding())
                .and_then(|ty| match ty {
                    TypeKind::AcceptedNominal(nominal) => Some(nominal.clone()),
                    _ => None,
                }),
            RegisteredExternalOwner::Character(_) => None,
        };
        let record = match self.lookup_external_record(
            site,
            &owner,
            bound_accepted.as_ref(),
            &child_causes,
        )? {
            ExternalRecordLookup::Record(record) => record,
            ExternalRecordLookup::BudgetExceeded(failed) => return Ok(*failed),
        };
        let actual = u16::try_from(arguments.len()).expect("parser cap");
        let expected = TypeArityExpectation::Exact(if bound_accepted.is_some() {
            0
        } else {
            record.arity()
        });
        if !expected.contains(actual) {
            return Ok(self.failed_name(
                site.context,
                site.node,
                TypeResolutionFailure::WrongArity {
                    target: TypeArityTarget::Accepted(record.id().clone()),
                    expected,
                    actual,
                },
                child_causes,
                record
                    .source()
                    .cloned()
                    .map(|source| {
                        vec![NominalDiagnosticRelated::new(
                            evidence_from_project(source),
                            NominalRelatedMessage::ExpectedArityDeclaration,
                        )]
                    })
                    .unwrap_or_default(),
            ));
        }
        let checked = arguments
            .into_iter()
            .map(|(path, value)| self.require_type(site.context, &path, value))
            .collect::<Vec<_>>();
        let (ty, resolution) = if let Some(accepted) = bound_accepted {
            let ty = TypeKind::AcceptedNominal(accepted.clone());
            (
                ty.clone(),
                ExternalNominalResolution::Exact {
                    external: site.external.declaration(),
                    ty,
                    accepted: accepted.declaration().clone(),
                },
            )
        } else {
            let accepted = AcceptedNominalType::new(record.id().clone(), checked);
            Self::external_nominal_product(&record, site.external, accepted)
        };
        Ok(NameResult {
            value: NodeValue::typed(ty, child_causes),
            outcome: TypeNameResolution::External(resolution),
        })
    }

    fn lookup_external_record(
        &mut self,
        site: &ExternalResolutionSite<'_, '_>,
        owner: &RegisteredExternalOwner,
        bound: Option<&AcceptedNominalType>,
        child_causes: &[TypePoisonId],
    ) -> Result<ExternalRecordLookup, TypeResolutionInputError> {
        if let Some(bound) = bound {
            if let Some(failed) =
                self.charge_name_work(1, site.context, site.node, child_causes.to_vec())
            {
                return Ok(ExternalRecordLookup::BudgetExceeded(Box::new(failed)));
            }
            let record = site
                .environment
                .accepted_record(bound.declaration())
                .cloned()
                .map_err(
                    |reason| TypeResolutionInputError::RegisteredNominalIntegrity {
                        external: site.external.declaration(),
                        reason: Box::new(reason),
                    },
                )?;
            return Ok(ExternalRecordLookup::Record(record));
        }

        for candidate in site
            .environment
            .typecheck_env()
            .nominal_catalog()
            .exact_records()
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(failed) =
                self.charge_name_work(1, site.context, site.node, child_causes.to_vec())
            {
                return Ok(ExternalRecordLookup::BudgetExceeded(Box::new(failed)));
            }
            if Self::external_record_matches(owner, site.external, &candidate) {
                return Ok(ExternalRecordLookup::Record(candidate));
            }
        }
        Err(TypeResolutionInputError::RegisteredEnvironmentIntegrity {
            external: site.external.declaration(),
            reason: Box::new(ExternalOwnerLookupError::Unknown {
                declaration: site.external.declaration(),
            }),
        })
    }

    fn external_record_matches(
        owner: &RegisteredExternalOwner,
        external: &ExternalSymbol,
        candidate: &AcceptedNominalRecord,
    ) -> bool {
        let owner_matches = match (owner, candidate.id().owner()) {
            (
                RegisteredExternalOwner::Character(expected),
                AcceptedNominalOwnerId::Character(actual),
            ) => expected == actual,
            (
                RegisteredExternalOwner::Environment(expected),
                AcceptedNominalOwnerId::Environment(actual),
            ) => expected.nominal_owner() == actual,
            _ => false,
        };
        owner_matches
            && SymbolPath::try_from(candidate.id().canonical_path().path())
                .is_ok_and(|path| &path == external.canonical_path())
    }

    fn registered_external_owner(
        symbols: &ProjectSymbolTable,
        environment: &AcceptedNominalWorld,
        external: &ExternalSymbol,
    ) -> Result<RegisteredExternalOwner, TypeResolutionInputError> {
        match environment.external_owner(
            symbols,
            external.declaration(),
            RegisteredExternalOwnerKind::Character,
        ) {
            Ok(owner) => Ok(owner.clone()),
            Err(ExternalOwnerLookupError::WrongKind {
                actual: RegisteredExternalOwnerKind::Environment,
                ..
            }) => environment
                .external_owner(
                    symbols,
                    external.declaration(),
                    RegisteredExternalOwnerKind::Environment,
                )
                .cloned()
                .map_err(
                    |reason| TypeResolutionInputError::RegisteredEnvironmentIntegrity {
                        external: external.declaration(),
                        reason: Box::new(reason),
                    },
                ),
            Err(reason) => Err(TypeResolutionInputError::RegisteredEnvironmentIntegrity {
                external: external.declaration(),
                reason: Box::new(reason),
            }),
        }
    }

    fn external_nominal_product(
        record: &AcceptedNominalRecord,
        external: &ExternalSymbol,
        accepted: AcceptedNominalType,
    ) -> (TypeKind, ExternalNominalResolution) {
        let instantiated = record
            .try_instantiate(accepted.arguments().to_vec())
            .expect("accepted catalog records retain valid semantics and checked arity");
        match record.semantics() {
            AcceptedNominalSemantics::Opaque => (
                instantiated,
                ExternalNominalResolution::Accepted {
                    external: external.declaration(),
                    nominal: accepted,
                },
            ),
            AcceptedNominalSemantics::Exact(_) => (
                instantiated.clone(),
                ExternalNominalResolution::Exact {
                    external: external.declaration(),
                    ty: instantiated,
                    accepted: record.id().clone(),
                },
            ),
            AcceptedNominalSemantics::Character(character) => (
                instantiated,
                ExternalNominalResolution::Character {
                    external: external.declaration(),
                    nominal: character.clone(),
                    accepted: record.id().clone(),
                },
            ),
        }
    }

    fn apply_builtin(
        &mut self,
        context: &SourceContext<'_>,
        constructor: BuiltinTypeConstructor,
        mut arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
    ) -> NodeValue {
        if let Some(ty) = Self::scalar_builtin(constructor) {
            return NodeValue::typed(ty, child_causes);
        }
        let target = TypeArityTarget::Builtin(constructor);
        match constructor {
            BuiltinTypeConstructor::Bool
            | BuiltinTypeConstructor::I8
            | BuiltinTypeConstructor::I16
            | BuiltinTypeConstructor::I32
            | BuiltinTypeConstructor::I64
            | BuiltinTypeConstructor::I128
            | BuiltinTypeConstructor::ISize
            | BuiltinTypeConstructor::U8
            | BuiltinTypeConstructor::U16
            | BuiltinTypeConstructor::U32
            | BuiltinTypeConstructor::U64
            | BuiltinTypeConstructor::U128
            | BuiltinTypeConstructor::USize
            | BuiltinTypeConstructor::F32
            | BuiltinTypeConstructor::F64
            | BuiltinTypeConstructor::String
            | BuiltinTypeConstructor::Char
            | BuiltinTypeConstructor::Bytes
            | BuiltinTypeConstructor::Unit
            | BuiltinTypeConstructor::Never => unreachable!("scalar constructors returned above"),
            BuiltinTypeConstructor::Vec
            | BuiltinTypeConstructor::Slice
            | BuiltinTypeConstructor::Seq
            | BuiltinTypeConstructor::Option
            | BuiltinTypeConstructor::Probe
            | BuiltinTypeConstructor::ThreadHandle
            | BuiltinTypeConstructor::Shared => {
                self.apply_unary_builtin(context, constructor, arguments.remove(0), child_causes)
            }
            BuiltinTypeConstructor::Array => {
                self.apply_array_builtin(context, arguments, child_causes, target)
            }
            BuiltinTypeConstructor::OrderedMap
            | BuiltinTypeConstructor::SortedMap
            | BuiltinTypeConstructor::BTreeMap
            | BuiltinTypeConstructor::Result
            | BuiltinTypeConstructor::Need
            | BuiltinTypeConstructor::Stream
            | BuiltinTypeConstructor::Source => {
                self.apply_binary_builtin(context, constructor, arguments, child_causes)
            }
            BuiltinTypeConstructor::Ref
            | BuiltinTypeConstructor::Speaker
            | BuiltinTypeConstructor::SpeakerPreset => self.apply_entity_family_builtin(
                context,
                constructor,
                arguments.remove(0),
                child_causes,
                target,
            ),
        }
    }

    fn scalar_builtin(constructor: BuiltinTypeConstructor) -> Option<TypeKind> {
        Some(match constructor {
            BuiltinTypeConstructor::Bool => TypeKind::Bool,
            BuiltinTypeConstructor::I8 => TypeKind::I8,
            BuiltinTypeConstructor::I16 => TypeKind::I16,
            BuiltinTypeConstructor::I32 => TypeKind::I32,
            BuiltinTypeConstructor::I64 => TypeKind::I64,
            BuiltinTypeConstructor::I128 => TypeKind::I128,
            BuiltinTypeConstructor::ISize => TypeKind::ISize,
            BuiltinTypeConstructor::U8 => TypeKind::U8,
            BuiltinTypeConstructor::U16 => TypeKind::U16,
            BuiltinTypeConstructor::U32 => TypeKind::U32,
            BuiltinTypeConstructor::U64 => TypeKind::U64,
            BuiltinTypeConstructor::U128 => TypeKind::U128,
            BuiltinTypeConstructor::USize => TypeKind::USize,
            BuiltinTypeConstructor::F32 => TypeKind::F32,
            BuiltinTypeConstructor::F64 => TypeKind::F64,
            BuiltinTypeConstructor::String => TypeKind::String,
            BuiltinTypeConstructor::Char => TypeKind::Char,
            BuiltinTypeConstructor::Bytes => TypeKind::Bytes,
            BuiltinTypeConstructor::Unit => TypeKind::Unit,
            BuiltinTypeConstructor::Never => TypeKind::Never,
            _ => return None,
        })
    }

    fn apply_unary_builtin(
        &mut self,
        context: &SourceContext<'_>,
        constructor: BuiltinTypeConstructor,
        (path, value): (TypeRefNodePath, NodeValue),
        child_causes: Vec<TypePoisonId>,
    ) -> NodeValue {
        let inner = self.require_type(context, &path, value);
        let ty = match constructor {
            BuiltinTypeConstructor::Vec => TypeKind::Vec(Box::new(inner)),
            BuiltinTypeConstructor::Slice => TypeKind::Slice(Box::new(inner)),
            BuiltinTypeConstructor::Seq => TypeKind::Seq(Box::new(inner)),
            BuiltinTypeConstructor::Option => TypeKind::Option(Box::new(inner)),
            BuiltinTypeConstructor::Probe => TypeKind::Probe(Box::new(inner)),
            BuiltinTypeConstructor::ThreadHandle => TypeKind::ThreadHandle(Box::new(inner)),
            BuiltinTypeConstructor::Shared => TypeKind::Shared(Box::new(inner)),
            _ => unreachable!("unary builtin dispatch preserves constructor arity"),
        };
        NodeValue::typed(ty, child_causes)
    }

    fn apply_array_builtin(
        &mut self,
        context: &SourceContext<'_>,
        mut arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
        target: TypeArityTarget,
    ) -> NodeValue {
        let (item_path, item) = arguments.remove(0);
        let (length_path, length) = arguments.remove(0);
        let item = self.require_type(context, &item_path, item);
        let length = if let Some(value) = length.const_int {
            ArrayLength::Const(value)
        } else {
            match length.ty {
                Some(TypeKind::GenericParam(parameter)) => ArrayLength::Generic(parameter),
                Some(TypeKind::Error(poison)) => ArrayLength::Error(poison),
                Some(actual) => {
                    let failure = TypeResolutionFailure::WrongArgumentKind {
                        target,
                        argument: 1,
                        expected: TypeArgumentExpectation::ConstInt,
                        actual: TypeArgumentKind::Type(actual),
                    };
                    let poison = self.emit_failure(
                        &failure,
                        context.evidence(&length_path, true),
                        Vec::new(),
                    );
                    self.replace_node_outcome(&length_path, TypeNameResolution::Failed(failure));
                    ArrayLength::Error(poison)
                }
                None => ArrayLength::Inferred,
            }
        };
        let causes = match &length {
            ArrayLength::Error(poison) => {
                canonical_poisons(child_causes.into_iter().chain([*poison]))
            }
            ArrayLength::Const(_) | ArrayLength::Generic(_) | ArrayLength::Inferred => child_causes,
        };
        NodeValue::typed(
            TypeKind::Array {
                item: Box::new(item),
                len: length,
            },
            causes,
        )
    }

    fn apply_binary_builtin(
        &mut self,
        context: &SourceContext<'_>,
        constructor: BuiltinTypeConstructor,
        mut arguments: Vec<(TypeRefNodePath, NodeValue)>,
        child_causes: Vec<TypePoisonId>,
    ) -> NodeValue {
        let (first_path, first) = arguments.remove(0);
        let (second_path, second) = arguments.remove(0);
        let first = self.require_type(context, &first_path, first);
        let second = self.require_type(context, &second_path, second);
        let ty = match constructor {
            BuiltinTypeConstructor::OrderedMap => TypeKind::Map {
                kind: MapKind::Ordered,
                key: Box::new(first),
                value: Box::new(second),
            },
            BuiltinTypeConstructor::SortedMap => TypeKind::Map {
                kind: MapKind::Sorted,
                key: Box::new(first),
                value: Box::new(second),
            },
            BuiltinTypeConstructor::BTreeMap => TypeKind::Map {
                kind: MapKind::BTree,
                key: Box::new(first),
                value: Box::new(second),
            },
            BuiltinTypeConstructor::Result => TypeKind::Result {
                ok: Box::new(first),
                error: Box::new(second),
            },
            BuiltinTypeConstructor::Need => TypeKind::Need {
                ready: Box::new(first),
                error: Box::new(second),
            },
            BuiltinTypeConstructor::Stream => TypeKind::Stream {
                item: Box::new(first),
                error: Box::new(second),
            },
            BuiltinTypeConstructor::Source => TypeKind::Source {
                item: Box::new(first),
                error: Box::new(second),
            },
            _ => unreachable!("binary builtin dispatch preserves constructor arity"),
        };
        NodeValue::typed(ty, child_causes)
    }

    fn apply_entity_family_builtin(
        &mut self,
        context: &SourceContext<'_>,
        constructor: BuiltinTypeConstructor,
        (path, value): (TypeRefNodePath, NodeValue),
        child_causes: Vec<TypePoisonId>,
        target: TypeArityTarget,
    ) -> NodeValue {
        if let Some(family) = value.entity_family.as_ref() {
            let ty = constructor
                .project_entity_family(family.clone())
                .expect("entity-family dispatch is closed by the owner enum");
            return NodeValue::typed(ty, child_causes);
        }
        if let Some(TypeKind::Error(poison)) = value.ty.as_ref() {
            return NodeValue::error(*poison, child_causes);
        }
        if let Some(actual) = value.argument_kind() {
            let failure = TypeResolutionFailure::WrongArgumentKind {
                target,
                argument: 0,
                expected: TypeArgumentExpectation::EntityFamily,
                actual,
            };
            let poison = self.emit_failure(&failure, context.evidence(&path, true), Vec::new());
            self.replace_node_outcome(&path, TypeNameResolution::Failed(failure));
            return NodeValue::error(poison, child_causes);
        }
        let poison = value
            .causes
            .first()
            .copied()
            .expect("a valueless argument is already poisoned");
        NodeValue::error(poison, child_causes)
    }
}
