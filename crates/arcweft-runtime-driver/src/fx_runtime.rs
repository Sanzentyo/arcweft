//! Deterministic live Fx instances owned by the portable runtime driver.

use std::sync::Arc;

use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_presentation::fx::{
    FiniteF32Error, FxAbiHash, FxApplication, FxApplicationDraft, FxAuthoredSeed,
    FxBoundApplicationTemplate, FxDefinition, FxDefinitionArgumentValue, FxDefinitionParameterType,
    FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext, FxGraphChildPath, FxId,
    FxInstanceActivation, FxInstanceId, FxInstanceIdentity, FxInstanceSnapshot,
    FxInstanceSnapshotError, FxLogicalTime, FxRuntimeType, FxRuntimeValue,
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
    #[error("retained Fx instance {instance:?} changed its bound application template")]
    RetainedTemplateChanged {
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
    #[error("Fx application for `{definition}` failed exact binding: {reason}")]
    ApplicationBinding {
        definition: Box<FxId>,
        reason: String,
    },
}

pub(crate) fn bind_runtime_template(
    definition: &FxDefinition,
    runtime: &[FxRuntimeValue],
) -> Result<Arc<FxBoundApplicationTemplate>, BundleFxRuntimeError> {
    let mut values = runtime.iter().copied();
    let arguments = definition
        .parameters()
        .iter()
        .map(|parameter| match parameter.parameter_type() {
            FxDefinitionParameterType::Runtime(_) => values
                .next()
                .map(FxDefinitionArgumentValue::Runtime)
                .map(Some)
                .ok_or_else(|| BundleFxRuntimeError::ApplicationBinding {
                    definition: Box::new(definition.id().clone()),
                    reason: "missing dense runtime argument".to_owned(),
                }),
            FxDefinitionParameterType::Resource | FxDefinitionParameterType::UniformRecord => {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.next().is_some() {
        return Err(BundleFxRuntimeError::ApplicationBinding {
            definition: Box::new(definition.id().clone()),
            reason: "extra dense runtime argument".to_owned(),
        });
    }
    let draft = FxApplicationDraft::try_new(definition.id().clone(), arguments, 0, None).map_err(
        |error| BundleFxRuntimeError::ApplicationBinding {
            definition: Box::new(definition.id().clone()),
            reason: error.to_string(),
        },
    )?;
    FxApplication::bind(definition, draft)
        .map(|application| application.template().clone())
        .map_err(|error| BundleFxRuntimeError::ApplicationBinding {
            definition: Box::new(definition.id().clone()),
            reason: error.to_string(),
        })
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
                    definition: Box::new(instance.definition().clone()),
                    instance: instance.instance(),
                    source: Box::new(source),
                }
            })?;
        }
        instances.sort_by_key(FxInstanceSnapshot::instance);
        for pair in instances.windows(2) {
            if pair[0].instance() == pair[1].instance() {
                return Err(BundleFxRuntimeError::DuplicateInstance {
                    instance: pair[0].instance(),
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
            .binary_search_by_key(&instance, FxInstanceSnapshot::instance)
            .ok()
            .map(|index| &self.instances[index])
    }

    /// Activates a new application or refreshes only its reactive parameter slots.
    ///
    /// A retained identity keeps its activation clock and deterministic seed.
    pub fn retain_instance(
        &mut self,
        definitions: &FxDefinitions,
        identity: FxInstanceIdentity,
        template: Arc<FxBoundApplicationTemplate>,
        parameters: Vec<FxRuntimeValue>,
        child_path: FxGraphChildPath,
        authored_seed: Option<FxAuthoredSeed>,
    ) -> Result<(), BundleFxRuntimeError> {
        let instance = identity.instance();
        let definition_id = identity.definition().clone();
        let definition = definitions.get(&definition_id).ok_or_else(|| {
            BundleFxRuntimeError::MissingDefinition {
                definition: Box::new(definition_id.clone()),
                instance,
            }
        })?;
        match self
            .instances
            .binary_search_by_key(&instance, FxInstanceSnapshot::instance)
        {
            Ok(index) => {
                let retained = &self.instances[index];
                if retained.definition() != &definition_id {
                    return Err(BundleFxRuntimeError::RetainedDefinitionChanged {
                        instance,
                        existing: Box::new(retained.definition().clone()),
                        requested: Box::new(definition_id.clone()),
                    });
                }
                if retained.identity() != &identity {
                    return Err(BundleFxRuntimeError::InvalidSnapshot {
                        definition: Box::new(definition_id.clone()),
                        instance,
                        source: Box::new(FxInstanceSnapshotError::IdentityMismatch),
                    });
                }
                if retained.child_path() != &child_path {
                    return Err(BundleFxRuntimeError::RetainedChildPathChanged {
                        definition: Box::new(definition_id.clone()),
                        instance,
                    });
                }
                if retained.template().layout_digest() != template.layout_digest()
                    || retained.template().static_arguments() != template.static_arguments()
                {
                    return Err(BundleFxRuntimeError::RetainedTemplateChanged {
                        definition: Box::new(definition_id.clone()),
                        instance,
                    });
                }
                let refreshed = retained
                    .clone()
                    .try_with_parameters(parameters.into_boxed_slice(), definition)
                    .map_err(|source| BundleFxRuntimeError::InvalidSnapshot {
                        definition: Box::new(definition_id.clone()),
                        instance,
                        source: Box::new(source),
                    })?;
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
                let snapshot = FxInstanceSnapshot::try_new(
                    identity,
                    definition,
                    FxInstanceActivation::new(self.logical_time, authored_seed, child_path),
                    template,
                    parameters.into_boxed_slice(),
                    Vec::new(),
                )
                .map_err(|source| BundleFxRuntimeError::InvalidSnapshot {
                    definition: Box::new(definition_id.clone()),
                    instance,
                    source: Box::new(source),
                })?;
                validate_instance(&snapshot, definition, self.logical_time)?;
                self.instances.insert(index, snapshot);
            }
        }
        Ok(())
    }

    pub fn remove_instance(&mut self, instance: FxInstanceId) -> Option<FxInstanceSnapshot> {
        self.instances
            .binary_search_by_key(&instance, FxInstanceSnapshot::instance)
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
            let definition = definitions.get(instance.definition()).ok_or_else(|| {
                BundleFxRuntimeError::MissingDefinition {
                    definition: Box::new(instance.definition().clone()),
                    instance: instance.instance(),
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
            | Self::RetainedTemplateChanged {
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
            Self::ApplicationBinding { definition, .. } => (
                FxDiagnosticCode::ProgramValidation,
                Some(definition.as_ref().clone()),
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
            definition: Box::new(instance.definition().clone()),
            instance: instance.instance(),
            source: Box::new(source),
        })?;
    instance
        .validate_for_definition(definition)
        .map_err(|source| match source {
            FxInstanceSnapshotError::AbiMismatch { .. } => BundleFxRuntimeError::AbiMismatch {
                definition: Box::new(instance.definition().clone()),
                instance: instance.instance(),
                saved: instance.abi_hash(),
                actual: definition.abi_hash(),
            },
            FxInstanceSnapshotError::ParameterCount {
                expected, actual, ..
            } => BundleFxRuntimeError::ParameterCount {
                definition: Box::new(instance.definition().clone()),
                instance: instance.instance(),
                expected,
                actual,
            },
            FxInstanceSnapshotError::ParameterType {
                index,
                expected,
                actual,
                ..
            } => BundleFxRuntimeError::ParameterType {
                definition: Box::new(instance.definition().clone()),
                instance: instance.instance(),
                index,
                expected,
                actual,
            },
            other => BundleFxRuntimeError::InvalidSnapshot {
                definition: Box::new(instance.definition().clone()),
                instance: instance.instance(),
                source: Box::new(other),
            },
        })?;
    if instance.activation_logical_time().seconds().seconds() > runtime_time.seconds().seconds() {
        return Err(BundleFxRuntimeError::ActivationAfterRuntime {
            definition: Box::new(instance.definition().clone()),
            instance: instance.instance(),
            activation_seconds: instance
                .activation_logical_time()
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
        FiniteF32, FxDefinition, FxDefinitionParameter, FxDefinitionParameterType, FxGraph,
        FxInstanceOwnerKey, FxRuntimeType, FxRuntimeValue,
    };

    use super::*;

    fn owner(bytes: &[u8]) -> FxInstanceOwnerKey {
        FxInstanceOwnerKey::from_dialogue_canonical_bytes(bytes)
    }

    fn definition() -> FxDefinition {
        FxDefinition::new(
            FxId::try_new("test", "wave").expect("identity"),
            vec![
                FxDefinitionParameter::try_new(
                    0,
                    "speed",
                    FxDefinitionParameterType::Runtime(FxRuntimeType::F32),
                    None,
                )
                .expect("parameter"),
            ],
            FxGraph::default(),
        )
        .expect("definition")
    }

    fn u32_definition() -> FxDefinition {
        FxDefinition::new(
            FxId::try_new("test", "u32").expect("identity"),
            vec![
                FxDefinitionParameter::try_new(
                    0,
                    "seed",
                    FxDefinitionParameterType::Runtime(FxRuntimeType::U32),
                    None,
                )
                .expect("parameter"),
            ],
            FxGraph::default(),
        )
        .expect("definition")
    }

    #[test]
    fn retained_instance_keeps_activation_seed_and_updates_parameter_snapshot() {
        let definition = definition();
        let definitions =
            FxDefinitions::try_new([definition.clone()]).expect("definition inventory");
        let identity = FxInstanceIdentity::new(definition.id(), owner(b"view-node"), 0);
        let id = identity.instance();
        let mut runtime = BundleFxRuntimeSnapshot::default();
        runtime.advance_millis(250).expect("clock");
        let initial = vec![FxRuntimeValue::F32(
            FiniteF32::try_new(1.0).expect("finite"),
        )];
        let initial_template = bind_runtime_template(&definition, &initial).unwrap();
        runtime
            .retain_instance(
                &definitions,
                identity.clone(),
                initial_template,
                initial,
                FxGraphChildPath::default(),
                None,
            )
            .expect("activation");
        let activated = runtime.instance(id).expect("instance").clone();
        runtime.advance_millis(750).expect("clock");
        let refreshed_parameters = vec![FxRuntimeValue::F32(
            FiniteF32::try_new(2.0).expect("finite"),
        )];
        let refreshed_template = bind_runtime_template(&definition, &refreshed_parameters).unwrap();
        runtime
            .retain_instance(
                &definitions,
                identity,
                refreshed_template,
                refreshed_parameters,
                FxGraphChildPath::default(),
                None,
            )
            .expect("reactive update");
        let refreshed = runtime.instance(id).expect("instance");

        assert_eq!(
            refreshed.activation_logical_time(),
            activated.activation_logical_time()
        );
        assert_eq!(
            refreshed.deterministic_seed(),
            activated.deterministic_seed()
        );
        assert_ne!(refreshed.parameters(), activated.parameters());
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
        let identity = FxInstanceIdentity::new(definition.id(), owner(b"glyph"), 0);
        let mut runtime = BundleFxRuntimeSnapshot::default();
        let parameters = vec![FxRuntimeValue::F32(FiniteF32::ONE)];
        let template = bind_runtime_template(&definition, &parameters).unwrap();
        runtime
            .retain_instance(
                &definitions,
                identity,
                template,
                parameters,
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

        let mut tampered = serde_json::to_value(&runtime).expect("snapshot JSON");
        tampered["instances"][0]["abi_hash"] =
            serde_json::to_value(FxAbiHash::from_bytes([0xa5; 32])).expect("ABI hash JSON");
        let tampered = serde_json::from_value::<BundleFxRuntimeSnapshot>(tampered)
            .expect("intrinsically valid tampered snapshot");
        let mismatch = tampered
            .validate_for_definitions(&definitions)
            .expect_err("ABI mismatch");
        assert_eq!(mismatch.diagnostic().code, FxDiagnosticCode::AbiMismatch);
    }

    #[test]
    fn runtime_snapshot_codec_round_trips_u32_parameter_boundaries() {
        let definition = u32_definition();
        let definitions = FxDefinitions::try_new([definition.clone()]).expect("definitions");
        let mut runtime = BundleFxRuntimeSnapshot::default();
        for (ordinal, value) in [0_u32, u32::MAX].into_iter().enumerate() {
            let identity = FxInstanceIdentity::new(
                definition.id(),
                owner(&[u8::try_from(ordinal).expect("owner byte")]),
                u32::try_from(ordinal).expect("ordinal"),
            );
            let parameters = vec![FxRuntimeValue::U32(value)];
            let template = bind_runtime_template(&definition, &parameters).unwrap();
            runtime
                .retain_instance(
                    &definitions,
                    identity,
                    template,
                    parameters,
                    FxGraphChildPath::default(),
                    None,
                )
                .expect("U32 parameter");
        }
        let encoded = serde_json::to_vec(&runtime).expect("runtime snapshot encodes");
        assert_eq!(
            serde_json::from_slice::<BundleFxRuntimeSnapshot>(&encoded)
                .expect("runtime snapshot decodes"),
            runtime
        );
    }
}
