//! Structured runtime-ID values.
//!
//! Runtime lookup IDs are resolved symbol paths. Source IDs and public/debug
//! labels cross this boundary through explicit constructors. Runtime ID wrappers
//! in `plan` never own source strings such as `flow.main` or public/debug labels
//! such as `flow.chapter.one.main`.

use serde::de::Error as _;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};
use thiserror::Error;

#[allow(dead_code, reason = "the canonical snapshot consumer lands in G1.2-D")]
pub(crate) mod binary;

macro_rules! runtime_u64_identity {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU64);

        impl $name {
            #[must_use]
            pub const fn get(self) -> NonZeroU64 {
                self.0
            }

            pub(crate) const fn from_allocated(raw: NonZeroU64) -> Self {
                Self(raw)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serialize_nonzero_u64(self.0, serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserialize_nonzero_u64(deserializer).map(Self::from_allocated)
            }
        }
    };
}

macro_rules! runtime_u32_identity {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU32);

        impl $name {
            #[must_use]
            pub const fn get(self) -> NonZeroU32 {
                self.0
            }

            pub(crate) const fn from_accepted_ordinal(raw: NonZeroU32) -> Self {
                Self(raw)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_u32(self.0.get())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                NonZeroU32::new(u32::deserialize(deserializer)?)
                    .map(Self::from_accepted_ordinal)
                    .ok_or_else(|| D::Error::custom("runtime identity must be nonzero"))
            }
        }
    };
}

runtime_u64_identity!(ExecutionInstanceId);
runtime_u64_identity!(RuntimeScopeInstanceId);
runtime_u64_identity!(RuntimeClosureInstanceId);
runtime_u64_identity!(RuntimeFiberInstanceId);
runtime_u64_identity!(RuntimeFrameInstanceId);
runtime_u64_identity!(RuntimeMailboxInstanceId);
runtime_u64_identity!(RuntimeChildInstanceId);
runtime_u64_identity!(RuntimeTransferInstanceId);
runtime_u64_identity!(RuntimeCleanupScopeId);
runtime_u64_identity!(RuntimeLocalSlotId);

runtime_u32_identity!(RuntimeLocalDeclarationId);
runtime_u32_identity!(RuntimeCaptureSlotId);
runtime_u32_identity!(RuntimeFrameLocalId);
runtime_u32_identity!(RuntimeMailboxLaneId);
runtime_u32_identity!(RuntimeChildPacketId);
runtime_u32_identity!(RuntimeTransferPacketId);
runtime_u32_identity!(RuntimeCleanupSlotId);
runtime_u32_identity!(RuntimeDialogueValueSlotId);
runtime_u32_identity!(RuntimeDialogueEffectSiteId);
runtime_u32_identity!(RuntimeLineTaskGroupId);
runtime_u32_identity!(RuntimeLineTaskNodeId);
runtime_u32_identity!(RuntimeDialogueMarkId);

/// Admitted number of source-ordered inline effect sites owned by one
/// dialogue content plan.
///
/// Individual site identities are derived from their dense source ordinal;
/// retaining the count avoids copying a metadata-free identity vector while
/// still preventing construction handles from minting out-of-range sites.
#[repr(transparent)]
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct RuntimeDialogueEffectSiteCount(u32);

impl RuntimeDialogueEffectSiteCount {
    #[must_use]
    pub fn try_from_len(len: usize) -> Option<Self> {
        u32::try_from(len).ok().map(Self)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, site: RuntimeDialogueEffectSiteId) -> bool {
        site.get().get() <= self.0
    }

    #[must_use]
    pub fn site(self, index: usize) -> Option<RuntimeDialogueEffectSiteId> {
        let index = u32::try_from(index).ok()?;
        if index >= self.0 {
            return None;
        }
        RuntimeDialogueEffectSiteId::from_zero_based(index as usize)
    }
}

