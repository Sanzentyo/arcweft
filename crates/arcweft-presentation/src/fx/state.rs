//! Deterministic Fx time, identity state, seed derivation, and save snapshots.

use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

use super::{
    application::FxBoundApplicationTemplate,
    canonical::{CanonicalEncoder, CanonicalHashSink},
    graph::{FX_MAX_PARAMETERS_PER_DEFINITION, FxDefinition},
    identity::{FxAbiHash, FxId, FxInstanceId, FxInstanceIdentity, FxSemanticHash},
    value::{FX_GOLDEN_ANGLE_RAD, FiniteF32, FiniteF32Error, FxRuntimeValue, Length, Seconds},
};

/// Maximum number of authored child ordinals retained in nested graph identity.
pub const FX_MAX_GRAPH_CHILD_DEPTH: usize = 64;

/// Maximum number of typed values retained by one provider state record.
pub const FX_MAX_PROVIDER_STATE_VALUES: usize = 256;

/// Maximum number of provider-owned records retained by one live Fx instance.
pub const FX_MAX_PROVIDER_STATES_PER_INSTANCE: usize = 64;

const FX_PROVIDER_STATE_VERSION: u8 = 1;

/// Non-negative deterministic runtime logical time.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxLogicalTime(Seconds);

/// Bounded nested authored graph path, retained across save/load.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct FxGraphChildPath {
    ordinals: Vec<u32>,
    depth: u8,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxAuthoredSeed(u32);

/// Typed, bounded, provider-versioned save state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxProviderStateRecord {
    provider: FxId,
    version: FxProviderStateVersion,
    values: Vec<FxRuntimeValue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FxProviderStateVersion;

/// Complete persisted state for one live Fx application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FxInstanceSnapshot {
    identity: FxInstanceIdentity,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
    activation_logical_time: FxLogicalTime,
    authored_seed: Option<FxAuthoredSeed>,
    template: Arc<FxBoundApplicationTemplate>,
    parameters: Box<[FxRuntimeValue]>,
    child_path: FxGraphChildPath,
    provider_state: Vec<FxProviderStateRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FxInstanceActivation {
    logical_time: FxLogicalTime,
    authored_seed: Option<FxAuthoredSeed>,
    child_path: FxGraphChildPath,
}

/// Invalid bounded runtime/save state.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxInstanceSnapshotError {
    #[error("logical time cannot be negative")]
    NegativeLogicalTime,
    #[error("Fx child path exceeds the maximum depth of {limit}")]
    ChildPathTooDeep { limit: usize },
    #[error("Fx snapshot has {actual} parameters, exceeding the limit of {limit}")]
    TooManyParameters { actual: usize, limit: usize },
    #[error("provider state has {actual} values, exceeding the limit of {limit}")]
    ProviderStateTooLarge { actual: usize, limit: usize },
    #[error("Fx snapshot has {actual} provider records, exceeding the limit of {limit}")]
    TooManyProviderStates { actual: usize, limit: usize },
    #[error("Fx snapshot repeats provider state for `{provider}`")]
    DuplicateProviderState { provider: Box<FxId> },
    #[error("Fx snapshot definition `{snapshot}` does not match `{actual}`")]
    DefinitionMismatch {
        snapshot: Box<FxId>,
        actual: Box<FxId>,
    },
    #[error("Fx snapshot instance identity does not match its owner and authored ordinal")]
    IdentityMismatch,
    #[error("Fx snapshot ABI does not match definition `{definition}`")]
    AbiMismatch { definition: Box<FxId> },
    #[error("Fx snapshot semantic hash does not match definition `{definition}`")]
    SemanticMismatch { definition: Box<FxId> },
    #[error("Fx snapshot parameter layout does not match definition `{definition}`")]
    LayoutMismatch { definition: Box<FxId> },
    #[error(
        "Fx snapshot has {actual} parameters, but definition `{definition}` requires {expected}"
    )]
    ParameterCount {
        definition: Box<FxId>,
        expected: usize,
        actual: usize,
    },
    #[error(
        "Fx snapshot parameter {index} for `{definition}` has type {actual:?}, expected {expected:?}"
    )]
    ParameterType {
        definition: Box<FxId>,
        index: usize,
        expected: super::value::FxRuntimeType,
        actual: super::value::FxRuntimeType,
    },
}

/// Per-target deterministic sampler context.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FxSampleGeometry {
    target_center_x: Length,
    target_center_y: Length,
    glyph_center_x: Length,
    glyph_center_y: Length,
}

