use std::collections::{BTreeMap, BTreeSet};

use arcweft_id::PublicId;
use arcweft_lang_hir::{
    identity::{ExprId, ItemId},
    item::{
        HirEntryDeclaration, HirEntryId, HirEntryKind, HirEntryMember, HirEntryPathValue,
        HirEntryTarget, HirFlowItem, HirFunctionItem, HirItem, HirItemKind, HirParameterKind,
    },
    leaf::{HirIdRef, HirIdRefValue, HirPathValue},
    module::HirModule,
    pattern::{HirPatternBinding, HirPatternKind},
    project::HirExecutableProjectView,
    source_index::{
        HirEntrySourcePart, HirExprSourceRole, HirItemSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite,
    },
    symbol::{
        CallableDeclarationId, CallableDeclarationKey, CallableDeclarationOwner, CallableSymbol,
        ProjectEntityReferenceLookupError, ProjectSymbolTable, ProjectSymbolTargetId,
        ProjectTypeCandidate, ProjectTypeLookupError, ProjectTypeTarget, ProjectValueLookup,
        ProjectValueLookupError, ResolvedProjectSymbol, nominal::ProjectNominalDeclarationKind,
    },
    type_ref::HirTypeKind,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceSpan;

use crate::{
    callable::{
        CallTargetFact, CallableCandidateId, CallableFamily, CheckedCallableCatalog,
        CheckedCallableFacts,
    },
    final_analysis::{CheckedProjectNominal, FinalSemanticAnalysis},
    types::TypeKind,
};

use super::{
    AgentBudget, BoundNominalKind, BoundNominalTypeKey, CheckedAgentEntry, CheckedAgentPolicy,
    CheckedCallableRole, CheckedEntryBinding, CheckedEntryCatalog, CheckedEntryId,
    CheckedEntryKind, CheckedExistingEntry, CheckedFlowId, CheckedInitialFlowRole,
    CheckedNominalRole, CheckedStatefulEntry, CheckedStatefulEntryKind, digest,
};

mod contract;

use contract::{EntryContractBuilder, ReducerContractNominals};

/// Source-backed failure produced while constructing checked entry bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEntryDiagnostic {
    code: &'static str,
    message: String,
    primary: SourceSpan,
    related: Vec<SourceSpan>,
}

impl CheckedEntryDiagnostic {
    fn new(code: &'static str, message: impl Into<String>, primary: SourceSpan) -> Self {
        Self {
            code,
            message: message.into(),
            primary,
            related: Vec::new(),
        }
    }

    fn with_related(mut self, related: impl IntoIterator<Item = SourceSpan>) -> Self {
        self.related.extend(related);
        self.related.sort_by(|left, right| {
            left.source()
                .id()
                .as_str()
                .cmp(right.source().id().as_str())
                .then_with(|| left.range().start().cmp(&right.range().start()))
                .then_with(|| left.range().end().cmp(&right.range().end()))
        });
        self.related.dedup();
        self
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn primary(&self) -> &SourceSpan {
        &self.primary
    }

    pub fn related(&self) -> &[SourceSpan] {
        &self.related
    }
}

/// Resolves every final-HIR Entry against the exact accepted semantic generation.
///
/// This boundary deliberately has no syntax-tree, detached HIR, type-check
/// sidecar, or registered-only callable overload. Entry roles consume the same
/// immutable HIR project, symbol table, and checked callable authority already
/// accepted by final semantic analysis.
///
/// # Panics
///
/// Panics when the supplied project or symbol table is not the generation
/// owned by `analysis`.
pub fn check_project_entries(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<CheckedEntryCatalog, Vec<CheckedEntryDiagnostic>> {
    analysis
        .validate_generation(project, symbols)
        .expect("Entry checking requires the exact accepted final-HIR generation");
    EntryCheckContext::new(project, symbols, analysis, analysis.checked_callables()).check()
}

struct EntryCheckContext<'a> {
    project: HirExecutableProjectView<'a>,
    symbols: &'a ProjectSymbolTable,
    analysis: &'a FinalSemanticAnalysis,
    callables: &'a CheckedCallableCatalog,
}

struct ResolvedCallable<'a> {
    declaration: CallableDeclarationId,
    module: &'a HirModule,
    item: &'a HirItem,
    function: &'a HirFunctionItem,
    facts: &'a CheckedCallableFacts,
    source: SourceSpan,
}

#[derive(Clone, Copy)]
enum Role {
    State,
    Initializer,
    Event,
    Reducer,
    Controller,
}

impl Role {
    const fn label(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Initializer => "initializer",
            Self::Event => "event",
            Self::Reducer => "reducer",
            Self::Controller => "controller",
        }
    }

    const fn matches(self, member: &HirEntryMember) -> bool {
        matches!(
            (self, member),
            (Self::State, HirEntryMember::StateType(_))
                | (Self::Initializer, HirEntryMember::Initializer(_))
                | (Self::Event, HirEntryMember::EventType(_))
                | (Self::Reducer, HirEntryMember::Reducer(_))
                | (Self::Controller, HirEntryMember::Controller(_))
        )
    }
}

