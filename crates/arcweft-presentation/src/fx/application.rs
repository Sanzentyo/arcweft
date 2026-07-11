//! Typed Fx applications and renderer-neutral runtime resolution boundaries.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use super::{
    FxDefinition, FxDiagnostic, FxId, FxInstanceId, FxInstanceSnapshot, FxLogicalTime,
    FxRuntimeType, FxRuntimeValue, FxSourceRange, graph::FX_MAX_PARAMETERS_PER_DEFINITION,
};

/// One authored application of a compiled Fx definition.
///
/// The application retains typed parameter values and source order. Runtime
/// occurrence identity, activation time, and deterministic seed live in the
/// corresponding [`FxInstanceSnapshot`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxApplication {
    definition: FxId,
    parameters: Vec<FxRuntimeValue>,
    authored_ordinal: u32,
    source_range: Option<FxSourceRange>,
}

/// Invalid authored application data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxApplicationError {
    #[error("Fx application has {actual} parameters, exceeding the limit of {limit}")]
    TooManyParameters { actual: usize, limit: usize },
    #[error("Fx application targets `{application}`, but the bound definition is `{definition}`")]
    DefinitionMismatch { application: FxId, definition: FxId },
    #[error("Fx application for `{definition}` has {actual} parameters, expected {expected}")]
    ParameterCount {
        definition: FxId,
        expected: usize,
        actual: usize,
    },
    #[error(
        "Fx application parameter {index} for `{definition}` has type {actual:?}, expected {expected:?}"
    )]
    ParameterType {
        definition: FxId,
        index: usize,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
}

/// Borrowed definition and live instance selected for one application.
#[derive(Clone, Copy, Debug)]
pub struct FxEvaluationBinding<'a> {
    pub definition: &'a FxDefinition,
    pub instance: &'a FxInstanceSnapshot,
    pub runtime_time: FxLogicalTime,
}

/// Owner-specific lookup used by shared View and `RichText` evaluators.
///
/// The owner derives occurrence identity (for example a View mount or dialogue
/// line occurrence) and returns only presentation-owned typed state.
pub trait FxApplicationResolver {
    fn resolve<'a>(
        &'a self,
        application: &FxApplication,
    ) -> Result<FxEvaluationBinding<'a>, Box<FxDiagnostic>>;
}

impl FxApplication {
    pub fn try_new(
        definition: FxId,
        parameters: Vec<FxRuntimeValue>,
        authored_ordinal: u32,
        source_range: Option<FxSourceRange>,
    ) -> Result<Self, FxApplicationError> {
        if parameters.len() > FX_MAX_PARAMETERS_PER_DEFINITION {
            return Err(FxApplicationError::TooManyParameters {
                actual: parameters.len(),
                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
            });
        }
        Ok(Self {
            definition,
            parameters,
            authored_ordinal,
            source_range,
        })
    }

    #[must_use]
    pub const fn definition(&self) -> &FxId {
        &self.definition
    }

    #[must_use]
    pub fn parameters(&self) -> &[FxRuntimeValue] {
        &self.parameters
    }

    #[must_use]
    pub const fn authored_ordinal(&self) -> u32 {
        self.authored_ordinal
    }

    #[must_use]
    pub const fn source_range(&self) -> Option<FxSourceRange> {
        self.source_range
    }

    /// Derives a stable occurrence identity and appends the authored Fx ordinal.
    #[must_use]
    pub fn derive_instance_id<'a>(
        &self,
        owner_components: impl IntoIterator<Item = &'a str>,
    ) -> FxInstanceId {
        let mut components = owner_components
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        components.push(format!("fx.{}", self.authored_ordinal));
        FxInstanceId::derive(&self.definition, components.iter().map(String::as_str))
    }

    pub fn validate_for_definition(
        &self,
        definition: &FxDefinition,
    ) -> Result<(), FxApplicationError> {
        if &self.definition != definition.id() {
            return Err(FxApplicationError::DefinitionMismatch {
                application: self.definition.clone(),
                definition: definition.id().clone(),
            });
        }
        if self.parameters.len() != definition.parameters().len() {
            return Err(FxApplicationError::ParameterCount {
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
                return Err(FxApplicationError::ParameterType {
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
struct FxApplicationWire {
    definition: FxId,
    parameters: Vec<FxRuntimeValue>,
    authored_ordinal: u32,
    source_range: Option<FxSourceRange>,
}

impl<'de> Deserialize<'de> for FxApplication {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxApplicationWire::deserialize(deserializer)?;
        Self::try_new(
            wire.definition,
            wire.parameters,
            wire.authored_ordinal,
            wire.source_range,
        )
        .map_err(D::Error::custom)
    }
}
