//! Checked final-HIR Entry projection into the executable runtime catalog.

use arcweft_core::{
    entry::{
        AgentBudget as RuntimeAgentBudget, AgentPolicyHash, CallableContractHash,
        EntryBindingIdentity, FlowContractHash, RuntimeAgentEntryRoles, RuntimeBytesFormat,
        RuntimeCallableId, RuntimeCallableRole, RuntimeCommandPolicy, RuntimeEntryRoles,
        RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeFlowExecutable,
        RuntimeFlowExecutableParameter, RuntimeFlowParameterMode, RuntimeFlowRole,
        RuntimeNominalRole, RuntimeNominalTypeId, RuntimeSchemaError, RuntimeSchemaField,
        RuntimeSchemaVariant, RuntimeStatefulEntryRoles, RuntimeTypeSchema, TypeLayoutHash,
    },
    plan::{
        EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
        RuntimeRouteBinding, RuntimeRouteBindingSource, RuntimeRouteSpec,
    },
};
use arcweft_data::{BytesFormat, EnumRepr, EnumTagStyle, TypeShape};
use arcweft_lang_hir::{
    identity::ItemId,
    item::{
        HirEntryDeclaration, HirEntryMember, HirEntryRoute, HirEntryRouteBindings, HirEntryTarget,
        HirHttpMethod, HirHttpMethodValue, HirItemKind, HirRoutePathValue,
    },
    module::HirModule,
    project::{
        HirExecutableProjectView, HirRuntimeExecutableOwner, HirRuntimeSemanticReachability,
    },
    source_index::{
        HirEntrySourcePart, HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite,
    },
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, ProjectSymbolTable, ResolvedProjectSymbol,
    },
};
use arcweft_lang_sema::{
    entry::{
        AgentBudget as CheckedAgentBudget, BoundNominalTypeKey, CheckedAgentEntry,
        CheckedCallableRole, CheckedEntryBinding, CheckedEntryCatalog, CheckedEntryId,
        CheckedEntryKind, CheckedInitialFlowRole, CheckedNominalRole, CheckedStatefulEntry,
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
    #[error("runtime schema for nominal `{nominal}` cannot be canonically encoded")]
    NominalLayoutHash {
        nominal: String,
        #[source]
        source: RuntimeSchemaError,
    },
    #[error(
        "checked nominal schema digest for `{nominal}` differs from the projected runtime schema hash"
    )]
    NominalSchemaDigestMismatch {
        nominal: String,
        checked: [u8; 32],
        projected: TypeLayoutHash,
    },
    #[error(
        "stateful entry `{entry}` requires an explicit selected-adapter command constructor policy"
    )]
    MissingCommandPolicy { entry: String },
    #[error("checked Entry `{entry}` has no executable goto or route target")]
    MissingEntryTarget { entry: String },
    #[error("checked Entry `{entry}` mixes or repeats incompatible goto/route targets")]
    AmbiguousEntryTarget { entry: String },
    #[error("checked Entry `{entry}` contains recovered target or route metadata")]
    RecoveredEntryTarget { entry: String },
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
    catalog: &CheckedEntryCatalog,
    reachability: &HirRuntimeSemanticReachability<'_>,
    command_policy: Option<&RuntimeCommandPolicy>,
) -> Result<RuntimeEntryLoweringInput, EntryRuntimeProjectionError> {
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
        let HirItemKind::Entry(hir_entry) = item.item().kind() else {
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
            CheckedEntryBinding::Existing(_) => (
                project_existing_target(binding.id(), owner, item.module(), symbols, hir_entry)?,
                RuntimeEntryRoles::None,
            ),
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
    let state = RuntimeSchemaProjection::nominal(checked.state())?;
    let event = RuntimeSchemaProjection::nominal(checked.event())?;
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
            parameters: vec![RuntimeFlowExecutableParameter {
                position: 0,
                name: checked.initial_flow().state_parameter_name().to_owned(),
                mode: RuntimeFlowParameterMode::Owned,
                nominal: state.identity.clone(),
                layout: state.layout,
            }],
            controller: None,
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

fn project_existing_target(
    id: &CheckedEntryId,
    owner: ItemId,
    module: &HirModule,
    symbols: &ProjectSymbolTable,
    entry: &HirEntryDeclaration,
) -> Result<RuntimeEntryTarget, EntryRuntimeProjectionError> {
    let gotos = entry
        .members()
        .iter()
        .filter_map(|member| match member {
            HirEntryMember::Goto(goto) => Some(goto),
            _ => None,
        })
        .collect::<Vec<_>>();
    let routes = entry
        .members()
        .iter()
        .filter_map(|member| match member {
            HirEntryMember::Route(route) => Some(route),
            _ => None,
        })
        .collect::<Vec<_>>();
    match (gotos.as_slice(), routes.as_slice()) {
        ([goto], []) => Ok(RuntimeEntryTarget::Flow(runtime_flow_target(
            id,
            owner,
            module,
            symbols,
            goto.target(),
        )?)),
        ([], routes) if !routes.is_empty() => routes
            .iter()
            .map(|route| runtime_route(id, owner, module, symbols, route))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeEntryTarget::Routes),
        ([], []) => Err(EntryRuntimeProjectionError::MissingEntryTarget {
            entry: id.to_string(),
        }),
        _ => Err(EntryRuntimeProjectionError::AmbiguousEntryTarget {
            entry: id.to_string(),
        }),
    }
}

fn runtime_route(
    entry: &CheckedEntryId,
    owner: ItemId,
    module: &HirModule,
    symbols: &ProjectSymbolTable,
    route: &HirEntryRoute,
) -> Result<RuntimeRouteSpec, EntryRuntimeProjectionError> {
    let HirHttpMethodValue::Resolved(method) = route.method() else {
        return Err(recovered_entry_target(entry));
    };
    let HirRoutePathValue::Resolved(path) = route.path() else {
        return Err(recovered_entry_target(entry));
    };
    let bindings = match route.bindings() {
        HirEntryRouteBindings::Absent => Vec::new(),
        HirEntryRouteBindings::Parenthesized { items, closed } if *closed => items
            .iter()
            .map(|binding| {
                let (Some(parameter), Some(capture)) = (
                    binding.parameter().resolved(),
                    binding.path_capture().resolved(),
                ) else {
                    return Err(recovered_entry_target(entry));
                };
                if binding.has_recovery() {
                    return Err(recovered_entry_target(entry));
                }
                Ok(RuntimeRouteBinding {
                    name: parameter.as_str().to_owned(),
                    source: RuntimeRouteBindingSource::PathParam(capture.as_str().to_owned()),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        HirEntryRouteBindings::Parenthesized { .. } => {
            return Err(recovered_entry_target(entry));
        }
    };
    Ok(RuntimeRouteSpec {
        method: runtime_http_method(*method).to_owned(),
        path: path.as_str().to_owned(),
        target: runtime_flow_target(entry, owner, module, symbols, route.target())?,
        bindings,
    })
}

fn runtime_flow_target(
    entry: &CheckedEntryId,
    owner: ItemId,
    module: &HirModule,
    symbols: &ProjectSymbolTable,
    target: &HirEntryTarget,
) -> Result<FlowRuntimeId, EntryRuntimeProjectionError> {
    let HirEntryTarget::Authored(value) = target else {
        return Err(recovered_entry_target(entry));
    };
    let Some(reference) = value.as_resolved() else {
        return Err(recovered_entry_target(entry));
    };
    let source = entry_whole_source(module, owner)?;
    let symbol = symbols
        .resolve_entity_reference(module.key().path(), reference, source)
        .map_err(|error| EntryRuntimeProjectionError::InvalidFlowIdentity(error.to_string()))?;
    let ResolvedProjectSymbol::StructuralCallable(symbol) = symbol else {
        return Err(EntryRuntimeProjectionError::InvalidFlowIdentity(format!(
            "Entry `{entry}` target does not resolve to a structural Flow"
        )));
    };
    if symbol.owner() != CallableDeclarationOwner::Flow {
        return Err(EntryRuntimeProjectionError::InvalidFlowIdentity(format!(
            "Entry `{entry}` target does not resolve to a Flow"
        )));
    }
    let CallableDeclarationKey::Flow(declaration) = symbol.declaration() else {
        unreachable!("accepted structural Flow symbol owns a Flow declaration key")
    };
    FlowRuntimeId::from_checked_declaration_digest(
        declaration.semantic_digest().into_bytes(),
        declaration.public_id().as_str(),
    )
    .map_err(|error| EntryRuntimeProjectionError::InvalidFlowIdentity(error.to_string()))
}

fn entry_whole_source(
    module: &HirModule,
    owner: ItemId,
) -> Result<arcweft_source::SourceSpan, EntryRuntimeProjectionError> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item {
                owner,
                role: HirItemSourceRole::Entry(HirEntrySourcePart::Whole),
            },
        )
        .map_err(|error| EntryRuntimeProjectionError::InvalidFlowIdentity(error.to_string()))?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(source)) => Ok(source.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            Err(EntryRuntimeProjectionError::InvalidFlowIdentity(format!(
                "checked Entry {owner:?} has no authored whole-declaration source"
            )))
        }
    }
}

const fn runtime_http_method(method: HirHttpMethod) -> &'static str {
    match method {
        HirHttpMethod::Get => "GET",
        HirHttpMethod::Post => "POST",
        HirHttpMethod::Put => "PUT",
        HirHttpMethod::Patch => "PATCH",
        HirHttpMethod::Delete => "DELETE",
        HirHttpMethod::Head => "HEAD",
        HirHttpMethod::Options => "OPTIONS",
    }
}

