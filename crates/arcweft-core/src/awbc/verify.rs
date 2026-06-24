//! Semantic verifier for canonical AWBC programs.

mod code;
mod structure;

use super::schema::{AWBC_ABI_VERSION, AwbcDigest, AwbcProgram};
use std::collections::BTreeSet;
use thiserror::Error;

/// Non-codec verifier limits. Decode limits must be enforced first for bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AwbcVerifyBudget {
    pub frame_slots_per_function: usize,
    pub params_per_signature: usize,
    pub args_per_call: usize,
    pub cfg_edges: usize,
    pub pattern_depth: usize,
    pub dataflow_steps: usize,
    pub source_span_bytes: u32,
}

impl Default for AwbcVerifyBudget {
    fn default() -> Self {
        Self {
            frame_slots_per_function: 65_536,
            params_per_signature: 4_096,
            args_per_call: 4_096,
            cfg_edges: 16_000_000,
            pattern_depth: 64,
            dataflow_steps: 32_000_000,
            source_span_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Host/product policy supplied as data; verification remains Sans I/O.
#[derive(Clone, Copy, Debug)]
pub struct AwbcVerifyContext<'a> {
    pub runtime_abi_version: u32,
    pub supported_feature_bits: u64,
    pub expected_host_abi_digest: Option<AwbcDigest>,
    pub allowed_capabilities: Option<&'a BTreeSet<String>>,
    pub allowed_effects: Option<&'a BTreeSet<String>>,
    pub require_entrypoint: bool,
}

impl Default for AwbcVerifyContext<'_> {
    fn default() -> Self {
        Self {
            runtime_abi_version: AWBC_ABI_VERSION,
            supported_feature_bits: 0,
            expected_host_abi_digest: None,
            allowed_capabilities: None,
            allowed_effects: None,
            require_entrypoint: true,
        }
    }
}

/// Stable semantic verification diagnostics.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwbcVerifyError {
    #[error("unsupported AWBC ABI version {actual}; expected {expected}")]
    UnsupportedAbi { actual: u32, expected: u32 },
    #[error("AWBC requires runtime ABI {required}, but runtime provides {actual}")]
    RuntimeAbiTooOld { required: u32, actual: u32 },
    #[error("AWBC uses unsupported feature bits 0x{unsupported:016x}")]
    UnsupportedFeatureBits { unsupported: u64 },
    #[error("AWBC host ABI digest does not match the runtime catalog")]
    HostAbiDigestMismatch,
    #[error("AWBC string table is not strictly sorted at index {index}")]
    NonCanonicalStringTable { index: usize },
    #[error("AWBC is missing a required public entrypoint")]
    MissingEntrypoint,
    #[error("AWBC `{table}` index {index} is out of bounds while checking {at}")]
    IndexOutOfBounds {
        table: &'static str,
        index: u32,
        at: String,
    },
    #[error("AWBC `{table}` range {start}..+{len} is out of bounds while checking {at}")]
    RangeOutOfBounds {
        table: &'static str,
        start: u32,
        len: u32,
        at: String,
    },
    #[error("AWBC table `{table}` item {index} is not owned exactly once")]
    InvalidTableOwnership { table: &'static str, index: usize },
    #[error("AWBC function {function} entry block {block} is outside its block range")]
    EntryBlockOutsideFunction { function: usize, block: u32 },
    #[error("AWBC block {block} owner {actual} does not match function {expected}")]
    BlockOwnerMismatch {
        block: usize,
        actual: u32,
        expected: usize,
    },
    #[error("AWBC function {function} parameter frame does not match signature")]
    ParameterLayoutMismatch { function: usize },
    #[error("AWBC frame layout {layout} exceeds `{budget}` budget")]
    FrameBudgetExceeded { layout: usize, budget: &'static str },
    #[error("AWBC signature {signature} exceeds `{budget}` budget")]
    SignatureBudgetExceeded {
        signature: usize,
        budget: &'static str,
    },
    #[error("AWBC effect set {effect_set} is not strictly sorted and unique")]
    NonCanonicalEffectSet { effect_set: usize },
    #[error("AWBC capability `{capability}` is not allowed")]
    CapabilityDenied { capability: String },
    #[error("AWBC effect `{effect}` is not allowed")]
    EffectDenied { effect: String },
    #[error("AWBC pattern graph contains a cycle at pattern {pattern}")]
    PatternCycle { pattern: usize },
    #[error("AWBC pattern graph exceeds depth {limit} at pattern {pattern}")]
    PatternDepthExceeded { pattern: usize, limit: usize },
    #[error("AWBC register {register} is out of bounds in function {function}, block {block}")]
    RegisterOutOfBounds {
        function: usize,
        block: usize,
        register: u32,
    },
    #[error("AWBC register {register} may be uninitialized in function {function}, block {block}")]
    UninitializedRegister {
        function: usize,
        block: usize,
        register: u32,
    },
    #[error("AWBC scope operation is invalid in function {function}, block {block}: {message}")]
    ScopeDiscipline {
        function: usize,
        block: usize,
        message: String,
    },
    #[error(
        "AWBC control-flow target block {target} escapes function {function} from block {block}"
    )]
    ControlFlowEscapesFunction {
        function: usize,
        block: usize,
        target: u32,
    },
    #[error("AWBC block {block} in function {function} is unreachable")]
    UnreachableBlock { function: usize, block: usize },
    #[error("AWBC loop backedge {block}->{target} is missing a loop safe point")]
    BackedgeWithoutSafePoint { block: usize, target: u32 },
    #[error("AWBC resume point {resume} does not match {at}")]
    ResumePointMismatch { resume: u32, at: String },
    #[error("AWBC safe point for block {block} is {actual:?}, expected {expected:?}")]
    SafePointMismatch {
        block: usize,
        actual: super::schema::AwbcSafePointKind,
        expected: super::schema::AwbcSafePointKind,
    },
    #[error("AWBC type mismatch at {at}: expected type {expected}, found {actual}")]
    TypeMismatch {
        at: String,
        expected: u32,
        actual: u32,
    },
    #[error("AWBC callable argument count mismatch at {at}: expected {expected}, found {actual}")]
    ArgumentCountMismatch {
        at: String,
        expected: usize,
        actual: usize,
    },
    #[error("AWBC callable result shape does not match destination at {at}")]
    ResultShapeMismatch { at: String },
    #[error("AWBC function {caller} does not declare all effects required by {callee}")]
    EffectSetMismatch { caller: usize, callee: String },
    #[error("AWBC entry {entry} signature does not match target function {function}")]
    EntrypointSignatureMismatch { entry: usize, function: u32 },
    #[error("AWBC source-map entry {entry} has invalid span {start}..{end}")]
    InvalidSourceSpan { entry: usize, start: u32, end: u32 },
    #[error("AWBC source/display map has duplicate identity at entry {entry}")]
    DuplicateMapIdentity { entry: usize },
    #[error("AWBC verifier exceeds `{budget}` budget")]
    BudgetExceeded { budget: &'static str },
    #[error("AWBC semantic invariant failed at {at}: {message}")]
    InvalidInvariant { at: String, message: String },
}

impl AwbcProgram {
    pub fn verify(
        &self,
        budget: AwbcVerifyBudget,
        context: AwbcVerifyContext<'_>,
    ) -> Result<(), AwbcVerifyError> {
        structure::verify_program(self, budget, context)
    }
}
