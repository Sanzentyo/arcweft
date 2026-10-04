use arcweft_lang_hir::identity::{ExprId, LocalId};

use super::CheckedFieldSelection;

/// Exact value source of a field access. A direct binding path reads at the
/// field expression; an expression receiver is evaluated at its own child.
/// Writability is independently owned by `CheckedPlace`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedFieldReceiver {
    Binding(LocalId),
    Expression(ExprId),
}

impl CheckedFieldReceiver {
    /// Follows only typed local/record-address sources. Computed receivers
    /// remain value evaluations. Selected expression graphs are acyclic.
    pub(crate) fn local_root(self, source: impl Fn(ExprId) -> Option<Self>) -> Option<LocalId> {
        let mut receiver = self;
        loop {
            match receiver {
                Self::Binding(local) => return Some(local),
                Self::Expression(owner) => receiver = source(owner)?,
            }
        }
    }
}

/// One field expression's accepted schema selection and evaluation source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFieldAccess {
    selection: CheckedFieldSelection,
    receiver: CheckedFieldReceiver,
}

impl CheckedFieldAccess {
    pub(crate) const fn new(
        selection: CheckedFieldSelection,
        receiver: CheckedFieldReceiver,
    ) -> Self {
        Self {
            selection,
            receiver,
        }
    }

    pub const fn selection(&self) -> &CheckedFieldSelection {
        &self.selection
    }

    pub const fn receiver(&self) -> CheckedFieldReceiver {
        self.receiver
    }
}
