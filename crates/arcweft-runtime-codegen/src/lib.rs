//! Runtime code-generation policy and safe-region contracts.
//!
//! This crate does not lower to Cranelift, load native objects, or allocate
//! executable memory. It records the executor-neutral data needed for full
//! script AOT/JIT layers while keeping AWBC/VM execution as the semantic source
//! of truth.

use arcweft_core::value::RuntimeValue;
use std::collections::{BTreeMap, BTreeSet};

/// Program generation attached to a runtime-code artifact.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgramGenerationId(pub u64);

/// Runtime callable table identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableId(pub u32);

/// Resume point where the VM and compiled regions may exchange control.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResumePointId(pub u32);

/// Frame layout table identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FrameLayoutId(pub u32);

/// Compiled region identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CodeRegionId(pub u32);

/// Runtime host request table identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostRequestId(pub u32);

/// Deterministic digest used for semantic and backend cache keys.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCodeDigest([u8; 32]);

impl RuntimeCodeDigest {
    /// Creates a digest from already canonical bytes.
    pub fn of(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes).into())
    }

    /// Creates a digest from raw digest bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// User/runtime executor selection mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeExecutorMode {
    Auto,
    BytecodeVm,
    Compiled,
}

/// Pure-helper backend mode used by profile/config policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PureBackendMode {
    Auto,
    Vm,
    Aot,
    Jit,
}

/// Runtime profile class used when resolving conservative executor behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProfileKind {
    Dev,
    Test,
    Release,
    Replay,
    AgentRepl,
    Server,
    Web,
}

/// Trust class for executable artifacts and runtime-generated code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeTrustPolicy {
    TrustedProduct,
    LocalDev,
    UntrustedMod,
}

/// Host/platform capabilities relevant to executable code.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimePlatformCapabilities {
    pub runtime_codegen: CapabilitySupport,
    pub executable_memory: CapabilitySupport,
    pub native_aot_loading: CapabilitySupport,
    pub wasm_module_loading: CapabilitySupport,
    pub worker_threads: CapabilitySupport,
    pub cpu_features: BTreeMap<String, CapabilitySupport>,
}

/// Availability of one runtime-codegen platform capability.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CapabilitySupport {
    #[default]
    Unavailable,
    Available,
}

impl CapabilitySupport {
    /// Returns whether this capability is available.
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Code artifacts available beside the mandatory AWBC bytecode.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProgramArtifactInventory {
    pub bytecode: bool,
    pub native_aot: Vec<TargetCodeArtifact>,
    pub wasm_aot: Vec<TargetCodeArtifact>,
}

/// One target-specific compiled artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetCodeArtifact {
    pub target_triple: String,
    pub cpu_features_digest: RuntimeCodeDigest,
    pub program_digest: RuntimeCodeDigest,
    pub code_digest: RuntimeCodeDigest,
}

/// Resolved executor policy and its explanation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExecutorPolicy {
    pub selected: ResolvedExecutor,
    pub reason: ExecutorSelectionReason,
}

/// Concrete executor selected for this run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedExecutor {
    BytecodeVm,
    NativeAot,
    WasmAot,
}

/// Why an executor was selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutorSelectionReason {
    PinnedVm,
    PinnedCompiled,
    TrustedNativeAotAvailable,
    TrustedWasmAotAvailable,
    RuntimeCodegenUnavailable,
    UntrustedCodeUsesVm,
    NoMatchingArtifact,
}

impl RuntimeExecutorMode {
    /// Resolves an executor without compiling or loading code.
    pub fn resolve(
        self,
        platform: &RuntimePlatformCapabilities,
        artifacts: &ProgramArtifactInventory,
        trust: CodeTrustPolicy,
        profile: RuntimeProfileKind,
    ) -> ResolvedExecutorPolicy {
        if matches!(trust, CodeTrustPolicy::UntrustedMod) {
            return ResolvedExecutorPolicy {
                selected: ResolvedExecutor::BytecodeVm,
                reason: ExecutorSelectionReason::UntrustedCodeUsesVm,
            };
        }
        match self {
            Self::BytecodeVm => ResolvedExecutorPolicy {
                selected: ResolvedExecutor::BytecodeVm,
                reason: ExecutorSelectionReason::PinnedVm,
            },
            Self::Compiled => compiled_policy(platform, artifacts),
            Self::Auto => auto_policy(platform, artifacts, profile),
        }
    }
}

