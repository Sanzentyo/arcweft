//! Static request-role transcript. It never reads expression literal payloads.

use super::function::{OriginalRequestEndpoint, ProducerEndpoint};
use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

mod admission;
pub(crate) mod codec;

/// Source-checked request proof; wire bytes remain untrusted definitions.
///
/// ```compile_fail
/// use arcweft_core::plan::TaskRequestTemplateDigest;
/// let _ = TaskRequestTemplateDigest::from_bytes([0; 32]);
/// ```
/// ```compile_fail
/// fn encode<T: serde::Serialize>() {}
/// encode::<arcweft_core::plan::TaskRequestTemplateDigest>();
/// ```
/// ```compile_fail
/// fn decode<T: serde::de::DeserializeOwned>() {}
/// decode::<arcweft_core::plan::TaskRequestTemplateDigest>();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskRequestTemplateDigest([u8; 32]);
impl TaskRequestTemplateDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub(crate) use crate::task::RuntimeRequestRoleIdentity;

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeRequestArgumentRole {
    Positional,
    Named,
    Spread,
    Capture,
    AwaitManyItem,
    TimeoutSource,
    TimeoutLimit,
    LineInput,
}
impl RuntimeRequestArgumentRole {
    const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Positional => 0,
            Self::Named => 1,
            Self::Spread => 2,
            Self::Capture => 3,
            Self::AwaitManyItem => 4,
            Self::TimeoutSource => 5,
            Self::TimeoutLimit => 6,
            Self::LineInput => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRequestValueSource {
    Literal,
    Local,
    Capture,
    Projection,
    CallResult,
    AggregateItem,
    NeedHandle,
}
impl RuntimeRequestValueSource {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::Literal => 0,
            Self::Local => 1,
            Self::Capture => 2,
            Self::Projection => 3,
            Self::CallResult => 4,
            Self::AggregateItem => 5,
            Self::NeedHandle => 6,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeRequestPathStep {
    Operand(u32),
    Tuple(u32),
    Record(RuntimeRequestRoleIdentity),
    Variant(RuntimeRequestRoleIdentity),
    CallArgument(u32),
    NamedArgument(RuntimeRequestRoleIdentity),
    SpreadArgument(u32),
    Capture(u32),
    AwaitManyItem,
    TimeoutSource,
    TimeoutLimit,
    LineChild(u32),
}
impl RuntimeRequestPathStep {
    fn semantic_tag(&self) -> u8 {
        match self {
            Self::Operand(_) => 0,
            Self::Tuple(_) => 1,
            Self::Record(_) => 2,
            Self::Variant(_) => 3,
            Self::CallArgument(_) => 4,
            Self::NamedArgument(_) => 5,
            Self::SpreadArgument(_) => 6,
            Self::Capture(_) => 7,
            Self::AwaitManyItem => 8,
            Self::TimeoutSource => 9,
            Self::TimeoutLimit => 10,
            Self::LineChild(_) => 11,
        }
    }
    fn encode(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.tag(self.semantic_tag());
        match self {
            Self::Operand(value)
            | Self::Tuple(value)
            | Self::CallArgument(value)
            | Self::SpreadArgument(value)
            | Self::Capture(value)
            | Self::LineChild(value) => encoder.ordinal(*value),
            Self::Record(identity) | Self::Variant(identity) | Self::NamedArgument(identity) => {
                encoder.digest(identity.as_bytes());
            }
            Self::AwaitManyItem | Self::TimeoutSource | Self::TimeoutLimit => {}
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeRequestArgument<Ty = crate::runtime_id::RuntimePlanTypeId> {
    pub role: RuntimeRequestArgumentRole,
    pub identity: Option<RuntimeRequestRoleIdentity>,
    pub ty: Ty,
    pub source: RuntimeRequestValueSource,
    pub path: Box<[RuntimeRequestPathStep]>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeRequestFieldRole {
    Required,
    Optional,
    Repeated,
    NamedOnly,
    PositionalOnly,
}
impl RuntimeRequestFieldRole {
    fn semantic_tag(&self) -> u8 {
        match self {
            Self::Required => 0,
            Self::Optional => 1,
            Self::Repeated => 2,
            Self::NamedOnly => 3,
            Self::PositionalOnly => 4,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeRequestField<Ty = crate::runtime_id::RuntimePlanTypeId> {
    pub identity: RuntimeRequestRoleIdentity,
    pub role: RuntimeRequestFieldRole,
    pub ty: Ty,
    pub path: Box<[RuntimeRequestPathStep]>,
}

/// The static request definition owned by a private task image. Runtime
/// argument values stay on their execution owner. These endpoint/role/type/path
/// definitions issue no Q proof until checked against the original F source.
#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeTaskRequestTemplate<Ty = crate::runtime_id::RuntimePlanTypeId> {
    source: RuntimeTaskRequestSource,
    pub(super) arguments: Box<[RuntimeRequestArgument<Ty>]>,
    fields: Box<[RuntimeRequestField<Ty>]>,
}

/// Construction data uses semantic types in the same definition grammar.
pub type RuntimeTaskRequestTemplateSeed =
    RuntimeTaskRequestTemplate<crate::pattern::RuntimeSemanticTypeId>;

/// Selects either the original function ABI or one original operation endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTaskRequestSource {
    FunctionRoot,
    Operation(u32),
}

impl<Ty> RuntimeTaskRequestTemplate<Ty> {
    #[must_use]
    pub fn new(
        source: RuntimeTaskRequestSource,
        arguments: Box<[RuntimeRequestArgument<Ty>]>,
        fields: Box<[RuntimeRequestField<Ty>]>,
    ) -> Self {
        Self {
            source,
            arguments,
            fields,
        }
    }

    #[must_use]
    pub fn arguments(&self) -> &[RuntimeRequestArgument<Ty>] {
        &self.arguments
    }

    #[must_use]
    pub fn fields(&self) -> &[RuntimeRequestField<Ty>] {
        &self.fields
    }

    /// The owning builder preflights the whole candidate before mapping types.
    /// All other role/source/path storage moves without cloning or re-extraction.
    pub(crate) fn try_map_types<Next, E>(
        self,
        mut map: impl FnMut(Ty) -> Result<Next, E>,
    ) -> Result<RuntimeTaskRequestTemplate<Next>, E> {
        let arguments = self
            .arguments
            .into_vec()
            .into_iter()
            .map(|argument| {
                Ok(RuntimeRequestArgument {
                    role: argument.role,
                    identity: argument.identity,
                    ty: map(argument.ty)?,
                    source: argument.source,
                    path: argument.path,
                })
            })
            .collect::<Result<Vec<_>, E>>()?
            .into_boxed_slice();
        let fields = self
            .fields
            .into_vec()
            .into_iter()
            .map(|field| {
                Ok(RuntimeRequestField {
                    identity: field.identity,
                    role: field.role,
                    ty: map(field.ty)?,
                    path: field.path,
                })
            })
            .collect::<Result<Vec<_>, E>>()?
            .into_boxed_slice();
        Ok(RuntimeTaskRequestTemplate {
            source: self.source,
            arguments,
            fields,
        })
    }
    pub(in crate::plan) fn try_visit_path_lengths<E>(
        &self,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        for argument in &self.arguments {
            visitor(argument.path.len())?;
        }
        for field in &self.fields {
            visitor(field.path.len())?;
        }
        Ok(())
    }

    pub(super) fn known_transcript_bytes(
        &self,
    ) -> Result<u64, crate::task::semantic::TaskSemanticEncodingError> {
        use crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow;
        let add = |left: u64, right: u64| left.checked_add(right).ok_or(ArithmeticOverflow);
        let path_bytes = |path: &[RuntimeRequestPathStep]| {
            path.iter().try_fold(0_u64, |bytes, step| {
                let payload = match step {
                    RuntimeRequestPathStep::Operand(_)
                    | RuntimeRequestPathStep::Tuple(_)
                    | RuntimeRequestPathStep::CallArgument(_)
                    | RuntimeRequestPathStep::SpreadArgument(_)
                    | RuntimeRequestPathStep::Capture(_)
                    | RuntimeRequestPathStep::LineChild(_) => 4,
                    RuntimeRequestPathStep::Record(_)
                    | RuntimeRequestPathStep::Variant(_)
                    | RuntimeRequestPathStep::NamedArgument(_) => 32,
                    RuntimeRequestPathStep::AwaitManyItem
                    | RuntimeRequestPathStep::TimeoutSource
                    | RuntimeRequestPathStep::TimeoutLimit => 0,
                };
                add(bytes, 1 + payload)
            })
        };
        // domain + F/endpoint/kind + argument and field counts
        let mut bytes = add(
            b"arcweft.task.request-template.v1\0".len() as u64,
            32 + 4 + 1 + 4 + 4,
        )?;
        for argument in &self.arguments {
            bytes = add(
                bytes,
                4 + 1 + 1 + u64::from(argument.identity.is_some()) * 32 + 32 + 1 + 4,
            )?;
            bytes = add(bytes, path_bytes(&argument.path)?)?;
        }
        for field in &self.fields {
            bytes = add(bytes, 4 + 32 + 1 + 32 + 4)?;
            bytes = add(bytes, path_bytes(&field.path)?)?;
        }
        Ok(bytes)
    }

    #[must_use]
    pub const fn source(&self) -> RuntimeTaskRequestSource {
        self.source
    }

    pub(in crate::plan) fn role_count(
        &self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<usize, RuntimeBodySemanticError> {
        meter
            .checked_count_sum(self.arguments.len(), self.fields.len())
            .map_err(Into::into)
    }

    pub(crate) fn at_operation(
        endpoint: u32,
        arguments: Box<[RuntimeRequestArgument<Ty>]>,
        fields: Box<[RuntimeRequestField<Ty>]>,
    ) -> Self {
        Self {
            source: RuntimeTaskRequestSource::Operation(endpoint),
            arguments,
            fields,
        }
    }
}

impl RuntimeBodySemanticContext<'_> {
    /// Reads original function inputs or operation arguments through F's proof.
    /// Literal/evaluated payloads and display names never supply Q evidence.
    pub(crate) fn static_request_template_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: ProducerEndpoint<'_>,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<TaskRequestTemplateDigest, RuntimeBodySemanticError> {
        let template = self.static_request_template(meter, endpoint, limits)?;
        self.encode_request_template(meter, endpoint, &template, limits)
            .map(|hash| TaskRequestTemplateDigest(*hash.as_bytes()))
    }

    /// Extracts the static definition for storage/codec from the same actual
    /// endpoint used by the Q transcript. Extraction issues no digest proof.
    pub(crate) fn static_request_template<'source>(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: impl Into<OriginalRequestEndpoint<'source>>,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<RuntimeTaskRequestTemplate, RuntimeBodySemanticError> {
        let endpoint = endpoint.into();
        meter.status()?;
        if !endpoint.belongs_to(self.plan) {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::InvalidRequestSource);
        }
        endpoint.definition(meter, limits)
    }
}

impl OriginalRequestEndpoint<'_> {
    pub(in crate::plan) fn definition(
        self,
        meter: &mut TaskSemanticMeter,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<RuntimeTaskRequestTemplate, RuntimeBodySemanticError> {
        use crate::plan::RuntimeFunctionInputSource as Source;
        let endpoint = self;
        meter.status()?;
        if let Some(inputs) = endpoint.function_inputs() {
            let actual = meter.checked_count_sum(inputs.len(), 0)?;
            if actual > limits.max_request_roles as usize {
                meter.reject_owner();
                return Err(RuntimeBodySemanticError::RequestRoles {
                    actual,
                    maximum: limits.max_request_roles,
                });
            }
            let mut arguments = Vec::with_capacity(actual);
            for input in inputs {
                meter.charge_work(1)?;
                let (role, source, path) = match input.source() {
                    Source::Capture { position } | Source::CapturedParameter { position, .. } => (
                        RuntimeRequestArgumentRole::Capture,
                        RuntimeRequestValueSource::Capture,
                        RuntimeRequestPathStep::Capture(position),
                    ),
                    Source::Parameter { position, .. } => (
                        RuntimeRequestArgumentRole::Positional,
                        RuntimeRequestValueSource::Local,
                        RuntimeRequestPathStep::Operand(position),
                    ),
                };
                arguments.push(RuntimeRequestArgument {
                    role,
                    identity: None,
                    ty: input.pattern().ty(),
                    source,
                    path: Box::new([path]),
                });
            }
            return Ok(RuntimeTaskRequestTemplate {
                source: RuntimeTaskRequestSource::FunctionRoot,
                arguments: arguments.into_boxed_slice(),
                fields: Box::new([]),
            });
        }
        let actual_arguments = endpoint.host_arguments().ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::InvalidRequestSource
        })?;
        let actual = meter.checked_count_sum(actual_arguments.len(), 0)?;
        if actual > limits.max_request_roles as usize {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::RequestRoles {
                actual,
                maximum: limits.max_request_roles,
            });
        }
        let mut arguments = Vec::with_capacity(actual);
        for (ordinal, argument) in actual_arguments.iter().enumerate() {
            meter.charge_work(1)?;
            let ordinal = u32::try_from(ordinal).map_err(|_| {
                meter.reject_owner();
                RuntimeBodySemanticError::Encoding(
                    crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow,
                )
            })?;
            arguments.push(argument.request_definition(
                ordinal,
                argument.value().ty(),
                endpoint.request_value_source(meter, argument.value())?,
            ));
        }
        Ok(RuntimeTaskRequestTemplate::at_operation(
            endpoint.ordinal(),
            arguments.into_boxed_slice(),
            Box::new([]),
        ))
    }

