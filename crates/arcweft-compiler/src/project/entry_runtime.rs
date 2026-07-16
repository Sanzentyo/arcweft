//! Checked entry-role projection into the executable runtime catalog.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::{
    entry::{
        AgentBudget as RuntimeAgentBudget, AgentPolicyHash, CallableContractHash,
        EntryBindingIdentity, FlowContractHash, RuntimeAgentEntryRoles, RuntimeBytesFormat,
        RuntimeCallableExecutable, RuntimeCallableExecutableCode, RuntimeCallableId,
        RuntimeCallableRole, RuntimeCommandPolicy, RuntimeEntryRoles, RuntimeEnumRepr,
        RuntimeEnumTagStyle, RuntimeFlowExecutable, RuntimeFlowExecutableParameter,
        RuntimeFlowParameterMode, RuntimeFlowRole, RuntimeNominalRole, RuntimeNominalTypeId,
        RuntimeSchemaField, RuntimeSchemaVariant, RuntimeStatefulEntryRoles, RuntimeTypeSchema,
        TypeLayoutHash,
    },
    plan::{EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget},
};
use arcweft_data::{BytesFormat, EnumRepr, EnumTagStyle, TypeShape};
use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_lang_sema::entry::{
    AgentBudget as CheckedAgentBudget, BoundNominalTypeKey, CheckedAgentEntry, CheckedCallableRole,
    CheckedEntryBinding, CheckedEntryCatalog, CheckedEntryId, CheckedEntryKind,
    CheckedInitialFlowRole, CheckedNominalRole, CheckedStatefulEntry,
};
use arcweft_runtime_plan::flow::{
    RuntimeAgentControllerRequest, RuntimePlanLowerReport, RuntimePureHelperSource,
};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub(super) enum EntryRuntimeProjectionError {
    #[error("checked entry `{entry}` has no raw runtime entry")]
    MissingRawEntry { entry: String },
    #[error("raw runtime entry `{entry}` has no checked semantic binding")]
    UncheckedRawEntry { entry: String },
    #[error(
        "checked entry `{entry}` has kind `{checked}`, but raw runtime lowering produced `{lowered}`"
    )]
    EntryKindMismatch {
        entry: String,
        checked: String,
        lowered: String,
    },
    #[error("checked entry runtime identity is invalid: {0}")]
    InvalidEntryIdentity(String),
    #[error("checked flow runtime identity is invalid: {0}")]
    InvalidFlowIdentity(String),
    #[error("checked role identity is invalid: {0}")]
    InvalidRoleIdentity(String),
    #[error(
        "stateful entry `{entry}` requires an explicit selected-adapter command constructor policy"
    )]
    MissingCommandPolicy { entry: String },
    #[error(
        "checked callable `{callable}` resolved to {matches} executable pure-helper candidates"
    )]
    CallableExecutableCardinality { callable: String, matches: usize },
    #[error("checked callable `{callable}` has conflicting executable metadata")]
    ConflictingCallableExecutable { callable: String },
    #[error("checked flow `{flow}` has conflicting executable metadata")]
    ConflictingFlowExecutable { flow: String },
}

/// Owns the typed projection from checked semantic entry contracts into the
/// executable runtime model.
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

/// Builds the exact ordinary-function controller requests needed before flow
/// lowering. The request is derived only from the accepted checked catalog.
pub(super) fn agent_controller_requests(
    catalog: &CheckedEntryCatalog,
) -> Result<Vec<RuntimeAgentControllerRequest>, EntryRuntimeProjectionError> {
    let mut requests = BTreeMap::new();
    for entry in catalog.entries().filter_map(CheckedEntryBinding::agent) {
        let declaration = entry.controller().declaration().clone();
        requests
            .entry(declaration.clone())
            .or_insert(RuntimeAgentControllerRequest {
                flow: agent_controller_flow(entry.controller())?,
                declaration,
            });
    }
    Ok(requests.into_values().collect())
}

/// Returns the exact ordinary functions that must be executable for stateful
/// entry transactions. This keeps nominal/reference helper support scoped to
/// accepted entry roles rather than broadening ordinary helper inference.
pub(super) fn stateful_callable_requests(
    catalog: &CheckedEntryCatalog,
) -> Vec<CallableDeclarationId> {
    let mut requests = BTreeSet::new();
    for entry in catalog.entries().filter_map(CheckedEntryBinding::stateful) {
        requests.insert(entry.initializer().declaration().clone());
        requests.insert(entry.reducer().declaration().clone());
    }
    requests.into_iter().collect()
}

