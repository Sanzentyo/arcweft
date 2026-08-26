//! Producer-argument ownership admission for checked semantic types.
//!
//! This is deliberately a crate-private Cut-2 boundary.  It classifies the
//! semantic type graph, then validates a live [`RuntimeValue`] against the
//! existing core [`RuntimeCheckedType`] and snapshot owners.  The classifier
//! does not introduce a second checked-type algebra, a runtime nominal
//! catalog, or a public Need certificate.

use std::collections::BTreeSet;

use arcweft_core::{
    entry::{RuntimeSchemaError, RuntimeSchemaLimits, RuntimeValueDigest, TypeLayoutHash},
    pattern::{
        RuntimeCheckedType, RuntimeCheckedVariantCase, RuntimeOpaqueTypeOwner,
        RuntimeSemanticTypeId,
    },
    plan::RuntimeAgentOperationalType,
    value::{
        AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeOpaquePersistence,
        RuntimeOpaqueValueClass, RuntimePayload, RuntimeValue,
    },
};
use arcweft_lang_hir::symbol::ProjectSymbolTable;
use arcweft_lang_syntax::reference::BorrowKind;
use thiserror::Error;

use crate::{
    env::{
        RegisteredSemanticWorld,
        nominal::{AcceptedNominalOwnerId, AcceptedNominalSemantics},
    },
    final_analysis::{FinalSemanticAnalysis, RuntimeProjectNominalKind},
    types::{
        AcceptedNominalType, AgentBuiltinType, ArrayLength, CharacterNominalFamily,
        CharacterNominalType, EntityKind, HandleState, IteratorStateKind, LifetimeScopeKind,
        MapKind, ProjectNominalType, StageActorHandleType, TypeKind,
    },
};

/// Stable digest of the exact ownership evidence consulted by one successful
/// retained-value classification.
///
/// The bytes are intentionally opaque.  Only the classifier in this module
/// may construct the digest; downstream View/task products can compare and
/// commit it without reconstructing catalog evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnershipEvidenceDigest([u8; 32]);

impl OwnershipEvidenceDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// One exact authority row consulted by a successful ownership traversal.
/// Ordering is the normative `(row_tag, semantic key bytes)` order because
/// declaration order matches the accepted row tags and every field is itself
/// ordered by its canonical byte representation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum OwnershipEvidenceRow {
    ProjectNominal {
        semantic_identity: [u8; 32],
        checked_type: [u8; 32],
        declaration_shape: [u8; 32],
    },
    AcceptedOpaque {
        semantic_identity: [u8; 32],
        runtime_producer: String,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
    },
    AgentDto {
        kind: u8,
        snapshot_contract: [u8; 32],
    },
    #[allow(
        dead_code,
        reason = "the value-level stable callable classifier is published after this type-level Cut 2 boundary"
    )]
    StableCallableValue {
        callable: String,
        contract: [u8; 32],
    },
}

impl OwnershipEvidenceDigest {
    fn from_consulted(rows: impl IntoIterator<Item = OwnershipEvidenceRow>) -> Self {
        let rows = rows.into_iter().collect::<BTreeSet<_>>();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.lang.ownership-evidence.v1\0");
        hasher.update(
            &u32::try_from(rows.len())
                .expect("ownership evidence is bounded below u32::MAX")
                .to_le_bytes(),
        );
        for row in rows {
            match row {
                OwnershipEvidenceRow::ProjectNominal {
                    semantic_identity,
                    checked_type,
                    declaration_shape,
                } => {
                    hasher.update(&[0]);
                    hasher.update(&semantic_identity);
                    hasher.update(&checked_type);
                    hasher.update(&declaration_shape);
                }
                OwnershipEvidenceRow::AcceptedOpaque {
                    semantic_identity,
                    runtime_producer,
                    value_class,
                    persistence,
                } => {
                    hasher.update(&[1]);
                    hasher.update(&semantic_identity);
                    write_evidence_string(&mut hasher, &runtime_producer);
                    hasher.update(&[value_class.semantic_tag()]);
                    hasher.update(&[persistence.semantic_tag()]);
                }
                OwnershipEvidenceRow::AgentDto {
                    kind,
                    snapshot_contract,
                } => {
                    hasher.update(&[2, kind]);
                    hasher.update(&snapshot_contract);
                }
                OwnershipEvidenceRow::StableCallableValue { callable, contract } => {
                    hasher.update(&[3]);
                    write_evidence_string(&mut hasher, &callable);
                    hasher.update(&contract);
                }
            }
        }
        Self::from_bytes(*hasher.finalize().as_bytes())
    }
}

fn write_evidence_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(
        &u32::try_from(value.len())
            .expect("accepted evidence strings fit the u32 grammar")
            .to_le_bytes(),
    );
    hasher.update(value.as_bytes());
}

/// Runtime retention disposition established by semantic ownership checking.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RetainedValueDisposition {
    Copy = 0,
    SnapshotClone = 1,
}

impl RetainedValueDisposition {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Copy => 0,
            Self::SnapshotClone => 1,
        }
    }
}

/// Complete public result of type-directed ownership checking.
///
/// Consulted evidence stays private so a consumer cannot edit or partially
/// replay it.  The public boundary exposes only the disposition and the
/// canonical digest committed by subsequent compiler products.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedOwnershipCertificate {
    disposition: RetainedValueDisposition,
    evidence: OwnershipEvidenceDigest,
}

/// Bounded work contract for one transactional ownership classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedOwnershipLimits {
    pub max_type_nodes: u64,
    pub max_recursion_depth: u64,
    pub max_nominal_edges: u64,
    pub max_active_nominal_depth: u64,
    pub max_evidence_rows: u64,
    pub max_value_certificate_nodes: u64,
    pub max_function_captures: u64,
    pub max_producer_arguments: u64,
}

impl CheckedOwnershipLimits {
    pub const PRODUCTION: Self = Self {
        max_type_nodes: 65_536,
        max_recursion_depth: 64,
        max_nominal_edges: 16_384,
        max_active_nominal_depth: 64,
        max_evidence_rows: 16_384,
        max_value_certificate_nodes: 65_536,
        max_function_captures: 4_096,
        max_producer_arguments: 4_096,
    };
}

impl Default for CheckedOwnershipLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}

/// Opaque public failure from the complete checked ownership boundary.
/// Detailed typed paths remain internal until a downstream diagnostic owner
/// consumes them without exposing construction of private projections.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CheckedOwnershipError {
    #[error("checked ownership was rejected")]
    Rejected,
    #[error("checked ownership exceeded its configured work limit")]
    WorkLimit,
}

impl From<RuntimeOwnershipError> for CheckedOwnershipError {
    fn from(error: RuntimeOwnershipError) -> Self {
        match error {
            RuntimeOwnershipError::WorkLimit => Self::WorkLimit,
            RuntimeOwnershipError::Rejected { .. }
            | RuntimeOwnershipError::CarrierMismatch { .. }
            | RuntimeOwnershipError::ArrayLengthMismatch { .. }
            | RuntimeOwnershipError::Canonical { .. }
            | RuntimeOwnershipError::Snapshot { .. } => Self::Rejected,
        }
    }
}

impl CheckedOwnershipCertificate {
    #[must_use]
    pub const fn disposition(&self) -> RetainedValueDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn evidence(&self) -> OwnershipEvidenceDigest {
        self.evidence
    }
}

/// A typed descent through a semantic type graph.
///
/// These segments are semantic coordinates.  They are not source labels and
/// are never used to reconstruct a type or a nominal identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RuntimeOwnershipPathSegment {
    SequenceItem,
    ArrayItem,
    TupleItem(u32),
    ResultOk,
    ResultError,
    OptionItem,
    ProbeResult,
    AcceptedNominalArgument(u32),
    ProjectNominalMember(u32),
}

/// Typed path of the first semantic admission failure.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeOwnershipPath(Box<[RuntimeOwnershipPathSegment]>);

impl RuntimeOwnershipPath {
    #[must_use]
    pub(crate) fn root() -> Self {
        Self(Box::new([]))
    }

    #[must_use]
    pub(crate) fn pushed(&self, segment: RuntimeOwnershipPathSegment) -> Self {
        let mut segments = self.0.to_vec();
        segments.push(segment);
        Self(segments.into_boxed_slice())
    }

    #[must_use]
    pub(crate) const fn segments(&self) -> &[RuntimeOwnershipPathSegment] {
        &self.0
    }
}

