//! Validated, bounded stack programs shared by Fx samplers and View values.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use super::{
    canonical::{
        CanonicalEncodeError, CanonicalEncoder, CanonicalHashSink, CanonicalLengthSink,
        CanonicalReader, CanonicalSink, CanonicalVecSink, FxCanonicalDecodeError,
    },
    evaluator,
    graph::{FxRuntimeParameterRef, FxRuntimeParameterSlot},
    state::FxSampleContext,
    value::{FxRuntimeType, FxRuntimeValue, FxRuntimeValueDecodeError},
};

pub const FX_MAX_INSTRUCTIONS_PER_SAMPLER: usize = 1_024;
pub const FX_MAX_CONSTANTS_PER_SAMPLER: usize = 256;
pub const FX_MAX_STACK_VALUES_PER_PROGRAM: usize = 64;
pub const FX_MAX_CAPTURED_OR_PARAMETER_SLOTS: usize = 64;
pub const FX_DEFAULT_EVALUATOR_OPERATIONS: u32 = 262_144;

/// Context values available without a host callback.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxContextSlot {
    /// Activation-relative logical seconds, represented as dimensionless `F32`.
    Time = 0,
    /// Target-local logical ordinal, represented as an integer-valued `F32`.
    Ordinal = 1,
    /// Golden-angle ordinal phase, represented as dimensionless radians in `F32`.
    OrdinalPhase = 2,
    ReduceMotion = 3,
    /// Center of the current renderer target relative to the sampled glyph.
    TargetCenterX = 4,
    /// Center of the current renderer target relative to the sampled glyph.
    TargetCenterY = 5,
    /// Center of the sampled glyph in glyph-local coordinates.
    GlyphCenterX = 6,
    /// Center of the sampled glyph in glyph-local coordinates.
    GlyphCenterY = 7,
}

/// Closed instruction inventory for deterministic value programs.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ValueInstruction {
    Constant {
        value: FxRuntimeValue,
    },
    LoadParameter {
        parameter: FxRuntimeParameterRef,
    },
    LoadState {
        slot: u16,
        ty: FxRuntimeType,
    },
    LoadContext {
        slot: FxContextSlot,
    },
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Abs,
    Min,
    Max,
    Clamp,
    Sin,
    Cos,
    Floor,
    Fract,
    Equal,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Not,
    And,
    Or,
    /// Pops `condition`, `when_true`, and `when_false` in that authored order.
    Select,
    /// Pops one `I32` bucket and hashes it with the context seed and ordinal.
    HashNoise,
    /// Floors one finite `F32` and converts it to `I32`, rejecting overflow.
    FloorToI32,
    /// Pops dimensionless `x` and `y` values in authored order.
    MakeVec2,
    /// Pops linear red, green, blue, and alpha `F32` values in authored order.
    MakeColor,
    /// Pops all ten [`super::Transform2D`] fields in declaration order.
    MakeTransform2D,
    Return,
    /// Reinterprets one `U32` bit pattern as an `I32` without changing bits.
    BitcastU32ToI32,
    /// Projects one `Seconds` value to its dimensionless `F32` magnitude.
    SecondsValue,
    /// Projects the x component of one `Vec2` value as `F32`.
    Vec2X,
    /// Projects the y component of one `Vec2` value as `F32`.
    Vec2Y,
}

/// Declared input and return types for a value program.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ValueProgramSchema {
    parameter_types: Vec<FxRuntimeType>,
    state_types: Vec<FxRuntimeType>,
    return_type: FxRuntimeType,
}

/// Validation limits selected by the owning program kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueProgramLimits {
    instructions: usize,
    constants: usize,
    stack_values: usize,
    parameter_and_state_slots: usize,
}

/// Common validated program body. View owns a distinct wrapper around this type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidatedValueProgram {
    schema: ValueProgramSchema,
    instructions: Vec<ValueInstruction>,
}

/// Validated owner for Fx sampler programs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxSamplerProgram {
    program: ValidatedValueProgram,
    #[serde(skip)]
    canonical_counts: CanonicalCounts,
    #[serde(skip)]
    canonical_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanonicalCounts {
    parameters: u16,
    states: u16,
    instructions: u16,
    constants: u16,
}

/// Borrowed runtime inputs for one program evaluation.
#[derive(Clone, Copy, Debug)]
pub struct ValueProgramInputs<'a> {
    pub parameters: &'a [FxRuntimeValue],
    pub state: &'a [FxRuntimeValue],
}

/// Per-instance, per-frame operation budget shared by all sampler evaluations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxEvaluationBudget {
    remaining: u32,
    limit: u32,
}

/// A malformed program rejected before execution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ValueProgramValidationError {
    #[error("program has {actual} instructions, exceeding the limit of {limit}")]
    TooManyInstructions { actual: usize, limit: usize },
    #[error("program has {actual} constants, exceeding the limit of {limit}")]
    TooManyConstants { actual: usize, limit: usize },
    #[error("program declares {actual} parameter/state slots, exceeding the limit of {limit}")]
    TooManySlots { actual: usize, limit: usize },
    #[error("instruction {instruction} references {kind} slot {slot}, but only {available} exist")]
    SlotOutOfBounds {
        instruction: usize,
        kind: &'static str,
        slot: u16,
        available: usize,
    },
    #[error(
        "instruction {instruction} declares {actual:?} for {kind} slot {slot}, expected {expected:?}"
    )]
    SlotTypeMismatch {
        instruction: usize,
        kind: &'static str,
        slot: u16,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error(
        "instruction {instruction} requires {required} stack values, but only {available} exist"
    )]
    StackUnderflow {
        instruction: usize,
        required: usize,
        available: usize,
    },
    #[error(
        "instruction {instruction} grows the stack to {actual}, exceeding the limit of {limit}"
    )]
    StackLimit {
        instruction: usize,
        actual: usize,
        limit: usize,
    },
    #[error("instruction {instruction} `{operation}` does not accept operand types {operands:?}")]
    InvalidOperands {
        instruction: usize,
        operation: &'static str,
        operands: Vec<FxRuntimeType>,
    },
    #[error("instruction {instruction} returns {actual:?}, but the program declares {expected:?}")]
    ReturnTypeMismatch {
        instruction: usize,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error("instruction {instruction} returns with {remaining} additional stack values")]
    ReturnStackNotEmpty {
        instruction: usize,
        remaining: usize,
    },
    #[error("return instruction {instruction} must be the final instruction")]
    ReturnNotLast { instruction: usize },
    #[error("program has no return instruction")]
    MissingReturn,
    #[error("validated sampler count cannot be represented in canonical form")]
    CanonicalCountOverflow,
    #[error("validated sampler canonical length overflow")]
    CanonicalLengthOverflow,
}

/// A host-width count cannot enter the schema-1 value-program digest domain.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ValueProgramSemanticDigestError {
    #[error("value-program semantic digest length does not fit u64")]
    LengthOverflow,
}