impl FxSampleGeometry {
    pub const fn new(
        target_center_x: Length,
        target_center_y: Length,
        glyph_center_x: Length,
        glyph_center_y: Length,
    ) -> Self {
        Self {
            target_center_x,
            target_center_y,
            glyph_center_x,
            glyph_center_y,
        }
    }

    pub const fn target_center(self) -> [Length; 2] {
        [self.target_center_x, self.target_center_y]
    }

    pub const fn glyph_center(self) -> [Length; 2] {
        [self.glyph_center_x, self.glyph_center_y]
    }
}

/// Per-target deterministic sampler context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxSampleContext {
    time: FiniteF32,
    ordinal: u32,
    deterministic_seed: u64,
    reduce_motion: bool,
    geometry: FxSampleGeometry,
}

impl FxLogicalTime {
    pub fn try_new(value: Seconds) -> Result<Self, FxInstanceSnapshotError> {
        if value.seconds() < 0.0 {
            Err(FxInstanceSnapshotError::NegativeLogicalTime)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn zero() -> Self {
        Self(Seconds::ZERO)
    }

    pub const fn seconds(self) -> Seconds {
        self.0
    }

    /// Advances the deterministic clock through the single finite-`f32` boundary.
    pub fn try_advance_millis(self, milliseconds: u64) -> Result<Self, FiniteF32Error> {
        #[expect(
            clippy::cast_precision_loss,
            reason = "logical milliseconds are intentionally narrowed once into the specified f32 Fx time domain"
        )]
        let delta_seconds = milliseconds as f64 / 1_000.0;
        Seconds::try_seconds_f64(f64::from(self.0.seconds()) + delta_seconds).map(Self)
    }
}

impl<'de> Deserialize<'de> for FxLogicalTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(Seconds::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl FxGraphChildPath {
    pub fn try_new(ordinals: Vec<u32>) -> Result<Self, FxInstanceSnapshotError> {
        if ordinals.len() > FX_MAX_GRAPH_CHILD_DEPTH {
            return Err(FxInstanceSnapshotError::ChildPathTooDeep {
                limit: FX_MAX_GRAPH_CHILD_DEPTH,
            });
        }
        let depth = u8::try_from(ordinals.len()).map_err(|_| {
            FxInstanceSnapshotError::ChildPathTooDeep {
                limit: FX_MAX_GRAPH_CHILD_DEPTH,
            }
        })?;
        Ok(Self { ordinals, depth })
    }

    pub fn ordinals(&self) -> &[u32] {
        &self.ordinals
    }

    pub const fn depth(&self) -> u8 {
        self.depth
    }

    pub fn try_with_child(&self, ordinal: u32) -> Result<Self, FxInstanceSnapshotError> {
        let mut ordinals = self.ordinals.clone();
        ordinals.push(ordinal);
        Self::try_new(ordinals)
    }
}

impl Serialize for FxGraphChildPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.ordinals.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FxGraphChildPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(Vec::<u32>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl FxAuthoredSeed {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl FxProviderStateRecord {
    pub fn try_new(
        provider: FxId,
        values: Vec<FxRuntimeValue>,
    ) -> Result<Self, FxInstanceSnapshotError> {
        if values.len() > FX_MAX_PROVIDER_STATE_VALUES {
            return Err(FxInstanceSnapshotError::ProviderStateTooLarge {
                actual: values.len(),
                limit: FX_MAX_PROVIDER_STATE_VALUES,
            });
        }
        Ok(Self {
            provider,
            version: FxProviderStateVersion,
            values,
        })
    }

    pub const fn provider(&self) -> &FxId {
        &self.provider
    }

    pub const fn version(&self) -> u8 {
        FX_PROVIDER_STATE_VERSION
    }

    pub fn values(&self) -> &[FxRuntimeValue] {
        &self.values
    }
}

impl FxInstanceActivation {
    pub const fn new(
        logical_time: FxLogicalTime,
        authored_seed: Option<FxAuthoredSeed>,
        child_path: FxGraphChildPath,
    ) -> Self {
        Self {
            logical_time,
            authored_seed,
            child_path,
        }
    }

    pub const fn logical_time(&self) -> FxLogicalTime {
        self.logical_time
    }

    pub const fn authored_seed(&self) -> Option<FxAuthoredSeed> {
        self.authored_seed
    }

    pub const fn child_path(&self) -> &FxGraphChildPath {
        &self.child_path
    }
}

#[derive(Deserialize)]
struct FxProviderStateWire {
    provider: FxId,
    version: FxProviderStateVersion,
    values: Vec<FxRuntimeValue>,
}

impl<'de> Deserialize<'de> for FxProviderStateRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxProviderStateWire::deserialize(deserializer)?;
        let _ = wire.version;
        Self::try_new(wire.provider, wire.values).map_err(D::Error::custom)
    }
}

impl Serialize for FxProviderStateVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(FX_PROVIDER_STATE_VERSION)
    }
}