impl std::fmt::Display for RuntimeOwnershipPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("$")?;
        for segment in self.segments() {
            match segment {
                RuntimeOwnershipPathSegment::SequenceItem => formatter.write_str(".sequence")?,
                RuntimeOwnershipPathSegment::ArrayItem => formatter.write_str(".array")?,
                RuntimeOwnershipPathSegment::TupleItem(index) => {
                    write!(formatter, ".tuple[{index}]")?;
                }
                RuntimeOwnershipPathSegment::ResultOk => formatter.write_str(".ok")?,
                RuntimeOwnershipPathSegment::ResultError => formatter.write_str(".error")?,
                RuntimeOwnershipPathSegment::OptionItem => formatter.write_str(".some")?,
                RuntimeOwnershipPathSegment::ProbeResult => formatter.write_str(".probe")?,
                RuntimeOwnershipPathSegment::AcceptedNominalArgument(index) => {
                    write!(formatter, ".nominal[{index}]")?;
                }
                RuntimeOwnershipPathSegment::ProjectNominalMember(index) => {
                    write!(formatter, ".project_nominal[{index}]")?;
                }
            }
        }
        Ok(())
    }
}

/// Why one semantic type cannot be admitted as a producer argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RuntimeOwnershipRejection {
    AffineValue,
    BorrowedValue,
    StreamValue,
    FrameLocalValue,
    MissingCanonicalIdentity,
    MissingRuntimeSnapshotOwner,
    FunctionValueRequiresCertificate,
    UnresolvedType,
    DeadHandle,
    MovedValue,
    StaleAuthority,
    RecursiveRetentionCycle,
}

impl std::fmt::Display for RuntimeOwnershipRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::AffineValue => "affine value",
            Self::BorrowedValue => "borrowed value",
            Self::StreamValue => "stream value",
            Self::FrameLocalValue => "frame-local value",
            Self::MissingCanonicalIdentity => "missing canonical identity",
            Self::MissingRuntimeSnapshotOwner => "missing runtime snapshot owner",
            Self::FunctionValueRequiresCertificate => "function value requires certificate",
            Self::UnresolvedType => "unresolved type",
            Self::DeadHandle => "dead handle",
            Self::MovedValue => "moved value",
            Self::StaleAuthority => "stale semantic ownership authority",
            Self::RecursiveRetentionCycle => "recursive retention cycle",
        })
    }
}

/// Typed failure produced by classification or live/snapshot admission.
#[allow(
    dead_code,
    reason = "live carrier mismatch variants become production-reachable with the atomic Cut 5 value boundary"
)]
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum RuntimeOwnershipError {
    #[error("checked ownership exceeded its configured work limit")]
    WorkLimit,
    #[error("runtime ownership rejected at {path}: {reason}")]
    Rejected {
        path: RuntimeOwnershipPath,
        reason: RuntimeOwnershipRejection,
    },
    #[error("runtime value at {path} does not satisfy its checked carrier")]
    CarrierMismatch { path: RuntimeOwnershipPath },
    #[error("array carrier at {path} has length {actual}, expected {expected}")]
    ArrayLengthMismatch {
        path: RuntimeOwnershipPath,
        expected: u64,
        actual: usize,
    },
    #[error("canonical runtime value failed at {path}: {source}")]
    Canonical {
        path: RuntimeOwnershipPath,
        #[source]
        source: RuntimeSchemaError,
    },
    #[error("runtime snapshot failed at {path}: {source}")]
    Snapshot {
        path: RuntimeOwnershipPath,
        #[source]
        source: AwbcRuntimeValueSnapshotError,
    },
}

impl RuntimeOwnershipError {
    fn rejected(path: &RuntimeOwnershipPath, reason: RuntimeOwnershipRejection) -> Self {
        Self::Rejected {
            path: path.clone(),
            reason,
        }
    }

    #[must_use]
    #[allow(
        dead_code,
        reason = "typed path inspection currently serves focused tests"
    )]
    pub(crate) const fn path(&self) -> &RuntimeOwnershipPath {
        match self {
            Self::WorkLimit => panic!("work-limit errors do not own a semantic path"),
            Self::Rejected { path, .. }
            | Self::CarrierMismatch { path }
            | Self::ArrayLengthMismatch { path, .. }
            | Self::Canonical { path, .. }
            | Self::Snapshot { path, .. } => path,
        }
    }

    #[must_use]
    #[allow(
        dead_code,
        reason = "typed rejection inspection currently serves focused tests"
    )]
    pub(crate) const fn rejection(&self) -> Option<RuntimeOwnershipRejection> {
        match self {
            Self::Rejected { reason, .. } => Some(*reason),
            Self::WorkLimit
            | Self::CarrierMismatch { .. }
            | Self::ArrayLengthMismatch { .. }
            | Self::Canonical { .. }
            | Self::Snapshot { .. } => None,
        }
    }
}

/// Semantic sequence shape retained in addition to core's checked item type.
///
/// Core's [`RuntimeCheckedType::Sequence`] is the carrier authority.  This
/// small wrapper retains the source sequence family and the exact array
/// length, which are semantic constraints not represented by that core type.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RuntimeSequenceOwnershipProjection {
    Vec(Box<RuntimeOwnershipProjection>),
    Array {
        item: Box<RuntimeOwnershipProjection>,
        length: u64,
    },
    Slice(Box<RuntimeOwnershipProjection>),
    Seq(Box<RuntimeOwnershipProjection>),
}

impl RuntimeSequenceOwnershipProjection {
    fn item(&self) -> &RuntimeOwnershipProjection {
        match self {
            Self::Vec(item) | Self::Array { item, .. } | Self::Slice(item) | Self::Seq(item) => {
                item
            }
        }
    }

    #[allow(
        dead_code,
        reason = "live array carrier validation is published in Cut 5"
    )]
    fn array_length(&self) -> Option<u64> {
        match self {
            Self::Array { length, .. } => Some(*length),
            Self::Vec(_) | Self::Slice(_) | Self::Seq(_) => None,
        }
    }
}

/// One checked projection shared by producer admission and later runtime
/// lowering.  `Need` is intentionally crate-private until Cut 5 publishes the
/// live and snapshot carriers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RuntimeOwnershipProjection {
    Checked(RuntimeCheckedType),
    Sequence(RuntimeSequenceOwnershipProjection),
    Nominal {
        checked: RuntimeCheckedType,
        schema: arcweft_core::entry::RuntimeTypeSchema,
        layout: TypeLayoutHash,
    },
    Text {
        checked: RuntimeCheckedType,
        semantic_identity: RuntimeSemanticTypeId,
    },
    Need(RuntimeNeedOwnershipCertificate),
}

impl RuntimeOwnershipProjection {
    fn checked_type_at(
        &self,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeCheckedType, RuntimeOwnershipError> {
        match self {
            Self::Checked(checked) | Self::Nominal { checked, .. } | Self::Text { checked, .. } => {
                Ok(checked.clone())
            }
            Self::Sequence(sequence) => {
                let item_path = path.pushed(RuntimeOwnershipPathSegment::SequenceItem);
                let item = sequence.item().checked_type_at(&item_path)?;
                Ok(RuntimeCheckedType::Sequence(Box::new(item)))
            }
            Self::Need(_) => Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            )),
        }
    }

    #[allow(dead_code, reason = "live carrier validation is published in Cut 5")]
    fn validate_value_at(
        &self,
        value: &RuntimeValue,
        path: &RuntimeOwnershipPath,
    ) -> Result<(), RuntimeOwnershipError> {
        let checked = self.checked_type_at(path)?;
        if !checked.accepts_value(value) {
            return Err(RuntimeOwnershipError::CarrierMismatch { path: path.clone() });
        }
        if let Self::Sequence(sequence) = self
            && let Some(expected) = sequence.array_length()
        {
            let actual = match value {
                RuntimeValue::Seq(sequence) => sequence.len(),
                _ => {
                    return Err(RuntimeOwnershipError::CarrierMismatch { path: path.clone() });
                }
            };
            if actual != usize::try_from(expected).unwrap_or(usize::MAX) {
                return Err(RuntimeOwnershipError::ArrayLengthMismatch {
                    path: path.clone(),
                    expected,
                    actual,
                });
            }
        }
        if let Self::Nominal {
            checked,
            schema,
            layout,
        } = self
        {
            let (RuntimeCheckedType::Nominal { nominal, .. }
            | RuntimeCheckedType::Variant { nominal, .. }) = checked
            else {
                unreachable!("nominal projection always carries a nominal checked type")
            };
            return validate_nominal_schema(schema, value, nominal, *layout, path);
        }
        Ok(())
    }
}