/// Failure while measuring the canonical v1 representation of a validated
/// sampler program.
///
/// A sampler is normally constructed through [`FxSamplerProgram::validate`],
/// so the owner limits are already guaranteed.  The checks remain here because
/// the canonical representation is also a separately bounded public boundary
/// and must never turn a malformed count or arithmetic overflow into a
/// truncated or substituted encoding.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxSamplerProgramCanonicalError {
    #[error("canonical sampler program {kind} count {actual} exceeds the owner limit of {limit}")]
    OwnerLimit {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("canonical sampler program length overflow")]
    LengthOverflow,
    #[error("canonical sampler program allocation failed")]
    AllocationFailed,
    #[error("canonical sampler program wrote {actual} bytes, expected {expected} bytes")]
    LengthMismatch { actual: usize, expected: usize },
}

/// Rejection while reconstructing one canonical v1 sampler program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxSamplerProgramDecodeError {
    #[error(transparent)]
    Canonical(#[from] FxCanonicalDecodeError),
    #[error("canonical sampler {kind} count {actual} exceeds the owner limit of {limit}")]
    OwnerLimit {
        kind: &'static str,
        actual: u64,
        limit: usize,
    },
    #[error("canonical sampler failed to allocate {count} {kind} rows")]
    AllocationFailed { kind: &'static str, count: usize },
    #[error("unknown Fx context slot tag {0}")]
    UnknownContextSlot(u8),
    #[error("unknown Fx value-program opcode {0}")]
    UnknownOpcode(u8),
    #[error("canonical sampler {kind} value {value} is out of range")]
    IntegerOutOfRange { kind: &'static str, value: u64 },
    #[error(transparent)]
    RuntimeValue(#[from] FxRuntimeValueDecodeError),
    #[error(transparent)]
    Validation(#[from] ValueProgramValidationError),
}

/// Deterministic evaluator failure. No partial application output is committed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxEvaluationError {
    #[error("program expected {expected} {kind} values, got {actual}")]
    InputCount {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{kind} slot {slot} has type {actual:?}, expected {expected:?}")]
    InputType {
        kind: &'static str,
        slot: usize,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error("Fx evaluator exhausted its {limit}-operation budget at instruction {instruction}")]
    BudgetExceeded { instruction: usize, limit: u32 },
    #[error("division by zero at instruction {instruction}")]
    DivisionByZero { instruction: usize },
    #[error("`{operation}` produced a non-finite result at instruction {instruction}")]
    NonFiniteResult {
        instruction: usize,
        operation: &'static str,
    },
    #[error("`{operation}` underflowed a non-zero value to zero at instruction {instruction}")]
    Underflow {
        instruction: usize,
        operation: &'static str,
    },
    #[error("integer `{operation}` overflow at instruction {instruction}")]
    IntegerOverflow {
        instruction: usize,
        operation: &'static str,
    },
    #[error("integer `{operation}` conversion is out of range at instruction {instruction}")]
    IntegerConversion {
        instruction: usize,
        operation: &'static str,
    },
    #[error(
        "unit/type mismatch for `{operation}` at instruction {instruction}: {left:?} and {right:?}"
    )]
    UnitMismatch {
        instruction: usize,
        operation: &'static str,
        left: FxRuntimeType,
        right: FxRuntimeType,
    },
    #[error("clamp minimum exceeds maximum at instruction {instruction}")]
    InvalidClampBounds { instruction: usize },
    #[error("invalid transform opacity at instruction {instruction}")]
    InvalidOpacity { instruction: usize },
    #[error("validated program stack invariant failed at instruction {instruction}")]
    InvalidProgramState { instruction: usize },
}

impl FxContextSlot {
    pub const fn value_type(self) -> FxRuntimeType {
        match self {
            Self::Time | Self::Ordinal | Self::OrdinalPhase => FxRuntimeType::F32,
            Self::ReduceMotion => FxRuntimeType::Bool,
            Self::TargetCenterX | Self::TargetCenterY | Self::GlyphCenterX | Self::GlyphCenterY => {
                FxRuntimeType::Length
            }
        }
    }
}

impl ValueProgramSchema {
    pub fn new(
        parameter_types: Vec<FxRuntimeType>,
        state_types: Vec<FxRuntimeType>,
        return_type: FxRuntimeType,
    ) -> Self {
        Self {
            parameter_types,
            state_types,
            return_type,
        }
    }

    pub fn parameter_types(&self) -> &[FxRuntimeType] {
        &self.parameter_types
    }

    pub fn parameter_ref(&self, index: usize) -> Option<FxRuntimeParameterRef> {
        let ty = *self.parameter_types.get(index)?;
        Some(FxRuntimeParameterRef::from_parts(
            FxRuntimeParameterSlot::from_index(index)?,
            ty,
        ))
    }

    pub fn state_types(&self) -> &[FxRuntimeType] {
        &self.state_types
    }

    pub const fn return_type(&self) -> FxRuntimeType {
        self.return_type
    }
}

impl ValueProgramLimits {
    pub const SAMPLER: Self = Self {
        instructions: FX_MAX_INSTRUCTIONS_PER_SAMPLER,
        constants: FX_MAX_CONSTANTS_PER_SAMPLER,
        stack_values: FX_MAX_STACK_VALUES_PER_PROGRAM,
        parameter_and_state_slots: FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
    };

    pub const VIEW: Self = Self {
        instructions: 4_096,
        constants: 1_024,
        stack_values: FX_MAX_STACK_VALUES_PER_PROGRAM,
        parameter_and_state_slots: 256,
    };
}

impl ValidatedValueProgram {
    /// Validates limits, slots, stack shape, operand types, and the single return.
    pub fn validate(
        schema: ValueProgramSchema,
        instructions: Vec<ValueInstruction>,
        limits: ValueProgramLimits,
    ) -> Result<Self, ValueProgramValidationError> {
        validate_program(&schema, &instructions, limits)?;
        Ok(Self {
            schema,
            instructions,
        })
    }

    pub const fn schema(&self) -> &ValueProgramSchema {
        &self.schema
    }

    pub fn instructions(&self) -> &[ValueInstruction] {
        &self.instructions
    }

    /// Returns the schema-1 semantic digest of the validated shared value
    /// program. Fx sampler and View owners use this single instruction codec;
    /// neither consumer maintains a parallel opcode inventory.
    pub fn semantic_digest_v1(&self) -> Result<[u8; 32], ValueProgramSemanticDigestError> {
        let parameter_count = u64::try_from(self.schema.parameter_types().len())
            .map_err(|_| ValueProgramSemanticDigestError::LengthOverflow)?;
        let state_count = u64::try_from(self.schema.state_types().len())
            .map_err(|_| ValueProgramSemanticDigestError::LengthOverflow)?;
        let instruction_count = u64::try_from(self.instructions.len())
            .map_err(|_| ValueProgramSemanticDigestError::LengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
        infallible(encoder.domain_v1(b"arcweft.value-program"));
        infallible(encode_value_program_body(
            &mut encoder,
            self,
            parameter_count,
            state_count,
            instruction_count,
        ));
        Ok(*hasher.finalize().as_bytes())
    }

    pub fn evaluate(
        &self,
        inputs: ValueProgramInputs<'_>,
        context: FxSampleContext,
        budget: &mut FxEvaluationBudget,
    ) -> Result<FxRuntimeValue, FxEvaluationError> {
        evaluator::evaluate(self, inputs, context, budget)
    }
}

impl CanonicalCounts {
    fn from_program(program: &ValidatedValueProgram) -> Result<Self, ValueProgramValidationError> {
        let schema = program.schema();
        let parameters = u16::try_from(schema.parameter_types().len())
            .map_err(|_| ValueProgramValidationError::CanonicalCountOverflow)?;
        let states = u16::try_from(schema.state_types().len())
            .map_err(|_| ValueProgramValidationError::CanonicalCountOverflow)?;
        let instructions = u16::try_from(program.instructions().len())
            .map_err(|_| ValueProgramValidationError::CanonicalCountOverflow)?;
        let constants = u16::try_from(
            program
                .instructions()
                .iter()
                .filter(|instruction| matches!(instruction, ValueInstruction::Constant { .. }))
                .count(),
        )
        .map_err(|_| ValueProgramValidationError::CanonicalCountOverflow)?;
        Ok(Self {
            parameters,
            states,
            instructions,
            constants,
        })
    }
}

impl FxSamplerProgram {
    pub fn decode_canonical_v1(bytes: &[u8]) -> Result<Self, FxSamplerProgramDecodeError> {
        let mut reader = CanonicalReader::new(bytes);
        let program = Self::decode_canonical_v1_body(&mut reader)?;
        reader.finish()?;
        Ok(program)
    }

    pub(super) fn decode_canonical_v1_reader(
        reader: &mut CanonicalReader<'_>,
        encoded_len: usize,
    ) -> Result<Self, FxSamplerProgramDecodeError> {
        let bytes = reader.raw_bytes(encoded_len)?;
        Self::decode_canonical_v1(bytes)
    }

    fn decode_canonical_v1_body(
        reader: &mut CanonicalReader<'_>,
    ) -> Result<Self, FxSamplerProgramDecodeError> {
        reader.domain_v1(b"arcweft.fx-sampler-program")?;

        let parameter_count =
            decode_count(reader, FX_MAX_CAPTURED_OR_PARAMETER_SLOTS, "parameter")?;
        let mut parameter_types = Vec::new();
        parameter_types
            .try_reserve_exact(parameter_count)
            .map_err(|_| FxSamplerProgramDecodeError::AllocationFailed {
                kind: "parameter",
                count: parameter_count,
            })?;
        for _ in 0..parameter_count {
            parameter_types.push(FxRuntimeType::decode_canonical_tag(reader.tag()?)?);
        }

        let state_count = decode_count(reader, FX_MAX_CAPTURED_OR_PARAMETER_SLOTS, "state")?;
        let total_slots = parameter_count.checked_add(state_count).ok_or(
            FxSamplerProgramDecodeError::OwnerLimit {
                kind: "parameter/state",
                actual: u64::MAX,
                limit: FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
            },
        )?;
        let total_slots_u64 =
            u64::try_from(total_slots).map_err(|_| FxCanonicalDecodeError::LengthOverflow)?;
        if total_slots > FX_MAX_CAPTURED_OR_PARAMETER_SLOTS {
            return Err(FxSamplerProgramDecodeError::OwnerLimit {
                kind: "parameter/state",
                actual: total_slots_u64,
                limit: FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
            });
        }
        let mut state_types = Vec::new();
        state_types.try_reserve_exact(state_count).map_err(|_| {
            FxSamplerProgramDecodeError::AllocationFailed {
                kind: "state",
                count: state_count,
            }
        })?;
        for _ in 0..state_count {
            state_types.push(FxRuntimeType::decode_canonical_tag(reader.tag()?)?);
        }

        let return_type = FxRuntimeType::decode_canonical_tag(reader.tag()?)?;
        let instruction_count =
            decode_count(reader, FX_MAX_INSTRUCTIONS_PER_SAMPLER, "instruction")?;
        let mut instructions = Vec::new();
        instructions
            .try_reserve_exact(instruction_count)
            .map_err(|_| FxSamplerProgramDecodeError::AllocationFailed {
                kind: "instruction",
                count: instruction_count,
            })?;
        for _ in 0..instruction_count {
            instructions.push(decode_instruction(reader)?);
        }

        Ok(Self::validate(
            ValueProgramSchema::new(parameter_types, state_types, return_type),
            instructions,
        )?)
    }

    pub fn validate(
        schema: ValueProgramSchema,
        instructions: Vec<ValueInstruction>,
    ) -> Result<Self, ValueProgramValidationError> {
        ValidatedValueProgram::validate(schema, instructions, ValueProgramLimits::SAMPLER).and_then(
            |program| {
                CanonicalCounts::from_program(&program).and_then(|canonical_counts| {
                    let mut sampler = Self {
                        program,
                        canonical_counts,
                        canonical_len: 0,
                    };
                    let canonical_len = sampler
                        .measure_canonical_v1()
                        .map_err(|_| ValueProgramValidationError::CanonicalLengthOverflow)?;
                    sampler.canonical_len = u64::try_from(canonical_len)
                        .map_err(|_| ValueProgramValidationError::CanonicalLengthOverflow)?;
                    Ok(sampler)
                })
            },
        )
    }

    pub const fn program(&self) -> &ValidatedValueProgram {
        &self.program
    }

    pub const fn return_type(&self) -> FxRuntimeType {
        self.program.schema.return_type
    }

    pub fn evaluate(
        &self,
        inputs: ValueProgramInputs<'_>,
        context: FxSampleContext,
        budget: &mut FxEvaluationBudget,
    ) -> Result<FxRuntimeValue, FxEvaluationError> {
        self.program.evaluate(inputs, context, budget)
    }

    /// Returns the canonical v1 bytes after checking the exact encoded size.
    pub fn canonical_v1_bytes(&self) -> Result<Vec<u8>, FxSamplerProgramCanonicalError> {
        let length = self.canonical_v1_len()?;
        let mut out = Vec::new();
        let sink =
            CanonicalVecSink::with_preflight(&mut out, length).map_err(map_canonical_error)?;
        let mut encoder = CanonicalEncoder::new(sink);
        self.encode_canonical_v1(&mut encoder)
            .map_err(map_canonical_error)?;
        encoder.into_inner().finish().map_err(map_canonical_error)?;
        Ok(out)
    }

    /// Returns the sealed canonical v1 length after checking host-width use.
    pub fn canonical_v1_len(&self) -> Result<usize, FxSamplerProgramCanonicalError> {
        self.validate_canonical_limits()?;
        usize::try_from(self.canonical_len)
            .map_err(|_| FxSamplerProgramCanonicalError::LengthOverflow)
    }

    /// Returns the validated canonical v1 length for infallible nested hashing.
    pub const fn canonical_v1_len_u64(&self) -> u64 {
        self.canonical_len
    }

    fn validate_canonical_limits(&self) -> Result<(), FxSamplerProgramCanonicalError> {
        let parameter_count = bounded_count(
            usize::from(self.canonical_counts.parameters),
            FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
            "parameter",
        )?;
        let state_count = bounded_count(
            usize::from(self.canonical_counts.states),
            FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
            "state",
        )?;
        let total_slots = usize::from(parameter_count)
            .checked_add(usize::from(state_count))
            .ok_or(FxSamplerProgramCanonicalError::LengthOverflow)?;
        if total_slots > FX_MAX_CAPTURED_OR_PARAMETER_SLOTS {
            return Err(FxSamplerProgramCanonicalError::OwnerLimit {
                kind: "parameter/state",
                actual: total_slots,
                limit: FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
            });
        }
        bounded_count(
            usize::from(self.canonical_counts.instructions),
            FX_MAX_INSTRUCTIONS_PER_SAMPLER,
            "instruction",
        )?;

        let constants = usize::from(self.canonical_counts.constants);
        if constants > FX_MAX_CONSTANTS_PER_SAMPLER {
            return Err(FxSamplerProgramCanonicalError::OwnerLimit {
                kind: "constant",
                actual: constants,
                limit: FX_MAX_CONSTANTS_PER_SAMPLER,
            });
        }
        Ok(())
    }

    fn measure_canonical_v1(&self) -> Result<usize, CanonicalEncodeError> {
        let mut encoder = CanonicalEncoder::new(CanonicalLengthSink::default());
        self.encode_canonical_v1(&mut encoder)?;
        Ok(encoder.into_inner().finish())
    }

    /// Appends canonical v1 bytes after a checked preflight.
    pub fn append_canonical_v1_bytes(
        &self,
        out: &mut Vec<u8>,
    ) -> Result<(), FxSamplerProgramCanonicalError> {
        let expected = self.canonical_v1_len()?;
        let sink = CanonicalVecSink::with_preflight(out, expected).map_err(map_canonical_error)?;
        let mut encoder = CanonicalEncoder::new(sink);
        self.encode_canonical_v1(&mut encoder)
            .map_err(map_canonical_error)?;
        encoder.into_inner().finish().map_err(map_canonical_error)
    }

    pub(super) fn encode_canonical_v1<S: CanonicalSink>(
        &self,
        encoder: &mut CanonicalEncoder<S>,
    ) -> Result<(), S::Error> {
        encoder.domain_v1(b"arcweft.fx-sampler-program")?;
        encode_value_program_body(
            encoder,
            &self.program,
            u64::from(self.canonical_counts.parameters),
            u64::from(self.canonical_counts.states),
            u64::from(self.canonical_counts.instructions),
        )
    }
}

fn encode_value_program_body<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    program: &ValidatedValueProgram,
    parameter_count: u64,
    state_count: u64,
    instruction_count: u64,
) -> Result<(), S::Error> {
    let schema = program.schema();
    encoder.unsigned(parameter_count)?;
    for value in schema.parameter_types() {
        encoder.tag(*value as u8)?;
    }
    encoder.unsigned(state_count)?;
    for value in schema.state_types() {
        encoder.tag(*value as u8)?;
    }
    encoder.tag(schema.return_type() as u8)?;
    encoder.unsigned(instruction_count)?;
    for instruction in program.instructions() {
        encode_instruction(encoder, instruction)?;
    }
    Ok(())
}

fn infallible(result: Result<(), std::convert::Infallible>) {
    match result {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

fn decode_count(
    reader: &mut CanonicalReader<'_>,
    limit: usize,
    kind: &'static str,
) -> Result<usize, FxSamplerProgramDecodeError> {
    let actual = reader.unsigned()?;
    let actual_usize =
        usize::try_from(actual).map_err(|_| FxSamplerProgramDecodeError::OwnerLimit {
            kind,
            actual,
            limit,
        })?;
    if actual_usize > limit {
        return Err(FxSamplerProgramDecodeError::OwnerLimit {
            kind,
            actual,
            limit,
        });
    }
    Ok(actual_usize)
}

fn decode_context_slot(tag: u8) -> Result<FxContextSlot, FxSamplerProgramDecodeError> {
    match tag {
        0 => Ok(FxContextSlot::Time),
        1 => Ok(FxContextSlot::Ordinal),
        2 => Ok(FxContextSlot::OrdinalPhase),
        3 => Ok(FxContextSlot::ReduceMotion),
        4 => Ok(FxContextSlot::TargetCenterX),
        5 => Ok(FxContextSlot::TargetCenterY),
        6 => Ok(FxContextSlot::GlyphCenterX),
        7 => Ok(FxContextSlot::GlyphCenterY),
        _ => Err(FxSamplerProgramDecodeError::UnknownContextSlot(tag)),
    }
}

fn decode_instruction(
    reader: &mut CanonicalReader<'_>,
) -> Result<ValueInstruction, FxSamplerProgramDecodeError> {
    let instruction = match reader.tag()? {
        0 => ValueInstruction::Constant {
            value: FxRuntimeValue::decode_canonical_v1(reader)?,
        },
        1 => {
            let ty = FxRuntimeType::decode_canonical_tag(reader.tag()?)?;
            let slot = decode_u16(reader, "parameter slot")?;
            ValueInstruction::LoadParameter {
                parameter: FxRuntimeParameterRef::from_parts(
                    FxRuntimeParameterSlot::from_index(usize::from(slot)).ok_or(
                        FxSamplerProgramDecodeError::IntegerOutOfRange {
                            kind: "parameter slot",
                            value: u64::from(slot),
                        },
                    )?,
                    ty,
                ),
            }
        }
        2 => {
            let ty = FxRuntimeType::decode_canonical_tag(reader.tag()?)?;
            ValueInstruction::LoadState {
                slot: decode_u16(reader, "state slot")?,
                ty,
            }
        }
        3 => ValueInstruction::LoadContext {
            slot: decode_context_slot(reader.tag()?)?,
        },
        4 => ValueInstruction::Neg,
        5 => ValueInstruction::Add,
        6 => ValueInstruction::Sub,
        7 => ValueInstruction::Mul,
        8 => ValueInstruction::Div,
        9 => ValueInstruction::Abs,
        10 => ValueInstruction::Min,
        11 => ValueInstruction::Max,
        12 => ValueInstruction::Clamp,
        13 => ValueInstruction::Sin,
        14 => ValueInstruction::Cos,
        15 => ValueInstruction::Floor,
        16 => ValueInstruction::Fract,
        17 => ValueInstruction::Equal,
        18 => ValueInstruction::Less,
        19 => ValueInstruction::LessEqual,
        20 => ValueInstruction::Greater,
        21 => ValueInstruction::GreaterEqual,
        22 => ValueInstruction::Not,
        23 => ValueInstruction::And,
        24 => ValueInstruction::Or,
        25 => ValueInstruction::Select,
        26 => ValueInstruction::HashNoise,
        27 => ValueInstruction::MakeVec2,
        28 => ValueInstruction::MakeTransform2D,
        29 => ValueInstruction::Return,
        30 => ValueInstruction::FloorToI32,
        31 => ValueInstruction::MakeColor,
        32 => ValueInstruction::BitcastU32ToI32,
        33 => ValueInstruction::SecondsValue,
        34 => ValueInstruction::Vec2X,
        35 => ValueInstruction::Vec2Y,
        opcode => return Err(FxSamplerProgramDecodeError::UnknownOpcode(opcode)),
    };
    Ok(instruction)
}

fn decode_u16(
    reader: &mut CanonicalReader<'_>,
    kind: &'static str,
) -> Result<u16, FxSamplerProgramDecodeError> {
    let value = reader.unsigned()?;
    u16::try_from(value).map_err(|_| FxSamplerProgramDecodeError::IntegerOutOfRange { kind, value })
}

fn bounded_count(
    actual: usize,
    limit: usize,
    kind: &'static str,
) -> Result<u16, FxSamplerProgramCanonicalError> {
    if actual > limit {
        return Err(FxSamplerProgramCanonicalError::OwnerLimit {
            kind,
            actual,
            limit,
        });
    }
    u16::try_from(actual).map_err(|_| FxSamplerProgramCanonicalError::LengthOverflow)
}

fn encode_instruction<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    instruction: &ValueInstruction,
) -> Result<(), S::Error> {
    match instruction {
        ValueInstruction::Constant { value } => {
            encoder.tag(0)?;
            value.encode_canonical_v1(encoder)?;
        }
        ValueInstruction::LoadParameter { parameter } => {
            encoder.tag(1)?;
            encoder.tag(parameter.runtime_type() as u8)?;
            encoder.unsigned(u64::from(parameter.slot().get()))?;
        }
        ValueInstruction::LoadState { slot, ty } => {
            encoder.tag(2)?;
            encoder.tag(*ty as u8)?;
            encoder.unsigned(u64::from(*slot))?;
        }
        ValueInstruction::LoadContext { slot } => {
            encoder.tag(3)?;
            encoder.tag(*slot as u8)?;
        }
        ValueInstruction::Neg => encoder.tag(4)?,
        ValueInstruction::Add => encoder.tag(5)?,
        ValueInstruction::Sub => encoder.tag(6)?,
        ValueInstruction::Mul => encoder.tag(7)?,
        ValueInstruction::Div => encoder.tag(8)?,
        ValueInstruction::Abs => encoder.tag(9)?,
        ValueInstruction::Min => encoder.tag(10)?,
        ValueInstruction::Max => encoder.tag(11)?,
        ValueInstruction::Clamp => encoder.tag(12)?,
        ValueInstruction::Sin => encoder.tag(13)?,
        ValueInstruction::Cos => encoder.tag(14)?,
        ValueInstruction::Floor => encoder.tag(15)?,
        ValueInstruction::Fract => encoder.tag(16)?,
        ValueInstruction::Equal => encoder.tag(17)?,
        ValueInstruction::Less => encoder.tag(18)?,
        ValueInstruction::LessEqual => encoder.tag(19)?,
        ValueInstruction::Greater => encoder.tag(20)?,
        ValueInstruction::GreaterEqual => encoder.tag(21)?,
        ValueInstruction::Not => encoder.tag(22)?,
        ValueInstruction::And => encoder.tag(23)?,
        ValueInstruction::Or => encoder.tag(24)?,
        ValueInstruction::Select => encoder.tag(25)?,
        ValueInstruction::HashNoise => encoder.tag(26)?,
        ValueInstruction::MakeVec2 => encoder.tag(27)?,
        ValueInstruction::MakeTransform2D => encoder.tag(28)?,
        ValueInstruction::Return => encoder.tag(29)?,
        ValueInstruction::FloorToI32 => encoder.tag(30)?,
        ValueInstruction::MakeColor => encoder.tag(31)?,
        ValueInstruction::BitcastU32ToI32 => encoder.tag(32)?,
        ValueInstruction::SecondsValue => encoder.tag(33)?,
        ValueInstruction::Vec2X => encoder.tag(34)?,
        ValueInstruction::Vec2Y => encoder.tag(35)?,
    }
    Ok(())
}

fn map_canonical_error(error: CanonicalEncodeError) -> FxSamplerProgramCanonicalError {
    match error {
        CanonicalEncodeError::LengthOverflow => FxSamplerProgramCanonicalError::LengthOverflow,
        CanonicalEncodeError::AllocationFailed => FxSamplerProgramCanonicalError::AllocationFailed,
        CanonicalEncodeError::LengthMismatch { actual, expected } => {
            FxSamplerProgramCanonicalError::LengthMismatch { actual, expected }
        }
    }
}

#[derive(Deserialize)]
struct FxSamplerProgramWire {
    program: RawValueProgram,
}

#[derive(Deserialize)]
struct RawValueProgram {
    schema: ValueProgramSchema,
    instructions: Vec<ValueInstruction>,
}

impl<'de> Deserialize<'de> for FxSamplerProgram {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxSamplerProgramWire::deserialize(deserializer)?;
        Self::validate(wire.program.schema, wire.program.instructions).map_err(D::Error::custom)
    }
}

impl Default for FxEvaluationBudget {
    fn default() -> Self {
        Self::new(FX_DEFAULT_EVALUATOR_OPERATIONS)
    }
}

impl FxEvaluationBudget {
    pub const fn new(limit: u32) -> Self {
        Self {
            remaining: limit,
            limit,
        }
    }

    pub const fn remaining(self) -> u32 {
        self.remaining
    }

    pub(crate) fn charge(&mut self, instruction: usize) -> Result<(), FxEvaluationError> {
        if self.remaining == 0 {
            Err(FxEvaluationError::BudgetExceeded {
                instruction,
                limit: self.limit,
            })
        } else {
            self.remaining -= 1;
            Ok(())
        }
    }
}

fn validate_program(
    schema: &ValueProgramSchema,
    instructions: &[ValueInstruction],
    limits: ValueProgramLimits,
) -> Result<(), ValueProgramValidationError> {
    validate_declared_limits(schema, instructions, limits)?;

    let mut state = ProgramValidationState {
        stack: Vec::with_capacity(limits.stack_values.min(instructions.len())),
        constants: 0,
        returned: false,
    };
    for (index, instruction) in instructions.iter().enumerate() {
        validate_instruction(
            index,
            instruction,
            schema,
            instructions.len(),
            limits.constants,
            &mut state,
        )?;
        if state.stack.len() > limits.stack_values {
            return Err(ValueProgramValidationError::StackLimit {
                instruction: index,
                actual: state.stack.len(),
                limit: limits.stack_values,
            });
        }
    }
    if state.returned {
        Ok(())
    } else {
        Err(ValueProgramValidationError::MissingReturn)
    }
}

struct ProgramValidationState {
    stack: Vec<FxRuntimeType>,
    constants: usize,
    returned: bool,
}

fn validate_instruction(
    index: usize,
    instruction: &ValueInstruction,
    schema: &ValueProgramSchema,
    instruction_count: usize,
    constant_limit: usize,
    state: &mut ProgramValidationState,
) -> Result<(), ValueProgramValidationError> {
    match instruction {
        ValueInstruction::Constant { value } => {
            state.constants += 1;
            if state.constants > constant_limit {
                return Err(ValueProgramValidationError::TooManyConstants {
                    actual: state.constants,
                    limit: constant_limit,
                });
            }
            state.stack.push(value.value_type());
        }
        ValueInstruction::LoadParameter { parameter } => {
            validate_slot(
                index,
                "parameter",
                parameter.slot().get(),
                parameter.runtime_type(),
                &schema.parameter_types,
            )?;
            state.stack.push(parameter.runtime_type());
        }
        ValueInstruction::LoadState { slot, ty } => {
            validate_slot(index, "state", *slot, *ty, &schema.state_types)?;
            state.stack.push(*ty);
        }
        ValueInstruction::LoadContext { slot } => state.stack.push(slot.value_type()),
        ValueInstruction::Return => validate_return(index, instruction_count, schema, state)?,
        instruction => validate_stack_operation(index, instruction, &mut state.stack)?,
    }
    Ok(())
}

fn validate_return(
    index: usize,
    instruction_count: usize,
    schema: &ValueProgramSchema,
    state: &mut ProgramValidationState,
) -> Result<(), ValueProgramValidationError> {
    if index + 1 != instruction_count {
        return Err(ValueProgramValidationError::ReturnNotLast { instruction: index });
    }
    let actual = pop_types(index, &mut state.stack, 1)?[0];
    if actual != schema.return_type {
        return Err(ValueProgramValidationError::ReturnTypeMismatch {
            instruction: index,
            expected: schema.return_type,
            actual,
        });
    }
    if !state.stack.is_empty() {
        return Err(ValueProgramValidationError::ReturnStackNotEmpty {
            instruction: index,
            remaining: state.stack.len(),
        });
    }
    state.returned = true;
    Ok(())
}

fn validate_stack_operation(
    index: usize,
    instruction: &ValueInstruction,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    match instruction {
        ValueInstruction::Neg => validate_unary(index, "neg", stack, neg_result),
        ValueInstruction::Add | ValueInstruction::Sub => {
            let name = if matches!(instruction, ValueInstruction::Add) {
                "add"
            } else {
                "sub"
            };
            validate_binary(index, name, stack, add_sub_result)
        }
        ValueInstruction::Mul => validate_binary(index, "mul", stack, mul_result),
        ValueInstruction::Div => validate_binary(index, "div", stack, div_result),
        ValueInstruction::Abs => validate_unary(index, "abs", stack, abs_result),
        ValueInstruction::Min | ValueInstruction::Max => {
            let name = if matches!(instruction, ValueInstruction::Min) {
                "min"
            } else {
                "max"
            };
            validate_binary(index, name, stack, order_result)
        }
        ValueInstruction::Clamp => validate_clamp(index, stack),
        ValueInstruction::Sin | ValueInstruction::Cos => {
            let name = if matches!(instruction, ValueInstruction::Sin) {
                "sin"
            } else {
                "cos"
            };
            validate_unary(index, name, stack, trig_result)
        }
        ValueInstruction::Floor | ValueInstruction::Fract => {
            let name = if matches!(instruction, ValueInstruction::Floor) {
                "floor"
            } else {
                "fract"
            };
            validate_unary(index, name, stack, f32_result)
        }
        ValueInstruction::Equal => validate_binary(index, "equal", stack, equal_result),
        ValueInstruction::Less
        | ValueInstruction::LessEqual
        | ValueInstruction::Greater
        | ValueInstruction::GreaterEqual => {
            let name = match instruction {
                ValueInstruction::Less => "less",
                ValueInstruction::LessEqual => "less_equal",
                ValueInstruction::Greater => "greater",
                ValueInstruction::GreaterEqual => "greater_equal",
                _ => unreachable!("comparison operation expected"),
            };
            validate_binary(index, name, stack, compare_result)
        }
        ValueInstruction::Not => validate_unary(index, "not", stack, bool_result),
        ValueInstruction::And | ValueInstruction::Or => {
            let name = if matches!(instruction, ValueInstruction::And) {
                "and"
            } else {
                "or"
            };
            validate_binary(index, name, stack, bools_result)
        }
        ValueInstruction::Select => validate_select(index, stack),
        ValueInstruction::HashNoise => {
            validate_unary(index, "hash_noise", stack, hash_noise_result)
        }
        ValueInstruction::FloorToI32 => {
            validate_unary(index, "floor_to_i32", stack, floor_to_i32_result)
        }
        ValueInstruction::MakeVec2 => validate_make_vec2(index, stack),
        ValueInstruction::MakeColor => validate_make_color(index, stack),
        ValueInstruction::MakeTransform2D => validate_make_transform(index, stack),
        ValueInstruction::BitcastU32ToI32 => validate_unary(
            index,
            "bitcast_u32_to_i32",
            stack,
            bitcast_u32_to_i32_result,
        ),
        ValueInstruction::SecondsValue => {
            validate_unary(index, "seconds_value", stack, seconds_value_result)
        }
        ValueInstruction::Vec2X => validate_unary(index, "vec2_x", stack, vec2_x_result),
        ValueInstruction::Vec2Y => validate_unary(index, "vec2_y", stack, vec2_y_result),
        ValueInstruction::Constant { .. }
        | ValueInstruction::LoadParameter { .. }
        | ValueInstruction::LoadState { .. }
        | ValueInstruction::LoadContext { .. }
        | ValueInstruction::Return => unreachable!("non-stack instruction reached stack validator"),
    }
}

fn validate_declared_limits(
    schema: &ValueProgramSchema,
    instructions: &[ValueInstruction],
    limits: ValueProgramLimits,
) -> Result<(), ValueProgramValidationError> {
    if instructions.len() > limits.instructions {
        return Err(ValueProgramValidationError::TooManyInstructions {
            actual: instructions.len(),
            limit: limits.instructions,
        });
    }
    let slot_count = schema.parameter_types.len() + schema.state_types.len();
    if slot_count > limits.parameter_and_state_slots {
        return Err(ValueProgramValidationError::TooManySlots {
            actual: slot_count,
            limit: limits.parameter_and_state_slots,
        });
    }
    Ok(())
}

fn validate_slot(
    instruction: usize,
    kind: &'static str,
    slot: u16,
    ty: FxRuntimeType,
    declared: &[FxRuntimeType],
) -> Result<(), ValueProgramValidationError> {
    let Some(expected) = declared.get(usize::from(slot)).copied() else {
        return Err(ValueProgramValidationError::SlotOutOfBounds {
            instruction,
            kind,
            slot,
            available: declared.len(),
        });
    };
    if expected != ty {
        return Err(ValueProgramValidationError::SlotTypeMismatch {
            instruction,
            kind,
            slot,
            expected,
            actual: ty,
        });
    }
    Ok(())
}

fn validate_unary(
    instruction: usize,
    operation: &'static str,
    stack: &mut Vec<FxRuntimeType>,
    result: fn(FxRuntimeType) -> Option<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operand = pop_types(instruction, stack, 1)?[0];
    result(operand).map_or_else(
        || {
            Err(ValueProgramValidationError::InvalidOperands {
                instruction,
                operation,
                operands: vec![operand],
            })
        },
        |ty| {
            stack.push(ty);
            Ok(())
        },
    )
}

fn validate_binary(
    instruction: usize,
    operation: &'static str,
    stack: &mut Vec<FxRuntimeType>,
    result: fn(FxRuntimeType, FxRuntimeType) -> Option<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 2)?;
    result(operands[0], operands[1]).map_or_else(
        || {
            Err(ValueProgramValidationError::InvalidOperands {
                instruction,
                operation,
                operands,
            })
        },
        |ty| {
            stack.push(ty);
            Ok(())
        },
    )
}

fn validate_clamp(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 3)?;
    if operands[0] == operands[1]
        && operands[1] == operands[2]
        && order_result(operands[0], operands[1]).is_some()
    {
        stack.push(operands[0]);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "clamp",
            operands,
        })
    }
}

