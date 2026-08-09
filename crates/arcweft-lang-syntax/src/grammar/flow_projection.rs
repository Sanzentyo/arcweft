//! Parser-owned source and identity projection for one ordinary Flow item.

use arcweft_source::SourceRange;

use crate::id_ref::{SyntaxIdRefComponent, SyntaxIdRefIssue, SyntaxIdRefPart, SyntaxIdRefSyntax};
use crate::name::SyntaxName;

/// How an authored Flow public-ID token obtains its suffix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingFlowPublicIdForm {
    Authored,
    DerivedFromEmptyMarker { family: Option<SyntaxName> },
}

/// One lexer-projected Flow public-ID token and its family validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingFlowPublicId {
    syntax: SyntaxIdRefSyntax,
    source: SourceRange,
    components: Box<[SyntaxIdRefComponent]>,
    form: PendingFlowPublicIdForm,
    canonical_flow_family: bool,
}

impl PendingFlowPublicId {
    pub(crate) const fn new(
        syntax: SyntaxIdRefSyntax,
        source: SourceRange,
        components: Box<[SyntaxIdRefComponent]>,
        form: PendingFlowPublicIdForm,
        canonical_flow_family: bool,
    ) -> Self {
        Self {
            syntax,
            source,
            components,
            form,
            canonical_flow_family,
        }
    }

    pub(crate) const fn syntax(&self) -> &SyntaxIdRefSyntax {
        &self.syntax
    }

    pub(crate) const fn source(&self) -> SourceRange {
        self.source
    }

    pub(crate) fn components(&self) -> &[SyntaxIdRefComponent] {
        &self.components
    }

    pub(crate) const fn form(&self) -> &PendingFlowPublicIdForm {
        &self.form
    }

    pub(crate) const fn is_canonical_flow_family(&self) -> bool {
        self.canonical_flow_family
    }

    pub(crate) fn has_recovery(&self) -> bool {
        !self.canonical_flow_family
            || !matches!(
                (&self.form, self.syntax.value()),
                (PendingFlowPublicIdForm::Authored, Ok(_))
                    | (
                        PendingFlowPublicIdForm::DerivedFromEmptyMarker { .. },
                        Err(SyntaxIdRefIssue::MissingSuffix)
                    )
            )
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            syntax: self.syntax.clone(),
            source: rebase_range(self.source, offset)?,
            components: self
                .components
                .iter()
                .map(|component| {
                    Some(SyntaxIdRefComponent::new(
                        component.part(),
                        rebase_range(component.range(), offset)?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            form: self.form.clone(),
            canonical_flow_family: self.canonical_flow_family,
        })
    }
}

/// Four admitted Flow identity states plus marker evidence on missing recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingFlowIdentity {
    Name {
        value: SyntaxName,
        source: SourceRange,
    },
    PublicId(PendingFlowPublicId),
    PublicIdAndName {
        public_id: PendingFlowPublicId,
        name: SyntaxName,
        name_source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
        public_id_recovery: Option<PendingFlowPublicId>,
    },
}

impl PendingFlowIdentity {
    pub(crate) fn has_recovery(&self) -> bool {
        match self {
            Self::Name { .. } => false,
            Self::PublicId(public_id) | Self::PublicIdAndName { public_id, .. } => {
                public_id.has_recovery()
            }
            Self::Missing { .. } => true,
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Name { value, source } => Self::Name {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
            },
            Self::PublicId(public_id) => Self::PublicId(public_id.rebased(offset)?),
            Self::PublicIdAndName {
                public_id,
                name,
                name_source,
            } => Self::PublicIdAndName {
                public_id: public_id.rebased(offset)?,
                name: name.clone(),
                name_source: rebase_range(*name_source, offset)?,
            },
            Self::Missing {
                insertion,
                public_id_recovery,
            } => Self::Missing {
                insertion: rebase_range(*insertion, offset)?,
                public_id_recovery: match public_id_recovery {
                    Some(public_id) => Some(public_id.rebased(offset)?),
                    None => None,
                },
            },
        })
    }
}

/// Source-bound decisions that are not represented by semantic child nodes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingFlowDeclarationProjection {
    keyword: SourceRange,
    identity: PendingFlowIdentity,
    signature_end: SourceRange,
}

impl PendingFlowDeclarationProjection {
    pub(crate) const fn new(
        keyword: SourceRange,
        identity: PendingFlowIdentity,
        signature_end: SourceRange,
    ) -> Self {
        Self {
            keyword,
            identity,
            signature_end,
        }
    }

    pub(crate) const fn keyword(&self) -> SourceRange {
        self.keyword
    }

    pub(crate) const fn identity(&self) -> &PendingFlowIdentity {
        &self.identity
    }

    pub(crate) const fn signature_end(&self) -> SourceRange {
        self.signature_end
    }

    pub(crate) fn ranges_are_valid_for(&self, owner: SourceRange) -> bool {
        token_belongs_to(owner, self.keyword)
            && self.signature_end.start() == self.signature_end.end()
            && owner.start() <= self.signature_end.start()
            && self.signature_end.end() <= owner.end()
            && identity_ranges_are_valid(owner, self.keyword.end(), &self.identity)
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.identity.has_recovery()
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            keyword: rebase_range(self.keyword, offset)?,
            identity: self.identity.rebased(offset)?,
            signature_end: rebase_range(self.signature_end, offset)?,
        })
    }
}

fn identity_ranges_are_valid(
    owner: SourceRange,
    minimum_start: usize,
    identity: &PendingFlowIdentity,
) -> bool {
    let site_after_keyword = |range: SourceRange| {
        owner.start() <= range.start()
            && range.end() <= owner.end()
            && minimum_start <= range.start()
    };
    let token_after_keyword =
        |range: SourceRange| token_belongs_to(owner, range) && site_after_keyword(range);
    match identity {
        PendingFlowIdentity::Name { source, .. } => token_after_keyword(*source),
        PendingFlowIdentity::PublicId(public_id) => {
            token_after_keyword(public_id.source()) && id_components_are_valid(public_id)
        }
        PendingFlowIdentity::PublicIdAndName {
            public_id,
            name_source,
            ..
        } => {
            token_after_keyword(public_id.source())
                && id_components_are_valid(public_id)
                && token_after_keyword(*name_source)
                && public_id.source().end() <= name_source.start()
        }
        PendingFlowIdentity::Missing {
            insertion,
            public_id_recovery,
        } => {
            insertion.start() == insertion.end()
                && site_after_keyword(*insertion)
                && public_id_recovery.as_ref().is_none_or(|public_id| {
                    token_after_keyword(public_id.source())
                        && id_components_are_valid(public_id)
                        && public_id.source().end() <= insertion.start()
                })
        }
    }
}

fn id_components_are_valid(public_id: &PendingFlowPublicId) -> bool {
    let whole_count = public_id
        .components()
        .iter()
        .filter(|component| component.part() == SyntaxIdRefPart::Whole)
        .count();
    whole_count == 1
        && public_id.components().iter().all(|component| {
            let range = component.range();
            public_id.source().start() <= range.start()
                && range.end() <= public_id.source().end()
                && (component.part() != SyntaxIdRefPart::Whole || range == public_id.source())
        })
}

fn token_belongs_to(owner: SourceRange, token: SourceRange) -> bool {
    token.start() < token.end() && owner.start() <= token.start() && token.end() <= owner.end()
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}
