//! Compact bytecode table model and structural verifier.

use crate::bytecode::BYTECODE_ABI_VERSION;
use std::collections::BTreeSet;
use thiserror::Error;

/// ABI version accepted by the compact bytecode verifier.
pub const COMPACT_BYTECODE_ABI_VERSION: u32 = BYTECODE_ABI_VERSION;

/// Stable numeric code slot identifier in a compact program.
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
pub struct CompactCodeSlotId(pub u32);

/// Stable numeric runtime type identifier in a compact program.
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
pub struct CompactRuntimeTypeId(pub u32);

/// Compact effect signature digest.
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
pub struct CompactEffectDigest(pub [u8; 32]);

/// Core compact bytecode opcodes whose operands index bounded tables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactOpcode {
    Return,
    Constant,
    Call,
    Jump,
    EnsureContent,
}

/// Runtime signature attached to one compact code slot.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CompactRuntimeSignature {
    pub params: Vec<CompactRuntimeTypeId>,
    pub result: CompactRuntimeTypeId,
    pub effects: CompactEffectDigest,
}

/// One compact instruction. The raw opcode is retained so unknown opcodes are
/// diagnosed instead of silently mapped to a placeholder.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CompactInstruction {
    pub opcode: u8,
    pub operand: u32,
}

/// One compact function body identified by a code slot.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CompactBytecodeFunction {
    pub slot: CompactCodeSlotId,
    pub signature: CompactRuntimeSignature,
    pub instructions: Vec<CompactInstruction>,
}

/// Compact bytecode program with bounded table counts and function bodies.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CompactBytecodeProgram {
    pub abi_version: u32,
    pub runtime_type_count: u32,
    pub constant_count: u32,
    pub content_unit_count: u32,
    pub functions: Vec<CompactBytecodeFunction>,
}

/// Validation budget for compact bytecode structural checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactBytecodeValidationBudget {
    pub functions: usize,
    pub instructions: usize,
    pub signature_params: usize,
}

/// Compact bytecode structural validation error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CompactBytecodeValidationError {
    #[error("unsupported compact bytecode ABI version {actual}; expected {expected}")]
    UnsupportedAbi { actual: u32, expected: u32 },
    #[error("compact bytecode duplicate code slot {0:?}")]
    DuplicateCodeSlot(CompactCodeSlotId),
    #[error("compact bytecode unknown opcode {opcode} at slot {slot:?} instruction {instruction}")]
    UnknownOpcode {
        slot: CompactCodeSlotId,
        instruction: usize,
        opcode: u8,
    },
    #[error("compact bytecode constant index {index} out of bounds at slot {slot:?}")]
    ConstantOutOfBounds { slot: CompactCodeSlotId, index: u32 },
    #[error("compact bytecode code slot index {index} out of bounds at slot {slot:?}")]
    CodeSlotOutOfBounds { slot: CompactCodeSlotId, index: u32 },
    #[error("compact bytecode jump target {index} out of bounds at slot {slot:?}")]
    JumpOutOfBounds { slot: CompactCodeSlotId, index: u32 },
    #[error("compact bytecode content unit index {index} out of bounds at slot {slot:?}")]
    ContentUnitOutOfBounds { slot: CompactCodeSlotId, index: u32 },
    #[error("compact bytecode runtime type index {index} out of bounds at slot {slot:?}")]
    RuntimeTypeOutOfBounds { slot: CompactCodeSlotId, index: u32 },
    #[error("compact bytecode exceeds validation budget `{budget}`")]
    BudgetExceeded { budget: &'static str },
}

impl CompactOpcode {
    pub const fn encoded(self) -> u8 {
        match self {
            Self::Return => 0,
            Self::Constant => 1,
            Self::Call => 2,
            Self::Jump => 3,
            Self::EnsureContent => 4,
        }
    }

    pub const fn from_encoded(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Return),
            1 => Some(Self::Constant),
            2 => Some(Self::Call),
            3 => Some(Self::Jump),
            4 => Some(Self::EnsureContent),
            _ => None,
        }
    }
}

