//! Checked final-HIR Entry projection into the executable runtime catalog.

use arcweft_core::{
    entry::{
        AgentBudget as RuntimeAgentBudget, AgentPolicyHash, CallableContractHash,
        EntryBindingIdentity, FlowContractHash, RuntimeAgentEntryRoles, RuntimeCallableId,
        RuntimeCallableRole, RuntimeCommandPolicy, RuntimeEntryRoles, RuntimeFlowExecutable,
        RuntimeFlowExecutableParameter, RuntimeFlowParameterMode, RuntimeFlowRole,
        RuntimeFlowSchema, RuntimeNominalRole, RuntimeStatefulEntryRoles,
    },
    pattern::RuntimeSemanticTypeId,
    plan::{
        EntryRuntimeId, FlowRuntimeId, RouteCaptureCoordinate, RuntimeEntryKind, RuntimeEntrySpec,
        RuntimeEntryTarget, RuntimeHttpMethod, RuntimeRouteBinding, RuntimeRouteBindingSource,
        RuntimeRoutePath, RuntimeRoutePathSegment, RuntimeRouteSpec,
    },
};
use arcweft_lang_hir::{
    item::{HirHttpMethod, HirItemKind, HirRoutePathSegment},
    project::{
        HirExecutableProjectView, HirRuntimeExecutableOwner, HirRuntimeSemanticReachability,
    },
    symbol::{CallableDeclarationKey, ProjectSymbolTable},
};
use arcweft_lang_sema::{
    entry::{
        AgentBudget as CheckedAgentBudget, CheckedAgentEntry, CheckedCallableRole,
        CheckedEntryBinding, CheckedEntryFlowTarget, CheckedEntryId, CheckedEntryKind,
        CheckedEntryRoute, CheckedEntryRouteBindingSource, CheckedExistingEntry,
        CheckedExistingEntryTarget, CheckedInitialFlowRole, CheckedNominalRole,
        CheckedStatefulEntry,
    },
    final_analysis::FinalSemanticAnalysis,
};
use arcweft_runtime_plan::flow::{
    RuntimeCheckedEntryInput, RuntimeEntryCallableBody, RuntimeEntryCallableInput,
    RuntimeEntryFlowInput, RuntimeEntryLoweringInput,
};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub(crate) enum EntryRuntimeProjectionError {
    #[error("checked Entry owner {owner:?} is absent from the accepted final-HIR generation")]
    MissingEntryOwner {
        owner: arcweft_lang_hir::identity::ItemId,
    },
    #[error("checked Entry owner {owner:?} does not resolve to an Entry declaration")]
    EntryOwnerMismatch {
        owner: arcweft_lang_hir::identity::ItemId,
    },
    #[error("checked callable `{callable}` is absent from the final checked callable catalog")]
    MissingCheckedCallable { callable: String },
    #[error("checked callable `{callable}` is absent from the accepted project symbol table")]
    MissingCallableSymbol { callable: String },
    #[error("checked entry runtime identity is invalid: {0}")]
    InvalidEntryIdentity(String),
    #[error("checked flow runtime identity is invalid: {0}")]
    InvalidFlowIdentity(String),
    #[error("checked role identity is invalid: {0}")]
    InvalidRoleIdentity(String),
    #[error("checked nominal role `{nominal}` has an invalid sealed runtime relation: {reason}")]
    InvalidNominalRelation { nominal: String, reason: String },
    #[error(
        "stateful entry `{entry}` requires an explicit selected-adapter command constructor policy"
    )]
    MissingCommandPolicy { entry: String },
    #[error("checked Entry `{entry}` has an invalid sealed route plan: {reason}")]
    InvalidRoutePlan { entry: String, reason: String },
}

/// Owns typed schema and budget projection into the runtime vocabulary.
pub(crate) struct EntryRuntimeProjection;

