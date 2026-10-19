//! A bounded relation between original declarations and actual AWBC type rows.
//! This borrows both existing graphs; it constructs neither a projected Source
//! nor another type authority. One memo spans all operands of an admitted row.

use crate::awbc::schema::{
    AwbcAgentTypeShape, AwbcProgram, AwbcRecordField, AwbcRuntimeTypeShape, AwbcStringId,
    AwbcTypeId, AwbcVariantCase, AwbcVariantIdentity,
};
use crate::entry::RuntimeCodecUse;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::{
    RuntimeAgentTypeProjection, RuntimePlan, RuntimePlanTypeProjection, RuntimeTaskPlanSealLimits,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError, TaskSemanticMeter};
use std::collections::BTreeSet;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAwbcSourceTypeField {
    Row,
    Identity,
    Scope,
    Declaration,
    Shape,
    Children,
    Field,
    Case,
    Contract,
    Codec,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeAwbcSourceTypeError {
    #[error("original type {original:?} and AWBC type {executable:?} differ in {field:?}")]
    Mismatch {
        original: RuntimePlanTypeId,
        executable: AwbcTypeId,
        field: RuntimeAwbcSourceTypeField,
    },
    #[error("original/AWBC type relation has {actual} children; limit is {maximum}")]
    Children { actual: usize, maximum: u32 },
    #[error("original/AWBC type relation exhausted its allowance: {0:?}")]
    Encoding(TaskSemanticEncodingError),
    #[error(transparent)]
    Policy(#[from] RuntimeBodySemanticError),
}

impl From<TaskSemanticEncodingError> for RuntimeAwbcSourceTypeError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

type Pair = (RuntimePlanTypeId, AwbcTypeId);

pub(super) struct RuntimeAwbcSourceTypeRelation<'program, 'work> {
    original: &'program RuntimePlan,
    executable: &'program AwbcProgram,
    limits: RuntimeTaskPlanSealLimits,
    meter: &'work mut TaskSemanticMeter,
    seen: BTreeSet<Pair>,
}

impl<'program, 'work> RuntimeAwbcSourceTypeRelation<'program, 'work> {
    pub(super) fn new(
        original: &'program RuntimePlan,
        executable: &'program AwbcProgram,
        limits: RuntimeTaskPlanSealLimits,
        meter: &'work mut TaskSemanticMeter,
    ) -> Self {
        Self {
            original,
            executable,
            limits,
            meter,
            seen: BTreeSet::new(),
        }
    }

    fn require(
        &mut self,
        pair: Pair,
        field: RuntimeAwbcSourceTypeField,
        equal: bool,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        self.meter.charge_work(1)?;
        if equal {
            Ok(())
        } else {
            Err(RuntimeAwbcSourceTypeError::Mismatch {
                original: pair.0,
                executable: pair.1,
                field,
            })
        }
    }

    fn count(&mut self, count: usize) -> Result<(), RuntimeAwbcSourceTypeError> {
        let count = self.meter.checked_count_sum(count, 0)?;
        if count > self.limits.max_children_per_row as usize {
            return Err(RuntimeAwbcSourceTypeError::Children {
                actual: count,
                maximum: self.limits.max_children_per_row,
            });
        }
        self.meter.charge_work(count as u64 + 1)?;
        Ok(())
    }

    fn text(
        &mut self,
        pair: Pair,
        expected: Option<&str>,
        actual: Option<AwbcStringId>,
        field: RuntimeAwbcSourceTypeField,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        let actual = match actual {
            Some(id) => Some(
                self.executable
                    .strings
                    .get(id.index())
                    .ok_or(RuntimeAwbcSourceTypeError::Mismatch {
                        original: pair.0,
                        executable: pair.1,
                        field: RuntimeAwbcSourceTypeField::Row,
                    })?
                    .as_str(),
            ),
            None => None,
        };
        if let (Some(expected), Some(actual)) = (expected, actual) {
            let count = self.meter.checked_count_sum(expected.len(), actual.len())?;
            self.meter.charge_work((count as u64).div_ceil(32))?;
        }
        self.require(pair, field, expected == actual)
    }

    fn children(
        &mut self,
        pair: Pair,
        expected: &[RuntimePlanTypeId],
        actual: &[AwbcTypeId],
        pending: &mut Vec<Pair>,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        self.count(expected.len())?;
        self.count(actual.len())?;
        self.require(
            pair,
            RuntimeAwbcSourceTypeField::Children,
            expected.len() == actual.len(),
        )?;
        pending.extend(expected.iter().copied().zip(actual.iter().copied()));
        Ok(())
    }

    fn optional_child(
        &mut self,
        pair: Pair,
        expected: Option<RuntimePlanTypeId>,
        actual: Option<AwbcTypeId>,
        pending: &mut Vec<Pair>,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        self.require(
            pair,
            RuntimeAwbcSourceTypeField::Children,
            expected.is_some() == actual.is_some(),
        )?;
        if let (Some(expected), Some(actual)) = (expected, actual) {
            self.count(1)?;
            pending.push((expected, actual));
        }
        Ok(())
    }

    fn policy(
        &mut self,
        pair: Pair,
        expected: Option<&RuntimeCodecUse>,
        actual: Option<&RuntimeCodecUse>,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        use RuntimeAwbcSourceTypeField::Codec;
        self.require(pair, Codec, expected.is_some() == actual.is_some())?;
        let (Some(expected), Some(actual)) = (expected, actual) else {
            return Ok(());
        };
        expected.try_visit_semantic_child_counts(&mut |count| self.count(count))?;
        actual.try_visit_semantic_child_counts(&mut |count| self.count(count))?;
        let context = RuntimeBodySemanticContext::new(self.original);
        let mut encoder = TaskSemanticEncoder::new(b"arcweft.type.source-codec.v1\0", self.meter);
        expected.encode_executable_policy(&context, &mut encoder)?;
        let expected = encoder.finish()?;
        let mut encoder = TaskSemanticEncoder::new(b"arcweft.type.source-codec.v1\0", self.meter);
        actual.encode_executable_policy(&context, &mut encoder)?;
        let actual = encoder.finish()?;
        self.require(pair, Codec, expected == actual)
    }

    pub(super) fn admit(
        &mut self,
        expected: RuntimePlanTypeId,
        actual: AwbcTypeId,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        let result = self.admit_type(expected, actual);
        if result.is_err() {
            self.meter.reject_owner();
        }
        result
    }

    fn admit_type(
        &mut self,
        expected: RuntimePlanTypeId,
        actual: AwbcTypeId,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        let original = self.original;
        let executable = self.executable;
        self.meter.status()?;
        self.meter.charge_work(1)?;
        let mut pending = vec![(expected, actual)];
        while let Some(pair) = pending.pop() {
            self.meter.charge_work(1)?;
            if self.seen.contains(&pair) {
                continue;
            }
            let source = original.type_table().get(pair.0);
            let code = executable.runtime_types.get(pair.1.index());
            self.require(
                pair,
                RuntimeAwbcSourceTypeField::Row,
                source.is_some() && code.is_some(),
            )?;
            let source = source.expect("the original row was checked");
            let code = code.expect("the executable row was checked");
            self.require(
                pair,
                RuntimeAwbcSourceTypeField::Identity,
                source.semantic_identity() == code.semantic_identity(),
            )?;
            source
                .scope()
                .try_visit_semantic_child_counts(&mut |count| self.count(count))?;
            code.scope()
                .try_visit_semantic_child_counts(&mut |count| self.count(count))?;
            self.require(
                pair,
                RuntimeAwbcSourceTypeField::Scope,
                source.scope() == code.scope(),
            )?;
            self.require(
                pair,
                RuntimeAwbcSourceTypeField::Declaration,
                source.nominal_declaration() == code.nominal_declaration(),
            )?;
            let nominal = original
                .nominal_record_domains()
                .get(pair.0)
                .and_then(|domain| domain.data_codec())
                .or_else(|| {
                    original
                        .variant_domains()
                        .get(pair.0)
                        .and_then(|domain| domain.data_codec())
                });
            let body = nominal
                .map(|codec| &codec.body)
                .or_else(|| source.data_codec());
            self.policy(pair, body, code.data_codec())?;
            let arguments = nominal.map(|codec| codec.arguments.as_ref());
            self.require(
                pair,
                RuntimeAwbcSourceTypeField::Codec,
                arguments.is_some() == code.data_codec_arguments().is_some(),
            )?;
            if let (Some(expected), Some(actual)) = (arguments, code.data_codec_arguments()) {
                self.count(expected.len())?;
                self.count(actual.len())?;
                self.require(
                    pair,
                    RuntimeAwbcSourceTypeField::Codec,
                    expected.len() == actual.len(),
                )?;
                for (expected, actual) in expected.iter().zip(actual) {
                    self.policy(pair, Some(expected), Some(actual))?;
                }
            }
            let children_start = pending.len();
            self.projection(pair, source.projection(), code.shape(), &mut pending)?;
            pending[children_start..].reverse();
            self.seen.insert(pair);
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive relation over the original closed type algebra, with borrowed child graphs"
    )]
    fn projection(
        &mut self,
        pair: Pair,
        source: &RuntimePlanTypeProjection<RuntimePlanTypeId>,
        code: &AwbcRuntimeTypeShape,
        pending: &mut Vec<Pair>,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        use AwbcRuntimeTypeShape as A;
        use RuntimeAwbcSourceTypeField as Field;
        use RuntimePlanTypeProjection as P;
        let equal = match (source, code) {
            (P::Never, A::Never)
            | (P::Unit, A::Unit)
            | (P::Bool, A::Bool)
            | (P::F32, A::F32)
            | (P::F64, A::F64)
            | (P::String, A::String)
            | (P::Color, A::Color)
            | (P::Char, A::Char)
            | (P::Bytes, A::Bytes)
            | (P::Duration, A::Duration)
            | (P::Progress, A::Progress)
            | (P::EntityReference, A::EntityRef)
            | (P::AgentValue, A::AgentValue) => true,
            (P::BoundType(expected), A::BoundType(actual)) => expected == actual,
            (P::Signed(expected), A::Int(actual)) => *expected == (*actual).into(),
            (P::Unsigned(expected), A::UInt(actual)) => *expected == (*actual).into(),
            (P::DenseNumeric(expected), actual) => A::from(*expected) == *actual,
            (P::Range(expected), A::Range(actual))
            | (P::Iterator(expected), A::Iterator(actual))
            | (P::Need(expected), A::Need(actual))
            | (P::ThreadHandle(expected), A::Task(actual))
            | (P::Shared(expected), A::Shared(actual))
            | (P::Reference(expected), A::Reference(actual)) => {
                self.children(pair, &[*expected], &[*actual], pending)?;
                true
            }
            (
                P::Sequence {
                    kind: expected_kind,
                    item: expected,
                },
                A::Sequence {
                    kind: actual_kind,
                    item: actual,
                },
            ) => {
                self.require(pair, Field::Shape, expected_kind == actual_kind)?;
                self.children(pair, &[*expected], &[*actual], pending)?;
                true
            }
            (
                P::Array {
                    item: expected,
                    length: expected_length,
                },
                A::Array {
                    item: actual,
                    length: actual_length,
                },
            ) => {
                self.require(pair, Field::Shape, expected_length == actual_length)?;
                self.children(pair, &[*expected], &[*actual], pending)?;
                true
            }
            (
                P::Map {
                    kind: expected_kind,
                    key,
                    value,
                },
                A::Map {
                    kind: actual_kind,
                    key: actual_key,
                    value: actual_value,
                },
            ) => {
                self.require(pair, Field::Shape, expected_kind == actual_kind)?;
                self.children(
                    pair,
                    &[*key, *value],
                    &[*actual_key, *actual_value],
                    pending,
                )?;
                true
            }
            (
                P::Stream { item, error },
                A::Stream {
                    item: actual_item,
                    error: actual_error,
                },
            ) => {
                self.children(
                    pair,
                    &[*item, *error],
                    &[*actual_item, *actual_error],
                    pending,
                )?;
                true
            }
            (P::Tuple(expected), A::Tuple(actual)) | (P::Choice(expected), A::Choice(actual)) => {
                self.children(pair, expected, actual, pending)?;
                true
            }
            (
                P::Function {
                    contract,
                    parameters,
                    result,
                },
                A::Function {
                    contract: actual_contract,
                    parameters: actual_parameters,
                    result: actual_result,
                },
            ) => {
                contract.try_visit_semantic_child_counts(&mut |count| self.count(count))?;
                actual_contract.try_visit_semantic_child_counts(&mut |count| self.count(count))?;
                self.require(pair, Field::Contract, contract == actual_contract)?;
                self.children(pair, parameters, actual_parameters, pending)?;
                self.children(pair, &[*result], &[*actual_result], pending)?;
                true
            }
            (P::Record(expected), A::Record { public_id, fields }) => {
                self.require(pair, Field::Shape, public_id.is_none())?;
                self.count(expected.len())?;
                self.count(fields.len())?;
                self.require(pair, Field::Children, expected.len() == fields.len())?;
                for (index, (expected, actual)) in expected.iter().zip(fields).enumerate() {
                    self.require(
                        pair,
                        Field::Field,
                        actual.field.zero_based() as usize == index,
                    )?;
                    self.text(
                        pair,
                        Some(expected.diagnostic_name()),
                        actual.name,
                        Field::Field,
                    )?;
                    self.children(pair, &[*expected.ty()], &[actual.ty], pending)?;
                }
                true
            }
            (
                P::Opaque {
                    producer,
                    admission,
                    value_class,
                    persistence,
                    arguments,
                },
                A::Opaque {
                    producer: actual_producer,
                    admission: actual_admission,
                    value_class: actual_class,
                    persistence: actual_persistence,
                    arguments: actual_arguments,
                },
            ) => {
                self.text(
                    pair,
                    Some(producer.as_str()),
                    Some(*actual_producer),
                    Field::Shape,
                )?;
                self.require(
                    pair,
                    Field::Shape,
                    admission == actual_admission
                        && value_class == actual_class
                        && persistence == actual_persistence,
                )?;
                self.children(pair, arguments, actual_arguments, pending)?;
                true
            }
            (P::Agent(agent), A::Agent(actual)) => match (agent, actual) {
                (
                    RuntimeAgentTypeProjection::Probe(expected),
                    AwbcAgentTypeShape::Probe(actual),
                )
                | (
                    RuntimeAgentTypeProjection::DataShape(expected),
                    AwbcAgentTypeShape::DataShape(actual),
                ) => {
                    self.children(pair, &[*expected], &[*actual], pending)?;
                    true
                }
                (expected, AwbcAgentTypeShape::Leaf(actual)) => {
                    expected.operational_type() == *actual
                }
                _ => false,
            },
            (
                P::Result {
                    value_payload,
                    error_payload,
                    ..
                },
                A::Variant {
                    owner,
                    arguments,
                    cases,
                },
            ) => {
                self.builtin(
                    pair,
                    crate::pattern::RuntimeBuiltinVariantIdentity::Result,
                    owner,
                    arguments,
                    cases,
                )?;
                self.optional_child(pair, Some(*value_payload), cases[0].payload, pending)?;
                self.optional_child(pair, Some(*error_payload), cases[1].payload, pending)?;
                true
            }
            (
                P::Option { some_payload, .. },
                A::Variant {
                    owner,
                    arguments,
                    cases,
                },
            ) => {
                self.builtin(
                    pair,
                    crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                    owner,
                    arguments,
                    cases,
                )?;
                self.optional_child(pair, Some(*some_payload), cases[0].payload, pending)?;
                self.optional_child(pair, None, cases[1].payload, pending)?;
                true
            }
            (
                P::BuiltinVariant {
                    owner,
                    cases: expected,
                },
                A::Variant {
                    owner: actual_owner,
                    arguments,
                    cases,
                },
            ) => {
                self.builtin(pair, *owner, actual_owner, arguments, cases)?;
                self.count(expected.len())?;
                self.require(pair, Field::Children, expected.len() == cases.len())?;
                for (expected, actual) in expected.iter().zip(cases) {
                    self.optional_child(pair, *expected, actual.payload, pending)?;
                }
                true
            }
            (
                P::Nominal {
                    nominal,
                    layout,
                    arguments,
                },
                actual,
            ) => {
                self.nominal(
                    pair,
                    nominal.as_str(),
                    layout.as_bytes(),
                    arguments,
                    actual,
                    pending,
                )?;
                true
            }
            _ => false,
        };
        self.require(pair, Field::Shape, equal)
    }

    fn builtin(
        &mut self,
        pair: Pair,
        expected: crate::pattern::RuntimeBuiltinVariantIdentity,
        actual: &AwbcVariantIdentity,
        arguments: &[AwbcTypeId],
        cases: &[AwbcVariantCase],
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        use RuntimeAwbcSourceTypeField as Field;
        self.count(arguments.len())?;
        self.count(cases.len())?;
        self.require(
            pair,
            Field::Shape,
            *actual == AwbcVariantIdentity::Builtin(expected) && arguments.is_empty(),
        )?;
        self.require(pair, Field::Children, expected.cases().len() == cases.len())?;
        for (expected, actual) in expected.cases().iter().zip(cases) {
            self.text(pair, Some(expected.name()), Some(actual.name), Field::Case)?;
        }
        Ok(())
    }

    fn nominal(
        &mut self,
        pair: Pair,
        nominal: &str,
        layout: &[u8; 32],
        arguments: &[RuntimePlanTypeId],
        code: &AwbcRuntimeTypeShape,
        pending: &mut Vec<Pair>,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        use RuntimeAwbcSourceTypeField as Field;
        let original = self.original;
        if let Some(domain) = original.nominal_record_domains().get(pair.0) {
            let AwbcRuntimeTypeShape::NominalRecord {
                public_id,
                layout: actual_layout,
                arguments: actual_arguments,
                shape,
                fields,
            } = code
            else {
                return self.require(pair, Field::Shape, false);
            };
            self.text(pair, Some(nominal), Some(*public_id), Field::Shape)?;
            self.require(
                pair,
                Field::Shape,
                layout == actual_layout && domain.shape() == *shape,
            )?;
            self.children(pair, arguments, actual_arguments, pending)?;
            self.nominal_fields(pair, domain.fields(), fields, pending)?;
        } else if let Some(domain) = original.variant_domains().get(pair.0) {
            let AwbcRuntimeTypeShape::Variant {
                owner:
                    AwbcVariantIdentity::Nominal {
                        public_id,
                        layout: actual_layout,
                    },
                arguments: actual_arguments,
                cases,
            } = code
            else {
                return self.require(pair, Field::Shape, false);
            };
            self.text(pair, Some(nominal), Some(*public_id), Field::Shape)?;
            self.require(pair, Field::Shape, layout == actual_layout)?;
            self.children(pair, arguments, actual_arguments, pending)?;
            self.count(domain.cases().len())?;
            self.count(cases.len())?;
            self.require(pair, Field::Children, domain.cases().len() == cases.len())?;
            for (expected, actual) in domain.cases().iter().zip(cases) {
                self.text(pair, Some(expected.name()), Some(actual.name), Field::Case)?;
                self.optional_child(pair, expected.payload(), actual.payload, pending)?;
            }
        } else {
            let AwbcRuntimeTypeShape::Nominal {
                public_id,
                layout: actual_layout,
                arguments: actual_arguments,
            } = code
            else {
                return self.require(pair, Field::Shape, false);
            };
            self.text(pair, Some(nominal), Some(*public_id), Field::Shape)?;
            self.require(pair, Field::Shape, layout == actual_layout)?;
            self.children(pair, arguments, actual_arguments, pending)?;
        }
        Ok(())
    }

    fn nominal_fields(
        &mut self,
        pair: Pair,
        expected: &[crate::plan::RuntimeNominalRecordDomainField],
        actual: &[AwbcRecordField],
        pending: &mut Vec<Pair>,
    ) -> Result<(), RuntimeAwbcSourceTypeError> {
        use RuntimeAwbcSourceTypeField as Field;
        self.count(expected.len())?;
        self.count(actual.len())?;
        self.require(pair, Field::Children, expected.len() == actual.len())?;
        for (expected, actual) in expected.iter().zip(actual) {
            self.require(pair, Field::Field, expected.field() == actual.field)?;
            self.text(pair, expected.name(), actual.name, Field::Field)?;
            self.children(pair, &[expected.ty()], &[actual.ty], pending)?;
        }
        Ok(())
    }
}
