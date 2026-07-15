//! Runtime emission policy for typed assertion statements.

use arcweft_core::effect::RuntimeAssertionProfile;
use arcweft_lang_hir::syntax::assertion::AssertionMode;

/// Build-time selection for debug assertion instructions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeAssertionBuildProfile {
    /// Retain `assert.debug` guards in the executable plan.
    #[default]
    Debug,
    /// Omit `assert.debug` guards and their condition evaluation entirely.
    Release,
}

/// Runtime-plan disposition selected for one typed assertion mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AssertionLoweringDisposition {
    /// The verifier must discharge the condition before code generation.
    CompileTimeProof,
    /// Emit one short-circuit runtime guard per authored condition.
    RuntimeGuard(RuntimeAssertionProfile),
    /// Emit no instruction and do not evaluate authored conditions.
    Omit,
}

impl RuntimeAssertionBuildProfile {
    /// Selects the executable disposition without inspecting source spellings.
    pub(crate) const fn disposition(self, mode: AssertionMode) -> AssertionLoweringDisposition {
        match mode {
            AssertionMode::Prove => AssertionLoweringDisposition::CompileTimeProof,
            AssertionMode::Check => {
                AssertionLoweringDisposition::RuntimeGuard(RuntimeAssertionProfile::Always)
            }
            AssertionMode::Debug if matches!(self, Self::Debug) => {
                AssertionLoweringDisposition::RuntimeGuard(RuntimeAssertionProfile::DebugOnly)
            }
            AssertionMode::Debug => AssertionLoweringDisposition::Omit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AssertionLoweringDisposition, RuntimeAssertionBuildProfile};
    use arcweft_core::effect::RuntimeAssertionProfile;
    use arcweft_lang_hir::syntax::assertion::AssertionMode;

    #[test]
    fn build_profile_maps_typed_modes_to_runtime_dispositions() {
        assert_eq!(
            RuntimeAssertionBuildProfile::Debug.disposition(AssertionMode::Prove),
            AssertionLoweringDisposition::CompileTimeProof
        );
        assert_eq!(
            RuntimeAssertionBuildProfile::Release.disposition(AssertionMode::Check),
            AssertionLoweringDisposition::RuntimeGuard(RuntimeAssertionProfile::Always)
        );
        assert_eq!(
            RuntimeAssertionBuildProfile::Debug.disposition(AssertionMode::Debug),
            AssertionLoweringDisposition::RuntimeGuard(RuntimeAssertionProfile::DebugOnly)
        );
        assert_eq!(
            RuntimeAssertionBuildProfile::Release.disposition(AssertionMode::Debug),
            AssertionLoweringDisposition::Omit
        );
    }
}
