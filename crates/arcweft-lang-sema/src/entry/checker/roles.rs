use arcweft_lang_hir::{
    entry::{HirEntryDecl, HirEntryItem},
    model::HirModule,
};

use super::{CheckedEntryDiagnostic, source_span};

#[derive(Clone, Copy)]
pub(super) enum Role {
    State,
    Initializer,
    Event,
    Reducer,
    Controller,
}

impl Role {
    const fn label(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Initializer => "initializer",
            Self::Event => "event",
            Self::Reducer => "reducer",
            Self::Controller => "controller",
        }
    }

    const fn matches(self, item: &HirEntryItem) -> bool {
        matches!(
            (self, item),
            (Self::State, HirEntryItem::StateType { .. })
                | (Self::Initializer, HirEntryItem::Initializer { .. })
                | (Self::Event, HirEntryItem::EventType { .. })
                | (Self::Reducer, HirEntryItem::Reducer { .. })
                | (Self::Controller, HirEntryItem::Controller { .. })
        )
    }
}

pub(super) fn unique_item<'a>(
    module: &HirModule,
    entry: &'a HirEntryDecl,
    role: Role,
    diagnostics: &mut Vec<CheckedEntryDiagnostic>,
) -> Option<&'a HirEntryItem> {
    let items = entry
        .items()
        .iter()
        .filter(|item| role.matches(item))
        .collect::<Vec<_>>();
    match items.as_slice() {
        [item] => Some(*item),
        [] => {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.missing_role",
                format!("entry is missing required `{}` role", role.label()),
                source_span(module, *entry.range()),
            ));
            None
        }
        [first, rest @ ..] => {
            diagnostics.push(
                CheckedEntryDiagnostic::new(
                    "sema.entry.duplicate_role",
                    format!("entry declares `{}` more than once", role.label()),
                    source_span(
                        module,
                        *rest[0]
                            .range()
                            .expect("typed role members retain their exact member range"),
                    ),
                )
                .with_related([source_span(
                    module,
                    *first
                        .range()
                        .expect("typed role members retain their exact member range"),
                )]),
            );
            None
        }
    }
}