/// Snapshot-stable identity of one runtime fiber that can own dialogue
/// activations across executor-local rescheduling.
#[repr(transparent)]
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct RuntimePersistentFiberId(u64);

impl RuntimePersistentFiberId {
    #[must_use]
    pub const fn from_allocated(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Dense zero-based identity of one handle-producing operation in a line
/// task group. Zero is valid because the identity is scoped by its dialogue
/// activation rather than used as an optional sentinel.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeLineHandleSiteId(u32);

impl RuntimeLineHandleSiteId {
    #[must_use]
    pub const fn from_zero_based(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for RuntimeLineHandleSiteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identity of one concrete activation of a dialogue content plan.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DialogueActivationId {
    artifact: crate::effect::RuntimeArtifactFingerprint,
    owner_fiber: RuntimePersistentFiberId,
    content: RuntimeDialogueContentPlanId,
    occurrence: u64,
}

impl DialogueActivationId {
    #[must_use]
    pub const fn new(
        artifact: crate::effect::RuntimeArtifactFingerprint,
        owner_fiber: RuntimePersistentFiberId,
        content: RuntimeDialogueContentPlanId,
        occurrence: u64,
    ) -> Self {
        Self {
            artifact,
            owner_fiber,
            content,
            occurrence,
        }
    }

    #[must_use]
    pub const fn artifact(&self) -> crate::effect::RuntimeArtifactFingerprint {
        self.artifact
    }

    #[must_use]
    pub const fn owner_fiber(&self) -> RuntimePersistentFiberId {
        self.owner_fiber
    }

    #[must_use]
    pub const fn content(&self) -> RuntimeDialogueContentPlanId {
        self.content
    }

    #[must_use]
    pub const fn occurrence(&self) -> u64 {
        self.occurrence
    }
}

/// Exact identity carried by every affine line handle.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeLineHandleToken {
    activation: DialogueActivationId,
    site: RuntimeLineHandleSiteId,
    issuance: u32,
}

impl RuntimeLineHandleToken {
    #[must_use]
    pub const fn new(
        activation: DialogueActivationId,
        site: RuntimeLineHandleSiteId,
        issuance: u32,
    ) -> Self {
        Self {
            activation,
            site,
            issuance,
        }
    }

    #[must_use]
    pub const fn activation(&self) -> &DialogueActivationId {
        &self.activation
    }

    #[must_use]
    pub const fn site(&self) -> RuntimeLineHandleSiteId {
        self.site
    }

    #[must_use]
    pub const fn issuance(&self) -> u32 {
        self.issuance
    }
}

impl RuntimeDialogueValueSlotId {
    /// Creates the canonical one-based identity for a document-local authored
    /// dialogue value slot.
    #[must_use]
    pub fn from_zero_based(index: usize) -> Option<Self> {
        let ordinal = u32::try_from(index).ok()?.checked_add(1)?;
        NonZeroU32::new(ordinal).map(Self::from_accepted_ordinal)
    }

    /// Returns the document-local zero-based slot index.
    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

impl RuntimeLineTaskNodeId {
    #[must_use]
    pub fn from_zero_based(index: usize) -> Option<Self> {
        let ordinal = u32::try_from(index).ok()?.checked_add(1)?;
        NonZeroU32::new(ordinal).map(Self::from_accepted_ordinal)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

impl RuntimeLineTaskGroupId {
    #[must_use]
    pub(crate) fn from_zero_based(index: usize) -> Option<Self> {
        let ordinal = u32::try_from(index).ok()?.checked_add(1)?;
        NonZeroU32::new(ordinal).map(Self::from_accepted_ordinal)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

impl RuntimeDialogueMarkId {
    #[must_use]
    pub fn from_zero_based(index: usize) -> Option<Self> {
        let ordinal = u32::try_from(index).ok()?.checked_add(1)?;
        NonZeroU32::new(ordinal).map(Self::from_accepted_ordinal)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

impl RuntimeDialogueEffectSiteId {
    #[must_use]
    pub fn from_zero_based(index: usize) -> Option<Self> {
        let ordinal = u32::try_from(index).ok()?.checked_add(1)?;
        NonZeroU32::new(ordinal).map(Self::from_accepted_ordinal)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// Contiguous plan-local identity of one interned semantic type declaration.
///
/// Unlike persisted runtime identities, this unreleased construction
/// substrate has no Serde or wire representation. Only the core plan type
/// table builder can issue a value.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimePlanTypeId(NonZeroU32);

impl RuntimePlanTypeId {
    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }

    pub(crate) const fn from_accepted_ordinal(raw: NonZeroU32) -> Self {
        Self(raw)
    }
}

/// Contiguous plan-local identity of one structured function body.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFunctionSiteId(NonZeroU32);

impl RuntimeFunctionSiteId {
    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }

    pub(crate) const fn from_accepted_ordinal(raw: NonZeroU32) -> Self {
        Self(raw)
    }
}

impl fmt::Display for RuntimeFunctionSiteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Contiguous plan-local identity of one dialogue content execution mapping.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDialogueContentPlanId(NonZeroU32);

impl RuntimeDialogueContentPlanId {
    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }

    pub(crate) const fn from_accepted_ordinal(raw: NonZeroU32) -> Self {
        Self(raw)
    }
}

impl Serialize for RuntimeDialogueContentPlanId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0.get())
    }
}

impl<'de> Deserialize<'de> for RuntimeDialogueContentPlanId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        NonZeroU32::new(u32::deserialize(deserializer)?)
            .map(Self::from_accepted_ordinal)
            .ok_or_else(|| D::Error::custom("runtime identity must be nonzero"))
    }
}

/// Failure to decode the sole internal line-handle token carrier.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeLineHandleTokenDecodeError {
    #[error("line handle token payload is not the canonical internal nominal record")]
    Shape,
    #[error("line handle token contains an invalid artifact fingerprint")]
    Artifact,
    #[error("line handle token contains an invalid dialogue content identity")]
    Content,
}

impl RuntimeLineHandleToken {
    /// Encodes the token into the sole runtime-value payload shape used by all
    /// structured, AWBC, save, and replay consumers.
    #[must_use]
    pub fn encode_payload(&self) -> crate::value::RuntimeValue {
        crate::value::RuntimeValue::NominalRecord(crate::value::RuntimeNominalRecordValue::new(
            line_handle_token_nominal(),
            line_handle_token_layout(),
            vec![
                crate::value::RuntimeValue::Seq(crate::value::RuntimeSeq::dense_bytes(
                    self.activation.artifact().as_bytes().to_vec(),
                )),
                crate::value::RuntimeValue::u64(self.activation.owner_fiber().get()),
                crate::value::RuntimeValue::u32(self.activation.content().get().get()),
                crate::value::RuntimeValue::u64(self.activation.occurrence()),
                crate::value::RuntimeValue::u32(self.site.get()),
                crate::value::RuntimeValue::u32(self.issuance),
            ],
        ))
    }

    /// Decodes only the canonical internal nominal record. Tuple, string, and
    /// debug-label representations are deliberately not accepted.
    pub fn try_decode_payload(
        value: &crate::value::RuntimeValue,
    ) -> Result<Self, RuntimeLineHandleTokenDecodeError> {
        let crate::value::RuntimeValue::NominalRecord(record) = value else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        record
            .validate_shape(&line_handle_token_nominal(), line_handle_token_layout(), 6)
            .map_err(|_| RuntimeLineHandleTokenDecodeError::Shape)?;
        let [artifact, owner_fiber, content, occurrence, site, issuance] = record.fields() else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        let crate::value::RuntimeValue::Seq(artifact) = artifact else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        let artifact: [u8; 32] = artifact
            .as_bytes()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(RuntimeLineHandleTokenDecodeError::Shape)?;
        let crate::value::RuntimeValue::UInt(crate::value::RuntimeUInt::U64(owner_fiber)) =
            owner_fiber
        else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        let crate::value::RuntimeValue::UInt(crate::value::RuntimeUInt::U32(content)) = content
        else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        let crate::value::RuntimeValue::UInt(crate::value::RuntimeUInt::U64(occurrence)) =
            occurrence
        else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        let crate::value::RuntimeValue::UInt(crate::value::RuntimeUInt::U32(site)) = site else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        let crate::value::RuntimeValue::UInt(crate::value::RuntimeUInt::U32(issuance)) = issuance
        else {
            return Err(RuntimeLineHandleTokenDecodeError::Shape);
        };
        let artifact = crate::effect::RuntimeArtifactFingerprint::try_from_bytes(artifact)
            .map_err(|_| RuntimeLineHandleTokenDecodeError::Artifact)?;
        let content = NonZeroU32::new(*content)
            .map(RuntimeDialogueContentPlanId::from_accepted_ordinal)
            .ok_or(RuntimeLineHandleTokenDecodeError::Content)?;
        Ok(Self::new(
            DialogueActivationId::new(
                artifact,
                RuntimePersistentFiberId::from_allocated(*owner_fiber),
                content,
                *occurrence,
            ),
            RuntimeLineHandleSiteId::from_zero_based(*site),
            *issuance,
        ))
    }
}

fn line_handle_token_nominal() -> crate::entry::RuntimeNominalTypeId {
    crate::entry::RuntimeNominalTypeId::try_new("std.runtime.LineHandleTokenV1")
        .expect("fixed line-handle token nominal identity is valid")
}

fn line_handle_token_layout() -> crate::entry::TypeLayoutHash {
    crate::entry::TypeLayoutHash::from_bytes(
        *blake3::hash(b"arcweft.runtime.line-handle-token.layout.v1").as_bytes(),
    )
}

impl fmt::Display for RuntimeDialogueContentPlanId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Display for RuntimePlanTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeIdCursor {
    Next(NonZeroU64),
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeIdNamespace {
    Execution,
    ExecutionReservation,
    Occurrence,
    LocalSlot,
    OwnershipTransaction,
    AffineOwner,
    FiberInstance,
    FrameInstance,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
#[error("{namespace:?} runtime identity space is exhausted")]
pub struct RuntimeIdExhausted {
    namespace: RuntimeIdNamespace,
}

impl RuntimeIdCursor {
    #[must_use]
    pub const fn initial() -> Self {
        Self::Next(NonZeroU64::MIN)
    }

    #[must_use]
    pub const fn next(self) -> Option<NonZeroU64> {
        match self {
            Self::Next(next) => Some(next),
            Self::Exhausted => None,
        }
    }

    #[must_use]
    pub const fn last_issued(self) -> Option<NonZeroU64> {
        match self {
            Self::Next(next) => NonZeroU64::new(next.get() - 1),
            Self::Exhausted => Some(NonZeroU64::MAX),
        }
    }

    #[allow(
        dead_code,
        reason = "the first allocator consumer lands in the following G1.2 stage"
    )]
    pub(crate) fn take_next(
        &mut self,
        namespace: RuntimeIdNamespace,
    ) -> Result<NonZeroU64, RuntimeIdExhausted> {
        let Self::Next(current) = *self else {
            return Err(RuntimeIdExhausted { namespace });
        };
        *self = match NonZeroU64::new(current.get().wrapping_add(1)) {
            Some(next) => Self::Next(next),
            None => Self::Exhausted,
        };
        Ok(current)
    }
}

impl RuntimeIdExhausted {
    #[must_use]
    pub const fn namespace(self) -> RuntimeIdNamespace {
        self.namespace
    }
}

impl Serialize for RuntimeIdCursor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let human_readable = serializer.is_human_readable();
        let mut state = serializer.serialize_struct(
            "RuntimeIdCursor",
            if matches!(self, Self::Next(_)) { 2 } else { 1 },
        )?;
        match self {
            Self::Next(value) => {
                state.serialize_field("state", "next")?;
                if human_readable {
                    state.serialize_field("value", &value.get().to_string())?;
                } else {
                    state.serialize_field("value", &value.get())?;
                }
            }
            Self::Exhausted => state.serialize_field("state", "exhausted")?,
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for RuntimeIdCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let value = HumanReadableCursor::deserialize(deserializer)?;
            match value {
                HumanReadableCursor::Next { value } => {
                    parse_strict_nonzero_u64(&value).map(Self::Next)
                }
                HumanReadableCursor::Exhausted {} => Ok(Self::Exhausted),
            }
        } else {
            let value = NonHumanReadableCursor::deserialize(deserializer)?;
            match value {
                NonHumanReadableCursor::Next { value } => NonZeroU64::new(value)
                    .map(Self::Next)
                    .ok_or_else(|| D::Error::custom("runtime cursor value must be nonzero")),
                NonHumanReadableCursor::Exhausted {} => Ok(Self::Exhausted),
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum HumanReadableCursor {
    Next { value: String },
    Exhausted {},
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum NonHumanReadableCursor {
    Next { value: u64 },
    Exhausted {},
}

fn serialize_nonzero_u64<S>(value: NonZeroU64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        serializer.serialize_str(&value.get().to_string())
    } else {
        serializer.serialize_u64(value.get())
    }
}

fn deserialize_nonzero_u64<'de, D>(deserializer: D) -> Result<NonZeroU64, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let raw = String::deserialize(deserializer)?;
        parse_strict_nonzero_u64(&raw)
    } else {
        NonZeroU64::new(u64::deserialize(deserializer)?)
            .ok_or_else(|| D::Error::custom("runtime identity must be nonzero"))
    }
}

fn parse_strict_nonzero_u64<E>(raw: &str) -> Result<NonZeroU64, E>
where
    E: serde::de::Error,
{
    if raw.is_empty() || raw.starts_with('0') || !raw.as_bytes().iter().all(u8::is_ascii_digit) {
        return Err(E::custom(
            "runtime u64 identity must be a canonical decimal string",
        ));
    }
    raw.parse::<u64>()
        .map_err(|_| E::custom("runtime u64 identity exceeds u64"))
        .and_then(|value| {
            NonZeroU64::new(value).ok_or_else(|| E::custom("runtime identity must be nonzero"))
        })
}

/// Namespace family attached to source declarations or public/debug labels.
///
/// Canonical runtime lookup IDs do not store this family string. The owning Rust
/// ID type owns the family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIdFamily {
    Flow,
    Entry,
    Line,
    Stream,
    View,
    Asset,
    Pure,
}

/// One validated runtime ID path segment.
///
/// This is intentionally an owned value, not an atom-table index. Runtime plans
/// are small enough that a separate interning table would make the boundary
/// harder to use without buying enough yet.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeIdSegment(String);

/// Canonical runtime lookup path.
///
/// This value stores only canonical path segments. It never includes source
/// families such as `flow`/`say` as selector prefixes.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeIdPath {
    segments: Vec<RuntimeIdSegment>,
}

/// Anchor for a source-side runtime ID reference before resolution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIdReferenceAnchor {
    /// A fully-qualified project/module root reference.
    Root,
    /// A reference relative to the current declaration scope.
    Current,
    /// A reference relative to an ancestor declaration scope.
    Parent(u16),
}