#[allow(
    dead_code,
    reason = "live nominal carrier validation is published in Cut 5"
)]
fn validate_nominal_schema(
    schema: &arcweft_core::entry::RuntimeTypeSchema,
    value: &RuntimeValue,
    nominal: &arcweft_core::entry::RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    path: &RuntimeOwnershipPath,
) -> Result<(), RuntimeOwnershipError> {
    schema
        .validate_nominal_payload(
            &RuntimePayload(value.clone()),
            nominal,
            layout,
            RuntimeSchemaLimits::engine_default(),
        )
        .map(|_| ())
        .map_err(|_| RuntimeOwnershipError::CarrierMismatch { path: path.clone() })
}

/// Private proof that a `Need` payload type was semantically selected.  The
/// payload is contract evidence only; no live `RuntimeValue` is embedded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeNeedOwnershipCertificate {
    need_identity: RuntimeSemanticTypeId,
    payload_identity: RuntimeSemanticTypeId,
}

impl RuntimeNeedOwnershipCertificate {
    #[must_use]
    #[allow(
        dead_code,
        reason = "private Need evidence is inspected by focused tests"
    )]
    pub(crate) const fn need_identity(&self) -> RuntimeSemanticTypeId {
        self.need_identity
    }

    #[must_use]
    #[allow(
        dead_code,
        reason = "private Need evidence is inspected by focused tests"
    )]
    pub(crate) const fn payload_identity(&self) -> RuntimeSemanticTypeId {
        self.payload_identity
    }
}

/// Result of the exhaustive producer-argument classifier.
///
/// This type and its constructors remain crate-private.  In particular, no
/// downstream crate can fabricate a successful ownership certificate before
/// the Cut-5 Need/value boundary is published.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RuntimeProducerArgumentAdmission {
    Copy(RuntimeOwnershipProjection),
    SnapshotClone(RuntimeOwnershipProjection),
}

impl RuntimeProducerArgumentAdmission {
    #[must_use]
    pub(crate) const fn projection(&self) -> &RuntimeOwnershipProjection {
        match self {
            Self::Copy(projection) | Self::SnapshotClone(projection) => projection,
        }
    }

    #[must_use]
    pub(crate) const fn permits_copy(&self) -> bool {
        matches!(self, Self::Copy(_))
    }

    #[allow(dead_code, reason = "live carrier validation is published in Cut 5")]
    pub(crate) fn validate_live_value(
        &self,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeOwnershipError> {
        let path = RuntimeOwnershipPath::root();
        if self.permits_copy() && !value.ownership().permits_copy() {
            return Err(RuntimeOwnershipError::rejected(
                &path,
                RuntimeOwnershipRejection::AffineValue,
            ));
        }
        self.projection().validate_value_at(value, &path)
    }

    #[allow(
        dead_code,
        reason = "live producer value admission is published in Cut 5"
    )]
    pub(crate) fn try_digest(
        &self,
        value: &RuntimeValue,
        max_encoded_bytes: usize,
    ) -> Result<RuntimeValueDigest, RuntimeOwnershipError> {
        self.validate_live_value(value)?;
        value
            .try_digest(max_encoded_bytes)
            .map_err(|source| RuntimeOwnershipError::Canonical {
                path: RuntimeOwnershipPath::root(),
                source,
            })
    }

    #[allow(
        dead_code,
        reason = "live producer snapshot admission is published in Cut 5"
    )]
    pub(crate) fn try_snapshot(
        &self,
        value: &RuntimeValue,
    ) -> Result<AwbcRuntimeValueSnapshot, RuntimeOwnershipError> {
        self.validate_live_value(value)?;
        AwbcRuntimeValueSnapshot::from_runtime_value(value).map_err(|source| {
            RuntimeOwnershipError::Snapshot {
                path: RuntimeOwnershipPath::root(),
                source,
            }
        })
    }
}

/// Exhaustive classifier for one checked semantic type and its accepted world.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RuntimeProducerArgumentClassifier<'a> {
    analysis: Option<&'a FinalSemanticAnalysis>,
    world: Option<&'a RegisteredSemanticWorld>,
}

struct OwnershipTraversal {
    limits: CheckedOwnershipLimits,
    type_nodes: u64,
    nominal_edges: u64,
    active_nominals: BTreeSet<RuntimeSemanticTypeId>,
    evidence: Vec<OwnershipEvidenceRow>,
}

impl OwnershipTraversal {
    fn new(limits: CheckedOwnershipLimits) -> Self {
        Self {
            limits,
            type_nodes: 0,
            nominal_edges: 0,
            active_nominals: BTreeSet::new(),
            evidence: Vec::new(),
        }
    }

    fn charge_type(&mut self, depth: u64) -> Result<(), RuntimeOwnershipError> {
        self.type_nodes = self
            .type_nodes
            .checked_add(1)
            .ok_or(RuntimeOwnershipError::WorkLimit)?;
        if self.type_nodes > self.limits.max_type_nodes || depth > self.limits.max_recursion_depth {
            return Err(RuntimeOwnershipError::WorkLimit);
        }
        Ok(())
    }

    fn child_depth(depth: u64) -> Result<u64, RuntimeOwnershipError> {
        depth.checked_add(1).ok_or(RuntimeOwnershipError::WorkLimit)
    }

    fn enter_nominal(
        &mut self,
        identity: RuntimeSemanticTypeId,
        path: &RuntimeOwnershipPath,
    ) -> Result<(), RuntimeOwnershipError> {
        self.nominal_edges = self
            .nominal_edges
            .checked_add(1)
            .ok_or(RuntimeOwnershipError::WorkLimit)?;
        let next_active_depth = u64::try_from(self.active_nominals.len())
            .map_err(|_| RuntimeOwnershipError::WorkLimit)?
            .checked_add(1)
            .ok_or(RuntimeOwnershipError::WorkLimit)?;
        if self.nominal_edges > self.limits.max_nominal_edges
            || next_active_depth > self.limits.max_active_nominal_depth
        {
            return Err(RuntimeOwnershipError::WorkLimit);
        }
        if !self.active_nominals.insert(identity) {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::RecursiveRetentionCycle,
            ));
        }
        Ok(())
    }

    fn leave_nominal(&mut self, identity: RuntimeSemanticTypeId) {
        self.active_nominals.remove(&identity);
    }

    fn push_evidence(&mut self, row: OwnershipEvidenceRow) -> Result<(), RuntimeOwnershipError> {
        let next_evidence_len = u64::try_from(self.evidence.len())
            .map_err(|_| RuntimeOwnershipError::WorkLimit)?
            .checked_add(1)
            .ok_or(RuntimeOwnershipError::WorkLimit)?;
        if next_evidence_len > self.limits.max_evidence_rows {
            return Err(RuntimeOwnershipError::WorkLimit);
        }
        self.evidence.push(row);
        Ok(())
    }
}

impl<'a> RuntimeProducerArgumentClassifier<'a> {
    pub(crate) fn try_new(
        analysis: &'a FinalSemanticAnalysis,
        world: &'a RegisteredSemanticWorld,
    ) -> Result<Self, RuntimeOwnershipError> {
        if !analysis.matches_symbol_lease(world.symbols()) {
            return Err(RuntimeOwnershipError::rejected(
                &RuntimeOwnershipPath::root(),
                RuntimeOwnershipRejection::StaleAuthority,
            ));
        }
        Ok(Self {
            analysis: Some(analysis),
            world: Some(world),
        })
    }

    #[cfg(test)]
    fn for_test() -> Self {
        Self {
            analysis: None,
            world: None,
        }
    }

    fn authority(
        &self,
    ) -> Option<(
        &'a FinalSemanticAnalysis,
        &'a RegisteredSemanticWorld,
        &'a ProjectSymbolTable,
    )> {
        let world = self.world?;
        Some((self.analysis?, world, world.symbols()))
    }

