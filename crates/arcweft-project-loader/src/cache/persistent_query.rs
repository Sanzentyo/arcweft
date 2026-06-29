//! Typed read-through for compiler-private persistent query objects.
//!
//! Read-through is deliberately adapter-owned: every local-cache absence,
//! staleness, corruption, or mismatch is returned as typed soft-miss evidence so
//! callers can rebuild from source instead of poisoning the build.

use super::{
    record::CacheRecord,
    store::{CacheStoreError, FilesystemCacheStore},
};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKind},
    fingerprint::{BuildDigest, NamedDigest},
    incremental::{CacheRecordStatus, InvalidationReason, QueryKind},
    persistent_object::{
        AWBO_SCHEMA_VERSION, AwboEnvelope, AwboError, BytecodeUnitObject, CompilerBuildIdentity,
        CompilerIdentityNamespaceObject, CompilerObjectKey, CompilerObjectKind,
        CompilerObjectPayload, CompilerStageInputsObject, HirBodyObject, InterfaceSummaryObject,
        LinkPlanObject, ParsedSyntaxObject, TypecheckGateObject, TypecheckGateReusePolicy,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// One adapter-owned persistent query read-through request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryReadRequest {
    pub query: QueryKind,
    pub artifact_key: ArtifactKey,
    pub object_key: CompilerObjectKey,
}

/// One adapter-owned persistent query write-through request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryWriteRequest {
    pub query: QueryKind,
    pub artifact_key: ArtifactKey,
    pub object_key: CompilerObjectKey,
    pub logical_item: String,
    pub payload: CompilerObjectPayload,
}

/// Read-through hit or recoverable soft miss.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum PersistentQueryReadOutcome {
    Hit(Box<PersistentQueryHit>),
    Miss(Box<PersistentQueryMiss>),
}

/// Successful read-through evidence and reusable payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryHit {
    pub query: QueryKind,
    pub artifact_key: ArtifactKey,
    pub artifact_kind: ArtifactKind,
    pub object_kind: CompilerObjectKind,
    pub record_path: PathBuf,
    pub object_path: PathBuf,
    pub object_digest: BuildDigest,
    pub object_len: u64,
    pub payload_digest: BuildDigest,
    pub payload_len: u64,
    pub record: CacheRecord,
    pub payload: PersistentQueryHitPayload,
}

/// Safe payloads enabled for persistent query read-through.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PersistentQueryHitPayload {
    ParsedSyntax(ParsedSyntaxObject),
    InterfaceSummary(InterfaceSummaryObject),
    HirBody(HirBodyObject),
    TypecheckGate(TypecheckGateObject),
    BytecodeUnit(BytecodeUnitObject),
    LinkPlan(LinkPlanObject),
}

/// Successful write-through evidence for one persistent compiler query object.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryWriteReceipt {
    pub query: QueryKind,
    pub artifact_key: ArtifactKey,
    pub artifact_kind: ArtifactKind,
    pub object_kind: CompilerObjectKind,
    pub logical_item: String,
    pub record_path: PathBuf,
    pub object_path: PathBuf,
    pub object_digest: BuildDigest,
    pub object_len: u64,
    pub payload_digest: BuildDigest,
    pub payload_len: u64,
    pub record: CacheRecord,
}

/// Cache-explain evidence for one persistent compiler query record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryExplainEvidence {
    pub query: QueryKind,
    pub artifact_key: String,
    pub object_kind: CompilerObjectKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_inputs: Option<PersistentQueryKeyInputEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_identity: Option<CompilerBuildIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_kind: Option<CompilerObjectKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_schema_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_schema_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_schema_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_len: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_object_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_object_len: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_object_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_object_len: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_len: Option<u64>,
    pub status: PersistentQueryExplainStatus,
    pub cache_record_status: CacheRecordStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_miss_reason: Option<PersistentQueryMissReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typecheck_gate_reuse_policy: Option<TypecheckGateReusePolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conservative_reuse_policy: Option<String>,
    pub recovery_action: PersistentQueryRecoveryAction,
}

/// Canonical key inputs surfaced by cache explain for persistent query records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryKeyInputEvidence {
    pub query_options_digest: String,
    pub dependency_interface_digests: Vec<PersistentQueryNamedDigestEvidence>,
    pub dependency_body_digests: Vec<PersistentQueryNamedDigestEvidence>,
    pub environment_digest: String,
}

/// Named dependency digest evidence in canonical order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryNamedDigestEvidence {
    pub name: String,
    pub digest: String,
}

/// Persistent query cache-explain hit/miss status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistentQueryExplainStatus {
    Hit,
    Miss,
}

/// Recommended recovery action for a persistent query explain result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistentQueryRecoveryAction {
    NoneRequired,
    RebuildFromSource,
}

/// Recoverable soft-miss evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryMiss {
    pub query: QueryKind,
    pub artifact_key: ArtifactKey,
    pub object_kind: CompilerObjectKind,
    pub record_path: PathBuf,
    pub object_path: Option<PathBuf>,
    pub record_object_digest: Option<BuildDigest>,
    pub record_object_len: Option<u64>,
    pub observed_object_digest: Option<BuildDigest>,
    pub observed_object_len: Option<u64>,
    pub reason: PersistentQueryMissReason,
}

