//! Runtime entry inventory records and whole-plan verification.
//!
//! The data-only role contracts remain owned by [`crate::entry`]. This module
//! composes those contracts with runtime-plan identifiers, launch targets, and
//! executable metadata, then verifies the complete entry graph before runtime
//! selection. It performs no source resolution, adapter work, or platform I/O.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::entry::{
    CallableContractHash, EntryBindingIdentity, FlowParameterCoordinate, RuntimeCallableExecutable,
    RuntimeCallableExecutableCode, RuntimeCallableId, RuntimeCallableRole, RuntimeCommandContract,
    RuntimeEntryRoles, RuntimeFlowParameterMode, RuntimeStatefulEntryRoles,
};
use crate::runtime_id::{RuntimeIdError, RuntimeIdFamily, RuntimeIdPath, RuntimePublicLabel};
use crate::value::RuntimeFlowParameterBinding;

use super::{FlowRuntimeId, RuntimePlan, RuntimePlanValueTypeError, RuntimePureHelperId};

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

/// Closed HTTP method family admitted by the runtime route registry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl RuntimeHttpMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

/// Stable source-order coordinate of one route-path capture.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RouteCaptureCoordinate(u32);

impl RouteCaptureCoordinate {
    #[must_use]
    pub const fn from_position(position: u32) -> Self {
        Self(position)
    }

    pub fn try_from_index(index: usize) -> Result<Self, RuntimeRoutePathError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| RuntimeRoutePathError::CaptureCapacity { index })
    }

    #[must_use]
    pub const fn position(self) -> u32 {
        self.0
    }

    pub fn index(self) -> Result<usize, RuntimeRoutePathError> {
        usize::try_from(self.0)
            .map_err(|_| RuntimeRoutePathError::CaptureCoordinate { position: self.0 })
    }
}

/// One canonical route-matching segment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeRoutePathSegment {
    Literal(String),
    Capture(RouteCaptureCoordinate),
}

/// Canonical path plus its typed dispatch/capture inventory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeRoutePath {
    segments: Vec<RuntimeRoutePathSegment>,
}

impl RuntimeRoutePath {
    pub fn try_new(
        segments: impl Into<Vec<RuntimeRoutePathSegment>>,
    ) -> Result<Self, RuntimeRoutePathError> {
        let segments = segments.into();
        let mut capture_count = 0usize;
        for segment in &segments {
            match segment {
                RuntimeRoutePathSegment::Literal(literal) => {
                    if literal.is_empty()
                        || literal.starts_with(':')
                        || literal.contains('/')
                        || literal.chars().any(char::is_control)
                    {
                        return Err(RuntimeRoutePathError::InvalidLiteral);
                    }
                }
                RuntimeRoutePathSegment::Capture(coordinate) => {
                    if coordinate.index()? != capture_count {
                        return Err(RuntimeRoutePathError::InvalidCapture);
                    }
                    capture_count += 1;
                }
            }
        }
        Ok(Self { segments })
    }

    #[must_use]
    pub fn segments(&self) -> &[RuntimeRoutePathSegment] {
        &self.segments
    }

