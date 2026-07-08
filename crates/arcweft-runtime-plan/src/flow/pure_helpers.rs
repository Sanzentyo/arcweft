use super::RuntimePlanLowerStats;
use crate::pure::PureHelperCandidate;
use arcweft_core::plan::{RuntimePureHelper, RuntimePureHelperId};
use std::collections::BTreeMap;

pub(super) fn runtime_pure_helper_inventory(
    candidates: &[PureHelperCandidate],
    stats: &mut RuntimePlanLowerStats,
) -> (
    Vec<RuntimePureHelper>,
    BTreeMap<String, RuntimePureHelperId>,
) {
    let helpers = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            stats.pure_expr_cloned_nodes += candidate.shape().expr_weight;
            candidate.to_runtime_helper(RuntimePureHelperId(index))
        })
        .collect::<Vec<_>>();
    let ids = helpers
        .iter()
        .map(|helper| (helper.name.clone(), helper.id))
        .collect();
    (helpers, ids)
}
