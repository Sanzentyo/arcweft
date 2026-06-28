use crate::artifact::{CodeRegionId, RuntimeCodeArtifactKind};
use crate::awbc_region::{AwbcRegionLowerOptions, lower_awbc_regions};
use crate::cache::{RuntimeCodeCacheInputs, RuntimeCodeCacheKey};
use crate::policy::{ProgramGenerationId, RuntimeExecutorKind, RuntimeOptimizationLevel};
use crate::region::{
    COMPILED_REGION_ABI_VERSION, CompiledApplyError, CompiledExecutionIdentity, CompiledFallback,
    CompiledFallbackReason, CompiledRegion, CompiledRegionEntry, CompiledRegionInput,
    CompiledRegionMetadata, CompiledRegionResult, CompiledStepExit, CompiledTransition,
    execute_compiled_region,
};
use arcweft_core::awbc::fiber::{FiberCursor, FiberSafePoint, FiberState};
use arcweft_core::awbc::schema::*;

const PROGRAM_DIGEST: AwbcDigest = AwbcDigest([7; 32]);

#[derive(Clone, Copy)]
enum RegionBehavior {
    Continue,
    ReturnString,
    Fallback { consumed: u64 },
}

struct TestRegion {
    metadata: CompiledRegionMetadata,
    behavior: RegionBehavior,
}

impl CompiledRegion for TestRegion {
    fn metadata(&self) -> &CompiledRegionMetadata {
        &self.metadata
    }

    fn step(&self, input: CompiledRegionInput<'_>) -> CompiledRegionResult {
        match self.behavior {
            RegionBehavior::Continue => {
                input.fiber.line_cursor = 42;
                // Generated code cannot account budget itself; the dispatcher
                // must restore this field before charging `consumed`.
                input.fiber.budget.remaining = 1;
                CompiledRegionResult {
                    consumed: 3,
                    exit: CompiledStepExit::Continue {
                        next: FiberSafePoint {
                            generation: input.fiber.generation,
                            cursor: FiberCursor {
                                function: AwbcFunctionId(0),
                                block: AwbcBlockId(1),
                                instruction_offset: 0,
                            },
                            frame_layout: AwbcFrameLayoutId(0),
                            resume: None,
                        },
                    },
                }
            }
            RegionBehavior::ReturnString => CompiledRegionResult {
                consumed: 1,
                exit: CompiledStepExit::Returned(Some(arcweft_core::value::RuntimeValue::String(
                    "compiled".to_owned(),
                ))),
            },
            RegionBehavior::Fallback { consumed } => {
                let at = input.fiber.safe_point(None).expect("safe point");
                input.fiber.line_cursor = 99;
                CompiledRegionResult {
                    consumed,
                    exit: CompiledStepExit::FallbackToVm(CompiledFallback {
                        reason: CompiledFallbackReason::UnsupportedOpcode(AwbcOpcode::Dialogue),
                        at,
                    }),
                }
            }
        }
    }
}

fn program() -> AwbcProgram {
    AwbcProgram {
        strings: vec!["main".to_owned()],
        signatures: vec![AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: Vec::new(),
            max_scope_depth: 0,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 2),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
        }],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Jump {
                    target: AwbcBlockId(1),
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::LoopBackedge,
                source_map: None,
            },
        ],
        entries: vec![AwbcEntry {
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Game,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
        }],
        ..AwbcProgram::default()
    }
}

fn metadata(program: &AwbcProgram) -> CompiledRegionMetadata {
    metadata_at(program, AwbcFunctionId(0), AwbcBlockId(0))
}

fn metadata_at(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    block: AwbcBlockId,
) -> CompiledRegionMetadata {
    CompiledRegionMetadata {
        abi_version: COMPILED_REGION_ABI_VERSION,
        region: CodeRegionId(0),
        program_digest: PROGRAM_DIGEST,
        runtime_layout_digest: program.header.runtime_layout_digest,
        host_abi_digest: program.header.host_abi_digest,
        entries: vec![CompiledRegionEntry {
            function,
            block,
            resume: None,
        }],
    }
}