/// Replaces provisional syntax-only entry records with exact checked roles and
/// executable ownership metadata.
pub(super) fn attach_checked_entries(
    report: &mut RuntimePlanLowerReport,
    catalog: &CheckedEntryCatalog,
    command_policy: Option<&RuntimeCommandPolicy>,
) -> Result<(), EntryRuntimeProjectionError> {
    let mut raw_entries = std::mem::take(&mut report.plan.entries)
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut entries = Vec::with_capacity(catalog.len());
    let mut callables = BTreeMap::<RuntimeCallableId, RuntimeCallableExecutable>::new();
    let mut flows = BTreeMap::<FlowRuntimeId, RuntimeFlowExecutable>::new();
    for binding in catalog.entries() {
        let runtime_id = runtime_entry_id(binding.id())?;
        let mut entry = raw_entries.remove(&runtime_id).ok_or_else(|| {
            EntryRuntimeProjectionError::MissingRawEntry {
                entry: binding.id().to_string(),
            }
        })?;
        let checked_kind = runtime_entry_kind(&binding.kind());
        if entry.kind.as_str() != checked_kind.as_str() {
            return Err(EntryRuntimeProjectionError::EntryKindMismatch {
                entry: binding.id().to_string(),
                checked: checked_kind.as_str().to_owned(),
                lowered: entry.kind.as_str().to_owned(),
            });
        }
        entry.kind = checked_kind;
        let binding_identity =
            EntryBindingIdentity::from_bytes(*binding.binding_digest().as_bytes());
        entry.binding = binding_identity;

        match binding {
            CheckedEntryBinding::Stateful(checked) => {
                project_stateful_entry(
                    &mut entry,
                    checked,
                    binding_identity,
                    command_policy,
                    &mut callables,
                    &report.pure_helper_sources,
                    &mut flows,
                )?;
            }
            CheckedEntryBinding::Agent(checked) => {
                project_agent_entry(
                    &mut entry,
                    checked,
                    binding_identity,
                    &mut callables,
                    &mut flows,
                )?;
            }
            CheckedEntryBinding::Existing(_) => {
                entry.roles = RuntimeEntryRoles::None;
            }
        }
        entries.push(entry);
    }

    if let Some((_, entry)) = raw_entries.into_iter().next() {
        return Err(EntryRuntimeProjectionError::UncheckedRawEntry {
            entry: entry.id.public_label().into_string(),
        });
    }

    report.plan.entries = entries;
    report.plan.callable_executables = callables.into_values().collect();
    report.plan.flow_executables = flows.into_values().collect();
    Ok(())
}

fn project_stateful_entry(
    entry: &mut RuntimeEntrySpec,
    checked: &CheckedStatefulEntry,
    binding: EntryBindingIdentity,
    command_policy: Option<&RuntimeCommandPolicy>,
    callables: &mut BTreeMap<RuntimeCallableId, RuntimeCallableExecutable>,
    pure_helper_sources: &[RuntimePureHelperSource],
    flows: &mut BTreeMap<FlowRuntimeId, RuntimeFlowExecutable>,
) -> Result<(), EntryRuntimeProjectionError> {
    let command_policy = command_policy.cloned().ok_or_else(|| {
        EntryRuntimeProjectionError::MissingCommandPolicy {
            entry: checked.id().to_string(),
        }
    })?;
    let state = RuntimeSchemaProjection::nominal(checked.state())?;
    let event = RuntimeSchemaProjection::nominal(checked.event())?;
    let initializer = runtime_callable_role(checked.initializer())?;
    let reducer = runtime_callable_role(checked.reducer())?;
    register_pure_callable(
        callables,
        pure_helper_sources,
        checked.initializer(),
        &initializer,
    )?;
    register_pure_callable(callables, pure_helper_sources, checked.reducer(), &reducer)?;
    let initial_flow = runtime_flow_role(checked.initial_flow())?;
    register_flow(
        flows,
        RuntimeFlowExecutable {
            flow: initial_flow.flow.clone(),
            contract: initial_flow.contract,
            parameters: vec![RuntimeFlowExecutableParameter {
                position: 0,
                name: checked.initial_flow().state_parameter_name().to_owned(),
                mode: RuntimeFlowParameterMode::Owned,
                nominal: state.identity.clone(),
                layout: state.layout,
            }],
            controller: None,
        },
    )?;
    entry.target = RuntimeEntryTarget::Flow(initial_flow.flow.clone());
    entry.roles = RuntimeEntryRoles::Stateful(Box::new(RuntimeStatefulEntryRoles {
        binding,
        state,
        initializer,
        event,
        reducer,
        initial_flow,
        command_policy,
    }));
    Ok(())
}

