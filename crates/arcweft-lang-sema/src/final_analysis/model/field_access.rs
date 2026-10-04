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

/// A local field path owns one complete place; a computed field selects from
/// its evaluated receiver. The terminal selection is never stored twice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFieldAccess {
    source: CheckedFieldAccessSource,
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckedFieldAccessSource {
    Binding(super::CheckedPlace),
    Expression {
        selection: CheckedFieldSelection,
        receiver: ExprId,
    },
}
impl CheckedFieldAccess {
    pub(crate) fn new(selection: CheckedFieldSelection, receiver: CheckedFieldReceiver) -> Self {
        match receiver {
            CheckedFieldReceiver::Binding(local) => Self {
                source: CheckedFieldAccessSource::Binding(super::CheckedPlace::new(
                    local,
                    vec![selection].into_boxed_slice(),
                )),
            },
            CheckedFieldReceiver::Expression(receiver) => Self {
                source: CheckedFieldAccessSource::Expression {
                    selection,
                    receiver,
                },
            },
        }
    }
    pub(crate) fn try_binding(place: super::CheckedPlace) -> Option<Self> {
        (!place.fields().is_empty()
            && place
                .fields()
                .iter()
                .all(|field| field.runtime_field().is_some())
            && place
                .fields()
                .windows(2)
                .all(|pair| pair[0].field_type() == pair[1].owner_type()))
        .then_some(Self {
            source: CheckedFieldAccessSource::Binding(place),
        })
    }
    pub fn selection(&self) -> &CheckedFieldSelection {
        match &self.source {
            CheckedFieldAccessSource::Binding(place) => place
                .fields()
                .last()
                .expect("binding field paths are nonempty"),
            CheckedFieldAccessSource::Expression { selection, .. } => selection,
        }
    }
    pub fn binding_place(&self) -> Option<&super::CheckedPlace> {
        match &self.source {
            CheckedFieldAccessSource::Binding(place) => Some(place),
            _ => None,
        }
    }
    pub const fn receiver(&self) -> CheckedFieldReceiver {
        match &self.source {
            CheckedFieldAccessSource::Binding(place) => {
                CheckedFieldReceiver::Binding(place.local())
            }
            CheckedFieldAccessSource::Expression { receiver, .. } => {
                CheckedFieldReceiver::Expression(*receiver)
            }
        }
    }
}
