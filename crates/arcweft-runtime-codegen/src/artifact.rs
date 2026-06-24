use crate::cache::RuntimeCodeCacheKey;
use crate::policy::{ProgramGenerationId, RuntimeExecutorKind};
use arcweft_core::awbc::schema::{
    AwbcBlockId, AwbcDigest, AwbcFunctionId, AwbcOpcode, AwbcResumePointId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CodeRegionId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodeProgram {
    pub generation: ProgramGenerationId,
    pub program_digest: AwbcDigest,
    pub regions: Vec<CodeRegion>,
    pub artifacts: Vec<RuntimeCodeArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeRegion {
    pub id: CodeRegionId,
    pub function: AwbcFunctionId,
    pub entry_block: AwbcBlockId,
    pub entry_resume_points: Vec<AwbcResumePointId>,
    pub supported_opcodes: AwbcOpcodeSet,
    pub semantic_digest: AwbcDigest,
    pub contract: CodeRegionContract,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "compiled-region contracts are stable capability flags, not mutually exclusive states"
)]
pub struct CodeRegionContract {
    pub may_suspend: bool,
    pub may_request_host: bool,
    pub has_dynamic_target: bool,
    pub stages_external_effects: bool,
}

/// Typed 256-bit opcode inventory. Unknown or unsupported operations never
/// silently enter a compiled region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AwbcOpcodeSet(pub [u64; 4]);

impl Default for AwbcOpcodeSet {
    fn default() -> Self {
        Self::empty()
    }
}

impl AwbcOpcodeSet {
    pub const fn empty() -> Self {
        Self([0; 4])
    }

    pub fn insert(&mut self, opcode: AwbcOpcode) {
        let encoded = usize::from(opcode.encoded());
        self.0[encoded / 64] |= 1_u64 << (encoded % 64);
    }

    pub const fn contains(self, opcode: AwbcOpcode) -> bool {
        let encoded = opcode.encoded() as usize;
        self.0[encoded / 64] & (1_u64 << (encoded % 64)) != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodeArtifact {
    pub kind: RuntimeCodeArtifactKind,
    pub region: CodeRegionId,
    pub cache_key: RuntimeCodeCacheKey,
    pub content_digest: AwbcDigest,
    pub byte_len: u64,
    pub capabilities: RuntimeArtifactCapabilities,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeCodeArtifactKind {
    Jit,
    NativeObject,
    NativeSharedLibrary,
    WasmModule,
}

impl RuntimeCodeArtifactKind {
    pub const fn executor(self) -> RuntimeExecutorKind {
        match self {
            Self::Jit => RuntimeExecutorKind::Jit,
            Self::NativeObject | Self::NativeSharedLibrary => RuntimeExecutorKind::NativeAot,
            Self::WasmModule => RuntimeExecutorKind::WasmAot,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "artifact metadata advertises independent executor capabilities"
)]
pub struct RuntimeArtifactCapabilities {
    pub baseline_full_script: bool,
    pub suspension: bool,
    pub host_requests: bool,
    pub dynamic_targets: bool,
}