fn validate_select(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 3)?;
    if operands[0] == FxRuntimeType::Bool && operands[1] == operands[2] {
        stack.push(operands[1]);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "select",
            operands,
        })
    }
}

fn validate_make_vec2(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 2)?;
    if operands == [FxRuntimeType::F32, FxRuntimeType::F32] {
        stack.push(FxRuntimeType::Vec2);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "make_vec2",
            operands,
        })
    }
}

fn validate_make_color(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 4)?;
    if operands == [FxRuntimeType::F32; 4] {
        stack.push(FxRuntimeType::Color);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "make_color",
            operands,
        })
    }
}

fn validate_make_transform(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    const FIELDS: [FxRuntimeType; 10] = [
        FxRuntimeType::Length,
        FxRuntimeType::Length,
        FxRuntimeType::F32,
        FxRuntimeType::F32,
        FxRuntimeType::Angle,
        FxRuntimeType::Angle,
        FxRuntimeType::Angle,
        FxRuntimeType::Length,
        FxRuntimeType::Length,
        FxRuntimeType::F32,
    ];
    let operands = pop_types(instruction, stack, FIELDS.len())?;
    if operands == FIELDS {
        stack.push(FxRuntimeType::Transform2D);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "make_transform_2d",
            operands,
        })
    }
}

