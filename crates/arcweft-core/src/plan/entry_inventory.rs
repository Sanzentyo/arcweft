//! Runtime entry inventory records and whole-plan verification.
//!
//! The data-only role contracts remain owned by [`crate::entry`]. This module
//! composes those contracts with runtime-plan identifiers, launch targets, and
//! executable metadata, then verifies the complete entry graph before runtime
//! selection. It performs no source resolution, adapter work, or platform I/O.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::entry::{
    CallableContractHash, EntryBindingIdentity, RuntimeCallableExecutable,
    RuntimeCallableExecutableCode, RuntimeCallableId, RuntimeCallableRole, RuntimeEntryRoles,
    RuntimeFlowExecutable, RuntimeFlowParameterMode, RuntimeStatefulEntryRoles,
};
use crate::runtime_id::{RuntimeIdError, RuntimeIdFamily, RuntimeIdPath, RuntimePublicLabel};

use super::{FlowRuntimeId, RuntimePlan};

/// Runtime identifier for a source-declared entry.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EntryRuntimeId {
    path: RuntimeIdPath,
}

/// Adapter family of a source-declared entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeEntryKind {
    Game,
    Editor,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
    Agent,
    Custom(String),
}

/// Launch target selected by an entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeEntryTarget {
    Flow(FlowRuntimeId),
    Routes(Vec<RuntimeRouteSpec>),
    Controller(FlowRuntimeId),
}

/// Route declaration in a server-like entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRouteSpec {
    pub method: String,
    pub path: String,
    pub target: FlowRuntimeId,
    pub bindings: Vec<RuntimeRouteBinding>,
}

/// Explicit route parameter binding for a target flow invocation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRouteBinding {
    pub name: String,
    pub source: RuntimeRouteBindingSource,
}

/// Adapter route value source used by a route binding.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeRouteBindingSource {
    PathParam(String),
}

/// Lowered entry declaration preserved for CLI/LSP/runtime launch selection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEntrySpec {
    pub id: EntryRuntimeId,
    pub kind: RuntimeEntryKind,
    /// Stable semantic identity of the complete checked entry binding.
    ///
    /// This exists for every entry, including non-stateful entries whose role
    /// variant is [`RuntimeEntryRoles::None`].
    pub binding: EntryBindingIdentity,
    pub target: RuntimeEntryTarget,
    pub roles: RuntimeEntryRoles,
}

impl EntryRuntimeId {
    pub fn canonical(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Entry, value).map(|path| Self { path })
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_source_entity_body(
            RuntimeIdFamily::Entry,
            value,
            RuntimeIdFamily::Entry.source_families(),
        )
        .map(|path| Self { path })
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }

    #[must_use]
    pub fn canonical_label(&self) -> String {
        self.path.label()
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::for_family(RuntimeIdFamily::Entry, &self.path)
    }
}

impl fmt::Display for EntryRuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}

impl RuntimeEntryKind {
    /// Canonical source/manifest spelling for this entry kind.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Game => "game",
            Self::Editor => "editor",
            Self::Cli => "cli",
            Self::Server => "server",
            Self::Activity => "activity",
            Self::Test => "test",
            Self::Bench => "bench",
            Self::Agent => "agent",
            Self::Custom(value) => value,
        }
    }

    #[must_use]
    pub const fn is_stateful(&self) -> bool {
        matches!(self, Self::Game | Self::Editor | Self::Test)
    }

    #[must_use]
    pub const fn is_agent(&self) -> bool {
        matches!(self, Self::Agent)
    }

    /// Stable discriminator used by entry-binding encoders.
    #[must_use]
    pub const fn canonical_tag(&self) -> u8 {
        match self {
            Self::Game => 1,
            Self::Editor => 2,
            Self::Test => 3,
            Self::Agent => 4,
            Self::Cli => 5,
            Self::Server => 6,
            Self::Activity => 7,
            Self::Bench => 8,
            Self::Custom(_) => u8::MAX,
        }
    }

    /// Payload carried only by a custom entry kind.
    #[must_use]
    pub fn custom_payload(&self) -> Option<&str> {
        match self {
            Self::Custom(value) => Some(value),
            Self::Game
            | Self::Editor
            | Self::Cli
            | Self::Server
            | Self::Activity
            | Self::Test
            | Self::Bench
            | Self::Agent => None,
        }
    }
}