impl Default for CompactRuntimeSignature {
    fn default() -> Self {
        Self {
            params: Vec::new(),
            result: CompactRuntimeTypeId(0),
            effects: CompactEffectDigest::default(),
        }
    }
}

impl Default for CompactBytecodeProgram {
    fn default() -> Self {
        Self {
            abi_version: COMPACT_BYTECODE_ABI_VERSION,
            runtime_type_count: 1,
            constant_count: 0,
            content_unit_count: 0,
            functions: Vec::new(),
        }
    }
}

impl Default for CompactBytecodeValidationBudget {
    fn default() -> Self {
        Self {
            functions: 262_144,
            instructions: 1_000_000,
            signature_params: 256,
        }
    }
}

impl CompactBytecodeProgram {
    pub fn verify(
        &self,
        budget: CompactBytecodeValidationBudget,
    ) -> Result<(), CompactBytecodeValidationError> {
        if self.abi_version != COMPACT_BYTECODE_ABI_VERSION {
            return Err(CompactBytecodeValidationError::UnsupportedAbi {
                actual: self.abi_version,
                expected: COMPACT_BYTECODE_ABI_VERSION,
            });
        }
        if self.functions.len() > budget.functions {
            return Err(CompactBytecodeValidationError::BudgetExceeded {
                budget: "functions",
            });
        }
        let function_count = u32::try_from(self.functions.len()).unwrap_or(u32::MAX);
        let mut slots = BTreeSet::new();
        let mut instruction_total = 0_usize;
        for function in &self.functions {
            if !slots.insert(function.slot) {
                return Err(CompactBytecodeValidationError::DuplicateCodeSlot(
                    function.slot,
                ));
            }
            verify_signature(function, self.runtime_type_count, budget.signature_params)?;
            instruction_total = instruction_total.saturating_add(function.instructions.len());
            if instruction_total > budget.instructions {
                return Err(CompactBytecodeValidationError::BudgetExceeded {
                    budget: "instructions",
                });
            }
            verify_instructions(function, self, function_count)?;
        }
        Ok(())
    }
}

fn verify_signature(
    function: &CompactBytecodeFunction,
    runtime_type_count: u32,
    signature_param_budget: usize,
) -> Result<(), CompactBytecodeValidationError> {
    if function.signature.params.len() > signature_param_budget {
        return Err(CompactBytecodeValidationError::BudgetExceeded {
            budget: "signature_params",
        });
    }
    function
        .signature
        .params
        .iter()
        .copied()
        .chain([function.signature.result])
        .try_for_each(|ty| {
            if ty.0 < runtime_type_count {
                Ok(())
            } else {
                Err(CompactBytecodeValidationError::RuntimeTypeOutOfBounds {
                    slot: function.slot,
                    index: ty.0,
                })
            }
        })
}

