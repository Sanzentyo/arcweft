//! Sans I/O release manifest for external AWFB content.

use crate::container::{BundleDigest, BundleKind, BundleView, ReadBudget};
use std::collections::BTreeSet;
use thiserror::Error;

pub const RELEASE_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const RELEASE_SIGNATURE_ENVELOPE_SCHEMA_VERSION: u32 = 1;
pub const RELEASE_SIGNATURE_ALGORITHM_ED25519_V1: &str = "ed25519-v1";
pub const DEFAULT_RELEASE_USER_AGENT: &str = "arcweft-release-cache/1";

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "ReleaseFetchPolicy::is_default")]
    pub fetch_policy: ReleaseFetchPolicy,
    #[serde(default, skip_serializing_if = "ReleaseSignaturePolicy::is_default")]
    pub signature_policy: ReleaseSignaturePolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<ReleaseBundleRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseBundleRef {
    pub content_root: BundleDigest,
    pub file_digest: BundleDigest,
    pub byte_len: u64,
    pub kind: BundleKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<ReleaseMirror>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseMirror {
    pub uri: String,
    #[serde(default)]
    pub priority: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseFetchPlan {
    pub content_root: BundleDigest,
    pub file_digest: BundleDigest,
    pub byte_len: u64,
    pub kind: BundleKind,
    pub fetch_policy: ReleaseFetchPolicy,
    pub signature_policy: ReleaseSignaturePolicy,
    pub mirrors: Vec<ReleaseMirror>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseFetchPolicy {
    #[serde(default = "default_max_attempts_per_mirror")]
    pub max_attempts_per_mirror: u8,
    #[serde(default = "default_candidate_byte_budget")]
    pub candidate_byte_budget: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_after_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "ReleaseNetworkFetchPolicy::is_default")]
    pub network_policy: ReleaseNetworkFetchPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseNetworkFetchPolicy {
    #[serde(default, skip_serializing_if = "is_false_bool")]
    pub require_https: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_profile: Option<String>,
    #[serde(
        default = "default_release_user_agent",
        skip_serializing_if = "is_default_release_user_agent"
    )]
    pub user_agent: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseSignaturePolicy {
    #[serde(default)]
    pub require_awfb_signature: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_signature_bytes: Option<u64>,
    #[serde(
        default = "default_release_signature_algorithms",
        skip_serializing_if = "is_default_release_signature_algorithms"
    )]
    pub allowed_algorithms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_signer_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_public_keys: Vec<ReleaseTrustedPublicKey>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseTrustedPublicKey {
    pub signer_id: String,
    #[serde(default = "default_release_signature_algorithm")]
    pub algorithm: String,
    pub public_key: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub valid_from_key_epoch: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until_key_epoch: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false_bool")]
    pub revoked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseSignatureEnvelope {
    pub schema_version: u32,
    pub signer_id: String,
    pub algorithm: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub key_epoch: u64,
    pub content_root: BundleDigest,
    pub kind: BundleKind,
    pub signing_digest: BundleDigest,
    pub signature: String,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ReleaseManifestError {
    #[error("unsupported AWFR schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("release manifest contains duplicate content root {0}")]
    DuplicateContentRoot(BundleDigest),
    #[error("release manifest bundle {0} has no mirrors")]
    MissingMirrors(BundleDigest),
    #[error("release manifest bundle {0} has zero byte length")]
    EmptyBundle(BundleDigest),
    #[error("release manifest has no bundle for content root {0}")]
    MissingContentRoot(BundleDigest),
    #[error("release manifest mirror URI `{0}` has an unsupported scheme")]
    UnsupportedMirrorScheme(String),
    #[error("release manifest mirror URI is empty")]
    EmptyMirrorUri,
    #[error("release manifest fetch policy is invalid: {0}")]
    InvalidFetchPolicy(String),
    #[error("release manifest signature policy is invalid: {0}")]
    InvalidSignaturePolicy(String),
    #[error(
        "release manifest candidate byte budget exceeded for {content_root}: byte length {byte_len}, budget {budget}"
    )]
    CandidateByteBudgetExceeded {
        content_root: BundleDigest,
        byte_len: u64,
        budget: u64,
    },
    #[error("failed to encode release manifest JSON: {0}")]
    EncodeJson(String),
    #[error("failed to decode release manifest JSON: {0}")]
    DecodeJson(String),
    #[error("failed to decode referenced AWFB bundle: {0}")]
    DecodeAwfb(String),
    #[error("external bundle {content_root} is missing a required AWFB signature block")]
    MissingAwfbSignature { content_root: BundleDigest },
    #[error("external bundle {content_root} AWFB signature envelope is invalid: {message}")]
    InvalidSignatureEnvelope {
        content_root: BundleDigest,
        message: String,
    },
    #[error("external bundle {content_root} was signed by untrusted signer `{signer_id}`")]
    UntrustedSigner {
        content_root: BundleDigest,
        signer_id: String,
    },
    #[error(
        "external bundle {content_root} has no trusted public key for signer `{signer_id}` and algorithm `{algorithm}`"
    )]
    MissingTrustedPublicKey {
        content_root: BundleDigest,
        signer_id: String,
        algorithm: String,
    },
    #[error(
        "external bundle {content_root} has no trusted public key currently valid for signer `{signer_id}`, algorithm `{algorithm}`, and key epoch {key_epoch}"
    )]
    NoValidTrustedPublicKey {
        content_root: BundleDigest,
        signer_id: String,
        algorithm: String,
        key_epoch: u64,
    },
    #[error(
        "external bundle {content_root} signature verification failed for signer `{signer_id}`"
    )]
    SignatureVerificationFailed {
        content_root: BundleDigest,
        signer_id: String,
    },
    #[error(
        "external bundle {content_root} signature envelope content root mismatch: expected {expected}, actual {actual}"
    )]
    SignatureContentRootMismatch {
        content_root: BundleDigest,
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error(
        "external bundle {content_root} signature envelope kind mismatch: expected {expected:?}, actual {actual:?}"
    )]
    SignatureKindMismatch {
        content_root: BundleDigest,
        expected: BundleKind,
        actual: BundleKind,
    },
    #[error(
        "external bundle {content_root} signature envelope signing digest mismatch: expected {expected}, actual {actual}"
    )]
    SignatureDigestMismatch {
        content_root: BundleDigest,
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error(
        "external bundle {content_root} AWFB signature block is too small: expected at least {minimum} byte(s), actual {actual}"
    )]
    SignatureTooSmall {
        content_root: BundleDigest,
        minimum: u64,
        actual: u64,
    },
    #[error(
        "external bundle byte length mismatch for {content_root}: expected {expected}, actual {actual}"
    )]
    ByteLengthMismatch {
        content_root: BundleDigest,
        expected: u64,
        actual: u64,
    },
    #[error(
        "external bundle digest mismatch for {content_root}: expected {expected}, actual {actual}"
    )]
    FileDigestMismatch {
        content_root: BundleDigest,
        expected: BundleDigest,
        actual: BundleDigest,
    },
}