/// Source-side absolute/relative runtime ID reference.
///
/// Runtime lookup IDs should be resolved before execution. This type is the
/// explicit place for relative selectors to live while parser/HIR/lowering still
/// needs them.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeIdReference {
    anchor: RuntimeIdReferenceAnchor,
    path: RuntimeIdPath,
}

/// Public/debug label deliberately emitted for maps, logs, diagnostics, and
/// AWBC public strings.
///
/// Dots inside a label are label text. Runtime lookup code must not recover a
/// runtime ID by splitting this value.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimePublicLabel(String);

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeIdError {
    #[error("canonical {family} runtime ID is empty")]
    Empty { family: RuntimeIdFamily },
    #[error("canonical {family} runtime ID segment is empty")]
    EmptySegment { family: RuntimeIdFamily },
    #[error("canonical {family} runtime ID contains reserved source-family segment `{segment}`")]
    ReservedFamilySegment {
        family: RuntimeIdFamily,
        segment: String,
    },
    #[error("source entity `{value}` belongs to `{found}`; expected `{expected}`")]
    WrongSourceFamily {
        expected: RuntimeIdFamily,
        found: String,
        value: String,
    },
    #[error("source entity `{value}` is missing a family prefix for {expected}")]
    MissingSourceFamily {
        expected: RuntimeIdFamily,
        value: String,
    },
}

