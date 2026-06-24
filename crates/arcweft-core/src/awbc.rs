//! AWBC executable table contracts and structural verifier.
//!
//! This module defines the Sans I/O core schema for the future product AWBC
//! executable-table payload. It intentionally does not replace the current
//! structured [`crate::bytecode::BytecodeProgram`] runtime path or product
//! payload until compact VM execution and migration rules are implemented.

use crate::bytecode::BYTECODE_ABI_VERSION;
use std::collections::BTreeSet;
use thiserror::Error;

/// ABI version accepted by the AWBC executable verifier.
pub const AWBC_ABI_VERSION: u32 = BYTECODE_ABI_VERSION;

/// Stable function identifier in an AWBC executable table.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcFunctionId(pub u32);

/// Stable basic-block identifier within one AWBC function.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcBlockId(pub u32);

/// Register identifier within one AWBC function frame.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcRegisterId(pub u32);

/// Runtime type table index.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcTypeId(pub u32);

/// Constant table index.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcConstantId(pub u32);

/// Host-call table index.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcHostCallId(pub u32);

/// Content-unit table index.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcContentUnitId(pub u32);

/// Stable effect/capability signature digest.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct AwbcEffectDigest(pub [u8; 32]);

/// AWBC executable table.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcProgram {
    pub abi_version: u32,
    pub runtime_types: Vec<AwbcRuntimeType>,
    pub constants: Vec<AwbcConstant>,
    pub host_calls: Vec<AwbcHostCall>,
    pub content_units: Vec<AwbcContentUnit>,
    pub functions: Vec<AwbcFunction>,
    pub entries: Vec<AwbcEntry>,
}

/// One executable AWBC function.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcFunction {
    pub id: AwbcFunctionId,
    pub signature: AwbcSignature,
    pub frame: AwbcFrameLayout,
    pub blocks: Vec<AwbcBlock>,
}

/// One AWBC basic block.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcBlock {
    pub id: AwbcBlockId,
    pub instructions: Vec<AwbcInstruction>,
    pub terminator: AwbcTerminator,
}

/// Callable signature attached to an AWBC function or host call.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcSignature {
    pub params: Vec<AwbcTypeId>,
    pub result: Option<AwbcTypeId>,
    pub effects: AwbcEffectDigest,
}

/// Register frame layout for a function.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcFrameLayout {
    pub registers: Vec<AwbcTypeId>,
}

/// Runtime type records referenced by signatures, frames, and host calls.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", content = "fields", rename_all = "snake_case")]
pub enum AwbcRuntimeType {
    Unit,
    Bool,
    I64,
    F64,
    String,
    EntityRef,
    Record(Vec<AwbcTypeId>),
    Variant(Vec<Option<AwbcTypeId>>),
}

/// Constant table entries for executable AWBC instructions.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AwbcConstant {
    Unit,
    Bool(bool),
    I64(i64),
    String(String),
    EntityRef(String),
}

/// Host-call ABI entry.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcHostCall {
    pub name: String,
    pub params: Vec<AwbcTypeId>,
    pub result: Option<AwbcTypeId>,
    pub effects: AwbcEffectDigest,
}

/// Content unit required by an executable program.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcContentUnit {
    pub public_id: String,
}

/// Public entrypoint into an AWBC program.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwbcEntry {
    pub public_id: String,
    pub target: AwbcFunctionId,
}

/// Reserved v1 opcode names for the executable-table schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AwbcOpcode {
    Nop,
    LoadConst,
    LoadLocal,
    StoreLocal,
    Move,
    Drop,
    EnterScope,
    ExitScope,
    MakeTuple,
    MakeRecord,
    MakeVariant,
    ProjectTuple,
    ProjectRecord,
    Unary,
    Binary,
    Compare,
    CallFunction,
    CallPureHelper,
    CallExtern,
    EmitEffect,
    StartTask,
    AwaitTask,
    AwaitMany,
    Dialogue,
    Choice,
    EnsureContent,
    Jump,
    Branch,
    MatchVariant,
    MatchLiteral,
    LoopBackedge,
    GotoFlow,
    GotoDynamic,
    Return,
    Trap,
}