    fn exact_runtime_type_identity(
        &self,
        ty: &TypeKind,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeSemanticTypeId, RuntimeOwnershipError> {
        match ty {
            TypeKind::GenericParam(_)
            | TypeKind::OpenNominal(_)
            | TypeKind::Error(_)
            | TypeKind::Projection { .. }
            | TypeKind::Named(_)
            | TypeKind::Array {
                len: ArrayLength::Generic(_) | ArrayLength::Error(_) | ArrayLength::Inferred,
                ..
            } => Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::UnresolvedType,
            )),
            TypeKind::ProjectNominal(nominal) => {
                let Some((analysis, _, _)) = self.authority() else {
                    return Err(RuntimeOwnershipError::rejected(
                        path,
                        RuntimeOwnershipRejection::MissingCanonicalIdentity,
                    ));
                };
                let semantic_type =
                    TypeKind::ProjectNominal(nominal.clone()).semantic_identity_digest();
                analysis
                    .runtime_nominal_projection(semantic_type)
                    .map(|projection| projection.semantic_identity())
                    .ok_or_else(|| {
                        RuntimeOwnershipError::rejected(
                            path,
                            RuntimeOwnershipRejection::MissingCanonicalIdentity,
                        )
                    })
            }
            TypeKind::AcceptedNominal(nominal) => {
                let Some((_, world, _)) = self.authority() else {
                    return Err(RuntimeOwnershipError::rejected(
                        path,
                        RuntimeOwnershipRejection::MissingCanonicalIdentity,
                    ));
                };
                world
                    .environment()
                    .nominal_catalog()
                    .exact(nominal.declaration().canonical_path())
                    .filter(|record| {
                        record.id() == nominal.declaration()
                            && usize::from(record.arity()) == nominal.arguments().len()
                    })
                    .ok_or_else(|| {
                        RuntimeOwnershipError::rejected(
                            path,
                            RuntimeOwnershipRejection::MissingCanonicalIdentity,
                        )
                    })?;
                Ok(runtime_semantic_identity(ty))
            }
            TypeKind::Need(payload) => self
                .exact_runtime_type_identity(payload, path)
                .map(|_| runtime_semantic_identity(ty)),
            _ => Ok(runtime_semantic_identity(ty)),
        }
    }

    #[allow(
        dead_code,
        reason = "private projection inspection remains available to focused tests"
    )]
    pub(crate) fn classify(
        &self,
        ty: &TypeKind,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        self.classify_with_evidence(ty, CheckedOwnershipLimits::PRODUCTION)
            .map(|(admission, _)| admission)
    }

    fn classify_with_evidence(
        &self,
        ty: &TypeKind,
        limits: CheckedOwnershipLimits,
    ) -> Result<(RuntimeProducerArgumentAdmission, Vec<OwnershipEvidenceRow>), RuntimeOwnershipError>
    {
        let mut traversal = OwnershipTraversal::new(limits);
        let admission = self.classify_at(ty, &RuntimeOwnershipPath::root(), 0, &mut traversal)?;
        Ok((admission, traversal.evidence))
    }

    fn classify_batch_with_evidence(
        &self,
        types: &[&TypeKind],
        limits: CheckedOwnershipLimits,
    ) -> Result<(Vec<RetainedValueDisposition>, CheckedOwnershipCertificate), RuntimeOwnershipError>
    {
        let mut traversal = OwnershipTraversal::new(limits);
        let mut dispositions = Vec::with_capacity(types.len());
        let mut aggregate = RetainedValueDisposition::Copy;
        for ty in types {
            let admission =
                self.classify_at(ty, &RuntimeOwnershipPath::root(), 0, &mut traversal)?;
            if matches!(admission.projection(), RuntimeOwnershipProjection::Need(_)) {
                return Err(RuntimeOwnershipError::rejected(
                    &RuntimeOwnershipPath::root(),
                    RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
                ));
            }
            let disposition = if admission.permits_copy() {
                RetainedValueDisposition::Copy
            } else {
                RetainedValueDisposition::SnapshotClone
            };
            if disposition == RetainedValueDisposition::SnapshotClone {
                aggregate = RetainedValueDisposition::SnapshotClone;
            }
            dispositions.push(disposition);
        }
        Ok((
            dispositions,
            CheckedOwnershipCertificate {
                disposition: aggregate,
                evidence: OwnershipEvidenceDigest::from_consulted(traversal.evidence),
            },
        ))
    }

    #[allow(
        clippy::match_same_arms,
        clippy::too_many_lines,
        reason = "one exhaustive TypeKind owner keeps closed-family classification and first-error precedence reviewable; same-result arms retain explicit closed-family coverage"
    )]
    fn classify_at(
        &self,
        ty: &TypeKind,
        path: &RuntimeOwnershipPath,
        depth: u64,
        traversal: &mut OwnershipTraversal,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        traversal.charge_type(depth)?;
        let checked = |ty| {
            Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
                RuntimeOwnershipProjection::Checked(ty),
            ))
        };
        let copy = |ty| {
            Ok(RuntimeProducerArgumentAdmission::Copy(
                RuntimeOwnershipProjection::Checked(ty),
            ))
        };
        let rejected = |reason| Err(RuntimeOwnershipError::rejected(path, reason));

        match ty {
            TypeKind::Bool => copy(RuntimeCheckedType::Bool),
            TypeKind::I8 => copy(RuntimeCheckedType::Signed(
                arcweft_core::value::RuntimeSignedIntWidth::I8,
            )),
            TypeKind::I16 => copy(RuntimeCheckedType::Signed(
                arcweft_core::value::RuntimeSignedIntWidth::I16,
            )),
            TypeKind::I32 => copy(RuntimeCheckedType::Signed(
                arcweft_core::value::RuntimeSignedIntWidth::I32,
            )),
            TypeKind::I64 => copy(RuntimeCheckedType::Signed(
                arcweft_core::value::RuntimeSignedIntWidth::I64,
            )),
            TypeKind::I128 => copy(RuntimeCheckedType::Signed(
                arcweft_core::value::RuntimeSignedIntWidth::I128,
            )),
            TypeKind::ISize => copy(RuntimeCheckedType::Signed(
                arcweft_core::value::RuntimeSignedIntWidth::ISize,
            )),
            TypeKind::U8 => copy(RuntimeCheckedType::Unsigned(
                arcweft_core::value::RuntimeUnsignedIntWidth::U8,
            )),
            TypeKind::U16 => copy(RuntimeCheckedType::Unsigned(
                arcweft_core::value::RuntimeUnsignedIntWidth::U16,
            )),
            TypeKind::U32 => copy(RuntimeCheckedType::Unsigned(
                arcweft_core::value::RuntimeUnsignedIntWidth::U32,
            )),
            TypeKind::U64 => copy(RuntimeCheckedType::Unsigned(
                arcweft_core::value::RuntimeUnsignedIntWidth::U64,
            )),
            TypeKind::U128 => copy(RuntimeCheckedType::Unsigned(
                arcweft_core::value::RuntimeUnsignedIntWidth::U128,
            )),
            TypeKind::USize => copy(RuntimeCheckedType::Unsigned(
                arcweft_core::value::RuntimeUnsignedIntWidth::USize,
            )),
            TypeKind::F32 => copy(RuntimeCheckedType::F32),
            TypeKind::F64 => copy(RuntimeCheckedType::F64),
            TypeKind::String => checked(RuntimeCheckedType::String),
            TypeKind::Char => copy(RuntimeCheckedType::Char),
            TypeKind::Bytes => checked(RuntimeCheckedType::Bytes),
            TypeKind::TextCluster | TypeKind::DisplayText => self.classify_text(ty, path),
            TypeKind::Duration => copy(RuntimeCheckedType::Duration),
            TypeKind::Progress => checked(RuntimeCheckedType::Progress),
            TypeKind::StageApi(_)
            | TypeKind::LineContext
            | TypeKind::StageActorHandle(
                StageActorHandleType::Exact(_) | StageActorHandleType::Any,
            )
            | TypeKind::CueHandle
            | TypeKind::VoiceHandle => rejected(RuntimeOwnershipRejection::AffineValue),
            TypeKind::Range(_) => rejected(RuntimeOwnershipRejection::MissingCanonicalIdentity),
            TypeKind::IteratorState {
                family:
                    IteratorStateKind::Range
                    | IteratorStateKind::Seq
                    | IteratorStateKind::Stream
                    | IteratorStateKind::Vec
                    | IteratorStateKind::Array
                    | IteratorStateKind::Slice,
                ..
            } => rejected(RuntimeOwnershipRejection::FrameLocalValue),
            TypeKind::DebugStatePath => {
                record_agent_evidence(traversal, RuntimeAgentOperationalType::DebugStatePath)?;
                checked(RuntimeCheckedType::Agent(
                    RuntimeAgentOperationalType::DebugStatePath,
                ))
            }
            TypeKind::ObservationFieldPath => {
                record_agent_evidence(
                    traversal,
                    RuntimeAgentOperationalType::ObservationFieldPath,
                )?;
                checked(RuntimeCheckedType::Agent(
                    RuntimeAgentOperationalType::ObservationFieldPath,
                ))
            }
            TypeKind::Ref(_) => checked(RuntimeCheckedType::EntityReference),
            TypeKind::Probe(result) => {
                self.classify_at(
                    result,
                    &path.pushed(RuntimeOwnershipPathSegment::ProbeResult),
                    OwnershipTraversal::child_depth(depth)?,
                    traversal,
                )?;
                record_agent_evidence(traversal, RuntimeAgentOperationalType::Probe)?;
                checked(RuntimeCheckedType::Agent(
                    RuntimeAgentOperationalType::Probe,
                ))
            }
            TypeKind::Predicate => {
                record_agent_evidence(traversal, RuntimeAgentOperationalType::Predicate)?;
                checked(RuntimeCheckedType::Agent(
                    RuntimeAgentOperationalType::Predicate,
                ))
            }
            TypeKind::Observation
            | TypeKind::ObservedObject
            | TypeKind::AgentBBox
            | TypeKind::ActionName
            | TypeKind::ActionResult
            | TypeKind::AgentValue
            | TypeKind::DataFormat
            | TypeKind::DataShape
            | TypeKind::AgentEntityMetadata
            | TypeKind::AgentSourceAnchor
            | TypeKind::AgentProjectGraphNeighborhood
            | TypeKind::AgentProjectGraphSymbol
            | TypeKind::AgentProjectGraphEdge
            | TypeKind::CaptureRef
            | TypeKind::AgentResource
            | TypeKind::AgentResourceBody
            | TypeKind::RagContextPack
            | TypeKind::Shared(_) => {
                rejected(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner)
            }
            TypeKind::ActionTarget => {
                record_agent_evidence(traversal, RuntimeAgentOperationalType::ActionTarget)?;
                checked(RuntimeCheckedType::Agent(
                    RuntimeAgentOperationalType::ActionTarget,
                ))
            }
            TypeKind::CaptureTarget => {
                record_agent_evidence(traversal, RuntimeAgentOperationalType::CaptureTarget)?;
                checked(RuntimeCheckedType::Agent(
                    RuntimeAgentOperationalType::CaptureTarget,
                ))
            }
            TypeKind::AgentBuiltin(builtin) => {
                Self::classify_agent_builtin(*builtin, path, traversal)
            }
            TypeKind::Vec(item) => self.classify_sequence(
                item,
                RuntimeSequenceOwnershipProjection::Vec,
                RuntimeOwnershipPathSegment::SequenceItem,
                path,
                depth,
                traversal,
            ),
            TypeKind::Array { item, len } => match len {
                ArrayLength::Const(length) => {
                    let length = u64::try_from(*length).map_err(|_| {
                        RuntimeOwnershipError::rejected(
                            path,
                            RuntimeOwnershipRejection::UnresolvedType,
                        )
                    })?;
                    let child = self.classify_at(
                        item,
                        &path.pushed(RuntimeOwnershipPathSegment::ArrayItem),
                        OwnershipTraversal::child_depth(depth)?,
                        traversal,
                    )?;
                    Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
                        RuntimeOwnershipProjection::Sequence(
                            RuntimeSequenceOwnershipProjection::Array {
                                item: Box::new(child.projection().clone()),
                                length,
                            },
                        ),
                    ))
                }
                ArrayLength::Generic(_) | ArrayLength::Error(_) | ArrayLength::Inferred => {
                    rejected(RuntimeOwnershipRejection::UnresolvedType)
                }
            },
            TypeKind::Slice(item) => self.classify_sequence(
                item,
                RuntimeSequenceOwnershipProjection::Slice,
                RuntimeOwnershipPathSegment::SequenceItem,
                path,
                depth,
                traversal,
            ),
            TypeKind::Seq(item) => self.classify_sequence(
                item,
                RuntimeSequenceOwnershipProjection::Seq,
                RuntimeOwnershipPathSegment::SequenceItem,
                path,
                depth,
                traversal,
            ),
            TypeKind::Map { kind, .. } => Self::classify_map(*kind, path),
            TypeKind::BorrowRef { kind, lifetime, .. } => {
                Self::classify_borrow(*kind, lifetime.as_ref(), path)
            }
            TypeKind::Need(payload) => {
                let payload_identity = self.exact_runtime_type_identity(payload, path)?;
                Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
                    RuntimeOwnershipProjection::Need(RuntimeNeedOwnershipCertificate {
                        need_identity: runtime_semantic_identity(ty),
                        payload_identity,
                    }),
                ))
            }
            TypeKind::Stream { .. } => rejected(RuntimeOwnershipRejection::StreamValue),
            TypeKind::Result { ok, error } => {
                let ok = self.classify_at(
                    ok,
                    &path.pushed(RuntimeOwnershipPathSegment::ResultOk),
                    OwnershipTraversal::child_depth(depth)?,
                    traversal,
                )?;
                let error = self.classify_at(
                    error,
                    &path.pushed(RuntimeOwnershipPathSegment::ResultError),
                    OwnershipTraversal::child_depth(depth)?,
                    traversal,
                )?;
                let checked_type = RuntimeCheckedType::Result {
                    ok: Box::new(ok.projection().checked_type_at(path)?),
                    error: Box::new(error.projection().checked_type_at(path)?),
                };
                validate_variant_cases(&checked_type, path)?;
                let RuntimeCheckedType::Result { ok, error } = checked_type else {
                    unreachable!("constructed result checked type")
                };
                checked(RuntimeCheckedType::Result { ok, error })
            }
            TypeKind::Option(item) => {
                let item = self.classify_at(
                    item,
                    &path.pushed(RuntimeOwnershipPathSegment::OptionItem),
                    OwnershipTraversal::child_depth(depth)?,
                    traversal,
                )?;
                let checked_type =
                    RuntimeCheckedType::Option(Box::new(item.projection().checked_type_at(path)?));
                validate_variant_cases(&checked_type, path)?;
                checked(checked_type)
            }
            TypeKind::Handle { state, .. } => match state {
                HandleState::Live | HandleState::Detached => {
                    rejected(RuntimeOwnershipRejection::AffineValue)
                }
                HandleState::Dropped => rejected(RuntimeOwnershipRejection::DeadHandle),
                HandleState::MovedOut => rejected(RuntimeOwnershipRejection::MovedValue),
            },
            TypeKind::ThreadHandle(_) => rejected(RuntimeOwnershipRejection::AffineValue),
            TypeKind::Function { .. } => {
                rejected(RuntimeOwnershipRejection::FunctionValueRequiresCertificate)
            }
            TypeKind::ProjectNominal(nominal) => {
                self.classify_project_nominal(nominal, path, depth, traversal)
            }
            TypeKind::AcceptedNominal(nominal) => {
                self.classify_accepted_nominal(nominal, path, depth, traversal)
            }
            TypeKind::GenericParam(_)
            | TypeKind::OpenNominal(_)
            | TypeKind::Error(_)
            | TypeKind::Projection { .. } => rejected(RuntimeOwnershipRejection::UnresolvedType),
            TypeKind::CharacterPatch(kind) => Self::classify_character_patch(kind, path),
            TypeKind::FocusPatch => {
                rejected(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner)
            }
            TypeKind::CharacterDialogue(dialogue) => {
                Self::classify_character_dialogue(dialogue.character(), path)
            }
            TypeKind::DialogueLine(_) => {
                rejected(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner)
            }
            TypeKind::ViewValue => rejected(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner),
            TypeKind::CharacterNominal(nominal) => Self::classify_character_nominal(nominal, path),
            TypeKind::Named(_) => self.classify_environment_runtime_nominal(ty, path, traversal),
            TypeKind::Tuple(items) => {
                let mut checked_items = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let index = u32::try_from(index).unwrap_or(u32::MAX);
                    let admission = self.classify_at(
                        item,
                        &path.pushed(RuntimeOwnershipPathSegment::TupleItem(index)),
                        OwnershipTraversal::child_depth(depth)?,
                        traversal,
                    )?;
                    checked_items.push(admission.projection().checked_type_at(path)?);
                }
                checked(RuntimeCheckedType::Tuple(checked_items))
            }
            TypeKind::Choice(_) => rejected(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner),
            TypeKind::Unit => copy(RuntimeCheckedType::Unit),
            TypeKind::Never => copy(RuntimeCheckedType::Never),
        }
    }

    fn classify_sequence(
        &self,
        item: &TypeKind,
        constructor: impl FnOnce(Box<RuntimeOwnershipProjection>) -> RuntimeSequenceOwnershipProjection,
        segment: RuntimeOwnershipPathSegment,
        path: &RuntimeOwnershipPath,
        depth: u64,
        traversal: &mut OwnershipTraversal,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        let item = self.classify_at(
            item,
            &path.pushed(segment),
            OwnershipTraversal::child_depth(depth)?,
            traversal,
        )?;
        Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
            RuntimeOwnershipProjection::Sequence(constructor(Box::new(item.projection().clone()))),
        ))
    }

    fn classify_text(
        &self,
        ty: &TypeKind,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        let Some((_, world, _)) = self.authority() else {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            ));
        };
        let accepted = world
            .environment()
            .nominal_catalog()
            .exact_records()
            .any(|record| {
                record.id().owner() == &AcceptedNominalOwnerId::Standard
                    && (matches!(
                        record.semantics(),
                        AcceptedNominalSemantics::Exact(exact) if exact == ty
                    ) || matches!(
                        record.semantics(),
                        AcceptedNominalSemantics::Record(record) if record.ty() == ty
                    ))
            });
        if !accepted {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            ));
        }
        Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
            RuntimeOwnershipProjection::Text {
                checked: RuntimeCheckedType::String,
                semantic_identity: runtime_semantic_identity(ty),
            },
        ))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one declaration-order traversal keeps nominal recursion, first-error precedence, schema projection, and evidence commit transactional"
    )]
    fn classify_project_nominal(
        &self,
        nominal: &ProjectNominalType,
        path: &RuntimeOwnershipPath,
        depth: u64,
        traversal: &mut OwnershipTraversal,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        let Some((analysis, _, _)) = self.authority() else {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            ));
        };
        let identity = runtime_semantic_identity(&TypeKind::ProjectNominal(nominal.clone()));
        traversal.enter_nominal(identity, path)?;
        let result = (|| {
            let semantic_type =
                TypeKind::ProjectNominal(nominal.clone()).semantic_identity_digest();
            let projected = analysis
                .runtime_nominal_projection(semantic_type)
                .ok_or_else(|| {
                    RuntimeOwnershipError::rejected(
                        path,
                        RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
                    )
                })?;
            let arguments = nominal
                .arguments()
                .iter()
                .enumerate()
                .map(|(index, argument)| {
                    let argument_path =
                        path.pushed(RuntimeOwnershipPathSegment::AcceptedNominalArgument(
                            u32::try_from(index)
                                .expect("accepted project nominal argument ordinal fits u32"),
                        ));
                    self.classify_at(
                        argument,
                        &argument_path,
                        OwnershipTraversal::child_depth(depth)?,
                        traversal,
                    )?
                    .projection()
                    .checked_type_at(&argument_path)
                })
                .collect::<Result<Vec<_>, RuntimeOwnershipError>>()?;
            let checked = match projected.kind() {
                RuntimeProjectNominalKind::Record => {
                    for field in projected.record_fields() {
                        self.classify_at(
                            field.ty(),
                            &path.pushed(RuntimeOwnershipPathSegment::ProjectNominalMember(
                                field.declaration_ordinal(),
                            )),
                            OwnershipTraversal::child_depth(depth)?,
                            traversal,
                        )?;
                    }
                    RuntimeCheckedType::Nominal {
                        nominal: projected.nominal().clone(),
                        semantic_identity: identity,
                        layout: projected.layout(),
                        arguments,
                    }
                }
                RuntimeProjectNominalKind::Variant => {
                    let mut cases = Vec::with_capacity(projected.variant_cases().len());
                    for variant in projected.variant_cases() {
                        let payload = variant
                            .payload()
                            .map(|ty| {
                                let member_path =
                                    path.pushed(RuntimeOwnershipPathSegment::ProjectNominalMember(
                                        variant.ordinal(),
                                    ));
                                self.classify_at(
                                    ty,
                                    &member_path,
                                    OwnershipTraversal::child_depth(depth)?,
                                    traversal,
                                )?
                                .projection()
                                .checked_type_at(&member_path)
                                .map(Box::new)
                            })
                            .transpose()?;
                        cases.push(RuntimeCheckedVariantCase {
                            name: variant.diagnostic_name().as_str().to_owned(),
                            payload,
                        });
                    }
                    RuntimeCheckedType::Variant {
                        nominal: projected.nominal().clone(),
                        semantic_identity: identity,
                        arguments,
                        cases,
                    }
                }
            };
            let checked_type = TypeKind::ProjectNominal(nominal.clone()).semantic_identity_digest();
            traversal.push_evidence(OwnershipEvidenceRow::ProjectNominal {
                semantic_identity: *identity.as_bytes(),
                checked_type: *checked_type.as_bytes(),
                declaration_shape: *projected.layout().as_bytes(),
            })?;
            Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
                RuntimeOwnershipProjection::Nominal {
                    checked,
                    schema: projected.schema().clone(),
                    layout: projected.layout(),
                },
            ))
        })();
        traversal.leave_nominal(identity);
        result
    }

    fn classify_accepted_nominal(
        &self,
        nominal: &AcceptedNominalType,
        path: &RuntimeOwnershipPath,
        depth: u64,
        traversal: &mut OwnershipTraversal,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        let Some((_, world, _)) = self.authority() else {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            ));
        };
        for (ordinal, argument) in nominal.arguments().iter().enumerate() {
            let ordinal = u32::try_from(ordinal).unwrap_or(u32::MAX);
            self.classify_at(
                argument,
                &path.pushed(RuntimeOwnershipPathSegment::AcceptedNominalArgument(
                    ordinal,
                )),
                OwnershipTraversal::child_depth(depth)?,
                traversal,
            )?;
        }
        let record = world
            .environment()
            .nominal_catalog()
            .exact(nominal.declaration().canonical_path())
            .filter(|record| {
                record.id() == nominal.declaration()
                    && usize::from(record.arity()) == nominal.arguments().len()
            })
            .ok_or_else(|| {
                RuntimeOwnershipError::rejected(
                    path,
                    RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
                )
            })?;
        let AcceptedNominalSemantics::Opaque(carrier) = record.semantics() else {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            ));
        };
        let semantic_identity =
            runtime_semantic_identity(&TypeKind::AcceptedNominal(nominal.clone()));
        if !matches!(carrier.value_class(), RuntimeOpaqueValueClass::Plain) {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::AffineValue,
            ));
        }
        traversal.push_evidence(OwnershipEvidenceRow::AcceptedOpaque {
            semantic_identity: *semantic_identity.as_bytes(),
            runtime_producer: carrier.producer().as_str().to_owned(),
            value_class: carrier.value_class(),
            persistence: carrier.persistence(),
        })?;
        Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
            RuntimeOwnershipProjection::Checked(RuntimeCheckedType::Opaque {
                owner: RuntimeOpaqueTypeOwner::exact_with(
                    carrier.producer().clone(),
                    semantic_identity,
                    carrier.value_class(),
                    carrier.persistence(),
                ),
            }),
        ))
    }

    fn classify_environment_runtime_nominal(
        &self,
        ty: &TypeKind,
        path: &RuntimeOwnershipPath,
        traversal: &mut OwnershipTraversal,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        let Some((_, world, _)) = self.authority() else {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            ));
        };
        let semantic_identity = runtime_semantic_identity(ty);
        let carrier = world
            .environment()
            .nominal_catalog()
            .environment_record_for_semantic_type(ty.semantic_identity_digest())
            .and_then(|record| record.runtime_carrier())
            .ok_or_else(|| {
                RuntimeOwnershipError::rejected(
                    path,
                    RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
                )
            })?;
        if !matches!(carrier.value_class(), RuntimeOpaqueValueClass::Plain) {
            return Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::AffineValue,
            ));
        }
        traversal.push_evidence(OwnershipEvidenceRow::AcceptedOpaque {
            semantic_identity: *semantic_identity.as_bytes(),
            runtime_producer: carrier.producer().as_str().to_owned(),
            value_class: carrier.value_class(),
            persistence: carrier.persistence(),
        })?;
        Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
            RuntimeOwnershipProjection::Checked(RuntimeCheckedType::Opaque {
                owner: RuntimeOpaqueTypeOwner::exact_with(
                    carrier.producer().clone(),
                    semantic_identity,
                    carrier.value_class(),
                    carrier.persistence(),
                ),
            }),
        ))
    }

    fn classify_agent_builtin(
        builtin: AgentBuiltinType,
        path: &RuntimeOwnershipPath,
        traversal: &mut OwnershipTraversal,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        match builtin {
            AgentBuiltinType::ObservedObjectId
            | AgentBuiltinType::CaptureFormat
            | AgentBuiltinType::CaptureKind
            | AgentBuiltinType::WaitError
            | AgentBuiltinType::PointerButton
            | AgentBuiltinType::RagError => Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            )),
            AgentBuiltinType::Diagnostics => {
                record_agent_evidence(traversal, RuntimeAgentOperationalType::Diagnostics)?;
                Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
                    RuntimeOwnershipProjection::Checked(RuntimeCheckedType::Agent(
                        RuntimeAgentOperationalType::Diagnostics,
                    )),
                ))
            }
            AgentBuiltinType::ViewportPoint => {
                record_agent_evidence(traversal, RuntimeAgentOperationalType::ViewportPoint)?;
                Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
                    RuntimeOwnershipProjection::Checked(RuntimeCheckedType::Agent(
                        RuntimeAgentOperationalType::ViewportPoint,
                    )),
                ))
            }
        }
    }

    fn classify_map(
        kind: MapKind,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        match kind {
            MapKind::Ordered | MapKind::Sorted | MapKind::BTree => {
                Err(RuntimeOwnershipError::rejected(
                    path,
                    RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
                ))
            }
        }
    }

    fn classify_borrow(
        kind: BorrowKind,
        lifetime: Option<&LifetimeScopeKind>,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        match kind {
            BorrowKind::Shared | BorrowKind::Mutable => {
                Self::classify_borrow_lifetime(lifetime, path)
            }
        }
    }

    fn classify_borrow_lifetime(
        lifetime: Option<&LifetimeScopeKind>,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        match lifetime {
            None
            | Some(
                LifetimeScopeKind::Frame
                | LifetimeScopeKind::Tick
                | LifetimeScopeKind::Cue
                | LifetimeScopeKind::Line
                | LifetimeScopeKind::Scene
                | LifetimeScopeKind::Flow
                | LifetimeScopeKind::Session
                | LifetimeScopeKind::Global
                | LifetimeScopeKind::Persistent
                | LifetimeScopeKind::Named(_),
            ) => Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::BorrowedValue,
            )),
        }
    }

    fn classify_character_patch(
        kind: &EntityKind,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        match kind {
            EntityKind::Agent
            | EntityKind::Entry
            | EntityKind::Flow
            | EntityKind::Choice
            | EntityKind::ChoiceOption
            | EntityKind::Character
            | EntityKind::View
            | EntityKind::Action
            | EntityKind::Activity
            | EntityKind::DialogueLine
            | EntityKind::Text
            | EntityKind::Content
            | EntityKind::Input
            | EntityKind::Button
            | EntityKind::Style
            | EntityKind::Asset
            | EntityKind::Image
            | EntityKind::Animation
            | EntityKind::Capture
            | EntityKind::Hook
            | EntityKind::Signal
            | EntityKind::Metric
            | EntityKind::Scene
            | EntityKind::Test
            | EntityKind::Bench
            | EntityKind::Layer
            | EntityKind::Voice
            | EntityKind::Se
            | EntityKind::Bgm
            | EntityKind::AudioBus
            | EntityKind::MixerSnapshot
            | EntityKind::Ducking
            | EntityKind::Motion
            | EntityKind::Rig
            | EntityKind::Slot
            | EntityKind::Target
            | EntityKind::Other(_) => Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            )),
        }
    }

    fn classify_character_dialogue(
        character: &arcweft_dialogue::CharacterDialogueCharacterType,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        match character {
            arcweft_dialogue::CharacterDialogueCharacterType::Exact(_)
            | arcweft_dialogue::CharacterDialogueCharacterType::Any => {
                Err(RuntimeOwnershipError::rejected(
                    path,
                    RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
                ))
            }
        }
    }

    fn classify_character_nominal(
        nominal: &CharacterNominalType,
        path: &RuntimeOwnershipPath,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        match nominal.family() {
            CharacterNominalFamily::Look
            | CharacterNominalFamily::Part
            | CharacterNominalFamily::Variant => Err(RuntimeOwnershipError::rejected(
                path,
                RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
            )),
        }
    }
}