impl Default for ReleaseManifest {
    fn default() -> Self {
        Self {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::default(),
            bundles: Vec::new(),
        }
    }
}

impl ReleaseManifest {
    pub fn new(
        bundles: impl IntoIterator<Item = ReleaseBundleRef>,
    ) -> Result<Self, ReleaseManifestError> {
        let manifest = Self {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::default(),
            bundles: bundles.into_iter().collect(),
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ReleaseManifestError> {
        if self.schema_version != RELEASE_MANIFEST_SCHEMA_VERSION {
            return Err(ReleaseManifestError::UnsupportedSchema {
                actual: self.schema_version,
                expected: RELEASE_MANIFEST_SCHEMA_VERSION,
            });
        }
        self.fetch_policy.validate()?;
        self.signature_policy.validate()?;
        let mut seen = BTreeSet::new();
        for bundle in &self.bundles {
            if !seen.insert(bundle.content_root) {
                return Err(ReleaseManifestError::DuplicateContentRoot(
                    bundle.content_root,
                ));
            }
            bundle.validate()?;
        }
        Ok(())
    }

    pub fn bundle(&self, content_root: BundleDigest) -> Option<&ReleaseBundleRef> {
        self.bundles
            .iter()
            .find(|bundle| bundle.content_root == content_root)
    }

    pub fn fetch_plan(
        &self,
        content_root: BundleDigest,
    ) -> Result<ReleaseFetchPlan, ReleaseManifestError> {
        self.validate()?;
        self.fetch_policy
            .check_byte_budget(content_root, self.bundle(content_root))?;
        self.bundle(content_root)
            .ok_or(ReleaseManifestError::MissingContentRoot(content_root))
            .map(|bundle| {
                bundle.fetch_plan_with_policy(
                    self.fetch_policy.clone(),
                    self.signature_policy.clone(),
                )
            })
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, ReleaseManifestError> {
        self.validate()?;
        serde_json::to_vec_pretty(self)
            .map_err(|error| ReleaseManifestError::EncodeJson(error.to_string()))
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, ReleaseManifestError> {
        let manifest = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| ReleaseManifestError::DecodeJson(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }
}

impl ReleaseBundleRef {
    pub fn from_awfb_bytes(
        bytes: &[u8],
        mirrors: impl IntoIterator<Item = ReleaseMirror>,
    ) -> Result<Self, ReleaseManifestError> {
        let view = BundleView::parse(bytes, ReadBudget::default())
            .map_err(|error| ReleaseManifestError::DecodeAwfb(error.to_string()))?;
        Self::new(
            view.content_root(),
            BundleDigest::of(bytes),
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            view.kind(),
            mirrors,
        )
    }

    pub fn new(
        content_root: BundleDigest,
        file_digest: BundleDigest,
        byte_len: u64,
        kind: BundleKind,
        mirrors: impl IntoIterator<Item = ReleaseMirror>,
    ) -> Result<Self, ReleaseManifestError> {
        let bundle = Self {
            content_root,
            file_digest,
            byte_len,
            kind,
            mirrors: mirrors.into_iter().collect(),
        };
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn validate(&self) -> Result<(), ReleaseManifestError> {
        if self.byte_len == 0 {
            return Err(ReleaseManifestError::EmptyBundle(self.content_root));
        }
        if self.mirrors.is_empty() {
            return Err(ReleaseManifestError::MissingMirrors(self.content_root));
        }
        for mirror in &self.mirrors {
            mirror.validate()?;
        }
        Ok(())
    }

    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), ReleaseManifestError> {
        let actual_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_len != self.byte_len {
            return Err(ReleaseManifestError::ByteLengthMismatch {
                content_root: self.content_root,
                expected: self.byte_len,
                actual: actual_len,
            });
        }
        let actual_digest = BundleDigest::of(bytes);
        if actual_digest != self.file_digest {
            return Err(ReleaseManifestError::FileDigestMismatch {
                content_root: self.content_root,
                expected: self.file_digest,
                actual: actual_digest,
            });
        }
        Ok(())
    }

    pub fn fetch_plan(&self) -> ReleaseFetchPlan {
        self.fetch_plan_with_policy(
            ReleaseFetchPolicy::default(),
            ReleaseSignaturePolicy::default(),
        )
    }

    pub fn fetch_plan_with_policy(
        &self,
        fetch_policy: ReleaseFetchPolicy,
        signature_policy: ReleaseSignaturePolicy,
    ) -> ReleaseFetchPlan {
        let mut mirrors = self.mirrors.clone();
        mirrors.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.uri.cmp(&right.uri))
        });
        ReleaseFetchPlan {
            content_root: self.content_root,
            file_digest: self.file_digest,
            byte_len: self.byte_len,
            kind: self.kind,
            fetch_policy,
            signature_policy,
            mirrors,
        }
    }
}

impl ReleaseFetchPlan {
    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), ReleaseManifestError> {
        ReleaseBundleRef {
            content_root: self.content_root,
            file_digest: self.file_digest,
            byte_len: self.byte_len,
            kind: self.kind,
            mirrors: self.mirrors.clone(),
        }
        .verify_bytes(bytes)?;
        self.signature_policy
            .verify_awfb_bytes(self.content_root, bytes)
    }
}

impl ReleaseMirror {
    pub fn new(uri: impl Into<String>) -> Result<Self, ReleaseManifestError> {
        let mirror = Self {
            uri: uri.into(),
            priority: 0,
        };
        mirror.validate()?;
        Ok(mirror)
    }

    pub fn with_priority(
        uri: impl Into<String>,
        priority: u16,
    ) -> Result<Self, ReleaseManifestError> {
        let mirror = Self {
            uri: uri.into(),
            priority,
        };
        mirror.validate()?;
        Ok(mirror)
    }