fn pop_types(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
    count: usize,
) -> Result<Vec<FxRuntimeType>, ValueProgramValidationError> {
    if stack.len() < count {
        return Err(ValueProgramValidationError::StackUnderflow {
            instruction,
            required: count,
            available: stack.len(),
        });
    }
    Ok(stack.split_off(stack.len() - count))
}

fn neg_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    matches!(
        ty,
        FxRuntimeType::I32
            | FxRuntimeType::F32
            | FxRuntimeType::Length
            | FxRuntimeType::Angle
            | FxRuntimeType::Seconds
            | FxRuntimeType::Vec2
    )
    .then_some(ty)
}

fn abs_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    neg_result(ty)
}

fn add_sub_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == right).then(|| neg_result(left)).flatten()
}

fn mul_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    match (left, right) {
        (FxRuntimeType::I32, FxRuntimeType::I32) => Some(FxRuntimeType::I32),
        (FxRuntimeType::F32, FxRuntimeType::F32) => Some(FxRuntimeType::F32),
        (unit, FxRuntimeType::F32) | (FxRuntimeType::F32, unit)
            if matches!(
                unit,
                FxRuntimeType::Length
                    | FxRuntimeType::Angle
                    | FxRuntimeType::Seconds
                    | FxRuntimeType::Vec2
            ) =>
        {
            Some(unit)
        }
        _ => None,
    }
}

