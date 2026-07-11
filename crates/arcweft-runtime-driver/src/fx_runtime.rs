//! Deterministic live Fx instances owned by the portable runtime driver.

use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_presentation::fx::{
    FiniteF32Error, FxAbiHash, FxDefinition, FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext,
    FxGraphChildPath, FxId, FxInstanceId, FxInstanceSnapshot, FxInstanceSnapshotError,
    FxLogicalTime, FxRuntimeType, FxRuntimeValue, derive_deterministic_seed,
};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

/// Maximum number of live applications retained in one session snapshot.
pub const MAX_LIVE_FX_INSTANCES: usize = 65_536;

/// Portable logical clock and canonically ordered live Fx applications.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BundleFxRuntimeSnapshot {
    pub logical_time: FxLogicalTime,
    pub instances: Vec<FxInstanceSnapshot>,
}

/// Invalid live state, parameter update, or definition binding.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BundleFxRuntimeError {
    #[error(transparent)]
    LogicalTime(#[from] FiniteF32Error),
    #[error("Fx runtime has {actual} live instances, exceeding the limit of {limit}")]
    TooManyInstances { actual: usize, limit: usize },
    #[error("Fx runtime repeats live instance {instance:?}")]
    DuplicateInstance { instance: FxInstanceId },
    #[error("Fx runtime has no definition `{definition}` for instance {instance:?}")]
    MissingDefinition {
        definition: Box<FxId>,
        instance: FxInstanceId,
    },
    #[error(
        "Fx instance {instance:?} ABI for `{definition}` does not match the active definition (saved {saved:?}, actual {actual:?})"
    )]
    AbiMismatch {
        definition: Box<FxId>,
        instance: FxInstanceId,
        saved: FxAbiHash,
        actual: FxAbiHash,
    },
    #[error(
        "Fx instance {instance:?} has {actual} parameters for `{definition}`, expected {expected}"
    )]
    ParameterCount {
        definition: Box<FxId>,
        instance: FxInstanceId,
        expected: usize,
        actual: usize,
    },
    #[error(
        "Fx instance {instance:?} parameter {index} for `{definition}` has type {actual:?}, expected {expected:?}"
    )]
    ParameterType {
        definition: Box<FxId>,
        instance: FxInstanceId,
        index: usize,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error(
        "Fx instance {instance:?} activates at {activation_seconds} seconds after runtime time {runtime_seconds} seconds"
    )]
    ActivationAfterRuntime {
        definition: Box<FxId>,
        instance: FxInstanceId,
        activation_seconds: String,
        runtime_seconds: String,
    },
    #[error(
        "retained Fx instance {instance:?} changed definition from `{existing}` to `{requested}`"
    )]
    RetainedDefinitionChanged {
        instance: FxInstanceId,
        existing: Box<FxId>,
        requested: Box<FxId>,
    },
    #[error("retained Fx instance {instance:?} changed its nested graph child path")]
    RetainedChildPathChanged {
        definition: Box<FxId>,
        instance: FxInstanceId,
    },
    #[error("invalid state for Fx instance {instance:?} of `{definition}`: {source}")]
    InvalidSnapshot {
        definition: Box<FxId>,
        instance: FxInstanceId,
        #[source]
        source: Box<FxInstanceSnapshotError>,
    },
}

impl Default for BundleFxRuntimeSnapshot {
    fn default() -> Self {
        Self {
            logical_time: FxLogicalTime::zero(),
            instances: Vec::new(),
        }
    }
}

impl BundleFxRuntimeSnapshot {
    /// Canonicalizes and intrinsically validates a programmatic snapshot.
    pub fn try_new(
        logical_time: FxLogicalTime,
        mut instances: Vec<FxInstanceSnapshot>,
    ) -> Result<Self, BundleFxRuntimeError> {
        if instances.len() > MAX_LIVE_FX_INSTANCES {
            return Err(BundleFxRuntimeError::TooManyInstances {
                actual: instances.len(),
                limit: MAX_LIVE_FX_INSTANCES,
            });
        }
        for instance in &instances {
            instance.clone().validate().map_err(|source| {
                BundleFxRuntimeError::InvalidSnapshot {
                    definition: Box::new(instance.definition.clone()),
                    instance: instance.instance,
                    source: Box::new(source),
                }
            })?;
        }
        instances.sort_by_key(|instance| instance.instance);
        for pair in instances.windows(2) {
            if pair[0].instance == pair[1].instance {
                return Err(BundleFxRuntimeError::DuplicateInstance {
                    instance: pair[0].instance,
                });
            }
        }
        Ok(Self {
            logical_time,
            instances,
        })
    }

