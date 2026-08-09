//! Shared syntax vocabulary for references and borrow expressions.

use crate::ast::common::TextRange;
use crate::types::{LifetimeName, TypeRef};

/// Mutability carried by a borrow expression or reference type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BorrowKind {
    Shared,
    Mutable,
}

impl BorrowKind {
    /// Returns whether the borrow permits mutation through the reference.
    pub const fn is_mutable(self) -> bool {
        matches!(self, Self::Mutable)
    }

    /// Returns the canonical source keyword when the borrow is mutable.
    pub const fn mut_keyword(self) -> Option<&'static str> {
        match self {
            Self::Shared => None,
            Self::Mutable => Some("mut"),
        }
    }

    /// Canonical source qualifier between an optional lifetime and referent.
    pub const fn source_qualifier(self) -> &'static str {
        match self {
            Self::Shared => "",
            Self::Mutable => "mut ",
        }
    }

    /// Stable lowercase identity used by fingerprints and project indexes.
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Mutable => "mutable",
        }
    }
}

/// Authored or elided lifetime region on a reference type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionSyntax {
    /// No lifetime was authored; the range is its zero-width insertion point.
    Elided { anchor: TextRange },
    /// An authored lifetime and its exact token range.
    Named {
        name: LifetimeName,
        range: TextRange,
    },
}

impl RegionSyntax {
    /// Returns the exact region range or elision insertion point.
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Elided { anchor } => *anchor,
            Self::Named { range, .. } => *range,
        }
    }

    /// Returns the authored lifetime name, if present.
    pub const fn name(&self) -> Option<&LifetimeName> {
        match self {
            Self::Elided { .. } => None,
            Self::Named { name, .. } => Some(name),
        }
    }

    pub(crate) fn rebase(&mut self, base: usize) {
        match self {
            Self::Elided { anchor } => *anchor = rebased_range(*anchor, base),
            Self::Named { range, .. } => *range = rebased_range(*range, base),
        }
    }
}

/// Typed surface representation of a reference type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceType {
    kind: BorrowKind,
    region: RegionSyntax,
    referent: Box<TypeRef>,
    amp_range: TextRange,
    mut_range: Option<TextRange>,
    range: TextRange,
}

impl ReferenceType {
    pub(crate) const fn new(
        kind: BorrowKind,
        region: RegionSyntax,
        referent: Box<TypeRef>,
        amp_range: TextRange,
        mut_range: Option<TextRange>,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            region,
            referent,
            amp_range,
            mut_range,
            range,
        }
    }

    /// Returns shared or mutable reference permission.
    pub const fn kind(&self) -> BorrowKind {
        self.kind
    }

    /// Returns the authored or elided lifetime region.
    pub const fn region(&self) -> &RegionSyntax {
        &self.region
    }

    /// Returns the typed referent.
    pub const fn referent(&self) -> &TypeRef {
        &self.referent
    }

    /// Returns the exact `&` token range.
    pub const fn amp_range(&self) -> TextRange {
        self.amp_range
    }

    /// Returns the exact contextual `mut` range when present.
    pub const fn mut_range(&self) -> Option<TextRange> {
        self.mut_range
    }

    /// Returns the complete reference-type range.
    pub const fn range(&self) -> TextRange {
        self.range
    }

    pub(crate) fn rebase(&mut self, base: usize) {
        self.region.rebase(base);
        self.referent.rebase_reference_ranges(base);
        self.amp_range = rebased_range(self.amp_range, base);
        self.mut_range = self.mut_range.map(|range| rebased_range(range, base));
        self.range = rebased_range(self.range, base);
    }
}

const fn rebased_range(range: TextRange, base: usize) -> TextRange {
    TextRange::new(range.start() + base, range.end() + base)
}

#[cfg(test)]
mod tests {
    use super::{BorrowKind, ReferenceType, RegionSyntax};
    use crate::ast::common::TextRange;
    use crate::types::TypeRef;

    #[test]
    fn borrow_kind_exposes_owned_mutability_behavior() {
        assert!(!BorrowKind::Shared.is_mutable());
        assert_eq!(BorrowKind::Shared.mut_keyword(), None);
        assert_eq!(BorrowKind::Shared.source_qualifier(), "");
        assert_eq!(BorrowKind::Shared.stable_label(), "shared");

        assert!(BorrowKind::Mutable.is_mutable());
        assert_eq!(BorrowKind::Mutable.mut_keyword(), Some("mut"));
        assert_eq!(BorrowKind::Mutable.source_qualifier(), "mut ");
        assert_eq!(BorrowKind::Mutable.stable_label(), "mutable");
    }

    #[test]
    fn reference_nodes_retain_typed_children_and_exact_ranges() {
        let reference = ReferenceType {
            kind: BorrowKind::Shared,
            region: RegionSyntax::Elided {
                anchor: TextRange::new(1, 1),
            },
            referent: Box::new(
                crate::types::parse_attached_type_for_test("State")
                    .expect("type path parses")
                    .into_value(),
            ),
            amp_range: TextRange::new(0, 1),
            mut_range: None,
            range: TextRange::new(0, 6),
        };
        assert_eq!(reference.kind(), BorrowKind::Shared);
        assert_eq!(reference.region().range(), TextRange::new(1, 1));
        assert_eq!(reference.region().name(), None);
        assert!(
            matches!(reference.referent(), TypeRef::Path(path) if path.canonical_string() == "State")
        );
        assert_eq!(reference.amp_range(), TextRange::new(0, 1));
        assert_eq!(reference.mut_range(), None);
        assert_eq!(reference.range(), TextRange::new(0, 6));
    }
}