/// All read-through failures that must remain recoverable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PersistentQueryMissReason {
    UnsupportedObjectKind {
        object_kind: CompilerObjectKind,
    },
    QueryKindMismatch {
        expected: QueryKind,
        actual: QueryKind,
    },
    MissingRecord,
    RecordReadFailed {
        io_kind: PersistentQueryIoKind,
        message: String,
    },
    CorruptRecord {
        message: String,
    },
    RecordSchemaMismatch {
        actual: u32,
        expected: u32,
    },
    RecordKeyMismatch,
    ArtifactKindMismatch {
        expected: ArtifactKind,
        actual: ArtifactKind,
    },
    MissingObject {
        object_digest: BuildDigest,
    },
    ObjectReadFailed {
        object_digest: BuildDigest,
        io_kind: PersistentQueryIoKind,
        message: String,
    },
    ObjectDigestMismatch {
        expected: BuildDigest,
        actual: BuildDigest,
    },
    ObjectLengthMismatch {
        expected: u64,
        actual: u64,
    },
    CorruptObject {
        message: String,
    },
    ObjectSchemaMismatch {
        actual: u32,
        expected: u32,
    },
    ObjectKindMismatch {
        expected: CompilerObjectKind,
        actual: CompilerObjectKind,
    },
    ObjectStabilityMismatch {
        object_kind: CompilerObjectKind,
    },
    PayloadKindMismatch {
        expected: CompilerObjectKind,
        actual: CompilerObjectKind,
    },
    PayloadSchemaMismatch {
        actual: u32,
        expected: u32,
    },
    PayloadDigestMismatch,
    PayloadLengthMismatch {
        expected: u64,
        actual: u64,
    },
    CompilerIdentityMismatch {
        expected: Box<CompilerBuildIdentity>,
        actual: Box<CompilerBuildIdentity>,
    },
    SourceDigestMismatch {
        expected: BuildDigest,
        actual: BuildDigest,
    },
    QueryOptionsDigestMismatch {
        expected: BuildDigest,
        actual: BuildDigest,
    },
    EnvironmentDigestMismatch {
        expected: BuildDigest,
        actual: BuildDigest,
    },
    DependencyInterfaceDigestMismatch {
        expected: Vec<NamedDigest>,
        actual: Vec<NamedDigest>,
    },
    DependencyBodyDigestMismatch {
        expected: Vec<NamedDigest>,
        actual: Vec<NamedDigest>,
    },
    KeyDigestMismatch {
        expected: BuildDigest,
        actual: BuildDigest,
    },
}

/// Serializable IO class for read-through evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistentQueryIoKind {
    NotFound,
    PermissionDenied,
    UnexpectedEof,
    Interrupted,
    InvalidData,
    Other,
}

/// Hard failures while writing a deterministic persistent query object.
#[derive(Debug, Error)]
pub enum PersistentQueryWriteError {
    #[error("persistent query write-through does not support object kind {object_kind:?}")]
    UnsupportedObjectKind { object_kind: CompilerObjectKind },
    #[error("persistent query write-through expected query {expected:?}, got {actual:?}")]
    QueryKindMismatch {
        expected: QueryKind,
        actual: QueryKind,
    },
    #[error(
        "persistent query write-through payload kind {payload:?} does not match key kind {key:?}"
    )]
    PayloadKindMismatch {
        key: CompilerObjectKind,
        payload: CompilerObjectKind,
    },
    #[error(transparent)]
    Awbo(#[from] AwboError),
    #[error(transparent)]
    Store(#[from] CacheStoreError),
}

impl PersistentQueryReadRequest {
    pub fn new(query: QueryKind, artifact_key: ArtifactKey, object_key: CompilerObjectKey) -> Self {
        Self {
            query,
            artifact_key,
            object_key,
        }
    }
}

impl PersistentQueryWriteRequest {
    pub fn new(
        query: QueryKind,
        artifact_key: ArtifactKey,
        object_key: CompilerObjectKey,
        logical_item: impl Into<String>,
        payload: CompilerObjectPayload,
    ) -> Self {
        Self {
            query,
            artifact_key,
            object_key,
            logical_item: logical_item.into(),
            payload,
        }
    }
}

impl PersistentQueryReadOutcome {
    pub const fn is_hit(&self) -> bool {
        matches!(self, Self::Hit(_))
    }

    pub fn cache_record_status(&self) -> CacheRecordStatus {
        match self {
            Self::Hit(hit) => hit.object_kind.read_through_hit_status(),
            Self::Miss(miss) => CacheRecordStatus::Miss {
                reason: miss.reason.invalidation_reason(),
            },
        }
    }
}

impl PersistentQueryMissReason {
    pub fn invalidation_reason(&self) -> InvalidationReason {
        match self {
            Self::MissingRecord => InvalidationReason::MissingRecord,
            Self::RecordSchemaMismatch { .. }
            | Self::ObjectSchemaMismatch { .. }
            | Self::PayloadSchemaMismatch { .. } => InvalidationReason::CacheSchemaChanged,
            Self::CompilerIdentityMismatch { .. } => InvalidationReason::CompilerChanged,
            Self::SourceDigestMismatch { .. } => InvalidationReason::SourceChanged,
            Self::QueryOptionsDigestMismatch { .. } => InvalidationReason::OptionsChanged,
            Self::EnvironmentDigestMismatch { .. } => InvalidationReason::EnvironmentChanged,
            Self::DependencyInterfaceDigestMismatch { expected, actual } => {
                dependency_interface_mismatch_invalidation_reason(expected, actual)
            }
            Self::DependencyBodyDigestMismatch { expected, actual } => {
                dependency_body_mismatch_invalidation_reason(expected, actual)
            }
            Self::UnsupportedObjectKind { .. }
            | Self::QueryKindMismatch { .. }
            | Self::RecordReadFailed { .. }
            | Self::CorruptRecord { .. }
            | Self::RecordKeyMismatch
            | Self::ArtifactKindMismatch { .. } => InvalidationReason::CorruptRecord,
            Self::MissingObject { .. }
            | Self::ObjectReadFailed { .. }
            | Self::ObjectDigestMismatch { .. }
            | Self::ObjectLengthMismatch { .. }
            | Self::CorruptObject { .. }
            | Self::ObjectKindMismatch { .. }
            | Self::ObjectStabilityMismatch { .. }
            | Self::PayloadKindMismatch { .. }
            | Self::PayloadDigestMismatch
            | Self::PayloadLengthMismatch { .. }
            | Self::KeyDigestMismatch { .. } => InvalidationReason::CorruptObject,
        }
    }
}

fn dependency_interface_mismatch_invalidation_reason(
    expected: &[NamedDigest],
    actual: &[NamedDigest],
) -> InvalidationReason {
    first_changed_dependency_name(expected, actual)
        .map_or(InvalidationReason::InterfaceChanged, |module| {
            InvalidationReason::DependencyInterfaceChanged { module }
        })
}

fn dependency_body_mismatch_invalidation_reason(
    expected: &[NamedDigest],
    actual: &[NamedDigest],
) -> InvalidationReason {
    first_changed_dependency_name(expected, actual)
        .map_or(InvalidationReason::BodyChanged, |module| {
            InvalidationReason::DependencyBodyChanged { module }
        })
}

fn first_changed_dependency_name(
    expected: &[NamedDigest],
    actual: &[NamedDigest],
) -> Option<String> {
    let actual_by_name = actual
        .iter()
        .map(|value| (value.name(), value.digest()))
        .collect::<BTreeMap<_, _>>();
    expected
        .iter()
        .find(|expected| match actual_by_name.get(expected.name()) {
            Some(actual_digest) => *actual_digest != expected.digest(),
            None => true,
        })
        .map(|value| value.name().to_owned())
        .or_else(|| {
            let expected_by_name = expected
                .iter()
                .map(|value| (value.name(), value.digest()))
                .collect::<BTreeMap<_, _>>();
            actual
                .iter()
                .find(|actual| !expected_by_name.contains_key(actual.name()))
                .map(|value| value.name().to_owned())
        })
}

impl From<ErrorKind> for PersistentQueryIoKind {
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::NotFound => Self::NotFound,
            ErrorKind::PermissionDenied => Self::PermissionDenied,
            ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            ErrorKind::Interrupted => Self::Interrupted,
            ErrorKind::InvalidData => Self::InvalidData,
            _ => Self::Other,
        }
    }
}