impl RuntimeEntryTarget {
    #[must_use]
    pub const fn flow(&self) -> Option<&FlowRuntimeId> {
        match self {
            Self::Flow(flow) | Self::Controller(flow) => Some(flow),
            Self::Routes(_) => None,
        }
    }

    #[must_use]
    pub const fn is_controller(&self) -> bool {
        matches!(self, Self::Controller(_))
    }
}

impl FlowRuntimeId {
    /// Creates one generated controller flow for an exact ordinary callable.
    ///
    /// Multiple Agent entries bound to the same callable intentionally share
    /// this code identity while retaining distinct entry bindings and policy.
    #[must_use]
    pub fn for_agent_controller_callable(callable: &RuntimeCallableId) -> Self {
        Self {
            path: RuntimeIdPath::for_agent_controller_callable(callable.as_str()),
        }
    }
}

/// Failure to validate one complete executable runtime-plan inventory.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimePlanError {
    #[error("duplicate runtime flow `{0}`")]
    DuplicateFlow(String),
    #[error("duplicate runtime entry `{0}`")]
    DuplicateEntry(String),
    #[error("duplicate runtime pure-helper slot {0}")]
    DuplicatePureHelper(usize),
    #[error("duplicate role-callable executable metadata for `{0}`")]
    DuplicateCallableExecutable(String),
    #[error("duplicate role-flow executable metadata for `{0}`")]
    DuplicateFlowExecutable(String),
    #[error("role-callable `{callable}` maps to missing pure-helper slot {helper}")]
    MissingCallableHelper { callable: String, helper: usize },
    #[error("role-callable `{callable}` maps to missing controller flow `{flow}`")]
    MissingCallableControllerFlow { callable: String, flow: String },
    #[error("role-flow executable metadata references missing flow `{0}`")]
    MissingExecutableFlow(String),
    #[error("entry `{entry}` targets missing flow `{flow}`")]
    MissingEntryTarget { entry: String, flow: String },
    #[error("entry `{entry}` has incompatible kind `{kind}`, target, and roles")]
    IncompatibleEntryRoles { entry: String, kind: String },
    #[error("entry `{entry}` top-level binding does not match its role binding")]
    EntryBindingMismatch { entry: String },
    #[error("entry `{entry}` state role layout does not match its runtime schema")]
    StateLayoutMismatch { entry: String },
    #[error("entry `{entry}` event role layout does not match its runtime schema")]
    EventLayoutMismatch { entry: String },
    #[error("entry `{entry}` initial flow has an invalid state-parameter binding")]
    InvalidInitialFlowStateParameter { entry: String },
    #[error("entry `{entry}` initial flow executable contract does not match its checked role")]
    InitialFlowContractMismatch { entry: String },
    #[error("entry `{entry}` has invalid durable-root execution limits")]
    InvalidRootExecutionLimits { entry: String },
    #[error("entry `{entry}` Agent controller flow does not match its checked controller role")]
    AgentControllerFlowMismatch { entry: String },
    #[error(
        "entry `{entry}` contains duplicate command constructor `{constructor}` for target `{target}`"
    )]
    DuplicateCommandContract {
        entry: String,
        constructor: String,
        target: String,
    },
    #[error(
        "entry `{entry}` command constructor `{constructor}` payload layout does not match its runtime schema"
    )]
    CommandLayoutMismatch { entry: String, constructor: String },
    #[error("entry `{entry}` role `{role}` has an invalid runtime schema: {message}")]
    InvalidRoleSchema {
        entry: String,
        role: &'static str,
        message: String,
    },
    #[error("entry `{entry}` role `{role}` references missing executable callable `{callable}`")]
    MissingCallable {
        entry: String,
        role: &'static str,
        callable: String,
    },
    #[error(
        "entry `{entry}` role `{role}` callable `{callable}` executable contract does not match"
    )]
    CallableContractMismatch {
        entry: String,
        role: &'static str,
        callable: String,
    },
    #[error("role-callable executable `{0}` is not reachable from an entry role")]
    UnreachableCallableExecutable(String),
    #[error("role-flow executable `{0}` is not reachable from an entry role")]
    UnreachableFlowExecutable(String),
    #[error("entry `{entry}` route targets missing flow `{flow}`")]
    MissingRouteTarget { entry: String, flow: String },
}