    #[must_use]
    pub fn capture_count(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| matches!(segment, RuntimeRoutePathSegment::Capture(_)))
            .count()
    }

    #[must_use]
    pub fn overlaps_dispatch(&self, other: &Self) -> bool {
        self.segments.len() == other.segments.len()
            && self
                .segments
                .iter()
                .zip(other.segments.iter())
                .all(|(left, right)| match (left, right) {
                    (
                        RuntimeRoutePathSegment::Literal(left),
                        RuntimeRoutePathSegment::Literal(right),
                    ) => left == right,
                    (
                        RuntimeRoutePathSegment::Literal(_)
                        | RuntimeRoutePathSegment::Capture(_),
                        RuntimeRoutePathSegment::Capture(_),
                    )
                    | (
                        RuntimeRoutePathSegment::Capture(_),
                        RuntimeRoutePathSegment::Literal(_),
                    ) => {
                        true
                    }
                })
    }

    #[must_use]
    pub fn dispatch_cmp(&self, other: &Self) -> Ordering {
        for (left, right) in self.segments.iter().zip(other.segments.iter()) {
            let ordering = match (left, right) {
                (
                    RuntimeRoutePathSegment::Literal(left),
                    RuntimeRoutePathSegment::Literal(right),
                ) => left.cmp(right),
                (RuntimeRoutePathSegment::Literal(_), RuntimeRoutePathSegment::Capture(_)) => {
                    Ordering::Less
                }
                (RuntimeRoutePathSegment::Capture(_), RuntimeRoutePathSegment::Literal(_)) => {
                    Ordering::Greater
                }
                (RuntimeRoutePathSegment::Capture(_), RuntimeRoutePathSegment::Capture(_)) => {
                    Ordering::Equal
                }
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        self.segments.len().cmp(&other.segments.len())
    }

    fn validate(&self) -> Result<(), RuntimeRoutePathError> {
        (Self::try_new(self.segments.clone())? == *self)
            .then_some(())
            .ok_or(RuntimeRoutePathError::NonCanonical)
    }
}

impl fmt::Display for RuntimeRoutePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.segments.is_empty() {
            return f.write_str("/");
        }
        for segment in &self.segments {
            f.write_str("/")?;
            match segment {
                RuntimeRoutePathSegment::Literal(literal) => f.write_str(literal)?,
                RuntimeRoutePathSegment::Capture(_) => f.write_str(":")?,
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeRoutePathError {
    #[error("route literal segment is not canonical")]
    InvalidLiteral,
    #[error("route capture inventory is not canonical")]
    InvalidCapture,
    #[error("route capture index {index} exceeds the coordinate domain")]
    CaptureCapacity { index: usize },
    #[error("route capture coordinate {position} does not fit this platform")]
    CaptureCoordinate { position: u32 },
    #[error("route path payload differs from its canonical segment inventory")]
    NonCanonical,
}

/// Route declaration in a server-like entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRouteSpec {
    pub method: RuntimeHttpMethod,
    pub path: RuntimeRoutePath,
    pub target: FlowRuntimeId,
    pub bindings: Vec<RuntimeRouteBinding>,
}

/// Explicit route parameter binding for a target flow invocation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRouteBinding {
    pub parameter: FlowParameterCoordinate,
    pub source: RuntimeRouteBindingSource,
}

/// Adapter route value source used by a route binding.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeRouteBindingSource {
    PathCapture(RouteCaptureCoordinate),
}

/// One complete, plan-validated Flow invocation consumed before the first
/// operation executes.
///
/// The carrier owns its exact immutable plan so a coordinate/value inventory
/// cannot be paired with a different runtime generation after sealing.
#[derive(Debug, PartialEq)]
pub struct RuntimeFlowInvocation {
    plan: Arc<RuntimePlan>,
    flow: FlowRuntimeId,
    bindings: Box<[RuntimeFlowParameterBinding]>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeFlowInvocationError {
    #[error("runtime Flow `{flow}` does not exist")]
    UnknownFlow { flow: String },
    #[error("runtime Flow `{flow}` has no invocation schema")]
    MissingSchema { flow: String },
    #[error("runtime Flow `{flow}` expects {expected} invocation bindings, received {actual}")]
    BindingCount {
        flow: String,
        expected: usize,
        actual: usize,
    },
    #[error("runtime Flow `{flow}` received duplicate parameter coordinate {parameter:?}")]
    DuplicateParameter {
        flow: String,
        parameter: FlowParameterCoordinate,
    },
    #[error("runtime Flow `{flow}` has no parameter coordinate {parameter:?}")]
    UnknownParameter {
        flow: String,
        parameter: FlowParameterCoordinate,
    },
    #[error(
        "runtime Flow `{flow}` invocation row {index} carries noncanonical parameter {parameter:?}"
    )]
    NonCanonicalParameter {
        flow: String,
        index: usize,
        parameter: FlowParameterCoordinate,
    },
    #[error("runtime Flow `{flow}` parameter {parameter:?} has no plan-local declaration")]
    MissingParameterLocal {
        flow: String,
        parameter: FlowParameterCoordinate,
    },
    #[error("runtime Flow `{flow}` parameter {parameter:?} does not match its checked type")]
    ParameterType {
        flow: String,
        parameter: FlowParameterCoordinate,
    },
    #[error(transparent)]
    ValueType(#[from] RuntimePlanValueTypeError),
}

