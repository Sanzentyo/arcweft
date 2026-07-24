use super::{ProfileTopologyLimitKind, ProfileTopologyLimits, ProfileTopologyLoadError};

#[derive(Clone, Copy, Debug)]
struct BudgetLimits {
    resources: u64,
    source_bytes: u64,
    overlay_bytes: u64,
    diagnostics: u64,
    work: u64,
}

impl From<ProfileTopologyLimits> for BudgetLimits {
    fn from(limits: ProfileTopologyLimits) -> Self {
        Self {
            resources: limits.resources(),
            source_bytes: limits.source_bytes(),
            overlay_bytes: limits.overlay_bytes(),
            diagnostics: limits.diagnostics(),
            work: limits.work(),
        }
    }
}

#[derive(Debug)]
pub(super) struct ProfileTopologyBudget {
    limits: BudgetLimits,
    resources: u64,
    source_bytes: u64,
    overlay_bytes: u64,
    diagnostics: u64,
    work: u64,
}

impl ProfileTopologyBudget {
    pub(super) fn production() -> Self {
        Self {
            limits: ProfileTopologyLimits::PRODUCTION.into(),
            resources: 0,
            source_bytes: 0,
            overlay_bytes: 0,
            diagnostics: 0,
            work: 0,
        }
    }

    pub(super) fn charge_resource(&mut self) -> Result<(), ProfileTopologyLoadError> {
        charge(
            &mut self.resources,
            1,
            self.limits.resources,
            ProfileTopologyLimitKind::Resources,
        )
    }

    pub(super) fn check_single_resource_bytes(
        &self,
        observed: usize,
    ) -> Result<u64, ProfileTopologyLoadError> {
        let observed =
            u64::try_from(observed).map_err(|_| ProfileTopologyLoadError::ArithmeticOverflow {
                kind: ProfileTopologyLimitKind::SourceBytes,
            })?;
        if observed > self.limits.source_bytes {
            return Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::SourceBytes,
                observed,
                maximum: self.limits.source_bytes,
            });
        }
        Ok(observed)
    }

    pub(super) fn charge_source_bytes(
        &mut self,
        amount: usize,
    ) -> Result<(), ProfileTopologyLoadError> {
        let amount =
            u64::try_from(amount).map_err(|_| ProfileTopologyLoadError::ArithmeticOverflow {
                kind: ProfileTopologyLimitKind::SourceBytes,
            })?;
        charge(
            &mut self.source_bytes,
            amount,
            self.limits.source_bytes,
            ProfileTopologyLimitKind::SourceBytes,
        )
    }

    pub(super) fn remaining_source_bytes(&self) -> u64 {
        self.limits.source_bytes.saturating_sub(self.source_bytes)
    }

    pub(super) fn charge_overlay_bytes(
        &mut self,
        amount: u64,
    ) -> Result<(), ProfileTopologyLoadError> {
        charge(
            &mut self.overlay_bytes,
            amount,
            self.limits.overlay_bytes,
            ProfileTopologyLimitKind::OverlayBytes,
        )
    }

    pub(super) fn charge_work(&mut self, amount: u64) -> Result<(), ProfileTopologyLoadError> {
        charge(
            &mut self.work,
            amount,
            self.limits.work,
            ProfileTopologyLimitKind::Work,
        )
    }

    pub(super) fn charge_diagnostics(
        &mut self,
        amount: u64,
    ) -> Result<(), ProfileTopologyLoadError> {
        charge(
            &mut self.diagnostics,
            amount,
            self.limits.diagnostics,
            ProfileTopologyLimitKind::Diagnostics,
        )
    }

    pub(super) const fn work(&self) -> u64 {
        self.work
    }

    #[cfg(test)]
    fn for_test(
        resources: u64,
        source_bytes: u64,
        overlay_bytes: u64,
        diagnostics: u64,
        work: u64,
    ) -> Self {
        Self {
            limits: BudgetLimits {
                resources,
                source_bytes,
                overlay_bytes,
                diagnostics,
                work,
            },
            resources: 0,
            source_bytes: 0,
            overlay_bytes: 0,
            diagnostics: 0,
            work: 0,
        }
    }
}