impl<'a> EntryCheckContext<'a> {
    fn new(
        project: HirExecutableProjectView<'a>,
        symbols: &'a ProjectSymbolTable,
        analysis: &'a FinalSemanticAnalysis,
        callables: &'a CheckedCallableCatalog,
    ) -> Self {
        Self {
            project,
            symbols,
            analysis,
            callables,
        }
    }

    fn check(self) -> Result<CheckedEntryCatalog, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Vec::new();
        let mut entries = Vec::new();
        let mut ids = BTreeMap::<PublicId, SourceSpan>::new();

        for item in self.project.items() {
            let HirItemKind::Entry(entry) = item.item().kind() else {
                continue;
            };
            let id_source = entry_source(item.module(), item.id(), HirEntrySourcePart::Id);
            let Some(id) = Self::checked_entry_id(entry, &id_source, &mut diagnostics) else {
                continue;
            };
            if let Some(first) = ids.insert(id.public_id().clone(), id_source.clone()) {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.duplicate_id",
                        format!("entry ID `{id}` is declared more than once"),
                        id_source,
                    )
                    .with_related([first]),
                );
                continue;
            }
            match self.check_entry(item.module_path(), item.module(), item.id(), entry, id) {
                Ok(binding) => entries.push(binding),
                Err(mut failures) => diagnostics.append(&mut failures),
            }
        }

        let selected_controllers = entries
            .iter()
            .filter_map(|entry| entry.agent())
            .map(|agent| agent.controller().declaration().clone())
            .collect::<BTreeSet<_>>();
        self.validate_function_role_attributes(&selected_controllers, &mut diagnostics);
        self.validate_agent_callable_roles(&selected_controllers, &mut diagnostics);

        if diagnostics.is_empty() {
            CheckedEntryCatalog::try_new(entries).map_err(|error| {
                vec![CheckedEntryDiagnostic::new(
                    "sema.entry.duplicate_id",
                    error.to_string(),
                    ids.values()
                        .next()
                        .expect("a duplicate checked ID requires one source Entry")
                        .clone(),
                )]
            })
        } else {
            Err(diagnostics)
        }
    }

    fn checked_entry_id(
        entry: &HirEntryDeclaration,
        source: &SourceSpan,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<CheckedEntryId> {
        let HirEntryId::Authored {
            value,
            canonical_entry_family,
        } = entry.id()
        else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_id",
                "entry declaration is missing its canonical public ID",
                source.clone(),
            ));
            return None;
        };
        let Some(public_id) = absolute_public_id_value(value) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_id",
                "entry ID must be one complete absolute public ID",
                source.clone(),
            ));
            return None;
        };
        if !canonical_entry_family || !public_id.as_str().starts_with("entry.") {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_id_family",
                format!("entry ID `{public_id}` must use the `entry.*` family"),
                source.clone(),
            ));
            return None;
        }
        CheckedEntryId::try_new(public_id.as_str().to_owned())
            .map_err(|_| {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.invalid_id",
                    format!("entry ID `{public_id}` is not canonical"),
                    source.clone(),
                ));
            })
            .ok()
    }

    fn check_entry(
        &self,
        module_path: &CanonicalModulePath,
        module: &'a HirModule,
        owner: ItemId,
        entry: &'a HirEntryDeclaration,
        id: CheckedEntryId,
    ) -> Result<CheckedEntryBinding, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Self::validate_entry_members(module, owner, entry);
        let result = match entry.kind() {
            HirEntryKind::Game => self.check_stateful(
                module_path,
                module,
                owner,
                entry,
                id,
                CheckedStatefulEntryKind::Game,
            ),
            HirEntryKind::Editor => self.check_stateful(
                module_path,
                module,
                owner,
                entry,
                id,
                CheckedStatefulEntryKind::Editor,
            ),
            HirEntryKind::Test => self.check_stateful(
                module_path,
                module,
                owner,
                entry,
                id,
                CheckedStatefulEntryKind::Test,
            ),
            HirEntryKind::Agent => self.check_agent(module_path, module, owner, entry, id),
            HirEntryKind::Cli
            | HirEntryKind::Server
            | HirEntryKind::Activity
            | HirEntryKind::Bench
            | HirEntryKind::Custom(_) => Ok(self.check_existing(owner, id, entry.kind())),
            HirEntryKind::Recovered(_) => Err(vec![CheckedEntryDiagnostic::new(
                "sema.entry.invalid_kind",
                "recovered Entry kind cannot enter an executable project",
                entry_source(module, owner, HirEntrySourcePart::Whole),
            )]),
        };
        match result {
            Ok(binding) if diagnostics.is_empty() => Ok(binding),
            Ok(_) => Err(diagnostics),
            Err(mut failures) => {
                diagnostics.append(&mut failures);
                Err(diagnostics)
            }
        }
    }

    fn validate_entry_members(
        module: &HirModule,
        owner: ItemId,
        entry: &HirEntryDeclaration,
    ) -> Vec<CheckedEntryDiagnostic> {
        let mut diagnostics = Vec::new();
        for (ordinal, member) in entry.members().iter().enumerate() {
            let source = entry_member_source(module, owner, ordinal);
            let role = match member {
                HirEntryMember::StateType(_) => Some(Role::State),
                HirEntryMember::Initializer(_) => Some(Role::Initializer),
                HirEntryMember::EventType(_) => Some(Role::Event),
                HirEntryMember::Reducer(_) => Some(Role::Reducer),
                HirEntryMember::Controller(_) => Some(Role::Controller),
                HirEntryMember::Goto(_)
                | HirEntryMember::Route(_)
                | HirEntryMember::Option(_)
                | HirEntryMember::Error => None,
            };
            if let Some(role) = role
                && !entry_kind_allows_role(entry.kind(), role)
            {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.incompatible_role",
                    format!(
                        "entry kind `{}` cannot bind the `{}` role",
                        entry_kind_label(entry.kind()),
                        role.label()
                    ),
                    source.clone(),
                ));
            }
            match member {
                HirEntryMember::Goto(_) if matches!(entry.kind(), HirEntryKind::Agent) => {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.incompatible_goto",
                        "Agent entry cannot declare `goto`",
                        source,
                    ));
                }
                HirEntryMember::Route(_)
                    if matches!(
                        entry.kind(),
                        HirEntryKind::Game
                            | HirEntryKind::Editor
                            | HirEntryKind::Test
                            | HirEntryKind::Agent
                    ) =>
                {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.incompatible_route",
                        format!(
                            "entry kind `{}` cannot declare adapter routes",
                            entry_kind_label(entry.kind())
                        ),
                        source,
                    ));
                }
                _ => {}
            }
        }
        diagnostics
    }

    #[allow(
        clippy::too_many_lines,
        reason = "stateful Entry checking atomically resolves and seals all required roles"
    )]
    fn check_stateful(
        &self,
        module_path: &CanonicalModulePath,
        module: &'a HirModule,
        owner: ItemId,
        entry: &'a HirEntryDeclaration,
        id: CheckedEntryId,
        kind: CheckedStatefulEntryKind,
    ) -> Result<CheckedEntryBinding, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Vec::new();
        let state = Self::unique_member(module, owner, entry, Role::State, &mut diagnostics)
            .and_then(|(ordinal, member)| {
                let HirEntryMember::StateType(binding) = member else {
                    unreachable!("role inventory returns its typed member family")
                };
                self.resolve_nominal(
                    module_path,
                    module,
                    binding.ty(),
                    entry_member_source(module, owner, ordinal),
                    "state",
                    BoundNominalKind::Struct,
                    &mut diagnostics,
                )
            });
        let initializer =
            Self::unique_member(module, owner, entry, Role::Initializer, &mut diagnostics)
                .and_then(|(ordinal, member)| {
                    let HirEntryMember::Initializer(binding) = member else {
                        unreachable!("role inventory returns its typed member family")
                    };
                    self.resolve_callable(
                        module_path,
                        binding.value(),
                        entry_member_source(module, owner, ordinal),
                        "initializer",
                        &mut diagnostics,
                    )
                });
        let event = Self::unique_member(module, owner, entry, Role::Event, &mut diagnostics)
            .and_then(|(ordinal, member)| {
                let HirEntryMember::EventType(binding) = member else {
                    unreachable!("role inventory returns its typed member family")
                };
                self.resolve_nominal(
                    module_path,
                    module,
                    binding.ty(),
                    entry_member_source(module, owner, ordinal),
                    "event",
                    BoundNominalKind::Enum,
                    &mut diagnostics,
                )
            });
        let reducer = Self::unique_member(module, owner, entry, Role::Reducer, &mut diagnostics)
            .and_then(|(ordinal, member)| {
                let HirEntryMember::Reducer(binding) = member else {
                    unreachable!("role inventory returns its typed member family")
                };
                self.resolve_callable(
                    module_path,
                    binding.value(),
                    entry_member_source(module, owner, ordinal),
                    "reducer",
                    &mut diagnostics,
                )
            });
        let (Some(state), Some(initializer), Some(event), Some(reducer)) =
            (state, initializer, event, reducer)
        else {
            return Err(diagnostics);
        };

        let contracts = EntryContractBuilder::new(self.analysis, self.project.package());
        let initializer_contract =
            match contracts.initializer(initializer.function, initializer.facts, state.key()) {
                Ok(contract) => contract,
                Err(message) => {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.invalid_initializer_contract",
                        message,
                        initializer.source,
                    ));
                    return Err(diagnostics);
                }
            };
        let reducer_contract = match contracts.reducer(
            reducer.function,
            reducer.facts,
            ReducerContractNominals {
                state: state.key(),
                event: event.key(),
            },
        ) {
            Ok(contract) => contract,
            Err(message) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.invalid_reducer_contract",
                    message,
                    reducer.source,
                ));
                return Err(diagnostics);
            }
        };
        let initializer = CheckedCallableRole {
            declaration: initializer.declaration,
            contract_digest: digest::callable_contract(&initializer_contract),
            source: initializer.source,
        };
        let reducer = CheckedCallableRole {
            declaration: reducer.declaration,
            contract_digest: digest::callable_contract(&reducer_contract),
            source: reducer.source,
        };
        let Some(initial_flow) =
            self.resolve_initial_flow(module, owner, entry, state.key(), &mut diagnostics)
        else {
            return Err(diagnostics);
        };
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        let binding_digest = digest::stateful_binding(digest::StatefulBindingInput {
            package: self.project.package(),
            id: &id,
            kind,
            state: (state.key(), state.schema_digest()),
            initializer: (initializer.declaration(), initializer.contract_digest()),
            event: (event.key(), event.schema_digest()),
            reducer: (reducer.declaration(), reducer.contract_digest()),
            initial_flow: (initial_flow.id(), initial_flow.contract_digest()),
        });
        Ok(CheckedEntryBinding::Stateful(Box::new(
            CheckedStatefulEntry {
                source_item: owner,
                id,
                kind,
                state,
                initializer,
                event,
                reducer,
                initial_flow,
                binding_digest,
            },
        )))
    }

    fn check_agent(
        &self,
        module_path: &CanonicalModulePath,
        module: &'a HirModule,
        owner: ItemId,
        entry: &'a HirEntryDeclaration,
        id: CheckedEntryId,
    ) -> Result<CheckedEntryBinding, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Vec::new();
        let controller =
            Self::unique_member(module, owner, entry, Role::Controller, &mut diagnostics).and_then(
                |(ordinal, member)| {
                    let HirEntryMember::Controller(binding) = member else {
                        unreachable!("role inventory returns its typed member family")
                    };
                    self.resolve_callable(
                        module_path,
                        binding.value(),
                        entry_member_source(module, owner, ordinal),
                        "controller",
                        &mut diagnostics,
                    )
                },
            );
        let Some(controller) = controller else {
            return Err(diagnostics);
        };
        let contracts = EntryContractBuilder::new(self.analysis, self.project.package());
        let (contract, allowed_effects, inferred_effects) =
            match contracts.agent_controller(controller.function, controller.facts) {
                Ok(contract) => contract,
                Err(message) => {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.invalid_agent_contract",
                        message,
                        controller.source,
                    ));
                    return Err(diagnostics);
                }
            };
        let budget = match AgentBudget::from_hir_attributes(
            controller.module,
            controller.item.prefix().attributes(),
        ) {
            Ok(budget) => budget,
            Err(error) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.invalid_agent_budget",
                    error.to_string(),
                    controller.source,
                ));
                return Err(diagnostics);
            }
        };
        let policy = CheckedAgentPolicy::new(allowed_effects, inferred_effects);
        let controller = CheckedCallableRole {
            declaration: controller.declaration,
            contract_digest: digest::callable_contract(&contract),
            source: controller.source,
        };
        let policy_digest = digest::agent_policy(&policy, budget);
        let binding_digest = digest::agent_binding(
            self.project.package(),
            &id,
            (controller.declaration(), controller.contract_digest()),
            &policy_digest,
        );
        Ok(CheckedEntryBinding::Agent(Box::new(CheckedAgentEntry {
            source_item: owner,
            id,
            controller,
            policy,
            budget,
            policy_digest,
            binding_digest,
        })))
    }

    fn check_existing(
        &self,
        source_item: ItemId,
        id: CheckedEntryId,
        kind: &HirEntryKind,
    ) -> CheckedEntryBinding {
        let checked_kind = match kind {
            HirEntryKind::Cli => CheckedEntryKind::Cli,
            HirEntryKind::Server => CheckedEntryKind::Server,
            HirEntryKind::Activity => CheckedEntryKind::Activity,
            HirEntryKind::Bench => CheckedEntryKind::Bench,
            HirEntryKind::Custom(value) => CheckedEntryKind::Custom(value.as_str().to_owned()),
            HirEntryKind::Game
            | HirEntryKind::Editor
            | HirEntryKind::Test
            | HirEntryKind::Agent
            | HirEntryKind::Recovered(_) => {
                unreachable!("special Entry kinds use their typed checking paths")
            }
        };
        let binding_digest = digest::existing_binding(self.project.package(), &id, &checked_kind);
        CheckedEntryBinding::Existing(CheckedExistingEntry {
            source_item,
            id,
            kind: checked_kind,
            binding_digest,
        })
    }

    fn unique_member(
        module: &HirModule,
        owner: ItemId,
        entry: &'a HirEntryDeclaration,
        role: Role,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<(usize, &'a HirEntryMember)> {
        let matches = entry
            .members()
            .iter()
            .enumerate()
            .filter(|(_, member)| role.matches(member))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [(ordinal, member)] => Some((*ordinal, *member)),
            [] => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.missing_role",
                    format!("entry is missing required `{}` role", role.label()),
                    entry_source(module, owner, HirEntrySourcePart::Whole),
                ));
                None
            }
            [(first_ordinal, _), (duplicate_ordinal, _), ..] => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.duplicate_role",
                        format!("entry declares `{}` more than once", role.label()),
                        entry_member_source(module, owner, *duplicate_ordinal),
                    )
                    .with_related([entry_member_source(
                        module,
                        owner,
                        *first_ordinal,
                    )]),
                );
                None
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "nominal role resolution keeps the exact HIR source and role contract together"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "Entry nominal admission keeps source lookup, schema expansion, family checks, and diagnostics in one typed transaction"
    )]
    fn resolve_nominal(
        &self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        ty: arcweft_lang_hir::identity::TypeId,
        source: SourceSpan,
        role: &str,
        expected_kind: BoundNominalKind,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<CheckedNominalRole> {
        let Ok(hir_type) = module.resolve_type(ty) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.missing_nominal_resolution",
                format!("{role} role type is absent from the accepted HIR generation"),
                source,
            ));
            return None;
        };
        let HirTypeKind::Path(path) = hir_type.kind() else {
            let code = if matches!(hir_type.kind(), HirTypeKind::Generic(_)) {
                "sema.entry.generic_nominal_root"
            } else {
                "sema.entry.role_not_direct_nominal"
            };
            diagnostics.push(CheckedEntryDiagnostic::new(
                code,
                format!("{role} role must name one direct non-generic project nominal"),
                source,
            ));
            return None;
        };
        let declaration =
            match self
                .symbols
                .resolve_hir_type_target(module_path, path, source.clone())
            {
                Ok(ProjectTypeTarget::Nominal(declaration)) => declaration,
                Ok(ProjectTypeTarget::External(_)) => {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.role_not_direct_nominal",
                        format!("{role} role must name a project struct or enum"),
                        source,
                    ));
                    return None;
                }
                Err(error) => {
                    let (code, related) = type_lookup_diagnostic(&error);
                    diagnostics.push(
                        CheckedEntryDiagnostic::new(
                            code,
                            format!("cannot resolve {role} role type in the accepted project"),
                            source,
                        )
                        .with_related(related),
                    );
                    return None;
                }
            };
        let kind = match declaration.id().kind() {
            ProjectNominalDeclarationKind::Struct => BoundNominalKind::Struct,
            ProjectNominalDeclarationKind::Enum => BoundNominalKind::Enum,
            ProjectNominalDeclarationKind::TypeAlias => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.alias_nominal_root",
                        format!(
                            "{role} role is a type alias; bind one direct project struct or enum"
                        ),
                        source,
                    )
                    .with_related([declaration.source().whole().clone()]),
                );
                return None;
            }
        };
        if kind != expected_kind {
            diagnostics.push(
                CheckedEntryDiagnostic::new(
                    "sema.entry.role_not_direct_nominal",
                    format!(
                        "{role} role requires a direct project {}, found {}",
                        nominal_kind_label(expected_kind),
                        nominal_kind_label(kind)
                    ),
                    source,
                )
                .with_related([declaration.source().whole().clone()]),
            );
            return None;
        }
        if !declaration.type_parameters().is_empty() {
            diagnostics.push(
                CheckedEntryDiagnostic::new(
                    "sema.entry.generic_nominal_root",
                    format!("{role} role must be a non-generic project nominal"),
                    source,
                )
                .with_related([declaration.source().whole().clone()]),
            );
            return None;
        }
        let Some(TypeKind::ProjectNominal(checked)) = self.analysis.ty(ty) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.missing_nominal_resolution",
                format!("{role} role has no accepted project-nominal type fact"),
                source,
            ));
            return None;
        };
        if checked.declaration() != declaration.id() || !checked.arguments().is_empty() {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.role_not_direct_nominal",
                format!("{role} role did not retain the direct nominal declaration identity"),
                source,
            ));
            return None;
        }
        let checked_nominal = CheckedProjectNominal::new(
            checked.declaration().clone(),
            declaration.owner(),
            TypeKind::ProjectNominal(checked.clone()).semantic_identity_digest(),
            checked.arguments().to_vec(),
        );
        match self
            .analysis
            .project_nominal_schema(self.symbols, &checked_nominal)
        {
            Ok(schema) => Some(CheckedNominalRole {
                key: BoundNominalTypeKey::new(
                    self.project.package().clone(),
                    declaration.id().module().clone(),
                    declaration.id().name().as_str(),
                    kind,
                ),
                schema_digest: digest::nominal_schema(&schema),
                schema,
                source,
            }),
            Err(error) => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.invalid_nominal_schema",
                        format!("{role} role type has no canonical data shape: {error}"),
                        source,
                    )
                    .with_related([declaration.source().whole().clone()]),
                );
                None
            }
        }
    }

    fn resolve_callable(
        &self,
        module_path: &CanonicalModulePath,
        value: &HirEntryPathValue,
        source: SourceSpan,
        role: &str,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<ResolvedCallable<'a>> {
        let HirEntryPathValue::Authored(HirPathValue::Resolved(path)) = value else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_callable_path",
                format!("{role} role requires one complete typed callable path"),
                source,
            ));
            return None;
        };
        let symbol = match self
            .symbols
            .resolve_hir_value_target(module_path, path, source.clone())
        {
            Ok(ProjectValueLookup::Present(symbol)) => symbol,
            Ok(ProjectValueLookup::Absent) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.unresolved_callable",
                    format!("cannot resolve {role} role in the callable value namespace"),
                    source,
                ));
                return None;
            }
            Err(error) => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.unresolved_callable",
                        format!("cannot resolve {role} role: {error}"),
                        source,
                    )
                    .with_related(self.value_lookup_sources(&error)),
                );
                return None;
            }
        };
        if symbol.owner() != CallableDeclarationOwner::Function {
            diagnostics.push(
                CheckedEntryDiagnostic::new(
                    "sema.entry.callable_not_function",
                    format!("{role} role must name an ordinary function"),
                    source,
                )
                .with_related([symbol.declaration_span().clone()]),
            );
            return None;
        }
        let CallableDeclarationKey::Existing(declaration) = symbol.declaration() else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.callable_not_function",
                format!("{role} role must use an ordinary function declaration identity"),
                source,
            ));
            return None;
        };
        let Some((module, item, function)) = self.function_for_symbol(symbol) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.callable_not_function",
                format!("{role} role function is absent from its accepted HIR generation"),
                source,
            ));
            return None;
        };
        let candidate = CallableCandidateId::Project(symbol.declaration().clone());
        let facts = self
            .callables
            .checked_for_candidate(&candidate)
            .and_then(|id| self.callables.callable(id));
        let Ok(facts) = facts else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.callable_not_registered",
                format!("{role} role is absent from the accepted checked callable catalog"),
                source,
            ));
            return None;
        };
        Some(ResolvedCallable {
            declaration: declaration.clone(),
            module,
            item,
            function,
            facts,
            source,
        })
    }

    fn function_for_symbol(
        &self,
        symbol: &CallableSymbol,
    ) -> Option<(&'a HirModule, &'a HirItem, &'a HirFunctionItem)> {
        let module = self
            .project
            .modules()
            .map(|(_, module)| module.as_ref())
            .find(|module| module.snapshot_id() == symbol.source_snapshot())?;
        let item = module.resolve_item(symbol.source_item()).ok()?;
        let HirItemKind::Function(function) = item.kind() else {
            return None;
        };
        Some((module, item, function))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "initial-flow resolution keeps project lookup, checked callable identity, suspension role, and diagnostics adjacent"
    )]
    fn resolve_initial_flow(
        &self,
        module: &HirModule,
        owner: ItemId,
        entry: &HirEntryDeclaration,
        state: &BoundNominalTypeKey,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<CheckedInitialFlowRole> {
        let gotos = entry
            .members()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, member)| match member {
                HirEntryMember::Goto(target) => Some((ordinal, target)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(ordinal, target)] = gotos.as_slice() else {
            diagnostics.push(
                CheckedEntryDiagnostic::new(
                    "sema.entry.goto_cardinality",
                    "stateful entry must contain exactly one initial `goto` target",
                    entry_source(module, owner, HirEntrySourcePart::Whole),
                )
                .with_related(
                    gotos
                        .iter()
                        .map(|(ordinal, _)| entry_member_source(module, owner, *ordinal)),
                ),
            );
            return None;
        };
        let source = entry_member_source(module, owner, *ordinal);
        let HirEntryTarget::Authored(value) = target.target() else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_flow_id",
                "initial target must be one complete absolute Flow ID",
                source,
            ));
            return None;
        };
        let Some(reference) = value.as_resolved() else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_flow_id",
                "initial target must be one complete absolute Flow ID",
                source,
            ));
            return None;
        };
        let Some(public_id) = absolute_public_id(reference) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_flow_id",
                "initial target must be one complete absolute Flow ID",
                source,
            ));
            return None;
        };
        if !public_id.as_str().starts_with("flow.") {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_flow_family",
                format!("initial target `{public_id}` must use the `flow.*` family"),
                source,
            ));
            return None;
        }
        let symbol = match self.symbols.resolve_entity_reference(
            module.key().path(),
            reference,
            source.clone(),
        ) {
            Ok(ResolvedProjectSymbol::StructuralCallable(symbol))
                if symbol.owner() == CallableDeclarationOwner::Flow =>
            {
                symbol
            }
            Ok(other) => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.invalid_flow_family",
                        format!("initial target `{public_id}` does not denote a Flow"),
                        source,
                    )
                    .with_related(resolved_symbol_source(&other)),
                );
                return None;
            }
            Err(error) => {
                let code = match &error {
                    ProjectEntityReferenceLookupError::Ambiguous { .. } => {
                        "sema.entry.ambiguous_flow"
                    }
                    _ => "sema.entry.unknown_flow",
                };
                diagnostics.push(
                    CheckedEntryDiagnostic::new(code, error.to_string(), source)
                        .with_related(entity_lookup_sources(self.symbols, &error)),
                );
                return None;
            }
        };
        let CallableDeclarationKey::Flow(declaration) = symbol.declaration() else {
            unreachable!("the structural Flow target owns a Flow declaration key")
        };
        let id = CheckedFlowId::from_declaration(declaration);
        let Some((flow_module, flow_owner, flow)) = self.flow_for_symbol(symbol) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.unknown_flow",
                format!("initial Flow `{public_id}` is absent from its accepted HIR snapshot"),
                source,
            ));
            return None;
        };
        let contracts = EntryContractBuilder::new(self.analysis, self.project.package());
        let contract = match contracts.flow(flow_owner, flow, state) {
            Ok(contract) => contract,
            Err(message) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.invalid_initial_flow_contract",
                    message,
                    source,
                ));
                return None;
            }
        };
        let [parameter] = flow.parameters() else {
            unreachable!("accepted initial Flow contract has exactly one parameter")
        };
        debug_assert_eq!(parameter.kind(), HirParameterKind::Fixed);
        let pattern = flow_module
            .resolve_pattern(parameter.pattern())
            .expect("accepted initial Flow parameter pattern remains live");
        let HirPatternKind::Binding(HirPatternBinding::Bound { name, .. }) = pattern.kind() else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_initial_flow_contract",
                "initial Flow State parameter must be one direct immutable binding",
                source,
            ));
            return None;
        };
        Some(CheckedInitialFlowRole {
            source_item: flow_owner,
            contract_digest: digest::flow_contract(&id, &contract),
            state_parameter_name: name.as_str().to_owned(),
            id,
            source,
        })
    }

    fn flow_for_symbol(
        &self,
        symbol: &CallableSymbol,
    ) -> Option<(&'a HirModule, ItemId, &'a HirFlowItem)> {
        let module = self
            .project
            .modules()
            .map(|(_, module)| module.as_ref())
            .find(|module| module.snapshot_id() == symbol.source_snapshot())?;
        let owner = symbol.source_item();
        let item = module.resolve_item(owner).ok()?;
        let HirItemKind::Flow(flow) = item.kind() else {
            return None;
        };
        Some((module, owner, flow))
    }

    fn validate_function_role_attributes(
        &self,
        selected: &BTreeSet<CallableDeclarationId>,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) {
        for symbol in self.symbols.callable_symbols() {
            if symbol.owner() != CallableDeclarationOwner::Function {
                continue;
            }
            let CallableDeclarationKey::Existing(declaration) = symbol.declaration() else {
                continue;
            };
            let Some((_, item, _)) = self.function_for_symbol(symbol) else {
                continue;
            };
            for attribute in item.prefix().attributes() {
                let Some(name) = simple_attribute_name(attribute.path()) else {
                    continue;
                };
                if matches!(name, "agent" | "launch" | "bind") {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.forbidden_role_attribute",
                        format!(
                            "`#[{name}]` cannot assign a function Entry role; bind the ordinary function from an `entry` declaration"
                        ),
                        symbol.declaration_span().clone(),
                    ));
                }
                if name == "budget" && !selected.contains(declaration) {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.unbound_agent_budget",
                        "`#[budget(...)]` is only valid on an ordinary function selected by an Agent entry",
                        symbol.declaration_span().clone(),
                    ));
                }
            }
        }
    }

    fn validate_agent_callable_roles(
        &self,
        selected: &BTreeSet<CallableDeclarationId>,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) {
        for (expression, facts) in self.analysis.calls() {
            let CallTargetFact::Selected {
                selected: callable, ..
            } = facts.target()
            else {
                continue;
            };
            let selected_owner = facts.enclosing_callable().and_then(|owner| match owner {
                CallableDeclarationKey::Existing(declaration) => Some(declaration),
                CallableDeclarationKey::TraitRequirement(_)
                | CallableDeclarationKey::ImplMethod(_)
                | CallableDeclarationKey::Flow(_) => None,
            });
            if callable.family() != CallableFamily::Agent
                || selected_owner.is_some_and(|owner| selected.contains(owner))
            {
                continue;
            }
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.unbound_agent_intrinsic",
                "Agent call is only valid inside an ordinary function selected as an Agent entry controller",
                expression_source(self.project, expression),
            ));
        }
    }

    fn value_lookup_sources(&self, error: &ProjectValueLookupError) -> Vec<SourceSpan> {
        let candidates = match error {
            ProjectValueLookupError::Ambiguous { candidates, .. }
            | ProjectValueLookupError::Inaccessible { candidates, .. } => candidates.as_ref(),
            ProjectValueLookupError::Poisoned { target, .. } => std::slice::from_ref(target),
            ProjectValueLookupError::InvalidPath { .. }
            | ProjectValueLookupError::InvalidHirPath { .. } => &[],
        };
        candidates
            .iter()
            .filter_map(|candidate| project_target_source(self.symbols, candidate))
            .collect()
    }
}