impl EntryRuntimeProjection {
    pub(crate) const fn agent_budget(checked: CheckedAgentBudget) -> RuntimeAgentBudget {
        RuntimeAgentBudget {
            logical_timeout_millis: checked.logical_timeout_millis(),
            max_vm_steps: checked.max_vm_steps(),
            max_host_calls: checked.max_host_calls(),
            max_observations: checked.max_observations(),
            max_captures: checked.max_captures(),
            max_capture_bytes: checked.max_capture_bytes(),
            max_rag_queries: checked.max_rag_queries(),
            max_context_bytes: checked.max_context_bytes(),
        }
    }
}

/// Builds the sole generation-bound runtime Entry input directly from checked
/// semantic bindings and their exact final-HIR owners.
pub(super) fn runtime_entry_lowering_input(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
    reachability: &HirRuntimeSemanticReachability<'_>,
    command_policy: Option<&RuntimeCommandPolicy>,
) -> Result<RuntimeEntryLoweringInput, EntryRuntimeProjectionError> {
    let catalog = analysis.checked_entries();
    let mut entries = Vec::with_capacity(catalog.len());
    let mut callables = Vec::new();
    let mut flows = Vec::new();

    for binding in catalog.entries() {
        let owner = binding.source_item();
        if !reachability.contains_runtime_owner(&HirRuntimeExecutableOwner::Item(owner)) {
            continue;
        }
        let Some(item) = project.items().find(|item| item.id() == owner) else {
            return Err(EntryRuntimeProjectionError::MissingEntryOwner { owner });
        };
        let HirItemKind::Entry(_) = item.item().kind() else {
            return Err(EntryRuntimeProjectionError::EntryOwnerMismatch { owner });
        };
        let runtime_id = runtime_entry_id(binding.id())?;
        let binding_identity =
            EntryBindingIdentity::from_bytes(*binding.binding_digest().as_bytes());
        let kind = runtime_entry_kind(&binding.kind());

        let (target, roles) = match binding {
            CheckedEntryBinding::Stateful(checked) => {
                let (target, roles, entry_callables, entry_flows) = project_stateful_entry(
                    checked,
                    binding_identity,
                    command_policy,
                    symbols,
                    analysis,
                )?;
                callables.extend(entry_callables);
                flows.extend(entry_flows);
                (target, roles)
            }
            CheckedEntryBinding::Agent(checked) => {
                let (target, roles, controller) =
                    project_agent_entry(checked, binding_identity, symbols, analysis)?;
                callables.push(controller);
                (target, roles)
            }
            CheckedEntryBinding::Existing(checked) => {
                let (target, entry_flows) = project_existing_entry(checked)?;
                flows.extend(entry_flows);
                (target, RuntimeEntryRoles::None)
            }
        };

        entries.push(RuntimeCheckedEntryInput::new(
            owner,
            RuntimeEntrySpec {
                id: runtime_id,
                kind,
                binding: binding_identity,
                target,
                roles,
            },
        ));
    }

    Ok(RuntimeEntryLoweringInput::new(
        project, entries, callables, flows,
    ))
}