fn compiled_policy(
    platform: &RuntimePlatformCapabilities,
    artifacts: &ProgramArtifactInventory,
) -> ResolvedExecutorPolicy {
    select_available_compiled(platform, artifacts).unwrap_or(ResolvedExecutorPolicy {
        selected: ResolvedExecutor::BytecodeVm,
        reason: ExecutorSelectionReason::RuntimeCodegenUnavailable,
    })
}

fn auto_policy(
    platform: &RuntimePlatformCapabilities,
    artifacts: &ProgramArtifactInventory,
    profile: RuntimeProfileKind,
) -> ResolvedExecutorPolicy {
    if matches!(
        profile,
        RuntimeProfileKind::Test | RuntimeProfileKind::Replay
    ) {
        return ResolvedExecutorPolicy {
            selected: ResolvedExecutor::BytecodeVm,
            reason: ExecutorSelectionReason::PinnedVm,
        };
    }
    select_available_compiled(platform, artifacts).unwrap_or(ResolvedExecutorPolicy {
        selected: ResolvedExecutor::BytecodeVm,
        reason: ExecutorSelectionReason::NoMatchingArtifact,
    })
}

fn select_available_compiled(
    platform: &RuntimePlatformCapabilities,
    artifacts: &ProgramArtifactInventory,
) -> Option<ResolvedExecutorPolicy> {
    if platform.native_aot_loading.is_available() && !artifacts.native_aot.is_empty() {
        return Some(ResolvedExecutorPolicy {
            selected: ResolvedExecutor::NativeAot,
            reason: ExecutorSelectionReason::TrustedNativeAotAvailable,
        });
    }
    if platform.wasm_module_loading.is_available() && !artifacts.wasm_aot.is_empty() {
        return Some(ResolvedExecutorPolicy {
            selected: ResolvedExecutor::WasmAot,
            reason: ExecutorSelectionReason::TrustedWasmAotAvailable,
        });
    }
    None
}

/// Facts inferred before a callable is considered for compiled regions.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CallableCodegenFacts {
    pub inferred_effects: InferredEffectSet,
    pub deterministic: bool,
    pub may_suspend: bool,
    pub may_allocate: bool,
    pub dynamic_targets: DynamicTargetSet,
    pub frame_layout: FrameLayoutId,
    pub backend_support: BackendSupport,
}

/// Effect names inferred for a callable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InferredEffectSet {
    effects: BTreeSet<String>,
}

impl InferredEffectSet {
    /// Creates an effect set from stable effect identifiers.
    pub fn new(effects: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            effects: effects.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns whether no effects were inferred.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Iterates effect identifiers in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.effects.iter().map(String::as_str)
    }
}

/// Dynamic dispatch targets observed for a callable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DynamicTargetSet {
    targets: BTreeSet<CallableId>,
}

impl DynamicTargetSet {
    /// Creates a target set.
    pub fn new(targets: impl IntoIterator<Item = CallableId>) -> Self {
        Self {
            targets: targets.into_iter().collect(),
        }
    }

    /// Returns whether no dynamic targets are present.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

/// Backend support result for a callable or region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendSupport {
    Supported,
    Unsupported(Vec<CodegenFallbackReason>),
}

impl Default for BackendSupport {
    fn default() -> Self {
        Self::Unsupported(Vec::new())
    }
}