fn entry_kind_allows_role(kind: &HirEntryKind, role: Role) -> bool {
    match kind {
        HirEntryKind::Game | HirEntryKind::Editor | HirEntryKind::Test => {
            !matches!(role, Role::Controller)
        }
        HirEntryKind::Agent => matches!(role, Role::Controller),
        HirEntryKind::Cli
        | HirEntryKind::Server
        | HirEntryKind::Activity
        | HirEntryKind::Bench
        | HirEntryKind::Custom(_)
        | HirEntryKind::Recovered(_) => false,
    }
}

fn entry_kind_label(kind: &HirEntryKind) -> &str {
    match kind {
        HirEntryKind::Game => "game",
        HirEntryKind::Editor => "editor",
        HirEntryKind::Cli => "cli",
        HirEntryKind::Server => "server",
        HirEntryKind::Activity => "activity",
        HirEntryKind::Test => "test",
        HirEntryKind::Bench => "bench",
        HirEntryKind::Agent => "agent",
        HirEntryKind::Custom(value) => value.as_str(),
        HirEntryKind::Recovered(_) => "<recovered>",
    }
}

const fn nominal_kind_label(kind: BoundNominalKind) -> &'static str {
    match kind {
        BoundNominalKind::Struct => "struct",
        BoundNominalKind::Enum => "enum",
    }
}

