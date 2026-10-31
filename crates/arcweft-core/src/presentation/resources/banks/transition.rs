//! Preflighted scope exit and escaping-value custody under the original banks.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl<V> PresentationBanks<V> {
    /// The source and receiver are actual borrowed storage anchors. All grant,
    /// token, queue and destination cursor admission precedes the first move.
    /// No body is cloned and a terminal receiver retains its original token.
    pub(crate) fn transition_scope_contour(
        &mut self,
        program: &RuntimeProgramOwner,
        targets: Vec<PresentationScopeExit<PresentationScopeTarget<'_>>>,
        mut destination: Option<PresentationScopeExit<PresentationScopeTarget<'_>>>,
        escaping: &[RuntimePresentationHandleToken],
        discarding: &[RuntimePresentationHandleToken],
    ) -> Result<(), RuntimePresentationError> {
        let root = self
            .ledgers
            .get(&RuntimePresentationBankId::Root)
            .ok_or(RuntimePresentationError::InvalidState)?;
        root.validate_program(program)?;
        if escaping
            .len()
            .checked_add(discarding.len())
            .is_none_or(|count| count > root.limits.handles)
        {
            return Err(RuntimePresentationError::Capacity);
        }
        let mut selected = BTreeSet::new();
        for token in escaping {
            if !selected.insert(token.clone()) {
                return Err(RuntimePresentationError::InvalidState);
            }
        }
        let mut discarded = BTreeSet::new();
        let mut discard_owners = Vec::new();
        for token in discarding {
            if selected.contains(token) || !discarded.insert(token.clone()) {
                return Err(RuntimePresentationError::InvalidState);
            }
            let ledger = self
                .ledgers
                .get(&token.bank())
                .ok_or(RuntimePresentationError::UnknownHandle)?;
            let lease = ledger
                .state
                .leases
                .get(&token.issuance())
                .ok_or(RuntimePresentationError::UnknownHandle)?;
            ledger.validate_handle(program, token, lease.owner)?;
            discard_owners.push((token, lease.owner));
        }
        let owners = self.inspect_scope_contour(
            program,
            targets.iter().map(|target| PresentationScopeExit {
                execution: target.execution,
                fiber: target.fiber,
                scope: target.scope.as_ref(),
            }),
        )?;
        self.preflight_scope_retirements(&owners, &selected, &discarded)?;
        let receiving_owners = if let Some(receiver) = &destination {
            let receiving = self.inspect_scope_contour(
                program,
                std::iter::once(PresentationScopeExit {
                    execution: receiver.execution,
                    fiber: receiver.fiber,
                    scope: receiver.scope.as_ref(),
                }),
            )?;
            if !owners.is_disjoint(&receiving) {
                return Err(RuntimePresentationError::InvalidState);
            }
            receiving
        } else {
            BTreeSet::new()
        };
        let mut moves = Vec::new();
        let mut receiving = BTreeMap::new();
        for token in escaping {
            let ledger = self
                .ledgers
                .get(&token.bank())
                .ok_or(RuntimePresentationError::UnknownHandle)?;
            let lease = ledger
                .state
                .leases
                .get(&token.issuance())
                .ok_or(RuntimePresentationError::UnknownHandle)?;
            ledger.validate_handle(program, token, lease.owner)?;
            // Only the actual receiver's existing grant already owns this
            // receipt. A grant in another execution can end independently.
            if lease
                .owner
                .is_some_and(|owner| receiving_owners.contains(&owner))
            {
                continue;
            }
            if let Some(receiver) = &destination {
                let released = owners
                    .iter()
                    .filter(|owner| owner.bank == token.bank())
                    .count();
                let owner = ledger.preflight_bind_scope(
                    program,
                    receiver.execution,
                    receiver.fiber,
                    receiver.scope.as_ref(),
                    released,
                )?;
                receiving.insert(token.bank(), owner);
            }
            moves.push((token.clone(), lease.owner));
        }
        // Detachment is internal to this exclusive transaction. The actual
        // value stays in its original storage until the VM commits its move.
        for (token, owner) in &moves {
            self.ledgers
                .get_mut(&token.bank())
                .expect("issuing ledger admitted")
                .transfer(program, token, *owner, None)
                .expect("original token owner admitted");
        }
        self.retire_scope_contour(program, targets)
            .expect("exclusive contour remains admitted after escaping leases detach");
        // Discards in the exiting contour already followed its deterministic
        // scope/bank/issuance order. The remaining packet values follow their
        // original source order, under the same aggregate capacity admission.
        for (token, owner) in discard_owners {
            if owner.is_some_and(|owner| owners.contains(&owner)) {
                continue;
            }
            let ledger = self
                .ledgers
                .get_mut(&token.bank())
                .expect("discarding ledger admitted");
            if let Some(body) = ledger
                .release(program, token, owner)
                .expect("discarding token and original owner admitted")
            {
                self.retirements.push(PresentationRetirement {
                    token: token.clone(),
                    body,
                });
            }
        }
        if let Some(receiver) = destination.as_mut() {
            for (bank, expected) in &receiving {
                let actual = self
                    .ledgers
                    .get_mut(bank)
                    .expect("receiving ledger admitted")
                    .bind_scope(
                        program,
                        receiver.execution,
                        receiver.fiber,
                        receiver.scope.reborrow(),
                    )
                    .expect("destination cursor and post-exit scope capacity admitted");
                assert_eq!(
                    actual, *expected,
                    "destination scope grant matches its preflight"
                );
            }
        }
        for (token, _) in moves {
            if let Some(owner) = receiving.get(&token.bank()) {
                self.ledgers
                    .get_mut(&token.bank())
                    .expect("receiving ledger admitted")
                    .transfer(program, &token, None, Some(*owner))
                    .expect("actual receiving grant was installed");
            }
        }
        Ok(())
    }
}
