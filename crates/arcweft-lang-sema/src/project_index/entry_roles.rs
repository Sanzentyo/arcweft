use std::collections::BTreeMap;

use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_source::SourceSpan;

use crate::entry::{
    BoundNominalTypeKey, CallableContractDigest, CheckedAgentPolicyDigest, CheckedEntryBinding,
    CheckedEntryBindingDigest, CheckedEntryCatalog, CheckedEntryId, CheckedEntryKind,
    CheckedFlowId, FlowContractDigest, NominalSchemaDigest,
};

/// Final schema-v1 record for one explicitly checked source entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEntryRecord {
    id: CheckedEntryId,
    kind: CheckedEntryKind,
    binding_digest: CheckedEntryBindingDigest,
    agent_policy_digest: Option<CheckedAgentPolicyDigest>,
}

/// Typed role label on an edge from one checked entry to its original declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectEntryRoleKind {
    State,
    Initializer,
    Event,
    Reducer,
    InitialFlow,
    Controller,
}

/// Original declaration identity plus the checked contract used by an entry role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectEntryRoleTarget {
    Nominal {
        key: BoundNominalTypeKey,
        schema_digest: NominalSchemaDigest,
    },
    Callable {
        declaration: CallableDeclarationId,
        contract_digest: CallableContractDigest,
    },
    Flow {
        id: CheckedFlowId,
        contract_digest: FlowContractDigest,
    },
}

/// Source-aware typed edge from a checked entry role to its original declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEntryRoleEdge {
    entry: CheckedEntryId,
    role: ProjectEntryRoleKind,
    target: ProjectEntryRoleTarget,
    source: SourceSpan,
}

pub(super) fn checked_entry_records_and_edges(
    catalog: &CheckedEntryCatalog,
) -> (
    BTreeMap<CheckedEntryId, ProjectEntryRecord>,
    Vec<ProjectEntryRoleEdge>,
) {
    let mut records = BTreeMap::new();
    let mut edges = Vec::new();
    for binding in catalog.entries() {
        let id = binding.id().clone();
        records.insert(id.clone(), ProjectEntryRecord::from_binding(binding));
        match binding {
            CheckedEntryBinding::Stateful(entry) => {
                edges.extend([
                    ProjectEntryRoleEdge::nominal(
                        id.clone(),
                        ProjectEntryRoleKind::State,
                        entry.state(),
                    ),
                    ProjectEntryRoleEdge::callable(
                        id.clone(),
                        ProjectEntryRoleKind::Initializer,
                        entry.initializer(),
                    ),
                    ProjectEntryRoleEdge::nominal(
                        id.clone(),
                        ProjectEntryRoleKind::Event,
                        entry.event(),
                    ),
                    ProjectEntryRoleEdge::callable(
                        id.clone(),
                        ProjectEntryRoleKind::Reducer,
                        entry.reducer(),
                    ),
                    ProjectEntryRoleEdge::flow(
                        id,
                        ProjectEntryRoleKind::InitialFlow,
                        entry.initial_flow(),
                    ),
                ]);
            }
            CheckedEntryBinding::Agent(entry) => {
                edges.push(ProjectEntryRoleEdge::callable(
                    id,
                    ProjectEntryRoleKind::Controller,
                    entry.controller(),
                ));
            }
            CheckedEntryBinding::Existing(_) => {}
        }
    }
    (records, edges)
}

impl ProjectEntryRecord {
    fn from_binding(binding: &CheckedEntryBinding) -> Self {
        Self {
            id: binding.id().clone(),
            kind: binding.kind(),
            binding_digest: *binding.binding_digest(),
            agent_policy_digest: binding.agent().map(|entry| *entry.policy_digest()),
        }
    }

    pub const fn id(&self) -> &CheckedEntryId {
        &self.id
    }

    pub const fn kind(&self) -> &CheckedEntryKind {
        &self.kind
    }

    pub const fn binding_digest(&self) -> &CheckedEntryBindingDigest {
        &self.binding_digest
    }

    pub const fn agent_policy_digest(&self) -> Option<&CheckedAgentPolicyDigest> {
        self.agent_policy_digest.as_ref()
    }
}

impl ProjectEntryRoleKind {
    /// Stable schema-v1 label used by Agent/tooling projections.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Initializer => "initializer",
            Self::Event => "event",
            Self::Reducer => "reducer",
            Self::InitialFlow => "initial_flow",
            Self::Controller => "controller",
        }
    }
}

impl ProjectEntryRoleEdge {
    fn nominal(
        entry: CheckedEntryId,
        role: ProjectEntryRoleKind,
        target: &crate::entry::CheckedNominalRole,
    ) -> Self {
        Self {
            entry,
            role,
            target: ProjectEntryRoleTarget::Nominal {
                key: target.key().clone(),
                schema_digest: *target.schema_digest(),
            },
            source: target.source().clone(),
        }
    }

    fn callable(
        entry: CheckedEntryId,
        role: ProjectEntryRoleKind,
        target: &crate::entry::CheckedCallableRole,
    ) -> Self {
        Self {
            entry,
            role,
            target: ProjectEntryRoleTarget::Callable {
                declaration: target.declaration().clone(),
                contract_digest: *target.contract_digest(),
            },
            source: target.source().clone(),
        }
    }

    fn flow(
        entry: CheckedEntryId,
        role: ProjectEntryRoleKind,
        target: &crate::entry::CheckedInitialFlowRole,
    ) -> Self {
        Self {
            entry,
            role,
            target: ProjectEntryRoleTarget::Flow {
                id: target.id().clone(),
                contract_digest: *target.contract_digest(),
            },
            source: target.source().clone(),
        }
    }

    pub const fn entry(&self) -> &CheckedEntryId {
        &self.entry
    }

    pub const fn role(&self) -> ProjectEntryRoleKind {
        self.role
    }

    pub const fn target(&self) -> &ProjectEntryRoleTarget {
        &self.target
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl ProjectEntryRoleTarget {
    pub const fn nominal(&self) -> Option<(&BoundNominalTypeKey, &NominalSchemaDigest)> {
        match self {
            Self::Nominal { key, schema_digest } => Some((key, schema_digest)),
            Self::Callable { .. } | Self::Flow { .. } => None,
        }
    }

    pub const fn callable(&self) -> Option<(&CallableDeclarationId, &CallableContractDigest)> {
        match self {
            Self::Callable {
                declaration,
                contract_digest,
            } => Some((declaration, contract_digest)),
            Self::Nominal { .. } | Self::Flow { .. } => None,
        }
    }

    pub const fn flow(&self) -> Option<(&CheckedFlowId, &FlowContractDigest)> {
        match self {
            Self::Flow {
                id,
                contract_digest,
            } => Some((id, contract_digest)),
            Self::Nominal { .. } | Self::Callable { .. } => None,
        }
    }
}
