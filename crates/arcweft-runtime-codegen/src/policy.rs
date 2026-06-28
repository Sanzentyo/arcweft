use arcweft_core::awbc::schema::AwbcDigest;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgramGenerationId(pub u64);

impl ProgramGenerationId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl RuntimeExecutorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompactVm => "compact_vm",
            Self::Jit => "jit",
            Self::NativeAot => "native_aot",
            Self::WasmAot => "wasm_aot",
        }
    }

    #[must_use]
    pub const fn is_compiled_backend(self) -> bool {
        matches!(self, Self::Jit | Self::NativeAot | Self::WasmAot)
    }
}

impl RuntimeOptimizationLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Speed => "speed",
            Self::Size => "size",
        }
    }
}

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
