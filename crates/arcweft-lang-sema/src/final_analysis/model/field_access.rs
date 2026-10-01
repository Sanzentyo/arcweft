use arcweft_lang_hir::identity::{ExprId, LocalId};

use super::CheckedFieldSelection;

/// Exact value source of a field access. A direct binding path reads at the
/// field expression; an expression receiver is evaluated at its own child.
/// Writability is independently owned by `CheckedMutablePlace`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedFieldReceiver {
    Binding(LocalId),
    Expression(ExprId),
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