impl<'de> Deserialize<'de> for FxProviderStateVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let actual = u8::deserialize(deserializer)?;
        if actual != FX_PROVIDER_STATE_VERSION {
            return Err(D::Error::custom(format_args!(
                "unsupported Fx provider state version {actual}"
            )));
        }
        Ok(Self)
    }
}

impl FxInstanceSnapshot {
    pub fn try_new(
        identity: FxInstanceIdentity,
        definition: &FxDefinition,
        activation: FxInstanceActivation,
        template: Arc<FxBoundApplicationTemplate>,
        parameters: Box<[FxRuntimeValue]>,
        provider_state: Vec<FxProviderStateRecord>,
    ) -> Result<Self, FxInstanceSnapshotError> {
        let snapshot = Self {
            identity,
            abi_hash: definition.abi_hash(),
            semantic_hash: definition.semantic_hash(),
            activation_logical_time: activation.logical_time,
            authored_seed: activation.authored_seed,
            template,
            parameters,
            child_path: activation.child_path,
            provider_state,
        }
        .validate()?;
        snapshot.validate_for_definition(definition)?;
        Ok(snapshot)
    }

    pub const fn identity(&self) -> &FxInstanceIdentity {
        &self.identity
    }

    pub const fn instance(&self) -> FxInstanceId {
        self.identity.instance()
    }

    pub const fn definition(&self) -> &FxId {
        self.identity.definition()
    }

    pub const fn abi_hash(&self) -> FxAbiHash {
        self.abi_hash
    }

    pub const fn semantic_hash(&self) -> FxSemanticHash {
        self.semantic_hash
    }

    pub const fn activation_logical_time(&self) -> FxLogicalTime {
        self.activation_logical_time
    }

    pub const fn authored_seed(&self) -> Option<FxAuthoredSeed> {
        self.authored_seed
    }

    pub fn template(&self) -> &Arc<FxBoundApplicationTemplate> {
        &self.template
    }

    pub fn parameters(&self) -> &[FxRuntimeValue] {
        &self.parameters
    }

    pub const fn child_path(&self) -> &FxGraphChildPath {
        &self.child_path
    }

    pub fn provider_state(&self) -> &[FxProviderStateRecord] {
        &self.provider_state
    }

    pub fn deterministic_seed(&self) -> u64 {
        derive_deterministic_seed(
            self.instance(),
            self.semantic_hash,
            self.authored_seed,
            &self.child_path,
        )
    }

    pub fn try_with_parameters(
        mut self,
        parameters: Box<[FxRuntimeValue]>,
        definition: &FxDefinition,
    ) -> Result<Self, FxInstanceSnapshotError> {
        self.parameters = parameters;
        self.validate_for_definition(definition)?;
        Ok(self)
    }

    /// Validates all bounded collections after programmatic construction.
    pub fn validate(mut self) -> Result<Self, FxInstanceSnapshotError> {
        if self.parameters.len() > FX_MAX_PARAMETERS_PER_DEFINITION {
            return Err(FxInstanceSnapshotError::TooManyParameters {
                actual: self.parameters.len(),
                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
            });
        }
        if self.provider_state.len() > FX_MAX_PROVIDER_STATES_PER_INSTANCE {
            return Err(FxInstanceSnapshotError::TooManyProviderStates {
                actual: self.provider_state.len(),
                limit: FX_MAX_PROVIDER_STATES_PER_INSTANCE,
            });
        }
        self.provider_state
            .sort_by(|left, right| left.provider().cmp(right.provider()));
        for pair in self.provider_state.windows(2) {
            if pair[0].provider() == pair[1].provider() {
                return Err(FxInstanceSnapshotError::DuplicateProviderState {
                    provider: Box::new(pair[0].provider().clone()),
                });
            }
        }
        Ok(self)
    }

