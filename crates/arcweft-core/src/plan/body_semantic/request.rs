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

/// Accepted semantic role bytes transported from the actual declaration/
/// schema owner. Debug spelling is never a constructor input.
pub(crate) struct RuntimeRequestRoleIdentity([u8; 32]);
impl RuntimeRequestRoleIdentity {
    pub(crate) const fn from_accepted_identity(identity: [u8; 32]) -> Self {
        Self(identity)
    }
}

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
                encoder.digest(&identity.0);
            }
            Self::Variant(identity) => {
                encoder.tag(3);
                encoder.digest(&identity.0);
            }
            Self::CallArgument(ordinal) => {
                encoder.tag(4);
                encoder.ordinal(*ordinal);
            }
            Self::NamedArgument(identity) => {
                encoder.tag(5);
                encoder.digest(&identity.0);
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
                encoder.digest(&identity.0);
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
            encoder.digest(&field.identity.0);
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
