use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use arcweft_lang_hir::{
    entry::{HirEntryDecl, HirEntryItem},
    model::{HirFlow, HirFunction, HirModule, HirTopLevelDecl},
    project::HirProject,
    symbol::{
        CallableDeclarationId, ProjectSymbolResolutionError, ProjectSymbolTable,
        ProjectSymbolTargetId,
    },
};
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        ids::EntityRef,
        items::{EntryKind, EntryRoleKind},
        module_path::CanonicalModulePath,
        symbol_path::{ProjectSymbolPath, SymbolPath},
    },
    expr::DottedPath,
    types::TypeRef,
};
use arcweft_source::SourceSpan;

use crate::{
    callable::{CallTargetFact, CallableFamily, CallableRecord, RegisteredCallableCatalog},
    check::TypeCheckReport,
};

use super::{
    AgentBudget, BoundNominalKind, BoundNominalTypeKey, CheckedAgentEntry, CheckedAgentPolicy,
    CheckedCallableRole, CheckedEntryBinding, CheckedEntryCatalog, CheckedEntryId,
    CheckedEntryKind, CheckedExistingEntry, CheckedFlowId, CheckedInitialFlowRole,
    CheckedNominalRole, CheckedStatefulEntry, CheckedStatefulEntryKind, digest,
};

mod contract;
mod nominal;
mod roles;

use contract::{EntryContractBuilder, ReducerContractNominals};
use nominal::{NominalResolutionError, NominalSchemaResolver};
use roles::{Role, unique_item};

/// Source-backed failure produced while constructing checked entry bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEntryDiagnostic {
    code: &'static str,
    message: String,
    primary: SourceSpan,
    related: Vec<SourceSpan>,
}

/// Exact ordinary-function declarations selected as Agent entry controllers.
///
/// This inventory is resolved from typed entry-role references and the accepted
/// project symbol table. It deliberately does not infer controller roles from
/// function bodies, effects, names, or attributes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SelectedAgentControllerRoles {
    declarations: BTreeSet<CallableDeclarationId>,
}

impl SelectedAgentControllerRoles {
    fn contains(&self, declaration: &CallableDeclarationId) -> bool {
        self.declarations.contains(declaration)
    }
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

/// Resolves every source entry against one already registered semantic world.
///
/// Callable roles are resolved by the ordinary project symbol table and must
/// name the same declaration published by the shared callable catalog.
pub fn check_project_entries(
    project: &HirProject,
    symbols: &ProjectSymbolTable,
    callables: &RegisteredCallableCatalog,
    typecheck: &TypeCheckReport,
) -> Result<CheckedEntryCatalog, Vec<CheckedEntryDiagnostic>> {
    let context = EntryCheckContext::new(project, symbols, callables, typecheck);
    context.check()
}

struct EntryCheckContext<'a> {
    project: &'a HirProject,
    symbols: &'a ProjectSymbolTable,
    callables: &'a RegisteredCallableCatalog,
    typecheck: &'a TypeCheckReport,
    nominals: NominalSchemaResolver<'a>,
    functions: BTreeMap<CallableDeclarationId, (&'a HirModule, &'a HirFunction)>,
    flows: BTreeMap<String, Vec<(&'a HirModule, &'a HirFlow)>>,
}

struct ResolvedCallable<'a> {
    declaration: &'a CallableDeclarationId,
    module_path: &'a CanonicalModulePath,
    module: &'a HirModule,
    function: &'a HirFunction,
    record: &'a CallableRecord,
    source: SourceSpan,
}

impl<'a> EntryCheckContext<'a> {
    fn new(
        project: &'a HirProject,
        symbols: &'a ProjectSymbolTable,
        callables: &'a RegisteredCallableCatalog,
        typecheck: &'a TypeCheckReport,
    ) -> Self {
        let nominals = NominalSchemaResolver::new(project);
        let mut functions = BTreeMap::new();
        let mut flows = BTreeMap::<String, Vec<_>>::new();
        for (_, module) in project.modules() {
            for function in module.functions() {
                if let Ok(declaration) =
                    CallableDeclarationId::for_function(project.package(), function)
                {
                    functions.insert(declaration, (module, function));
                }
            }
            for flow in module.flows() {
                if let Some(id) = flow.id() {
                    flows
                        .entry(id.body().to_owned())
                        .or_default()
                        .push((module, flow));
                }
            }
        }
        Self {
            project,
            symbols,
            callables,
            typecheck,
            nominals,
            functions,
            flows,
        }
    }