/// Typed AWBC instruction variants with table/register operands.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "instruction", rename_all = "snake_case")]
pub enum AwbcInstruction {
    Nop,
    LoadConst {
        dst: AwbcRegisterId,
        constant: AwbcConstantId,
    },
    Move {
        dst: AwbcRegisterId,
        src: AwbcRegisterId,
    },
    Unary {
        dst: AwbcRegisterId,
        op: AwbcUnaryOp,
        src: AwbcRegisterId,
    },
    Binary {
        dst: AwbcRegisterId,
        op: AwbcBinaryOp,
        lhs: AwbcRegisterId,
        rhs: AwbcRegisterId,
    },
    CallFunction {
        dst: Option<AwbcRegisterId>,
        function: AwbcFunctionId,
        args: Vec<AwbcRegisterId>,
    },
    CallExtern {
        dst: Option<AwbcRegisterId>,
        call: AwbcHostCallId,
        args: Vec<AwbcRegisterId>,
    },
    Dialogue {
        task_group: u32,
    },
    Choice {
        choice_id: String,
    },
    EnsureContent {
        content: AwbcContentUnitId,
    },
}

/// Unary operation encoded in an AWBC instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AwbcUnaryOp {
    Not,
    Neg,
}

/// Binary operation encoded in an AWBC instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AwbcBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Block terminator. Every AWBC block must end in one terminator.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "terminator", rename_all = "snake_case")]
pub enum AwbcTerminator {
    Jump(AwbcBlockId),
    Branch {
        condition: AwbcRegisterId,
        then_block: AwbcBlockId,
        else_block: AwbcBlockId,
    },
    Goto(AwbcFunctionId),
    Return(Option<AwbcRegisterId>),
    Suspend {
        kind: AwbcSuspendKind,
        resume: AwbcBlockId,
    },
    Trap(AwbcTrapKind),
}

/// Runtime suspension category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AwbcSuspendKind {
    Dialogue,
    Choice,
    Await,
    AwaitMany,
    HostRequest,
    StepBudget,
}

/// Runtime trap category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AwbcTrapKind {
    TypeMismatch,
    InvalidIndex,
    DivisionByZero,
    Panic,
}

/// Structural verifier budgets for AWBC executable tables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AwbcVerifierBudget {
    pub runtime_types: usize,
    pub constants: usize,
    pub host_calls: usize,
    pub content_units: usize,
    pub functions: usize,
    pub entries: usize,
    pub blocks: usize,
    pub instructions: usize,
    pub registers: usize,
    pub operands: usize,
}

/// AWBC structural verifier error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwbcVerifyError {
    #[error("unsupported AWBC ABI version {actual}; expected {expected}")]
    UnsupportedAbi { actual: u32, expected: u32 },
    #[error("AWBC exceeds verifier budget `{0}`")]
    BudgetExceeded(&'static str),
    #[error("duplicate AWBC function id {0:?}")]
    DuplicateFunction(AwbcFunctionId),
    #[error("duplicate AWBC block id {function:?}:{block:?}")]
    DuplicateBlock {
        function: AwbcFunctionId,
        block: AwbcBlockId,
    },
    #[error("AWBC type id {0:?} is out of bounds")]
    TypeOutOfBounds(AwbcTypeId),
    #[error("AWBC register id {register:?} is out of bounds in function {function:?}")]
    RegisterOutOfBounds {
        function: AwbcFunctionId,
        register: AwbcRegisterId,
    },
    #[error("AWBC function id {0:?} is out of bounds")]
    FunctionOutOfBounds(AwbcFunctionId),
    #[error("AWBC block id {function:?}:{block:?} is out of bounds")]
    BlockOutOfBounds {
        function: AwbcFunctionId,
        block: AwbcBlockId,
    },
    #[error("AWBC constant id {0:?} is out of bounds")]
    ConstantOutOfBounds(AwbcConstantId),
    #[error("AWBC host call id {0:?} is out of bounds")]
    HostCallOutOfBounds(AwbcHostCallId),
    #[error("AWBC content unit id {0:?} is out of bounds")]
    ContentUnitOutOfBounds(AwbcContentUnitId),
}

impl Default for AwbcVerifierBudget {
    fn default() -> Self {
        Self {
            runtime_types: 262_144,
            constants: 1_000_000,
            host_calls: 262_144,
            content_units: 1_000_000,
            functions: 262_144,
            entries: 262_144,
            blocks: 1_000_000,
            instructions: 8_000_000,
            registers: 65_536,
            operands: 16_000_000,
        }
    }
}

