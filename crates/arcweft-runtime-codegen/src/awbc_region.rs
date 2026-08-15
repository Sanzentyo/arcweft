//! Baseline full-script compiled-region lowering over verified AWBC.
//!
//! The first backend is intentionally a safe Rust baseline region. It does not
//! allocate executable memory and does not perform host I/O. Eligible regions are
//! deterministic, effect-free AWBC block ranges; unsupported operations return a
//! zero-budget VM fallback request.

use crate::artifact::CodeRegionId;
use crate::policy::ProgramGenerationId;
use crate::region::{
    COMPILED_REGION_ABI_VERSION, CompiledExecutionIdentity, CompiledFallback,
    CompiledFallbackReason, CompiledRegion, CompiledRegionEntry, CompiledRegionInput,
    CompiledRegionMetadata, CompiledRegionResult, CompiledStepExit, RuntimeFailure,
};
use arcweft_core::awbc::schema::{
    AwbcBlockId, AwbcDigest, AwbcFunctionId, AwbcOpcode, AwbcProgram, AwbcTerminator, AwbcTrapCode,
};
use arcweft_core::awbc::vm::{RejectingVmHost, VmExit, VmStepOptions, step_with_host};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwbcRegionLowerOptions {
    pub generation: ProgramGenerationId,
    pub program_digest: AwbcDigest,
    pub allow_host_boundaries: bool,
    pub max_region_blocks: usize,
}

