//! Typed source-role vocabulary for final View item owners.

/// Exact source component of one final View body.
///
/// `Whole` is the authored braced body or the parser-owned missing-body
/// insertion. Delimiters and the fragment are optional only when the body is
/// missing; an authored but unclosed body retains its zero-width close
/// insertion as a present component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirViewBodySourcePart {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
    Fragment,
}

/// Exact source component of one source-ordered View export.
///
/// The ordinal is the retained declaration-member ordinal. Misplaced exports
/// use the same component family and remain poisoned on the retained member;
/// no recovery-only source role or historical syntax reader is introduced.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirViewExportSourcePart {
    Whole,
    PartKeyword,
    LocalPart,
    AliasKeyword,
    PublicPart,
}

/// Typed source component owned by one final View item.
///
/// View parameters use the shared callable source-role family with
/// `HirCallableSourceOwner::ViewItem`. Expression values keep their existing
/// `ExprId` source roles. This family therefore owns only the View-specific
/// identity, body, and export components.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirViewSourceRole {
    Whole,
    ItemId,
    Body(HirViewBodySourcePart),
    Export {
        ordinal: u32,
        part: HirViewExportSourcePart,
    },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{HirViewBodySourcePart, HirViewExportSourcePart, HirViewSourceRole};

    #[test]
    fn view_role_vocabulary_keeps_body_and_export_components_distinct() {
        let mut roles = BTreeSet::from([
            HirViewSourceRole::Whole,
            HirViewSourceRole::ItemId,
            HirViewSourceRole::Body(HirViewBodySourcePart::Whole),
            HirViewSourceRole::Body(HirViewBodySourcePart::OpenDelimiter),
            HirViewSourceRole::Body(HirViewBodySourcePart::CloseDelimiter),
            HirViewSourceRole::Body(HirViewBodySourcePart::Fragment),
        ]);
        roles.extend(
            [
                HirViewExportSourcePart::Whole,
                HirViewExportSourcePart::PartKeyword,
                HirViewExportSourcePart::LocalPart,
                HirViewExportSourcePart::AliasKeyword,
                HirViewExportSourcePart::PublicPart,
            ]
            .map(|part| HirViewSourceRole::Export { ordinal: 3, part }),
        );

        assert_eq!(roles.len(), 11);
    }
}
