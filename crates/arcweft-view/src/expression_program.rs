//! Retained expression programs use the ordinary executable value ABI.

use arcweft_id::{RuntimeSemanticTypeId, runtime_program::RuntimePureProgramId};
use serde::{Deserialize, Serialize};

use crate::{ViewParameterCoordinate, ViewParameterInput};

/// One output of an accepted binding program, separate from parameter ordinals.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewLocalCoordinate {
    pub program: RuntimePureProgramId,
    pub output: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewExecutionInputSource {
    Parameter(ViewParameterCoordinate),
    Local(ViewLocalCoordinate),
}

/// Canonical free input address and exact semantic type.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewExecutionInput {
    pub source: ViewExecutionInputSource,
    pub value_type: RuntimeSemanticTypeId,
}

impl ViewExecutionInput {
    pub const fn value_type(self) -> RuntimeSemanticTypeId {
        self.value_type
    }
    pub const fn parameter(self) -> Option<ViewParameterCoordinate> {
        match self.source {
            ViewExecutionInputSource::Parameter(parameter) => Some(parameter),
            ViewExecutionInputSource::Local(_) => None,
        }
    }
}

impl From<ViewParameterInput> for ViewExecutionInput {
    fn from(input: ViewParameterInput) -> Self {
        Self {
            source: ViewExecutionInputSource::Parameter(input.parameter()),
            value_type: input.value_type(),
        }
    }
}

/// A Core selector returns the selected arm ordinal and its owned binding tuple.
/// Output ordinals are unique across the complete source-ordered arm inventory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchProgram {
    pub execution: ViewExpressionProgram,
    pub arms: Box<[ViewMatchArm]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchArm {
    pub body_span: u32,
    pub outputs: Box<[ViewLocalOutput]>,
}

/// Checked contiguous Match arm ranges within the enclosing region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMatchRanges {
    arms: Box<[crate::ViewInstructionRange]>,
    continuation: u32,
}

impl ViewMatchProgram {
    pub fn outputs_are_canonical(&self) -> bool {
        let mut ordinal = 0usize;
        self.arms
            .iter()
            .flat_map(|arm| arm.outputs.iter())
            .all(|output| {
                let valid = output.coordinate.program == self.execution.program
                    && usize::from(output.coordinate.output) == ordinal;
                ordinal += 1;
                valid
            })
    }

    pub fn ranges(&self, instruction: u32, enclosing_end: u32) -> Option<ViewMatchRanges> {
        if !self.outputs_are_canonical() {
            return None;
        }
        let mut start = instruction.checked_add(1)?;
        if start > enclosing_end {
            return None;
        }
        let mut arms = Vec::with_capacity(self.arms.len());
        for arm in &self.arms {
            let end = start.checked_add(arm.body_span)?;
            if end > enclosing_end {
                return None;
            }
            arms.push(crate::ViewInstructionRange::new(start, end));
            start = end;
        }
        Some(ViewMatchRanges {
            arms: arms.into_boxed_slice(),
            continuation: start,
        })
    }
}

impl ViewMatchRanges {
    pub const fn arms(&self) -> &[crate::ViewInstructionRange] {
        &self.arms
    }
    pub const fn continuation(&self) -> u32 {
        self.continuation
    }
}

/// Owned binding outputs are transported by Core in this exact order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewBindingProgram {
    pub execution: ViewExpressionProgram,
    pub outputs: Box<[ViewLocalOutput]>,
}

impl ViewBindingProgram {
    pub fn outputs_are_canonical(&self) -> bool {
        self.outputs.iter().enumerate().all(|(index, output)| {
            output.coordinate.program == self.execution.program
                && usize::from(output.coordinate.output) == index
        })
    }
}

/// The source uses Core iteration and returns owned binding tuples. The key
/// is evaluated in each item's binding scope before any retained body runs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewRepeatProgram {
    pub source: ViewBindingProgram,
    pub key: ViewExpressionProgram,
    pub body_span: u32,
}

impl ViewRepeatProgram {
    pub fn body_range(
        &self,
        instruction: u32,
        enclosing_end: u32,
    ) -> Option<crate::ViewInstructionRange> {
        if !self.source.outputs_are_canonical() {
            return None;
        }
        let start = instruction.checked_add(1)?;
        let end = start.checked_add(self.body_span)?;
        (end <= enclosing_end).then(|| crate::ViewInstructionRange::new(start, end))
    }
}

/// A typed, canonical key value digest; no VM register or instruction offset
/// participates in the value identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewRepeatKey {
    value_type: RuntimeSemanticTypeId,
    digest: [u8; 32],
}

impl ViewRepeatKey {
    /// The caller must validate and hash the value with its admitted Core type.
    pub const fn from_checked_digest(value_type: RuntimeSemanticTypeId, digest: [u8; 32]) -> Self {
        Self { value_type, digest }
    }
    pub const fn value_type(&self) -> RuntimeSemanticTypeId {
        self.value_type
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn identity_bytes(&self) -> [u8; 64] {
        let mut bytes = [0; 64];
        bytes[..32].copy_from_slice(self.value_type.as_bytes());
        bytes[32..].copy_from_slice(&self.digest);
        bytes
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewLocalOutput {
    pub coordinate: ViewLocalCoordinate,
    pub value_type: RuntimeSemanticTypeId,
}

/// One expression's exact program identity, canonical free-input order and result type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewExpressionProgram {
    pub program: RuntimePureProgramId,
    pub inputs: Box<[ViewExecutionInput]>,
    pub result_type: RuntimeSemanticTypeId,
}

/// A declared constant or an ordinary pure expression producing a property.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ViewExpressionValue<T> {
    Constant { value: T },
    Program { program: ViewExpressionProgram },
}

impl<T> ViewExpressionValue<T> {
    pub const fn constant(value: T) -> Self {
        Self::Constant { value }
    }
    pub const fn program(&self) -> Option<&ViewExpressionProgram> {
        match self {
            Self::Constant { .. } => None,
            Self::Program { program } => Some(program),
        }
    }
}