impl RuntimePlan {
    /// Consumes this immutable plan and seals a complete coordinate-addressed
    /// invocation for one exact Flow.
    pub fn seal_flow_invocation(
        self,
        flow: FlowRuntimeId,
        bindings: impl IntoIterator<Item = RuntimeFlowParameterBinding>,
    ) -> Result<RuntimeFlowInvocation, RuntimeFlowInvocationError> {
        let flow_label = flow.canonical_label();
        let runtime_flow = self
            .flows
            .iter()
            .find(|candidate| candidate.id == flow)
            .ok_or_else(|| RuntimeFlowInvocationError::UnknownFlow {
                flow: flow_label.clone(),
            })?;
        let schema = self
            .flow_schemas
            .iter()
            .find(|candidate| candidate.flow == flow)
            .ok_or_else(|| RuntimeFlowInvocationError::MissingSchema {
                flow: flow_label.clone(),
            })?;
        let bindings = bindings.into_iter().collect::<Vec<_>>();
        if bindings.len() != schema.parameters.len() {
            return Err(RuntimeFlowInvocationError::BindingCount {
                flow: flow_label,
                expected: schema.parameters.len(),
                actual: bindings.len(),
            });
        }
        let mut unique = BTreeSet::new();
        for (index, binding) in bindings.iter().enumerate() {
            if !unique.insert(binding.parameter) {
                return Err(RuntimeFlowInvocationError::DuplicateParameter {
                    flow: flow_label,
                    parameter: binding.parameter,
                });
            }
            if binding.parameter.index().ok() != Some(index) {
                return Err(RuntimeFlowInvocationError::NonCanonicalParameter {
                    flow: flow_label,
                    index,
                    parameter: binding.parameter,
                });
            }
            let parameter = schema
                .parameters
                .get(index)
                .filter(|parameter| parameter.coordinate == binding.parameter)
                .ok_or_else(|| RuntimeFlowInvocationError::UnknownParameter {
                    flow: flow_label.clone(),
                    parameter: binding.parameter,
                })?;
            let local = runtime_flow.params.get(index).copied().ok_or_else(|| {
                RuntimeFlowInvocationError::MissingParameterLocal {
                    flow: flow_label.clone(),
                    parameter: parameter.coordinate,
                }
            })?;
            let declaration = self.local_declarations.get(local).ok_or_else(|| {
                RuntimeFlowInvocationError::MissingParameterLocal {
                    flow: flow_label.clone(),
                    parameter: parameter.coordinate,
                }
            })?;
            if !self.value_matches_type(declaration.ty(), &binding.value)? {
                return Err(RuntimeFlowInvocationError::ParameterType {
                    flow: flow_label,
                    parameter: parameter.coordinate,
                });
            }
        }
        Ok(RuntimeFlowInvocation {
            plan: Arc::new(self),
            flow,
            bindings: bindings.into_boxed_slice(),
        })
    }
}