fn div_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    match (left, right) {
        (FxRuntimeType::I32, FxRuntimeType::I32) => Some(FxRuntimeType::I32),
        (FxRuntimeType::F32, FxRuntimeType::F32) => Some(FxRuntimeType::F32),
        (unit, FxRuntimeType::F32)
            if matches!(
                unit,
                FxRuntimeType::Length
                    | FxRuntimeType::Angle
                    | FxRuntimeType::Seconds
                    | FxRuntimeType::Vec2
            ) =>
        {
            Some(unit)
        }
        (left, right)
            if left == right
                && matches!(
                    left,
                    FxRuntimeType::Length | FxRuntimeType::Angle | FxRuntimeType::Seconds
                ) =>
        {
            Some(FxRuntimeType::F32)
        }
        _ => None,
    }
}

fn order_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == right
        && matches!(
            left,
            FxRuntimeType::I32
                | FxRuntimeType::F32
                | FxRuntimeType::Length
                | FxRuntimeType::Angle
                | FxRuntimeType::Seconds
        ))
    .then_some(left)
}

fn trig_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    matches!(ty, FxRuntimeType::F32 | FxRuntimeType::Angle).then_some(FxRuntimeType::F32)
}

fn f32_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::F32).then_some(FxRuntimeType::F32)
}