impl FilesystemCacheStore {
    /// Reads and validates a parse/interface/HIR persistent compiler object.
    pub fn read_persistent_query(
        &self,
        request: &PersistentQueryReadRequest,
    ) -> PersistentQueryReadOutcome {
        self.read_persistent_query_checked(request)
    }

    /// Explains a decoded cache record as a persistent compiler query when supported.
    pub fn explain_persistent_query_record(
        &self,
        query: QueryKind,
        record: &CacheRecord,
    ) -> Option<PersistentQueryExplainEvidence> {
        let object_kind =
            CompilerObjectKind::from_safe_read_through_artifact_kind(record.artifact_kind())?;
        let object_path = self.object_path(record.object_digest());
        let object_bytes = match fs::read(&object_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Some(explain_persistent_query_preflight_miss(
                    query,
                    record,
                    object_kind,
                    None,
                    None,
                    PersistentQueryMissReason::MissingObject {
                        object_digest: record.object_digest(),
                    },
                ));
            }
            Err(error) => {
                return Some(explain_persistent_query_preflight_miss(
                    query,
                    record,
                    object_kind,
                    None,
                    None,
                    PersistentQueryMissReason::ObjectReadFailed {
                        object_digest: record.object_digest(),
                        io_kind: error.kind().into(),
                        message: error.to_string(),
                    },
                ));
            }
        };
        let observed_len = len_u64(&object_bytes);
        if observed_len != record.object_len() {
            return Some(explain_persistent_query_preflight_miss(
                query,
                record,
                object_kind,
                None,
                Some(observed_len),
                PersistentQueryMissReason::ObjectLengthMismatch {
                    expected: record.object_len(),
                    actual: observed_len,
                },
            ));
        }
        let observed_digest = BuildDigest::of(&object_bytes);
        if observed_digest != record.object_digest() {
            return Some(explain_persistent_query_preflight_miss(
                query,
                record,
                object_kind,
                Some(observed_digest),
                Some(observed_len),
                PersistentQueryMissReason::ObjectDigestMismatch {
                    expected: record.object_digest(),
                    actual: observed_digest,
                },
            ));
        }
        let envelope = match AwboEnvelope::decode_detached(&object_bytes) {
            Ok(envelope) => envelope,
            Err(error) => {
                return Some(explain_persistent_query_preflight_miss(
                    query,
                    record,
                    object_kind,
                    Some(observed_digest),
                    Some(observed_len),
                    awbo_error_reason(&error),
                ));
            }
        };
        let Some(object_key) = object_key_from_envelope(&envelope) else {
            return Some(explain_persistent_query_preflight_miss(
                query,
                record,
                object_kind,
                Some(observed_digest),
                Some(observed_len),
                PersistentQueryMissReason::PayloadKindMismatch {
                    expected: object_kind,
                    actual: envelope.payload.kind(),
                },
            ));
        };
        let request = PersistentQueryReadRequest::new(query, record.key(), object_key.clone());
        Some(explain_persistent_query_outcome(
            &self.read_persistent_query(&request),
            Some(record),
            Some(&object_key),
            Some(&envelope),
        ))
    }

    /// Writes a deterministic parse/interface/HIR `.awbo` object and key-addressed record.
    pub fn write_persistent_query(
        &self,
        request: &PersistentQueryWriteRequest,
    ) -> Result<PersistentQueryWriteReceipt, PersistentQueryWriteError> {
        let Some(expected_query) = request.object_key.kind.safe_read_through_query_kind() else {
            return Err(PersistentQueryWriteError::UnsupportedObjectKind {
                object_kind: request.object_key.kind,
            });
        };
        if request.query != expected_query {
            return Err(PersistentQueryWriteError::QueryKindMismatch {
                expected: expected_query,
                actual: request.query,
            });
        }
        let Some(artifact_kind) = request.object_key.kind.safe_read_through_artifact_kind() else {
            return Err(PersistentQueryWriteError::UnsupportedObjectKind {
                object_kind: request.object_key.kind,
            });
        };
        let payload_kind = request.payload.kind();
        if payload_kind != request.object_key.kind {
            return Err(PersistentQueryWriteError::PayloadKindMismatch {
                key: request.object_key.kind,
                payload: payload_kind,
            });
        }

        let envelope = AwboEnvelope::new(&request.object_key, request.payload.clone())?;
        let payload_digest = envelope.payload_digest;
        let payload_len = envelope.payload_len;
        let bytes = envelope.encode()?;
        let record = self.store_artifact_with_logical_item(
            request.query,
            request.artifact_key,
            artifact_kind,
            Some(request.logical_item.as_str()),
            &bytes,
        )?;
        Ok(PersistentQueryWriteReceipt {
            query: request.query,
            artifact_key: request.artifact_key,
            artifact_kind,
            object_kind: request.object_key.kind,
            logical_item: request.logical_item.clone(),
            record_path: self.record_path(request.query, request.artifact_key),
            object_path: self.object_path(record.object_digest()),
            object_digest: record.object_digest(),
            object_len: record.object_len(),
            payload_digest,
            payload_len,
            record,
        })
    }

    fn read_persistent_query_checked(
        &self,
        request: &PersistentQueryReadRequest,
    ) -> PersistentQueryReadOutcome {
        let record_path = self.record_path(request.query, request.artifact_key);
        let Some(expected_query) = request.object_key.kind.safe_read_through_query_kind() else {
            return miss(
                request,
                record_path,
                None,
                PersistentQueryMissReason::UnsupportedObjectKind {
                    object_kind: request.object_key.kind,
                },
            );
        };
        if request.query != expected_query {
            return miss(
                request,
                record_path,
                None,
                PersistentQueryMissReason::QueryKindMismatch {
                    expected: expected_query,
                    actual: request.query,
                },
            );
        }
        let Some(expected_artifact_kind) =
            request.object_key.kind.safe_read_through_artifact_kind()
        else {
            return miss(
                request,
                record_path,
                None,
                PersistentQueryMissReason::UnsupportedObjectKind {
                    object_kind: request.object_key.kind,
                },
            );
        };

        let record =
            match Self::read_persistent_query_record(request, &record_path, expected_artifact_kind)
            {
                Ok(record) => record,
                Err(outcome) => return outcome,
            };
        let (object_bytes, evidence, observed_digest, observed_len) =
            match self.read_persistent_query_object(request, &record_path, &record) {
                Ok(object) => object,
                Err(outcome) => return outcome,
            };
        let envelope = match Self::decode_persistent_query_envelope(
            request,
            &record_path,
            evidence.clone(),
            &object_bytes,
        ) {
            Ok(envelope) => envelope,
            Err(outcome) => return outcome,
        };
        let payload =
            match persistent_query_hit_payload(request, &record_path, &evidence, &envelope) {
                Ok(payload) => payload,
                Err(outcome) => return outcome,
            };

        PersistentQueryReadOutcome::Hit(Box::new(PersistentQueryHit {
            query: request.query,
            artifact_key: request.artifact_key,
            artifact_kind: expected_artifact_kind,
            object_kind: request.object_key.kind,
            record_path,
            object_path: evidence.object_path.expect("hit has object path"),
            object_digest: observed_digest,
            object_len: observed_len,
            payload_digest: envelope.payload_digest,
            payload_len: envelope.payload_len,
            record,
            payload,
        }))
    }

    fn read_persistent_query_record(
        request: &PersistentQueryReadRequest,
        record_path: &Path,
        expected_artifact_kind: ArtifactKind,
    ) -> Result<CacheRecord, PersistentQueryReadOutcome> {
        let record_bytes = read_persistent_query_record_bytes(request, record_path)?;
        let record = CacheRecord::from_slice_for_key(request.artifact_key, &record_bytes).map_err(
            |error| {
                miss(
                    request,
                    record_path.to_path_buf(),
                    None,
                    record_error_reason(error),
                )
            },
        )?;
        if record.artifact_kind() != expected_artifact_kind {
            return Err(miss(
                request,
                record_path.to_path_buf(),
                Some(ReadObjectEvidence::from_record(&record)),
                PersistentQueryMissReason::ArtifactKindMismatch {
                    expected: expected_artifact_kind,
                    actual: record.artifact_kind(),
                },
            ));
        }
        Ok(record)
    }

    fn read_persistent_query_object(
        &self,
        request: &PersistentQueryReadRequest,
        record_path: &Path,
        record: &CacheRecord,
    ) -> Result<(Vec<u8>, ReadObjectEvidence, BuildDigest, u64), PersistentQueryReadOutcome> {
        let object_path = self.object_path(record.object_digest());
        let object_bytes = match fs::read(&object_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(miss(
                    request,
                    record_path.to_path_buf(),
                    Some(ReadObjectEvidence::from_record_path(record, object_path)),
                    PersistentQueryMissReason::MissingObject {
                        object_digest: record.object_digest(),
                    },
                ));
            }
            Err(error) => {
                return Err(miss(
                    request,
                    record_path.to_path_buf(),
                    Some(ReadObjectEvidence::from_record_path(record, object_path)),
                    PersistentQueryMissReason::ObjectReadFailed {
                        object_digest: record.object_digest(),
                        io_kind: error.kind().into(),
                        message: error.to_string(),
                    },
                ));
            }
        };
        let observed_len = len_u64(&object_bytes);
        let evidence = ReadObjectEvidence::from_record_path(record, object_path)
            .with_observed_len(observed_len);
        if observed_len != record.object_len() {
            return Err(miss(
                request,
                record_path.to_path_buf(),
                Some(evidence),
                PersistentQueryMissReason::ObjectLengthMismatch {
                    expected: record.object_len(),
                    actual: observed_len,
                },
            ));
        }
        let observed_digest = BuildDigest::of(&object_bytes);
        let evidence = evidence.with_observed_digest(observed_digest);
        if observed_digest != record.object_digest() {
            return Err(miss(
                request,
                record_path.to_path_buf(),
                Some(evidence),
                PersistentQueryMissReason::ObjectDigestMismatch {
                    expected: record.object_digest(),
                    actual: observed_digest,
                },
            ));
        }
        Ok((object_bytes, evidence, observed_digest, observed_len))
    }

    fn decode_persistent_query_envelope(
        request: &PersistentQueryReadRequest,
        record_path: &Path,
        evidence: ReadObjectEvidence,
        object_bytes: &[u8],
    ) -> Result<AwboEnvelope, PersistentQueryReadOutcome> {
        let envelope = AwboEnvelope::decode_detached(object_bytes).map_err(|error| {
            miss(
                request,
                record_path.to_path_buf(),
                Some(evidence.clone()),
                awbo_error_reason(&error),
            )
        })?;
        validate_envelope_for_request(&envelope, &request.object_key)
            .map_err(|reason| miss(request, record_path.to_path_buf(), Some(evidence), reason))?;
        Ok(envelope)
    }
}

