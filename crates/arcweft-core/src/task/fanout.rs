//! Source-position ownership shared by Native and AWBC suspended fanout.

/// Borrowed control metadata, before its pending/result partition is admitted.
/// Callers admit collection lengths before validation grows its index set.
pub(crate) struct RuntimeFanoutProgress {
    pub total: usize,
    pub next_index: usize,
    pub limit: u32,
    pub initialized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum RuntimeFanoutProgressError {
    #[error("fanout inventories disagree with its frontier or bound")]
    Inventories,
    #[error("fanout pending positions exceed the frontier or repeat")]
    Pending,
    #[error("uninitialized fanout has started or completed positions")]
    Uninitialized,
    #[error("fanout result ownership differs from its source frontier")]
    Results,
}

impl RuntimeFanoutProgress {
    /// Each started source position has exactly one pending or completed owner;
    /// future positions have neither. No runtime value or handle is copied.
    pub(crate) fn validate(
        self,
        pending: impl ExactSizeIterator<Item = usize>,
        completed: impl ExactSizeIterator<Item = bool>,
    ) -> Result<(), RuntimeFanoutProgressError> {
        use RuntimeFanoutProgressError as Error;
        if self.limit == 0
            || u32::try_from(self.total).is_err()
            || self.next_index > self.total
            || completed.len() != self.total
            || pending.len() > self.limit as usize
        {
            return Err(Error::Inventories);
        }
        if !self.initialized && (self.next_index != 0 || pending.len() != 0) {
            return Err(Error::Uninitialized);
        }
        let mut indices = std::collections::BTreeSet::new();
        for index in pending {
            if index >= self.next_index || !indices.insert(index) {
                return Err(Error::Pending);
            }
        }
        for (index, complete) in completed.enumerate() {
            if !self.initialized && complete {
                return Err(Error::Uninitialized);
            }
            if if index < self.next_index {
                complete == indices.contains(&index)
            } else {
                complete || indices.contains(&index)
            } {
                return Err(Error::Results);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(
        next_index: usize,
        pending: &[usize],
        completed: &[bool],
    ) -> Result<(), RuntimeFanoutProgressError> {
        RuntimeFanoutProgress {
            total: completed.len(),
            next_index,
            limit: 2,
            initialized: true,
        }
        .validate(pending.iter().copied(), completed.iter().copied())
    }

    #[test]
    fn admitted_progress_partitions_started_positions_in_source_order() {
        assert_eq!(validate(3, &[2, 0], &[false, true, false, false]), Ok(()));
        assert_eq!(validate(3, &[], &[true, true, true]), Ok(()));
        assert_eq!(validate(0, &[], &[]), Ok(()));
    }

    #[test]
    fn started_positions_cannot_lose_or_duplicate_their_owner() {
        assert_eq!(
            validate(1, &[], &[false]),
            Err(RuntimeFanoutProgressError::Results)
        );
        assert_eq!(
            validate(1, &[0], &[true]),
            Err(RuntimeFanoutProgressError::Results)
        );
        assert_eq!(
            validate(0, &[], &[true]),
            Err(RuntimeFanoutProgressError::Results)
        );
    }

    #[test]
    fn pending_positions_cannot_repeat_or_point_beyond_the_started_frontier() {
        assert_eq!(
            validate(2, &[0, 0], &[false, false]),
            Err(RuntimeFanoutProgressError::Pending)
        );
        assert_eq!(
            validate(1, &[1], &[false, false]),
            Err(RuntimeFanoutProgressError::Pending)
        );
        assert_eq!(
            validate(1, &[usize::MAX], &[false]),
            Err(RuntimeFanoutProgressError::Pending)
        );
    }

    #[test]
    fn invalid_frontiers_and_in_flight_bounds_are_rejected() {
        for (total, next_index, limit, pending, completed) in [
            (1, 1, 0, vec![0], vec![false]),
            (2, 2, 1, vec![0, 1], vec![false, false]),
            (1, 2, 2, vec![], vec![true]),
            (2, 1, 2, vec![0], vec![false]),
        ] {
            assert_eq!(
                RuntimeFanoutProgress {
                    total,
                    next_index,
                    limit,
                    initialized: true
                }
                .validate(pending.into_iter(), completed.into_iter()),
                Err(RuntimeFanoutProgressError::Inventories),
            );
        }
    }

    #[test]
    fn uninitialized_progress_has_no_started_or_ready_positions() {
        for (next_index, pending, completed, expected) in [
            (0, vec![], vec![false], Ok(())),
            (
                1,
                vec![0],
                vec![false],
                Err(RuntimeFanoutProgressError::Uninitialized),
            ),
            (
                0,
                vec![],
                vec![true],
                Err(RuntimeFanoutProgressError::Uninitialized),
            ),
        ] {
            let result = RuntimeFanoutProgress {
                total: completed.len(),
                next_index,
                limit: 2,
                initialized: false,
            }
            .validate(pending.into_iter(), completed.into_iter());
            assert_eq!(result, expected);
        }
    }
}