fn identity() -> CompiledExecutionIdentity {
    CompiledExecutionIdentity {
        generation: ProgramGenerationId(1),
        program_digest: PROGRAM_DIGEST,
    }
}

#[test]
fn compiled_continue_uses_dispatcher_budget_accounting() {
    let program = program();
    let region = TestRegion {
        metadata: metadata(&program),
        behavior: RegionBehavior::Continue,
    };
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 10).expect("fiber");

    let transition = execute_compiled_region(&region, identity(), &program, &mut fiber, 10)
        .expect("compiled transition");

    assert_eq!(transition, CompiledTransition::Continue);
    assert_eq!(fiber.line_cursor, 42);
    assert_eq!(fiber.cursor.block, AwbcBlockId(1));
    assert_eq!(fiber.budget.remaining, 7);
}

#[test]
fn compiled_fallback_restores_the_entry_checkpoint() {
    let program = program();
    let region = TestRegion {
        metadata: metadata(&program),
        behavior: RegionBehavior::Fallback { consumed: 0 },
    };
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 10).expect("fiber");

    let transition = execute_compiled_region(&region, identity(), &program, &mut fiber, 10)
        .expect("VM fallback");

    assert!(matches!(transition, CompiledTransition::FallbackToVm(_)));
    assert_eq!(fiber.line_cursor, 0);
    assert_eq!(fiber.cursor.block, AwbcBlockId(0));
    assert_eq!(fiber.budget.remaining, 10);
}

#[test]
fn compiled_fallback_must_not_consume_instruction_budget() {
    let program = program();
    let region = TestRegion {
        metadata: metadata(&program),
        behavior: RegionBehavior::Fallback { consumed: 1 },
    };
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 10).expect("fiber");

    let error = execute_compiled_region(&region, identity(), &program, &mut fiber, 10)
        .expect_err("fallback budget contract must fail");

    assert_eq!(
        error,
        CompiledApplyError::FallbackConsumedBudget { consumed: 1 }
    );
    assert_eq!(fiber.line_cursor, 0);
    assert_eq!(fiber.budget.remaining, 10);
}

#[test]
fn cache_key_digest_is_deterministic_and_backend_sensitive() {
    let inputs = RuntimeCodeCacheInputs {
        artifact_kind: RuntimeCodeArtifactKind::Jit,
        program_digest: AwbcDigest([1; 32]),
        region_digest: AwbcDigest([2; 32]),
        runtime_layout_digest: AwbcDigest([3; 32]),
        host_abi_digest: AwbcDigest([4; 32]),
        target_triple: "x86_64-unknown-linux-gnu".to_owned(),
        cpu_features_digest: AwbcDigest([5; 32]),
        wasm_features_digest: None,
        backend_id: "cranelift".to_owned(),
        backend_revision: "0.1".to_owned(),
        optimization: RuntimeOptimizationLevel::Baseline,
    };
    let first = RuntimeCodeCacheKey::new(inputs.clone());
    let second = RuntimeCodeCacheKey::new(inputs.clone());
    assert_eq!(first.digest(), second.digest());

    let changed = RuntimeCodeCacheKey::new(RuntimeCodeCacheInputs {
        backend_revision: "0.2".to_owned(),
        ..inputs
    });
    assert_ne!(first.digest(), changed.digest());
}