    /// Checks the persisted ABI and reactive parameter snapshot against one definition.
    pub fn validate_for_definition(
        &self,
        definition: &FxDefinition,
    ) -> Result<(), FxInstanceSnapshotError> {
        if self.definition() != definition.id() {
            return Err(FxInstanceSnapshotError::DefinitionMismatch {
                snapshot: Box::new(self.definition().clone()),
                actual: Box::new(definition.id().clone()),
            });
        }
        if self.abi_hash != definition.abi_hash() {
            return Err(FxInstanceSnapshotError::AbiMismatch {
                definition: Box::new(self.definition().clone()),
            });
        }
        if self.semantic_hash != definition.semantic_hash() {
            return Err(FxInstanceSnapshotError::SemanticMismatch {
                definition: Box::new(self.definition().clone()),
            });
        }
        if self.parameters.len() != definition.parameter_layout().runtime_rows().len() {
            return Err(FxInstanceSnapshotError::ParameterCount {
                definition: Box::new(self.definition().clone()),
                expected: definition.parameter_layout().runtime_rows().len(),
                actual: self.parameters.len(),
            });
        }
        if self.template.layout_digest() != definition.parameter_layout().digest()
            || self.template.initial_runtime().len()
                != definition.parameter_layout().runtime_rows().len()
            || self.template.static_arguments().len()
                != definition.parameter_layout().static_rows().len()
        {
            return Err(FxInstanceSnapshotError::LayoutMismatch {
                definition: Box::new(self.definition().clone()),
            });
        }
        for (value, row) in self
            .template
            .initial_runtime()
            .iter()
            .zip(definition.parameter_layout().runtime_rows())
        {
            if value.value_type() != row.reference().runtime_type() {
                return Err(FxInstanceSnapshotError::LayoutMismatch {
                    definition: Box::new(self.definition().clone()),
                });
            }
        }
        for (value, row) in self
            .template
            .static_arguments()
            .iter()
            .zip(definition.parameter_layout().static_rows())
        {
            if value.parameter_type() != row.parameter().parameter_type() {
                return Err(FxInstanceSnapshotError::LayoutMismatch {
                    definition: Box::new(self.definition().clone()),
                });
            }
            if let super::FxStaticDefinitionArgumentValue::UniformRecord(record) = value {
                record
                    .validate_definition_layout(definition.parameter_layout())
                    .map_err(|_| FxInstanceSnapshotError::LayoutMismatch {
                        definition: Box::new(self.definition().clone()),
                    })?;
            }
        }
        for (index, (value, parameter)) in self
            .parameters
            .iter()
            .zip(definition.parameter_layout().runtime_rows())
            .enumerate()
        {
            let expected = parameter.reference().runtime_type();
            if value.value_type() != expected {
                return Err(FxInstanceSnapshotError::ParameterType {
                    definition: Box::new(self.definition().clone()),
                    index,
                    expected,
                    actual: value.value_type(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct FxInstanceSnapshotWire {
    identity: FxInstanceIdentity,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
    activation_logical_time: FxLogicalTime,
    authored_seed: Option<FxAuthoredSeed>,
    template: FxBoundApplicationTemplate,
    parameters: Vec<FxRuntimeValue>,
    child_path: FxGraphChildPath,
    provider_state: Vec<FxProviderStateRecord>,
}

impl<'de> Deserialize<'de> for FxInstanceSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxInstanceSnapshotWire::deserialize(deserializer)?;
        Self {
            identity: wire.identity,
            abi_hash: wire.abi_hash,
            semantic_hash: wire.semantic_hash,
            activation_logical_time: wire.activation_logical_time,
            authored_seed: wire.authored_seed,
            template: Arc::new(wire.template),
            parameters: wire.parameters.into_boxed_slice(),
            child_path: wire.child_path,
            provider_state: wire.provider_state,
        }
        .validate()
        .map_err(D::Error::custom)
    }
}

#[derive(Serialize)]
struct FxInstanceSnapshotSerializeWire<'a> {
    identity: &'a FxInstanceIdentity,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
    activation_logical_time: FxLogicalTime,
    authored_seed: Option<FxAuthoredSeed>,
    template: &'a FxBoundApplicationTemplate,
    parameters: &'a [FxRuntimeValue],
    child_path: &'a FxGraphChildPath,
    provider_state: &'a [FxProviderStateRecord],
}

impl Serialize for FxInstanceSnapshot {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FxInstanceSnapshotSerializeWire {
            identity: &self.identity,
            abi_hash: self.abi_hash,
            semantic_hash: self.semantic_hash,
            activation_logical_time: self.activation_logical_time,
            authored_seed: self.authored_seed,
            template: &self.template,
            parameters: &self.parameters,
            child_path: &self.child_path,
            provider_state: &self.provider_state,
        }
        .serialize(serializer)
    }
}

impl FxSampleContext {
    /// Builds activation-relative time from deterministic logical clocks.
    pub fn from_logical_times(
        runtime_time: FxLogicalTime,
        activation_time: FxLogicalTime,
        ordinal: u32,
        deterministic_seed: u64,
        reduce_motion: bool,
    ) -> Result<Self, FiniteF32Error> {
        let elapsed = if reduce_motion {
            0.0
        } else {
            (runtime_time.seconds().seconds() - activation_time.seconds().seconds()).max(0.0)
        };
        Ok(Self {
            time: FiniteF32::try_new(elapsed)?,
            ordinal,
            deterministic_seed,
            reduce_motion,
            geometry: FxSampleGeometry::default(),
        })
    }

    pub fn from_elapsed(
        elapsed: Seconds,
        ordinal: u32,
        deterministic_seed: u64,
        reduce_motion: bool,
    ) -> Self {
        let time = if reduce_motion || elapsed.seconds() < 0.0 {
            FiniteF32::ZERO
        } else {
            elapsed.value()
        };
        Self {
            time,
            ordinal,
            deterministic_seed,
            reduce_motion,
            geometry: FxSampleGeometry::default(),
        }
    }

    #[must_use]
    pub const fn with_geometry(mut self, geometry: FxSampleGeometry) -> Self {
        self.geometry = geometry;
        self
    }

    pub const fn time(self) -> FiniteF32 {
        self.time
    }

    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    pub fn ordinal_value(self) -> Result<FiniteF32, FiniteF32Error> {
        FiniteF32::try_new(u32_as_f32(self.ordinal))
    }

    pub const fn deterministic_seed(self) -> u64 {
        self.deterministic_seed
    }

    pub const fn reduce_motion(self) -> bool {
        self.reduce_motion
    }

    pub const fn geometry(self) -> FxSampleGeometry {
        self.geometry
    }

    pub fn ordinal_phase(self) -> Result<FiniteF32, FiniteF32Error> {
        let phase =
            (u32_as_f32(self.ordinal) * FX_GOLDEN_ANGLE_RAD).rem_euclid(std::f32::consts::TAU);
        FiniteF32::try_new(phase)
    }

    /// Samples deterministic hash-noise from this instance seed, logical
    /// ordinal, and an authored integer time bucket.
    pub fn deterministic_noise(self, bucket: i32) -> Result<FiniteF32, FiniteF32Error> {
        let mut hasher = blake3::Hasher::new();
        {
            let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
            let encoding = (|| {
                encoder.domain_v1(b"arcweft.fx-noise")?;
                encoder.unsigned(self.deterministic_seed)?;
                encoder.unsigned(u64::from(self.ordinal))?;
                encoder.signed_i32(bucket)
            })();
            match encoding {
                Ok(()) => {}
                Err(error) => match error {},
            }
        }
        let digest = hasher.finalize();
        let bytes = digest.as_bytes();
        let raw = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let mantissa = raw >> 8;
        FiniteF32::try_new(u32_as_f32(mantissa) / 16_777_216.0)
    }
}

/// Derives the default deterministic seed from application and nested graph identity.
pub fn derive_deterministic_seed(
    instance: FxInstanceId,
    semantic_hash: FxSemanticHash,
    authored_seed: Option<FxAuthoredSeed>,
    child_path: &FxGraphChildPath,
) -> u64 {
    let mut hasher = blake3::Hasher::new();
    {
        let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
        let encoding: Result<(), std::convert::Infallible> = (|| {
            encoder.domain_v1(b"arcweft.fx-seed")?;
            encoder.digest32(instance.as_bytes())?;
            encoder.digest32(semantic_hash.as_bytes())?;
            match authored_seed {
                Some(seed) => {
                    encoder.boolean(true)?;
                    encoder.unsigned(u64::from(seed.get()))?;
                }
                None => encoder.boolean(false)?,
            }
            encoder.unsigned(u64::from(child_path.depth()))?;
            for ordinal in child_path.ordinals() {
                encoder.unsigned(u64::from(*ordinal))?;
            }
            Ok(())
        })();
        match encoding {
            Ok(()) => {}
            Err(error) => match error {},
        }
    }
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn u32_as_f32(value: u32) -> f32 {
    let bytes = value.to_le_bytes();
    let low = u16::from_le_bytes([bytes[0], bytes[1]]);
    let high = u16::from_le_bytes([bytes[2], bytes[3]]);
    f32::from(high) * 65_536.0 + f32::from(low)
}
