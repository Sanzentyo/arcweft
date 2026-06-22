use arcweft_agent_protocol::resource::AgentResource;
use arcweft_content_policy::{ContentDigest, PolicyDecision, PolicyDisposition, PolicyReceipt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Public policy metadata attached out-of-band to an already-safe resource.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPolicySummary {
    pub profile_id: String,
    pub profile_version: String,
    pub disposition: PolicyDisposition,
    pub sanitized: bool,
    pub receipt_ids: Vec<String>,
    pub input_digest: ContentDigest,
    pub output_digest: Option<ContentDigest>,
    pub public_labels: BTreeSet<String>,
    pub reason_codes: BTreeSet<String>,
}

impl AgentPolicySummary {
    pub fn from_receipt(receipt: &PolicyReceipt) -> Self {
        Self {
            profile_id: receipt.profile_id.as_str().to_owned(),
            profile_version: receipt.profile_version.clone(),
            disposition: receipt.decision.disposition,
            sanitized: receipt.sanitized,
            receipt_ids: vec![receipt.id.as_str().to_owned()],
            input_digest: receipt.input_digest.clone(),
            output_digest: receipt.output_digest.clone(),
            public_labels: receipt
                .decision
                .public_labels
                .iter()
                .map(|category| category.as_str().to_owned())
                .collect(),
            reason_codes: receipt.decision.reason_codes.clone(),
        }
    }

    pub fn aggregate(
        profile_id: impl Into<String>,
        profile_version: impl Into<String>,
        input_digest: ContentDigest,
        output_digest: Option<ContentDigest>,
        receipts: &[PolicyReceipt],
        fallback: PolicyDecision,
    ) -> Self {
        let decision = receipts
            .iter()
            .map(|receipt| receipt.decision.clone())
            .fold(fallback, PolicyDecision::merge);
        Self {
            profile_id: profile_id.into(),
            profile_version: profile_version.into(),
            disposition: decision.disposition,
            sanitized: receipts.iter().any(|receipt| receipt.sanitized),
            receipt_ids: receipts
                .iter()
                .map(|receipt| receipt.id.as_str().to_owned())
                .collect(),
            input_digest,
            output_digest,
            public_labels: decision
                .public_labels
                .iter()
                .map(|category| category.as_str().to_owned())
                .collect(),
            reason_codes: decision.reason_codes,
        }
    }

    /// Opaque stable token used for moderated resource addresses and scope ids.
    pub fn opaque_token(&self) -> String {
        self.receipt_ids.first().map_or_else(
            || {
                self.output_digest
                    .as_ref()
                    .unwrap_or(&self.input_digest)
                    .as_str()
                    .chars()
                    .take(20)
                    .collect()
            },
            |receipt| {
                blake3::hash(receipt.as_bytes())
                    .to_hex()
                    .as_str()
                    .chars()
                    .take(20)
                    .collect()
            },
        )
    }

    /// External URI that does not preserve the raw resource path or query.
    pub fn moderated_uri(&self, extension: &str) -> String {
        format!("arcweft://moderated/{}.{}", self.opaque_token(), extension)
    }

    /// Derives a stable opaque token for one child of a moderated aggregate.
    pub fn opaque_child_token(&self, namespace: &str, index: usize) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.moderated-child.v1");
        hasher.update(self.opaque_token().as_bytes());
        let namespace_len = u64::try_from(namespace.len()).unwrap_or(u64::MAX);
        hasher.update(&namespace_len.to_le_bytes());
        hasher.update(namespace.as_bytes());
        let index = u64::try_from(index).unwrap_or(u64::MAX);
        hasher.update(&index.to_le_bytes());
        hasher
            .finalize()
            .to_hex()
            .as_str()
            .chars()
            .take(20)
            .collect()
    }

    /// External URI for one child resource without leaking its internal id.
    pub fn moderated_child_uri(&self, namespace: &str, index: usize, extension: &str) -> String {
        format!(
            "arcweft://moderated/{}.{}",
            self.opaque_child_token(namespace, index),
            extension
        )
    }

    pub fn synthetic(
        profile_id: impl Into<String>,
        profile_version: impl Into<String>,
        disposition: PolicyDisposition,
        reason: impl Into<String>,
        input_digest: ContentDigest,
        output_digest: Option<ContentDigest>,
        sanitized: bool,
    ) -> Self {
        Self {
            profile_id: profile_id.into(),
            profile_version: profile_version.into(),
            disposition,
            sanitized,
            receipt_ids: Vec::new(),
            input_digest,
            output_digest,
            public_labels: BTreeSet::new(),
            reason_codes: BTreeSet::from([reason.into()]),
        }
    }
}

/// Resource proven safe for the configured Agent/MCP publication profile.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PublishedAgentResource {
    resource: AgentResource,
    policy: AgentPolicySummary,
}

impl PublishedAgentResource {
    pub(crate) const fn new(resource: AgentResource, policy: AgentPolicySummary) -> Self {
        Self { resource, policy }
    }

    pub const fn resource(&self) -> &AgentResource {
        &self.resource
    }

    pub const fn policy(&self) -> &AgentPolicySummary {
        &self.policy
    }

    pub fn into_resource(self) -> AgentResource {
        self.resource
    }
}
