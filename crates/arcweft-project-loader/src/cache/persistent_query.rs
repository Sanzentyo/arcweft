//! Typed read-through for compiler-private persistent query objects.
//!
//! Read-through is deliberately adapter-owned: every local-cache absence,
//! staleness, corruption, or mismatch is returned as typed soft-miss evidence so
//! callers can rebuild from source instead of poisoning the build.

use super::{record::CacheRecord, store::FilesystemCacheStore};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKind},
    fingerprint::{BuildDigest, NamedDigest},
    incremental::{CacheRecordStatus, InvalidationReason, QueryKind},
    persistent_object::{
        AWBO_SCHEMA_VERSION, AwboEnvelope, AwboError, CompilerBuildIdentity,
        CompilerIdentityNamespaceObject, CompilerObjectKey, CompilerObjectKind,
        CompilerObjectPayload, CompilerStageInputsObject, HirBodyObject, ParsedSyntaxObject,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// One adapter-owned persistent query read-through request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentQueryReadRequest {
    pub query: QueryKind,
    pub artifact_key: ArtifactKey,
    pub object_key: CompilerObjectKey,
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

/// Safe payloads enabled in seq04.2.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PersistentQueryHitPayload {
    ParsedSyntax(ParsedSyntaxObject),
    HirBody(HirBodyObject),
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

impl PersistentQueryReadRequest {
    pub fn new(query: QueryKind, artifact_key: ArtifactKey, object_key: CompilerObjectKey) -> Self {
        Self {
            query,
            artifact_key,
            object_key,
        }
    }
}

impl PersistentQueryReadOutcome {
    pub const fn is_hit(&self) -> bool {
        matches!(self, Self::Hit(_))
    }

    pub fn cache_record_status(&self) -> CacheRecordStatus {
        match self {
            Self::Hit(_) => CacheRecordStatus::Hit,
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
            Self::DependencyInterfaceDigestMismatch { .. } => InvalidationReason::InterfaceChanged,
            Self::DependencyBodyDigestMismatch { .. } => InvalidationReason::BodyChanged,
            Self::UnsupportedObjectKind { .. }
            | Self::QueryKindMismatch { .. }
            | Self::RecordReadFailed { .. }
            | Self::CorruptRecord { .. }
            | Self::RecordKeyMismatch
            | Self::ArtifactKindMismatch { .. }
            | Self::MissingObject { .. }
            | Self::ObjectReadFailed { .. }
            | Self::ObjectDigestMismatch { .. }
            | Self::ObjectLengthMismatch { .. }
            | Self::CorruptObject { .. }
            | Self::ObjectKindMismatch { .. }
            | Self::ObjectStabilityMismatch { .. }
            | Self::PayloadKindMismatch { .. }
            | Self::PayloadDigestMismatch
            | Self::PayloadLengthMismatch { .. }
            | Self::KeyDigestMismatch { .. } => InvalidationReason::CorruptRecord,
        }
    }
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
    /// Reads and validates a parse/HIR persistent compiler object.
    pub fn read_persistent_query(
        &self,
        request: &PersistentQueryReadRequest,
    ) -> PersistentQueryReadOutcome {
        self.read_persistent_query_checked(request)
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
        CompilerObjectPayload::HirBody(payload) => {
            Ok(PersistentQueryHitPayload::HirBody(payload.clone()))
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
        CompilerObjectPayload::HirBody(payload) => {
            validate_hir_body_payload(payload, key)?;
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