#[test]
fn compiled_nested_return_restores_caller_and_writes_destination() {
    let mut program = program();
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(1),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.frame_layouts.push(AwbcFrameLayout {
        slots: Vec::new(),
        max_scope_depth: 0,
    });
    program.signatures.push(AwbcSignature {
        params: Vec::new(),
        result: Some(AwbcTypeId(1)),
        effects: AwbcEffectSetId(0),
    });
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Synthetic,
        signature: AwbcSignatureId(1),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(2, 1),
        entry_block: AwbcBlockId(2),
        flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
    });
    program.blocks[0].terminator = AwbcTerminator::CallFunction {
        function: AwbcFunctionId(1),
        args: Vec::new(),
        dst: Some(AwbcRegisterId(0)),
        resume: AwbcResumePointId(0),
    };
    program.blocks[1].safe_point = AwbcSafePointKind::CallableBoundary;
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(1),
        frame_layout: AwbcFrameLayoutId(0),
        kind: AwbcSafePointKind::CallableBoundary,
    });

    let region = TestRegion {
        metadata: metadata_at(&program, AwbcFunctionId(1), AwbcBlockId(2)),
        behavior: RegionBehavior::ReturnString,
    };
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 10).expect("fiber");
    fiber
        .push_call_frame(
            &program,
            AwbcFunctionId(1),
            AwbcResumePointId(0),
            Some(AwbcRegisterId(0)),
        )
        .expect("push callee");

    let transition = execute_compiled_region(&region, identity(), &program, &mut fiber, 10)
        .expect("compiled return");

    assert_eq!(transition, CompiledTransition::Continue);
    assert_eq!(fiber.frames.len(), 1);
    assert_eq!(fiber.cursor.function, AwbcFunctionId(0));
    assert_eq!(fiber.cursor.block, AwbcBlockId(1));
    assert_eq!(fiber.budget.remaining, 9);
    assert_eq!(
        fiber
            .active_frame()
            .expect("caller frame")
            .register(AwbcRegisterId(0))
            .expect("return register"),
        &arcweft_core::value::RuntimeValue::String("compiled".to_owned())
    );
}

#[test]
fn awbc_region_lowering_accepts_basic_verified_flow() {
    let program = program();
    let report = lower_awbc_regions(
        &program,
        &AwbcRegionLowerOptions {
            generation: ProgramGenerationId(1),
            program_digest: PROGRAM_DIGEST,
            ..AwbcRegionLowerOptions::default()
        },
    );

    assert_eq!(report.regions.len(), 1);
    assert!(report.rejected.is_empty());
    assert_eq!(report.regions[0].metadata().program_digest, PROGRAM_DIGEST);
}

#[test]
fn awbc_region_lowering_rejects_host_boundary_without_opt_in() {
    let mut program = program();
    program.instructions.push(AwbcInstruction::EnsureContent {
        content: AwbcContentUnitId(0),
    });
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);

    let report = lower_awbc_regions(
        &program,
        &AwbcRegionLowerOptions {
            generation: ProgramGenerationId(1),
            program_digest: PROGRAM_DIGEST,
            allow_host_boundaries: false,
            ..AwbcRegionLowerOptions::default()
        },
    );

    assert!(report.regions.is_empty());
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(
        report.rejected[0].reason,
        CompiledFallbackReason::UnsupportedOpcode(AwbcOpcode::EnsureContent)
    );
}

#[test]
fn awbc_region_baseline_executes_through_compact_vm() {
    let program = program();
    let report = lower_awbc_regions(
        &program,
        &AwbcRegionLowerOptions {
            generation: ProgramGenerationId(1),
            program_digest: PROGRAM_DIGEST,
            ..AwbcRegionLowerOptions::default()
        },
    );
    let region = report.regions[0].clone();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 10).expect("fiber");

    let transition = execute_compiled_region(region.as_ref(), identity(), &program, &mut fiber, 10)
        .expect("baseline AWBC region executes");

    assert_eq!(transition, CompiledTransition::Returned(None));
    assert_eq!(
        fiber.status,
        arcweft_core::awbc::fiber::FiberStatus::Returned
    );
}

#[test]
fn runtime_codegen_policy_labels_are_stable_for_repl_tiering_status() {
    assert_eq!(ProgramGenerationId::new(7).as_u64(), 7);
    assert_eq!(RuntimeExecutorKind::CompactVm.as_str(), "compact_vm");
    assert_eq!(RuntimeExecutorKind::Jit.as_str(), "jit");
    assert!(RuntimeExecutorKind::Jit.is_compiled_backend());
    assert!(!RuntimeExecutorKind::CompactVm.is_compiled_backend());
    assert_eq!(RuntimeOptimizationLevel::Baseline.as_str(), "baseline");
}