impl Default for AwbcProgram {
    fn default() -> Self {
        Self {
            abi_version: AWBC_ABI_VERSION,
            runtime_types: vec![AwbcRuntimeType::Unit],
            constants: Vec::new(),
            host_calls: Vec::new(),
            content_units: Vec::new(),
            functions: Vec::new(),
            entries: Vec::new(),
        }
    }
}

impl AwbcProgram {
    /// Verifies top-level AWBC table shape and typed cross references.
    pub fn verify(&self, budget: AwbcVerifierBudget) -> Result<(), AwbcVerifyError> {
        if self.abi_version != AWBC_ABI_VERSION {
            return Err(AwbcVerifyError::UnsupportedAbi {
                actual: self.abi_version,
                expected: AWBC_ABI_VERSION,
            });
        }
        self.check_top_level_budgets(budget)?;
        self.verify_runtime_types()?;
        self.verify_host_calls()?;
        let mut seen = BTreeSet::new();
        for function in &self.functions {
            if !seen.insert(function.id) {
                return Err(AwbcVerifyError::DuplicateFunction(function.id));
            }
            self.verify_function(function)?;
        }
        self.entries
            .iter()
            .try_for_each(|entry| self.check_function(entry.target))
    }

    fn check_top_level_budgets(&self, budget: AwbcVerifierBudget) -> Result<(), AwbcVerifyError> {
        check_budget(
            self.runtime_types.len(),
            budget.runtime_types,
            "runtime_types",
        )?;
        check_budget(self.constants.len(), budget.constants, "constants")?;
        check_budget(self.host_calls.len(), budget.host_calls, "host_calls")?;
        check_budget(
            self.content_units.len(),
            budget.content_units,
            "content_units",
        )?;
        check_budget(self.functions.len(), budget.functions, "functions")?;
        check_budget(self.entries.len(), budget.entries, "entries")?;

        let blocks = self
            .functions
            .iter()
            .map(|function| function.blocks.len())
            .sum::<usize>();
        check_budget(blocks, budget.blocks, "blocks")?;

        let instructions = self
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter())
            .map(|block| block.instructions.len())
            .sum::<usize>();
        check_budget(instructions, budget.instructions, "instructions")?;

        let registers = self
            .functions
            .iter()
            .map(|function| function.frame.registers.len())
            .max()
            .unwrap_or(0);
        check_budget(registers, budget.registers, "registers")?;