fn bool_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::Bool).then_some(FxRuntimeType::Bool)
}

fn bools_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == FxRuntimeType::Bool && right == FxRuntimeType::Bool).then_some(FxRuntimeType::Bool)
}

fn equal_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == right).then_some(FxRuntimeType::Bool)
}

fn compare_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    order_result(left, right).map(|_| FxRuntimeType::Bool)
}

fn hash_noise_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::I32).then_some(FxRuntimeType::F32)
}

fn floor_to_i32_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::F32).then_some(FxRuntimeType::I32)
}

fn bitcast_u32_to_i32_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::U32).then_some(FxRuntimeType::I32)
}

fn seconds_value_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::Seconds).then_some(FxRuntimeType::F32)
}

fn vec2_x_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::Vec2).then_some(FxRuntimeType::F32)
}

fn vec2_y_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::Vec2).then_some(FxRuntimeType::F32)
}

#[cfg(test)]
mod canonical_decode_tests {
    use super::*;

    fn minimal_bytes(
        return_type: FxRuntimeType,
        instruction_count: u8,
        instructions: &[u8],
    ) -> Vec<u8> {
        let mut bytes = b"arcweft.fx-sampler-program\0".to_vec();
        bytes.extend_from_slice(&[1, 0, 0, return_type as u8, instruction_count]);
        bytes.extend_from_slice(instructions);
        bytes
    }

