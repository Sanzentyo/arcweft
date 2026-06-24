use arcweft_core::awbc::schema::AwbcDigest;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgramGenerationId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeExecutorKind {
    CompactVm,
    Jit,
    NativeAot,
    WasmAot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeOptimizationLevel {
    Baseline,
    Speed,
    Size,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodegenPolicy {
    pub preferred: RuntimeExecutorKind,
    pub allow_vm_fallback: bool,
    pub optimization: RuntimeOptimizationLevel,
    pub target: RuntimeCodegenTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodegenTarget {
    pub triple: String,
    pub cpu_features_digest: AwbcDigest,
    pub wasm_features_digest: Option<AwbcDigest>,
}

impl RuntimeCodegenPolicy {
    pub fn compact_vm(target: RuntimeCodegenTarget) -> Self {
        Self {
            preferred: RuntimeExecutorKind::CompactVm,
            allow_vm_fallback: true,
            optimization: RuntimeOptimizationLevel::Baseline,
            target,
        }
    }
}