    pub fn advance_millis(&mut self, milliseconds: u64) -> Result<(), BundleFxRuntimeError> {
        self.logical_time = self.logical_time.try_advance_millis(milliseconds)?;
        Ok(())
    }

    pub fn instance(&self, instance: FxInstanceId) -> Option<&FxInstanceSnapshot> {
        self.instances
            .binary_search_by_key(&instance, |snapshot| snapshot.instance)
            .ok()
            .map(|index| &self.instances[index])
    }

    /// Activates a new application or refreshes only its reactive parameter slots.
    ///
    /// A retained identity keeps its activation clock and deterministic seed.
    pub fn retain_instance(
        &mut self,
        definitions: &FxDefinitions,
        definition_id: &FxId,
        instance: FxInstanceId,
        parameters: Vec<FxRuntimeValue>,
        child_path: FxGraphChildPath,
        authored_seed: Option<&[u8]>,
    ) -> Result<(), BundleFxRuntimeError> {
        let definition = definitions.get(definition_id).ok_or_else(|| {
            BundleFxRuntimeError::MissingDefinition {
                definition: Box::new(definition_id.clone()),
                instance,
            }
        })?;
        match self
            .instances
            .binary_search_by_key(&instance, |snapshot| snapshot.instance)
        {
            Ok(index) => {
                let retained = &self.instances[index];
                if &retained.definition != definition_id {
                    return Err(BundleFxRuntimeError::RetainedDefinitionChanged {
                        instance,
                        existing: Box::new(retained.definition.clone()),
                        requested: Box::new(definition_id.clone()),
                    });
                }
                if retained.child_path != child_path {
                    return Err(BundleFxRuntimeError::RetainedChildPathChanged {
                        definition: Box::new(definition_id.clone()),
                        instance,
                    });
                }
                let mut refreshed = retained.clone();
                refreshed.parameters = parameters;
                validate_instance(&refreshed, definition, self.logical_time)?;
                self.instances[index] = refreshed;
            }
            Err(index) => {
                if self.instances.len() == MAX_LIVE_FX_INSTANCES {
                    return Err(BundleFxRuntimeError::TooManyInstances {
                        actual: self.instances.len().saturating_add(1),
                        limit: MAX_LIVE_FX_INSTANCES,
                    });
                }
                let snapshot = FxInstanceSnapshot {
                    instance,
                    definition: definition_id.clone(),
                    abi_hash: definition.abi_hash(),
                    activation_logical_time: self.logical_time,
                    deterministic_seed: derive_deterministic_seed(
                        instance,
                        definition.semantic_hash(),
                        authored_seed,
                        &child_path,
                    ),
                    parameters,
                    child_path,
                    provider_state: Vec::new(),
                };
                validate_instance(&snapshot, definition, self.logical_time)?;
                self.instances.insert(index, snapshot);
            }
        }
        Ok(())
    }

    pub fn remove_instance(&mut self, instance: FxInstanceId) -> Option<FxInstanceSnapshot> {
        self.instances
            .binary_search_by_key(&instance, |snapshot| snapshot.instance)
            .ok()
            .map(|index| self.instances.remove(index))
    }

    /// Validates every definition/ABI/parameter binding before atomic restore.
    pub fn validate_for_definitions(
        &self,
        definitions: &FxDefinitions,
    ) -> Result<(), BundleFxRuntimeError> {
        Self::try_new(self.logical_time, self.instances.clone())?;
        for instance in &self.instances {
            let definition = definitions.get(&instance.definition).ok_or_else(|| {
                BundleFxRuntimeError::MissingDefinition {
                    definition: Box::new(instance.definition.clone()),
                    instance: instance.instance,
                }
            })?;
            validate_instance(instance, definition, self.logical_time)?;
        }
        Ok(())
    }
}