fn project_agent_entry(
    entry: &mut RuntimeEntrySpec,
    checked: &CheckedAgentEntry,
    binding: EntryBindingIdentity,
    callables: &mut BTreeMap<RuntimeCallableId, RuntimeCallableExecutable>,
    flows: &mut BTreeMap<FlowRuntimeId, RuntimeFlowExecutable>,
) -> Result<(), EntryRuntimeProjectionError> {
    let controller = runtime_callable_role(checked.controller())?;
    let controller_flow = agent_controller_flow(checked.controller())?;
    register_callable(
        callables,
        RuntimeCallableExecutable {
            callable: controller.callable.clone(),
            contract: controller.contract,
            code: RuntimeCallableExecutableCode::ControllerFlow(controller_flow.clone()),
        },
    )?;
    register_flow(
        flows,
        RuntimeFlowExecutable {
            flow: controller_flow.clone(),
            contract: FlowContractHash::from_bytes(
                *checked.controller().contract_digest().as_bytes(),
            ),
            parameters: Vec::new(),
            controller: Some(controller.clone()),
        },
    )?;
    entry.target = RuntimeEntryTarget::Controller(controller_flow);
    entry.roles = RuntimeEntryRoles::Agent(Box::new(RuntimeAgentEntryRoles {
        binding,
        controller,
        policy: AgentPolicyHash::from_bytes(*checked.policy_digest().as_bytes()),
        budget: EntryRuntimeProjection::agent_budget(checked.budget()),
    }));
    Ok(())
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
        flow: FlowRuntimeId::from_source_entity_body(checked.id().public_id().as_str())
            .map_err(|error| EntryRuntimeProjectionError::InvalidFlowIdentity(error.to_string()))?,
        contract: FlowContractHash::from_bytes(*checked.contract_digest().as_bytes()),
    })
}

fn register_pure_callable(
    callables: &mut BTreeMap<RuntimeCallableId, RuntimeCallableExecutable>,
    sources: &[RuntimePureHelperSource],
    checked: &CheckedCallableRole,
    role: &RuntimeCallableRole,
) -> Result<(), EntryRuntimeProjectionError> {
    let declaration = checked.declaration();
    let matches = sources
        .iter()
        .filter(|source| {
            source.module.as_ref() == Some(declaration.module())
                && source.name == declaration.name()
        })
        .collect::<Vec<_>>();
    let [source] = matches.as_slice() else {
        return Err(EntryRuntimeProjectionError::CallableExecutableCardinality {
            callable: declaration.to_string(),
            matches: matches.len(),
        });
    };
    register_callable(
        callables,
        RuntimeCallableExecutable {
            callable: role.callable.clone(),
            contract: role.contract,
            code: RuntimeCallableExecutableCode::PureHelper(source.helper),
        },
    )
}

fn register_callable(
    callables: &mut BTreeMap<RuntimeCallableId, RuntimeCallableExecutable>,
    executable: RuntimeCallableExecutable,
) -> Result<(), EntryRuntimeProjectionError> {
    if let Some(existing) = callables.get(&executable.callable) {
        if existing != &executable {
            return Err(EntryRuntimeProjectionError::ConflictingCallableExecutable {
                callable: executable.callable.as_str().to_owned(),
            });
        }
        return Ok(());
    }
    callables.insert(executable.callable.clone(), executable);
    Ok(())
}

fn register_flow(
    flows: &mut BTreeMap<FlowRuntimeId, RuntimeFlowExecutable>,
    executable: RuntimeFlowExecutable,
) -> Result<(), EntryRuntimeProjectionError> {
    if let Some(existing) = flows.get(&executable.flow) {
        if existing != &executable {
            return Err(EntryRuntimeProjectionError::ConflictingFlowExecutable {
                flow: executable.flow.public_label().into_string(),
            });
        }
        return Ok(());
    }
    flows.insert(executable.flow.clone(), executable);
    Ok(())
}

struct RuntimeSchemaProjection;

impl RuntimeSchemaProjection {
    fn nominal(
        checked: &CheckedNominalRole,
    ) -> Result<RuntimeNominalRole, EntryRuntimeProjectionError> {
        Ok(RuntimeNominalRole {
            identity: RuntimeNominalTypeId::try_new(nominal_identity(checked.key())).map_err(
                |error| EntryRuntimeProjectionError::InvalidRoleIdentity(error.to_string()),
            )?,
            layout: TypeLayoutHash::from_bytes(*checked.schema_digest().as_bytes()),
            schema: Self::schema(checked.schema()),
        })
    }