fn project_stateful_entry(
    checked: &CheckedStatefulEntry,
    binding: EntryBindingIdentity,
    command_policy: Option<&RuntimeCommandPolicy>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<
    (
        RuntimeEntryTarget,
        RuntimeEntryRoles,
        Vec<RuntimeEntryCallableInput>,
        Vec<RuntimeEntryFlowInput>,
    ),
    EntryRuntimeProjectionError,
> {
    let command_policy = command_policy.cloned().ok_or_else(|| {
        EntryRuntimeProjectionError::MissingCommandPolicy {
            entry: checked.id().to_string(),
        }
    })?;
    let state = RuntimeSchemaProjection::nominal(analysis, checked.state())?;
    let event = RuntimeSchemaProjection::nominal(analysis, checked.event())?;
    let initializer = runtime_callable_role(checked.initializer())?;
    let reducer = runtime_callable_role(checked.reducer())?;
    let initial_flow = runtime_flow_role(checked.initial_flow())?;
    let callable_inputs = vec![
        runtime_callable_input(
            checked.initializer(),
            initializer.clone(),
            RuntimeEntryCallableBody::PureHelper,
            symbols,
            analysis,
        )?,
        runtime_callable_input(
            checked.reducer(),
            reducer.clone(),
            RuntimeEntryCallableBody::PureHelper,
            symbols,
            analysis,
        )?,
    ];
    let flow_input = RuntimeEntryFlowInput::new(
        checked.initial_flow().source_item(),
        RuntimeFlowExecutable {
            flow: initial_flow.flow.clone(),
            contract: initial_flow.contract,
            controller: None,
        },
        RuntimeFlowSchema {
            flow: initial_flow.flow.clone(),
            parameters: vec![RuntimeFlowExecutableParameter {
                coordinate: arcweft_core::entry::FlowParameterCoordinate::from_position(0),
                name: checked.initial_flow().state_parameter_name().to_owned(),
                mode: RuntimeFlowParameterMode::Owned,
                semantic_identity: state.semantic_identity,
            }],
        },
    );
    let target = RuntimeEntryTarget::Flow(initial_flow.flow.clone());
    let roles = RuntimeEntryRoles::Stateful(Box::new(RuntimeStatefulEntryRoles {
        binding,
        state,
        initializer,
        event,
        reducer,
        initial_flow,
        command_policy,
    }));
    Ok((target, roles, callable_inputs, vec![flow_input]))
}

fn project_agent_entry(
    checked: &CheckedAgentEntry,
    binding: EntryBindingIdentity,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<
    (
        RuntimeEntryTarget,
        RuntimeEntryRoles,
        RuntimeEntryCallableInput,
    ),
    EntryRuntimeProjectionError,
> {
    let controller = runtime_callable_role(checked.controller())?;
    let controller_flow = agent_controller_flow(checked.controller())?;
    let callable = runtime_callable_input(
        checked.controller(),
        controller.clone(),
        RuntimeEntryCallableBody::ControllerFlow(controller_flow.clone()),
        symbols,
        analysis,
    )?;
    let target = RuntimeEntryTarget::Controller(controller_flow);
    let roles = RuntimeEntryRoles::Agent(Box::new(RuntimeAgentEntryRoles {
        binding,
        controller,
        policy: AgentPolicyHash::from_bytes(*checked.policy_digest().as_bytes()),
        budget: EntryRuntimeProjection::agent_budget(checked.budget()),
    }));
    Ok((target, roles, callable))
}

fn runtime_callable_input(
    checked: &CheckedCallableRole,
    role: RuntimeCallableRole,
    body: RuntimeEntryCallableBody,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeEntryCallableInput, EntryRuntimeProjectionError> {
    let declaration = CallableDeclarationKey::Existing(checked.declaration().clone());
    analysis
        .checked_callables()
        .project_callable(&declaration)
        .map_err(|_| EntryRuntimeProjectionError::MissingCheckedCallable {
            callable: checked.declaration().to_string(),
        })?;
    let symbol = symbols.callable(&declaration).ok_or_else(|| {
        EntryRuntimeProjectionError::MissingCallableSymbol {
            callable: checked.declaration().to_string(),
        }
    })?;
    Ok(RuntimeEntryCallableInput::new(
        declaration,
        symbol.source_item(),
        role,
        body,
    ))
}

fn project_existing_entry(
    checked: &CheckedExistingEntry,
) -> Result<(RuntimeEntryTarget, Vec<RuntimeEntryFlowInput>), EntryRuntimeProjectionError> {
    let mut flows = std::collections::BTreeMap::new();
    let target = match checked.target() {
        CheckedExistingEntryTarget::Flow(flow) => {
            let (runtime, input) = runtime_entry_flow(flow)?;
            flows.insert(runtime.clone(), input);
            RuntimeEntryTarget::Flow(runtime)
        }
        CheckedExistingEntryTarget::Routes(routes) => {
            let mut projected = Vec::with_capacity(routes.len());
            for route in routes {
                let (runtime, input) = runtime_entry_flow(route.target())?;
                if let Some(previous) = flows.insert(runtime.clone(), input.clone())
                    && previous != input
                {
                    return Err(EntryRuntimeProjectionError::InvalidFlowIdentity(format!(
                        "Entry `{}` retains conflicting schemas for Flow `{}`",
                        checked.id(),
                        runtime
                    )));
                }
                projected.push(runtime_entry_route(checked.id(), route, runtime)?);
            }
            RuntimeEntryTarget::Routes(projected)
        }
    };
    Ok((target, flows.into_values().collect()))
}

fn runtime_entry_flow(
    checked: &CheckedEntryFlowTarget,
) -> Result<(FlowRuntimeId, RuntimeEntryFlowInput), EntryRuntimeProjectionError> {
    let flow = FlowRuntimeId::from_checked_declaration_digest(
        checked.id().declaration_digest().into_bytes(),
        checked.id().public_id().as_str(),
    )
    .map_err(|error| EntryRuntimeProjectionError::InvalidFlowIdentity(error.to_string()))?;
    let executable = RuntimeFlowExecutable {
        flow: flow.clone(),
        contract: FlowContractHash::from_bytes(*checked.contract_digest().as_bytes()),
        controller: None,
    };
    let expected_schema = RuntimeFlowSchema {
        flow: flow.clone(),
        parameters: checked
            .parameters()
            .iter()
            .map(|parameter| RuntimeFlowExecutableParameter {
                coordinate: parameter.coordinate(),
                name: parameter.name().as_str().to_owned(),
                mode: RuntimeFlowParameterMode::Owned,
                semantic_identity: RuntimeSemanticTypeId::from_bytes(
                    *parameter.semantic_type().as_bytes(),
                ),
            })
            .collect(),
    };
    Ok((
        flow,
        RuntimeEntryFlowInput::new(checked.source_item(), executable, expected_schema),
    ))
}

fn runtime_entry_route(
    entry: &CheckedEntryId,
    checked: &CheckedEntryRoute,
    target: FlowRuntimeId,
) -> Result<RuntimeRouteSpec, EntryRuntimeProjectionError> {
    let segments = checked
        .path()
        .segments()
        .iter()
        .map(|segment| match segment {
            HirRoutePathSegment::Literal(literal) => {
                Ok(RuntimeRoutePathSegment::Literal(literal.to_string()))
            }
            HirRoutePathSegment::Capture(coordinate) => {
                let index = usize::try_from(coordinate.position()).map_err(|_| {
                    EntryRuntimeProjectionError::InvalidRoutePlan {
                        entry: entry.to_string(),
                        reason: "capture coordinate does not fit this platform".to_owned(),
                    }
                })?;
                let capture = checked.path().captures().get(index).ok_or_else(|| {
                    EntryRuntimeProjectionError::InvalidRoutePlan {
                        entry: entry.to_string(),
                        reason: "capture coordinate is absent from the checked path".to_owned(),
                    }
                })?;
                if capture.coordinate() != *coordinate {
                    return Err(EntryRuntimeProjectionError::InvalidRoutePlan {
                        entry: entry.to_string(),
                        reason: "capture coordinate/name relation is inconsistent".to_owned(),
                    });
                }
                Ok(RuntimeRoutePathSegment::Capture(
                    RouteCaptureCoordinate::from_position(coordinate.position()),
                ))
            }
        })
        .collect::<Result<Vec<_>, EntryRuntimeProjectionError>>()?;
    let path = RuntimeRoutePath::try_new(segments).map_err(|error| {
        EntryRuntimeProjectionError::InvalidRoutePlan {
            entry: entry.to_string(),
            reason: error.to_string(),
        }
    })?;
    let bindings = checked
        .bindings()
        .iter()
        .map(|binding| RuntimeRouteBinding {
            parameter: binding.parameter(),
            source: match binding.source() {
                CheckedEntryRouteBindingSource::PathCapture(capture) => {
                    RuntimeRouteBindingSource::PathCapture(RouteCaptureCoordinate::from_position(
                        capture.position(),
                    ))
                }
            },
        })
        .collect();
    Ok(RuntimeRouteSpec {
        method: runtime_http_method(checked.method()),
        path,
        target,
        bindings,
    })
}

const fn runtime_http_method(method: HirHttpMethod) -> RuntimeHttpMethod {
    match method {
        HirHttpMethod::Get => RuntimeHttpMethod::Get,
        HirHttpMethod::Post => RuntimeHttpMethod::Post,
        HirHttpMethod::Put => RuntimeHttpMethod::Put,
        HirHttpMethod::Patch => RuntimeHttpMethod::Patch,
        HirHttpMethod::Delete => RuntimeHttpMethod::Delete,
        HirHttpMethod::Head => RuntimeHttpMethod::Head,
        HirHttpMethod::Options => RuntimeHttpMethod::Options,
    }
}

fn runtime_entry_id(id: &CheckedEntryId) -> Result<EntryRuntimeId, EntryRuntimeProjectionError> {
    EntryRuntimeId::from_source_entity_body(id.public_id().as_str())
        .map_err(|error| EntryRuntimeProjectionError::InvalidEntryIdentity(error.to_string()))
}

fn agent_controller_flow(
    controller: &CheckedCallableRole,
) -> Result<FlowRuntimeId, EntryRuntimeProjectionError> {
    let callable = RuntimeCallableId::try_new(controller.declaration().to_string())
        .map_err(|error| EntryRuntimeProjectionError::InvalidRoleIdentity(error.to_string()))?;
    Ok(FlowRuntimeId::for_agent_controller_callable(&callable))
}

fn runtime_entry_kind(kind: &CheckedEntryKind) -> RuntimeEntryKind {
    match kind {
        CheckedEntryKind::Game => RuntimeEntryKind::Game,
        CheckedEntryKind::Editor => RuntimeEntryKind::Editor,
        CheckedEntryKind::Cli => RuntimeEntryKind::Cli,
        CheckedEntryKind::Server => RuntimeEntryKind::Server,
        CheckedEntryKind::Activity => RuntimeEntryKind::Activity,
        CheckedEntryKind::Test => RuntimeEntryKind::Test,
        CheckedEntryKind::Bench => RuntimeEntryKind::Bench,
        CheckedEntryKind::Agent => RuntimeEntryKind::Agent,
        CheckedEntryKind::Custom(value) => RuntimeEntryKind::Custom(value.clone()),
    }
}

fn runtime_callable_role(
    checked: &CheckedCallableRole,
) -> Result<RuntimeCallableRole, EntryRuntimeProjectionError> {
    Ok(RuntimeCallableRole {
        callable: RuntimeCallableId::try_new(checked.declaration().to_string())
            .map_err(|error| EntryRuntimeProjectionError::InvalidRoleIdentity(error.to_string()))?,
        contract: CallableContractHash::from_bytes(*checked.contract_digest().as_bytes()),
    })
}

fn runtime_flow_role(
    checked: &CheckedInitialFlowRole,
) -> Result<RuntimeFlowRole, EntryRuntimeProjectionError> {
    Ok(RuntimeFlowRole {
        flow: FlowRuntimeId::from_checked_declaration_digest(
            checked.id().declaration_digest().into_bytes(),
            checked.id().public_id().as_str(),
        )
        .map_err(|error| EntryRuntimeProjectionError::InvalidFlowIdentity(error.to_string()))?,
        contract: FlowContractHash::from_bytes(*checked.contract_digest().as_bytes()),
    })
}

pub(crate) struct RuntimeSchemaProjection;

impl RuntimeSchemaProjection {
    fn nominal(
        analysis: &FinalSemanticAnalysis,
        checked: &CheckedNominalRole,
    ) -> Result<RuntimeNominalRole, EntryRuntimeProjectionError> {
        let projection = analysis
            .checked_entry_runtime_nominal(checked)
            .map_err(
                |error| EntryRuntimeProjectionError::InvalidNominalRelation {
                    nominal: format!("{:?}", checked.runtime_nominal()),
                    reason: error.to_string(),
                },
            )?;
        Ok(RuntimeNominalRole {
            identity: projection.nominal().clone(),
            semantic_identity: RuntimeSemanticTypeId::from_bytes(
                *checked.semantic_type().as_bytes(),
            ),
            layout: projection.layout(),
            schema: projection.schema().clone(),
        })
    }
}