impl RuntimeIdFamily {
    #[must_use]
    pub const fn namespace(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Entry => "entry",
            Self::Line => "say",
            Self::Stream => "stream",
            Self::View => "view",
            Self::Asset => "asset",
            Self::Pure => "pure",
        }
    }

    #[must_use]
    pub fn source_families(self) -> &'static [&'static str] {
        match self {
            Self::Flow => &["flow"],
            Self::Entry => &["entry"],
            Self::Line => &["say", "line"],
            Self::Stream => &["stream"],
            Self::View => &["view"],
            Self::Asset => &["asset"],
            Self::Pure => &["pure"],
        }
    }

    #[must_use]
    pub fn flow_source_families() -> &'static [&'static str] {
        &["flow"]
    }
}

impl fmt::Display for RuntimeIdFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.namespace())
    }
}

impl RuntimeIdSegment {
    pub fn new(family: RuntimeIdFamily, value: &str) -> Result<Self, RuntimeIdError> {
        validate_segment(family, value)?;
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_source_payload(family: RuntimeIdFamily, value: &str) -> Result<Self, RuntimeIdError> {
        if value.is_empty() {
            return Err(RuntimeIdError::EmptySegment { family });
        }
        if matches!(value, "__agent_controller" | "__checked_flow") {
            return Err(RuntimeIdError::ReservedFamilySegment {
                family,
                segment: value.to_owned(),
            });
        }
        Ok(Self(value.to_owned()))
    }
}

impl RuntimeIdPath {
    pub fn from_canonical_str(
        family: RuntimeIdFamily,
        value: &str,
    ) -> Result<Self, RuntimeIdError> {
        if value.is_empty() {
            return Err(RuntimeIdError::Empty { family });
        }
        value
            .split('.')
            .map(|segment| RuntimeIdSegment::new(family, segment))
            .collect::<Result<Vec<_>, _>>()
            .map(|segments| Self { segments })
    }