fn absolute_public_id(reference: &HirIdRef) -> Option<PublicId> {
    let HirIdRef::Absolute(reference) = reference else {
        return None;
    };
    PublicId::try_new(reference.as_str().to_owned()).ok()
}

fn absolute_public_id_value(value: &HirIdRefValue) -> Option<PublicId> {
    value.as_resolved().and_then(absolute_public_id)
}

fn simple_attribute_name(path: &arcweft_lang_hir::leaf::HirPath) -> Option<&str> {
    let [arcweft_lang_hir::leaf::HirPathSegment::Identifier(name)] = path.segments() else {
        return None;
    };
    Some(name.as_str())
}

fn type_lookup_diagnostic(error: &ProjectTypeLookupError) -> (&'static str, Vec<SourceSpan>) {
    match error {
        ProjectTypeLookupError::Ambiguous { candidates, .. } => (
            "sema.entry.ambiguous_nominal",
            type_candidate_sources(candidates),
        ),
        ProjectTypeLookupError::Inaccessible { candidates, .. } => (
            "sema.entry.inaccessible_nominal",
            type_candidate_sources(candidates),
        ),
        ProjectTypeLookupError::Unknown { .. } => ("sema.entry.unresolved_nominal", Vec::new()),
        ProjectTypeLookupError::WrongKind { actual, .. } => (
            "sema.entry.role_not_direct_nominal",
            type_candidate_sources(std::slice::from_ref(actual)),
        ),
        ProjectTypeLookupError::InvalidPath { .. } => {
            ("sema.entry.role_not_direct_nominal", Vec::new())
        }
    }
}

