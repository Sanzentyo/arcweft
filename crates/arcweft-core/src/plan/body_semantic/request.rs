//! Static request-role transcript. It never reads expression literal payloads.

use super::function::ProducerEndpoint;
use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TaskRequestTemplateDigest([u8; 32]);
impl TaskRequestTemplateDigest {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub(crate) use crate::task::RuntimeRequestRoleIdentity;

pub(crate) enum RuntimeRequestArgumentRole {
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

pub(crate) enum RuntimeRequestValueSource {
    Literal,
    Local,
    Capture,
    Projection,
    CallResult,
    AggregateItem,
    NeedHandle,
}
impl RuntimeRequestValueSource {
    const fn semantic_tag(&self) -> u8 {
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

pub(crate) enum RuntimeRequestPathStep {
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
    fn encode(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Operand(ordinal) => {
                encoder.tag(0);
                encoder.ordinal(*ordinal);
            }
            Self::Tuple(ordinal) => {
                encoder.tag(1);
                encoder.ordinal(*ordinal);
            }
            Self::Record(identity) => {
                encoder.tag(2);
                encoder.digest(identity.as_bytes());
            }
            Self::Variant(identity) => {
                encoder.tag(3);
                encoder.digest(identity.as_bytes());
            }
            Self::CallArgument(ordinal) => {
                encoder.tag(4);
                encoder.ordinal(*ordinal);
            }
            Self::NamedArgument(identity) => {
                encoder.tag(5);
                encoder.digest(identity.as_bytes());
            }
            Self::SpreadArgument(ordinal) => {
                encoder.tag(6);
                encoder.ordinal(*ordinal);
            }
            Self::Capture(ordinal) => {
                encoder.tag(7);
                encoder.ordinal(*ordinal);
            }
            Self::AwaitManyItem => encoder.tag(8),
            Self::TimeoutSource => encoder.tag(9),
            Self::TimeoutLimit => encoder.tag(10),
            Self::LineChild(ordinal) => {
                encoder.tag(11);
                encoder.ordinal(*ordinal);
            }
        }
    }
}

pub(crate) struct RuntimeRequestArgument {
    pub(crate) role: RuntimeRequestArgumentRole,
    pub(crate) identity: Option<RuntimeRequestRoleIdentity>,
    pub(crate) ty: crate::runtime_id::RuntimePlanTypeId,
    pub(crate) source: RuntimeRequestValueSource,
    pub(crate) path: Box<[RuntimeRequestPathStep]>,
}

pub(crate) enum RuntimeRequestFieldRole {
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

pub(crate) struct RuntimeRequestField {
    pub(crate) identity: RuntimeRequestRoleIdentity,
    pub(crate) role: RuntimeRequestFieldRole,
    pub(crate) ty: crate::runtime_id::RuntimePlanTypeId,
    pub(crate) path: Box<[RuntimeRequestPathStep]>,
}

impl RuntimeBodySemanticContext<'_> {
    /// Reads the actual admitted `HostCall` referenced by F's opaque endpoint.
    /// Literal/evaluated payloads and display names never supply Q evidence.
    pub(crate) fn host_request_template_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: ProducerEndpoint<'_>,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<TaskRequestTemplateDigest, RuntimeBodySemanticError> {
        meter.status()?;
        if !endpoint.belongs_to(self.plan) {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::InvalidHostRequestEndpoint);
        }
        let target = endpoint.host_target().ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::InvalidHostRequestEndpoint
        })?;
        let actual = meter.checked_count_sum(target.args.len(), 0)?;
        if actual > limits.max_request_roles as usize {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::RequestRoles {
                actual,
                maximum: limits.max_request_roles,
            });
        }
        let mut arguments = Vec::with_capacity(actual);
        for (ordinal, argument) in target.args.iter().enumerate() {
            meter.charge_work(1)?;
            let ordinal = u32::try_from(ordinal).map_err(|_| {
                meter.reject_owner();
                RuntimeBodySemanticError::Encoding(
                    crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow,
                )
            })?;
            let identity = argument.identity();
            let mut path = vec![RuntimeRequestPathStep::Operand(ordinal)];
            let (role, name) = match argument {
                crate::task::RuntimeHostArgumentTemplate::Positional(..) => {
                    (RuntimeRequestArgumentRole::Positional, None)
                }
                crate::task::RuntimeHostArgumentTemplate::Named(..) => {
                    path.push(RuntimeRequestPathStep::NamedArgument(identity));
                    (RuntimeRequestArgumentRole::Named, Some(identity))
                }
                crate::task::RuntimeHostArgumentTemplate::Spread(..) => {
                    path.push(RuntimeRequestPathStep::SpreadArgument(ordinal));
                    (RuntimeRequestArgumentRole::Spread, None)
                }
            };
            arguments.push(RuntimeRequestArgument {
                role,
                identity: name,
                ty: argument.value().ty(),
                source: self.request_value_source(meter, endpoint, argument.value())?,
                path: path.into_boxed_slice(),
            });
        }
        self.request_template_digest(meter, endpoint, &arguments, &[], limits)
    }

    fn request_value_source(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: ProducerEndpoint<'_>,
        mut expression: &crate::value::RuntimeExpr,
    ) -> Result<RuntimeRequestValueSource, RuntimeBodySemanticError> {
        use crate::value::RuntimeExprKind as Expr;
        loop {
            meter.charge_work(1)?;
            let ty = self.plan.type_table().get(expression.ty()).ok_or_else(|| {
                meter.reject_owner();
                RuntimeBodySemanticError::UnknownType {
                    ty: expression.ty(),
                }
            })?;
            if matches!(
                ty.projection(),
                crate::plan::RuntimePlanTypeProjection::Need(_)
            ) {
                return Ok(RuntimeRequestValueSource::NeedHandle);
            }
            return Ok(match expression.kind() {
                Expr::Value(_) | Expr::EntityRef(_) => RuntimeRequestValueSource::Literal,
                Expr::Local(read) if !read.fields().is_empty() => {
                    RuntimeRequestValueSource::Projection
                }
                Expr::Local(read) => {
                    let mut captured = false;
                    for input in endpoint.function().capture_inputs() {
                        meter.charge_work(1)?;
                        if input.input_local() == read.local() {
                            captured = true;
                            break;
                        }
                        crate::value::RuntimeExpressionNode::Pattern(input.pattern()).try_visit_owned_events(&mut |event| {
                            meter.charge_work(1)?;
                            if let crate::value::expression_tree::RuntimeExpressionTreeEvent::Enter { node: crate::value::RuntimeExpressionNode::Pattern(pattern), .. } = event {
                                match pattern.kind() {
                                    crate::pattern::RuntimePatternKind::Bind { binding, .. }
                                    | crate::pattern::RuntimePatternKind::Whole { binding, .. }
                                    | crate::pattern::RuntimePatternKind::Typed { binding } => { captured |= binding.local() == read.local(); }
                                    crate::pattern::RuntimePatternKind::Record { rest, .. }
                                    | crate::pattern::RuntimePatternKind::Sequence { rest, .. } => { captured |= rest.binding().is_some_and(|binding| binding.local() == read.local()); }
                                    crate::pattern::RuntimePatternKind::Discard | crate::pattern::RuntimePatternKind::Literal(_)
                                    | crate::pattern::RuntimePatternKind::Entity(_) | crate::pattern::RuntimePatternKind::Tuple(_)
                                    | crate::pattern::RuntimePatternKind::Or(_) | crate::pattern::RuntimePatternKind::Variant { .. } => {}
                                }
                            }
                            Ok::<(), crate::task::semantic::TaskSemanticEncodingError>(())
                        })?;
                        if captured {
                            break;
                        }
                    }
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

    pub(crate) fn request_template_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: ProducerEndpoint<'_>,
        arguments: &[RuntimeRequestArgument],
        fields: &[RuntimeRequestField],
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<TaskRequestTemplateDigest, RuntimeBodySemanticError> {
        meter.status()?;
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
            return Err(RuntimeBodySemanticError::InvalidHostRequestEndpoint);
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
        encoder
            .finish()
            .map(|hash| TaskRequestTemplateDigest(*hash.as_bytes()))
            .map_err(Into::into)
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
