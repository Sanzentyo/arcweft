//! Checked source-entry bindings and deterministic semantic identities.

mod checker;
mod digest;

#[cfg(test)]
mod bind_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod tests;

use std::{collections::BTreeMap, fmt};

use arcweft_data::TypeShape;
use arcweft_id::PublicId;
use arcweft_lang_hir::symbol::{CallableDeclarationId, CallablePackageId};
use arcweft_lang_syntax::ast::{items::Attribute, module_path::CanonicalModulePath};
use arcweft_source::SourceSpan;
use thiserror::Error;

use crate::effects::{EffectId, EffectSet};

pub use checker::{CheckedEntryDiagnostic, check_project_entries};

/// Canonical public identity of one checked source entry.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedEntryId(PublicId);

/// Canonical public identity of one checked source flow.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedFlowId(PublicId);

/// Entry-binding projection of an ordinary nominal declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundNominalTypeKey {
    package: CallablePackageId,
    module: CanonicalModulePath,
    name: String,
    kind: BoundNominalKind,
}

/// Ordinary nominal declaration family accepted as a root role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundNominalKind {
    Struct,
    Enum,
}

macro_rules! digest_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

digest_type!(NominalSchemaDigest);
digest_type!(CallableContractDigest);
digest_type!(FlowContractDigest);
digest_type!(CheckedEntryBindingDigest);
digest_type!(CheckedAgentPolicyDigest);

/// Closed semantic entry kind used after syntax checking.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedEntryKind {
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

/// Stateful entry kinds with the shared root-transition contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedStatefulEntryKind {
    Game,
    Editor,
    Test,
}

/// Exact checked role binding for one source entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedEntryBinding {
    Stateful(Box<CheckedStatefulEntry>),
    Agent(Box<CheckedAgentEntry>),
    Existing(CheckedExistingEntry),
}

/// Deterministically ordered checked source-entry catalog.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedEntryCatalog {
    entries: BTreeMap<CheckedEntryId, CheckedEntryBinding>,
}

/// Checked stateful entry and every explicit role identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStatefulEntry {
    id: CheckedEntryId,
    kind: CheckedStatefulEntryKind,
    state: CheckedNominalRole,
    initializer: CheckedCallableRole,
    event: CheckedNominalRole,
    reducer: CheckedCallableRole,
    initial_flow: CheckedInitialFlowRole,
    binding_digest: CheckedEntryBindingDigest,
}

/// Checked ordinary nominal role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedNominalRole {
    key: BoundNominalTypeKey,
    schema: TypeShape,
    schema_digest: NominalSchemaDigest,
    source: SourceSpan,
}

/// Checked ordinary function role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableRole {
    declaration: CallableDeclarationId,
    contract_digest: CallableContractDigest,
    source: SourceSpan,
}

/// Checked initial source-flow role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInitialFlowRole {
    id: CheckedFlowId,
    contract_digest: FlowContractDigest,
    state_parameter_name: String,
    source: SourceSpan,
}

/// Checked Agent entry over an ordinary function declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAgentEntry {
    id: CheckedEntryId,
    controller: CheckedCallableRole,
    policy: CheckedAgentPolicy,
    budget: AgentBudget,
    policy_digest: CheckedAgentPolicyDigest,
    binding_digest: CheckedEntryBindingDigest,
}

/// Closed Agent policy selected for one checked binding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedAgentPolicy {
    allowed_effects: EffectSet,
    inferred_effects: EffectSet,
}

/// Effective hard limits included in Agent binding identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentBudget {
    logical_timeout_millis: u64,
    max_vm_steps: u64,
    max_host_calls: u32,
    max_observations: u32,
    max_captures: u32,
    max_capture_bytes: u64,
    max_rag_queries: u32,
    max_context_bytes: u64,
}

/// Invalid `#[budget(...)]` policy on an Agent-bound ordinary function.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct AgentBudgetError {
    message: String,
}

/// Checked non-stateful entry retained by the existing launch model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExistingEntry {
    id: CheckedEntryId,
    kind: CheckedEntryKind,
    binding_digest: CheckedEntryBindingDigest,
}

/// Duplicate canonical entry identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("duplicate checked entry ID `{id}`")]
pub struct DuplicateCheckedEntryId {
    id: CheckedEntryId,
}

impl CheckedEntryId {
    fn try_new(value: impl Into<String>) -> Result<Self, arcweft_id::IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }
}

impl CheckedFlowId {
    fn try_new(value: impl Into<String>) -> Result<Self, arcweft_id::IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }
}

impl fmt::Display for CheckedEntryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl BoundNominalTypeKey {
    pub(crate) fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        name: impl Into<String>,
        kind: BoundNominalKind,
    ) -> Self {
        Self {
            package,
            module,
            name: name.into(),
            kind,
        }
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn kind(&self) -> BoundNominalKind {
        self.kind
    }
}