    fn request_value_source(
        &self,
        meter: &mut TaskSemanticMeter,
        mut expression: &crate::value::RuntimeExpr,
    ) -> Result<RuntimeRequestValueSource, RuntimeBodySemanticError> {
        use crate::value::RuntimeExprKind as Expr;
        loop {
            meter.charge_work(1)?;
            if self
                .source_type_is_need(expression.ty())
                .inspect_err(|_| meter.reject_owner())?
            {
                return Ok(RuntimeRequestValueSource::NeedHandle);
            }
            return Ok(match expression.kind() {
                Expr::Value(_) | Expr::EntityRef(_) => RuntimeRequestValueSource::Literal,
                Expr::Local(read) if !read.fields().is_empty() => {
                    RuntimeRequestValueSource::Projection
                }
                Expr::Local(read) => {
                    meter.charge_work(1)?;
                    let captured = self
                        .source_local_is_capture(read.local())
                        .inspect_err(|_| meter.reject_owner())?;
                    if captured {
                        RuntimeRequestValueSource::Capture
                    } else {
                        RuntimeRequestValueSource::Local
                    }
                }
                Expr::Field { .. } | Expr::ProjectTuple { .. } | Expr::ProjectRecord { .. } => {
                    RuntimeRequestValueSource::Projection
                }
                Expr::Tuple(_)
                | Expr::BracketSeq(_)
                | Expr::RepeatSeq { .. }
                | Expr::Range { .. }
                | Expr::NominalRecord(_)
                | Expr::Variant { .. } => RuntimeRequestValueSource::AggregateItem,
                Expr::Let { body, .. } | Expr::Scope { body, .. } | Expr::Assign { body, .. } => {
                    expression = body;
                    continue;
                }
                Expr::Agent(_)
                | Expr::SequencePopFront { .. }
                | Expr::SequencePopBack { .. }
                | Expr::SequencePush { .. }
                | Expr::DialogueContent { .. }
                | Expr::FormatContent { .. }
                | Expr::CharacterDialogue { .. }
                | Expr::Call { .. }
                | Expr::MakeCallable { .. }
                | Expr::SpecializeCallable { .. }
                | Expr::ApplyGroup { .. }
                | Expr::TraitCall { .. }
                | Expr::PureCall { .. }
                | Expr::StandardMap { .. }
                | Expr::Sum { .. }
                | Expr::Unary { .. }
                | Expr::Binary { .. }
                | Expr::If { .. }
                | Expr::IfLet { .. }
                | Expr::Match { .. }
                | Expr::ReductionUnchanged { .. } => RuntimeRequestValueSource::CallResult,
            });
        }
    }
}

impl RuntimeBodySemanticContext<'_> {
    /// Encodes the definition grammar without issuing admission evidence.
    /// The source-derived and strict decoded-definition paths own Q issuance.
    pub(super) fn encode_request_template(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: ProducerEndpoint<'_>,
        template: &RuntimeTaskRequestTemplate,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        let arguments = &template.arguments;
        let fields = &template.fields;
        let actual = meter.checked_count_sum(arguments.len(), fields.len())?;
        if actual > limits.max_request_roles as usize {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::RequestRoles {
                actual,
                maximum: limits.max_request_roles,
            });
        }
        if !endpoint.belongs_to(self.plan) {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::InvalidRequestSource);
        }
        if template.source != endpoint.request_source() {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::InvalidRequestEndpoint {
                expected: endpoint.request_source(),
                actual: template.source,
            });
        }
        let mut encoder = TaskSemanticEncoder::new(b"arcweft.task.request-template.v1\0", meter);
        encoder.digest(endpoint.producer_digest().as_bytes());
        encoder.ordinal(endpoint.ordinal());
        encoder.tag(endpoint.kind().semantic_tag());
        encoder.count(arguments.len());
        for (ordinal, argument) in arguments.iter().enumerate() {
            encoder.enter_element();
            encoder.enter_role();
            encoder.count(ordinal);
            encoder.tag(argument.role.semantic_tag());
            encoder.tag(u8::from(argument.identity.is_some()));
            if let Some(identity) = &argument.identity {
                encoder.digest(identity.as_bytes());
            }
            self.write_type(&mut encoder, argument.ty)?;
            encoder.tag(argument.source.semantic_tag());
            encoder.count(argument.path.len());
            for step in &argument.path {
                encoder.enter_element();
                encoder.enter_role();
                step.encode(&mut encoder);
            }
        }
        encoder.count(fields.len());
        for (ordinal, field) in fields.iter().enumerate() {
            encoder.enter_element();
            encoder.enter_role();
            encoder.count(ordinal);
            encoder.digest(field.identity.as_bytes());
            encoder.tag(field.role.semantic_tag());
            self.write_type(&mut encoder, field.ty)?;
            encoder.count(field.path.len());
            for step in &field.path {
                encoder.enter_element();
                encoder.enter_role();
                step.encode(&mut encoder);
            }
        }
        encoder.finish().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_request_path_grammar_covers_all_twelve_steps() {
        let steps = [
            RuntimeRequestPathStep::Operand(4),
            RuntimeRequestPathStep::Tuple(5),
            RuntimeRequestPathStep::Record(RuntimeRequestRoleIdentity::from_accepted_identity(
                [6; 32],
            )),
            RuntimeRequestPathStep::Variant(RuntimeRequestRoleIdentity::from_accepted_identity(
                [7; 32],
            )),
            RuntimeRequestPathStep::CallArgument(8),
            RuntimeRequestPathStep::NamedArgument(
                RuntimeRequestRoleIdentity::from_accepted_identity([9; 32]),
            ),
            RuntimeRequestPathStep::SpreadArgument(10),
            RuntimeRequestPathStep::Capture(11),
            RuntimeRequestPathStep::AwaitManyItem,
            RuntimeRequestPathStep::TimeoutSource,
            RuntimeRequestPathStep::TimeoutLimit,
            RuntimeRequestPathStep::LineChild(12),
        ];
        let mut meter = TaskSemanticMeter::new(100, 1000);
        let mut encoder = TaskSemanticEncoder::new(b"p", &mut meter);
        for step in steps {
            step.encode(&mut encoder);
        }
        let mut expected = vec![b'p'];
        for (tag, ordinal) in [(0, 4u32), (1, 5)] {
            expected.push(tag);
            expected.extend(ordinal.to_le_bytes());
        }
        expected.push(2);
        expected.extend([6; 32]);
        expected.push(3);
        expected.extend([7; 32]);
        expected.push(4);
        expected.extend(8u32.to_le_bytes());
        expected.push(5);
        expected.extend([9; 32]);
        expected.push(6);
        expected.extend(10u32.to_le_bytes());
        expected.push(7);
        expected.extend(11u32.to_le_bytes());
        expected.extend([8, 9, 10, 11]);
        expected.extend(12u32.to_le_bytes());
        assert_eq!(encoder.finish().unwrap(), blake3::hash(&expected));
    }
}