impl RuntimeFlowInvocation {
    /// Borrows the exact immutable plan pinned by this affine invocation.
    pub const fn plan(&self) -> &Arc<RuntimePlan> {
        &self.plan
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        RuntimePlan,
        FlowRuntimeId,
        Box<[RuntimeFlowParameterBinding]>,
    ) {
        (Arc::unwrap_or_clone(self.plan), self.flow, self.bindings)
    }
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
        Self::from_runtime_path(RuntimeIdPath::for_agent_controller_callable(
            callable.as_str(),
        ))
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
    #[error("duplicate runtime Flow invocation schema for `{0}`")]
    DuplicateFlowSchema(String),
    #[error("runtime Flow invocation schema references missing flow `{0}`")]
    MissingSchemaFlow(String),
    #[error("runtime Flow `{0}` has no invocation schema")]
    MissingFlowSchema(String),
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
    #[error("runtime pure program {program} is bound more than once")]
    DuplicatePureProgram {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    },
    #[error("runtime pure program {program} references missing helper {helper}")]
    MissingPureProgramHelper {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
        helper: usize,
    },
    #[error("runtime pure program {program} semantic signature does not match helper {helper}")]
    PureProgramSignatureMismatch {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
        helper: usize,
    },
    #[error("role-flow executable `{0}` is not reachable from an entry role")]
    UnreachableFlowExecutable(String),
    #[error("entry `{entry}` route targets missing flow `{flow}`")]
    MissingRouteTarget { entry: String, flow: String },
    #[error("entry `{entry}` route set is empty")]
    EmptyRouteSet { entry: String },
    #[error("entry `{entry}` contains a non-canonical route path: {reason}")]
    InvalidRoutePath { entry: String, reason: String },
    #[error("entry `{entry}` contains overlapping or non-canonically ordered routes")]
    InvalidRouteOrder { entry: String },
    #[error("entry `{entry}` target flow `{flow}` has no executable parameter schema")]
    MissingEntryFlowExecutable { entry: String, flow: String },
    #[error("entry `{entry}` route to `{flow}` has an invalid closed binding plan")]
    InvalidRouteBindings { entry: String, flow: String },
    #[error("entry `{entry}` direct target `{flow}` requires parameters")]
    ParameterizedDirectEntryTarget { entry: String, flow: String },
}