impl Default for AwbcRegionLowerOptions {
    fn default() -> Self {
        Self {
            generation: ProgramGenerationId(0),
            program_digest: AwbcDigest::default(),
            allow_host_boundaries: false,
            max_region_blocks: 64,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AwbcRegionLowerReport {
    pub regions: Vec<Arc<BaselineAwbcRegion>>,
    pub rejected: Vec<RejectedRegion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedRegion {
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    pub reason: CompiledFallbackReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaselineAwbcRegion {
    metadata: CompiledRegionMetadata,
    entry: BaselineRegionEntry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BaselineRegionEntry {
    function: AwbcFunctionId,
    block: AwbcBlockId,
}

pub fn lower_awbc_regions(
    program: &AwbcProgram,
    options: &AwbcRegionLowerOptions,
) -> AwbcRegionLowerReport {
    let mut report = AwbcRegionLowerReport::default();
    for (function_index, function) in program.functions.iter().enumerate() {
        let function_id = AwbcFunctionId(table_index(function_index));
        let entry_block = function.entry_block;
        match eligible_function(program, function_id, entry_block, options) {
            Ok(()) => {
                let metadata = CompiledRegionMetadata {
                    abi_version: COMPILED_REGION_ABI_VERSION,
                    region: CodeRegionId(table_index(report.regions.len())),
                    program_digest: options.program_digest,
                    runtime_layout_digest: program.header.runtime_layout_digest,
                    host_abi_digest: program.header.host_abi_digest,
                    entries: vec![CompiledRegionEntry {
                        function: function_id,
                        block: entry_block,
                        resume: None,
                    }],
                };
                report.regions.push(Arc::new(BaselineAwbcRegion {
                    metadata,
                    entry: BaselineRegionEntry {
                        function: function_id,
                        block: entry_block,
                    },
                }));
            }
            Err(reason) => report.rejected.push(RejectedRegion {
                function: function_id,
                block: entry_block,
                reason,
            }),
        }
    }
    report
}

fn eligible_function(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    entry: AwbcBlockId,
    options: &AwbcRegionLowerOptions,
) -> Result<(), CompiledFallbackReason> {
    let function_record = program
        .functions
        .get(function.index())
        .ok_or(CompiledFallbackReason::RegionNotFound)?;
    if function_record.blocks.len as usize > options.max_region_blocks {
        return Err(CompiledFallbackReason::BudgetPreemption);
    }
    let start = function_record.blocks.start as usize;
    let end = function_record
        .blocks
        .checked_end()
        .ok_or(CompiledFallbackReason::ArtifactRejected)? as usize;
    for block in &program.blocks[start..end] {
        if block.owner != function {
            return Err(CompiledFallbackReason::ArtifactRejected);
        }
        for id in block.instructions.start
            ..block
                .instructions
                .checked_end()
                .unwrap_or(block.instructions.start)
        {
            let instruction = program
                .instructions
                .get(id as usize)
                .ok_or(CompiledFallbackReason::ArtifactRejected)?;
            if !opcode_eligible(instruction.opcode(), options) {
                return Err(CompiledFallbackReason::UnsupportedOpcode(
                    instruction.opcode(),
                ));
            }
        }
        if !terminator_eligible(&block.terminator, options) {
            return Err(CompiledFallbackReason::UnsupportedOpcode(
                block.terminator.opcode(),
            ));
        }
    }
    if entry.index() < start || entry.index() >= end {
        return Err(CompiledFallbackReason::RegionNotFound);
    }
    Ok(())
}

fn opcode_eligible(opcode: AwbcOpcode, options: &AwbcRegionLowerOptions) -> bool {
    match opcode {
        AwbcOpcode::Nop
        | AwbcOpcode::LoadConst
        | AwbcOpcode::Move
        | AwbcOpcode::Clear
        | AwbcOpcode::EnterScope
        | AwbcOpcode::ExitScope
        | AwbcOpcode::BindPattern
        | AwbcOpcode::TestPattern
        | AwbcOpcode::MakeTuple
        | AwbcOpcode::MakeSequence
        | AwbcOpcode::RepeatSequence
        | AwbcOpcode::SequenceLen
        | AwbcOpcode::SequenceGet
        | AwbcOpcode::SequenceSlice
        | AwbcOpcode::SequencePush
        | AwbcOpcode::MakeRecord
        | AwbcOpcode::MakeVariant
        | AwbcOpcode::MakeAgent
        | AwbcOpcode::ProjectTuple
        | AwbcOpcode::ProjectRecord
        | AwbcOpcode::ProjectField
        | AwbcOpcode::Unary
        | AwbcOpcode::Binary
        | AwbcOpcode::CallPureHelper
        | AwbcOpcode::CallIntrinsic
        | AwbcOpcode::Drop => true,
        AwbcOpcode::EnsureContent
        | AwbcOpcode::EmitEffect
        | AwbcOpcode::StartTask
        | AwbcOpcode::SpawnFiber
        | AwbcOpcode::StreamYield
        | AwbcOpcode::StreamClose
        | AwbcOpcode::SourceClose => options.allow_host_boundaries,
        _ => false,
    }
}

fn terminator_eligible(terminator: &AwbcTerminator, options: &AwbcRegionLowerOptions) -> bool {
    match terminator {
        AwbcTerminator::Jump { .. }
        | AwbcTerminator::Branch { .. }
        | AwbcTerminator::Match { .. }
        | AwbcTerminator::Return { .. }
        | AwbcTerminator::Trap { .. }
        | AwbcTerminator::BudgetYield { .. }
        | AwbcTerminator::Unreachable
        | AwbcTerminator::CallFunction { .. }
        | AwbcTerminator::GotoStatic { .. } => true,
        AwbcTerminator::GotoDynamic { .. }
        | AwbcTerminator::Dialogue { .. }
        | AwbcTerminator::Choice { .. }
        | AwbcTerminator::Await { .. }
        | AwbcTerminator::AwaitMany { .. }
        | AwbcTerminator::HostCall { .. } => options.allow_host_boundaries,
    }
}

impl CompiledRegion for BaselineAwbcRegion {
    fn metadata(&self) -> &CompiledRegionMetadata {
        &self.metadata
    }

    fn step(&self, input: CompiledRegionInput<'_>) -> CompiledRegionResult {
        if input.fiber.cursor.function != self.entry.function
            || input.fiber.cursor.block != self.entry.block
        {
            let at = input
                .fiber
                .safe_point(None)
                .expect("fiber at entry has a safe point");
            return CompiledRegionResult {
                consumed: 0,
                exit: CompiledStepExit::FallbackToVm(CompiledFallback {
                    reason: CompiledFallbackReason::RegionNotFound,
                    at,
                }),
            };
        }
        let mut host = RejectingVmHost;
        match step_with_host(
            input.program,
            input.fiber,
            VmStepOptions {
                max_instructions: input.instruction_budget,
            },
            &mut host,
        ) {
            Ok(result) => CompiledRegionResult {
                consumed: result.executed,
                exit: map_vm_exit(input.program, input.fiber, result.exit),
            },
            Err(_error) => {
                let at = input
                    .fiber
                    .safe_point(None)
                    .expect("fiber still has a checkpointable safe point after VM error");
                CompiledRegionResult {
                    consumed: 0,
                    exit: CompiledStepExit::FallbackToVm(CompiledFallback {
                        reason: CompiledFallbackReason::ArtifactRejected,
                        at,
                    }),
                }
            }
        }
    }
}

fn map_vm_exit(
    _program: &AwbcProgram,
    fiber: &mut arcweft_core::awbc::fiber::FiberState,
    exit: VmExit,
) -> CompiledStepExit {
    match exit {
        VmExit::Running => fiber.safe_point(None).map_or_else(
            |error| failed(AwbcTrapCode::InternalInvariant, error.to_string()),
            |next| CompiledStepExit::Continue { next },
        ),
        VmExit::Suspended(reason) => fiber.suspension.clone().map_or_else(
            || {
                failed(
                    AwbcTrapCode::InternalInvariant,
                    format!("missing suspension for {reason:?}"),
                )
            },
            CompiledStepExit::Suspended,
        ),
        VmExit::Returned(value) => CompiledStepExit::Returned(value),
        VmExit::Cancelled => failed(
            AwbcTrapCode::InternalInvariant,
            "running effect-free compiled region observed cancellation".to_owned(),
        ),
        VmExit::Trapped(trap) => CompiledStepExit::Failed(RuntimeFailure {
            code: trap.code,
            message: trap.message,
            source_map: trap.source_map,
        }),
        VmExit::BudgetYield(safe_point) => safe_point.resume.map_or_else(
            || {
                failed(
                    AwbcTrapCode::InternalInvariant,
                    "budget yield has no resume point".to_owned(),
                )
            },
            |resume| CompiledStepExit::BudgetExhausted { resume },
        ),
    }
}

fn failed(code: AwbcTrapCode, message: String) -> CompiledStepExit {
    CompiledStepExit::Failed(RuntimeFailure {
        code,
        message: Some(message),
        source_map: None,
    })
}

pub fn compiled_identity(options: &AwbcRegionLowerOptions) -> CompiledExecutionIdentity {
    CompiledExecutionIdentity {
        generation: options.generation,
        program_digest: options.program_digest,
    }
}

fn table_index(value: usize) -> u32 {
    u32::try_from(value).expect("AWBC region table index exceeded u32 address space")
}
