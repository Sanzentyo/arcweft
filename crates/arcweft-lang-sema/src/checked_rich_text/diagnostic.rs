use arcweft_lang_hir::dialogue_application::{
    HirDialogueContentId, HirDialogueNodeId, HirDialoguePointActionArgumentId,
};
use arcweft_lang_hir::source_index::HirSourceSite;

/// Stable semantic diagnostic identity for typed dialogue content.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextDiagnosticCode {
    SchemaUnavailable,
    PositionalForbidden,
    RequiredMissing,
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
    ResourceLimit,
}

impl RichTextDiagnosticCode {
    /// Stable code shared by compiler and tooling projections.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaUnavailable => "sema.rich_text.schema.unavailable",
            Self::PositionalForbidden => "sema.rich_text.argument.positional_forbidden",
            Self::RequiredMissing => "sema.rich_text.argument.required_missing",
            Self::Duplicate => "sema.rich_text.argument.duplicate",
            Self::UnknownProperty => "sema.rich_text.argument.unknown",
            Self::PropertyNotInPhase => "sema.rich_text.argument.property_not_in_phase",
            Self::Conflict => "sema.rich_text.argument.conflict",
            Self::InvalidKind => "sema.rich_text.argument.invalid_kind",
            Self::InvalidBoolean => "sema.rich_text.argument.invalid_boolean",
            Self::InvalidInteger => "sema.rich_text.argument.invalid_integer",
            Self::InvalidDecimal => "sema.rich_text.argument.invalid_decimal",
            Self::NonFinite => "sema.rich_text.argument.non_finite",
            Self::Overflow => "sema.rich_text.argument.overflow",
            Self::Underflow => "sema.rich_text.argument.underflow",
            Self::Negative => "sema.rich_text.argument.negative",
            Self::OutOfRange => "sema.rich_text.argument.out_of_range",
            Self::InvalidUnit => "sema.rich_text.argument.invalid_unit",
            Self::InvalidEnum => "sema.rich_text.argument.invalid_enum",
            Self::InvalidSelector => "sema.rich_text.selector.invalid",
            Self::EmptyValue => "sema.rich_text.argument.empty",
            Self::InvalidColor => "sema.rich_text.argument.invalid_color",
            Self::InvalidVec2 => "sema.rich_text.argument.invalid_vec2",
            Self::InvalidDuration => "sema.rich_text.argument.invalid_duration",
            Self::InvalidArgument => "sema.rich_text.argument.invalid",
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

/// Required recovery/execution effect of a typed RichText failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextFailureEffect {
    RejectPointEvent,
    RejectCompilation,
}

/// Final-HIR owner responsible for one diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextDiagnosticOwner {
    Content(HirDialogueContentId),
    Node(HirDialogueNodeId),
    PointAction(HirDialogueNodeId),
    Argument(HirDialoguePointActionArgumentId),
}

/// Complete structured diagnostic bound to final-HIR identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RichTextDiagnostic {
    code: RichTextDiagnosticCode,
    owner: RichTextDiagnosticOwner,
    primary: HirSourceSite,
    related: Vec<RichTextRelatedSite>,
    effect: RichTextFailureEffect,
}

impl RichTextDiagnostic {
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

    pub const fn point_action(&self) -> Option<HirDialogueNodeId> {
        match self.owner {
            RichTextDiagnosticOwner::PointAction(action) => Some(action),
            RichTextDiagnosticOwner::Content(_)
            | RichTextDiagnosticOwner::Node(_)
            | RichTextDiagnosticOwner::Argument(_) => None,
        }
    }

    pub const fn argument(&self) -> Option<HirDialoguePointActionArgumentId> {
        match self.owner {
            RichTextDiagnosticOwner::Argument(argument) => Some(argument),
            RichTextDiagnosticOwner::Content(_)
            | RichTextDiagnosticOwner::Node(_)
            | RichTextDiagnosticOwner::PointAction(_) => None,
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