pub(crate) fn classify_checked_producer_arguments(
    analysis: &FinalSemanticAnalysis,
    world: &RegisteredSemanticWorld,
    types: &[&TypeKind],
    limits: CheckedOwnershipLimits,
) -> Result<(Vec<RetainedValueDisposition>, CheckedOwnershipCertificate), CheckedOwnershipError> {
    RuntimeProducerArgumentClassifier::try_new(analysis, world)
        .map_err(CheckedOwnershipError::from)?
        .classify_batch_with_evidence(types, limits)
        .map_err(CheckedOwnershipError::from)
}

impl RegisteredSemanticWorld {
    /// Classifies one complete checked type and publishes only its retention
    /// disposition plus the digest of exact evidence consulted in this world.
    /// Runtime projections and live Need certificates remain private.
    pub fn checked_ownership(
        &self,
        analysis: &FinalSemanticAnalysis,
        ty: &TypeKind,
        limits: CheckedOwnershipLimits,
    ) -> Result<CheckedOwnershipCertificate, CheckedOwnershipError> {
        let classifier = RuntimeProducerArgumentClassifier::try_new(analysis, self)
            .map_err(CheckedOwnershipError::from)?;
        let (admission, evidence) = classifier
            .classify_with_evidence(ty, limits)
            .map_err(CheckedOwnershipError::from)?;
        if matches!(admission.projection(), RuntimeOwnershipProjection::Need(_)) {
            return Err(CheckedOwnershipError::Rejected);
        }
        Ok(CheckedOwnershipCertificate {
            disposition: if admission.permits_copy() {
                RetainedValueDisposition::Copy
            } else {
                RetainedValueDisposition::SnapshotClone
            },
            evidence: OwnershipEvidenceDigest::from_consulted(evidence),
        })
    }
}