    /// Decodes an already-verified runtime contract identity.
    ///
    /// Product artifacts may contain the reserved runtime-only Agent controller
    /// owner prefix. All authored source-family spellings remain rejected.
    pub(crate) fn from_runtime_contract_str(
        family: RuntimeIdFamily,
        value: &str,
    ) -> Result<Self, RuntimeIdError> {
        let mut raw_segments = value.split('.');
        let Some(first) = raw_segments.next() else {
            return Err(RuntimeIdError::Empty { family });
        };
        if !matches!(first, "__agent_controller" | "__checked_flow") {
            return Self::from_canonical_str(family, value);
        }
        let mut segments = vec![RuntimeIdSegment(first.to_owned())];
        for segment in raw_segments {
            segments.push(RuntimeIdSegment::new(family, segment)?);
        }
        if segments.len() == 1 {
            return Err(RuntimeIdError::EmptySegment { family });
        }
        Ok(Self { segments })
    }

    pub fn from_source_entity_body(
        expected: RuntimeIdFamily,
        value: &str,
        accepted_families: &[&str],
    ) -> Result<Self, RuntimeIdError> {
        let Some((found, suffix)) = value.split_once('.') else {
            return Err(RuntimeIdError::MissingSourceFamily {
                expected,
                value: value.to_owned(),
            });
        };
        if !accepted_families.contains(&found) {
            return Err(RuntimeIdError::WrongSourceFamily {
                expected,
                found: found.to_owned(),
                value: value.to_owned(),
            });
        }
        if suffix.is_empty() {
            return Err(RuntimeIdError::Empty { family: expected });
        }
        suffix
            .split('.')
            .map(|segment| RuntimeIdSegment::from_source_payload(expected, segment))
            .collect::<Result<Vec<_>, _>>()
            .map(|segments| Self { segments })
    }