impl CheckedEntryKind {
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

    pub(crate) const fn canonical_tag(&self) -> u8 {
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

    pub(crate) fn custom_payload(&self) -> Option<&str> {
        match self {
            Self::Custom(value) => Some(value),
            _ => None,
        }
    }
}

impl CheckedStatefulEntryKind {
    pub const fn as_checked(self) -> CheckedEntryKind {
        match self {
            Self::Game => CheckedEntryKind::Game,
            Self::Editor => CheckedEntryKind::Editor,
            Self::Test => CheckedEntryKind::Test,
        }
    }
}

impl CheckedEntryCatalog {
    pub fn try_new(
        entries: impl IntoIterator<Item = CheckedEntryBinding>,
    ) -> Result<Self, DuplicateCheckedEntryId> {
        let mut catalog = BTreeMap::new();
        for entry in entries {
            let id = entry.id().clone();
            if catalog.insert(id.clone(), entry).is_some() {
                return Err(DuplicateCheckedEntryId { id });
            }
        }
        Ok(Self { entries: catalog })
    }

    pub fn get(&self, id: &CheckedEntryId) -> Option<&CheckedEntryBinding> {
        self.entries.get(id)
    }

    pub fn get_public(&self, id: &PublicId) -> Option<&CheckedEntryBinding> {
        self.entries
            .iter()
            .find_map(|(candidate, entry)| (candidate.public_id() == id).then_some(entry))
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = &CheckedEntryBinding> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl CheckedEntryBinding {
    pub const fn id(&self) -> &CheckedEntryId {
        match self {
            Self::Stateful(entry) => &entry.id,
            Self::Agent(entry) => &entry.id,
            Self::Existing(entry) => &entry.id,
        }
    }

    pub fn kind(&self) -> CheckedEntryKind {
        match self {
            Self::Stateful(entry) => entry.kind.as_checked(),
            Self::Agent(_) => CheckedEntryKind::Agent,
            Self::Existing(entry) => entry.kind.clone(),
        }
    }

    pub const fn binding_digest(&self) -> &CheckedEntryBindingDigest {
        match self {
            Self::Stateful(entry) => &entry.binding_digest,
            Self::Agent(entry) => &entry.binding_digest,
            Self::Existing(entry) => &entry.binding_digest,
        }
    }

    pub const fn stateful(&self) -> Option<&CheckedStatefulEntry> {
        match self {
            Self::Stateful(entry) => Some(entry),
            Self::Agent(_) | Self::Existing(_) => None,
        }
    }

    pub const fn agent(&self) -> Option<&CheckedAgentEntry> {
        match self {
            Self::Agent(entry) => Some(entry),
            Self::Stateful(_) | Self::Existing(_) => None,
        }
    }
}

impl CheckedStatefulEntry {
    pub const fn id(&self) -> &CheckedEntryId {
        &self.id
    }

    pub const fn kind(&self) -> CheckedStatefulEntryKind {
        self.kind
    }

    pub const fn state(&self) -> &CheckedNominalRole {
        &self.state
    }

    pub const fn initializer(&self) -> &CheckedCallableRole {
        &self.initializer
    }

    pub const fn event(&self) -> &CheckedNominalRole {
        &self.event
    }

    pub const fn reducer(&self) -> &CheckedCallableRole {
        &self.reducer
    }

    pub const fn initial_flow(&self) -> &CheckedInitialFlowRole {
        &self.initial_flow
    }
}

impl CheckedNominalRole {
    pub const fn key(&self) -> &BoundNominalTypeKey {
        &self.key
    }

    pub const fn schema(&self) -> &TypeShape {
        &self.schema
    }

    pub const fn schema_digest(&self) -> &NominalSchemaDigest {
        &self.schema_digest
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl CheckedCallableRole {
    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    pub const fn contract_digest(&self) -> &CallableContractDigest {
        &self.contract_digest
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl CheckedInitialFlowRole {
    pub const fn id(&self) -> &CheckedFlowId {
        &self.id
    }

    pub const fn contract_digest(&self) -> &FlowContractDigest {
        &self.contract_digest
    }

    pub fn state_parameter_name(&self) -> &str {
        &self.state_parameter_name
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl CheckedAgentEntry {
    pub const fn id(&self) -> &CheckedEntryId {
        &self.id
    }

    pub const fn controller(&self) -> &CheckedCallableRole {
        &self.controller
    }

    pub const fn policy(&self) -> &CheckedAgentPolicy {
        &self.policy
    }

    pub const fn budget(&self) -> AgentBudget {
        self.budget
    }

    pub const fn policy_digest(&self) -> &CheckedAgentPolicyDigest {
        &self.policy_digest
    }

    pub const fn binding_digest(&self) -> &CheckedEntryBindingDigest {
        &self.binding_digest
    }
}

impl CheckedAgentPolicy {
    pub(crate) fn new(allowed_effects: EffectSet, inferred_effects: EffectSet) -> Self {
        Self {
            allowed_effects,
            inferred_effects,
        }
    }

    pub const fn allowed_effects(&self) -> &EffectSet {
        &self.allowed_effects
    }

    pub const fn inferred_effects(&self) -> &EffectSet {
        &self.inferred_effects
    }

    pub fn allowed_effects_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &EffectId> + DoubleEndedIterator {
        self.allowed_effects.iter()
    }

    pub fn inferred_effects_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &EffectId> + DoubleEndedIterator {
        self.inferred_effects.iter()
    }
}

impl AgentBudget {
    /// Parses and validates the effective budget from ordinary function attributes.
    pub fn from_attributes(attributes: &[Attribute]) -> Result<Self, AgentBudgetError> {
        attributes
            .iter()
            .filter(|attribute| attribute.name() == "budget")
            .try_fold(Self::default_checked(), Self::apply_attribute)
    }

    fn default_checked() -> Self {
        Self {
            logical_timeout_millis: 30_000,
            max_vm_steps: 100_000,
            max_host_calls: 256,
            max_observations: 256,
            max_captures: 16,
            max_capture_bytes: 64 * 1024 * 1024,
            max_rag_queries: 8,
            max_context_bytes: 1024 * 1024,
        }
    }

    fn apply_attribute(mut self, attribute: &Attribute) -> Result<Self, AgentBudgetError> {
        let args = attribute.args().ok_or_else(|| AgentBudgetError {
            message: "budget attribute requires key/value arguments".to_owned(),
        })?;
        for item in args
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            let (key, value) = item.split_once('=').ok_or_else(|| AgentBudgetError {
                message: format!("budget item `{item}` must use key = value"),
            })?;
            let value = value.trim();
            match key.trim() {
                "timeout" => self.logical_timeout_millis = parse_budget_duration(value)?,
                "steps" => self.max_vm_steps = parse_budget_u64(value)?,
                "host_calls" => self.max_host_calls = parse_budget_u32(value)?,
                "observations" => self.max_observations = parse_budget_u32(value)?,
                "captures" => self.max_captures = parse_budget_u32(value)?,
                "stored_bytes" => self.max_capture_bytes = parse_budget_u64(value)?,
                "rag_queries" => self.max_rag_queries = parse_budget_u32(value)?,
                "context_bytes" => self.max_context_bytes = parse_budget_u64(value)?,
                other => {
                    return Err(AgentBudgetError {
                        message: format!("unsupported budget key `{other}`"),
                    });
                }
            }
        }
        Ok(self)
    }

    pub const fn logical_timeout_millis(self) -> u64 {
        self.logical_timeout_millis
    }

    pub const fn max_vm_steps(self) -> u64 {
        self.max_vm_steps
    }

    pub const fn max_host_calls(self) -> u32 {
        self.max_host_calls
    }

    pub const fn max_observations(self) -> u32 {
        self.max_observations
    }

    pub const fn max_captures(self) -> u32 {
        self.max_captures
    }

    pub const fn max_capture_bytes(self) -> u64 {
        self.max_capture_bytes
    }

    pub const fn max_rag_queries(self) -> u32 {
        self.max_rag_queries
    }

    pub const fn max_context_bytes(self) -> u64 {
        self.max_context_bytes
    }
}

fn parse_budget_duration(value: &str) -> Result<u64, AgentBudgetError> {
    let (number, multiplier) = value
        .trim()
        .strip_suffix("ms")
        .map(|number| (number, 1))
        .or_else(|| value.trim().strip_suffix('s').map(|number| (number, 1_000)))
        .ok_or_else(|| AgentBudgetError {
            message: format!("budget timeout `{value}` must use an `ms` or `s` suffix"),
        })?;
    parse_budget_u64(number)?
        .checked_mul(multiplier)
        .ok_or_else(|| AgentBudgetError {
            message: format!("budget timeout `{value}` overflows"),
        })
}

fn parse_budget_u32(value: &str) -> Result<u32, AgentBudgetError> {
    u32::try_from(parse_budget_u64(value)?).map_err(|_| AgentBudgetError {
        message: format!("budget value `{value}` does not fit in u32"),
    })
}

fn parse_budget_u64(value: &str) -> Result<u64, AgentBudgetError> {
    let trimmed = value.trim();
    let number = ["usize", "u64", "u32"]
        .into_iter()
        .find_map(|suffix| trimmed.strip_suffix(suffix))
        .unwrap_or(trimmed);
    let digits = number.replace('_', "");
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        return Err(AgentBudgetError {
            message: format!("budget value `{value}` must be a non-negative integer"),
        });
    }
    digits.parse().map_err(|error| AgentBudgetError {
        message: format!("invalid budget value `{value}`: {error}"),
    })
}