impl BundleFxRuntimeError {
    /// Converts all runtime/save failures to the Web/native/Agent diagnostic contract.
    pub fn diagnostic(&self) -> FxDiagnostic {
        let (code, definition, instance, child_path) = match self {
            Self::MissingDefinition {
                definition,
                instance,
            } => (
                FxDiagnosticCode::MissingDefinition,
                Some(definition.as_ref().clone()),
                Some(*instance),
                FxGraphChildPath::default(),
            ),
            Self::AbiMismatch {
                definition,
                instance,
                ..
            } => (
                FxDiagnosticCode::AbiMismatch,
                Some(definition.as_ref().clone()),
                Some(*instance),
                FxGraphChildPath::default(),
            ),
            Self::ParameterType {
                definition,
                instance,
                ..
            }
            | Self::ParameterCount {
                definition,
                instance,
                ..
            } => (
                FxDiagnosticCode::UnitMismatch,
                Some(definition.as_ref().clone()),
                Some(*instance),
                FxGraphChildPath::default(),
            ),
            Self::RetainedChildPathChanged {
                definition,
                instance,
            }
            | Self::InvalidSnapshot {
                definition,
                instance,
                ..
            }
            | Self::ActivationAfterRuntime {
                definition,
                instance,
                ..
            } => (
                FxDiagnosticCode::ProgramValidation,
                Some(definition.as_ref().clone()),
                Some(*instance),
                FxGraphChildPath::default(),
            ),
            Self::RetainedDefinitionChanged {
                instance,
                requested,
                ..
            } => (
                FxDiagnosticCode::ProgramValidation,
                Some(requested.as_ref().clone()),
                Some(*instance),
                FxGraphChildPath::default(),
            ),
            Self::LogicalTime(_) => (
                FxDiagnosticCode::NumericNonFinite,
                None,
                None,
                FxGraphChildPath::default(),
            ),
            Self::TooManyInstances { .. } | Self::DuplicateInstance { .. } => (
                FxDiagnosticCode::ProgramValidation,
                None,
                None,
                FxGraphChildPath::default(),
            ),
        };
        FxDiagnostic::error(
            code,
            FxDiagnosticContext {
                definition,
                instance,
                child_path,
                ..FxDiagnosticContext::default()
            },
            self.to_string(),
        )
    }
}

#[derive(Deserialize)]
struct BundleFxRuntimeSnapshotWire {
    logical_time: FxLogicalTime,
    instances: Vec<FxInstanceSnapshot>,
}

impl<'de> Deserialize<'de> for BundleFxRuntimeSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = BundleFxRuntimeSnapshotWire::deserialize(deserializer)?;
        Self::try_new(wire.logical_time, wire.instances).map_err(D::Error::custom)
    }
}

