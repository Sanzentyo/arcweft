//! Fresh-session projection of persisted runtime assertion failures.

use arcweft_core::effect::{
    RuntimeArtifactFingerprint, RuntimeAssertionFailure, RuntimeAssertionProfile,
};

use crate::assertion_identity::{
    AssertionConditionIndex, RuntimeAssertionFault, RuntimeAssertionFaultIdentity,
    RuntimeAssertionInventory, RuntimeAssertionMode, RuntimeAssertionProjectionError,
};

pub(crate) fn project_failure(
    inventory: &RuntimeAssertionInventory,
    artifact: RuntimeArtifactFingerprint,
    failure: RuntimeAssertionFailure,
) -> Result<RuntimeAssertionFault, RuntimeAssertionProjectionError> {
    if artifact != inventory.artifact() {
        return Err(RuntimeAssertionProjectionError::ArtifactMismatch {
            expected: inventory.artifact(),
            actual: artifact,
        });
    }

    let observed = failure.into_assertion();
    let guard = observed.guard();
    let site = inventory
        .site(guard)
        .ok_or(RuntimeAssertionProjectionError::UnknownGuard { guard })?;
    if !profile_matches_mode(observed.profile(), site.mode()) {
        return Err(RuntimeAssertionProjectionError::ProfileModeMismatch {
            guard,
            profile: observed.profile(),
            mode: site.mode(),
        });
    }
    let condition = AssertionConditionIndex::try_new(site.condition().get().into(), 64)?;
    let identity = RuntimeAssertionFaultIdentity::new(
        site.statement(),
        condition,
        site.mode(),
        site.condition_span().clone(),
    );
    Ok(RuntimeAssertionFault::new(
        identity,
        guard,
        site.presentation().clone(),
        observed,
    ))
}

const fn profile_matches_mode(
    profile: RuntimeAssertionProfile,
    mode: RuntimeAssertionMode,
) -> bool {
    matches!(
        (profile, mode),
        (RuntimeAssertionProfile::Always, RuntimeAssertionMode::Check)
            | (
                RuntimeAssertionProfile::DebugOnly,
                RuntimeAssertionMode::Debug
            )
    )
}