    #[test]
    fn canonical_sampler_decode_round_trips_sealed_program() {
        let program = FxSamplerProgram::validate(
            ValueProgramSchema::new(
                vec![FxRuntimeType::F32],
                vec![FxRuntimeType::Bool],
                FxRuntimeType::I32,
            ),
            vec![
                ValueInstruction::Constant {
                    value: FxRuntimeValue::U32(u32::MAX),
                },
                ValueInstruction::BitcastU32ToI32,
                ValueInstruction::Return,
            ],
        )
        .unwrap();
        let bytes = program.canonical_v1_bytes().unwrap();
        let decoded = FxSamplerProgram::decode_canonical_v1(&bytes).unwrap();
        assert_eq!(decoded, program);
        assert_eq!(decoded.canonical_v1_bytes().unwrap(), bytes);
    }

    #[test]
    fn canonical_sampler_decode_round_trips_typed_projection_instructions() {
        let program = FxSamplerProgram::validate(
            ValueProgramSchema::new(
                vec![FxRuntimeType::Seconds, FxRuntimeType::Vec2],
                Vec::new(),
                FxRuntimeType::F32,
            ),
            vec![
                ValueInstruction::LoadParameter {
                    parameter: ValueProgramSchema::new(
                        vec![FxRuntimeType::Seconds, FxRuntimeType::Vec2],
                        Vec::new(),
                        FxRuntimeType::F32,
                    )
                    .parameter_ref(0)
                    .unwrap(),
                },
                ValueInstruction::SecondsValue,
                ValueInstruction::LoadParameter {
                    parameter: ValueProgramSchema::new(
                        vec![FxRuntimeType::Seconds, FxRuntimeType::Vec2],
                        Vec::new(),
                        FxRuntimeType::F32,
                    )
                    .parameter_ref(1)
                    .unwrap(),
                },
                ValueInstruction::Vec2X,
                ValueInstruction::Add,
                ValueInstruction::LoadParameter {
                    parameter: ValueProgramSchema::new(
                        vec![FxRuntimeType::Seconds, FxRuntimeType::Vec2],
                        Vec::new(),
                        FxRuntimeType::F32,
                    )
                    .parameter_ref(1)
                    .unwrap(),
                },
                ValueInstruction::Vec2Y,
                ValueInstruction::Sub,
                ValueInstruction::Return,
            ],
        )
        .unwrap();
        let bytes = program.canonical_v1_bytes().unwrap();
        let decoded = FxSamplerProgram::decode_canonical_v1(&bytes).unwrap();
        assert_eq!(decoded, program);
        assert_eq!(decoded.canonical_v1_bytes().unwrap(), bytes);
    }