fn read_persistent_query_record_bytes(
    request: &PersistentQueryReadRequest,
    record_path: &Path,
) -> Result<Vec<u8>, PersistentQueryReadOutcome> {
    match fs::read(record_path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(miss(
            request,
            record_path.to_path_buf(),
            None,
            PersistentQueryMissReason::MissingRecord,
        )),
        Err(error) => Err(miss(
            request,
            record_path.to_path_buf(),
            None,
            PersistentQueryMissReason::RecordReadFailed {
                io_kind: error.kind().into(),
                message: error.to_string(),
            },
        )),
    }
}

fn persistent_query_hit_payload(
    request: &PersistentQueryReadRequest,
    record_path: &Path,
    evidence: &ReadObjectEvidence,
    envelope: &AwboEnvelope,
) -> Result<PersistentQueryHitPayload, PersistentQueryReadOutcome> {
    match &envelope.payload {
        CompilerObjectPayload::ParsedSyntax(payload) => {
            Ok(PersistentQueryHitPayload::ParsedSyntax(payload.clone()))
        }
        CompilerObjectPayload::InterfaceSummary(payload) => {
            Ok(PersistentQueryHitPayload::InterfaceSummary(payload.clone()))
        }
        CompilerObjectPayload::HirBody(payload) => {
            Ok(PersistentQueryHitPayload::HirBody(payload.clone()))
        }
        CompilerObjectPayload::TypecheckGate(payload) => {
            Ok(PersistentQueryHitPayload::TypecheckGate(payload.clone()))
        }
        CompilerObjectPayload::BytecodeUnit(payload) => {
            Ok(PersistentQueryHitPayload::BytecodeUnit(payload.clone()))
        }
        CompilerObjectPayload::LinkPlan(payload) => {
            Ok(PersistentQueryHitPayload::LinkPlan(payload.clone()))
        }
        other => Err(miss(
            request,
            record_path.to_path_buf(),
            Some(evidence.clone()),
            PersistentQueryMissReason::PayloadKindMismatch {
                expected: request.object_key.kind,
                actual: other.kind(),
            },
        )),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReadObjectEvidence {
    object_path: Option<PathBuf>,
    record_object_digest: BuildDigest,
    record_object_len: u64,
    observed_object_digest: Option<BuildDigest>,
    observed_object_len: Option<u64>,
}

impl ReadObjectEvidence {
    fn from_record(record: &CacheRecord) -> Self {
        Self {
            object_path: None,
            record_object_digest: record.object_digest(),
            record_object_len: record.object_len(),
            observed_object_digest: None,
            observed_object_len: None,
        }
    }

    fn from_record_path(record: &CacheRecord, object_path: PathBuf) -> Self {
        Self {
            object_path: Some(object_path),
            ..Self::from_record(record)
        }
    }

    fn with_observed_digest(mut self, digest: BuildDigest) -> Self {
        self.observed_object_digest = Some(digest);
        self
    }

    fn with_observed_len(mut self, len: u64) -> Self {
        self.observed_object_len = Some(len);
        self
    }
}

fn miss(
    request: &PersistentQueryReadRequest,
    record_path: PathBuf,
    object: Option<ReadObjectEvidence>,
    reason: PersistentQueryMissReason,
) -> PersistentQueryReadOutcome {
    let (
        object_path,
        record_object_digest,
        record_object_len,
        observed_object_digest,
        observed_object_len,
    ) = object.map_or((None, None, None, None, None), |object| {
        (
            object.object_path,
            Some(object.record_object_digest),
            Some(object.record_object_len),
            object.observed_object_digest,
            object.observed_object_len,
        )
    });
    PersistentQueryReadOutcome::Miss(Box::new(PersistentQueryMiss {
        query: request.query,
        artifact_key: request.artifact_key,
        object_kind: request.object_key.kind,
        record_path,
        object_path,
        record_object_digest,
        record_object_len,
        observed_object_digest,
        observed_object_len,
        reason,
    }))
}

fn explain_persistent_query_outcome(
    outcome: &PersistentQueryReadOutcome,
    record: Option<&CacheRecord>,
    object_key: Option<&CompilerObjectKey>,
    envelope: Option<&AwboEnvelope>,
) -> PersistentQueryExplainEvidence {
    match outcome {
        PersistentQueryReadOutcome::Hit(hit) => PersistentQueryExplainEvidence {
            query: hit.query,
            artifact_key: hit.artifact_key.digest().to_hex(),
            object_kind: hit.object_kind,
            query_key: object_key.map(|key| key.digest().to_hex()),
            key_inputs: object_key.map(key_input_evidence),
            compiler_identity: object_key.map(|key| key.compiler.clone().canonicalized()),
            source_digest: object_key.map(|key| key.source_digest.to_hex()),
            payload_kind: Some(hit.object_kind),
            record_schema_version: Some(hit.record.schema_version()),
            object_schema_version: envelope.map(|value| value.schema_version),
            payload_schema_version: envelope
                .and_then(|value| payload_schema_version(&value.payload)),
            object_digest: Some(hit.object_digest.to_hex()),
            object_len: Some(hit.object_len),
            record_object_digest: Some(hit.record.object_digest().to_hex()),
            record_object_len: Some(hit.record.object_len()),
            observed_object_digest: Some(hit.object_digest.to_hex()),
            observed_object_len: Some(hit.object_len),
            payload_digest: Some(hit.payload_digest.to_hex()),
            payload_len: Some(hit.payload_len),
            status: PersistentQueryExplainStatus::Hit,
            cache_record_status: hit.object_kind.read_through_hit_status(),
            soft_miss_reason: None,
            typecheck_gate_reuse_policy: envelope.and_then(typecheck_gate_reuse_policy),
            conservative_reuse_policy: hit
                .object_kind
                .conservative_read_through_policy()
                .map(str::to_owned),
            recovery_action: if hit.object_kind.read_through_hit_requires_rebuild() {
                PersistentQueryRecoveryAction::RebuildFromSource
            } else {
                PersistentQueryRecoveryAction::NoneRequired
            },
        },
        PersistentQueryReadOutcome::Miss(miss) => {
            let reason = miss.reason.clone();
            PersistentQueryExplainEvidence {
                query: miss.query,
                artifact_key: miss.artifact_key.digest().to_hex(),
                object_kind: miss.object_kind,
                query_key: object_key.map(|key| key.digest().to_hex()),
                key_inputs: object_key.map(key_input_evidence),
                compiler_identity: object_key.map(|key| key.compiler.clone().canonicalized()),
                source_digest: object_key.map(|key| key.source_digest.to_hex()),
                payload_kind: envelope.map(|value| value.payload.kind()),
                record_schema_version: record.map(CacheRecord::schema_version),
                object_schema_version: envelope.map(|value| value.schema_version),
                payload_schema_version: envelope
                    .and_then(|value| payload_schema_version(&value.payload)),
                object_digest: miss
                    .record_object_digest
                    .or(miss.observed_object_digest)
                    .map(BuildDigest::to_hex),
                object_len: miss.record_object_len.or(miss.observed_object_len),
                record_object_digest: miss.record_object_digest.map(BuildDigest::to_hex),
                record_object_len: miss.record_object_len,
                observed_object_digest: miss.observed_object_digest.map(BuildDigest::to_hex),
                observed_object_len: miss.observed_object_len,
                payload_digest: envelope.map(|value| value.payload_digest.to_hex()),
                payload_len: envelope.map(|value| value.payload_len),
                status: PersistentQueryExplainStatus::Miss,
                cache_record_status: CacheRecordStatus::Miss {
                    reason: reason.invalidation_reason(),
                },
                soft_miss_reason: Some(reason),
                typecheck_gate_reuse_policy: envelope.and_then(typecheck_gate_reuse_policy),
                conservative_reuse_policy: miss
                    .object_kind
                    .conservative_read_through_policy()
                    .map(str::to_owned),
                recovery_action: PersistentQueryRecoveryAction::RebuildFromSource,
            }
        }
    }
}

fn explain_persistent_query_preflight_miss(
    query: QueryKind,
    record: &CacheRecord,
    object_kind: CompilerObjectKind,
    observed_digest: Option<BuildDigest>,
    observed_len: Option<u64>,
    reason: PersistentQueryMissReason,
) -> PersistentQueryExplainEvidence {
    let status = CacheRecordStatus::Miss {
        reason: reason.invalidation_reason(),
    };
    PersistentQueryExplainEvidence {
        query,
        artifact_key: record.key().digest().to_hex(),
        object_kind,
        query_key: None,
        key_inputs: None,
        compiler_identity: None,
        source_digest: None,
        payload_kind: None,
        record_schema_version: Some(record.schema_version()),
        object_schema_version: None,
        payload_schema_version: None,
        object_digest: Some(record.object_digest().to_hex()),
        object_len: Some(record.object_len()),
        record_object_digest: Some(record.object_digest().to_hex()),
        record_object_len: Some(record.object_len()),
        observed_object_digest: observed_digest.map(BuildDigest::to_hex),
        observed_object_len: observed_len,
        payload_digest: None,
        payload_len: None,
        status: PersistentQueryExplainStatus::Miss,
        cache_record_status: status,
        soft_miss_reason: Some(reason),
        typecheck_gate_reuse_policy: None,
        conservative_reuse_policy: object_kind
            .conservative_read_through_policy()
            .map(str::to_owned),
        recovery_action: PersistentQueryRecoveryAction::RebuildFromSource,
    }
}

fn object_key_from_envelope(envelope: &AwboEnvelope) -> Option<CompilerObjectKey> {
    let key = match &envelope.payload {
        CompilerObjectPayload::ParsedSyntax(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::ParsedSyntax,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::InterfaceSummary(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::InterfaceSummary,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::HirBody(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::HirBody,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::TypecheckGate(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::TypecheckGate,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::BytecodeUnit(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::BytecodeUnit,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::LinkPlan(payload) => CompilerObjectKey {
            kind: CompilerObjectKind::LinkPlan,
            compiler: payload.compiler_namespace.compiler.clone(),
            source_digest: payload.source_digest,
            query_options_digest: payload.stage_inputs.query_options_digest,
            dependency_interface_digests: payload.stage_inputs.dependency_interface_digests.clone(),
            dependency_body_digests: payload.stage_inputs.dependency_body_digests.clone(),
            environment_digest: payload.stage_inputs.environment_digest,
        },
        CompilerObjectPayload::LineTaskEvidence(_) | CompilerObjectPayload::RuntimePlanUnit(_) => {
            return None;
        }
    };
    Some(key.canonicalized())
}

fn key_input_evidence(key: &CompilerObjectKey) -> PersistentQueryKeyInputEvidence {
    PersistentQueryKeyInputEvidence {
        query_options_digest: key.query_options_digest.to_hex(),
        dependency_interface_digests: named_digest_evidence(&key.dependency_interface_digests),
        dependency_body_digests: named_digest_evidence(&key.dependency_body_digests),
        environment_digest: key.environment_digest.to_hex(),
    }
}

fn named_digest_evidence(values: &[NamedDigest]) -> Vec<PersistentQueryNamedDigestEvidence> {
    values
        .iter()
        .map(|value| PersistentQueryNamedDigestEvidence {
            name: value.name().to_owned(),
            digest: value.digest().to_hex(),
        })
        .collect()
}

fn payload_schema_version(payload: &CompilerObjectPayload) -> Option<u32> {
    match payload {
        CompilerObjectPayload::ParsedSyntax(value) => Some(value.schema_version),
        CompilerObjectPayload::InterfaceSummary(value) => Some(value.schema_version),
        CompilerObjectPayload::HirBody(value) => Some(value.schema_version),
        CompilerObjectPayload::TypecheckGate(value) => Some(value.schema_version),
        CompilerObjectPayload::BytecodeUnit(value) => Some(value.schema_version),
        CompilerObjectPayload::LinkPlan(value) => Some(value.schema_version),
        CompilerObjectPayload::LineTaskEvidence(_) | CompilerObjectPayload::RuntimePlanUnit(_) => {
            None
        }
    }
}

fn record_error_reason(error: super::record::CacheRecordError) -> PersistentQueryMissReason {
    match error {
        super::record::CacheRecordError::Encode(error) => {
            PersistentQueryMissReason::CorruptRecord {
                message: error.to_string(),
            }
        }
        super::record::CacheRecordError::UnsupportedSchema { actual, expected } => {
            PersistentQueryMissReason::RecordSchemaMismatch { actual, expected }
        }
        super::record::CacheRecordError::KeyMismatch => {
            PersistentQueryMissReason::RecordKeyMismatch
        }
    }
}

fn awbo_error_reason(error: &AwboError) -> PersistentQueryMissReason {
    match error {
        AwboError::BadMagic
        | AwboError::UnsupportedWireTag { .. }
        | AwboError::PayloadTooLarge { .. }
        | AwboError::MalformedPayload { .. }
        | AwboError::KeyDigestMismatch
        | AwboError::PayloadKeyInputMismatch { .. } => PersistentQueryMissReason::CorruptObject {
            message: error.to_string(),
        },
        AwboError::UnsupportedSchema { actual, expected } => {
            PersistentQueryMissReason::ObjectSchemaMismatch {
                actual: *actual,
                expected: *expected,
            }
        }
        AwboError::KindMismatch { key, payload } => {
            PersistentQueryMissReason::PayloadKindMismatch {
                expected: *key,
                actual: *payload,
            }
        }
        AwboError::StabilityMismatch { kind, .. } => {
            PersistentQueryMissReason::ObjectStabilityMismatch { object_kind: *kind }
        }
        AwboError::PayloadDigestMismatch => PersistentQueryMissReason::PayloadDigestMismatch,
        AwboError::PayloadLengthMismatch { expected, actual } => {
            PersistentQueryMissReason::PayloadLengthMismatch {
                expected: *expected,
                actual: *actual,
            }
        }
        AwboError::PayloadSchemaMismatch { actual, expected } => {
            PersistentQueryMissReason::PayloadSchemaMismatch {
                actual: *actual,
                expected: *expected,
            }
        }
    }
}

fn validate_envelope_for_request(
    envelope: &AwboEnvelope,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    if envelope.kind != key.kind {
        return Err(PersistentQueryMissReason::ObjectKindMismatch {
            expected: key.kind,
            actual: envelope.kind,
        });
    }
    if envelope.stability != key.kind.stability() {
        return Err(PersistentQueryMissReason::ObjectStabilityMismatch {
            object_kind: envelope.kind,
        });
    }
    match &envelope.payload {
        CompilerObjectPayload::ParsedSyntax(payload) => {
            validate_parsed_syntax_payload(payload, key)?;
        }
        CompilerObjectPayload::InterfaceSummary(payload) => {
            validate_interface_summary_payload(payload, key)?;
        }
        CompilerObjectPayload::HirBody(payload) => {
            validate_hir_body_payload(payload, key)?;
        }
        CompilerObjectPayload::TypecheckGate(payload) => {
            validate_typecheck_gate_payload(payload, key)?;
        }
        CompilerObjectPayload::BytecodeUnit(payload) => {
            validate_bytecode_unit_payload(payload, key)?;
        }
        CompilerObjectPayload::LinkPlan(payload) => {
            validate_link_plan_payload(payload, key)?;
        }
        other => {
            return Err(PersistentQueryMissReason::PayloadKindMismatch {
                expected: key.kind,
                actual: other.kind(),
            });
        }
    }
    let expected = key.digest();
    if envelope.key_digest != expected {
        return Err(PersistentQueryMissReason::KeyDigestMismatch {
            expected,
            actual: envelope.key_digest,
        });
    }
    Ok(())
}

fn validate_parsed_syntax_payload(
    payload: &ParsedSyntaxObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    validate_payload_schema(payload.schema_version)?;
    validate_namespace(&payload.compiler_namespace, key)?;
    validate_source_digest(payload.source_digest, key)?;
    payload
        .source_span
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    payload
        .diagnostics
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    validate_stage_inputs(&payload.stage_inputs, key)
}

fn validate_hir_body_payload(
    payload: &HirBodyObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    validate_payload_schema(payload.schema_version)?;
    validate_namespace(&payload.compiler_namespace, key)?;
    validate_source_digest(payload.source_digest, key)?;
    payload
        .source_span
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    payload
        .diagnostics
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    if payload.body_digest != payload.facts.body_shape_digest {
        return Err(PersistentQueryMissReason::CorruptObject {
            message: "HIR body digest does not match body shape digest".to_owned(),
        });
    }
    validate_stage_inputs(&payload.stage_inputs, key)
}

fn validate_typecheck_gate_payload(
    payload: &TypecheckGateObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    validate_payload_schema(payload.schema_version)?;
    validate_namespace(&payload.compiler_namespace, key)?;
    validate_source_digest(payload.source_digest, key)?;
    payload
        .source_span
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    payload
        .diagnostics
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    validate_stage_inputs(&payload.stage_inputs, key)?;
    payload
        .validate_gate_shape()
        .map_err(|error| corrupt_object(&error))
}

fn validate_bytecode_unit_payload(
    payload: &BytecodeUnitObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    validate_payload_schema(payload.schema_version)?;
    validate_namespace(&payload.compiler_namespace, key)?;
    validate_source_digest(payload.source_digest, key)?;
    payload
        .source_span
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    payload
        .diagnostics
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    validate_stage_inputs(&payload.stage_inputs, key)?;
    payload
        .validate_gate_shape()
        .map_err(|error| corrupt_object(&error))
}

fn validate_link_plan_payload(
    payload: &LinkPlanObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    validate_payload_schema(payload.schema_version)?;
    validate_namespace(&payload.compiler_namespace, key)?;
    validate_source_digest(payload.source_digest, key)?;
    payload
        .source_span
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    payload
        .diagnostics
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    validate_stage_inputs(&payload.stage_inputs, key)?;
    payload
        .validate_gate_shape()
        .map_err(|error| corrupt_object(&error))
}

fn typecheck_gate_reuse_policy(envelope: &AwboEnvelope) -> Option<TypecheckGateReusePolicy> {
    match &envelope.payload {
        CompilerObjectPayload::TypecheckGate(payload) => Some(payload.reuse_policy),
        CompilerObjectPayload::ParsedSyntax(_)
        | CompilerObjectPayload::InterfaceSummary(_)
        | CompilerObjectPayload::HirBody(_)
        | CompilerObjectPayload::LineTaskEvidence(_)
        | CompilerObjectPayload::RuntimePlanUnit(_)
        | CompilerObjectPayload::BytecodeUnit(_)
        | CompilerObjectPayload::LinkPlan(_) => None,
    }
}

fn validate_interface_summary_payload(
    payload: &InterfaceSummaryObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    validate_payload_schema(payload.schema_version)?;
    validate_namespace(&payload.compiler_namespace, key)?;
    validate_source_digest(payload.source_digest, key)?;
    payload
        .source_span
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    payload
        .diagnostics
        .validate()
        .map_err(|error| corrupt_object(&error))?;
    validate_stage_inputs(&payload.stage_inputs, key)?;
    payload
        .validate_summary_shape()
        .map_err(|error| corrupt_object(&error))
}

fn validate_payload_schema(version: u32) -> Result<(), PersistentQueryMissReason> {
    if version == AWBO_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(PersistentQueryMissReason::PayloadSchemaMismatch {
            actual: version,
            expected: AWBO_SCHEMA_VERSION,
        })
    }
}

fn validate_namespace(
    namespace: &CompilerIdentityNamespaceObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    let actual = namespace.clone().canonicalized();
    let expected = key.identity_namespace();
    if actual.object_kind != expected.object_kind
        || actual.cache_namespace != expected.cache_namespace
    {
        return Err(PersistentQueryMissReason::PayloadKindMismatch {
            expected: key.kind,
            actual: actual.object_kind,
        });
    }
    if actual.compiler != expected.compiler {
        return Err(PersistentQueryMissReason::CompilerIdentityMismatch {
            expected: Box::new(expected.compiler),
            actual: Box::new(actual.compiler),
        });
    }
    Ok(())
}

fn validate_source_digest(
    actual: BuildDigest,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    if actual == key.source_digest {
        Ok(())
    } else {
        Err(PersistentQueryMissReason::SourceDigestMismatch {
            expected: key.source_digest,
            actual,
        })
    }
}

fn validate_stage_inputs(
    inputs: &CompilerStageInputsObject,
    key: &CompilerObjectKey,
) -> Result<(), PersistentQueryMissReason> {
    let actual = inputs.clone().canonicalized();
    let expected = key.stage_inputs();
    if actual.query_options_digest != expected.query_options_digest {
        return Err(PersistentQueryMissReason::QueryOptionsDigestMismatch {
            expected: expected.query_options_digest,
            actual: actual.query_options_digest,
        });
    }
    if actual.environment_digest != expected.environment_digest {
        return Err(PersistentQueryMissReason::EnvironmentDigestMismatch {
            expected: expected.environment_digest,
            actual: actual.environment_digest,
        });
    }
    if actual.dependency_interface_digests != expected.dependency_interface_digests {
        return Err(
            PersistentQueryMissReason::DependencyInterfaceDigestMismatch {
                expected: expected.dependency_interface_digests,
                actual: actual.dependency_interface_digests,
            },
        );
    }
    if actual.dependency_body_digests != expected.dependency_body_digests {
        return Err(PersistentQueryMissReason::DependencyBodyDigestMismatch {
            expected: expected.dependency_body_digests,
            actual: actual.dependency_body_digests,
        });
    }
    Ok(())
}

fn corrupt_object(error: &AwboError) -> PersistentQueryMissReason {
    PersistentQueryMissReason::CorruptObject {
        message: error.to_string(),
    }
}

fn len_u64(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
