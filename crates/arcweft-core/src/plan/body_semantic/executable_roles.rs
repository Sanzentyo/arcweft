//! Entry executable rows bind actual completed code proofs to accepted roles.
//! The target owns its ABI and body; this owner never walks that body again.

use super::function::ProducerFunctionSemantic;
use super::pure_rows::PureHelperSemantic;
use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::entry::{RuntimeCallableExecutableCode, RuntimeFlowParameterMode};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

/// The exhaustive semantic substrates of an admitted entry callable.
/// Each reference is an owner-issued proof, never caller-provided hash bytes.
pub(crate) enum ExecutableCallableCodeSemantic<'proof, 'plan> {
    PureHelper(&'proof PureHelperSemantic<'plan>),
    FunctionSite(&'proof ProducerFunctionSemantic<'plan>),
    ControllerFlow(&'proof ProducerFunctionSemantic<'plan>),
}

impl RuntimeBodySemanticContext<'_> {
    pub(crate) fn callable_executable_row_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        ordinal: usize,
        code: ExecutableCallableCodeSemantic<'_, '_>,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        let row = self
            .plan
            .callable_executables()
            .get(ordinal)
            .ok_or_else(|| {
                meter.reject_owner();
                RuntimeBodySemanticError::MissingRow {
                    table: "callable executables",
                    ordinal,
                }
            })?;
        let target = match (&row.code, code) {
            (
                RuntimeCallableExecutableCode::PureHelper(helper),
                ExecutableCallableCodeSemantic::PureHelper(proof),
            ) if proof.matches_helper(self, *helper) => *proof.digest().as_bytes(),
            (
                RuntimeCallableExecutableCode::FunctionSite(function),
                ExecutableCallableCodeSemantic::FunctionSite(proof),
            ) if proof.matches_function(self, *function) => *proof.digest().as_bytes(),
            (
                RuntimeCallableExecutableCode::ControllerFlow(flow),
                ExecutableCallableCodeSemantic::ControllerFlow(proof),
            ) if proof.matches_flow(self, flow) => *proof.digest().as_bytes(),
            _ => {
                meter.reject_owner();
                return Err(
                    RuntimeBodySemanticError::InvalidCallableExecutableProducer { ordinal },
                );
            }
        };
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(7);
        encoder.tag(row.code.semantic_tag());
        encoder.string(row.callable.as_str());
        encoder.digest(row.contract.as_bytes());
        // These entry-role substrates have ordinary formal inputs and no
        // implicit method receiver. Trait-method receivers belong to E11.
        encoder.tag(0);
        if let RuntimeCallableExecutableCode::ControllerFlow(flow) = &row.code {
            flow.write_executable_identity(&mut encoder);
        }
        encoder.digest(&target);
        encoder.finish().map_err(Into::into)
    }

    pub(crate) fn flow_executable_row_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        ordinal: usize,
        producer: &ProducerFunctionSemantic<'_>,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        let row = self.plan.flow_executables().get(ordinal).ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "Flow executables",
                ordinal,
            }
        })?;
        if !producer.matches_flow(self, &row.flow) {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::InvalidFlowExecutableProducer { ordinal });
        }
        let schema = self.plan.flows.schema(&row.flow).ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "Flow invocation schemas",
                ordinal,
            }
        })?;
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(8);
        encoder.tag(0);
        row.flow.write_executable_identity(&mut encoder);
        encoder.digest(row.contract.as_bytes());
        encoder.count(schema.parameters.len());
        for (ordinal, parameter) in schema.parameters.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.digest(parameter.identity.as_bytes());
            encoder.ordinal(parameter.coordinate.position());
            encoder.tag(parameter.mode.semantic_tag());
            encoder.tag(parameter.passing.semantic_tag());
            encoder.digest(parameter.semantic_identity.as_bytes());
        }
        // F commits the exact return type, checked invocation effects, input
        // patterns/captures and complete body of this same Flow definition.
        encoder.digest(producer.digest().as_bytes());
        encoder.tag(u8::from(row.controller.is_some()));
        if let Some(controller) = &row.controller {
            encoder.string(controller.callable.as_str());
            encoder.digest(controller.contract.as_bytes());
        }
        encoder.finish().map_err(Into::into)
    }
}

impl crate::plan::FlowRuntimeId {
    pub(super) fn write_executable_identity(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.count(self.path().segments().len());
        for segment in self.path().segments() {
            encoder.enter_element();
            encoder.string(segment.as_str());
        }
        encoder.string(self.public_label_ref().as_str());
    }
}

impl RuntimeCallableExecutableCode {
    pub(crate) const fn semantic_tag(&self) -> u8 {
        match self {
            Self::PureHelper(_) => 0,
            Self::FunctionSite(_) => 1,
            Self::ControllerFlow(_) => 2,
        }
    }
}

impl RuntimeFlowParameterMode {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Owned => 0,
            Self::Shared => 1,
            Self::Mutable => 2,
        }
    }
}

#[cfg(test)]
mod tests;
