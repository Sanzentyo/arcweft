use crate::artifact::{CodeRegionId, RuntimeCodeArtifactKind};
use crate::policy::{ProgramGenerationId, RuntimeOptimizationLevel};
use arcweft_core::awbc::schema::{AWBC_ABI_VERSION, AWBC_CODEC_VERSION, AwbcDigest};

pub const RUNTIME_CODE_CACHE_KEY_VERSION: u32 = 1;

/// Inputs that define one persistent compiled-artifact identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodeCacheInputs {
    pub artifact_kind: RuntimeCodeArtifactKind,
    pub program_digest: AwbcDigest,
    pub region_digest: AwbcDigest,
    pub runtime_layout_digest: AwbcDigest,
    pub host_abi_digest: AwbcDigest,
    pub target_triple: String,
    pub cpu_features_digest: AwbcDigest,
    pub wasm_features_digest: Option<AwbcDigest>,
    pub backend_id: String,
    pub backend_revision: String,
    pub optimization: RuntimeOptimizationLevel,
}

/// Complete semantic and target identity for JIT/native/Wasm artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCodeCacheKey {
    pub version: u32,
    pub artifact_kind: RuntimeCodeArtifactKind,
    pub program_digest: AwbcDigest,
    pub region_digest: AwbcDigest,
    pub awbc_abi_version: u32,
    pub awbc_codec_version: u16,
    pub runtime_layout_digest: AwbcDigest,
    pub host_abi_digest: AwbcDigest,
    pub target_triple: String,
    pub cpu_features_digest: AwbcDigest,
    pub wasm_features_digest: Option<AwbcDigest>,
    pub backend_id: String,
    pub backend_revision: String,
    pub optimization: RuntimeOptimizationLevel,
}

impl RuntimeCodeCacheKey {
    pub fn new(inputs: RuntimeCodeCacheInputs) -> Self {
        Self {
            version: RUNTIME_CODE_CACHE_KEY_VERSION,
            artifact_kind: inputs.artifact_kind,
            program_digest: inputs.program_digest,
            region_digest: inputs.region_digest,
            awbc_abi_version: AWBC_ABI_VERSION,
            awbc_codec_version: AWBC_CODEC_VERSION,
            runtime_layout_digest: inputs.runtime_layout_digest,
            host_abi_digest: inputs.host_abi_digest,
            target_triple: inputs.target_triple,
            cpu_features_digest: inputs.cpu_features_digest,
            wasm_features_digest: inputs.wasm_features_digest,
            backend_id: inputs.backend_id,
            backend_revision: inputs.backend_revision,
            optimization: inputs.optimization,
        }
    }

    pub fn digest(&self) -> AwbcDigest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.runtime-code-cache-key.v1\0");
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&[artifact_kind_tag(self.artifact_kind)]);
        hasher.update(&self.program_digest.0);
        hasher.update(&self.region_digest.0);
        hasher.update(&self.awbc_abi_version.to_le_bytes());
        hasher.update(&self.awbc_codec_version.to_le_bytes());
        hasher.update(&self.runtime_layout_digest.0);
        hasher.update(&self.host_abi_digest.0);
        update_string(&mut hasher, &self.target_triple);
        hasher.update(&self.cpu_features_digest.0);
        match self.wasm_features_digest {
            None => hasher.update(&[0]),
            Some(digest) => {
                hasher.update(&[1]);
                hasher.update(&digest.0)
            }
        };
        update_string(&mut hasher, &self.backend_id);
        update_string(&mut hasher, &self.backend_revision);
        hasher.update(&[optimization_tag(self.optimization)]);
        AwbcDigest(*hasher.finalize().as_bytes())
    }
}

/// Hot-swap dispatch identity. Generation is deliberately outside persistent
/// artifact identity so semantically identical generations can reuse code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDispatchKey {
    pub generation: ProgramGenerationId,
    pub region: CodeRegionId,
    pub artifact_digest: AwbcDigest,
}

fn update_string(hasher: &mut blake3::Hasher, value: &str) {
    let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
    hasher.update(&len.to_le_bytes());
    hasher.update(value.as_bytes());
}

const fn artifact_kind_tag(kind: RuntimeCodeArtifactKind) -> u8 {
    match kind {
        RuntimeCodeArtifactKind::Jit => 0,
        RuntimeCodeArtifactKind::NativeObject => 1,
        RuntimeCodeArtifactKind::NativeSharedLibrary => 2,
        RuntimeCodeArtifactKind::WasmModule => 3,
    }
}

const fn optimization_tag(level: RuntimeOptimizationLevel) -> u8 {
    match level {
        RuntimeOptimizationLevel::Baseline => 0,
        RuntimeOptimizationLevel::Speed => 1,
        RuntimeOptimizationLevel::Size => 2,
    }
}
