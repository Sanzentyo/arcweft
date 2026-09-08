//! Arcweft-owned typed Fx callables and graph templates.

mod programs;
mod schema;

use std::collections::BTreeSet;

use thiserror::Error;

use super::{
    Angle, FiniteF32, FiniteF32Error, FxApplication, FxApplicationDraft, FxApplicationError,
    FxDefinition, FxDefinitionArgumentValue, FxDefinitionError, FxDefinitionGraphContext,
    FxDefinitionParameter, FxDefinitionParameterIndex, FxDefinitionParameterLayoutDigest,
    FxDefinitionParameterRef, FxDefinitionParameterSchema, FxDefinitionParameterType, FxGraph,
    FxGraphError, FxId, FxIdError, FxRuntimeParameterRef, FxRuntimeType, FxRuntimeValue,
    FxSourceRange, FxStaticValue, FxTarget, FxVec2, Length, MotionFunction, Seconds,
    ValueProgramValidationError,
    canonical::{CanonicalEncoder, CanonicalHashSink},
    uniform::FxUniformError,
};

pub use schema::{
    BUILTIN_FX_CALLABLE_CATALOG, BUILTIN_FX_CALLABLE_SCHEMA_VERSION, BuiltinFxCallableCatalog,
    BuiltinFxCallableId, BuiltinFxCallableParameter, BuiltinFxCallableRow, BuiltinFxCallableRowId,
    BuiltinFxCallableSchemaDigest, BuiltinFxDefaultValue, BuiltinFxNumericConstraint,
    BuiltinFxParameterBinding, BuiltinFxParameterId, BuiltinFxParameterPassing,
    BuiltinFxParameterPredicate, BuiltinFxParameterPresence, BuiltinFxParameterType, BuiltinFxUnit,
    BuiltinFxValueConstraint,
};

/// Exact ABI-backed source parameters active in one structural specialization.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuiltinFxActiveAbiParameterSet(u32);

/// Structural values that select one immutable builtin graph template.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuiltinFxSpecialization {
    row: BuiltinFxCallableRowId,
    target: FxTarget,
    motion_function: Option<MotionFunction>,
    active_abi_parameters: BuiltinFxActiveAbiParameterSet,
}

/// One source parameter's direct row in the heterogeneous definition ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinFxAbiProjection {
    Direct(FxDefinitionParameterIndex),
}

/// One builtin source argument represented symbolically while composing a
/// graph fragment into an outer definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinFxGraphArgument {
    /// Reuses one parameter issued by the outer definition schema.
    Parameter(FxDefinitionParameterRef),
    /// Embeds one validated closed value into the graph fragment.
    Constant(FxDefinitionArgumentValue),
}

/// Binding-plan row for one ABI-backed source parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinFxApplicationParameter {
    id: BuiltinFxParameterId,
    parameter_type: BuiltinFxParameterType,
    projection: BuiltinFxAbiProjection,
}

/// Sealed application binding plan for one builtin definition template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinFxApplicationBindingPlan {
    definition: FxId,
    schema: BuiltinFxCallableSchemaDigest,
    layout: FxDefinitionParameterLayoutDigest,
    parameters: Box<[BuiltinFxApplicationParameter]>,
}

/// Typed source argument supplied to a builtin Fx application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinFxArgument {
    parameter: BuiltinFxParameterId,
    value: FxDefinitionArgumentValue,
}

