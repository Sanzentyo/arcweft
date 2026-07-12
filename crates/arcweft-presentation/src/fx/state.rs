//! Deterministic Fx time, identity state, seed derivation, and save snapshots.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use super::{
    graph::{FX_MAX_PARAMETERS_PER_DEFINITION, FxDefinition},
    identity::{FxAbiHash, FxId, FxInstanceId, FxSemanticHash, hash_bytes, hash_str},
    value::{FX_GOLDEN_ANGLE_RAD, FiniteF32, FiniteF32Error, FxRuntimeValue, Length, Seconds},
};

/// Maximum number of authored child ordinals retained in nested graph identity.
pub const FX_MAX_GRAPH_CHILD_DEPTH: usize = 64;

/// Maximum number of typed values retained by one provider state record.
pub const FX_MAX_PROVIDER_STATE_VALUES: usize = 256;

/// Maximum number of provider-owned records retained by one live Fx instance.
pub const FX_MAX_PROVIDER_STATES_PER_INSTANCE: usize = 64;

/// Non-negative deterministic runtime logical time.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxLogicalTime(Seconds);

/// Bounded nested authored graph path, retained across save/load.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxGraphChildPath(Vec<u32>);

/// Typed, bounded, provider-versioned save state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxProviderStateRecord {
    provider: FxId,
    version: u32,
    values: Vec<FxRuntimeValue>,
}

/// Complete persisted state for one live Fx application.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxInstanceSnapshot {
    pub instance: FxInstanceId,
    pub definition: FxId,
    pub abi_hash: FxAbiHash,
    pub activation_logical_time: FxLogicalTime,
    pub deterministic_seed: u64,
    pub parameters: Vec<FxRuntimeValue>,
    pub child_path: FxGraphChildPath,
    pub provider_state: Vec<FxProviderStateRecord>,
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
    DuplicateProviderState { provider: FxId },
    #[error("Fx snapshot definition `{snapshot}` does not match `{actual}`")]
    DefinitionMismatch { snapshot: FxId, actual: FxId },
    #[error("Fx snapshot ABI does not match definition `{definition}`")]
    AbiMismatch { definition: FxId },
    #[error(
        "Fx snapshot has {actual} parameters, but definition `{definition}` requires {expected}"
    )]
    ParameterCount {
        definition: FxId,
        expected: usize,
        actual: usize,
    },
    #[error(
        "Fx snapshot parameter {index} for `{definition}` has type {actual:?}, expected {expected:?}"
    )]
    ParameterType {
        definition: FxId,
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
        Ok(Self(ordinals))
    }

    pub fn ordinals(&self) -> &[u32] {
        &self.0
    }

    pub fn try_with_child(&self, ordinal: u32) -> Result<Self, FxInstanceSnapshotError> {
        let mut ordinals = self.0.clone();
        ordinals.push(ordinal);
        Self::try_new(ordinals)
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

impl FxProviderStateRecord {
    pub fn try_new(
        provider: FxId,
        version: u32,
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
            version,
            values,
        })
    }

    pub const fn provider(&self) -> &FxId {
        &self.provider
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub fn values(&self) -> &[FxRuntimeValue] {
        &self.values
    }
}

#[derive(Deserialize)]
struct FxProviderStateWire {
    provider: FxId,
    version: u32,
    values: Vec<FxRuntimeValue>,
}

impl<'de> Deserialize<'de> for FxProviderStateRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxProviderStateWire::deserialize(deserializer)?;
        Self::try_new(wire.provider, wire.version, wire.values).map_err(D::Error::custom)
    }
}

impl FxInstanceSnapshot {
    /// Validates all bounded collections after programmatic construction.
    pub fn validate(self) -> Result<Self, FxInstanceSnapshotError> {
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
        let mut providers = BTreeSet::new();
        for state in &self.provider_state {
            if !providers.insert(state.provider()) {
                return Err(FxInstanceSnapshotError::DuplicateProviderState {
                    provider: state.provider().clone(),
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
        if &self.definition != definition.id() {
            return Err(FxInstanceSnapshotError::DefinitionMismatch {
                snapshot: self.definition.clone(),
                actual: definition.id().clone(),
            });
        }
        if self.abi_hash != definition.abi_hash() {
            return Err(FxInstanceSnapshotError::AbiMismatch {
                definition: self.definition.clone(),
            });
        }
        if self.parameters.len() != definition.parameters().len() {
            return Err(FxInstanceSnapshotError::ParameterCount {
                definition: self.definition.clone(),
                expected: definition.parameters().len(),
                actual: self.parameters.len(),
            });
        }
        for (index, (value, parameter)) in self
            .parameters
            .iter()
            .zip(definition.parameters())
            .enumerate()
        {
            if value.value_type() != parameter.value_type() {
                return Err(FxInstanceSnapshotError::ParameterType {
                    definition: self.definition.clone(),
                    index,
                    expected: parameter.value_type(),
                    actual: value.value_type(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct FxInstanceSnapshotWire {
    instance: FxInstanceId,
    definition: FxId,
    abi_hash: FxAbiHash,
    activation_logical_time: FxLogicalTime,
    deterministic_seed: u64,
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
            instance: wire.instance,
            definition: wire.definition,
            abi_hash: wire.abi_hash,
            activation_logical_time: wire.activation_logical_time,
            deterministic_seed: wire.deterministic_seed,
            parameters: wire.parameters,
            child_path: wire.child_path,
            provider_state: wire.provider_state,
        }
        .validate()
        .map_err(D::Error::custom)
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
        hash_str(&mut hasher, "arcweft.fx-noise.v1");
        hasher.update(&self.deterministic_seed.to_le_bytes());
        hasher.update(&self.ordinal.to_le_bytes());
        hasher.update(&bucket.to_le_bytes());
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
    authored_seed: Option<&[u8]>,
    child_path: &FxGraphChildPath,
) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hash_str(&mut hasher, "arcweft.fx-seed.v1");
    hash_bytes(&mut hasher, instance.as_bytes());
    hash_bytes(&mut hasher, semantic_hash.as_bytes());
    match authored_seed {
        Some(seed) => {
            hasher.update(&[1]);
            hash_bytes(&mut hasher, seed);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(&(child_path.ordinals().len() as u64).to_le_bytes());
    for ordinal in child_path.ordinals() {
        hasher.update(&ordinal.to_le_bytes());
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