fn charge(
    counter: &mut u64,
    amount: u64,
    maximum: u64,
    kind: ProfileTopologyLimitKind,
) -> Result<(), ProfileTopologyLoadError> {
    let observed = counter
        .checked_add(amount)
        .ok_or(ProfileTopologyLoadError::ArithmeticOverflow { kind })?;
    if observed > maximum {
        return Err(ProfileTopologyLoadError::Limit {
            kind,
            observed,
            maximum,
        });
    }
    *counter = observed;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_budget_accepts_exact_maximum_and_rejects_one_over() {
        let mut budget = ProfileTopologyBudget::for_test(1, 3, 3, 1, 1);
        budget.charge_resource().expect("resource maximum");
        budget
            .charge_source_bytes(3)
            .expect("aggregate source maximum");
        budget.charge_overlay_bytes(3).expect("overlay maximum");
        budget.charge_diagnostics(1).expect("diagnostic maximum");
        budget.charge_work(1).expect("work maximum");

        assert!(matches!(
            budget.charge_resource(),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::Resources,
                observed: 2,
                maximum: 1,
            })
        ));
        assert!(matches!(
            budget.charge_source_bytes(1),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::SourceBytes,
                observed: 4,
                maximum: 3,
            })
        ));
    }

    #[test]
    fn multiple_small_text_resources_share_one_aggregate_source_budget() {
        let mut budget = ProfileTopologyBudget::for_test(4, 8, 8, 1, 1);
        for bytes in [2, 3, 3] {
            budget
                .charge_source_bytes(bytes)
                .expect("small resource stays within aggregate");
        }
        assert_eq!(budget.remaining_source_bytes(), 0);
        assert!(matches!(
            budget.charge_source_bytes(1),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::SourceBytes,
                observed: 9,
                maximum: 8,
            })
        ));
    }

    #[test]
    fn budget_overflow_is_distinct_from_an_ordinary_limit() {
        let mut budget = ProfileTopologyBudget::for_test(u64::MAX, 1, u64::MAX, u64::MAX, u64::MAX);
        budget.overlay_bytes = u64::MAX;
        assert!(matches!(
            budget.charge_overlay_bytes(1),
            Err(ProfileTopologyLoadError::ArithmeticOverflow {
                kind: ProfileTopologyLimitKind::OverlayBytes,
            })
        ));

        let mut budget =
            ProfileTopologyBudget::for_test(u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX);
        budget.source_bytes = u64::MAX;
        assert!(matches!(
            budget.charge_source_bytes(1),
            Err(ProfileTopologyLoadError::ArithmeticOverflow {
                kind: ProfileTopologyLimitKind::SourceBytes,
            })
        ));
    }

    #[test]
    fn production_limits_are_inclusive_and_reject_one_over() {
        let limits = ProfileTopologyLimits::PRODUCTION;
        let mut budget = ProfileTopologyBudget::production();
        budget
            .charge_overlay_bytes(limits.overlay_bytes())
            .expect("overlay exact maximum");
        budget
            .charge_diagnostics(limits.diagnostics())
            .expect("diagnostic exact maximum");
        budget
            .charge_work(limits.work())
            .expect("work exact maximum");
        for _ in 0..limits.resources() {
            budget.charge_resource().expect("resource within maximum");
        }
        budget
            .charge_source_bytes(
                usize::try_from(limits.source_bytes()).expect("source limit fits usize"),
            )
            .expect("aggregate source exact maximum");

        assert!(matches!(
            budget.charge_resource(),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::Resources,
                observed,
                maximum,
            }) if observed == limits.resources() + 1 && maximum == limits.resources()
        ));
        assert!(matches!(
            budget.charge_overlay_bytes(1),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::OverlayBytes,
                observed,
                maximum,
            }) if observed == limits.overlay_bytes() + 1 && maximum == limits.overlay_bytes()
        ));
        assert!(matches!(
            budget.charge_diagnostics(1),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::Diagnostics,
                observed,
                maximum,
            }) if observed == limits.diagnostics() + 1 && maximum == limits.diagnostics()
        ));
        assert!(matches!(
            budget.charge_work(1),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::Work,
                observed,
                maximum,
            }) if observed == limits.work() + 1 && maximum == limits.work()
        ));
        assert!(matches!(
            budget.charge_source_bytes(1),
            Err(ProfileTopologyLoadError::Limit {
                kind: ProfileTopologyLimitKind::SourceBytes,
                observed,
                maximum,
            }) if observed == limits.source_bytes() + 1 && maximum == limits.source_bytes()
        ));
    }
}
