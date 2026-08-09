use arcweft_lang_hir::dialogue_application::{
    HirDialogueContentId, HirDialogueNodeId, HirRichTextArgumentId, HirRichTextTagId,
};
use arcweft_lang_hir::source_index::HirSourceSite;

/// Stable semantic `RichText` diagnostic identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextDiagnosticCode {
    UnknownTag,
    UnknownSelector,
    SchemaUnavailable,
    PositionalForbidden,
    PositionalArity,
    MixedForms,
    RequiredMissing,
    UnexpectedValue,
    Duplicate,
    UnknownProperty,
    PropertyNotInPhase,
    Conflict,
    InvalidKind,
    InvalidBoolean,
    InvalidInteger,
    InvalidDecimal,
    NonFinite,
    Overflow,
    Underflow,
    Negative,
    OutOfRange,
    InvalidUnit,
    InvalidEnum,
    InvalidSelector,
    EmptyValue,
    InvalidColor,
    InvalidVec2,
    InvalidDuration,
    InvalidArgument,
    NestingLimit,
    CrossingSpan,
    UnmatchedClose,
    UnclosedSpan,
    ResourceLimit,
}

impl RichTextDiagnosticCode {
    /// Stable code shared by compiler and tooling projections.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownTag => "sema.rich_text.tag.unknown",
            Self::UnknownSelector => "sema.rich_text.selector.unknown",
            Self::SchemaUnavailable => "sema.rich_text.schema.unavailable",
            Self::PositionalForbidden => "sema.rich_text.attribute.positional_forbidden",
            Self::PositionalArity => "sema.rich_text.attribute.positional_arity",
            Self::MixedForms => "sema.rich_text.attribute.mixed_forms",
            Self::RequiredMissing => "sema.rich_text.attribute.required_missing",
            Self::UnexpectedValue => "sema.rich_text.attribute.unexpected_value",
            Self::Duplicate => "sema.rich_text.attribute.duplicate",
            Self::UnknownProperty => "sema.rich_text.attribute.unknown",
            Self::PropertyNotInPhase => "sema.rich_text.attribute.property_not_in_phase",
            Self::Conflict => "sema.rich_text.attribute.conflict",
            Self::InvalidKind => "sema.rich_text.attribute.invalid_kind",
            Self::InvalidBoolean => "sema.rich_text.attribute.invalid_boolean",
            Self::InvalidInteger => "sema.rich_text.attribute.invalid_integer",
            Self::InvalidDecimal => "sema.rich_text.attribute.invalid_decimal",
            Self::NonFinite => "sema.rich_text.attribute.non_finite",
            Self::Overflow => "sema.rich_text.attribute.overflow",
            Self::Underflow => "sema.rich_text.attribute.underflow",
            Self::Negative => "sema.rich_text.attribute.negative",
            Self::OutOfRange => "sema.rich_text.attribute.out_of_range",
            Self::InvalidUnit => "sema.rich_text.attribute.invalid_unit",
            Self::InvalidEnum => "sema.rich_text.attribute.invalid_enum",
            Self::InvalidSelector => "sema.rich_text.attribute.invalid_selector",
            Self::EmptyValue => "sema.rich_text.attribute.empty_value",
            Self::InvalidColor => "sema.rich_text.attribute.invalid_color",
            Self::InvalidVec2 => "sema.rich_text.attribute.invalid_vec2",
            Self::InvalidDuration => "sema.rich_text.attribute.invalid_duration",
            Self::InvalidArgument => "sema.rich_text.attribute.invalid",
            Self::NestingLimit => "sema.rich_text.span.nesting_limit",
            Self::CrossingSpan => "sema.rich_text.span.crossing",
            Self::UnmatchedClose => "sema.rich_text.span.unmatched_close",
            Self::UnclosedSpan => "sema.rich_text.span.unclosed",
            Self::ResourceLimit => "sema.rich_text.resource_limit",
        }
    }
}

/// One typed related source component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RichTextRelatedSite {
    site: HirSourceSite,
    label: &'static str,
}

impl RichTextRelatedSite {
    pub(crate) const fn new(site: HirSourceSite, label: &'static str) -> Self {
        Self { site, label }
    }

    pub const fn site(&self) -> &HirSourceSite {
        &self.site
    }

    pub const fn label(&self) -> &'static str {
        self.label
    }
}

/// Required recovery/execution effect of a `RichText` failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextFailureEffect {
    RejectTag,
    RejectPointEvent,
    RejectCompilation,
}

/// Final-HIR owner responsible for one diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextDiagnosticOwner {
    Content(HirDialogueContentId),
    Node(HirDialogueNodeId),
    Tag(HirRichTextTagId),
    Argument(HirRichTextArgumentId),
}

/// Complete structured `RichText` diagnostic bound to final-HIR identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RichTextAttributeDiagnostic {
    code: RichTextDiagnosticCode,
    owner: RichTextDiagnosticOwner,
    primary: HirSourceSite,
    related: Vec<RichTextRelatedSite>,
    effect: RichTextFailureEffect,
}

impl RichTextAttributeDiagnostic {
    pub(crate) const fn new(
        code: RichTextDiagnosticCode,
        owner: RichTextDiagnosticOwner,
        primary: HirSourceSite,
        effect: RichTextFailureEffect,
    ) -> Self {
        Self {
            code,
            owner,
            primary,
            related: Vec::new(),
            effect,
        }
    }

    pub(crate) fn with_related(mut self, related: RichTextRelatedSite) -> Self {
        self.related.push(related);
        self
    }

    pub const fn code(&self) -> RichTextDiagnosticCode {
        self.code
    }

    pub const fn owner(&self) -> RichTextDiagnosticOwner {
        self.owner
    }

    pub const fn tag(&self) -> Option<HirRichTextTagId> {
        match self.owner {
            RichTextDiagnosticOwner::Tag(tag) => Some(tag),
            RichTextDiagnosticOwner::Argument(argument) => Some(argument.tag()),
            RichTextDiagnosticOwner::Content(_) | RichTextDiagnosticOwner::Node(_) => None,
        }
    }

    pub const fn argument(&self) -> Option<HirRichTextArgumentId> {
        match self.owner {
            RichTextDiagnosticOwner::Argument(argument) => Some(argument),
            RichTextDiagnosticOwner::Content(_)
            | RichTextDiagnosticOwner::Node(_)
            | RichTextDiagnosticOwner::Tag(_) => None,
        }
    }

    pub const fn primary(&self) -> &HirSourceSite {
        &self.primary
    }

    pub fn related(&self) -> &[RichTextRelatedSite] {
        &self.related
    }

    pub const fn effect(&self) -> RichTextFailureEffect {
        self.effect
    }
}