    #[test]
    fn typed_projection_instructions_validate_their_source_units() {
        for (input, instruction, operation) in [
            (
                FxRuntimeType::F32,
                ValueInstruction::SecondsValue,
                "seconds_value",
            ),
            (FxRuntimeType::F32, ValueInstruction::Vec2X, "vec2_x"),
            (FxRuntimeType::F32, ValueInstruction::Vec2Y, "vec2_y"),
        ] {
            let schema = ValueProgramSchema::new(vec![input], Vec::new(), FxRuntimeType::F32);
            let error = FxSamplerProgram::validate(
                schema.clone(),
                vec![
                    ValueInstruction::LoadParameter {
                        parameter: schema.parameter_ref(0).unwrap(),
                    },
                    instruction,
                    ValueInstruction::Return,
                ],
            )
            .unwrap_err();
            assert_eq!(
                error,
                ValueProgramValidationError::InvalidOperands {
                    instruction: 1,
                    operation,
                    operands: vec![input],
                }
            );
        }
    }

    #[test]
    fn canonical_sampler_decode_rejects_unknown_tags_and_noncanonical_values() {
        let mut unknown_type = minimal_bytes(FxRuntimeType::Bool, 2, &[0, 0, 1, 29]);
        let return_type = b"arcweft.fx-sampler-program\0".len() + 3;
        unknown_type[return_type] = 0xff;
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&unknown_type),
            Err(FxSamplerProgramDecodeError::RuntimeValue(
                FxRuntimeValueDecodeError::UnknownRuntimeType(0xff)
            ))
        );

        let unknown_opcode = minimal_bytes(FxRuntimeType::Bool, 1, &[0xff]);
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&unknown_opcode),
            Err(FxSamplerProgramDecodeError::UnknownOpcode(0xff))
        );

        let mut negative_zero = minimal_bytes(FxRuntimeType::F32, 2, &[]);
        negative_zero.extend_from_slice(&[0, FxRuntimeType::F32 as u8]);
        negative_zero.extend_from_slice(&(-0.0_f32).to_bits().to_le_bytes());
        negative_zero.push(29);
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&negative_zero),
            Err(FxSamplerProgramDecodeError::RuntimeValue(
                FxRuntimeValueDecodeError::NonCanonicalFloat {
                    bits: (-0.0_f32).to_bits()
                }
            ))
        );
    }

    #[test]
    fn canonical_sampler_decode_rejects_limits_varints_truncation_and_trailing() {
        let mut too_many_parameters = b"arcweft.fx-sampler-program\0".to_vec();
        too_many_parameters.extend_from_slice(&[1, 65]);
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&too_many_parameters),
            Err(FxSamplerProgramDecodeError::OwnerLimit {
                kind: "parameter",
                actual: 65,
                limit: FX_MAX_CAPTURED_OR_PARAMETER_SLOTS
            })
        );

        let mut overlong = minimal_bytes(FxRuntimeType::Bool, 2, &[0, 0, 1, 29]);
        let instruction_count = b"arcweft.fx-sampler-program\0".len() + 4;
        overlong.splice(instruction_count..=instruction_count, [0x81, 0x00]);
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&overlong),
            Err(FxSamplerProgramDecodeError::Canonical(
                FxCanonicalDecodeError::NonCanonicalVarint
            ))
        );

        let mut truncated = minimal_bytes(FxRuntimeType::Bool, 2, &[0, 0, 1, 29]);
        truncated.pop();
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&truncated),
            Err(FxSamplerProgramDecodeError::Canonical(
                FxCanonicalDecodeError::Truncated
            ))
        );

        let mut trailing = minimal_bytes(FxRuntimeType::Bool, 2, &[0, 0, 1, 29]);
        trailing.push(0);
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&trailing),
            Err(FxSamplerProgramDecodeError::Canonical(
                FxCanonicalDecodeError::TrailingBytes(1)
            ))
        );

        let no_return = minimal_bytes(FxRuntimeType::Bool, 0, &[]);
        assert_eq!(
            FxSamplerProgram::decode_canonical_v1(&no_return),
            Err(FxSamplerProgramDecodeError::Validation(
                ValueProgramValidationError::MissingReturn
            ))
        );
    }
}