        let operands = self.operand_count();
        check_budget(operands, budget.operands, "operands")
    }

    fn operand_count(&self) -> usize {
        let runtime_type_operands = self
            .runtime_types
            .iter()
            .map(|ty| match ty {
                AwbcRuntimeType::Record(fields) => fields.len(),
                AwbcRuntimeType::Variant(fields) => fields.len(),
                AwbcRuntimeType::Unit
                | AwbcRuntimeType::Bool
                | AwbcRuntimeType::I64
                | AwbcRuntimeType::F64
                | AwbcRuntimeType::String
                | AwbcRuntimeType::EntityRef => 0,
            })
            .sum::<usize>();
        let host_call_operands = self
            .host_calls
            .iter()
            .map(|call| call.params.len() + usize::from(call.result.is_some()))
            .sum::<usize>();
        let function_operands = self
            .functions
            .iter()
            .map(|function| {
                function.signature.params.len()
                    + usize::from(function.signature.result.is_some())
                    + function.frame.registers.len()
                    + function
                        .blocks
                        .iter()
                        .flat_map(|block| block.instructions.iter())
                        .map(AwbcInstruction::operand_count)
                        .sum::<usize>()
            })
            .sum::<usize>();
        runtime_type_operands + host_call_operands + function_operands
    }

    fn verify_runtime_types(&self) -> Result<(), AwbcVerifyError> {
        self.runtime_types
            .iter()
            .try_for_each(|runtime_type| match runtime_type {
                AwbcRuntimeType::Record(fields) => fields
                    .iter()
                    .copied()
                    .try_for_each(|ty| self.check_type(ty)),
                AwbcRuntimeType::Variant(fields) => fields
                    .iter()
                    .copied()
                    .flatten()
                    .try_for_each(|ty| self.check_type(ty)),
                AwbcRuntimeType::Unit
                | AwbcRuntimeType::Bool
                | AwbcRuntimeType::I64
                | AwbcRuntimeType::F64
                | AwbcRuntimeType::String
                | AwbcRuntimeType::EntityRef => Ok(()),
            })
    }

    fn verify_host_calls(&self) -> Result<(), AwbcVerifyError> {
        self.host_calls.iter().try_for_each(|call| {
            call.params
                .iter()
                .copied()
                .chain(call.result)
                .try_for_each(|ty| self.check_type(ty))
        })
    }

    fn verify_function(&self, function: &AwbcFunction) -> Result<(), AwbcVerifyError> {
        function
            .signature
            .params
            .iter()
            .copied()
            .chain(function.signature.result)
            .try_for_each(|ty| self.check_type(ty))?;
        function
            .frame
            .registers
            .iter()
            .copied()
            .try_for_each(|ty| self.check_type(ty))?;

        let mut blocks = BTreeSet::new();
        for block in &function.blocks {
            if !blocks.insert(block.id) {
                return Err(AwbcVerifyError::DuplicateBlock {
                    function: function.id,
                    block: block.id,
                });
            }
        }
        function
            .blocks
            .iter()
            .try_for_each(|block| self.verify_block(function, block))
    }

    fn verify_block(
        &self,
        function: &AwbcFunction,
        block: &AwbcBlock,
    ) -> Result<(), AwbcVerifyError> {
        block
            .instructions
            .iter()
            .try_for_each(|instruction| self.verify_instruction(function, instruction))?;
        self.verify_terminator(function, &block.terminator)
    }

    fn verify_instruction(
        &self,
        function: &AwbcFunction,
        instruction: &AwbcInstruction,
    ) -> Result<(), AwbcVerifyError> {
        match instruction {
            AwbcInstruction::Nop
            | AwbcInstruction::Dialogue { .. }
            | AwbcInstruction::Choice { .. } => Ok(()),
            AwbcInstruction::LoadConst { dst, constant } => {
                Self::check_register(function, *dst)?;
                self.check_constant(*constant)
            }
            AwbcInstruction::Move { dst, src } | AwbcInstruction::Unary { dst, src, .. } => {
                Self::check_registers(function, [*dst, *src])
            }
            AwbcInstruction::Binary { dst, lhs, rhs, .. } => {
                Self::check_registers(function, [*dst, *lhs, *rhs])
            }
            AwbcInstruction::CallFunction {
                dst,
                function: callee,
                args,
            } => {
                Self::check_optional_register(function, *dst)?;
                args.iter()
                    .copied()
                    .try_for_each(|arg| Self::check_register(function, arg))?;
                self.check_function(*callee)
            }
            AwbcInstruction::CallExtern { dst, call, args } => {
                Self::check_optional_register(function, *dst)?;
                args.iter()
                    .copied()
                    .try_for_each(|arg| Self::check_register(function, arg))?;
                self.check_host_call(*call)
            }
            AwbcInstruction::EnsureContent { content } => self.check_content_unit(*content),
        }
    }

    fn verify_terminator(
        &self,
        function: &AwbcFunction,
        terminator: &AwbcTerminator,
    ) -> Result<(), AwbcVerifyError> {
        match terminator {
            AwbcTerminator::Jump(block) => Self::check_block(function, *block),
            AwbcTerminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                Self::check_register(function, *condition)?;
                Self::check_block(function, *then_block)?;
                Self::check_block(function, *else_block)
            }
            AwbcTerminator::Goto(target) => self.check_function(*target),
            AwbcTerminator::Return(register) => Self::check_optional_register(function, *register),
            AwbcTerminator::Suspend { resume, .. } => Self::check_block(function, *resume),
            AwbcTerminator::Trap(_) => Ok(()),
        }
    }

    fn check_type(&self, id: AwbcTypeId) -> Result<(), AwbcVerifyError> {
        if usize::try_from(id.0).is_ok_and(|index| index < self.runtime_types.len()) {
            Ok(())
        } else {
            Err(AwbcVerifyError::TypeOutOfBounds(id))
        }
    }

    fn check_function(&self, id: AwbcFunctionId) -> Result<(), AwbcVerifyError> {
        if self.functions.iter().any(|function| function.id == id) {
            Ok(())
        } else {
            Err(AwbcVerifyError::FunctionOutOfBounds(id))
        }
    }

    fn check_block(function: &AwbcFunction, id: AwbcBlockId) -> Result<(), AwbcVerifyError> {
        if function.blocks.iter().any(|block| block.id == id) {
            Ok(())
        } else {
            Err(AwbcVerifyError::BlockOutOfBounds {
                function: function.id,
                block: id,
            })
        }
    }

    fn check_register(function: &AwbcFunction, id: AwbcRegisterId) -> Result<(), AwbcVerifyError> {
        if usize::try_from(id.0).is_ok_and(|index| index < function.frame.registers.len()) {
            Ok(())
        } else {
            Err(AwbcVerifyError::RegisterOutOfBounds {
                function: function.id,
                register: id,
            })
        }
    }

    fn check_optional_register(
        function: &AwbcFunction,
        id: Option<AwbcRegisterId>,
    ) -> Result<(), AwbcVerifyError> {
        id.map_or(Ok(()), |id| Self::check_register(function, id))
    }

    fn check_registers(
        function: &AwbcFunction,
        registers: impl IntoIterator<Item = AwbcRegisterId>,
    ) -> Result<(), AwbcVerifyError> {
        registers
            .into_iter()
            .try_for_each(|register| Self::check_register(function, register))
    }

    fn check_constant(&self, id: AwbcConstantId) -> Result<(), AwbcVerifyError> {
        if usize::try_from(id.0).is_ok_and(|index| index < self.constants.len()) {
            Ok(())
        } else {
            Err(AwbcVerifyError::ConstantOutOfBounds(id))
        }
    }

    fn check_host_call(&self, id: AwbcHostCallId) -> Result<(), AwbcVerifyError> {
        if usize::try_from(id.0).is_ok_and(|index| index < self.host_calls.len()) {
            Ok(())
        } else {
            Err(AwbcVerifyError::HostCallOutOfBounds(id))
        }
    }

    fn check_content_unit(&self, id: AwbcContentUnitId) -> Result<(), AwbcVerifyError> {
        if usize::try_from(id.0).is_ok_and(|index| index < self.content_units.len()) {
            Ok(())
        } else {
            Err(AwbcVerifyError::ContentUnitOutOfBounds(id))
        }
    }
}