fn record_agent_evidence(
    traversal: &mut OwnershipTraversal,
    kind: RuntimeAgentOperationalType,
) -> Result<(), RuntimeOwnershipError> {
    traversal.push_evidence(OwnershipEvidenceRow::AgentDto {
        kind: kind.semantic_tag(),
        snapshot_contract: kind.snapshot_contract_digest(),
    })
}

fn validate_variant_cases(
    checked: &RuntimeCheckedType,
    path: &RuntimeOwnershipPath,
) -> Result<(), RuntimeOwnershipError> {
    match checked {
        RuntimeCheckedType::Option(_) | RuntimeCheckedType::Result { .. } => {
            if checked.variant_case(0).is_none() || checked.variant_case(1).is_none() {
                return Err(RuntimeOwnershipError::rejected(
                    path,
                    RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
                ));
            }
        }
        RuntimeCheckedType::Never
        | RuntimeCheckedType::Unit
        | RuntimeCheckedType::Bool
        | RuntimeCheckedType::Signed(_)
        | RuntimeCheckedType::Unsigned(_)
        | RuntimeCheckedType::F32
        | RuntimeCheckedType::F64
        | RuntimeCheckedType::String
        | RuntimeCheckedType::Char
        | RuntimeCheckedType::Duration
        | RuntimeCheckedType::Progress
        | RuntimeCheckedType::EntityReference
        | RuntimeCheckedType::Bytes
        | RuntimeCheckedType::Sequence(_)
        | RuntimeCheckedType::Tuple(_)
        | RuntimeCheckedType::Choice(_)
        | RuntimeCheckedType::Nominal { .. }
        | RuntimeCheckedType::Opaque { .. }
        | RuntimeCheckedType::Variant { .. }
        | RuntimeCheckedType::Agent(_) => {}
    }
    Ok(())
}