fn type_candidate_sources(candidates: &[ProjectTypeCandidate]) -> Vec<SourceSpan> {
    candidates
        .iter()
        .flat_map(|candidate| {
            candidate
                .declaration()
                .into_iter()
                .chain(candidate.binding_sites())
        })
        .cloned()
        .collect()
}

fn project_target_source(
    symbols: &ProjectSymbolTable,
    target: &ProjectSymbolTargetId,
) -> Option<SourceSpan> {
    match target {
        ProjectSymbolTargetId::Callable(id) | ProjectSymbolTargetId::StructuralCallable(id) => {
            symbols
                .callable(id)
                .map(|symbol| symbol.declaration_span().clone())
        }
        ProjectSymbolTargetId::External(id) => symbols
            .external(*id)
            .map(|symbol| symbol.declaration_span().clone()),
        ProjectSymbolTargetId::Nominal(id) => symbols
            .nominal(id)
            .map(|declaration| declaration.source().whole().clone()),
        ProjectSymbolTargetId::Retained(id) => symbols
            .retained(id)
            .map(|symbol| symbol.declaration_span().clone()),
        ProjectSymbolTargetId::Module(_) => None,
    }
}

fn resolved_symbol_source(symbol: &ResolvedProjectSymbol<'_>) -> Option<SourceSpan> {
    match symbol {
        ResolvedProjectSymbol::Callable(symbol)
        | ResolvedProjectSymbol::StructuralCallable(symbol) => {
            Some(symbol.declaration_span().clone())
        }
        ResolvedProjectSymbol::External(symbol) => Some(symbol.declaration_span().clone()),
        ResolvedProjectSymbol::Nominal(symbol) => Some(symbol.source().whole().clone()),
        ResolvedProjectSymbol::Retained(symbol) => Some(symbol.declaration_span().clone()),
        ResolvedProjectSymbol::Module(_) => None,
    }
}