impl AwbcInstruction {
    fn operand_count(&self) -> usize {
        match self {
            Self::Nop | Self::Dialogue { .. } | Self::Choice { .. } => 0,
            Self::LoadConst { .. } | Self::Move { .. } | Self::Unary { .. } => 2,
            Self::Binary { .. } => 3,
            Self::CallFunction { args, dst, .. } | Self::CallExtern { args, dst, .. } => {
                args.len() + usize::from(dst.is_some()) + 1
            }
            Self::EnsureContent { .. } => 1,
        }
    }
}

fn check_budget(actual: usize, budget: usize, name: &'static str) -> Result<(), AwbcVerifyError> {
    if actual > budget {
        Err(AwbcVerifyError::BudgetExceeded(name))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program() -> AwbcProgram {
        AwbcProgram {
            constants: vec![AwbcConstant::I64(7)],
            content_units: vec![AwbcContentUnit {
                public_id: "content.intro".to_owned(),
            }],
            functions: vec![AwbcFunction {
                id: AwbcFunctionId(0),
                signature: AwbcSignature::default(),
                frame: AwbcFrameLayout {
                    registers: vec![AwbcTypeId(0)],
                },
                blocks: vec![AwbcBlock {
                    id: AwbcBlockId(0),
                    instructions: vec![AwbcInstruction::LoadConst {
                        dst: AwbcRegisterId(0),
                        constant: AwbcConstantId(0),
                    }],
                    terminator: AwbcTerminator::Return(Some(AwbcRegisterId(0))),
                }],
            }],
            entries: vec![AwbcEntry {
                public_id: "main".to_owned(),
                target: AwbcFunctionId(0),
            }],
            ..AwbcProgram::default()
        }
    }

    #[test]
    fn awbc_executable_program_validates_well_formed_tables() {
        program()
            .verify(AwbcVerifierBudget::default())
            .expect("AWBC program verifies");
    }

    #[test]
    fn awbc_executable_program_rejects_unsupported_abi() {
        let mut program = program();
        program.abi_version = AWBC_ABI_VERSION + 1;

        assert_eq!(
            program.verify(AwbcVerifierBudget::default()),
            Err(AwbcVerifyError::UnsupportedAbi {
                actual: AWBC_ABI_VERSION + 1,
                expected: AWBC_ABI_VERSION,
            })
        );
    }

    #[test]
    fn awbc_executable_program_rejects_duplicate_function_and_block_ids() {
        let mut duplicate_function = program();
        duplicate_function
            .functions
            .push(duplicate_function.functions[0].clone());
        assert_eq!(
            duplicate_function.verify(AwbcVerifierBudget::default()),
            Err(AwbcVerifyError::DuplicateFunction(AwbcFunctionId(0)))
        );

        let mut duplicate_block = program();
        let duplicate = duplicate_block.functions[0].blocks[0].clone();
        duplicate_block.functions[0].blocks.push(duplicate);
        assert_eq!(
            duplicate_block.verify(AwbcVerifierBudget::default()),
            Err(AwbcVerifyError::DuplicateBlock {
                function: AwbcFunctionId(0),
                block: AwbcBlockId(0),
            })
        );
    }

    #[test]
    fn awbc_executable_program_rejects_out_of_bounds_references() {
        let cases = [
            (
                AwbcInstruction::LoadConst {
                    dst: AwbcRegisterId(0),
                    constant: AwbcConstantId(1),
                },
                AwbcVerifyError::ConstantOutOfBounds(AwbcConstantId(1)),
            ),
            (
                AwbcInstruction::Move {
                    dst: AwbcRegisterId(0),
                    src: AwbcRegisterId(1),
                },
                AwbcVerifyError::RegisterOutOfBounds {
                    function: AwbcFunctionId(0),
                    register: AwbcRegisterId(1),
                },
            ),
            (
                AwbcInstruction::CallFunction {
                    dst: None,
                    function: AwbcFunctionId(1),
                    args: Vec::new(),
                },
                AwbcVerifyError::FunctionOutOfBounds(AwbcFunctionId(1)),
            ),
            (
                AwbcInstruction::EnsureContent {
                    content: AwbcContentUnitId(1),
                },
                AwbcVerifyError::ContentUnitOutOfBounds(AwbcContentUnitId(1)),
            ),
        ];

        for (instruction, expected) in cases {
            let mut program = program();
            program.functions[0].blocks[0].instructions = vec![instruction];

            assert_eq!(program.verify(AwbcVerifierBudget::default()), Err(expected));
        }
    }

    #[test]
    fn awbc_executable_program_rejects_branch_to_missing_block() {
        let mut program = program();
        program.functions[0].blocks[0].terminator = AwbcTerminator::Branch {
            condition: AwbcRegisterId(0),
            then_block: AwbcBlockId(0),
            else_block: AwbcBlockId(1),
        };

        assert_eq!(
            program.verify(AwbcVerifierBudget::default()),
            Err(AwbcVerifyError::BlockOutOfBounds {
                function: AwbcFunctionId(0),
                block: AwbcBlockId(1),
            })
        );
    }

    #[test]
    fn awbc_executable_program_rejects_runtime_type_and_host_call_bounds() {
        let mut bad_runtime_type = program();
        bad_runtime_type
            .runtime_types
            .push(AwbcRuntimeType::Record(vec![AwbcTypeId(2)]));
        assert_eq!(
            bad_runtime_type.verify(AwbcVerifierBudget::default()),
            Err(AwbcVerifyError::TypeOutOfBounds(AwbcTypeId(2)))
        );

        let mut bad_host_call = program();
        bad_host_call.host_calls.push(AwbcHostCall {
            name: "host.bad".to_owned(),
            params: vec![AwbcTypeId(1)],
            result: None,
            effects: AwbcEffectDigest::default(),
        });
        assert_eq!(
            bad_host_call.verify(AwbcVerifierBudget::default()),
            Err(AwbcVerifyError::TypeOutOfBounds(AwbcTypeId(1)))
        );
    }

    #[test]
    fn awbc_executable_program_enforces_budgets() {
        let program = program();

        assert_eq!(
            program.verify(AwbcVerifierBudget {
                instructions: 0,
                ..AwbcVerifierBudget::default()
            }),
            Err(AwbcVerifyError::BudgetExceeded("instructions"))
        );
        assert_eq!(
            program.verify(AwbcVerifierBudget {
                registers: 0,
                ..AwbcVerifierBudget::default()
            }),
            Err(AwbcVerifyError::BudgetExceeded("registers"))
        );
    }
}