    fn schema(shape: &TypeShape) -> RuntimeTypeSchema {
        match shape {
            TypeShape::Unit => RuntimeTypeSchema::Unit,
            TypeShape::Bool => RuntimeTypeSchema::Bool,
            TypeShape::I8 => RuntimeTypeSchema::I8,
            TypeShape::I16 => RuntimeTypeSchema::I16,
            TypeShape::I32 => RuntimeTypeSchema::I32,
            TypeShape::I64 => RuntimeTypeSchema::I64,
            TypeShape::I128 => RuntimeTypeSchema::I128,
            TypeShape::Isize => RuntimeTypeSchema::ISize,
            TypeShape::U8 => RuntimeTypeSchema::U8,
            TypeShape::U16 => RuntimeTypeSchema::U16,
            TypeShape::U32 => RuntimeTypeSchema::U32,
            TypeShape::U64 => RuntimeTypeSchema::U64,
            TypeShape::U128 => RuntimeTypeSchema::U128,
            TypeShape::Usize => RuntimeTypeSchema::USize,
            TypeShape::F32 => RuntimeTypeSchema::F32,
            TypeShape::F64 => RuntimeTypeSchema::F64,
            TypeShape::String => RuntimeTypeSchema::String,
            TypeShape::Char => RuntimeTypeSchema::Char,
            TypeShape::Bytes { format } => RuntimeTypeSchema::Bytes {
                format: runtime_bytes_format(*format),
            },
            TypeShape::Option(inner) => RuntimeTypeSchema::Option(Box::new(Self::schema(inner))),
            TypeShape::Seq(inner) => RuntimeTypeSchema::Seq(Box::new(Self::schema(inner))),
            TypeShape::Map { key, value } => RuntimeTypeSchema::Map {
                key: Box::new(Self::schema(key)),
                value: Box::new(Self::schema(value)),
            },
            TypeShape::Record {
                name,
                fields,
                policy,
            } => RuntimeTypeSchema::Record {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| RuntimeSchemaField {
                        rust_name: field.rust_name.clone(),
                        wire_name: field.wire_name.clone(),
                        schema: Self::schema(&field.shape),
                        has_default: field.has_default,
                        skip: field.skip,
                        bytes_format: field.bytes_format.map(runtime_bytes_format),
                    })
                    .collect(),
                deny_unknown_fields: policy.deny_unknown_fields,
            },
            TypeShape::Enum {
                name,
                variants,
                tag,
                repr,
            } => RuntimeTypeSchema::Enum {
                name: name.clone(),
                variants: variants
                    .iter()
                    .map(|variant| RuntimeSchemaVariant {
                        rust_name: variant.rust_name.clone(),
                        wire_name: variant.wire_name.clone(),
                        payload: variant.payload.as_ref().map(Self::schema),
                        discriminant: variant.discriminant,
                    })
                    .collect(),
                tag: match tag {
                    EnumTagStyle::External => RuntimeEnumTagStyle::External,
                    EnumTagStyle::Internal { tag } => {
                        RuntimeEnumTagStyle::Internal { tag: tag.clone() }
                    }
                    EnumTagStyle::Adjacent { tag, content } => RuntimeEnumTagStyle::Adjacent {
                        tag: tag.clone(),
                        content: content.clone(),
                    },
                },
                repr: repr.map(runtime_enum_repr),
            },
            TypeShape::Named(name) => RuntimeTypeSchema::Named(name.clone()),
        }
    }
}

fn nominal_identity(key: &BoundNominalTypeKey) -> String {
    format!(
        "{}::{}::{}",
        key.package().as_str(),
        key.module(),
        key.name()
    )
}

const fn runtime_bytes_format(format: BytesFormat) -> RuntimeBytesFormat {
    match format {
        BytesFormat::Binary => RuntimeBytesFormat::Binary,
        BytesFormat::Base64 => RuntimeBytesFormat::Base64,
        BytesFormat::Hex => RuntimeBytesFormat::Hex,
        BytesFormat::Array => RuntimeBytesFormat::Array,
    }
}

const fn runtime_enum_repr(repr: EnumRepr) -> RuntimeEnumRepr {
    match repr {
        EnumRepr::I8 => RuntimeEnumRepr::I8,
        EnumRepr::I16 => RuntimeEnumRepr::I16,
        EnumRepr::I32 => RuntimeEnumRepr::I32,
        EnumRepr::I64 => RuntimeEnumRepr::I64,
        EnumRepr::I128 => RuntimeEnumRepr::I128,
        EnumRepr::Isize => RuntimeEnumRepr::ISize,
        EnumRepr::U8 => RuntimeEnumRepr::U8,
        EnumRepr::U16 => RuntimeEnumRepr::U16,
        EnumRepr::U32 => RuntimeEnumRepr::U32,
        EnumRepr::U64 => RuntimeEnumRepr::U64,
        EnumRepr::U128 => RuntimeEnumRepr::U128,
        EnumRepr::Usize => RuntimeEnumRepr::USize,
    }
}