fn verify_instructions(
    function: &CompactBytecodeFunction,
    program: &CompactBytecodeProgram,
    function_count: u32,
) -> Result<(), CompactBytecodeValidationError> {
    let instruction_count = u32::try_from(function.instructions.len()).unwrap_or(u32::MAX);
    for (index, instruction) in function.instructions.iter().enumerate() {
        let Some(opcode) = CompactOpcode::from_encoded(instruction.opcode) else {
            return Err(CompactBytecodeValidationError::UnknownOpcode {
                slot: function.slot,
                instruction: index,
                opcode: instruction.opcode,
            });
        };
        match opcode {
            CompactOpcode::Return => {}
            CompactOpcode::Constant => {
                if instruction.operand >= program.constant_count {
                    return Err(CompactBytecodeValidationError::ConstantOutOfBounds {
                        slot: function.slot,
                        index: instruction.operand,
                    });
                }
            }
            CompactOpcode::Call => {
                if instruction.operand >= function_count {
                    return Err(CompactBytecodeValidationError::CodeSlotOutOfBounds {
                        slot: function.slot,
                        index: instruction.operand,
                    });
                }
            }
            CompactOpcode::Jump => {
                if instruction.operand >= instruction_count {
                    return Err(CompactBytecodeValidationError::JumpOutOfBounds {
                        slot: function.slot,
                        index: instruction.operand,
                    });
                }
            }
            CompactOpcode::EnsureContent => {
                if instruction.operand >= program.content_unit_count {
                    return Err(CompactBytecodeValidationError::ContentUnitOutOfBounds {
                        slot: function.slot,
                        index: instruction.operand,
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program() -> CompactBytecodeProgram {
        CompactBytecodeProgram {
            abi_version: COMPACT_BYTECODE_ABI_VERSION,
            runtime_type_count: 1,
            constant_count: 1,
            content_unit_count: 1,
            functions: vec![CompactBytecodeFunction {
                slot: CompactCodeSlotId(0),
                signature: CompactRuntimeSignature::default(),
                instructions: vec![CompactInstruction {
                    opcode: CompactOpcode::Return.encoded(),
                    operand: 0,
                }],
            }],
        }
    }

    #[test]
    fn compact_bytecode_validates_well_formed_program() {
        program()
            .verify(CompactBytecodeValidationBudget::default())
            .expect("compact bytecode validates");
    }

    #[test]
    fn compact_bytecode_rejects_unknown_opcode() {
        let mut program = program();
        program.functions[0].instructions[0].opcode = 255;

        assert!(matches!(
            program.verify(CompactBytecodeValidationBudget::default()),
            Err(CompactBytecodeValidationError::UnknownOpcode { .. })
        ));
    }

    #[test]
    fn compact_bytecode_rejects_out_of_bounds_operands() {
        let cases = [
            (
                CompactOpcode::Constant,
                CompactBytecodeValidationError::ConstantOutOfBounds {
                    slot: CompactCodeSlotId(0),
                    index: 1,
                },
            ),
            (
                CompactOpcode::Call,
                CompactBytecodeValidationError::CodeSlotOutOfBounds {
                    slot: CompactCodeSlotId(0),
                    index: 1,
                },
            ),
            (
                CompactOpcode::Jump,
                CompactBytecodeValidationError::JumpOutOfBounds {
                    slot: CompactCodeSlotId(0),
                    index: 1,
                },
            ),
            (
                CompactOpcode::EnsureContent,
                CompactBytecodeValidationError::ContentUnitOutOfBounds {
                    slot: CompactCodeSlotId(0),
                    index: 1,
                },
            ),
        ];
        for (opcode, expected) in cases {
            let mut program = program();
            program.functions[0].instructions[0] = CompactInstruction {
                opcode: opcode.encoded(),
                operand: 1,
            };

            assert_eq!(
                program.verify(CompactBytecodeValidationBudget::default()),
                Err(expected)
            );
        }
    }

    #[test]
    fn compact_bytecode_rejects_duplicate_slots_and_runtime_type_bounds() {
        let mut duplicate = program();
        duplicate.functions.push(duplicate.functions[0].clone());
        assert!(matches!(
            duplicate.verify(CompactBytecodeValidationBudget::default()),
            Err(CompactBytecodeValidationError::DuplicateCodeSlot(
                CompactCodeSlotId(0)
            ))
        ));

        let mut bad_type = program();
        bad_type.functions[0].signature.result = CompactRuntimeTypeId(1);
        assert!(matches!(
            bad_type.verify(CompactBytecodeValidationBudget::default()),
            Err(CompactBytecodeValidationError::RuntimeTypeOutOfBounds {
                slot: CompactCodeSlotId(0),
                index: 1
            })
        ));
    }

    #[test]
    fn compact_bytecode_enforces_validation_budgets() {
        let mut too_many_functions = program();
        too_many_functions.functions.push(CompactBytecodeFunction {
            slot: CompactCodeSlotId(1),
            ..too_many_functions.functions[0].clone()
        });
        assert_eq!(
            too_many_functions.verify(CompactBytecodeValidationBudget {
                functions: 1,
                ..CompactBytecodeValidationBudget::default()
            }),
            Err(CompactBytecodeValidationError::BudgetExceeded {
                budget: "functions"
            })
        );

        let too_many_instructions = program();
        assert_eq!(
            too_many_instructions.verify(CompactBytecodeValidationBudget {
                instructions: 0,
                ..CompactBytecodeValidationBudget::default()
            }),
            Err(CompactBytecodeValidationError::BudgetExceeded {
                budget: "instructions"
            })
        );
    }
}