impl RuntimePlan {
    #[must_use]
    pub fn with_entries(mut self, entries: Vec<RuntimeEntrySpec>) -> Self {
        self.entries = entries;
        self
    }

    #[must_use]
    pub fn with_entry_executables(
        mut self,
        callables: Vec<RuntimeCallableExecutable>,
        flows: Vec<RuntimeFlowExecutable>,
    ) -> Self {
        self.callable_executables = callables;
        self.flow_executables = flows;
        self
    }

    /// Verifies the complete executable entry inventory before selection.
    pub fn verify(&self) -> Result<(), RuntimePlanError> {
        let mut flow_ids = BTreeSet::new();
        for flow in &self.flows {
            if !flow_ids.insert(flow.id.clone()) {
                return Err(RuntimePlanError::DuplicateFlow(flow.id.canonical_label()));
            }
        }
        let mut helper_ids = BTreeSet::new();
        for helper in &self.pure_helpers {
            if !helper_ids.insert(helper.id) {
                return Err(RuntimePlanError::DuplicatePureHelper(helper.id.0));
            }
        }
        let mut callable_ids = BTreeSet::new();
        for executable in &self.callable_executables {
            if !callable_ids.insert(executable.callable.clone()) {
                return Err(RuntimePlanError::DuplicateCallableExecutable(
                    executable.callable.as_str().to_owned(),
                ));
            }
            match &executable.code {
                RuntimeCallableExecutableCode::PureHelper(helper)
                    if !helper_ids.contains(helper) =>
                {
                    return Err(RuntimePlanError::MissingCallableHelper {
                        callable: executable.callable.as_str().to_owned(),
                        helper: helper.0,
                    });
                }
                RuntimeCallableExecutableCode::ControllerFlow(flow) if !flow_ids.contains(flow) => {
                    return Err(RuntimePlanError::MissingCallableControllerFlow {
                        callable: executable.callable.as_str().to_owned(),
                        flow: flow.canonical_label(),
                    });
                }
                RuntimeCallableExecutableCode::PureHelper(_)
                | RuntimeCallableExecutableCode::ControllerFlow(_) => {}
            }
        }
        let mut executable_flow_ids = BTreeSet::new();
        for executable in &self.flow_executables {
            if !executable_flow_ids.insert(executable.flow.clone()) {
                return Err(RuntimePlanError::DuplicateFlowExecutable(
                    executable.flow.canonical_label(),
                ));
            }
            if !flow_ids.contains(&executable.flow) {
                return Err(RuntimePlanError::MissingExecutableFlow(
                    executable.flow.canonical_label(),
                ));
            }
        }
        let mut entry_ids = BTreeSet::new();
        for entry in &self.entries {
            if !entry_ids.insert(entry.id.clone()) {
                return Err(RuntimePlanError::DuplicateEntry(entry.id.canonical_label()));
            }
            self.verify_entry(entry, &flow_ids)?;
        }
        for executable in &self.callable_executables {
            if !self.entries.iter().any(|entry| {
                entry
                    .roles
                    .references_callable(&executable.callable, executable.contract)
            }) {
                return Err(RuntimePlanError::UnreachableCallableExecutable(
                    executable.callable.as_str().to_owned(),
                ));
            }
        }
        for executable in &self.flow_executables {
            if !self
                .entries
                .iter()
                .any(|entry| entry.references_role_flow(&executable.flow))
            {
                return Err(RuntimePlanError::UnreachableFlowExecutable(
                    executable.flow.canonical_label(),
                ));
            }
        }
        Ok(())
    }