/// Parameterized graph template and its exact application binding plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinFxDefinitionTemplate {
    specialization: BuiltinFxSpecialization,
    definition: FxDefinition,
    binding_plan: BuiltinFxApplicationBindingPlan,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BuiltinFxBuildError {
    #[error("builtin Fx catalog is missing closed row {row:?}")]
    CatalogInvariant { row: BuiltinFxCallableRowId },
    #[error("builtin Fx row {row:?} has an invalid schema for parameter {parameter:?}")]
    ParameterSchemaInvariant {
        row: BuiltinFxCallableRowId,
        parameter: BuiltinFxParameterId,
    },
    #[error("target {target:?} is not admitted by builtin Fx row {row:?}")]
    InvalidTarget {
        row: BuiltinFxCallableRowId,
        target: FxTarget,
    },
    #[error("builtin Fx row {row:?} does not accept motion_function")]
    UnexpectedMotionFunction { row: BuiltinFxCallableRowId },
    #[error("parameter {parameter:?} is not an ABI parameter of row {row:?}")]
    InvalidActiveAbiParameter {
        row: BuiltinFxCallableRowId,
        parameter: BuiltinFxParameterId,
    },
    #[error("required/defaulted ABI parameter {parameter:?} is inactive for row {row:?}")]
    MissingActiveAbiParameter {
        row: BuiltinFxCallableRowId,
        parameter: BuiltinFxParameterId,
    },
    #[error("builtin Fx argument {parameter:?} is not present in the selected ABI")]
    UnknownArgument { parameter: BuiltinFxParameterId },
    #[error("builtin Fx argument {parameter:?} was supplied more than once")]
    DuplicateArgument { parameter: BuiltinFxParameterId },
    #[error("builtin Fx graph argument count is {actual}, but specialization requires {expected}")]
    GraphArgumentCount { expected: usize, actual: usize },
    #[error("builtin Fx argument {parameter:?} has type {actual:?}, expected {expected:?}")]
    ArgumentType {
        parameter: BuiltinFxParameterId,
        expected: BuiltinFxParameterType,
        actual: FxDefinitionParameterType,
    },
    #[error("builtin Fx argument {parameter:?} violates its owning row constraint")]
    ArgumentConstraint { parameter: BuiltinFxParameterId },
    #[error("builtin Fx graph parameter {parameter:?} is absent")]
    MissingGraphParameter { parameter: BuiltinFxParameterId },
    #[error("builtin Fx graph parameter {parameter:?} has no runtime slot")]
    NonRuntimeGraphParameter { parameter: BuiltinFxParameterId },
    #[error("builtin Fx graph parameter {parameter:?} has no static graph value")]
    NonStaticGraphParameter { parameter: BuiltinFxParameterId },
    #[error("builtin Fx default {0:?} cannot populate an ABI parameter")]
    StructuralDefault(BuiltinFxDefaultValue),
    #[error("builtin Fx milli value {0} cannot be represented by the runtime")]
    DefaultOutOfRange(i64),
    #[error("builtin Fx parameter layout exceeds the runtime slot domain")]
    RuntimeParameterLimit,
    #[error(transparent)]
    Numeric(#[from] FiniteF32Error),
    #[error(transparent)]
    Identity(#[from] FxIdError),
    #[error(transparent)]
    Definition(#[from] FxDefinitionError),
    #[error(transparent)]
    Graph(#[from] FxGraphError),
    #[error(transparent)]
    Program(#[from] ValueProgramValidationError),
    #[error(transparent)]
    Uniform(#[from] FxUniformError),
    #[error(transparent)]
    Application(Box<FxApplicationError>),
}

impl From<FxApplicationError> for BuiltinFxBuildError {
    fn from(error: FxApplicationError) -> Self {
        Self::Application(Box::new(error))
    }
}

impl BuiltinFxActiveAbiParameterSet {
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub fn from_parameters(parameters: impl IntoIterator<Item = BuiltinFxParameterId>) -> Self {
        parameters
            .into_iter()
            .fold(Self::empty(), |mut set, parameter| {
                set.0 |= 1_u32 << u32::from(parameter.semantic_tag());
                set
            })
    }

    #[must_use]
    pub const fn contains(self, parameter: BuiltinFxParameterId) -> bool {
        self.0 & (1_u32 << parameter.semantic_tag()) != 0
    }

    fn iter(self) -> impl Iterator<Item = BuiltinFxParameterId> {
        BuiltinFxParameterId::ALL
            .into_iter()
            .filter(move |parameter| self.contains(*parameter))
    }

    const fn bits(self) -> u32 {
        self.0
    }
}

impl BuiltinFxSpecialization {
    pub fn try_new(
        row: BuiltinFxCallableRowId,
        target: FxTarget,
        motion_function: Option<MotionFunction>,
        active_abi_parameters: BuiltinFxActiveAbiParameterSet,
    ) -> Result<Self, BuiltinFxBuildError> {
        let schema = row_schema(row)?;
        let Some(target_schema) = schema.parameter(BuiltinFxParameterId::Target) else {
            return Err(BuiltinFxBuildError::ParameterSchemaInvariant {
                row,
                parameter: BuiltinFxParameterId::Target,
            });
        };
        let BuiltinFxValueConstraint::AllowedTargets(allowed) = target_schema.constraint() else {
            return Err(BuiltinFxBuildError::ParameterSchemaInvariant {
                row,
                parameter: BuiltinFxParameterId::Target,
            });
        };
        if !allowed.contains(&target) {
            return Err(BuiltinFxBuildError::InvalidTarget { row, target });
        }
        let has_motion_function = schema
            .parameter(BuiltinFxParameterId::MotionFunction)
            .is_some();
        let motion_function = match (has_motion_function, motion_function) {
            (true, value) => Some(value.unwrap_or(MotionFunction::BreathOrbit)),
            (false, None) => None,
            (false, Some(_)) => return Err(BuiltinFxBuildError::UnexpectedMotionFunction { row }),
        };
        for parameter in active_abi_parameters.iter() {
            let valid = schema
                .parameter(parameter)
                .is_some_and(|parameter| parameter.binding() == BuiltinFxParameterBinding::Abi);
            if !valid {
                return Err(BuiltinFxBuildError::InvalidActiveAbiParameter { row, parameter });
            }
        }
        for parameter in schema.parameters().iter().copied().filter(|parameter| {
            parameter.binding() == BuiltinFxParameterBinding::Abi
                && matches!(
                    parameter.presence(),
                    BuiltinFxParameterPresence::Required | BuiltinFxParameterPresence::Defaulted(_)
                )
        }) {
            if !active_abi_parameters.contains(parameter.id()) {
                return Err(BuiltinFxBuildError::MissingActiveAbiParameter {
                    row,
                    parameter: parameter.id(),
                });
            }
        }
        Ok(Self {
            row,
            target,
            motion_function,
            active_abi_parameters,
        })
    }

    #[must_use]
    pub const fn row(self) -> BuiltinFxCallableRowId {
        self.row
    }
    #[must_use]
    pub const fn target(self) -> FxTarget {
        self.target
    }
    #[must_use]
    pub const fn motion_function(self) -> Option<MotionFunction> {
        self.motion_function
    }
    #[must_use]
    pub const fn active_abi_parameters(self) -> BuiltinFxActiveAbiParameterSet {
        self.active_abi_parameters
    }

    /// Canonical v1 digest of the complete structural specialization authority.
    pub fn semantic_digest_v1(self) -> Result<[u8; 32], BuiltinFxBuildError> {
        let row = row_schema(self.row)?;
        let mut hasher = blake3::Hasher::new();
        let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
        infallible(encoder.domain_v1(b"arcweft.builtin-fx-specialization"));
        infallible(encoder.tag(row.callable().semantic_tag()));
        infallible(encoder.tag(self.row.semantic_tag()));
        infallible(encoder.unsigned(u64::from(self.target.tag())));
        infallible(encoder.boolean(self.motion_function.is_some()));
        if let Some(function) = self.motion_function {
            infallible(encoder.unsigned(u64::from(function.tag())));
        }
        infallible(encoder.unsigned(u64::from(self.active_abi_parameters.bits())));
        infallible(encoder.digest32(row.schema_digest().as_bytes()));
        Ok(*hasher.finalize().as_bytes())
    }

    pub fn definition_id(self) -> Result<FxId, BuiltinFxBuildError> {
        let row = row_schema(self.row)?;
        let digest = self.semantic_digest_v1()?;
        Ok(FxId::from_builtin_structural_digest(
            row.callable().source_name(),
            &digest,
        )?)
    }
}

fn infallible(result: Result<(), std::convert::Infallible>) {
    match result {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

#[derive(Clone, Debug)]
enum GraphParameterArgument {
    Parameter {
        definition: FxDefinitionParameterRef,
        runtime: Option<FxRuntimeParameterRef>,
    },
    Constant(FxDefinitionArgumentValue),
}

#[derive(Clone, Debug)]
struct GraphParameter {
    id: BuiltinFxParameterId,
    argument: GraphParameterArgument,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum GraphRuntimeArgument {
    Parameter(FxRuntimeParameterRef),
    Constant(FxRuntimeValue),
}

pub(super) struct GraphTemplateContext {
    specialization: BuiltinFxSpecialization,
    definition: FxId,
    runtime_types: Vec<FxRuntimeType>,
    parameters: Vec<GraphParameter>,
}

impl GraphTemplateContext {
    pub(super) const fn specialization(&self) -> BuiltinFxSpecialization {
        self.specialization
    }

    pub(super) const fn definition(&self) -> &FxId {
        &self.definition
    }

    pub(super) fn runtime_types(&self) -> &[FxRuntimeType] {
        &self.runtime_types
    }

    pub(super) fn has_parameter(&self, id: BuiltinFxParameterId) -> bool {
        self.parameters.iter().any(|parameter| parameter.id == id)
    }

    pub(super) fn runtime_argument(
        &self,
        id: BuiltinFxParameterId,
    ) -> Result<GraphRuntimeArgument, BuiltinFxBuildError> {
        let parameter = self
            .parameters
            .iter()
            .find(|parameter| parameter.id == id)
            .ok_or(BuiltinFxBuildError::MissingGraphParameter { parameter: id })?;
        match &parameter.argument {
            GraphParameterArgument::Parameter {
                runtime: Some(runtime),
                ..
            } => Ok(GraphRuntimeArgument::Parameter(*runtime)),
            GraphParameterArgument::Parameter { runtime: None, .. }
            | GraphParameterArgument::Constant(
                FxDefinitionArgumentValue::Resource(_)
                | FxDefinitionArgumentValue::UniformRecord(_),
            ) => Err(BuiltinFxBuildError::NonRuntimeGraphParameter { parameter: id }),
            GraphParameterArgument::Constant(FxDefinitionArgumentValue::Runtime(value)) => {
                Ok(GraphRuntimeArgument::Constant(*value))
            }
        }
    }

    pub(super) fn static_value(
        &self,
        id: BuiltinFxParameterId,
    ) -> Result<FxStaticValue, BuiltinFxBuildError> {
        let parameter = self
            .parameters
            .iter()
            .find(|parameter| parameter.id == id)
            .ok_or(BuiltinFxBuildError::MissingGraphParameter { parameter: id })?;
        match &parameter.argument {
            GraphParameterArgument::Parameter { definition, .. } => {
                Ok(FxStaticValue::Parameter(*definition))
            }
            GraphParameterArgument::Constant(FxDefinitionArgumentValue::Resource(value)) => {
                Ok(FxStaticValue::Resource(value.clone()))
            }
            GraphParameterArgument::Constant(FxDefinitionArgumentValue::UniformRecord(value)) => {
                Ok(FxStaticValue::UniformRecord(value.clone()))
            }
            GraphParameterArgument::Constant(FxDefinitionArgumentValue::Runtime(_)) => {
                Err(BuiltinFxBuildError::NonStaticGraphParameter { parameter: id })
            }
        }
    }
}

impl BuiltinFxGraphArgument {
    /// Constructs an outer-parameter graph argument.
    #[must_use]
    pub const fn parameter(reference: FxDefinitionParameterRef) -> Self {
        Self::Parameter(reference)
    }

    /// Constructs an inline closed-value graph argument.
    #[must_use]
    pub fn constant(value: FxDefinitionArgumentValue) -> Self {
        Self::Constant(value)
    }
}

impl BuiltinFxArgument {
    #[must_use]
    pub const fn new(parameter: BuiltinFxParameterId, value: FxDefinitionArgumentValue) -> Self {
        Self { parameter, value }
    }

    #[must_use]
    pub const fn parameter(&self) -> BuiltinFxParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn value(&self) -> &FxDefinitionArgumentValue {
        &self.value
    }
}

impl BuiltinFxApplicationParameter {
    #[must_use]
    pub const fn id(&self) -> BuiltinFxParameterId {
        self.id
    }
    #[must_use]
    pub const fn parameter_type(&self) -> BuiltinFxParameterType {
        self.parameter_type
    }
    #[must_use]
    pub const fn projection(&self) -> &BuiltinFxAbiProjection {
        &self.projection
    }
}

impl BuiltinFxApplicationBindingPlan {
    #[must_use]
    pub const fn definition(&self) -> &FxId {
        &self.definition
    }
    #[must_use]
    pub const fn schema_digest(&self) -> BuiltinFxCallableSchemaDigest {
        self.schema
    }
    #[must_use]
    pub const fn layout_digest(&self) -> FxDefinitionParameterLayoutDigest {
        self.layout
    }
    #[must_use]
    pub fn parameters(&self) -> &[BuiltinFxApplicationParameter] {
        &self.parameters
    }
}

impl BuiltinFxDefinitionTemplate {
    #[must_use]
    pub const fn specialization(&self) -> BuiltinFxSpecialization {
        self.specialization
    }
    #[must_use]
    pub const fn definition(&self) -> &FxDefinition {
        &self.definition
    }
    #[must_use]
    pub const fn binding_plan(&self) -> &BuiltinFxApplicationBindingPlan {
        &self.binding_plan
    }

    pub fn bind_application(
        &self,
        arguments: impl IntoIterator<Item = BuiltinFxArgument>,
        authored_ordinal: u32,
        source_range: Option<FxSourceRange>,
    ) -> Result<FxApplication, BuiltinFxBuildError> {
        let row = row_schema(self.specialization.row)?;
        let mut supplied = BTreeSet::new();
        let mut abi = vec![None; self.definition.parameters().len()];
        for argument in arguments {
            if !supplied.insert(argument.parameter) {
                return Err(BuiltinFxBuildError::DuplicateArgument {
                    parameter: argument.parameter,
                });
            }
            let binding = self
                .binding_plan
                .parameters
                .iter()
                .find(|binding| binding.id == argument.parameter)
                .ok_or(BuiltinFxBuildError::UnknownArgument {
                    parameter: argument.parameter,
                })?;
            let schema = row.parameter(argument.parameter).ok_or(
                BuiltinFxBuildError::ParameterSchemaInvariant {
                    row: row.id(),
                    parameter: argument.parameter,
                },
            )?;
            validate_source_value(schema, &argument.value)?;
            let BuiltinFxAbiProjection::Direct(index) = &binding.projection;
            abi[usize::from(index.get())] = Some(argument.value);
        }
        let draft = FxApplicationDraft::try_new(
            self.definition.id().clone(),
            abi,
            authored_ordinal,
            source_range,
        )?;
        Ok(FxApplication::bind(&self.definition, draft)?)
    }
}

/// Builds one parameterized builtin Fx graph template. Only structural
/// specialization contributes to the definition identity; every numeric and
/// resource value remains an application ABI binding.
pub fn build_builtin_fx_definition(
    specialization: BuiltinFxSpecialization,
) -> Result<BuiltinFxDefinitionTemplate, BuiltinFxBuildError> {
    let row = row_schema(specialization.row)?;
    let definition_id = specialization.definition_id()?;
    let mut definition_parameters = Vec::new();
    let mut application_parameters = Vec::new();
    let mut parameter_refs = Vec::new();

    for source in row.parameters() {
        if source.binding() != BuiltinFxParameterBinding::Abi
            || !specialization.active_abi_parameters.contains(source.id())
        {
            continue;
        }
        let definition_type = definition_parameter_type(source.parameter_type()).ok_or(
            BuiltinFxBuildError::ParameterSchemaInvariant {
                row: specialization.row,
                parameter: source.id(),
            },
        )?;
        let default = source_default(source)
            .map(materialize_default)
            .transpose()?;
        let parameter = FxDefinitionParameter::try_new(
            definition_parameters.len(),
            source.source_name(),
            definition_type,
            default,
        )?;
        let definition_ref = parameter.parameter_ref();
        application_parameters.push(BuiltinFxApplicationParameter {
            id: source.id(),
            parameter_type: source.parameter_type(),
            projection: BuiltinFxAbiProjection::Direct(parameter.index()),
        });
        parameter_refs.push((source.id(), definition_ref));
        definition_parameters.push(parameter);
    }

    let parameter_schema =
        FxDefinitionParameterSchema::new(definition_id.clone(), definition_parameters)?;
    let context = FxDefinitionGraphContext::new(&parameter_schema);
    let ordered_arguments = row
        .parameters()
        .iter()
        .copied()
        .filter(|source| {
            source.binding() == BuiltinFxParameterBinding::Abi
                && specialization.active_abi_parameters.contains(source.id())
        })
        .map(|source| {
            let reference = parameter_refs
                .iter()
                .find(|(id, _)| *id == source.id())
                .map(|(_, reference)| *reference)
                .ok_or(BuiltinFxBuildError::ParameterSchemaInvariant {
                    row: specialization.row,
                    parameter: source.id(),
                })?;
            Ok(BuiltinFxGraphArgument::Parameter(reference))
        })
        .collect::<Result<Vec<_>, BuiltinFxBuildError>>()?;
    let graph = build_builtin_fx_graph(&context, specialization, ordered_arguments)?;
    let definition = FxDefinition::from_parameter_schema(parameter_schema, graph)?;
    let binding_plan = BuiltinFxApplicationBindingPlan {
        definition: definition_id,
        schema: row.schema_digest(),
        layout: definition.parameter_layout().digest(),
        parameters: application_parameters.into_boxed_slice(),
    };
    Ok(BuiltinFxDefinitionTemplate {
        specialization,
        definition,
        binding_plan,
    })
}

/// Builds one builtin graph fragment against an outer definition schema.
///
/// `ordered_arguments` follows the active ABI parameters in the builtin row's
/// canonical source order. Each argument either reuses an outer definition
/// parameter or embeds one validated closed value. The returned graph retains
/// the outer definition identity and dense runtime schema; no cloned
/// definition or parameter-rewrite pass is created.
pub fn build_builtin_fx_graph(
    context: &FxDefinitionGraphContext<'_>,
    specialization: BuiltinFxSpecialization,
    ordered_arguments: impl IntoIterator<Item = BuiltinFxGraphArgument>,
) -> Result<FxGraph, BuiltinFxBuildError> {
    let row = row_schema(specialization.row)?;
    let active_parameters = row
        .parameters()
        .iter()
        .copied()
        .filter(|source| {
            source.binding() == BuiltinFxParameterBinding::Abi
                && specialization.active_abi_parameters.contains(source.id())
        })
        .collect::<Vec<_>>();
    let ordered_arguments = ordered_arguments.into_iter().collect::<Vec<_>>();
    if ordered_arguments.len() != active_parameters.len() {
        return Err(BuiltinFxBuildError::GraphArgumentCount {
            expected: active_parameters.len(),
            actual: ordered_arguments.len(),
        });
    }
    let parameters = active_parameters
        .into_iter()
        .zip(ordered_arguments)
        .map(|(source, argument)| graph_parameter(*context, specialization.row, source, argument))
        .collect::<Result<Vec<_>, BuiltinFxBuildError>>()?;
    let graph_context = GraphTemplateContext {
        specialization,
        definition: context.definition_id().clone(),
        runtime_types: context.runtime_types().collect(),
        parameters,
    };
    let graph = programs::build_graph(&graph_context)?;
    context.validate_graph(&graph)?;
    Ok(graph)
}

fn graph_parameter(
    context: FxDefinitionGraphContext<'_>,
    row: BuiltinFxCallableRowId,
    source: BuiltinFxCallableParameter,
    argument: BuiltinFxGraphArgument,
) -> Result<GraphParameter, BuiltinFxBuildError> {
    let expected = definition_parameter_type(source.parameter_type()).ok_or(
        BuiltinFxBuildError::ParameterSchemaInvariant {
            row,
            parameter: source.id(),
        },
    )?;
    let argument = match argument {
        BuiltinFxGraphArgument::Parameter(reference) => {
            let reference = context.validate_parameter_ref(reference)?;
            if reference.parameter_type() != expected {
                return Err(BuiltinFxBuildError::ArgumentType {
                    parameter: source.id(),
                    expected: source.parameter_type(),
                    actual: reference.parameter_type(),
                });
            }
            let runtime = match expected {
                FxDefinitionParameterType::Runtime(_) => context
                    .runtime_parameter_ref(reference.index())
                    .ok_or(BuiltinFxBuildError::NonRuntimeGraphParameter {
                        parameter: source.id(),
                    })
                    .map(Some)?,
                FxDefinitionParameterType::Resource | FxDefinitionParameterType::UniformRecord => {
                    None
                }
            };
            GraphParameterArgument::Parameter {
                definition: reference,
                runtime,
            }
        }
        BuiltinFxGraphArgument::Constant(value) => {
            validate_source_value(source, &value)?;
            if value.parameter_type() != expected {
                return Err(BuiltinFxBuildError::ArgumentType {
                    parameter: source.id(),
                    expected: source.parameter_type(),
                    actual: value.parameter_type(),
                });
            }
            GraphParameterArgument::Constant(value)
        }
    };
    Ok(GraphParameter {
        id: source.id(),
        argument,
    })
}

fn row_schema(row: BuiltinFxCallableRowId) -> Result<BuiltinFxCallableRow, BuiltinFxBuildError> {
    BUILTIN_FX_CALLABLE_CATALOG
        .get(row)
        .ok_or(BuiltinFxBuildError::CatalogInvariant { row })
}

fn definition_parameter_type(
    parameter_type: BuiltinFxParameterType,
) -> Option<FxDefinitionParameterType> {
    parameter_type.direct_definition_parameter_type()
}

fn source_default(parameter: &BuiltinFxCallableParameter) -> Option<BuiltinFxDefaultValue> {
    match parameter.presence() {
        BuiltinFxParameterPresence::Defaulted(value)
        | BuiltinFxParameterPresence::Conditional {
            default: Some(value),
            ..
        } => Some(value),
        BuiltinFxParameterPresence::Required
        | BuiltinFxParameterPresence::Optional
        | BuiltinFxParameterPresence::Conditional { default: None, .. } => None,
    }
}

fn materialize_default(
    value: BuiltinFxDefaultValue,
) -> Result<FxDefinitionArgumentValue, BuiltinFxBuildError> {
    let runtime = match value {
        BuiltinFxDefaultValue::Bool(value) => FxRuntimeValue::Bool(value),
        BuiltinFxDefaultValue::Milli(value) | BuiltinFxDefaultValue::RatioMilli(value) => {
            FxRuntimeValue::F32(finite_milli(value)?)
        }
        BuiltinFxDefaultValue::LengthMilliPx(value) => {
            FxRuntimeValue::Length(Length::try_pixels_f64(milli_f64(value)?)?)
        }
        BuiltinFxDefaultValue::AngleMilliDegrees(value) => {
            FxRuntimeValue::Angle(Angle::try_degrees(milli_f64(value)?)?)
        }
        BuiltinFxDefaultValue::DurationMillis(value) => {
            FxRuntimeValue::Seconds(Seconds::try_milliseconds(integer_f64(value)?)?)
        }
        BuiltinFxDefaultValue::Seed32(value) => FxRuntimeValue::U32(value),
        BuiltinFxDefaultValue::Vec2Milli([x, y]) => FxRuntimeValue::Vec2(FxVec2 {
            x: finite_milli(x)?,
            y: finite_milli(y)?,
        }),
        BuiltinFxDefaultValue::Phase(_)
        | BuiltinFxDefaultValue::Target(_)
        | BuiltinFxDefaultValue::MotionFunction(_) => {
            return Err(BuiltinFxBuildError::StructuralDefault(value));
        }
    };
    Ok(FxDefinitionArgumentValue::Runtime(runtime))
}

fn finite_milli(value: i64) -> Result<FiniteF32, BuiltinFxBuildError> {
    Ok(FiniteF32::try_from_f64(milli_f64(value)?)?)
}

fn milli_f64(value: i64) -> Result<f64, BuiltinFxBuildError> {
    Ok(integer_f64(value)? / 1_000.0)
}

fn integer_f64(value: i64) -> Result<f64, BuiltinFxBuildError> {
    let value = i32::try_from(value).map_err(|_| BuiltinFxBuildError::DefaultOutOfRange(value))?;
    Ok(f64::from(value))
}

fn validate_source_value(
    schema: BuiltinFxCallableParameter,
    value: &FxDefinitionArgumentValue,
) -> Result<(), BuiltinFxBuildError> {
    let type_matches = match (schema.parameter_type(), value) {
        (source, FxDefinitionArgumentValue::Runtime(actual)) => {
            source.runtime_value_type() == Some(actual.value_type())
        }
        (BuiltinFxParameterType::Resource, FxDefinitionArgumentValue::Resource(_)) => true,
        _ => false,
    };
    if !type_matches {
        return Err(BuiltinFxBuildError::ArgumentType {
            parameter: schema.id(),
            expected: schema.parameter_type(),
            actual: value.parameter_type(),
        });
    }
    if let BuiltinFxValueConstraint::Numeric(constraint) = schema.constraint() {
        let in_range = runtime_numeric_components(value).is_some_and(|components| {
            components.into_iter().all(|component| {
                let milli = component * 1_000.0;
                milli >= integer_as_f64(constraint.inclusive_min_milli)
                    && milli <= integer_as_f64(constraint.inclusive_max_milli)
            })
        });
        if !in_range {
            return Err(BuiltinFxBuildError::ArgumentConstraint {
                parameter: schema.id(),
            });
        }
    }
    Ok(())
}

fn runtime_numeric_components(value: &FxDefinitionArgumentValue) -> Option<Vec<f64>> {
    let FxDefinitionArgumentValue::Runtime(value) = value else {
        return None;
    };
    Some(match value {
        FxRuntimeValue::F32(value) => vec![f64::from(value.get())],
        FxRuntimeValue::Length(value) => vec![f64::from(value.pixels())],
        FxRuntimeValue::Angle(value) => vec![f64::from(value.radians()).to_degrees()],
        FxRuntimeValue::Seconds(value) => vec![f64::from(value.seconds())],
        FxRuntimeValue::Vec2(value) => vec![f64::from(value.x.get()), f64::from(value.y.get())],
        FxRuntimeValue::Bool(_)
        | FxRuntimeValue::I32(_)
        | FxRuntimeValue::Color(_)
        | FxRuntimeValue::Transform2D(_)
        | FxRuntimeValue::U32(_) => return None,
    })
}

fn integer_as_f64(value: i64) -> f64 {
    let high = i32::try_from(value / 1_000)
        .expect("builtin Fx numeric limits fit the exact chunked f64 conversion");
    let low = i32::try_from(value % 1_000).expect("builtin Fx numeric remainder always fits i32");
    f64::from(high) * 1_000.0 + f64::from(low)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{
        FxColor, FxEvaluationBudget, FxResourceId, FxSampleContext, FxStaticValue,
        ValueProgramInputs,
    };

    fn wave() -> BuiltinFxSpecialization {
        BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::WaveGlyphTransform,
            FxTarget::Glyph,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::Amplitude,
                BuiltinFxParameterId::Period,
                BuiltinFxParameterId::Speed,
                BuiltinFxParameterId::Direction,
            ]),
        )
        .unwrap()
    }

    #[test]
    fn numeric_values_bind_without_changing_definition_identity() {
        let template = build_builtin_fx_definition(wave()).unwrap();
        let first = template
            .bind_application(
                [BuiltinFxArgument::new(
                    BuiltinFxParameterId::Amplitude,
                    FxDefinitionArgumentValue::Runtime(FxRuntimeValue::Length(
                        Length::try_pixels(2.0).unwrap(),
                    )),
                )],
                0,
                None,
            )
            .unwrap();
        let second = template
            .bind_application(
                [BuiltinFxArgument::new(
                    BuiltinFxParameterId::Amplitude,
                    FxDefinitionArgumentValue::Runtime(FxRuntimeValue::Length(
                        Length::try_pixels(9.0).unwrap(),
                    )),
                )],
                1,
                None,
            )
            .unwrap();
        assert_eq!(first.definition(), second.definition());
        assert_ne!(
            first.template().initial_runtime(),
            second.template().initial_runtime()
        );
        assert_eq!(first.definition(), template.definition().id());
    }

    #[test]
    fn shader_resource_and_color_are_application_bindings() {
        let optional = BuiltinFxActiveAbiParameterSet::from_parameters([
            BuiltinFxParameterId::Resource,
            BuiltinFxParameterId::Color,
        ]);
        let specialization = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::ShaderPostProcess,
            FxTarget::Viewport,
            None,
            optional,
        )
        .unwrap();
        let template = build_builtin_fx_definition(specialization).unwrap();
        let bind = |resource: &str, color: [u8; 4]| {
            template
                .bind_application(
                    [
                        BuiltinFxArgument::new(
                            BuiltinFxParameterId::Resource,
                            FxDefinitionArgumentValue::Resource(
                                FxResourceId::try_new(resource).unwrap(),
                            ),
                        ),
                        BuiltinFxArgument::new(
                            BuiltinFxParameterId::Color,
                            FxDefinitionArgumentValue::Runtime(FxRuntimeValue::Color(
                                FxColor::from_rgba8(color),
                            )),
                        ),
                    ],
                    0,
                    None,
                )
                .unwrap()
        };
        let first = bind("screen.tint.red", [255, 0, 0, 255]);
        let second = bind("screen.tint.blue", [0, 0, 255, 255]);
        assert_eq!(first.definition(), second.definition());
        assert_ne!(
            first.template().static_arguments(),
            second.template().static_arguments()
        );
        assert_ne!(
            first.template().initial_runtime(),
            second.template().initial_runtime()
        );
    }

    #[test]
    fn only_structural_shape_changes_builtin_definition_id() {
        let base = wave();
        let post = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::WavePostProcess,
            FxTarget::Viewport,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::Amplitude,
                BuiltinFxParameterId::Period,
                BuiltinFxParameterId::Speed,
                BuiltinFxParameterId::Direction,
                BuiltinFxParameterId::Seed,
            ]),
        )
        .unwrap();
        let shader_without_color = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::ShaderPostProcess,
            FxTarget::Viewport,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([BuiltinFxParameterId::Resource]),
        )
        .unwrap();
        let shader_with_color = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::ShaderPostProcess,
            FxTarget::Viewport,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::Resource,
                BuiltinFxParameterId::Color,
            ]),
        )
        .unwrap();
        assert_ne!(base.definition_id().unwrap(), post.definition_id().unwrap());
        assert_ne!(
            shader_without_color.definition_id().unwrap(),
            shader_with_color.definition_id().unwrap()
        );
    }

    #[test]
    fn binding_plan_maps_each_source_value_to_one_definition_parameter() {
        let template = build_builtin_fx_definition(wave()).unwrap();
        let direction = template
            .binding_plan()
            .parameters()
            .iter()
            .find(|parameter| parameter.id() == BuiltinFxParameterId::Direction)
            .unwrap();
        assert!(matches!(
            direction.projection(),
            BuiltinFxAbiProjection::Direct(_)
        ));
        let definition_parameter = template
            .definition()
            .parameters()
            .iter()
            .find(|parameter| parameter.name() == "direction")
            .expect("direct vector ABI row");
        assert_eq!(
            definition_parameter.parameter_type(),
            FxDefinitionParameterType::Runtime(FxRuntimeType::Vec2)
        );
        assert_eq!(template.definition().parameters().len(), 4);
    }

    #[test]
    fn duration_abi_is_one_direct_seconds_definition_parameter() {
        let specialization = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::TypewriterGlyphMask,
            FxTarget::Content,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::CharactersPerSecond,
                BuiltinFxParameterId::Delay,
                BuiltinFxParameterId::Cursor,
            ]),
        )
        .unwrap();
        let template = build_builtin_fx_definition(specialization).unwrap();
        let delay = template
            .binding_plan()
            .parameters()
            .iter()
            .find(|parameter| parameter.id() == BuiltinFxParameterId::Delay)
            .expect("direct duration ABI row");
        assert!(matches!(
            delay.projection(),
            BuiltinFxAbiProjection::Direct(_)
        ));
        let definition_parameter = template
            .definition()
            .parameters()
            .iter()
            .find(|parameter| parameter.name() == "delay")
            .expect("direct duration ABI row");
        assert_eq!(
            definition_parameter.parameter_type(),
            FxDefinitionParameterType::Runtime(FxRuntimeType::Seconds)
        );
    }

    #[test]
    fn builtin_graphs_project_direct_vector_and_duration_abi_values() {
        let wave_template = build_builtin_fx_definition(wave()).unwrap();
        let wave_application = wave_template
            .bind_application([], 0, None)
            .expect("builtin defaults bind through the direct ABI");
        let wave_sampler = first_sampler(&wave_template);
        let wave_context =
            FxSampleContext::from_elapsed(Seconds::try_seconds(0.25).unwrap(), 0, 0, false);
        let mut wave_budget = FxEvaluationBudget::default();
        let FxRuntimeValue::Transform2D(wave_transform) = wave_sampler
            .evaluate(
                ValueProgramInputs {
                    parameters: wave_application.template().initial_runtime(),
                    state: &[],
                },
                wave_context,
                &mut wave_budget,
            )
            .unwrap()
        else {
            panic!("wave builtin must return a transform");
        };
        assert_eq!(wave_transform.translate_x, Length::ZERO);
        assert!(
            (wave_transform.translate_y.pixels() - 4.0).abs() <= f32::EPSILON,
            "wave transform y translation should be exactly four pixels"
        );

        let typewriter_specialization = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::TypewriterGlyphMask,
            FxTarget::Content,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::CharactersPerSecond,
                BuiltinFxParameterId::Delay,
                BuiltinFxParameterId::Cursor,
            ]),
        )
        .unwrap();
        let typewriter_template = build_builtin_fx_definition(typewriter_specialization).unwrap();
        let typewriter_application = typewriter_template
            .bind_application([], 0, None)
            .expect("builtin defaults bind through the direct ABI");
        let typewriter_sampler = first_sampler(&typewriter_template);
        let mut typewriter_budget = FxEvaluationBudget::default();
        assert_eq!(
            typewriter_sampler
                .evaluate(
                    ValueProgramInputs {
                        parameters: typewriter_application.template().initial_runtime(),
                        state: &[],
                    },
                    FxSampleContext::from_elapsed(
                        Seconds::try_seconds(0.25).unwrap(),
                        0,
                        0,
                        false,
                    ),
                    &mut typewriter_budget,
                )
                .unwrap(),
            FxRuntimeValue::F32(FiniteF32::ONE)
        );
    }

    fn first_sampler(template: &BuiltinFxDefinitionTemplate) -> &crate::fx::FxSamplerProgram {
        template.definition().graph().nodes()[0]
            .properties()
            .unwrap()
            .iter()
            .find(|property| matches!(property.value(), FxStaticValue::Sampler(_)))
            .and_then(|property| match property.value() {
                FxStaticValue::Sampler(program) => Some(program),
                _ => None,
            })
            .expect("builtin graph has one sampler")
    }

    #[test]
    fn parameter_schema_roundtrips_and_seals_the_same_definition_layout() {
        let template = build_builtin_fx_definition(wave()).unwrap();
        let schema = FxDefinitionParameterSchema::new(
            template.definition().id().clone(),
            template.definition().parameters().to_vec(),
        )
        .unwrap();
        let decoded =
            FxDefinitionParameterSchema::decode_canonical_v1(&schema.canonical_v1_bytes().unwrap())
                .unwrap();
        assert_eq!(decoded, schema);
        let expected_digest = schema.digest();
        let expected_layout = schema.parameter_layout().clone();
        let definition =
            FxDefinition::from_parameter_schema(schema, template.definition().graph().clone())
                .unwrap();
        assert_eq!(definition.parameter_layout(), &expected_layout);
        assert_eq!(
            FxDefinitionParameterSchema::new(
                definition.id().clone(),
                definition.parameters().to_vec(),
            )
            .unwrap()
            .digest(),
            expected_digest
        );
    }

    #[test]
    fn active_abi_set_rejects_structural_rows_and_missing_mandatory_rows() {
        let structural = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::WaveGlyphTransform,
            FxTarget::Glyph,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::Phase,
                BuiltinFxParameterId::Amplitude,
                BuiltinFxParameterId::Period,
                BuiltinFxParameterId::Speed,
                BuiltinFxParameterId::Direction,
            ]),
        );
        assert!(matches!(
            structural,
            Err(BuiltinFxBuildError::InvalidActiveAbiParameter {
                parameter: BuiltinFxParameterId::Phase,
                ..
            })
        ));

        let missing = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::WaveGlyphTransform,
            FxTarget::Glyph,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::Amplitude,
                BuiltinFxParameterId::Period,
                BuiltinFxParameterId::Speed,
            ]),
        );
        assert!(matches!(
            missing,
            Err(BuiltinFxBuildError::MissingActiveAbiParameter {
                parameter: BuiltinFxParameterId::Direction,
                ..
            })
        ));
    }

    #[test]
    fn inactive_typewriter_cursor_alpha_is_not_required_by_the_graph() {
        let specialization = BuiltinFxSpecialization::try_new(
            BuiltinFxCallableRowId::TypewriterGlyphMask,
            FxTarget::Content,
            None,
            BuiltinFxActiveAbiParameterSet::from_parameters([
                BuiltinFxParameterId::CharactersPerSecond,
                BuiltinFxParameterId::Delay,
                BuiltinFxParameterId::Cursor,
            ]),
        )
        .unwrap();
        let template = build_builtin_fx_definition(specialization).unwrap();
        assert!(
            template
                .binding_plan()
                .parameters()
                .iter()
                .all(|parameter| parameter.id() != BuiltinFxParameterId::CursorAlpha)
        );
    }
}