    pub fn from_segments(
        family: RuntimeIdFamily,
        segments: impl IntoIterator<Item = RuntimeIdSegment>,
    ) -> Result<Self, RuntimeIdError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(RuntimeIdError::Empty { family });
        }
        Ok(Self { segments })
    }

    /// Creates one runtime-only controller path shared by every Agent entry
    /// bound to the same exact callable identity.
    pub(crate) fn for_agent_controller_callable(callable: &str) -> Self {
        let mut hasher =
            blake3::Hasher::new_derive_key("arcweft.agent-controller.callable-flow.v1");
        hasher.update(&(callable.len() as u64).to_le_bytes());
        hasher.update(callable.as_bytes());
        Self {
            segments: vec![
                RuntimeIdSegment("__agent_controller".to_owned()),
                RuntimeIdSegment(hasher.finalize().to_hex().to_string()),
            ],
        }
    }

    /// Creates the opaque one-way runtime path for an accepted structural
    /// Flow declaration. The semantic digest is never decoded back into HIR.
    pub(crate) fn for_checked_flow_declaration(digest: [u8; 32]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(64);
        for byte in digest {
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Self {
            segments: vec![
                RuntimeIdSegment("__checked_flow".to_owned()),
                RuntimeIdSegment(encoded),
            ],
        }
    }

    #[must_use]
    pub fn segments(&self) -> &[RuntimeIdSegment] {
        &self.segments
    }

    #[must_use]
    pub fn label(&self) -> String {
        self.segments
            .iter()
            .map(RuntimeIdSegment::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl fmt::Display for RuntimeIdPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut segments = self.segments.iter();
        if let Some(first) = segments.next() {
            f.write_str(first.as_str())?;
        }
        for segment in segments {
            f.write_str(".")?;
            f.write_str(segment.as_str())?;
        }
        Ok(())
    }
}

impl RuntimeIdReference {
    #[must_use]
    pub const fn new(anchor: RuntimeIdReferenceAnchor, path: RuntimeIdPath) -> Self {
        Self { anchor, path }
    }

    #[must_use]
    pub const fn anchor(&self) -> RuntimeIdReferenceAnchor {
        self.anchor
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }
}

impl RuntimePublicLabel {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn for_family(family: RuntimeIdFamily, path: &RuntimeIdPath) -> Self {
        Self(format!("{}.{}", family.namespace(), path.label()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RuntimePublicLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_segment(family: RuntimeIdFamily, value: &str) -> Result<(), RuntimeIdError> {
    if value.is_empty() {
        return Err(RuntimeIdError::EmptySegment { family });
    }
    if reserved_family_segment(value) {
        return Err(RuntimeIdError::ReservedFamilySegment {
            family,
            segment: value.to_owned(),
        });
    }
    Ok(())
}

fn reserved_family_segment(value: &str) -> bool {
    matches!(
        value,
        "flow"
            | "fragment"
            | "frag"
            | "entry"
            | "say"
            | "line"
            | "stream"
            | "view"
            | "asset"
            | "pure"
            | "__agent_controller"
            | "__checked_flow"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_identity_json_is_strict_and_canonical() {
        let execution = ExecutionInstanceId::from_allocated(NonZeroU64::new(7).unwrap());
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(3).unwrap());

        assert_eq!(serde_json::to_string(&execution).unwrap(), "\"7\"");
        assert_eq!(serde_json::to_string(&local).unwrap(), "3");
        assert_eq!(
            serde_json::from_str::<ExecutionInstanceId>("\"7\"").unwrap(),
            execution
        );
        assert_eq!(
            serde_json::from_str::<RuntimeLocalDeclarationId>("3").unwrap(),
            local
        );

        for invalid in ["7", "\"0\"", "\"07\"", "\"+7\"", "\" 7\"", "\"7 \""] {
            assert!(serde_json::from_str::<ExecutionInstanceId>(invalid).is_err());
        }
        assert!(serde_json::from_str::<RuntimeLocalDeclarationId>("\"3\"").is_err());
        assert!(serde_json::from_str::<RuntimeLocalDeclarationId>("0").is_err());
    }

    #[test]
    fn runtime_id_cursor_tracks_issued_high_water_mark() {
        let mut cursor = RuntimeIdCursor::initial();
        assert_eq!(cursor.next(), Some(NonZeroU64::MIN));
        assert_eq!(cursor.last_issued(), None);
        assert_eq!(
            cursor.take_next(RuntimeIdNamespace::Occurrence).unwrap(),
            NonZeroU64::MIN
        );
        assert_eq!(cursor.next(), NonZeroU64::new(2));
        assert_eq!(cursor.last_issued(), Some(NonZeroU64::MIN));
    }

    #[test]
    fn runtime_id_cursor_issues_maximum_then_exhausts() {
        let mut cursor = RuntimeIdCursor::Next(NonZeroU64::MAX);
        assert_eq!(
            cursor.take_next(RuntimeIdNamespace::AffineOwner).unwrap(),
            NonZeroU64::MAX
        );
        assert_eq!(cursor, RuntimeIdCursor::Exhausted);
        assert_eq!(cursor.last_issued(), Some(NonZeroU64::MAX));
        assert_eq!(cursor.next(), None);
        assert_eq!(
            cursor
                .take_next(RuntimeIdNamespace::AffineOwner)
                .unwrap_err()
                .namespace(),
            RuntimeIdNamespace::AffineOwner
        );
    }

    #[test]
    fn runtime_id_cursor_json_rejects_noncanonical_shapes() {
        let next = RuntimeIdCursor::initial();
        assert_eq!(
            serde_json::to_string(&next).unwrap(),
            "{\"state\":\"next\",\"value\":\"1\"}"
        );
        assert_eq!(
            serde_json::to_string(&RuntimeIdCursor::Exhausted).unwrap(),
            "{\"state\":\"exhausted\"}"
        );

        for invalid in [
            "{\"state\":\"next\"}",
            "{\"state\":\"next\",\"value\":1}",
            "{\"state\":\"next\",\"value\":\"01\"}",
            "{\"state\":\"exhausted\",\"value\":\"1\"}",
            "{\"state\":\"exhausted\",\"value\":null}",
            "{\"state\":\"unknown\"}",
            "{\"state\":\"exhausted\",\"unknown\":true}",
            "{\"state\":\"exhausted\",\"state\":\"exhausted\"}",
        ] {
            assert!(serde_json::from_str::<RuntimeIdCursor>(invalid).is_err());
        }
    }

    #[test]
    fn runtime_identity_accessors_preserve_distinct_wrappers() {
        let scope = RuntimeScopeInstanceId::from_allocated(NonZeroU64::new(2).unwrap());
        let capture = RuntimeCaptureSlotId::from_accepted_ordinal(NonZeroU32::new(4).unwrap());

        assert_eq!(scope.get(), NonZeroU64::new(2).unwrap());
        assert_eq!(capture.get(), NonZeroU32::new(4).unwrap());
        assert_eq!(scope.to_string(), "2");
        assert_eq!(capture.to_string(), "4");
    }
}