/// Executor-neutral runtime-code program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeCodeProgram {
    pub generation: ProgramGenerationId,
    pub callables: Vec<RuntimeCodeCallable>,
    pub frame_layouts: Vec<FrameLayout>,
    pub entrypoints: Vec<RuntimeCodeEntrypoint>,
}

/// Runtime-code callable split into safe regions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodeCallable {
    pub id: CallableId,
    pub frame: FrameLayoutId,
    pub facts: CallableCodegenFacts,
    pub regions: Vec<CodeRegion>,
}

/// Named runtime entry into a callable resume point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodeEntrypoint {
    pub name: String,
    pub callable: CallableId,
    pub resume: ResumePointId,
}

/// Continuation region between runtime safe points.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeRegion {
    pub id: CodeRegionId,
    pub entry: ResumePointId,
    pub blocks: Vec<BasicBlock>,
    pub semantic_digest: RuntimeCodeDigest,
}

/// Basic block placeholder shared by future bytecode and native lowerers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlock {
    pub label: String,
    pub safe_point: RuntimeSafePoint,
    pub instructions: usize,
}

/// Runtime safe point where compiled code must return or may be resumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSafePoint {
    FlowEntry,
    CallableBoundary,
    Dialogue,
    Choice,
    Await,
    AwaitMany,
    HostRequest,
    LoopBackedge,
    StepBudgetYield,
    Return,
    Trap,
}

/// Frame layout visible to both VM and compiled region calls.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameLayout {
    pub id: FrameLayoutId,
    pub slots: Vec<FrameSlot>,
}

/// One frame slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameSlot {
    pub name: String,
    pub storage: FrameSlotStorage,
}

/// Storage class for one frame slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameSlotStorage {
    Value,
    Bool,
    I64,
    U64,
    F64,
    Ref,
}

/// Structured exit from one compiled step.
#[derive(Clone, Debug, PartialEq)]
pub enum CompiledStepExit {
    Resume {
        next: ResumePointId,
    },
    HostRequest {
        request: HostRequestId,
        resume: ResumePointId,
    },
    Suspended {
        state: SuspensionState,
    },
    Returned {
        value: RuntimeValue,
    },
    BudgetExhausted {
        resume: ResumePointId,
    },
    Failed {
        error: RuntimeFailure,
    },
    FallbackToVm {
        resume: ResumePointId,
        reason: CodegenFallbackReason,
    },
}

/// Executor-neutral suspended compiled state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuspensionState {
    pub resume: ResumePointId,
    pub reason: SuspensionReason,
}

/// Why a compiled region suspended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuspensionReason {
    Await,
    AwaitMany,
    Dialogue,
    Choice,
    HostRequest,
}

/// Runtime failure reported by compiled code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFailure {
    pub message: String,
}

/// Reason a region or executor must fall back to the VM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodegenFallbackReason {
    UnsupportedType(String),
    UnsupportedExpression(String),
    MaySuspend,
    HostIo,
    DynamicTarget,
    BackendUnavailable,
    BudgetExhausted,
}

/// Cache key for host-local runtime codegen artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodeCacheKey {
    pub semantic_digest: RuntimeCodeDigest,
    pub target_triple: String,
    pub cpu_features_digest: RuntimeCodeDigest,
    pub backend_revision: String,
    pub optimization_level: OptimizationLevel,
}