fn runtime_semantic_identity(ty: &TypeKind) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes(*ty.semantic_identity_digest().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_row::EffectRow;
    use arcweft_core::{
        pattern::RuntimeVariantIdentity,
        value::{RuntimeInt, RuntimeSeq, RuntimeUInt},
    };

    fn rejected(ty: TypeKind, reason: RuntimeOwnershipRejection) {
        let error = RuntimeProducerArgumentClassifier::for_test()
            .classify(&ty)
            .expect_err("type is intentionally rejected");
        assert_eq!(error.rejection(), Some(reason));
        assert!(error.path().segments().is_empty());
    }

    #[test]
    fn primitive_checked_carriers_are_exact_and_digestable() {
        let classifier = RuntimeProducerArgumentClassifier::for_test();
        let admission = classifier.classify(&TypeKind::I32).expect("i32 admission");
        let value = RuntimeValue::Int(RuntimeInt::I32(7));
        admission
            .validate_live_value(&value)
            .expect("exact i32 carrier");
        assert_eq!(
            admission.try_digest(&value, 128).expect("canonical digest"),
            value.try_digest(128).expect("same canonical digest owner")
        );
        assert!(admission.permits_copy());
        assert!(
            classifier
                .classify(&TypeKind::U8)
                .expect("u8 admission")
                .validate_live_value(&RuntimeValue::UInt(RuntimeUInt::U8(1)))
                .is_ok()
        );
    }

    #[test]
    fn result_and_option_use_core_variant_case_authority() {
        let classifier = RuntimeProducerArgumentClassifier::for_test();
        let result = classifier
            .classify(&TypeKind::Result {
                ok: Box::new(TypeKind::I32),
                error: Box::new(TypeKind::String),
            })
            .expect("result admission");
        let value = RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Result,
            ordinal: 0,
            name: "Ok".to_owned(),
            payload: Some(Box::new(RuntimeValue::i32(3))),
        };
        result
            .validate_live_value(&value)
            .expect("core Result Some payload");
        let option = classifier
            .classify(&TypeKind::Option(Box::new(TypeKind::String)))
            .expect("option admission");
        option
            .validate_live_value(&RuntimeValue::Variant {
                owner: RuntimeVariantIdentity::Option,
                ordinal: 1,
                name: "None".to_owned(),
                payload: None,
            })
            .expect("core Option None payload");
    }

    #[test]
    fn nested_first_rejection_retains_typed_source_order_path() {
        let ty = TypeKind::Tuple(vec![
            TypeKind::I32,
            TypeKind::Tuple(vec![
                TypeKind::Bool,
                TypeKind::Shared(Box::new(TypeKind::I8)),
            ]),
        ]);
        let error = RuntimeProducerArgumentClassifier::for_test()
            .classify(&ty)
            .expect_err("Shared is not admitted");
        assert_eq!(
            error.rejection(),
            Some(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner)
        );
        assert_eq!(
            error.path().segments(),
            &[
                RuntimeOwnershipPathSegment::TupleItem(1),
                RuntimeOwnershipPathSegment::TupleItem(1),
            ]
        );
    }

    #[test]
    fn sequence_and_array_validation_use_exact_carriers() {
        let classifier = RuntimeProducerArgumentClassifier::for_test();
        let sequence = classifier
            .classify(&TypeKind::Vec(Box::new(TypeKind::U8)))
            .expect("Vec admission");
        sequence
            .validate_live_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::u8(1),
                RuntimeValue::u8(2),
            ])))
            .expect("Vec carrier");
        let array = classifier
            .classify(&TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Const(2),
            })
            .expect("array admission");
        assert!(matches!(
            array.validate_live_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::Bool(true),
            ]))),
            Err(RuntimeOwnershipError::ArrayLengthMismatch { .. })
        ));
    }

    #[test]
    fn agent_builtin_successes_use_existing_core_agent_carriers() {
        let classifier = RuntimeProducerArgumentClassifier::for_test();
        let diagnostics = classifier
            .classify(&TypeKind::AgentBuiltin(AgentBuiltinType::Diagnostics))
            .expect("Diagnostics admission");
        diagnostics
            .validate_live_value(&RuntimeValue::Agent(
                arcweft_core::value::RuntimeAgentValue::Diagnostics,
            ))
            .expect("Diagnostics carrier");
        let viewport = classifier
            .classify(&TypeKind::AgentBuiltin(AgentBuiltinType::ViewportPoint))
            .expect("ViewportPoint admission");
        viewport
            .validate_live_value(&RuntimeValue::Agent(
                arcweft_core::value::RuntimeAgentValue::ViewportPoint { x: 3, y: 4 },
            ))
            .expect("ViewportPoint carrier");
        rejected(
            TypeKind::AgentBuiltin(AgentBuiltinType::CaptureKind),
            RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner,
        );
    }

    #[test]
    fn need_uses_payload_identity_without_recursing_payload_retainability() {
        let payload = TypeKind::Stream {
            item: Box::new(TypeKind::I32),
            error: Box::new(TypeKind::Unit),
        };
        let admission = RuntimeProducerArgumentClassifier::for_test()
            .classify(&TypeKind::Need(Box::new(payload.clone())))
            .expect("Need certificate is computed inside sema");
        let RuntimeOwnershipProjection::Need(certificate) = admission.projection() else {
            panic!("Need admission retains the private payload certificate")
        };
        assert_eq!(
            certificate.payload_identity(),
            runtime_semantic_identity(&payload)
        );
        assert_eq!(
            certificate.need_identity(),
            runtime_semantic_identity(&TypeKind::Need(Box::new(payload)))
        );
        assert_eq!(
            admission
                .validate_live_value(&RuntimeValue::Unit)
                .expect_err("Cut 5 carrier is not present")
                .rejection(),
            Some(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner)
        );
    }

    fn opaque_evidence(
        identity: u8,
        producer: &str,
        persistence: RuntimeOpaquePersistence,
    ) -> OwnershipEvidenceRow {
        OwnershipEvidenceRow::AcceptedOpaque {
            semantic_identity: [identity; 32],
            runtime_producer: producer.to_owned(),
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence,
        }
    }

    #[test]
    fn ownership_evidence_digest_sorts_and_deduplicates_consulted_rows() {
        let first = opaque_evidence(
            1,
            "std.image_handle",
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let second = OwnershipEvidenceRow::AgentDto {
            kind: RuntimeAgentOperationalType::Diagnostics.semantic_tag(),
            snapshot_contract: RuntimeAgentOperationalType::Diagnostics.snapshot_contract_digest(),
        };

        assert_eq!(
            OwnershipEvidenceDigest::from_consulted([first.clone(), second.clone()]),
            OwnershipEvidenceDigest::from_consulted([second, first.clone(), first,])
        );
    }

    #[test]
    fn ownership_evidence_digest_changes_with_consulted_opaque_carrier() {
        let constant = OwnershipEvidenceDigest::from_consulted([opaque_evidence(
            7,
            "std.image_handle",
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        )]);
        let snapshot = OwnershipEvidenceDigest::from_consulted([opaque_evidence(
            7,
            "std.image_handle",
            RuntimeOpaquePersistence::SnapshotOnly,
        )]);
        let other_producer = OwnershipEvidenceDigest::from_consulted([opaque_evidence(
            7,
            "std.other_handle",
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        )]);

        assert_ne!(constant, snapshot);
        assert_ne!(constant, other_producer);
    }

    #[test]
    fn unsupported_families_have_individual_rejection_reasons() {
        rejected(
            TypeKind::Range(Box::new(TypeKind::I32)),
            RuntimeOwnershipRejection::MissingCanonicalIdentity,
        );
        rejected(
            TypeKind::IteratorState {
                family: IteratorStateKind::Array,
                item: Box::new(TypeKind::I32),
            },
            RuntimeOwnershipRejection::FrameLocalValue,
        );
        rejected(
            TypeKind::BorrowRef {
                kind: BorrowKind::Mutable,
                lifetime: Some(LifetimeScopeKind::Persistent),
                inner: Box::new(TypeKind::I32),
            },
            RuntimeOwnershipRejection::BorrowedValue,
        );
        rejected(
            TypeKind::Function {
                params: vec![TypeKind::I32],
                return_type: Box::new(TypeKind::Unit),
                effects: EffectRow::closed(crate::effects::EffectSet::new()),
            },
            RuntimeOwnershipRejection::FunctionValueRequiresCertificate,
        );
    }
}
