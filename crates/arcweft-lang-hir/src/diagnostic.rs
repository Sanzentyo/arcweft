//! Typed recoverable diagnostics retained by one immutable HIR module.

use core::cmp::Ordering;

use arcweft_lang_syntax::incremental::SyntaxDiagnostic;

use crate::identity::SyntheticOwner;
use crate::source_index::{HirSourceQuery, HirSourceSite};

/// Parser-owned recovery or a typed semantic owner whose payload records the
/// exact HIR recovery issue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDiagnostic {
    Syntax(SyntaxDiagnostic),
    Recovery(HirRecoveryDiagnostic),
}

impl HirDiagnostic {
    pub fn source_site(&self) -> HirSourceSiteRef<'_> {
        match self {
            Self::Syntax(diagnostic) => HirSourceSiteRef::Span(diagnostic.primary()),
            Self::Recovery(diagnostic) => HirSourceSiteRef::Hir(diagnostic.primary()),
        }
    }

    pub(crate) fn compare_for_publication(&self, other: &Self) -> Ordering {
        self.source_site()
            .start_end()
            .cmp(&other.source_site().start_end())
            .then_with(|| self.kind_rank().cmp(&other.kind_rank()))
            .then_with(|| match (self, other) {
                (Self::Syntax(left), Self::Syntax(right)) => left
                    .code()
                    .cmp(right.code())
                    .then_with(|| left.message().cmp(right.message())),
                (Self::Recovery(left), Self::Recovery(right)) => left
                    .owner
                    .cmp(&right.owner)
                    .then_with(|| left.primary.cmp(&right.primary)),
                _ => Ordering::Equal,
            })
    }

    pub fn syntax(&self) -> Option<&SyntaxDiagnostic> {
        match self {
            Self::Syntax(diagnostic) => Some(diagnostic),
            Self::Recovery(_) => None,
        }
    }

    const fn kind_rank(&self) -> u8 {
        match self {
            Self::Syntax(_) => 0,
            Self::Recovery(_) => 1,
        }
    }
}

pub enum HirSourceSiteRef<'a> {
    Span(&'a arcweft_source::SourceSpan),
    Hir(&'a HirSourceSite),
}

impl HirSourceSiteRef<'_> {
    fn start_end(&self) -> (usize, usize) {
        match self {
            Self::Span(span) => (span.range().start(), span.range().end()),
            Self::Hir(HirSourceSite::Span(span)) => (span.range().start(), span.range().end()),
            Self::Hir(HirSourceSite::Insertion(insertion)) => {
                (insertion.offset(), insertion.offset())
            }
        }
    }

    pub fn source_identity(&self) -> &arcweft_source::SourceDocumentIdentity {
        match self {
            Self::Span(span) => span.source(),
            Self::Hir(site) => site.source_identity(),
        }
    }
}

/// Typed primary-source authority for one HIR recovery event.
///
/// Semantic owners with a component-role family use the sole typed source
/// query. Owners without such a family keep their exact slot-owned `Whole`
/// site explicitly instead of being forced through a fabricated role.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRecoveryPrimary {
    Query(HirSourceQuery),
    OwnerWhole(SyntheticOwner),
}

impl HirRecoveryPrimary {
    pub const fn query(query: HirSourceQuery) -> Self {
        Self::Query(query)
    }

    pub const fn owner_whole(owner: SyntheticOwner) -> Self {
        Self::OwnerWhole(owner)
    }

    pub const fn owner(&self) -> SyntheticOwner {
        match self {
            Self::Query(query) => query.owner(),
            Self::OwnerWhole(owner) => *owner,
        }
    }
}

/// One recovery diagnostic keyed by its exact semantic arena owner.
///
/// The owner's payload contains the closed issue enum. Keeping only the owner
/// plus a typed primary descriptor and its retained site here avoids a second
/// error vocabulary and lets module freeze prove that no caller substituted a
/// different source role or coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRecoveryDiagnostic {
    owner: SyntheticOwner,
    primary: HirRecoveryPrimary,
    primary_site: HirSourceSite,
}

impl HirRecoveryDiagnostic {
    pub(crate) fn new(
        owner: SyntheticOwner,
        primary: HirRecoveryPrimary,
        primary_site: HirSourceSite,
    ) -> Self {
        Self {
            owner,
            primary,
            primary_site,
        }
    }

    pub const fn owner(&self) -> SyntheticOwner {
        self.owner
    }

    pub fn primary_role(&self) -> HirRecoveryPrimary {
        self.primary.clone()
    }

    pub const fn primary(&self) -> &HirSourceSite {
        &self.primary_site
    }
}
