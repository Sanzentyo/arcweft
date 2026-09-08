//! Typed Fx applications and renderer-neutral runtime resolution boundaries.

use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

use super::{
    FxDefinition, FxDefinitionArgumentValue, FxDefinitionParameterLayoutDigest, FxDiagnostic, FxId,
    FxInstanceIdentity, FxInstanceOwnerKey, FxInstanceSnapshot, FxLogicalTime,
    FxParameterStorageSlot, FxResourceId, FxRuntimeValue, FxSourceRange, FxUniformRecord,
    graph::FX_MAX_PARAMETERS_PER_DEFINITION,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FxStaticDefinitionArgumentValue {
    Resource(FxResourceId),
    UniformRecord(FxUniformRecord),
}

impl FxStaticDefinitionArgumentValue {
    pub const fn parameter_type(&self) -> super::FxDefinitionParameterType {
        match self {
            Self::Resource(_) => super::FxDefinitionParameterType::Resource,
            Self::UniformRecord(_) => super::FxDefinitionParameterType::UniformRecord,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum FxStaticDefinitionArgumentValueWire {
    Resource(FxResourceId),
    UniformRecord(FxUniformRecord),
}

impl<'de> Deserialize<'de> for FxStaticDefinitionArgumentValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(
            match FxStaticDefinitionArgumentValueWire::deserialize(deserializer)? {
                FxStaticDefinitionArgumentValueWire::Resource(value) => Self::Resource(value),
                FxStaticDefinitionArgumentValueWire::UniformRecord(value) => {
                    Self::UniformRecord(value)
                }
            },
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxBoundApplicationTemplate {
    layout: FxDefinitionParameterLayoutDigest,
    initial_runtime: Box<[FxRuntimeValue]>,
    static_: Box<[FxStaticDefinitionArgumentValue]>,
}

impl FxBoundApplicationTemplate {
    pub(crate) fn try_new(
        layout: FxDefinitionParameterLayoutDigest,
        initial_runtime: Vec<FxRuntimeValue>,
        static_: Vec<FxStaticDefinitionArgumentValue>,
    ) -> Result<Self, FxApplicationError> {
        let total = initial_runtime
            .len()
            .checked_add(static_.len())
            .ok_or(FxApplicationError::ArgumentCountOverflow)?;
        if total > FX_MAX_PARAMETERS_PER_DEFINITION {
            return Err(FxApplicationError::TooManyArguments {
                actual: total,
                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
            });
        }
        Ok(Self {
            layout,
            initial_runtime: initial_runtime.into_boxed_slice(),
            static_: static_.into_boxed_slice(),
        })
    }

    pub const fn layout_digest(&self) -> FxDefinitionParameterLayoutDigest {
        self.layout
    }

    pub fn initial_runtime(&self) -> &[FxRuntimeValue] {
        &self.initial_runtime
    }

    pub fn static_arguments(&self) -> &[FxStaticDefinitionArgumentValue] {
        &self.static_
    }
}

#[derive(Deserialize)]
struct FxBoundApplicationTemplateWire {
    layout: FxDefinitionParameterLayoutDigest,
    initial_runtime: Vec<FxRuntimeValue>,
    static_: Vec<FxStaticDefinitionArgumentValue>,
}

impl<'de> Deserialize<'de> for FxBoundApplicationTemplate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = FxBoundApplicationTemplateWire::deserialize(deserializer)?;
        Self::try_new(wire.layout, wire.initial_runtime, wire.static_).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FxApplicationDraft {
    definition: FxId,
    arguments: Box<[Option<FxDefinitionArgumentValue>]>,
    authored_ordinal: u32,
    source_range: Option<FxSourceRange>,
}

impl FxApplicationDraft {
    pub fn try_new(
        definition: FxId,
        arguments: Vec<Option<FxDefinitionArgumentValue>>,
        authored_ordinal: u32,
        source_range: Option<FxSourceRange>,
    ) -> Result<Self, FxApplicationError> {
        if arguments.len() > FX_MAX_PARAMETERS_PER_DEFINITION {
            return Err(FxApplicationError::TooManyArguments {
                actual: arguments.len(),
                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
            });
        }
        Ok(Self {
            definition,
            arguments: arguments.into_boxed_slice(),
            authored_ordinal,
            source_range,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FxApplication {
    definition: FxId,
    template: Arc<FxBoundApplicationTemplate>,
    authored_ordinal: u32,
    source_range: Option<FxSourceRange>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxApplicationError {
    #[error("Fx application has {actual} arguments, exceeding the limit of {limit}")]
    TooManyArguments { actual: usize, limit: usize },
    #[error("Fx application argument count overflowed")]
    ArgumentCountOverflow,
    #[error("Fx application targets `{application}`, but the bound definition is `{definition}`")]
    DefinitionMismatch {
        application: Box<FxId>,
        definition: Box<FxId>,
    },
    #[error("Fx application for `{definition}` has {actual} ABI slots, expected {expected}")]
    ArgumentCount {
        definition: Box<FxId>,
        expected: usize,
        actual: usize,
    },
    #[error("Fx application argument {index} for `{definition}` is omitted without a default")]
    MissingArgument { definition: Box<FxId>, index: usize },
    #[error("Fx application argument {index} has type {actual:?}, expected {expected:?}")]
    ArgumentType {
        index: usize,
        expected: super::FxDefinitionParameterType,
        actual: super::FxDefinitionParameterType,
    },
    #[error("Fx application layout does not match definition `{definition}`")]
    LayoutMismatch { definition: Box<FxId> },
    #[error("Fx application dense storage is inconsistent with its definition layout")]
    StorageMismatch,
}

#[derive(Clone, Copy, Debug)]
pub struct FxEvaluationBinding<'a> {
    pub definition: &'a FxDefinition,
    pub instance: &'a FxInstanceSnapshot,
    pub runtime_time: FxLogicalTime,
}

pub trait FxApplicationResolver {
    fn resolve<'a>(
        &'a self,
        application: &FxApplication,
    ) -> Result<FxEvaluationBinding<'a>, Box<FxDiagnostic>>;
}

impl FxApplication {
    pub fn bind(
        definition: &FxDefinition,
        draft: FxApplicationDraft,
    ) -> Result<Self, FxApplicationError> {
        if &draft.definition != definition.id() {
            return Err(FxApplicationError::DefinitionMismatch {
                application: Box::new(draft.definition),
                definition: Box::new(definition.id().clone()),
            });
        }
        if draft.arguments.len() != definition.parameters().len() {
            return Err(FxApplicationError::ArgumentCount {
                definition: Box::new(definition.id().clone()),
                expected: definition.parameters().len(),
                actual: draft.arguments.len(),
            });
        }
        let mut runtime = Vec::with_capacity(definition.parameter_layout().runtime_rows().len());
        let mut static_ = Vec::with_capacity(definition.parameter_layout().static_rows().len());
        for ((parameter, supplied), layout_row) in definition
            .parameters()
            .iter()
            .zip(draft.arguments.into_vec())
            .zip(definition.parameter_layout().abi_rows())
        {
            let value = supplied
                .or_else(|| parameter.default().cloned())
                .ok_or_else(|| FxApplicationError::MissingArgument {
                    definition: Box::new(definition.id().clone()),
                    index: usize::from(parameter.index().get()),
                })?;
            if value.parameter_type() != parameter.parameter_type() {
                return Err(FxApplicationError::ArgumentType {
                    index: usize::from(parameter.index().get()),
                    expected: parameter.parameter_type(),
                    actual: value.parameter_type(),
                });
            }
            match (layout_row.storage(), value) {
                (
                    FxParameterStorageSlot::Runtime(slot),
                    FxDefinitionArgumentValue::Runtime(value),
                ) if usize::from(slot.get()) == runtime.len() => {
                    runtime.push(value);
                }
                (
                    FxParameterStorageSlot::Static(slot),
                    FxDefinitionArgumentValue::Resource(value),
                ) if usize::from(slot.get()) == static_.len() => {
                    static_.push(FxStaticDefinitionArgumentValue::Resource(value));
                }
                (
                    FxParameterStorageSlot::Static(slot),
                    FxDefinitionArgumentValue::UniformRecord(value),
                ) if usize::from(slot.get()) == static_.len() => {
                    value
                        .validate_definition_layout(definition.parameter_layout())
                        .map_err(|_| FxApplicationError::StorageMismatch)?;
                    static_.push(FxStaticDefinitionArgumentValue::UniformRecord(value));
                }
                _ => return Err(FxApplicationError::StorageMismatch),
            }
        }
        let template = Arc::new(FxBoundApplicationTemplate::try_new(
            definition.parameter_layout().digest(),
            runtime,
            static_,
        )?);
        Ok(Self {
            definition: draft.definition,
            template,
            authored_ordinal: draft.authored_ordinal,
            source_range: draft.source_range,
        })
    }

    pub const fn definition(&self) -> &FxId {
        &self.definition
    }

    pub fn template(&self) -> &Arc<FxBoundApplicationTemplate> {
        &self.template
    }

    pub const fn authored_ordinal(&self) -> u32 {
        self.authored_ordinal
    }

    pub const fn source_range(&self) -> Option<FxSourceRange> {
        self.source_range
    }

    pub fn instance_identity(&self, owner_key: FxInstanceOwnerKey) -> FxInstanceIdentity {
        FxInstanceIdentity::new(&self.definition, owner_key, self.authored_ordinal)
    }

    pub fn validate_for_definition(
        &self,
        definition: &FxDefinition,
    ) -> Result<(), FxApplicationError> {
        if &self.definition != definition.id() {
            return Err(FxApplicationError::DefinitionMismatch {
                application: Box::new(self.definition.clone()),
                definition: Box::new(definition.id().clone()),
            });
        }
        if self.template.layout != definition.parameter_layout().digest()
            || self.template.initial_runtime.len()
                != definition.parameter_layout().runtime_rows().len()
            || self.template.static_.len() != definition.parameter_layout().static_rows().len()
        {
            return Err(FxApplicationError::LayoutMismatch {
                definition: Box::new(definition.id().clone()),
            });
        }
        for (value, row) in self
            .template
            .initial_runtime
            .iter()
            .zip(definition.parameter_layout().runtime_rows())
        {
            if value.value_type() != row.reference().runtime_type() {
                return Err(FxApplicationError::StorageMismatch);
            }
        }
        for (value, row) in self
            .template
            .static_
            .iter()
            .zip(definition.parameter_layout().static_rows())
        {
            if value.parameter_type() != row.parameter().parameter_type() {
                return Err(FxApplicationError::StorageMismatch);
            }
            if let FxStaticDefinitionArgumentValue::UniformRecord(record) = value {
                record
                    .validate_definition_layout(definition.parameter_layout())
                    .map_err(|_| FxApplicationError::StorageMismatch)?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct FxApplicationSerializeWire<'a> {
    definition: &'a FxId,
    layout: FxDefinitionParameterLayoutDigest,
    initial_runtime: &'a [FxRuntimeValue],
    static_: &'a [FxStaticDefinitionArgumentValue],
    authored_ordinal: u32,
    source_range: Option<FxSourceRange>,
}

#[derive(Deserialize)]
struct FxApplicationWire {
    definition: FxId,
    layout: FxDefinitionParameterLayoutDigest,
    initial_runtime: Vec<FxRuntimeValue>,
    static_: Vec<FxStaticDefinitionArgumentValue>,
    authored_ordinal: u32,
    source_range: Option<FxSourceRange>,
}

impl Serialize for FxApplication {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FxApplicationSerializeWire {
            definition: &self.definition,
            layout: self.template.layout,
            initial_runtime: &self.template.initial_runtime,
            static_: &self.template.static_,
            authored_ordinal: self.authored_ordinal,
            source_range: self.source_range,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FxApplication {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = FxApplicationWire::deserialize(deserializer)?;
        let template = Arc::new(
            FxBoundApplicationTemplate::try_new(wire.layout, wire.initial_runtime, wire.static_)
                .map_err(D::Error::custom)?,
        );
        Ok(Self {
            definition: wire.definition,
            template,
            authored_ordinal: wire.authored_ordinal,
            source_range: wire.source_range,
        })
    }
}