    pub fn validate(&self) -> Result<(), ReleaseManifestError> {
        if self.uri.is_empty() {
            return Err(ReleaseManifestError::EmptyMirrorUri);
        }
        if !is_supported_mirror_uri(&self.uri) {
            return Err(ReleaseManifestError::UnsupportedMirrorScheme(
                self.uri.clone(),
            ));
        }
        Ok(())
    }
}

impl Default for ReleaseFetchPolicy {
    fn default() -> Self {
        Self {
            max_attempts_per_mirror: default_max_attempts_per_mirror(),
            candidate_byte_budget: default_candidate_byte_budget(),
            cancel_after_millis: None,
            network_policy: ReleaseNetworkFetchPolicy::default(),
        }
    }
}

impl Default for ReleaseNetworkFetchPolicy {
    fn default() -> Self {
        Self {
            require_https: false,
            proxy_profile: None,
            auth_profile: None,
            client_profile: None,
            user_agent: default_release_user_agent(),
        }
    }
}

impl Default for ReleaseSignaturePolicy {
    fn default() -> Self {
        Self {
            require_awfb_signature: false,
            minimum_signature_bytes: None,
            allowed_algorithms: default_release_signature_algorithms(),
            trusted_signer_ids: Vec::new(),
            trusted_public_keys: Vec::new(),
        }
    }
}

impl ReleaseFetchPolicy {
    pub fn new(
        max_attempts_per_mirror: u8,
        candidate_byte_budget: u64,
        cancel_after_millis: Option<u64>,
    ) -> Result<Self, ReleaseManifestError> {
        let policy = Self {
            max_attempts_per_mirror,
            candidate_byte_budget,
            cancel_after_millis,
            network_policy: ReleaseNetworkFetchPolicy::default(),
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn with_network_policy(
        mut self,
        network_policy: ReleaseNetworkFetchPolicy,
    ) -> Result<Self, ReleaseManifestError> {
        self.network_policy = network_policy;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ReleaseManifestError> {
        if self.max_attempts_per_mirror == 0 {
            return Err(ReleaseManifestError::InvalidFetchPolicy(
                "max_attempts_per_mirror must be greater than zero".to_owned(),
            ));
        }
        if self.candidate_byte_budget == 0 {
            return Err(ReleaseManifestError::InvalidFetchPolicy(
                "candidate_byte_budget must be greater than zero".to_owned(),
            ));
        }
        if self.cancel_after_millis == Some(0) {
            return Err(ReleaseManifestError::InvalidFetchPolicy(
                "cancel_after_millis must be greater than zero when set".to_owned(),
            ));
        }
        self.network_policy.validate()?;
        Ok(())
    }

    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    fn check_byte_budget(
        &self,
        content_root: BundleDigest,
        bundle: Option<&ReleaseBundleRef>,
    ) -> Result<(), ReleaseManifestError> {
        let Some(bundle) = bundle else {
            return Ok(());
        };
        if bundle.byte_len > self.candidate_byte_budget {
            return Err(ReleaseManifestError::CandidateByteBudgetExceeded {
                content_root,
                byte_len: bundle.byte_len,
                budget: self.candidate_byte_budget,
            });
        }
        Ok(())
    }
}

impl ReleaseNetworkFetchPolicy {
    pub fn require_https() -> Self {
        Self {
            require_https: true,
            ..Self::default()
        }
    }

    pub fn with_proxy_profile(
        mut self,
        proxy_profile: impl Into<String>,
    ) -> Result<Self, ReleaseManifestError> {
        self.proxy_profile = Some(proxy_profile.into());
        self.validate()?;
        Ok(self)
    }

    pub fn with_auth_profile(
        mut self,
        auth_profile: impl Into<String>,
    ) -> Result<Self, ReleaseManifestError> {
        self.auth_profile = Some(auth_profile.into());
        self.validate()?;
        Ok(self)
    }

    pub fn with_client_profile(
        mut self,
        client_profile: impl Into<String>,
    ) -> Result<Self, ReleaseManifestError> {
        self.client_profile = Some(client_profile.into());
        self.validate()?;
        Ok(self)
    }

    pub fn with_user_agent(
        mut self,
        user_agent: impl Into<String>,
    ) -> Result<Self, ReleaseManifestError> {
        self.user_agent = user_agent.into();
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ReleaseManifestError> {
        validate_profile_id("proxy_profile", self.proxy_profile.as_deref())?;
        validate_profile_id("auth_profile", self.auth_profile.as_deref())?;
        validate_profile_id("client_profile", self.client_profile.as_deref())?;
        if self.user_agent.is_empty() {
            return Err(ReleaseManifestError::InvalidFetchPolicy(
                "network_policy user_agent must not be empty".to_owned(),
            ));
        }
        if self.user_agent.len() > 256 {
            return Err(ReleaseManifestError::InvalidFetchPolicy(
                "network_policy user_agent must not exceed 256 bytes".to_owned(),
            ));
        }
        if self
            .user_agent
            .chars()
            .any(|ch| matches!(ch, '\r' | '\n') || ch.is_control())
        {
            return Err(ReleaseManifestError::InvalidFetchPolicy(
                "network_policy user_agent must not contain control characters".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

impl ReleaseSignaturePolicy {
    pub fn require_signature(
        minimum_signature_bytes: Option<u64>,
    ) -> Result<Self, ReleaseManifestError> {
        let policy = Self {
            require_awfb_signature: true,
            minimum_signature_bytes,
            allowed_algorithms: default_release_signature_algorithms(),
            trusted_signer_ids: Vec::new(),
            trusted_public_keys: Vec::new(),
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn require_trusted_signers(
        minimum_signature_bytes: Option<u64>,
        trusted_signer_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, ReleaseManifestError> {
        let policy = Self {
            require_awfb_signature: true,
            minimum_signature_bytes,
            allowed_algorithms: default_release_signature_algorithms(),
            trusted_signer_ids: trusted_signer_ids.into_iter().map(Into::into).collect(),
            trusted_public_keys: Vec::new(),
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn require_trusted_public_keys(
        minimum_signature_bytes: Option<u64>,
        trusted_public_keys: impl IntoIterator<Item = ReleaseTrustedPublicKey>,
    ) -> Result<Self, ReleaseManifestError> {
        let policy = Self {
            require_awfb_signature: true,
            minimum_signature_bytes,
            allowed_algorithms: default_release_signature_algorithms(),
            trusted_signer_ids: Vec::new(),
            trusted_public_keys: trusted_public_keys.into_iter().collect(),
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), ReleaseManifestError> {
        if self.minimum_signature_bytes == Some(0) {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(
                "minimum_signature_bytes must be greater than zero when set".to_owned(),
            ));
        }
        if self.minimum_signature_bytes.is_some() && !self.require_awfb_signature {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(
                "minimum_signature_bytes requires require_awfb_signature".to_owned(),
            ));
        }
        if !self.trusted_signer_ids.is_empty() && !self.require_awfb_signature {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(
                "trusted_signer_ids requires require_awfb_signature".to_owned(),
            ));
        }
        if !self.trusted_public_keys.is_empty() && !self.require_awfb_signature {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(
                "trusted_public_keys requires require_awfb_signature".to_owned(),
            ));
        }
        let mut algorithms = BTreeSet::new();
        for algorithm in &self.allowed_algorithms {
            if algorithm.is_empty() {
                return Err(ReleaseManifestError::InvalidSignaturePolicy(
                    "allowed_algorithms must not contain empty algorithms".to_owned(),
                ));
            }
            if !is_supported_release_signature_algorithm(algorithm) {
                return Err(ReleaseManifestError::InvalidSignaturePolicy(format!(
                    "unsupported signature algorithm `{algorithm}`"
                )));
            }
            if !algorithms.insert(algorithm) {
                return Err(ReleaseManifestError::InvalidSignaturePolicy(format!(
                    "duplicate signature algorithm `{algorithm}`"
                )));
            }
        }
        if algorithms.is_empty() {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(
                "allowed_algorithms must not be empty".to_owned(),
            ));
        }
        let mut signer_ids = BTreeSet::new();
        for signer_id in &self.trusted_signer_ids {
            if signer_id.is_empty() {
                return Err(ReleaseManifestError::InvalidSignaturePolicy(
                    "trusted_signer_ids must not contain empty ids".to_owned(),
                ));
            }
            if !signer_ids.insert(signer_id) {
                return Err(ReleaseManifestError::InvalidSignaturePolicy(format!(
                    "duplicate trusted signer id `{signer_id}`"
                )));
            }
        }
        let mut public_keys = BTreeSet::new();
        for trusted in &self.trusted_public_keys {
            trusted.validate()?;
            if !public_keys.insert((
                trusted.signer_id.as_str(),
                trusted.algorithm.as_str(),
                trusted.public_key.as_str(),
            )) {
                return Err(ReleaseManifestError::InvalidSignaturePolicy(format!(
                    "duplicate trusted public key for signer `{}`",
                    trusted.signer_id
                )));
            }
        }
        Ok(())
    }

    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn verify_awfb_bytes(
        &self,
        content_root: BundleDigest,
        bytes: &[u8],
    ) -> Result<(), ReleaseManifestError> {
        if !self.require_awfb_signature {
            return Ok(());
        }
        let view = BundleView::parse(bytes, ReadBudget::default())
            .map_err(|error| ReleaseManifestError::DecodeAwfb(error.to_string()))?;
        let Some(signature) = view.signature() else {
            return Err(ReleaseManifestError::MissingAwfbSignature { content_root });
        };
        let actual = u64::try_from(signature.len()).unwrap_or(u64::MAX);
        if let Some(minimum) = self.minimum_signature_bytes
            && actual < minimum
        {
            return Err(ReleaseManifestError::SignatureTooSmall {
                content_root,
                minimum,
                actual,
            });
        }
        if !self.trusted_signer_ids.is_empty() || !self.trusted_public_keys.is_empty() {
            let envelope = ReleaseSignatureEnvelope::from_json_slice(content_root, signature)?;
            self.validate_envelope_algorithm(content_root, &envelope)?;
            let signing_digest = view
                .signing_digest()
                .map_err(|error| ReleaseManifestError::DecodeAwfb(error.to_string()))?;
            envelope.verify_bundle_binding(content_root, view.kind(), signing_digest)?;
            if !self.trusted_signer_ids.is_empty()
                && !self
                    .trusted_signer_ids
                    .iter()
                    .any(|trusted| trusted == &envelope.signer_id)
            {
                return Err(ReleaseManifestError::UntrustedSigner {
                    content_root,
                    signer_id: envelope.signer_id.clone(),
                });
            }
            self.verify_trusted_signature_payload(content_root, &envelope)?;
        }
        Ok(())
    }

    fn validate_envelope_algorithm(
        &self,
        content_root: BundleDigest,
        envelope: &ReleaseSignatureEnvelope,
    ) -> Result<(), ReleaseManifestError> {
        if self
            .allowed_algorithms
            .iter()
            .any(|algorithm| algorithm == &envelope.algorithm)
        {
            return Ok(());
        }
        Err(ReleaseManifestError::InvalidSignatureEnvelope {
            content_root,
            message: format!(
                "signature algorithm `{}` is not allowed by release policy",
                envelope.algorithm
            ),
        })
    }

    fn verify_trusted_signature_payload(
        &self,
        content_root: BundleDigest,
        envelope: &ReleaseSignatureEnvelope,
    ) -> Result<(), ReleaseManifestError> {
        if self.trusted_public_keys.is_empty() {
            return Ok(());
        }
        let matching_keys = self
            .trusted_public_keys
            .iter()
            .filter(|trusted| {
                trusted.signer_id == envelope.signer_id && trusted.algorithm == envelope.algorithm
            })
            .collect::<Vec<_>>();
        if matching_keys.is_empty() {
            return Err(ReleaseManifestError::MissingTrustedPublicKey {
                content_root,
                signer_id: envelope.signer_id.clone(),
                algorithm: envelope.algorithm.clone(),
            });
        }
        let valid_keys = matching_keys
            .iter()
            .copied()
            .filter(|trusted| trusted.is_valid_for_epoch(envelope.key_epoch))
            .collect::<Vec<_>>();
        if valid_keys.is_empty() {
            return Err(ReleaseManifestError::NoValidTrustedPublicKey {
                content_root,
                signer_id: envelope.signer_id.clone(),
                algorithm: envelope.algorithm.clone(),
                key_epoch: envelope.key_epoch,
            });
        }
        for trusted in valid_keys {
            match envelope.verify_ed25519(content_root, trusted) {
                Ok(()) => return Ok(()),
                Err(ReleaseManifestError::SignatureVerificationFailed { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Err(ReleaseManifestError::SignatureVerificationFailed {
            content_root,
            signer_id: envelope.signer_id.clone(),
        })
    }
}

impl ReleaseTrustedPublicKey {
    pub fn ed25519_v1(
        signer_id: impl Into<String>,
        public_key: impl Into<String>,
    ) -> Result<Self, ReleaseManifestError> {
        let key = Self {
            signer_id: signer_id.into(),
            algorithm: RELEASE_SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
            public_key: public_key.into(),
            valid_from_key_epoch: 0,
            valid_until_key_epoch: None,
            revoked: false,
        };
        key.validate()?;
        Ok(key)
    }

    pub fn with_key_epoch_validity(
        mut self,
        valid_from_key_epoch: u64,
        valid_until_key_epoch: Option<u64>,
    ) -> Result<Self, ReleaseManifestError> {
        self.valid_from_key_epoch = valid_from_key_epoch;
        self.valid_until_key_epoch = valid_until_key_epoch;
        self.validate()?;
        Ok(self)
    }

    pub fn revoked(mut self) -> Result<Self, ReleaseManifestError> {
        self.revoked = true;
        self.validate()?;
        Ok(self)
    }

    fn is_valid_for_epoch(&self, key_epoch: u64) -> bool {
        !self.revoked
            && key_epoch >= self.valid_from_key_epoch
            && self
                .valid_until_key_epoch
                .is_none_or(|valid_until| key_epoch < valid_until)
    }

    fn validate(&self) -> Result<(), ReleaseManifestError> {
        if self.signer_id.is_empty() {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(
                "trusted_public_keys signer_id must not be empty".to_owned(),
            ));
        }
        if self.algorithm != RELEASE_SIGNATURE_ALGORITHM_ED25519_V1 {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(format!(
                "trusted_public_keys only supports `{RELEASE_SIGNATURE_ALGORITHM_ED25519_V1}`"
            )));
        }
        decode_hex_array::<32>(&self.public_key).map_err(|message| {
            ReleaseManifestError::InvalidSignaturePolicy(format!(
                "trusted_public_keys public_key for signer `{}` is invalid: {message}",
                self.signer_id
            ))
        })?;
        if let Some(valid_until) = self.valid_until_key_epoch
            && valid_until <= self.valid_from_key_epoch
        {
            return Err(ReleaseManifestError::InvalidSignaturePolicy(format!(
                "trusted_public_keys key epoch window for signer `{}` must have valid_until_key_epoch greater than valid_from_key_epoch",
                self.signer_id
            )));
        }
        Ok(())
    }
}

impl ReleaseSignatureEnvelope {
    pub fn new(
        signer_id: impl Into<String>,
        algorithm: impl Into<String>,
        content_root: BundleDigest,
        kind: BundleKind,
        signing_digest: BundleDigest,
        signature: impl Into<String>,
    ) -> Result<Self, ReleaseManifestError> {
        let envelope = Self {
            schema_version: RELEASE_SIGNATURE_ENVELOPE_SCHEMA_VERSION,
            signer_id: signer_id.into(),
            algorithm: algorithm.into(),
            key_epoch: 0,
            content_root,
            kind,
            signing_digest,
            signature: signature.into(),
        };
        envelope.validate(BundleDigest::ZERO)?;
        Ok(envelope)
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, ReleaseManifestError> {
        self.validate(BundleDigest::ZERO)?;
        serde_json::to_vec(self)
            .map_err(|error| ReleaseManifestError::EncodeJson(error.to_string()))
    }

    fn from_json_slice(
        content_root: BundleDigest,
        bytes: &[u8],
    ) -> Result<Self, ReleaseManifestError> {
        let envelope = serde_json::from_slice::<Self>(bytes).map_err(|error| {
            ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: error.to_string(),
            }
        })?;
        envelope.validate(content_root)?;
        Ok(envelope)
    }

    fn validate(&self, content_root: BundleDigest) -> Result<(), ReleaseManifestError> {
        if self.schema_version != RELEASE_SIGNATURE_ENVELOPE_SCHEMA_VERSION {
            return Err(ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: format!(
                    "unsupported signature envelope schema version {}; expected {}",
                    self.schema_version, RELEASE_SIGNATURE_ENVELOPE_SCHEMA_VERSION
                ),
            });
        }
        if self.signer_id.is_empty() {
            return Err(ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: "signer_id must not be empty".to_owned(),
            });
        }
        if self.algorithm.is_empty() {
            return Err(ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: "algorithm must not be empty".to_owned(),
            });
        }
        if !is_supported_release_signature_algorithm(&self.algorithm) {
            return Err(ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: format!("unsupported signature algorithm `{}`", self.algorithm),
            });
        }
        if self.signature.is_empty() {
            return Err(ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: "signature must not be empty".to_owned(),
            });
        }
        Ok(())
    }

    fn verify_bundle_binding(
        &self,
        content_root: BundleDigest,
        kind: BundleKind,
        signing_digest: BundleDigest,
    ) -> Result<(), ReleaseManifestError> {
        if self.content_root != content_root {
            return Err(ReleaseManifestError::SignatureContentRootMismatch {
                content_root,
                expected: content_root,
                actual: self.content_root,
            });
        }
        if self.kind != kind {
            return Err(ReleaseManifestError::SignatureKindMismatch {
                content_root,
                expected: kind,
                actual: self.kind,
            });
        }
        if self.signing_digest != signing_digest {
            return Err(ReleaseManifestError::SignatureDigestMismatch {
                content_root,
                expected: signing_digest,
                actual: self.signing_digest,
            });
        }
        Ok(())
    }

    pub fn signing_message(&self) -> Vec<u8> {
        let mut message = Vec::with_capacity(160 + self.signer_id.len() + self.algorithm.len());
        message.extend_from_slice(b"arcweft.release.signature.v1\0");
        push_len_prefixed_bytes(&mut message, self.signer_id.as_bytes());
        push_len_prefixed_bytes(&mut message, self.algorithm.as_bytes());
        message.extend_from_slice(&self.key_epoch.to_le_bytes());
        message.extend_from_slice(&self.kind.encoded().to_le_bytes());
        message.extend_from_slice(&self.content_root.as_bytes());
        message.extend_from_slice(&self.signing_digest.as_bytes());
        message
    }

    fn verify_ed25519(
        &self,
        content_root: BundleDigest,
        trusted: &ReleaseTrustedPublicKey,
    ) -> Result<(), ReleaseManifestError> {
        if self.algorithm != RELEASE_SIGNATURE_ALGORITHM_ED25519_V1 {
            return Err(ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: format!(
                    "unsupported signature algorithm `{}` for trusted public-key verification",
                    self.algorithm
                ),
            });
        }
        let public_key = decode_hex_array::<32>(&trusted.public_key).map_err(|message| {
            ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: format!("trusted public key is invalid: {message}"),
            }
        })?;
        let signature = decode_hex_array::<64>(&self.signature).map_err(|message| {
            ReleaseManifestError::InvalidSignatureEnvelope {
                content_root,
                message: format!("signature is invalid: {message}"),
            }
        })?;
        let verifying_key =
            ed25519_dalek::VerifyingKey::from_bytes(&public_key).map_err(|error| {
                ReleaseManifestError::InvalidSignatureEnvelope {
                    content_root,
                    message: format!("trusted public key is invalid: {error}"),
                }
            })?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature);
        ed25519_dalek::Verifier::verify(&verifying_key, &self.signing_message(), &signature)
            .map_err(|_| ReleaseManifestError::SignatureVerificationFailed {
                content_root,
                signer_id: self.signer_id.clone(),
            })
    }
}

pub fn is_supported_release_signature_algorithm(algorithm: &str) -> bool {
    algorithm == RELEASE_SIGNATURE_ALGORITHM_ED25519_V1
}

const fn default_max_attempts_per_mirror() -> u8 {
    1
}

const fn default_candidate_byte_budget() -> u64 {
    u64::MAX
}

fn default_release_signature_algorithm() -> String {
    RELEASE_SIGNATURE_ALGORITHM_ED25519_V1.to_owned()
}

fn default_release_signature_algorithms() -> Vec<String> {
    vec![RELEASE_SIGNATURE_ALGORITHM_ED25519_V1.to_owned()]
}

fn is_default_release_signature_algorithms(value: &[String]) -> bool {
    value.len() == 1 && value[0] == RELEASE_SIGNATURE_ALGORITHM_ED25519_V1
}

fn default_release_user_agent() -> String {
    DEFAULT_RELEASE_USER_AGENT.to_owned()
}

fn is_default_release_user_agent(value: &str) -> bool {
    value == DEFAULT_RELEASE_USER_AGENT
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false_bool(value: &bool) -> bool {
    !*value
}

fn is_supported_mirror_uri(uri: &str) -> bool {
    matches!(
        uri.split_once(':').map(|(scheme, _)| scheme),
        Some("https" | "http" | "file" | "arcweft-cache")
    )
}

fn validate_profile_id(label: &str, value: Option<&str>) -> Result<(), ReleaseManifestError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_empty() {
        return Err(ReleaseManifestError::InvalidFetchPolicy(format!(
            "network_policy {label} must not be empty"
        )));
    }
    if value.len() > 128 {
        return Err(ReleaseManifestError::InvalidFetchPolicy(format!(
            "network_policy {label} must not exceed 128 bytes"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
    {
        return Err(ReleaseManifestError::InvalidFetchPolicy(format!(
            "network_policy {label} must contain only ASCII alphanumeric, '.', '_', '-', or ':' characters"
        )));
    }
    Ok(())
}

fn push_len_prefixed_bytes(out: &mut Vec<u8>, value: &[u8]) {
    let len = u32::try_from(value.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value);
}

fn decode_hex_array<const N: usize>(value: &str) -> Result<[u8; N], String> {
    let value = value.strip_prefix("ed25519:").unwrap_or(value).trim();
    if value.len() != N * 2 {
        return Err(format!(
            "expected {} hex characters, got {}",
            N * 2,
            value.len()
        ));
    }
    let mut bytes = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let hex = std::str::from_utf8(chunk).map_err(|error| error.to_string())?;
        bytes[index] = u8::from_str_radix(hex, 16)
            .map_err(|_| format!("invalid hex byte `{hex}` at offset {}", index * 2))?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::{
        BundleKind, BundleSectionKind, ContentResidency, SectionId, SectionInput, encode_bundle,
    };
    use ed25519_dalek::Signer as _;

    fn content_pack(bytes: &'static [u8]) -> Vec<u8> {
        encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::embedded(
                SectionId::from_bytes([1; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                bytes,
            )],
        )
        .expect("content pack encodes")
    }

    #[test]
    fn release_manifest_round_trips_external_awfb_reference() {
        let bundle = content_pack(b"voice-pack");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("https://cdn.example.test/content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        bundle_ref
            .verify_bytes(&bundle)
            .expect("referenced bytes match");
        let manifest = ReleaseManifest::new([bundle_ref.clone()]).expect("manifest");

        let bytes = manifest.to_json_bytes().expect("manifest encodes");
        let decoded = ReleaseManifest::from_json_slice(&bytes).expect("manifest decodes");

        assert_eq!(decoded.bundle(bundle_ref.content_root), Some(&bundle_ref));
    }

    #[test]
    fn release_manifest_builds_sorted_fetch_plan_and_verifies_result() {
        let bundle = content_pack(b"voice-pack");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [
                ReleaseMirror::with_priority("https://cdn.example.test/slow.awfb", 20)
                    .expect("slow mirror"),
                ReleaseMirror::with_priority("arcweft-cache:voice-pack", 0).expect("cache mirror"),
                ReleaseMirror::with_priority("file:voice-pack.awfb", 10).expect("file mirror"),
            ],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest::new([bundle_ref.clone()]).expect("manifest");

        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        assert_eq!(plan.content_root, bundle_ref.content_root);
        assert_eq!(plan.file_digest, bundle_ref.file_digest);
        assert_eq!(plan.mirrors[0].uri, "arcweft-cache:voice-pack");
        assert_eq!(plan.mirrors[1].uri, "file:voice-pack.awfb");
        assert_eq!(plan.mirrors[2].uri, "https://cdn.example.test/slow.awfb");
        plan.verify_bytes(&bundle).expect("fetched bytes verify");
    }

    #[test]
    fn release_manifest_fetch_plan_carries_retry_cancel_and_budget_policy() {
        let bundle = content_pack(b"voice-pack");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(3, bundle.len() as u64, Some(1_000))
                .expect("policy"),
            signature_policy: ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };

        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        assert_eq!(plan.fetch_policy.max_attempts_per_mirror, 3);
        assert_eq!(plan.fetch_policy.candidate_byte_budget, bundle.len() as u64);
        assert_eq!(plan.fetch_policy.cancel_after_millis, Some(1_000));
    }

    #[test]
    fn release_manifest_fetch_plan_carries_network_client_policy() {
        let bundle = content_pack(b"voice-pack");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("https://cdn.example.test/voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let network_policy = ReleaseNetworkFetchPolicy::require_https()
            .with_proxy_profile("corp-egress")
            .expect("proxy profile")
            .with_auth_profile("release-token")
            .expect("auth profile")
            .with_client_profile("strict-tls")
            .expect("client profile")
            .with_user_agent("arcweft-test-release/1")
            .expect("user agent");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(3, bundle.len() as u64, Some(1_000))
                .expect("policy")
                .with_network_policy(network_policy)
                .expect("network policy"),
            signature_policy: ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };

        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        assert!(plan.fetch_policy.network_policy.require_https);
        assert_eq!(
            plan.fetch_policy.network_policy.proxy_profile.as_deref(),
            Some("corp-egress")
        );
        assert_eq!(
            plan.fetch_policy.network_policy.auth_profile.as_deref(),
            Some("release-token")
        );
        assert_eq!(
            plan.fetch_policy.network_policy.client_profile.as_deref(),
            Some("strict-tls")
        );
        assert_eq!(
            plan.fetch_policy.network_policy.user_agent,
            "arcweft-test-release/1"
        );
    }

    #[test]
    fn release_manifest_rejects_invalid_network_policy_profiles() {
        let error = ReleaseNetworkFetchPolicy::default()
            .with_proxy_profile("bad profile")
            .expect_err("profile ids reject spaces");

        assert!(matches!(
            error,
            ReleaseManifestError::InvalidFetchPolicy(message)
                if message.contains("proxy_profile")
        ));
    }

    #[test]
    fn release_manifest_rejects_invalid_network_policy_user_agent() {
        let error = ReleaseNetworkFetchPolicy::default()
            .with_user_agent("bad\r\nheader")
            .expect_err("user agent rejects control characters");

        assert!(matches!(
            error,
            ReleaseManifestError::InvalidFetchPolicy(message)
                if message.contains("user_agent")
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_candidate_over_byte_budget() {
        let bundle = content_pack(b"voice-pack");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(1, (bundle.len() - 1) as u64, None)
                .expect("policy"),
            signature_policy: ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };

        let error = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect_err("oversized fetch candidate rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::CandidateByteBudgetExceeded { content_root, .. }
                if content_root == bundle_ref.content_root
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_missing_content_root() {
        let manifest = ReleaseManifest::new([ReleaseBundleRef::from_awfb_bytes(
            &content_pack(b"voice-pack"),
            [ReleaseMirror::new("arcweft-cache:voice-pack").expect("mirror")],
        )
        .expect("bundle ref")])
        .expect("manifest");
        let missing = BundleDigest::of(b"missing");

        let error = manifest
            .fetch_plan(missing)
            .expect_err("missing content root rejects");

        assert_eq!(error, ReleaseManifestError::MissingContentRoot(missing));
    }

    #[test]
    fn release_manifest_fetch_plan_enforces_required_awfb_signature() {
        let unsigned = content_pack(b"voice-pack");
        let signed = append_signature_block(unsigned.clone(), b"signature");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_signature(Some(8)).expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        plan.verify_bytes(&signed)
            .expect("signed bundle satisfies release policy");

        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let unsigned_manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_signature(None).expect("policy"),
            bundles: vec![unsigned_ref.clone()],
        };
        let unsigned_plan = unsigned_manifest
            .fetch_plan(unsigned_ref.content_root)
            .expect("unsigned fetch plan");

        let error = unsigned_plan
            .verify_bytes(&unsigned)
            .expect_err("unsigned bundle rejects when signature required");

        assert!(matches!(
            error,
            ReleaseManifestError::MissingAwfbSignature { content_root }
                if content_root == unsigned_ref.content_root
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_accepts_trusted_signature_envelope() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let envelope = signature_envelope(&unsigned, &unsigned_ref, "release-key-main")
            .expect("signature envelope")
            .to_json_bytes()
            .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_signers(
                Some(8),
                ["release-key-main"],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        plan.verify_bytes(&signed)
            .expect("trusted signer satisfies release policy");
    }

    #[test]
    fn release_manifest_fetch_plan_verifies_ed25519_signature_payload() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&signing_key.verifying_key().to_bytes()),
        )
        .expect("trusted public key");
        let envelope =
            ed25519_signature_envelope(&unsigned, &unsigned_ref, "release-key-main", &signing_key)
                .expect("signature envelope")
                .to_json_bytes()
                .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_public_keys(
                Some(64),
                [trusted_key],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        plan.verify_bytes(&signed)
            .expect("ed25519 signature satisfies release policy");
    }

    #[test]
    fn release_manifest_fetch_plan_accepts_rotated_ed25519_key_epoch() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let old_signing_key = ed25519_dalek::SigningKey::from_bytes(&[5; 32]);
        let new_signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let old_trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&old_signing_key.verifying_key().to_bytes()),
        )
        .expect("old trusted public key")
        .with_key_epoch_validity(0, Some(10))
        .expect("old key validity");
        let new_trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&new_signing_key.verifying_key().to_bytes()),
        )
        .expect("new trusted public key")
        .with_key_epoch_validity(10, None)
        .expect("new key validity");
        let envelope = ed25519_signature_envelope_at_epoch(
            &unsigned,
            &unsigned_ref,
            "release-key-main",
            12,
            &new_signing_key,
        )
        .expect("signature envelope")
        .to_json_bytes()
        .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_public_keys(
                Some(64),
                [old_trusted_key, new_trusted_key],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        plan.verify_bytes(&signed)
            .expect("rotated ed25519 key epoch satisfies release policy");
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_revoked_ed25519_key() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&signing_key.verifying_key().to_bytes()),
        )
        .expect("trusted public key")
        .revoked()
        .expect("revoked key is still a well-formed policy entry");
        let envelope =
            ed25519_signature_envelope(&unsigned, &unsigned_ref, "release-key-main", &signing_key)
                .expect("signature envelope")
                .to_json_bytes()
                .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_public_keys(
                Some(64),
                [trusted_key],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan.verify_bytes(&signed).expect_err("revoked key rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::NoValidTrustedPublicKey {
                content_root,
                signer_id,
                key_epoch,
                ..
            } if content_root == bundle_ref.content_root
                && signer_id == "release-key-main"
                && key_epoch == 0
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_ed25519_key_epoch_outside_validity() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&signing_key.verifying_key().to_bytes()),
        )
        .expect("trusted public key")
        .with_key_epoch_validity(10, None)
        .expect("trusted public key validity");
        let envelope = ed25519_signature_envelope_at_epoch(
            &unsigned,
            &unsigned_ref,
            "release-key-main",
            5,
            &signing_key,
        )
        .expect("signature envelope")
        .to_json_bytes()
        .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_public_keys(
                Some(64),
                [trusted_key],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan
            .verify_bytes(&signed)
            .expect_err("out-of-window key epoch rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::NoValidTrustedPublicKey {
                content_root,
                signer_id,
                key_epoch,
                ..
            } if content_root == bundle_ref.content_root
                && signer_id == "release-key-main"
                && key_epoch == 5
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_bad_ed25519_signature_payload() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let trusted_signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let other_signing_key = ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
        let trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&trusted_signing_key.verifying_key().to_bytes()),
        )
        .expect("trusted public key");
        let envelope = ed25519_signature_envelope(
            &unsigned,
            &unsigned_ref,
            "release-key-main",
            &other_signing_key,
        )
        .expect("signature envelope")
        .to_json_bytes()
        .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_public_keys(
                Some(64),
                [trusted_key],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan
            .verify_bytes(&signed)
            .expect_err("wrong ed25519 payload signature rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::SignatureVerificationFailed { content_root, signer_id }
                if content_root == bundle_ref.content_root && signer_id == "release-key-main"
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_untrusted_signature_envelope() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let envelope = signature_envelope(&unsigned, &unsigned_ref, "release-key-other")
            .expect("signature envelope")
            .to_json_bytes()
            .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_signers(
                None,
                ["release-key-main"],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan
            .verify_bytes(&signed)
            .expect_err("untrusted signer rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::UntrustedSigner { content_root, signer_id }
                if content_root == bundle_ref.content_root && signer_id == "release-key-other"
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_invalid_signature_envelope() {
        let signed = append_signature_block(content_pack(b"voice-pack"), b"not-json");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_signers(
                None,
                ["release-key-main"],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan
            .verify_bytes(&signed)
            .expect_err("invalid envelope rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::InvalidSignatureEnvelope { content_root, .. }
                if content_root == bundle_ref.content_root
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_signature_envelope_for_other_content_root() {
        let unsigned = content_pack(b"voice-pack");
        let other = content_pack(b"other-pack");
        let other_ref = ReleaseBundleRef::from_awfb_bytes(
            &other,
            [ReleaseMirror::new("file:other.awfb").expect("mirror")],
        )
        .expect("other bundle ref");
        let envelope = signature_envelope(&other, &other_ref, "release-key-main")
            .expect("signature envelope")
            .to_json_bytes()
            .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_signers(
                None,
                ["release-key-main"],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan
            .verify_bytes(&signed)
            .expect_err("envelope for another content root rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::SignatureContentRootMismatch { content_root, expected, actual }
                if content_root == bundle_ref.content_root
                    && expected == bundle_ref.content_root
                    && actual == other_ref.content_root
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_signature_envelope_with_wrong_signing_digest() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let envelope = ReleaseSignatureEnvelope::new(
            "release-key-main",
            RELEASE_SIGNATURE_ALGORITHM_ED25519_V1,
            unsigned_ref.content_root,
            unsigned_ref.kind,
            BundleDigest::of(b"wrong signing digest"),
            "sig",
        )
        .expect("signature envelope")
        .to_json_bytes()
        .expect("signature envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_signers(
                None,
                ["release-key-main"],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan
            .verify_bytes(&signed)
            .expect_err("wrong signing digest rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::SignatureDigestMismatch { content_root, expected, actual }
                if content_root == bundle_ref.content_root
                    && expected != actual
                    && actual == BundleDigest::of(b"wrong signing digest")
        ));
    }

    #[test]
    fn release_manifest_rejects_invalid_signature_policy() {
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy {
                require_awfb_signature: false,
                minimum_signature_bytes: Some(8),
                allowed_algorithms: default_release_signature_algorithms(),
                trusted_signer_ids: Vec::new(),
                trusted_public_keys: Vec::new(),
            },
            bundles: Vec::new(),
        };

        let error = manifest
            .to_json_bytes()
            .expect_err("minimum without required signature rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::InvalidSignaturePolicy(_)
        ));
    }

    #[test]
    fn release_manifest_rejects_unknown_signature_algorithm_policy() {
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy {
                require_awfb_signature: true,
                minimum_signature_bytes: None,
                allowed_algorithms: vec!["test-algorithm".to_owned()],
                trusted_signer_ids: Vec::new(),
                trusted_public_keys: Vec::new(),
            },
            bundles: Vec::new(),
        };

        let error = manifest
            .to_json_bytes()
            .expect_err("unknown signature algorithm policy rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::InvalidSignaturePolicy(message)
                if message.contains("unsupported signature algorithm `test-algorithm`")
        ));
    }

    #[test]
    fn release_manifest_fetch_plan_rejects_unsupported_signature_envelope_algorithm() {
        let unsigned = content_pack(b"voice-pack");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let signing_digest = BundleView::parse(&unsigned, ReadBudget::default())
            .expect("unsigned parses")
            .signing_digest()
            .expect("signing digest");
        let envelope = serde_json::json!({
            "schema_version": RELEASE_SIGNATURE_ENVELOPE_SCHEMA_VERSION,
            "signer_id": "release-key-main",
            "algorithm": "test-algorithm",
            "key_epoch": 0,
            "content_root": unsigned_ref.content_root,
            "kind": unsigned_ref.kind,
            "signing_digest": signing_digest,
            "signature": "sig"
        });
        let envelope = serde_json::to_vec(&envelope).expect("unsupported envelope encodes");
        let signed = append_signature_block(unsigned, &envelope);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &signed,
            [ReleaseMirror::new("file:voice-pack.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_signers(
                None,
                ["release-key-main"],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan");

        let error = plan
            .verify_bytes(&signed)
            .expect_err("unsupported signature envelope algorithm rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::InvalidSignatureEnvelope { content_root, message }
                if content_root == bundle_ref.content_root
                    && message.contains("unsupported signature algorithm `test-algorithm`")
        ));
    }

    #[test]
    fn release_manifest_rejects_invalid_trusted_key_epoch_window() {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let error = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&signing_key.verifying_key().to_bytes()),
        )
        .expect("trusted public key")
        .with_key_epoch_validity(10, Some(10))
        .expect_err("empty epoch window rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::InvalidSignaturePolicy(message)
                if message.contains("valid_until_key_epoch greater than valid_from_key_epoch")
        ));
    }

    #[test]
    fn release_manifest_rejects_duplicate_content_roots() {
        let bundle = content_pack(b"voice-pack");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("arcweft-cache:content").expect("mirror")],
        )
        .expect("bundle ref");

        let error = ReleaseManifest::new([bundle_ref.clone(), bundle_ref])
            .expect_err("duplicate content roots reject");

        assert!(matches!(
            error,
            ReleaseManifestError::DuplicateContentRoot(_)
        ));
    }

    #[test]
    fn release_manifest_rejects_unsupported_mirror_scheme() {
        let error = ReleaseMirror::new("ftp://cdn.example.test/content.awfb")
            .expect_err("unsupported scheme rejects");

        assert!(matches!(
            error,
            ReleaseManifestError::UnsupportedMirrorScheme(_)
        ));
    }

    #[test]
    fn release_bundle_ref_verifies_length_and_digest() {
        let bundle = content_pack(b"voice-pack");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let mut corrupted = bundle.clone();
        corrupted.push(0);

        let error = bundle_ref
            .verify_bytes(&corrupted)
            .expect_err("length mismatch rejects first");

        assert!(matches!(
            error,
            ReleaseManifestError::ByteLengthMismatch { .. }
        ));
    }

    fn append_signature_block(mut bytes: Vec<u8>, signature: &[u8]) -> Vec<u8> {
        let signature_offset = bytes.len();
        bytes.extend_from_slice(signature);
        write_u64(&mut bytes, 56, signature_offset as u64);
        write_u64(&mut bytes, 64, signature.len() as u64);
        let file_len = bytes.len() as u64;
        write_u64(&mut bytes, 72, file_len);
        bytes
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn signature_envelope(
        bytes: &[u8],
        bundle_ref: &ReleaseBundleRef,
        signer_id: &str,
    ) -> Result<ReleaseSignatureEnvelope, ReleaseManifestError> {
        let signing_digest = BundleView::parse(bytes, ReadBudget::default())
            .map_err(|error| ReleaseManifestError::DecodeAwfb(error.to_string()))?
            .signing_digest()
            .map_err(|error| ReleaseManifestError::DecodeAwfb(error.to_string()))?;
        ReleaseSignatureEnvelope::new(
            signer_id,
            RELEASE_SIGNATURE_ALGORITHM_ED25519_V1,
            bundle_ref.content_root,
            bundle_ref.kind,
            signing_digest,
            "sig",
        )
    }

    fn ed25519_signature_envelope(
        bytes: &[u8],
        bundle_ref: &ReleaseBundleRef,
        signer_id: &str,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<ReleaseSignatureEnvelope, ReleaseManifestError> {
        ed25519_signature_envelope_at_epoch(bytes, bundle_ref, signer_id, 0, signing_key)
    }

    fn ed25519_signature_envelope_at_epoch(
        bytes: &[u8],
        bundle_ref: &ReleaseBundleRef,
        signer_id: &str,
        key_epoch: u64,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<ReleaseSignatureEnvelope, ReleaseManifestError> {
        let signing_digest = BundleView::parse(bytes, ReadBudget::default())
            .map_err(|error| ReleaseManifestError::DecodeAwfb(error.to_string()))?
            .signing_digest()
            .map_err(|error| ReleaseManifestError::DecodeAwfb(error.to_string()))?;
        let mut envelope = ReleaseSignatureEnvelope::new(
            signer_id,
            RELEASE_SIGNATURE_ALGORITHM_ED25519_V1,
            bundle_ref.content_root,
            bundle_ref.kind,
            signing_digest,
            encode_hex(&[0; 64]),
        )?;
        envelope.key_epoch = key_epoch;
        let signature = signing_key.sign(&envelope.signing_message());
        envelope.signature = encode_hex(&signature.to_bytes());
        Ok(envelope)
    }

    fn encode_hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
                use std::fmt::Write as _;
                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }
}