impl RuntimePlan {
    /// Verifies the complete executable entry inventory before selection.
    pub fn verify(&self) -> Result<(), RuntimePlanError> {
        let flow_ids = self.verify_flow_schemas()?;
        let mut helper_ids = BTreeSet::new();
        for helper in &self.pure_helpers {
            if !helper_ids.insert(helper.id) {
                return Err(RuntimePlanError::DuplicatePureHelper(helper.id.0));
            }
        }
        self.verify_pure_programs(&helper_ids)?;
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
                .any(|entry| entry.references_flow(&executable.flow))
            {
                return Err(RuntimePlanError::UnreachableFlowExecutable(
                    executable.flow.canonical_label(),
                ));
            }
        }
        Ok(())
    }

    fn verify_flow_schemas(&self) -> Result<BTreeSet<FlowRuntimeId>, RuntimePlanError> {
        let mut flow_ids = BTreeSet::new();
        for flow in &self.flows {
            if !flow_ids.insert(flow.id.clone()) {
                return Err(RuntimePlanError::DuplicateFlow(flow.id.canonical_label()));
            }
        }
        let mut schema_flow_ids = BTreeSet::new();
        for schema in &self.flow_schemas {
            if !schema_flow_ids.insert(schema.flow.clone()) {
                return Err(RuntimePlanError::DuplicateFlowSchema(
                    schema.flow.canonical_label(),
                ));
            }
            if !flow_ids.contains(&schema.flow) {
                return Err(RuntimePlanError::MissingSchemaFlow(
                    schema.flow.canonical_label(),
                ));
            }
        }
        if let Some(flow) = flow_ids
            .iter()
            .find(|flow| !schema_flow_ids.contains(*flow))
        {
            return Err(RuntimePlanError::MissingFlowSchema(flow.canonical_label()));
        }
        Ok(flow_ids)
    }

    fn verify_pure_programs(
        &self,
        helper_ids: &BTreeSet<RuntimePureHelperId>,
    ) -> Result<(), RuntimePlanError> {
        let mut pure_programs = BTreeSet::new();
        for binding in &self.pure_programs {
            if !pure_programs.insert(binding.program()) {
                return Err(RuntimePlanError::DuplicatePureProgram {
                    program: binding.program(),
                });
            }
            if !helper_ids.contains(&binding.helper()) {
                return Err(RuntimePlanError::MissingPureProgramHelper {
                    program: binding.program(),
                    helper: binding.helper().0,
                });
            }
            let Some(helper) = self
                .pure_helpers
                .iter()
                .find(|helper| helper.id == binding.helper())
            else {
                return Err(RuntimePlanError::MissingPureProgramHelper {
                    program: binding.program(),
                    helper: binding.helper().0,
                });
            };
            let input_types = helper
                .input_locals
                .iter()
                .map(|local| {
                    self.local_declarations
                        .get(*local)
                        .and_then(|declaration| self.type_table.get(declaration.ty()))
                        .map(super::type_table::RuntimePlanTypeDeclaration::semantic_identity)
                })
                .collect::<Option<Vec<_>>>();
            let result_type = self
                .type_table
                .get(helper.expr.ty())
                .map(super::type_table::RuntimePlanTypeDeclaration::semantic_identity);
            if input_types.as_deref() != Some(binding.input_types())
                || result_type != Some(binding.result_type())
            {
                return Err(RuntimePlanError::PureProgramSignatureMismatch {
                    program: binding.program(),
                    helper: binding.helper().0,
                });
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
            ) => self.verify_existing_entry_target(&entry.id, &entry.target),
            _ => Err(RuntimePlanError::IncompatibleEntryRoles {
                entry: entry.id.canonical_label(),
                kind: entry.kind.as_str().to_owned(),
            }),
        }
    }

    fn verify_existing_entry_target(
        &self,
        entry: &EntryRuntimeId,
        target: &RuntimeEntryTarget,
    ) -> Result<(), RuntimePlanError> {
        match target {
            RuntimeEntryTarget::Flow(flow) => {
                self.flow_executables
                    .iter()
                    .find(|row| row.flow == *flow)
                    .ok_or_else(|| RuntimePlanError::MissingEntryFlowExecutable {
                        entry: entry.canonical_label(),
                        flow: flow.canonical_label(),
                    })?;
                let schema = self
                    .flow_schemas
                    .iter()
                    .find(|row| row.flow == *flow)
                    .ok_or_else(|| RuntimePlanError::MissingFlowSchema(flow.canonical_label()))?;
                if !schema.parameters.is_empty() {
                    return Err(RuntimePlanError::ParameterizedDirectEntryTarget {
                        entry: entry.canonical_label(),
                        flow: flow.canonical_label(),
                    });
                }
                Ok(())
            }
            RuntimeEntryTarget::Routes(routes) => self.verify_route_entry_target(entry, routes),
            RuntimeEntryTarget::Controller(_) => Err(RuntimePlanError::IncompatibleEntryRoles {
                entry: entry.canonical_label(),
                kind: "non-stateful".to_owned(),
            }),
        }
    }

    fn verify_route_entry_target(
        &self,
        entry: &EntryRuntimeId,
        routes: &[RuntimeRouteSpec],
    ) -> Result<(), RuntimePlanError> {
        if routes.is_empty() {
            return Err(RuntimePlanError::EmptyRouteSet {
                entry: entry.canonical_label(),
            });
        }
        for (index, route) in routes.iter().enumerate() {
            route
                .path
                .validate()
                .map_err(|error| RuntimePlanError::InvalidRoutePath {
                    entry: entry.canonical_label(),
                    reason: error.to_string(),
                })?;
            if routes[..index].iter().any(|previous| {
                previous.method == route.method && previous.path.overlaps_dispatch(&route.path)
            }) || index > 0
                && routes[index - 1]
                    .method
                    .cmp(&route.method)
                    .then_with(|| routes[index - 1].path.dispatch_cmp(&route.path))
                    != Ordering::Less
            {
                return Err(RuntimePlanError::InvalidRouteOrder {
                    entry: entry.canonical_label(),
                });
            }
            self.flow_executables
                .iter()
                .find(|row| row.flow == route.target)
                .ok_or_else(|| RuntimePlanError::MissingEntryFlowExecutable {
                    entry: entry.canonical_label(),
                    flow: route.target.canonical_label(),
                })?;
            let schema = self
                .flow_schemas
                .iter()
                .find(|row| row.flow == route.target)
                .ok_or_else(|| {
                    RuntimePlanError::MissingFlowSchema(route.target.canonical_label())
                })?;
            let string_identity =
                crate::pattern::RuntimeCheckedType::String.semantic_identity_digest();
            let mut captures = BTreeSet::new();
            let parameters_valid =
                schema
                    .parameters
                    .iter()
                    .enumerate()
                    .all(|(parameter_index, parameter)| {
                        parameter.coordinate.index().ok() == Some(parameter_index)
                            && parameter.mode == RuntimeFlowParameterMode::Owned
                            && parameter.semantic_identity == string_identity
                    });
            let bindings_valid =
                route
                    .bindings
                    .iter()
                    .enumerate()
                    .all(|(parameter_index, binding)| {
                        binding.parameter.index().ok() == Some(parameter_index)
                            && match binding.source {
                                RuntimeRouteBindingSource::PathCapture(capture) => {
                                    capture.index().ok().is_some_and(|capture_index| {
                                        capture_index < route.path.capture_count()
                                            && captures.insert(capture_index)
                                    })
                                }
                            }
                    });
            if schema.parameters.len() != route.bindings.len()
                || route.path.capture_count() != route.bindings.len()
                || !parameters_valid
                || !bindings_valid
                || captures.len() != route.path.capture_count()
            {
                return Err(RuntimePlanError::InvalidRouteBindings {
                    entry: entry.canonical_label(),
                    flow: route.target.canonical_label(),
                });
            }
        }
        Ok(())
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
        let Some(schema) = self
            .flow_schemas
            .iter()
            .find(|schema| schema.flow == roles.initial_flow.flow)
        else {
            return Err(RuntimePlanError::InvalidInitialFlowStateParameter {
                entry: entry.canonical_label(),
            });
        };
        let [state_parameter] = schema.parameters.as_slice() else {
            return Err(RuntimePlanError::InvalidInitialFlowStateParameter {
                entry: entry.canonical_label(),
            });
        };
        if state_parameter.coordinate.position() != 0
            || state_parameter.name.is_empty()
            || state_parameter.name.chars().any(char::is_control)
            || state_parameter.mode != RuntimeFlowParameterMode::Owned
            || state_parameter.semantic_identity != roles.state.semantic_identity
        {
            return Err(RuntimePlanError::InvalidInitialFlowStateParameter {
                entry: entry.canonical_label(),
            });
        }
        Self::verify_stateful_command_contracts(entry, &roles.command_policy.admitted)?;
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

    fn verify_stateful_command_contracts(
        entry: &EntryRuntimeId,
        commands: &[RuntimeCommandContract],
    ) -> Result<(), RuntimePlanError> {
        let mut command_contracts = BTreeSet::new();
        for command in commands {
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
    /// Returns whether this exact checked Entry plan retains the Flow as an
    /// executable launch target.
    ///
    /// Consumers must use this owner behavior instead of reconstructing Flow
    /// reachability from Entry kind, roles, or route rows independently.
    #[must_use]
    pub fn references_flow(&self, flow: &FlowRuntimeId) -> bool {
        match &self.roles {
            RuntimeEntryRoles::Stateful(roles) => roles.initial_flow.flow == *flow,
            RuntimeEntryRoles::Agent(_) => {
                matches!(&self.target, RuntimeEntryTarget::Controller(target) if target == flow)
            }
            RuntimeEntryRoles::None => match &self.target {
                RuntimeEntryTarget::Flow(target) => target == flow,
                RuntimeEntryTarget::Routes(routes) => {
                    routes.iter().any(|route| &route.target == flow)
                }
                RuntimeEntryTarget::Controller(_) => false,
            },
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