fn recovered_entry_target(entry: &CheckedEntryId) -> EntryRuntimeProjectionError {
    EntryRuntimeProjectionError::RecoveredEntryTarget {
        entry: entry.to_string(),
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
        checked: &CheckedNominalRole,
    ) -> Result<RuntimeNominalRole, EntryRuntimeProjectionError> {
        let nominal = nominal_identity(checked.key());
        let schema = Self::schema(checked.schema());
        let layout = Self::layout_hash(&nominal, &schema)?;
        let checked_digest = *checked.schema_digest().as_bytes();
        if layout.as_bytes() != &checked_digest {
            return Err(EntryRuntimeProjectionError::NominalSchemaDigestMismatch {
                nominal,
                checked: checked_digest,
                projected: layout,
            });
        }
        Ok(RuntimeNominalRole {
            identity: RuntimeNominalTypeId::try_new(nominal).map_err(|error| {
                EntryRuntimeProjectionError::InvalidRoleIdentity(error.to_string())
            })?,
            layout,
            schema,
        })
    }

    pub(crate) fn layout_hash(
        nominal: &str,
        schema: &RuntimeTypeSchema,
    ) -> Result<TypeLayoutHash, EntryRuntimeProjectionError> {
        schema
            .try_layout_hash()
            .map_err(|source| EntryRuntimeProjectionError::NominalLayoutHash {
                nominal: nominal.to_owned(),
                source,
            })
    }

    pub(crate) fn schema(shape: &TypeShape) -> RuntimeTypeSchema {
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