fn validate_instance(
    instance: &FxInstanceSnapshot,
    definition: &FxDefinition,
    runtime_time: FxLogicalTime,
) -> Result<(), BundleFxRuntimeError> {
    instance
        .clone()
        .validate()
        .map_err(|source| BundleFxRuntimeError::InvalidSnapshot {
            definition: Box::new(instance.definition.clone()),
            instance: instance.instance,
            source: Box::new(source),
        })?;
    instance
        .validate_for_definition(definition)
        .map_err(|source| match source {
            FxInstanceSnapshotError::AbiMismatch { .. } => BundleFxRuntimeError::AbiMismatch {
                definition: Box::new(instance.definition.clone()),
                instance: instance.instance,
                saved: instance.abi_hash,
                actual: definition.abi_hash(),
            },
            FxInstanceSnapshotError::ParameterCount {
                expected, actual, ..
            } => BundleFxRuntimeError::ParameterCount {
                definition: Box::new(instance.definition.clone()),
                instance: instance.instance,
                expected,
                actual,
            },
            FxInstanceSnapshotError::ParameterType {
                index,
                expected,
                actual,
                ..
            } => BundleFxRuntimeError::ParameterType {
                definition: Box::new(instance.definition.clone()),
                instance: instance.instance,
                index,
                expected,
                actual,
            },
            other => BundleFxRuntimeError::InvalidSnapshot {
                definition: Box::new(instance.definition.clone()),
                instance: instance.instance,
                source: Box::new(other),
            },
        })?;
    if instance.activation_logical_time.seconds().seconds() > runtime_time.seconds().seconds() {
        return Err(BundleFxRuntimeError::ActivationAfterRuntime {
            definition: Box::new(instance.definition.clone()),
            instance: instance.instance,
            activation_seconds: instance
                .activation_logical_time
                .seconds()
                .seconds()
                .to_string(),
            runtime_seconds: runtime_time.seconds().seconds().to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use arcweft_bundle::fx_definitions::FxDefinitions;
    use arcweft_presentation::fx::{
        FiniteF32, FxDefinition, FxGraph, FxParameter, FxRuntimeType, FxRuntimeValue,
    };

    use super::*;

    fn definition() -> FxDefinition {
        FxDefinition::new(
            FxId::try_new("test", "wave").expect("identity"),
            vec![FxParameter::try_new("speed", FxRuntimeType::F32, None).expect("parameter")],
            FxGraph::default(),
        )
        .expect("definition")
    }

    #[test]
    fn retained_instance_keeps_activation_seed_and_updates_parameter_snapshot() {
        let definition = definition();
        let definitions =
            FxDefinitions::try_new([definition.clone()]).expect("definition inventory");
        let id = FxInstanceId::derive(definition.id(), ["view.hud", "node.1"]);
        let mut runtime = BundleFxRuntimeSnapshot::default();
        runtime.advance_millis(250).expect("clock");
        runtime
            .retain_instance(
                &definitions,
                definition.id(),
                id,
                vec![FxRuntimeValue::F32(
                    FiniteF32::try_new(1.0).expect("finite"),
                )],
                FxGraphChildPath::default(),
                None,
            )
            .expect("activation");
        let activated = runtime.instance(id).expect("instance").clone();
        runtime.advance_millis(750).expect("clock");
        runtime
            .retain_instance(
                &definitions,
                definition.id(),
                id,
                vec![FxRuntimeValue::F32(
                    FiniteF32::try_new(2.0).expect("finite"),
                )],
                FxGraphChildPath::default(),
                None,
            )
            .expect("reactive update");
        let refreshed = runtime.instance(id).expect("instance");

        assert_eq!(
            refreshed.activation_logical_time,
            activated.activation_logical_time
        );
        assert_eq!(refreshed.deterministic_seed, activated.deterministic_seed);
        assert_ne!(refreshed.parameters, activated.parameters);
        assert_eq!(runtime.logical_time.seconds().value(), FiniteF32::ONE);
        runtime
            .validate_for_definitions(&definitions)
            .expect("restorable state");
        assert_eq!(
            serde_json::from_slice::<BundleFxRuntimeSnapshot>(
                &serde_json::to_vec(&runtime).expect("encode")
            )
            .expect("decode"),
            runtime
        );
    }

    #[test]
    fn restore_failures_have_typed_missing_definition_and_abi_diagnostics() {
        let definition = definition();
        let definitions =
            FxDefinitions::try_new([definition.clone()]).expect("definition inventory");
        let id = FxInstanceId::derive(definition.id(), ["glyph.0"]);
        let mut runtime = BundleFxRuntimeSnapshot::default();
        runtime
            .retain_instance(
                &definitions,
                definition.id(),
                id,
                vec![FxRuntimeValue::F32(FiniteF32::ONE)],
                FxGraphChildPath::default(),
                None,
            )
            .expect("activation");

        let missing = runtime
            .validate_for_definitions(&FxDefinitions::default())
            .expect_err("missing definition");
        assert_eq!(
            missing.diagnostic().code,
            FxDiagnosticCode::MissingDefinition
        );

        runtime.instances[0].abi_hash = FxAbiHash::derive(["wrong"]);
        let mismatch = runtime
            .validate_for_definitions(&definitions)
            .expect_err("ABI mismatch");
        assert_eq!(mismatch.diagnostic().code, FxDiagnosticCode::AbiMismatch);
    }
}