/// Baseline and optimized runtime-code tiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptimizationLevel {
    Baseline,
    Optimized,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> RuntimeCodeDigest {
        RuntimeCodeDigest::of(label.as_bytes())
    }

    #[test]
    fn untrusted_code_always_uses_vm() {
        let platform = RuntimePlatformCapabilities {
            native_aot_loading: CapabilitySupport::Available,
            ..RuntimePlatformCapabilities::default()
        };
        let artifacts = ProgramArtifactInventory {
            bytecode: true,
            native_aot: vec![TargetCodeArtifact {
                target_triple: "x86_64-pc-windows-msvc".to_owned(),
                cpu_features_digest: digest("cpu"),
                program_digest: digest("program"),
                code_digest: digest("code"),
            }],
            wasm_aot: Vec::new(),
        };

        let resolved = RuntimeExecutorMode::Auto.resolve(
            &platform,
            &artifacts,
            CodeTrustPolicy::UntrustedMod,
            RuntimeProfileKind::Dev,
        );

        assert_eq!(resolved.selected, ResolvedExecutor::BytecodeVm);
        assert_eq!(
            resolved.reason,
            ExecutorSelectionReason::UntrustedCodeUsesVm
        );
    }

    #[test]
    fn test_and_replay_profiles_pin_vm_in_auto_mode() {
        let platform = RuntimePlatformCapabilities {
            native_aot_loading: CapabilitySupport::Available,
            ..RuntimePlatformCapabilities::default()
        };
        let artifacts = ProgramArtifactInventory {
            bytecode: true,
            native_aot: vec![TargetCodeArtifact {
                target_triple: "x86_64-pc-windows-msvc".to_owned(),
                cpu_features_digest: digest("cpu"),
                program_digest: digest("program"),
                code_digest: digest("code"),
            }],
            wasm_aot: Vec::new(),
        };

        let resolved = RuntimeExecutorMode::Auto.resolve(
            &platform,
            &artifacts,
            CodeTrustPolicy::TrustedProduct,
            RuntimeProfileKind::Replay,
        );

        assert_eq!(resolved.selected, ResolvedExecutor::BytecodeVm);
        assert_eq!(resolved.reason, ExecutorSelectionReason::PinnedVm);
    }

    #[test]
    fn trusted_native_aot_is_selected_when_available() {
        let platform = RuntimePlatformCapabilities {
            native_aot_loading: CapabilitySupport::Available,
            ..RuntimePlatformCapabilities::default()
        };
        let artifacts = ProgramArtifactInventory {
            bytecode: true,
            native_aot: vec![TargetCodeArtifact {
                target_triple: "x86_64-pc-windows-msvc".to_owned(),
                cpu_features_digest: digest("cpu"),
                program_digest: digest("program"),
                code_digest: digest("code"),
            }],
            wasm_aot: Vec::new(),
        };

        let resolved = RuntimeExecutorMode::Compiled.resolve(
            &platform,
            &artifacts,
            CodeTrustPolicy::TrustedProduct,
            RuntimeProfileKind::Release,
        );

        assert_eq!(resolved.selected, ResolvedExecutor::NativeAot);
        assert_eq!(
            resolved.reason,
            ExecutorSelectionReason::TrustedNativeAotAvailable
        );
    }

    #[test]
    fn runtime_code_program_records_safe_region_boundaries() {
        let program = RuntimeCodeProgram {
            generation: ProgramGenerationId(7),
            callables: vec![RuntimeCodeCallable {
                id: CallableId(1),
                frame: FrameLayoutId(2),
                facts: CallableCodegenFacts {
                    deterministic: true,
                    backend_support: BackendSupport::Supported,
                    ..CallableCodegenFacts::default()
                },
                regions: vec![CodeRegion {
                    id: CodeRegionId(3),
                    entry: ResumePointId(4),
                    blocks: vec![BasicBlock {
                        label: "entry".to_owned(),
                        safe_point: RuntimeSafePoint::LoopBackedge,
                        instructions: 5,
                    }],
                    semantic_digest: digest("region"),
                }],
            }],
            frame_layouts: vec![FrameLayout {
                id: FrameLayoutId(2),
                slots: vec![FrameSlot {
                    name: "score".to_owned(),
                    storage: FrameSlotStorage::I64,
                }],
            }],
            entrypoints: vec![RuntimeCodeEntrypoint {
                name: "main".to_owned(),
                callable: CallableId(1),
                resume: ResumePointId(4),
            }],
        };

        assert_eq!(
            program.callables[0].regions[0].blocks[0].safe_point,
            RuntimeSafePoint::LoopBackedge
        );
        assert_eq!(program.entrypoints[0].callable, CallableId(1));
    }
}
