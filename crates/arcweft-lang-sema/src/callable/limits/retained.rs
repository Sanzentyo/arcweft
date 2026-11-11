//! One resumable work ledger shared by a source's retained alternatives.

use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use super::{
    CallResolverAccountingReport, ResolverWork, ResolverWorkComponent, TypeConstraintAbort,
};
use crate::callable::CallConstraintInvariant;

#[derive(Debug)]
struct RetainedWork {
    /// Immutable source-preparation observation used for semantic replay, not
    /// an allowance from which a later session can reconstruct a new ledger.
    preparation: CallResolverAccountingReport,
    work: RefCell<ResolverWork>,
}

/// Cloning shares the actual already-consumed ledger. Equality compares only
/// the historical preparation observation, as correlated recipe replay does;
/// it supplies no ownership or execution authority.
#[derive(Clone, Debug)]
pub(crate) struct RetainedResolverWork(Rc<RetainedWork>);

impl PartialEq for RetainedResolverWork {
    fn eq(&self, other: &Self) -> bool {
        self.0.preparation == other.0.preparation
    }
}
impl Eq for RetainedResolverWork {}

impl RetainedResolverWork {
    /// Admit the fixed owner allocation before moving the original ledger.
    pub(crate) fn retain(mut work: ResolverWork) -> Result<Self, TypeConstraintAbort> {
        work.charge_component(1, ResolverWorkComponent::Resolver)
            .map_err(TypeConstraintAbort::from)?;
        let preparation = work.call_accounting();
        Ok(Self(Rc::new(RetainedWork {
            preparation,
            work: RefCell::new(work),
        })))
    }

    pub(crate) fn borrow_work(&self) -> Result<RefMut<'_, ResolverWork>, CallConstraintInvariant> {
        self.0
            .work
            .try_borrow_mut()
            .map_err(|_| CallConstraintInvariant::RetainedCallWorkBorrowed)
    }

    /// Publication reads the current observation after all sessions release
    /// their original ledger; it never restores work from the frozen report.
    pub(crate) fn report(&self) -> Result<CallResolverAccountingReport, CallConstraintInvariant> {
        self.0
            .work
            .try_borrow()
            .map(|work| work.call_accounting())
            .map_err(|_| CallConstraintInvariant::RetainedCallWorkBorrowed)
    }

    #[cfg(test)]
    fn same_ledger(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests;