fn entity_lookup_sources(
    symbols: &ProjectSymbolTable,
    error: &ProjectEntityReferenceLookupError,
) -> Vec<SourceSpan> {
    match error {
        ProjectEntityReferenceLookupError::Ambiguous { candidates, .. }
        | ProjectEntityReferenceLookupError::Inaccessible { candidates, .. } => candidates
            .iter()
            .filter_map(|candidate| project_target_source(symbols, candidate))
            .collect(),
        ProjectEntityReferenceLookupError::Poisoned { declaration, .. } => {
            vec![declaration.clone()]
        }
        ProjectEntityReferenceLookupError::Unknown { .. }
        | ProjectEntityReferenceLookupError::RelativeRequiresFamily { .. }
        | ProjectEntityReferenceLookupError::UnsupportedParentDepth { .. }
        | ProjectEntityReferenceLookupError::InvalidIdentity { .. }
        | ProjectEntityReferenceLookupError::InvalidReferencePath { .. }
        | ProjectEntityReferenceLookupError::InvalidModulePath { .. }
        | ProjectEntityReferenceLookupError::CatalogOwned { .. } => Vec::new(),
    }
}

fn entry_source(module: &HirModule, owner: ItemId, part: HirEntrySourcePart) -> SourceSpan {
    item_source(module, owner, HirItemSourceRole::Entry(part))
}

fn entry_member_source(module: &HirModule, owner: ItemId, ordinal: usize) -> SourceSpan {
    let member = u32::try_from(ordinal).expect("accepted Entry member ordinal fits u32");
    entry_source(module, owner, HirEntrySourcePart::MemberValue { member })
}

fn expression_source(project: HirExecutableProjectView<'_>, expression: ExprId) -> SourceSpan {
    let module = project
        .modules()
        .map(|(_, module)| module.as_ref())
        .find(|module| module.module_id() == expression.module())
        .expect("checked expression belongs to the accepted project generation");
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner: expression,
                role: HirExprSourceRole::Whole,
            },
        )
        .expect("checked expression owns its validated source role");
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => span.clone(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            unreachable!("executable checked Call expressions retain authored source")
        }
    }
}

fn item_source(module: &HirModule, owner: ItemId, role: HirItemSourceRole) -> SourceSpan {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item { owner, role },
        )
        .expect("accepted final-HIR item owns its validated source role");
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => span.clone(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            unreachable!("executable Entry/Flow roles retain authored source")
        }
    }
}