    fn verify_entry(
        &self,
        entry: &RuntimeEntrySpec,
        flow_ids: &BTreeSet<FlowRuntimeId>,
    ) -> Result<(), RuntimePlanError> {
        let entry_label = entry.id.canonical_label();
        match &entry.target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => {
                if !flow_ids.contains(flow) {
                    return Err(RuntimePlanError::MissingEntryTarget {
                        entry: entry_label,
                        flow: flow.canonical_label(),
                    });
                }
            }
            RuntimeEntryTarget::Routes(routes) => {
                for route in routes {
                    if !flow_ids.contains(&route.target) {
                        return Err(RuntimePlanError::MissingRouteTarget {
                            entry: entry_label,
                            flow: route.target.canonical_label(),
                        });
                    }
                }
            }
        }
        if entry
            .roles
            .binding()
            .is_some_and(|role_binding| role_binding != entry.binding)
        {
            return Err(RuntimePlanError::EntryBindingMismatch {
                entry: entry.id.canonical_label(),
            });
        }

        match (&entry.kind, &entry.target, &entry.roles) {
            (
                RuntimeEntryKind::Game | RuntimeEntryKind::Editor | RuntimeEntryKind::Test,
                RuntimeEntryTarget::Flow(target),
                RuntimeEntryRoles::Stateful(roles),
            ) if target == &roles.initial_flow.flow => self.verify_stateful_entry(&entry.id, roles),
            (
                RuntimeEntryKind::Agent,
                RuntimeEntryTarget::Controller(flow),
                RuntimeEntryRoles::Agent(roles),
            ) => {
                let callable = self.verify_callable(&entry.id, "controller", &roles.controller)?;
                if callable.code != RuntimeCallableExecutableCode::ControllerFlow(flow.clone()) {
                    return Err(RuntimePlanError::AgentControllerFlowMismatch {
                        entry: entry.id.canonical_label(),
                    });
                }
                let Some(executable) = self
                    .flow_executables
                    .iter()
                    .find(|executable| executable.flow == *flow)
                else {
                    return Err(RuntimePlanError::AgentControllerFlowMismatch {
                        entry: entry.id.canonical_label(),
                    });
                };
                if executable.controller.as_ref() != Some(&roles.controller) {
                    return Err(RuntimePlanError::AgentControllerFlowMismatch {
                        entry: entry.id.canonical_label(),
                    });
                }
                Ok(())
            }
            (
                RuntimeEntryKind::Cli
                | RuntimeEntryKind::Server
                | RuntimeEntryKind::Activity
                | RuntimeEntryKind::Bench
                | RuntimeEntryKind::Custom(_),
                RuntimeEntryTarget::Flow(_) | RuntimeEntryTarget::Routes(_),
                RuntimeEntryRoles::None,
            ) => Ok(()),
            _ => Err(RuntimePlanError::IncompatibleEntryRoles {
                entry: entry.id.canonical_label(),
                kind: entry.kind.as_str().to_owned(),
            }),
        }
    }

    fn verify_stateful_entry(
        &self,
        entry: &EntryRuntimeId,
        roles: &RuntimeStatefulEntryRoles,
    ) -> Result<(), RuntimePlanError> {
        if !roles.command_policy.root_limits.is_valid() {
            return Err(RuntimePlanError::InvalidRootExecutionLimits {
                entry: entry.canonical_label(),
            });
        }
        let state_layout = roles.state.schema.try_layout_hash().map_err(|error| {
            RuntimePlanError::InvalidRoleSchema {
                entry: entry.canonical_label(),
                role: "state",
                message: error.to_string(),
            }
        })?;
        if roles.state.layout != state_layout {
            return Err(RuntimePlanError::StateLayoutMismatch {
                entry: entry.canonical_label(),
            });
        }
        let event_layout = roles.event.schema.try_layout_hash().map_err(|error| {
            RuntimePlanError::InvalidRoleSchema {
                entry: entry.canonical_label(),
                role: "event",
                message: error.to_string(),
            }
        })?;
        if roles.event.layout != event_layout {
            return Err(RuntimePlanError::EventLayoutMismatch {
                entry: entry.canonical_label(),
            });
        }
        let Some(flow) = self
            .flow_executables
            .iter()
            .find(|flow| flow.flow == roles.initial_flow.flow)
        else {
            return Err(RuntimePlanError::InitialFlowContractMismatch {
                entry: entry.canonical_label(),
            });
        };
        if flow.contract != roles.initial_flow.contract {
            return Err(RuntimePlanError::InitialFlowContractMismatch {
                entry: entry.canonical_label(),
            });
        }
        let [state_parameter] = flow.parameters.as_slice() else {
            return Err(RuntimePlanError::InvalidInitialFlowStateParameter {
                entry: entry.canonical_label(),
            });
        };
        if state_parameter.position != 0
            || state_parameter.name.is_empty()
            || state_parameter.name.chars().any(char::is_control)
            || state_parameter.mode != RuntimeFlowParameterMode::Owned
            || state_parameter.nominal != roles.state.identity
            || state_parameter.layout != roles.state.layout
        {
            return Err(RuntimePlanError::InvalidInitialFlowStateParameter {
                entry: entry.canonical_label(),
            });
        }
        let mut command_contracts = BTreeSet::new();
        for command in &roles.command_policy.admitted {
            let key = (
                command.constructor.as_str().to_owned(),
                command.target.as_str().to_owned(),
            );
            if !command_contracts.insert(key) {
                return Err(RuntimePlanError::DuplicateCommandContract {
                    entry: entry.canonical_label(),
                    constructor: command.constructor.as_str().to_owned(),
                    target: command.target.as_str().to_owned(),
                });
            }
            let layout = command.payload_schema.try_layout_hash().map_err(|error| {
                RuntimePlanError::InvalidRoleSchema {
                    entry: entry.canonical_label(),
                    role: "command payload",
                    message: error.to_string(),
                }
            })?;
            if layout != command.payload_layout {
                return Err(RuntimePlanError::CommandLayoutMismatch {
                    entry: entry.canonical_label(),
                    constructor: command.constructor.as_str().to_owned(),
                });
            }
        }
        let initializer = self.verify_callable(entry, "initializer", &roles.initializer)?;
        let reducer = self.verify_callable(entry, "reducer", &roles.reducer)?;
        if !matches!(
            initializer.code,
            RuntimeCallableExecutableCode::PureHelper(_)
        ) || !matches!(reducer.code, RuntimeCallableExecutableCode::PureHelper(_))
        {
            return Err(RuntimePlanError::IncompatibleEntryRoles {
                entry: entry.canonical_label(),
                kind: "stateful callable code".to_owned(),
            });
        }
        Ok(())
    }

    fn verify_callable(
        &self,
        entry: &EntryRuntimeId,
        role_name: &'static str,
        role: &RuntimeCallableRole,
    ) -> Result<&RuntimeCallableExecutable, RuntimePlanError> {
        let matches = self
            .callable_executables
            .iter()
            .filter(|executable| executable.callable == role.callable)
            .collect::<Vec<_>>();
        let [executable] = matches.as_slice() else {
            return Err(RuntimePlanError::MissingCallable {
                entry: entry.canonical_label(),
                role: role_name,
                callable: role.callable.as_str().to_owned(),
            });
        };
        if executable.contract != role.contract {
            return Err(RuntimePlanError::CallableContractMismatch {
                entry: entry.canonical_label(),
                role: role_name,
                callable: role.callable.as_str().to_owned(),
            });
        }
        Ok(executable)
    }
}

impl RuntimeEntryRoles {
    fn references_callable(
        &self,
        callable: &RuntimeCallableId,
        contract: CallableContractHash,
    ) -> bool {
        match self {
            Self::Stateful(roles) => {
                role_matches(&roles.initializer, callable, contract)
                    || role_matches(&roles.reducer, callable, contract)
            }
            Self::Agent(roles) => role_matches(&roles.controller, callable, contract),
            Self::None => false,
        }
    }
}

impl RuntimeEntrySpec {
    fn references_role_flow(&self, flow: &FlowRuntimeId) -> bool {
        match &self.roles {
            RuntimeEntryRoles::Stateful(roles) => roles.initial_flow.flow == *flow,
            RuntimeEntryRoles::Agent(_) => {
                matches!(&self.target, RuntimeEntryTarget::Controller(target) if target == flow)
            }
            RuntimeEntryRoles::None => false,
        }
    }
}

fn role_matches(
    role: &RuntimeCallableRole,
    callable: &RuntimeCallableId,
    contract: CallableContractHash,
) -> bool {
    role.callable == *callable && role.contract == contract
}
