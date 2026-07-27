//! Private typed identity construction for runtime assertion lowering.

use arcweft_core::effect::{RuntimeAssertionGuardId, RuntimeAssertionProfile};
use arcweft_lang_hir::symbol::{CallableDeclarationId, CallablePackageId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use crate::assertion_identity::AssertionConditionIndex;

const RUNTIME_ASSERTION_GUARD_SCHEMA: u16 = 1;
const RUNTIME_ASSERTION_GUARD_CONTEXT: &str = "arcweft.runtime.assertion-guard.v1";

/// Canonical typed inputs for one artifact-stable runtime assertion guard.
///
/// This remains private until final HIR assertion lowering can build the seed
/// from one accepted module snapshot. In particular, no source spelling,
/// materialized condition label, or runtime message participates in identity.
struct RuntimeAssertionGuardSeed<'a> {
    schema: u16,
    package: &'a CallablePackageId,
    module: &'a CanonicalModulePath,
    callable: &'a CallableDeclarationId,
    assertion_ordinal: u32,
    condition: AssertionConditionIndex,
    profile: RuntimeAssertionProfile,
}

impl<'a> RuntimeAssertionGuardSeed<'a> {
    fn new(
        package: &'a CallablePackageId,
        module: &'a CanonicalModulePath,
        callable: &'a CallableDeclarationId,
        assertion_ordinal: u32,
        condition: AssertionConditionIndex,
        profile: RuntimeAssertionProfile,
    ) -> Self {
        Self {
            schema: RUNTIME_ASSERTION_GUARD_SCHEMA,
            package,
            module,
            callable,
            assertion_ordinal,
            condition,
            profile,
        }
    }

    fn derive(&self) -> RuntimeAssertionGuardId {
        let mut hasher = blake3::Hasher::new_derive_key(RUNTIME_ASSERTION_GUARD_CONTEXT);
        hasher.update(&self.schema.to_le_bytes());
        hash_text(&mut hasher, self.package.as_str());
        hash_module(&mut hasher, self.module);
        hash_callable(&mut hasher, self.callable);
        hasher.update(&self.assertion_ordinal.to_le_bytes());
        hasher.update(&[self.condition.get()]);
        hasher.update(&[match self.profile {
            RuntimeAssertionProfile::Always => 0,
            RuntimeAssertionProfile::DebugOnly => 1,
        }]);

        let digest = hasher.finalize();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest.as_bytes()[..16]);
        if bytes == [0; 16] {
            bytes[15] = 1;
        }
        RuntimeAssertionGuardId::try_from_bytes(bytes)
            .expect("runtime assertion guard derivation replaces the reserved zero value")
    }
}

pub(crate) fn derive_runtime_assertion_guard(
    package: &CallablePackageId,
    module: &CanonicalModulePath,
    callable: &CallableDeclarationId,
    assertion_ordinal: u32,
    condition: AssertionConditionIndex,
    profile: RuntimeAssertionProfile,
) -> RuntimeAssertionGuardId {
    RuntimeAssertionGuardSeed::new(
        package,
        module,
        callable,
        assertion_ordinal,
        condition,
        profile,
    )
    .derive()
}

fn hash_callable(hasher: &mut blake3::Hasher, callable: &CallableDeclarationId) {
    hash_text(hasher, callable.package().as_str());
    hash_module(hasher, callable.module());
    hash_text(hasher, callable.owner().as_str());
    hash_len(hasher, callable.owner_path().len());
    for segment in callable.owner_path() {
        hash_text(hasher, segment.as_str());
    }
    hash_text(hasher, callable.name());
}

fn hash_module(hasher: &mut blake3::Hasher, module: &CanonicalModulePath) {
    hash_len(hasher, module.segments().len());
    for segment in module.segments() {
        hash_text(hasher, segment.as_str());
    }
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_len(hasher: &mut blake3::Hasher, len: usize) {
    hasher.update(&(len as u64).to_le_bytes());
}