    fn check(self) -> Result<CheckedEntryCatalog, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Vec::new();
        let mut entries = Vec::new();
        let mut ids = BTreeMap::<String, SourceSpan>::new();
        let selected_agent_controllers = self.selected_agent_controllers();
        self.validate_function_role_attributes(&selected_agent_controllers, &mut diagnostics);
        self.validate_agent_callable_roles(&selected_agent_controllers, &mut diagnostics);

        for (module_path, module) in self.project.modules() {
            for declaration in module.declarations() {
                let HirTopLevelDecl::Entry(entry) = declaration else {
                    continue;
                };
                let id_span = source_span(module, *entry.id().range());
                let id_label = entry.id().body();
                if !id_label.starts_with("entry.") {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.invalid_id_family",
                        format!("entry ID `{id_label}` must use the `entry.*` family"),
                        id_span,
                    ));
                    continue;
                }
                let Ok(id) = CheckedEntryId::try_new(id_label.to_owned()) else {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.invalid_id",
                        format!("entry ID `{id_label}` is not canonical"),
                        id_span,
                    ));
                    continue;
                };
                if let Some(first) = ids.insert(id_label.to_owned(), id_span.clone()) {
                    diagnostics.push(
                        CheckedEntryDiagnostic::new(
                            "sema.entry.duplicate_id",
                            format!("entry ID `{id_label}` is declared more than once"),
                            id_span,
                        )
                        .with_related([first]),
                    );
                    continue;
                }

                match self.check_entry(module_path, module, entry, id) {
                    Ok(binding) => entries.push(binding),
                    Err(mut entry_diagnostics) => diagnostics.append(&mut entry_diagnostics),
                }
            }
        }

        if diagnostics.is_empty() {
            CheckedEntryCatalog::try_new(entries).map_err(|error| {
                vec![CheckedEntryDiagnostic::new(
                    "sema.entry.duplicate_id",
                    error.to_string(),
                    ids.values()
                        .next()
                        .expect("a duplicate checked ID requires at least one source entry")
                        .clone(),
                )]
            })
        } else {
            Err(diagnostics)
        }
    }

    fn selected_agent_controllers(&self) -> SelectedAgentControllerRoles {
        let declarations = self
            .project
            .modules()
            .flat_map(|(module_path, module)| {
                module.declarations().iter().filter_map(move |declaration| {
                    let HirTopLevelDecl::Entry(entry) = declaration else {
                        return None;
                    };
                    if entry.kind() != &EntryKind::Agent {
                        return None;
                    }
                    entry.items().iter().find_map(|item| {
                        let HirEntryItem::Controller {
                            path, value_range, ..
                        } = item
                        else {
                            return None;
                        };
                        let source = source_span(module, *value_range);
                        resolve_selected_agent_controller(self.symbols, module_path, path, &source)
                    })
                })
            })
            .collect();
        SelectedAgentControllerRoles { declarations }
    }

    fn validate_function_role_attributes(
        &self,
        selected_agent_controllers: &SelectedAgentControllerRoles,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) {
        for (declaration, (module, function)) in &self.functions {
            for attribute in function.attributes() {
                if matches!(attribute.name(), "agent" | "launch" | "bind") {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.forbidden_role_attribute",
                        format!(
                            "`#[{}]` cannot assign a function entry role; bind the ordinary function from an `entry` declaration",
                            attribute.name()
                        ),
                        source_span(module, *attribute.range()),
                    ));
                }
                if attribute.name() == "budget" && !selected_agent_controllers.contains(declaration)
                {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.unbound_agent_budget",
                        "`#[budget(...)]` is only valid on an ordinary function selected by an Agent entry",
                        source_span(module, *attribute.range()),
                    ));
                }
            }
        }
    }

    fn validate_agent_callable_roles(
        &self,
        selected_agent_controllers: &SelectedAgentControllerRoles,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) {
        // Rejected calls already stop at type checking. Entry policy owns only
        // successful Agent-family selections whose lexical function lacks the
        // exact controller role resolved above.
        diagnostics.extend(self.typecheck.retained_call_target_facts().filter_map(|facts| {
            let CallTargetFact::Selected { selected, .. } = facts.target() else {
                return None;
            };
            if selected.id().family() != CallableFamily::Agent
                || facts
                    .enclosing_callable()
                    .is_some_and(|owner| selected_agent_controllers.contains(owner))
            {
                return None;
            }
            Some(CheckedEntryDiagnostic::new(
                "sema.entry.unbound_agent_intrinsic",
                "Agent call is only valid inside an ordinary function selected as an Agent entry controller",
                facts.call_span().clone(),
            ))
        }));
    }

    fn check_entry(
        &self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        entry: &HirEntryDecl,
        id: CheckedEntryId,
    ) -> Result<CheckedEntryBinding, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Self::validate_entry_members(module, entry);
        let result = match entry.kind() {
            EntryKind::Game => self.check_stateful(
                module_path,
                module,
                entry,
                id,
                CheckedStatefulEntryKind::Game,
            ),
            EntryKind::Editor => self.check_stateful(
                module_path,
                module,
                entry,
                id,
                CheckedStatefulEntryKind::Editor,
            ),
            EntryKind::Test => self.check_stateful(
                module_path,
                module,
                entry,
                id,
                CheckedStatefulEntryKind::Test,
            ),
            EntryKind::Agent => self.check_agent(module_path, module, entry, id),
            kind => Ok(self.check_existing(id, kind)),
        };
        match result {
            Ok(binding) if diagnostics.is_empty() => Ok(binding),
            Ok(_) => Err(diagnostics),
            Err(mut binding_diagnostics) => {
                diagnostics.append(&mut binding_diagnostics);
                Err(diagnostics)
            }
        }
    }

    fn validate_entry_members(
        module: &HirModule,
        entry: &HirEntryDecl,
    ) -> Vec<CheckedEntryDiagnostic> {
        let mut diagnostics = Vec::new();
        for item in entry.items() {
            let role = match item {
                HirEntryItem::StateType { .. } => Some(EntryRoleKind::State),
                HirEntryItem::Initializer { .. } => Some(EntryRoleKind::Initializer),
                HirEntryItem::EventType { .. } => Some(EntryRoleKind::Event),
                HirEntryItem::Reducer { .. } => Some(EntryRoleKind::Reducer),
                HirEntryItem::Controller { .. } => Some(EntryRoleKind::Controller),
                HirEntryItem::Goto(_)
                | HirEntryItem::Route { .. }
                | HirEntryItem::Option { .. }
                | HirEntryItem::Raw(_) => None,
            };
            if let Some(role) = role
                && !entry.kind().allows_role(role)
            {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.incompatible_role",
                    format!(
                        "entry kind `{}` cannot bind the `{}` role",
                        entry.kind().as_str(),
                        role.as_str()
                    ),
                    source_span(
                        module,
                        *item
                            .range()
                            .expect("typed HIR role members retain their source range"),
                    ),
                ));
            }
            match item {
                HirEntryItem::Goto(target) if !entry.kind().allows_goto() => {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.incompatible_goto",
                        format!(
                            "entry kind `{}` cannot declare `goto`",
                            entry.kind().as_str()
                        ),
                        source_span(module, *target.range()),
                    ));
                }
                HirEntryItem::Route { target, .. } if !entry.kind().allows_routes() => {
                    diagnostics.push(CheckedEntryDiagnostic::new(
                        "sema.entry.incompatible_route",
                        format!(
                            "entry kind `{}` cannot declare adapter routes",
                            entry.kind().as_str()
                        ),
                        source_span(module, *target.range()),
                    ));
                }
                _ => {}
            }
        }
        diagnostics
    }

    #[allow(
        clippy::too_many_lines,
        reason = "stateful entry checking owns the atomic resolve-validate-digest transaction for all five required roles"
    )]
    fn check_stateful(
        &self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        entry: &HirEntryDecl,
        id: CheckedEntryId,
        kind: CheckedStatefulEntryKind,
    ) -> Result<CheckedEntryBinding, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Vec::new();
        let state = unique_item(module, entry, Role::State, &mut diagnostics).and_then(|item| {
            let HirEntryItem::StateType {
                ty, value_range, ..
            } = item
            else {
                unreachable!("role filter returns the matching typed entry member")
            };
            self.resolve_nominal(
                module_path,
                module,
                ty.value(),
                *value_range,
                "state",
                &mut diagnostics,
            )
        });
        let initializer =
            unique_item(module, entry, Role::Initializer, &mut diagnostics).and_then(|item| {
                let HirEntryItem::Initializer {
                    path, value_range, ..
                } = item
                else {
                    unreachable!("role filter returns the matching typed entry member")
                };
                self.resolve_callable(
                    module_path,
                    module,
                    path,
                    *value_range,
                    "initializer",
                    &mut diagnostics,
                )
            });
        let event = unique_item(module, entry, Role::Event, &mut diagnostics).and_then(|item| {
            let HirEntryItem::EventType {
                ty, value_range, ..
            } = item
            else {
                unreachable!("role filter returns the matching typed entry member")
            };
            self.resolve_nominal(
                module_path,
                module,
                ty.value(),
                *value_range,
                "event",
                &mut diagnostics,
            )
        });
        let reducer =
            unique_item(module, entry, Role::Reducer, &mut diagnostics).and_then(|item| {
                let HirEntryItem::Reducer {
                    path, value_range, ..
                } = item
                else {
                    unreachable!("role filter returns the matching typed entry member")
                };
                self.resolve_callable(
                    module_path,
                    module,
                    path,
                    *value_range,
                    "reducer",
                    &mut diagnostics,
                )
            });
        let (Some(state), Some(initializer), Some(event), Some(reducer)) =
            (state, initializer, event, reducer)
        else {
            return Err(diagnostics);
        };
        let contracts = EntryContractBuilder::new(&self.nominals, self.typecheck);
        let initializer = match contracts.initializer(
            initializer.module_path,
            initializer.module,
            initializer.function,
            initializer.record,
            initializer.declaration,
            state.key(),
        ) {
            Ok(contract) => CheckedCallableRole {
                declaration: initializer.declaration.clone(),
                contract_digest: digest::callable_contract(&contract),
                source: initializer.source,
            },
            Err(message) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.invalid_initializer_contract",
                    message,
                    initializer.source,
                ));
                return Err(diagnostics);
            }
        };
        let reducer = match contracts.reducer(
            reducer.module_path,
            reducer.module,
            reducer.function,
            reducer.record,
            reducer.declaration,
            ReducerContractNominals {
                state: state.key(),
                event: event.key(),
            },
        ) {
            Ok(contract) => CheckedCallableRole {
                declaration: reducer.declaration.clone(),
                contract_digest: digest::callable_contract(&contract),
                source: reducer.source,
            },
            Err(message) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.invalid_reducer_contract",
                    message,
                    reducer.source,
                ));
                return Err(diagnostics);
            }
        };
        let initial_flow =
            self.resolve_initial_flow(module_path, module, entry, state.key(), &mut diagnostics);
        let Some(initial_flow) = initial_flow else {
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
        module: &HirModule,
        entry: &HirEntryDecl,
        id: CheckedEntryId,
    ) -> Result<CheckedEntryBinding, Vec<CheckedEntryDiagnostic>> {
        let mut diagnostics = Vec::new();
        let controller =
            unique_item(module, entry, Role::Controller, &mut diagnostics).and_then(|item| {
                let HirEntryItem::Controller {
                    path, value_range, ..
                } = item
                else {
                    unreachable!("role filter returns the matching typed entry member")
                };
                self.resolve_callable(
                    module_path,
                    module,
                    path,
                    *value_range,
                    "controller",
                    &mut diagnostics,
                )
            });
        let Some(controller) = controller else {
            return Err(diagnostics);
        };
        let contracts = EntryContractBuilder::new(&self.nominals, self.typecheck);
        let (contract, allowed_effects, inferred_effects) = match contracts.agent_controller(
            controller.module_path,
            controller.module,
            controller.function,
            controller.record,
            controller.declaration,
        ) {
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
        let policy = CheckedAgentPolicy::new(allowed_effects, inferred_effects);
        let budget = match AgentBudget::from_attributes(controller.function.attributes()) {
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
        let controller = CheckedCallableRole {
            declaration: controller.declaration.clone(),
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
            id,
            controller,
            policy,
            budget,
            policy_digest,
            binding_digest,
        })))
    }

    fn check_existing(&self, id: CheckedEntryId, kind: &EntryKind) -> CheckedEntryBinding {
        let checked_kind = match kind {
            EntryKind::Cli => CheckedEntryKind::Cli,
            EntryKind::Server => CheckedEntryKind::Server,
            EntryKind::Activity => CheckedEntryKind::Activity,
            EntryKind::Bench => CheckedEntryKind::Bench,
            EntryKind::Custom(value) => CheckedEntryKind::Custom(value.clone()),
            EntryKind::Game | EntryKind::Editor | EntryKind::Test | EntryKind::Agent => {
                unreachable!("special entry kinds are checked by their typed paths")
            }
        };
        let binding_digest = digest::existing_binding(self.project.package(), &id, &checked_kind);
        CheckedEntryBinding::Existing(CheckedExistingEntry {
            id,
            kind: checked_kind,
            binding_digest,
        })
    }

    fn resolve_nominal(
        &self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        ty: &TypeRef,
        range: TextRange,
        role: &str,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<CheckedNominalRole> {
        let source = source_span(module, range);
        let TypeRef::Path(path) = ty else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.role_not_direct_nominal",
                format!("{role} role must name one direct project struct or enum"),
                source,
            ));
            return None;
        };
        match self
            .nominals
            .resolve_nominal(module_path, module, &path.canonical_string())
        {
            Ok(record) if record.is_generic() => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.generic_nominal_root",
                        format!(
                            "{role} role type `{path}` must be a non-generic project struct or enum"
                        ),
                        source,
                    )
                    .with_related([record.source.clone()]),
                );
                None
            }
            Ok(record) => match self.nominals.schema(record) {
                Ok(schema) => Some(CheckedNominalRole {
                    key: record.key.clone(),
                    schema_digest: digest::nominal_schema(&schema),
                    schema,
                    source,
                }),
                Err(error) => {
                    diagnostics.push(
                        CheckedEntryDiagnostic::new(
                            "sema.entry.invalid_nominal_schema",
                            format!(
                                "{role} role type `{path}` has no canonical data shape: {error}"
                            ),
                            source,
                        )
                        .with_related([record.source.clone()]),
                    );
                    None
                }
            },
            Err(NominalResolutionError::Unknown) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.unknown_nominal",
                    format!("{role} role type `{path}` is not visible from this module"),
                    source,
                ));
                None
            }
            Err(NominalResolutionError::Alias(candidates)) => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.alias_nominal_root",
                        format!(
                            "{role} role type `{path}` is a type alias; bind one direct project struct or enum"
                        ),
                        source,
                    )
                    .with_related(candidates),
                );
                None
            }
            Err(NominalResolutionError::Ambiguous(candidates)) => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.ambiguous_nominal",
                        format!("{role} role type `{path}` is ambiguous"),
                        source,
                    )
                    .with_related(candidates),
                );
                None
            }
        }
    }

    fn callable_resolution_sources(&self, error: &ProjectSymbolResolutionError) -> Vec<SourceSpan> {
        let declaration_source = |target: &ProjectSymbolTargetId| match target {
            ProjectSymbolTargetId::Callable(id) => self
                .symbols
                .callable(id.clone())
                .map(|symbol| symbol.source().clone()),
            ProjectSymbolTargetId::External(id) => self
                .symbols
                .external(*id)
                .map(|symbol| symbol.declaration_span().clone()),
            ProjectSymbolTargetId::Nominal(id) => self
                .symbols
                .nominal(id)
                .map(|symbol| symbol.source().whole().clone()),
            ProjectSymbolTargetId::Module(_) => None,
        };
        match error {
            ProjectSymbolResolutionError::Ambiguous { candidates, .. } => {
                candidates.iter().filter_map(declaration_source).collect()
            }
            ProjectSymbolResolutionError::NotCallable { actual, .. } => {
                declaration_source(actual).into_iter().collect()
            }
            ProjectSymbolResolutionError::Unknown { .. }
            | ProjectSymbolResolutionError::InvalidPath { .. } => Vec::new(),
        }
    }

    fn resolve_callable<'b>(
        &'b self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        path: &DottedPath,
        range: TextRange,
        role: &str,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<ResolvedCallable<'b>> {
        let source = source_span(module, range);
        let authored_leaf = path
            .segments()
            .last()
            .and_then(|leaf| range.end().checked_sub(leaf.as_str().len()))
            .map_or_else(
                || source.clone(),
                |start| source_span(module, TextRange::new(start, range.end())),
            );
        let parsed = ProjectSymbolPath::from_str(path.as_str())
            .map_err(|error| error.to_string())
            .and_then(|path| SymbolPath::try_from(&path).map_err(|error| error.to_string()));
        let symbol_path = match parsed {
            Ok(path) => path,
            Err(error) => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.invalid_callable_path",
                    format!("invalid {role} role path `{path}`: {error}"),
                    source,
                ));
                return None;
            }
        };
        let symbol = match self
            .symbols
            .resolve_callable(module_path, &symbol_path, &source)
        {
            Ok(symbol) => symbol,
            Err(error) => {
                let related = self.callable_resolution_sources(&error);
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.unresolved_callable",
                        format!("cannot resolve {role} role `{path}`: {error}"),
                        source,
                    )
                    .with_related(related),
                );
                return None;
            }
        };
        let declaration = symbol.declaration();
        let Some(record) = self.callables.project_record(declaration) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.callable_not_registered",
                format!("{role} role `{path}` is not in the accepted callable catalog"),
                source,
            ));
            return None;
        };
        let Some((function_module, function)) = self.functions.get(declaration) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.callable_not_function",
                format!("{role} role `{path}` does not name an ordinary function"),
                source,
            ));
            return None;
        };
        Some(ResolvedCallable {
            declaration,
            module_path: function
                .module_path()
                .expect("registered project functions retain a canonical module path"),
            module: function_module,
            function,
            record,
            source: authored_leaf,
        })
    }

    fn resolve_initial_flow(
        &self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        entry: &HirEntryDecl,
        state: &BoundNominalTypeKey,
        diagnostics: &mut Vec<CheckedEntryDiagnostic>,
    ) -> Option<CheckedInitialFlowRole> {
        let target = unique_initial_flow_target(module, entry, diagnostics)?;
        let source = source_span(module, *target.range());
        if !target.body().starts_with("flow.") {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_flow_family",
                format!(
                    "initial target `{}` must use the `flow.*` family",
                    target.body()
                ),
                source,
            ));
            return None;
        }
        let Ok(id) = CheckedFlowId::try_new(target.body().to_owned()) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.invalid_flow_id",
                format!("initial target `{}` is not canonical", target.body()),
                source,
            ));
            return None;
        };
        match self.flows.get(target.body()).map(Vec::as_slice) {
            Some([(flow_module, flow)]) => {
                let contracts = EntryContractBuilder::new(&self.nominals, self.typecheck);
                match contracts.flow(
                    flow.module_path().unwrap_or(module_path),
                    flow_module,
                    flow,
                    state,
                ) {
                    Ok(contract) => Some(CheckedInitialFlowRole {
                        contract_digest: digest::flow_contract(&id, &contract),
                        state_parameter_name: flow
                            .signature()
                            .and_then(|signature| signature.param_groups().first())
                            .and_then(|group| group.params().first())
                            .and_then(|parameter| parameter.pattern().simple_binding_name())
                            .expect("accepted initial-flow contract has one direct state binding")
                            .to_owned(),
                        id,
                        source,
                    }),
                    Err(message) => {
                        diagnostics.push(CheckedEntryDiagnostic::new(
                            "sema.entry.invalid_initial_flow_contract",
                            message,
                            source,
                        ));
                        None
                    }
                }
            }
            Some(candidates) if candidates.len() > 1 => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.ambiguous_flow",
                        format!(
                            "initial flow `{}` is declared more than once",
                            target.body()
                        ),
                        source,
                    )
                    .with_related(
                        candidates
                            .iter()
                            .map(|(module, flow)| source_span(module, *flow.range())),
                    ),
                );
                None
            }
            _ => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.unknown_flow",
                    format!("initial flow `{}` does not exist", target.body()),
                    source,
                ));
                None
            }
        }
    }
}

