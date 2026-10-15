//! External API compile witness: an upper owner can encode the complete Core
//! prefix using only the borrowed protocol, without constructing a capability
//! or accessing an executable/source row. Actual View admission is separate.

use arcweft_core::task::{RuntimeTaskPlanDigestBase, TaskPlanSemanticDigest};

fn write_core_prefix(base: &RuntimeTaskPlanDigestBase<'_>) -> blake3::Hasher {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TaskPlanSemanticDigest::DOMAIN);
    hasher.update(&[base.plan_owner_tag()]);
    hasher.update(base.executable_semantic_digest().as_bytes());
    hasher.update(base.producer_function_semantic_digest().as_bytes());
    hasher.update(&[
        base.family().semantic_tag(),
        base.task_class().semantic_tag(),
    ]);
    hasher.update(base.request_template_digest().as_bytes());
    hasher.update(base.control_effect_contract_digest().as_bytes());
    hasher
}

#[test]
fn upper_authority_has_public_read_only_access_to_the_complete_core_prefix() {
    let _: fn(&RuntimeTaskPlanDigestBase<'_>) -> blake3::Hasher = write_core_prefix;
}