fn unique_initial_flow_target<'a>(
    module: &HirModule,
    entry: &'a HirEntryDecl,
    diagnostics: &mut Vec<CheckedEntryDiagnostic>,
) -> Option<&'a EntityRef> {
    let gotos = entry
        .items()
        .iter()
        .filter_map(|item| match item {
            HirEntryItem::Goto(target) => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [target] = gotos.as_slice() else {
        diagnostics.push(
            CheckedEntryDiagnostic::new(
                "sema.entry.goto_cardinality",
                "stateful entry must contain exactly one initial `goto` target",
                source_span(module, *entry.range()),
            )
            .with_related(
                gotos
                    .iter()
                    .map(|target| source_span(module, *target.range())),
            ),
        );
        return None;
    };
    Some(target)
}

fn resolve_selected_agent_controller(
    symbols: &ProjectSymbolTable,
    module: &CanonicalModulePath,
    path: &DottedPath,
    source: &SourceSpan,
) -> Option<CallableDeclarationId> {
    let path = ProjectSymbolPath::from_str(path.as_str()).ok()?;
    let path = SymbolPath::try_from(&path).ok()?;
    symbols
        .resolve_callable(module, &path, source)
        .ok()
        .map(|symbol| symbol.declaration().clone())
}

fn source_span(module: &HirModule, range: TextRange) -> SourceSpan {
    module
        .source_span(range)
        .expect("HIR projects retain the exact source document used for lowering")
}
