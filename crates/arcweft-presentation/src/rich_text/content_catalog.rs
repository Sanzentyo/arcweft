//! Immutable presentation-owned Content callable schemas.
//!
//! This module is the presentation half of the Content callable boundary. It
//! contains only typed, dependency-neutral descriptors. Semantic checking
//! projects these rows into its own callable schema; it does not re-create
//! selector branches from source names.

use arcweft_rich_text_schema::{
    CheckedOutputKind, PropertyPresence, RichTextCallableParameterPassing,
    RichTextCallableParameterPresence, RichTextCallableParameterSpec, RichTextCallableSchemaDigest,
    RichTextDefaultValue, RichTextEnumDomain, RichTextEnumValueConstraint, RichTextNumericLimits,
    RichTextPropertyPredicate, RichTextUnit, RichTextValueKind, RichTextValueLimits,
};

use super::{
    RichTextLayoutProperty, RichTextLayoutSelector, RichTextStyleProperty, RichTextStyleSelector,
    RichTextTransformProperty, RichTextTransformSelector,
};

/// Typed identity of one parameter in a presentation Content callable row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationContentCallableParameterId {
    /// Direct scalar value of `color`, `font`, `size`, or `ruby`.
    Value,
    /// Selector discriminator of a style/layout/transform branch.
    Selector,
    /// Typed `Fx` application value.
    Fx,
    /// Style branch property.
    Style(RichTextStyleProperty),
    /// Layout branch property.
    Layout(RichTextLayoutProperty),
    /// Transform branch property.
    Transform(RichTextTransformProperty),
}

/// Concrete parameter descriptor used by presentation Content rows.
pub type PresentationContentCallableParameterSpec =
    RichTextCallableParameterSpec<PresentationContentCallableParameterId>;

/// Canonical implicit Content head.
///
/// A head names the source-level callable namespace. It is intentionally
/// separate from [`PresentationContentEmissionFamily`]: one head such as
/// `style` owns several selector rows, while the emission family describes
/// what the selected row produces.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationContentCallableHead {
    Strong,
    Em,
    Color,
    Font,
    Size,
    Style,
    Layout,
    Transform,
    Fx,
    Ruby,
    Raw,
}

impl PresentationContentCallableHead {
    /// Returns the canonical source head spelling owned by this row family.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Em => "em",
            Self::Color => "color",
            Self::Font => "font",
            Self::Size => "size",
            Self::Style => "style",
            Self::Layout => "layout",
            Self::Transform => "transform",
            Self::Fx => "fx",
            Self::Ruby => "ruby",
            Self::Raw => "raw",
        }
    }

    /// Stable canonical tag for the head identity.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Strong => 0,
            Self::Em => 1,
            Self::Color => 2,
            Self::Font => 3,
            Self::Size => 4,
            Self::Style => 5,
            Self::Layout => 6,
            Self::Transform => 7,
            Self::Fx => 8,
            Self::Ruby => 9,
            Self::Raw => 10,
        }
    }

    const fn for_definition(id: PresentationContentCallableDefinitionId) -> Self {
        match id {
            PresentationContentCallableDefinitionId::Strong => Self::Strong,
            PresentationContentCallableDefinitionId::Em => Self::Em,
            PresentationContentCallableDefinitionId::Color => Self::Color,
            PresentationContentCallableDefinitionId::Font => Self::Font,
            PresentationContentCallableDefinitionId::Size => Self::Size,
            PresentationContentCallableDefinitionId::Style(_) => Self::Style,
            PresentationContentCallableDefinitionId::Layout(_) => Self::Layout,
            PresentationContentCallableDefinitionId::Transform(_) => Self::Transform,
            PresentationContentCallableDefinitionId::Fx => Self::Fx,
            PresentationContentCallableDefinitionId::Ruby => Self::Ruby,
            PresentationContentCallableDefinitionId::Raw => Self::Raw,
        }
    }

    const ALL: [Self; 11] = [
        Self::Strong,
        Self::Em,
        Self::Color,
        Self::Font,
        Self::Size,
        Self::Style,
        Self::Layout,
        Self::Transform,
        Self::Fx,
        Self::Ruby,
        Self::Raw,
    ];
}

/// Attached-body admission policy owned by one presentation Content callable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationContentAttachedBodyPolicy {
    /// Preserve the caller's checked body role while styling its fragments.
    PreserveBodyRole,
    /// Admit only an `InlineContent` body.
    InlineOnly,
    /// Admit only a `LiteralContent` body.
    LiteralOnly,
}

/// Renderer-neutral emission family of one Content callable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationContentEmissionFamily {
    /// Strong emphasis span.
    Strong,
    /// Emphasis span.
    Em,
    /// Color span.
    Color,
    /// Font span.
    Font,
    /// Size span.
    Size,
    /// Style selector span.
    Style,
    /// Layout selector span.
    Layout,
    /// Transform selector span.
    Transform,
    /// Typed reusable Fx span.
    Fx,
    /// Ruby content node.
    Ruby,
    /// Literal content node.
    Raw,
}

/// Closed presentation Content callable identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationContentCallableDefinitionId {
    /// Strong emphasis.
    Strong,
    /// Emphasis.
    Em,
    /// Foreground color.
    Color,
    /// Font family.
    Font,
    /// Font size.
    Size,
    /// Style selector branch.
    Style(RichTextStyleSelector),
    /// Layout selector branch.
    Layout(RichTextLayoutSelector),
    /// Transform selector branch.
    Transform(RichTextTransformSelector),
    /// Reusable typed Fx application.
    Fx,
    /// Ruby content.
    Ruby,
    /// Literal content.
    Raw,
}

/// One immutable presentation Content callable row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationContentCallableDefinition {
    /// Source-level implicit head owning this row.
    head: PresentationContentCallableHead,
    /// Closed row identity.
    id: PresentationContentCallableDefinitionId,
    /// Exact parameters in source/application order.
    parameters: &'static [PresentationContentCallableParameterSpec],
    /// Attached body policy.
    attached_body_policy: PresentationContentAttachedBodyPolicy,
    /// Renderer-neutral emission family.
    emission_family: PresentationContentEmissionFamily,
    /// Coarse output family retained for shared rich-text schema consumers.
    output: CheckedOutputKind,
}

impl PresentationContentCallableDefinition {
    const fn new(
        id: PresentationContentCallableDefinitionId,
        parameters: &'static [PresentationContentCallableParameterSpec],
        attached_body_policy: PresentationContentAttachedBodyPolicy,
        emission_family: PresentationContentEmissionFamily,
        output: CheckedOutputKind,
    ) -> Self {
        Self {
            head: PresentationContentCallableHead::for_definition(id),
            id,
            parameters,
            attached_body_policy,
            emission_family,
            output,
        }
    }

    /// Returns the stable digest of this complete row.
    #[must_use]
    pub fn schema_digest(&self) -> RichTextCallableSchemaDigest {
        RichTextCallableSchemaDigest::derive(&canonical_row_bytes(self))
    }

    /// Alias used by digest-oriented consumers.
    #[must_use]
    pub fn digest(&self) -> RichTextCallableSchemaDigest {
        self.schema_digest()
    }

    /// Returns the exact row identity.
    #[must_use]
    pub const fn id(&self) -> PresentationContentCallableDefinitionId {
        self.id
    }

    /// Returns the source-level implicit head owning this row.
    #[must_use]
    pub const fn head(&self) -> PresentationContentCallableHead {
        self.head
    }

    /// Returns the exact ordered parameter descriptors.
    #[must_use]
    pub const fn parameters(&self) -> &'static [PresentationContentCallableParameterSpec] {
        self.parameters
    }

    /// Returns this row's attached-body policy.
    #[must_use]
    pub const fn attached_body_policy(&self) -> PresentationContentAttachedBodyPolicy {
        self.attached_body_policy
    }

    /// Returns this row's renderer-neutral emission family.
    #[must_use]
    pub const fn emission_family(&self) -> PresentationContentEmissionFamily {
        self.emission_family
    }

    /// Returns the shared coarse output family.
    #[must_use]
    pub const fn output(&self) -> CheckedOutputKind {
        self.output
    }
}

/// Immutable catalog of all presentation-owned Content callable rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationContentCallableCatalog {
    rows: &'static [PresentationContentCallableDefinition],
}

impl PresentationContentCallableCatalog {
    /// Constructs an immutable catalog over static rows.
    #[must_use]
    const fn new(rows: &'static [PresentationContentCallableDefinition]) -> Self {
        Self { rows }
    }

    /// Complete static row inventory in deterministic order.
    #[must_use]
    pub const fn rows(&self) -> &'static [PresentationContentCallableDefinition] {
        self.rows
    }

    /// Alias for consumers that call the inventory `definitions`.
    #[must_use]
    pub const fn definitions(&self) -> &'static [PresentationContentCallableDefinition] {
        self.rows
    }

    /// Number of immutable rows.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether this catalog contains no rows.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Iterates rows in canonical identity order.
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &'static PresentationContentCallableDefinition> {
        self.rows.iter()
    }

    /// Resolves one exact typed definition identity.
    #[must_use]
    pub fn get(
        &self,
        id: PresentationContentCallableDefinitionId,
    ) -> Option<&'static PresentationContentCallableDefinition> {
        self.rows.iter().find(|row| row.id == id)
    }

    /// Resolves one exact typed definition identity.
    #[must_use]
    pub fn definition(
        &self,
        id: PresentationContentCallableDefinitionId,
    ) -> Option<&'static PresentationContentCallableDefinition> {
        self.get(id)
    }

    /// Resolves one canonical implicit Content head from the authored name.
    /// Unknown heads are not mapped to an operation or fallback row.
    #[must_use]
    pub fn resolve_implicit_head(
        &self,
        source_name: &str,
    ) -> Option<PresentationContentCallableHead> {
        PresentationContentCallableHead::ALL
            .into_iter()
            .find(|head| {
                head.source_name() == source_name && self.rows_for_head(*head).next().is_some()
            })
    }

    /// Returns all exact definition rows owned by one head in canonical order.
    pub fn rows_for_head(
        &self,
        head: PresentationContentCallableHead,
    ) -> impl Iterator<Item = &'static PresentationContentCallableDefinition> {
        self.rows.iter().filter(move |row| row.head == head)
    }

    /// Stable digest of the complete catalog row sequence.
    #[must_use]
    pub fn schema_digest(&self) -> RichTextCallableSchemaDigest {
        let mut bytes = Vec::new();
        bytes.push(1);
        push_usize(&mut bytes, self.rows.len());
        for row in self.rows {
            bytes.extend_from_slice(row.schema_digest().as_bytes());
        }
        RichTextCallableSchemaDigest::derive(&bytes)
    }

    /// Stable digest of the complete catalog row sequence.
    #[must_use]
    pub fn digest(&self) -> RichTextCallableSchemaDigest {
        self.schema_digest()
    }
}

impl Default for PresentationContentCallableCatalog {
    fn default() -> Self {
        PRESENTATION_CONTENT_CALLABLE_CATALOG
    }
}

/// The one published presentation Content callable catalog.
pub const PRESENTATION_CONTENT_CALLABLE_CATALOG: PresentationContentCallableCatalog =
    PresentationContentCallableCatalog::new(CONTENT_CALLABLE_DEFINITIONS);

const NO_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[];

const SELECTOR_LIMITS: RichTextValueLimits = closed_enum_limits();
const LAYOUT_SELECTOR_LIMITS: RichTextValueLimits = closed_enum_limits();
const TRANSFORM_SELECTOR_LIMITS: RichTextValueLimits = closed_enum_limits();
const COLOR_LIMITS: RichTextValueLimits = text_limits(4_096, 4_096);
const STRING_LIMITS: RichTextValueLimits = text_limits(4_096, 4_096);
const FX_LIMITS: RichTextValueLimits = text_limits(4_096, 4_096);
const SIZE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(1_000),
        inclusive_max_milli: Some(512_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Pt],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};

const fn closed_enum_limits() -> RichTextValueLimits {
    RichTextValueLimits {
        numeric: None,
        units: &[],
        // Membership is carried by the typed `enum_constraint` below. Keep
        // this diagnostic-only legacy slot empty so it cannot become a
        // parallel source-string authority.
        enum_values: &[],
        max_encoded_bytes: 64,
        max_decoded_bytes: 64,
    }
}

const fn text_limits(encoded: u16, decoded: u16) -> RichTextValueLimits {
    RichTextValueLimits {
        numeric: None,
        units: &[],
        enum_values: &[],
        max_encoded_bytes: encoded,
        max_decoded_bytes: decoded,
    }
}

const fn style_predicate(
    predicate: RichTextPropertyPredicate<RichTextStyleProperty>,
) -> RichTextPropertyPredicate<PresentationContentCallableParameterId> {
    match predicate {
        RichTextPropertyPredicate::Present(property) => RichTextPropertyPredicate::Present(
            PresentationContentCallableParameterId::Style(property),
        ),
        RichTextPropertyPredicate::BoolEquals { property, value } => {
            RichTextPropertyPredicate::BoolEquals {
                property: PresentationContentCallableParameterId::Style(property),
                value,
            }
        }
        RichTextPropertyPredicate::EnumEquals { property, variant } => {
            RichTextPropertyPredicate::EnumEquals {
                property: PresentationContentCallableParameterId::Style(property),
                variant,
            }
        }
    }
}

const fn layout_predicate(
    predicate: RichTextPropertyPredicate<RichTextLayoutProperty>,
) -> RichTextPropertyPredicate<PresentationContentCallableParameterId> {
    match predicate {
        RichTextPropertyPredicate::Present(property) => RichTextPropertyPredicate::Present(
            PresentationContentCallableParameterId::Layout(property),
        ),
        RichTextPropertyPredicate::BoolEquals { property, value } => {
            RichTextPropertyPredicate::BoolEquals {
                property: PresentationContentCallableParameterId::Layout(property),
                value,
            }
        }
        RichTextPropertyPredicate::EnumEquals { property, variant } => {
            RichTextPropertyPredicate::EnumEquals {
                property: PresentationContentCallableParameterId::Layout(property),
                variant,
            }
        }
    }
}

const fn transform_predicate(
    predicate: RichTextPropertyPredicate<RichTextTransformProperty>,
) -> RichTextPropertyPredicate<PresentationContentCallableParameterId> {
    match predicate {
        RichTextPropertyPredicate::Present(property) => RichTextPropertyPredicate::Present(
            PresentationContentCallableParameterId::Transform(property),
        ),
        RichTextPropertyPredicate::BoolEquals { property, value } => {
            RichTextPropertyPredicate::BoolEquals {
                property: PresentationContentCallableParameterId::Transform(property),
                value,
            }
        }
        RichTextPropertyPredicate::EnumEquals { property, variant } => {
            RichTextPropertyPredicate::EnumEquals {
                property: PresentationContentCallableParameterId::Transform(property),
                variant,
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ParameterMetadata {
    passing: RichTextCallableParameterPassing,
    presence: RichTextCallableParameterPresence<PresentationContentCallableParameterId>,
    enum_constraint: RichTextEnumValueConstraint,
    conditional_default: Option<RichTextDefaultValue>,
    limits: RichTextValueLimits,
    allow_empty: bool,
}

const fn parameter_metadata(
    passing: RichTextCallableParameterPassing,
    presence: RichTextCallableParameterPresence<PresentationContentCallableParameterId>,
    enum_constraint: RichTextEnumValueConstraint,
    conditional_default: Option<RichTextDefaultValue>,
    limits: RichTextValueLimits,
    allow_empty: bool,
) -> ParameterMetadata {
    ParameterMetadata {
        passing,
        presence,
        enum_constraint,
        conditional_default,
        limits,
        allow_empty,
    }
}

const fn parameter(
    id: PresentationContentCallableParameterId,
    source_name: &'static str,
    kind: RichTextValueKind,
    metadata: ParameterMetadata,
) -> PresentationContentCallableParameterSpec {
    RichTextCallableParameterSpec {
        id,
        source_name,
        kind,
        passing: metadata.passing,
        presence: metadata.presence,
        conditional_default: metadata.conditional_default,
        enum_constraint: metadata.enum_constraint,
        limits: metadata.limits,
        allow_empty: metadata.allow_empty,
    }
}

const fn required_positional(
    id: PresentationContentCallableParameterId,
    source_name: &'static str,
    kind: RichTextValueKind,
    limits: RichTextValueLimits,
) -> PresentationContentCallableParameterSpec {
    parameter(
        id,
        source_name,
        kind,
        parameter_metadata(
            RichTextCallableParameterPassing::PositionalOnly,
            RichTextCallableParameterPresence::Required,
            RichTextEnumValueConstraint::All,
            None,
            limits,
            false,
        ),
    )
}

const fn property_parameter(
    id: PresentationContentCallableParameterId,
    source_name: &'static str,
    kind: RichTextValueKind,
    presence: RichTextCallableParameterPresence<PresentationContentCallableParameterId>,
    limits: RichTextValueLimits,
    allow_empty: bool,
) -> PresentationContentCallableParameterSpec {
    parameter(
        id,
        source_name,
        kind,
        parameter_metadata(
            RichTextCallableParameterPassing::NamedOnly,
            presence,
            RichTextEnumValueConstraint::All,
            None,
            limits,
            allow_empty,
        ),
    )
}

const fn selector_parameter(
    domain: arcweft_id::closed_enum::ClosedEnumDomainId,
    ordinal: u16,
    limits: RichTextValueLimits,
) -> PresentationContentCallableParameterSpec {
    parameter(
        PresentationContentCallableParameterId::Selector,
        "selector",
        RichTextValueKind::ClosedEnum(domain),
        parameter_metadata(
            RichTextCallableParameterPassing::PositionalOnly,
            RichTextCallableParameterPresence::Required,
            RichTextEnumValueConstraint::Exact(ordinal),
            None,
            limits,
            false,
        ),
    )
}

const fn style_selector_parameter(
    selector: RichTextStyleSelector,
) -> PresentationContentCallableParameterSpec {
    selector_parameter(
        RichTextEnumDomain::StyleSelector.domain_id(),
        selector.ordinal(),
        SELECTOR_LIMITS,
    )
}

const fn layout_selector_parameter(
    selector: RichTextLayoutSelector,
) -> PresentationContentCallableParameterSpec {
    selector_parameter(
        RichTextEnumDomain::LayoutSelector.domain_id(),
        selector.ordinal(),
        LAYOUT_SELECTOR_LIMITS,
    )
}

const fn transform_selector_parameter(
    selector: RichTextTransformSelector,
) -> PresentationContentCallableParameterSpec {
    selector_parameter(
        RichTextEnumDomain::TransformSelector.domain_id(),
        selector.ordinal(),
        TRANSFORM_SELECTOR_LIMITS,
    )
}

const fn style_property(
    selector: RichTextStyleSelector,
    index: usize,
) -> PresentationContentCallableParameterSpec {
    let spec = selector.property_schema().properties[index];
    let presence = match spec.presence {
        PropertyPresence::Required => RichTextCallableParameterPresence::Required,
        PropertyPresence::Optional => RichTextCallableParameterPresence::Optional,
        PropertyPresence::Defaulted(value) => RichTextCallableParameterPresence::Defaulted(value),
        PropertyPresence::Conditional { predicate } => {
            RichTextCallableParameterPresence::Conditional {
                predicate: style_predicate(predicate),
            }
        }
    };
    let limits = match spec.kind {
        RichTextValueKind::ClosedEnum(_) => closed_enum_limits(),
        _ => spec.limits,
    };
    property_parameter(
        PresentationContentCallableParameterId::Style(spec.id),
        spec.source_name,
        spec.kind,
        presence,
        limits,
        spec.allow_empty,
    )
}

const fn layout_property(
    selector: RichTextLayoutSelector,
    index: usize,
) -> PresentationContentCallableParameterSpec {
    let spec = selector.property_schema().properties[index];
    let presence = match spec.presence {
        PropertyPresence::Required => RichTextCallableParameterPresence::Required,
        PropertyPresence::Optional => RichTextCallableParameterPresence::Optional,
        PropertyPresence::Defaulted(value) => RichTextCallableParameterPresence::Defaulted(value),
        PropertyPresence::Conditional { predicate } => {
            RichTextCallableParameterPresence::Conditional {
                predicate: layout_predicate(predicate),
            }
        }
    };
    let limits = match spec.kind {
        RichTextValueKind::ClosedEnum(_) => closed_enum_limits(),
        _ => spec.limits,
    };
    property_parameter(
        PresentationContentCallableParameterId::Layout(spec.id),
        spec.source_name,
        spec.kind,
        presence,
        limits,
        spec.allow_empty,
    )
}

const fn transform_property(
    selector: RichTextTransformSelector,
    index: usize,
) -> PresentationContentCallableParameterSpec {
    let spec = selector.property_schema().properties[index];
    let presence = match spec.presence {
        PropertyPresence::Required => RichTextCallableParameterPresence::Required,
        PropertyPresence::Optional => RichTextCallableParameterPresence::Optional,
        PropertyPresence::Defaulted(value) => RichTextCallableParameterPresence::Defaulted(value),
        PropertyPresence::Conditional { predicate } => {
            RichTextCallableParameterPresence::Conditional {
                predicate: transform_predicate(predicate),
            }
        }
    };
    let limits = match spec.kind {
        RichTextValueKind::ClosedEnum(_) => closed_enum_limits(),
        _ => spec.limits,
    };
    property_parameter(
        PresentationContentCallableParameterId::Transform(spec.id),
        spec.source_name,
        spec.kind,
        presence,
        limits,
        spec.allow_empty,
    )
}

const STYLE_ITALIC_PARAMETERS: &[PresentationContentCallableParameterSpec] =
    &[style_selector_parameter(RichTextStyleSelector::Italic)];
const STYLE_OBLIQUE_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    style_selector_parameter(RichTextStyleSelector::Oblique),
    style_property(RichTextStyleSelector::Oblique, 0),
];
const STYLE_OPACITY_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    style_selector_parameter(RichTextStyleSelector::Opacity),
    style_property(RichTextStyleSelector::Opacity, 0),
];
const STYLE_LAYER_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    style_selector_parameter(RichTextStyleSelector::Layer),
    style_property(RichTextStyleSelector::Layer, 0),
];
const STYLE_Z_INDEX_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    style_selector_parameter(RichTextStyleSelector::ZIndex),
    style_property(RichTextStyleSelector::ZIndex, 0),
];

const LAYOUT_HORIZONTAL_TB_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    layout_selector_parameter(RichTextLayoutSelector::HorizontalTb),
    layout_property(RichTextLayoutSelector::HorizontalTb, 0),
    layout_property(RichTextLayoutSelector::HorizontalTb, 1),
    layout_property(RichTextLayoutSelector::HorizontalTb, 2),
    layout_property(RichTextLayoutSelector::HorizontalTb, 3),
    layout_property(RichTextLayoutSelector::HorizontalTb, 4),
    layout_property(RichTextLayoutSelector::HorizontalTb, 5),
    layout_property(RichTextLayoutSelector::HorizontalTb, 6),
    layout_property(RichTextLayoutSelector::HorizontalTb, 7),
];
const LAYOUT_VERTICAL_RL_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    layout_selector_parameter(RichTextLayoutSelector::VerticalRl),
    layout_property(RichTextLayoutSelector::VerticalRl, 0),
    layout_property(RichTextLayoutSelector::VerticalRl, 1),
    layout_property(RichTextLayoutSelector::VerticalRl, 2),
    layout_property(RichTextLayoutSelector::VerticalRl, 3),
    layout_property(RichTextLayoutSelector::VerticalRl, 4),
    layout_property(RichTextLayoutSelector::VerticalRl, 5),
    layout_property(RichTextLayoutSelector::VerticalRl, 6),
    layout_property(RichTextLayoutSelector::VerticalRl, 7),
];
const LAYOUT_VERTICAL_LR_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    layout_selector_parameter(RichTextLayoutSelector::VerticalLr),
    layout_property(RichTextLayoutSelector::VerticalLr, 0),
    layout_property(RichTextLayoutSelector::VerticalLr, 1),
    layout_property(RichTextLayoutSelector::VerticalLr, 2),
    layout_property(RichTextLayoutSelector::VerticalLr, 3),
    layout_property(RichTextLayoutSelector::VerticalLr, 4),
    layout_property(RichTextLayoutSelector::VerticalLr, 5),
    layout_property(RichTextLayoutSelector::VerticalLr, 6),
    layout_property(RichTextLayoutSelector::VerticalLr, 7),
];
const LAYOUT_DIRECTION_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    layout_selector_parameter(RichTextLayoutSelector::Direction),
    layout_property(RichTextLayoutSelector::Direction, 0),
    layout_property(RichTextLayoutSelector::Direction, 1),
    layout_property(RichTextLayoutSelector::Direction, 2),
    layout_property(RichTextLayoutSelector::Direction, 3),
    layout_property(RichTextLayoutSelector::Direction, 4),
    layout_property(RichTextLayoutSelector::Direction, 5),
    layout_property(RichTextLayoutSelector::Direction, 6),
    layout_property(RichTextLayoutSelector::Direction, 7),
];
const LAYOUT_RUBY_OVER_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    layout_selector_parameter(RichTextLayoutSelector::RubyOver),
    layout_property(RichTextLayoutSelector::RubyOver, 0),
    layout_property(RichTextLayoutSelector::RubyOver, 1),
    layout_property(RichTextLayoutSelector::RubyOver, 2),
    layout_property(RichTextLayoutSelector::RubyOver, 3),
    layout_property(RichTextLayoutSelector::RubyOver, 4),
    layout_property(RichTextLayoutSelector::RubyOver, 5),
    layout_property(RichTextLayoutSelector::RubyOver, 6),
    layout_property(RichTextLayoutSelector::RubyOver, 7),
];
const LAYOUT_RUBY_UNDER_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    layout_selector_parameter(RichTextLayoutSelector::RubyUnder),
    layout_property(RichTextLayoutSelector::RubyUnder, 0),
    layout_property(RichTextLayoutSelector::RubyUnder, 1),
    layout_property(RichTextLayoutSelector::RubyUnder, 2),
    layout_property(RichTextLayoutSelector::RubyUnder, 3),
    layout_property(RichTextLayoutSelector::RubyUnder, 4),
    layout_property(RichTextLayoutSelector::RubyUnder, 5),
    layout_property(RichTextLayoutSelector::RubyUnder, 6),
    layout_property(RichTextLayoutSelector::RubyUnder, 7),
];
const LAYOUT_RUBY_INTER_CHARACTER_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    layout_selector_parameter(RichTextLayoutSelector::RubyInterCharacter),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 0),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 1),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 2),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 3),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 4),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 5),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 6),
    layout_property(RichTextLayoutSelector::RubyInterCharacter, 7),
];

const TRANSFORM_OFFSET_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    transform_selector_parameter(RichTextTransformSelector::Offset),
    transform_property(RichTextTransformSelector::Offset, 0),
    transform_property(RichTextTransformSelector::Offset, 1),
    transform_property(RichTextTransformSelector::Offset, 2),
    transform_property(RichTextTransformSelector::Offset, 3),
];
const TRANSFORM_ROTATE_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    transform_selector_parameter(RichTextTransformSelector::Rotate),
    transform_property(RichTextTransformSelector::Rotate, 0),
    transform_property(RichTextTransformSelector::Rotate, 1),
    transform_property(RichTextTransformSelector::Rotate, 2),
];
const TRANSFORM_SCALE_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    transform_selector_parameter(RichTextTransformSelector::Scale),
    transform_property(RichTextTransformSelector::Scale, 0),
    transform_property(RichTextTransformSelector::Scale, 1),
    transform_property(RichTextTransformSelector::Scale, 2),
    transform_property(RichTextTransformSelector::Scale, 3),
];
const TRANSFORM_SKEW_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[
    transform_selector_parameter(RichTextTransformSelector::Skew),
    transform_property(RichTextTransformSelector::Skew, 0),
    transform_property(RichTextTransformSelector::Skew, 1),
    transform_property(RichTextTransformSelector::Skew, 2),
    transform_property(RichTextTransformSelector::Skew, 3),
];

const COLOR_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[required_positional(
    PresentationContentCallableParameterId::Value,
    "value",
    RichTextValueKind::Color,
    COLOR_LIMITS,
)];
const FONT_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[required_positional(
    PresentationContentCallableParameterId::Value,
    "value",
    RichTextValueKind::Text,
    STRING_LIMITS,
)];
const SIZE_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[required_positional(
    PresentationContentCallableParameterId::Value,
    "value",
    RichTextValueKind::Length,
    SIZE_LIMITS,
)];
const RUBY_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[required_positional(
    PresentationContentCallableParameterId::Value,
    "reading",
    RichTextValueKind::Text,
    STRING_LIMITS,
)];
const FX_PARAMETERS: &[PresentationContentCallableParameterSpec] = &[required_positional(
    PresentationContentCallableParameterId::Fx,
    "fx",
    RichTextValueKind::Fx,
    FX_LIMITS,
)];

const fn direct_definition(
    id: PresentationContentCallableDefinitionId,
    parameters: &'static [PresentationContentCallableParameterSpec],
    family: PresentationContentEmissionFamily,
    output: CheckedOutputKind,
) -> PresentationContentCallableDefinition {
    PresentationContentCallableDefinition::new(
        id,
        parameters,
        PresentationContentAttachedBodyPolicy::PreserveBodyRole,
        family,
        output,
    )
}

const fn style_definition(
    selector: RichTextStyleSelector,
    parameters: &'static [PresentationContentCallableParameterSpec],
) -> PresentationContentCallableDefinition {
    direct_definition(
        PresentationContentCallableDefinitionId::Style(selector),
        parameters,
        PresentationContentEmissionFamily::Style,
        CheckedOutputKind::Span,
    )
}

const fn layout_definition(
    selector: RichTextLayoutSelector,
    parameters: &'static [PresentationContentCallableParameterSpec],
) -> PresentationContentCallableDefinition {
    direct_definition(
        PresentationContentCallableDefinitionId::Layout(selector),
        parameters,
        PresentationContentEmissionFamily::Layout,
        CheckedOutputKind::Span,
    )
}

const fn transform_definition(
    selector: RichTextTransformSelector,
    parameters: &'static [PresentationContentCallableParameterSpec],
) -> PresentationContentCallableDefinition {
    direct_definition(
        PresentationContentCallableDefinitionId::Transform(selector),
        parameters,
        PresentationContentEmissionFamily::Transform,
        CheckedOutputKind::Span,
    )
}

const fn ruby_definition() -> PresentationContentCallableDefinition {
    PresentationContentCallableDefinition::new(
        PresentationContentCallableDefinitionId::Ruby,
        RUBY_PARAMETERS,
        PresentationContentAttachedBodyPolicy::InlineOnly,
        PresentationContentEmissionFamily::Ruby,
        CheckedOutputKind::Span,
    )
}

const fn raw_definition() -> PresentationContentCallableDefinition {
    PresentationContentCallableDefinition::new(
        PresentationContentCallableDefinitionId::Raw,
        NO_PARAMETERS,
        PresentationContentAttachedBodyPolicy::LiteralOnly,
        PresentationContentEmissionFamily::Raw,
        CheckedOutputKind::Marker,
    )
}

const CONTENT_CALLABLE_DEFINITIONS: &[PresentationContentCallableDefinition] = &[
    direct_definition(
        PresentationContentCallableDefinitionId::Strong,
        NO_PARAMETERS,
        PresentationContentEmissionFamily::Strong,
        CheckedOutputKind::Span,
    ),
    direct_definition(
        PresentationContentCallableDefinitionId::Em,
        NO_PARAMETERS,
        PresentationContentEmissionFamily::Em,
        CheckedOutputKind::Span,
    ),
    direct_definition(
        PresentationContentCallableDefinitionId::Color,
        COLOR_PARAMETERS,
        PresentationContentEmissionFamily::Color,
        CheckedOutputKind::Span,
    ),
    direct_definition(
        PresentationContentCallableDefinitionId::Font,
        FONT_PARAMETERS,
        PresentationContentEmissionFamily::Font,
        CheckedOutputKind::Span,
    ),
    direct_definition(
        PresentationContentCallableDefinitionId::Size,
        SIZE_PARAMETERS,
        PresentationContentEmissionFamily::Size,
        CheckedOutputKind::Span,
    ),
    style_definition(RichTextStyleSelector::Italic, STYLE_ITALIC_PARAMETERS),
    style_definition(RichTextStyleSelector::Oblique, STYLE_OBLIQUE_PARAMETERS),
    style_definition(RichTextStyleSelector::Opacity, STYLE_OPACITY_PARAMETERS),
    style_definition(RichTextStyleSelector::Layer, STYLE_LAYER_PARAMETERS),
    style_definition(RichTextStyleSelector::ZIndex, STYLE_Z_INDEX_PARAMETERS),
    layout_definition(
        RichTextLayoutSelector::HorizontalTb,
        LAYOUT_HORIZONTAL_TB_PARAMETERS,
    ),
    layout_definition(
        RichTextLayoutSelector::VerticalRl,
        LAYOUT_VERTICAL_RL_PARAMETERS,
    ),
    layout_definition(
        RichTextLayoutSelector::VerticalLr,
        LAYOUT_VERTICAL_LR_PARAMETERS,
    ),
    layout_definition(
        RichTextLayoutSelector::Direction,
        LAYOUT_DIRECTION_PARAMETERS,
    ),
    layout_definition(
        RichTextLayoutSelector::RubyOver,
        LAYOUT_RUBY_OVER_PARAMETERS,
    ),
    layout_definition(
        RichTextLayoutSelector::RubyUnder,
        LAYOUT_RUBY_UNDER_PARAMETERS,
    ),
    layout_definition(
        RichTextLayoutSelector::RubyInterCharacter,
        LAYOUT_RUBY_INTER_CHARACTER_PARAMETERS,
    ),
    transform_definition(
        RichTextTransformSelector::Offset,
        TRANSFORM_OFFSET_PARAMETERS,
    ),
    transform_definition(
        RichTextTransformSelector::Rotate,
        TRANSFORM_ROTATE_PARAMETERS,
    ),
    transform_definition(RichTextTransformSelector::Scale, TRANSFORM_SCALE_PARAMETERS),
    transform_definition(RichTextTransformSelector::Skew, TRANSFORM_SKEW_PARAMETERS),
    direct_definition(
        PresentationContentCallableDefinitionId::Fx,
        FX_PARAMETERS,
        PresentationContentEmissionFamily::Fx,
        CheckedOutputKind::Span,
    ),
    ruby_definition(),
    raw_definition(),
];

fn canonical_row_bytes(row: &PresentationContentCallableDefinition) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.push(row.head.semantic_tag());
    encode_definition_id(&mut bytes, row.id);
    bytes.push(match row.attached_body_policy {
        PresentationContentAttachedBodyPolicy::PreserveBodyRole => 0,
        PresentationContentAttachedBodyPolicy::InlineOnly => 1,
        PresentationContentAttachedBodyPolicy::LiteralOnly => 2,
    });
    bytes.push(match row.emission_family {
        PresentationContentEmissionFamily::Strong => 0,
        PresentationContentEmissionFamily::Em => 1,
        PresentationContentEmissionFamily::Color => 2,
        PresentationContentEmissionFamily::Font => 3,
        PresentationContentEmissionFamily::Size => 4,
        PresentationContentEmissionFamily::Style => 5,
        PresentationContentEmissionFamily::Layout => 6,
        PresentationContentEmissionFamily::Transform => 7,
        PresentationContentEmissionFamily::Fx => 8,
        PresentationContentEmissionFamily::Ruby => 9,
        PresentationContentEmissionFamily::Raw => 10,
    });
    bytes.push(output_discriminant(row.output));
    push_usize(&mut bytes, row.parameters.len());
    for parameter in row.parameters {
        encode_parameter(&mut bytes, parameter);
    }
    bytes
}

fn output_discriminant(output: CheckedOutputKind) -> u8 {
    match output {
        CheckedOutputKind::PointControl => 0,
        CheckedOutputKind::Span => 1,
        CheckedOutputKind::Object => 2,
        CheckedOutputKind::Host => 3,
        CheckedOutputKind::Marker => 4,
    }
}

fn encode_definition_id(bytes: &mut Vec<u8>, id: PresentationContentCallableDefinitionId) {
    match id {
        PresentationContentCallableDefinitionId::Strong => bytes.push(0),
        PresentationContentCallableDefinitionId::Em => bytes.push(1),
        PresentationContentCallableDefinitionId::Color => bytes.push(2),
        PresentationContentCallableDefinitionId::Font => bytes.push(3),
        PresentationContentCallableDefinitionId::Size => bytes.push(4),
        PresentationContentCallableDefinitionId::Style(selector) => {
            bytes.extend_from_slice(&[5, ordinal_byte(selector.ordinal())]);
        }
        PresentationContentCallableDefinitionId::Layout(selector) => {
            bytes.extend_from_slice(&[6, ordinal_byte(selector.ordinal())]);
        }
        PresentationContentCallableDefinitionId::Transform(selector) => {
            bytes.extend_from_slice(&[7, ordinal_byte(selector.ordinal())]);
        }
        PresentationContentCallableDefinitionId::Fx => bytes.push(8),
        PresentationContentCallableDefinitionId::Ruby => bytes.push(9),
        PresentationContentCallableDefinitionId::Raw => bytes.push(10),
    }
}

fn ordinal_byte(ordinal: u16) -> u8 {
    u8::try_from(ordinal).expect("presentation inventory ordinal fits digest discriminant")
}

fn encode_parameter(bytes: &mut Vec<u8>, parameter: &PresentationContentCallableParameterSpec) {
    encode_parameter_id(bytes, parameter.id);
    push_str(bytes, parameter.source_name);
    encode_value_kind(bytes, parameter.kind);
    bytes.push(match parameter.passing {
        RichTextCallableParameterPassing::PositionalOnly => 0,
        RichTextCallableParameterPassing::NamedOnly => 1,
    });
    encode_presence(bytes, parameter.presence);
    encode_optional_default(bytes, parameter.conditional_default);
    encode_enum_constraint(bytes, parameter.enum_constraint);
    encode_limits(bytes, parameter.limits);
    bytes.push(u8::from(parameter.allow_empty));
}

fn encode_optional_default(bytes: &mut Vec<u8>, value: Option<RichTextDefaultValue>) {
    match value {
        Some(value) => {
            bytes.push(1);
            encode_default(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn encode_enum_constraint(bytes: &mut Vec<u8>, constraint: RichTextEnumValueConstraint) {
    match constraint {
        RichTextEnumValueConstraint::All => bytes.push(0),
        RichTextEnumValueConstraint::Exact(variant) => {
            bytes.push(1);
            bytes.extend_from_slice(&variant.to_le_bytes());
        }
        RichTextEnumValueConstraint::Allowed(variants) => {
            bytes.push(2);
            push_usize(bytes, variants.len());
            for variant in variants {
                bytes.extend_from_slice(&variant.to_le_bytes());
            }
        }
    }
}

fn encode_parameter_id(bytes: &mut Vec<u8>, id: PresentationContentCallableParameterId) {
    match id {
        PresentationContentCallableParameterId::Value => bytes.push(0),
        PresentationContentCallableParameterId::Selector => bytes.push(1),
        PresentationContentCallableParameterId::Fx => bytes.push(2),
        PresentationContentCallableParameterId::Style(property) => {
            bytes.extend_from_slice(&[3, style_property_ordinal(property)]);
        }
        PresentationContentCallableParameterId::Layout(property) => {
            bytes.extend_from_slice(&[4, layout_property_ordinal(property)]);
        }
        PresentationContentCallableParameterId::Transform(property) => {
            bytes.extend_from_slice(&[5, transform_property_ordinal(property)]);
        }
    }
}

fn encode_value_kind(bytes: &mut Vec<u8>, kind: RichTextValueKind) {
    match kind {
        RichTextValueKind::Bool => bytes.push(0),
        RichTextValueKind::Int => bytes.push(1),
        RichTextValueKind::FixedMilli => bytes.push(2),
        RichTextValueKind::Ratio => bytes.push(3),
        RichTextValueKind::Length => bytes.push(4),
        RichTextValueKind::Angle => bytes.push(5),
        RichTextValueKind::Duration => bytes.push(6),
        RichTextValueKind::ClosedEnum(domain) => {
            bytes.extend_from_slice(&[7, domain.owner().tag(), domain.local_tag()]);
        }
        RichTextValueKind::Selector(selector) => {
            bytes.extend_from_slice(&[8, selector as u8]);
        }
        RichTextValueKind::PublicId => bytes.push(9),
        RichTextValueKind::Text => bytes.push(10),
        RichTextValueKind::Color => bytes.push(11),
        RichTextValueKind::Vec2 => bytes.push(12),
        RichTextValueKind::Seed32 => bytes.push(13),
        RichTextValueKind::Fx => bytes.push(14),
    }
}

fn encode_presence(
    bytes: &mut Vec<u8>,
    presence: RichTextCallableParameterPresence<PresentationContentCallableParameterId>,
) {
    match presence {
        RichTextCallableParameterPresence::Required => bytes.push(0),
        RichTextCallableParameterPresence::Optional => bytes.push(1),
        RichTextCallableParameterPresence::Defaulted(default) => {
            bytes.push(2);
            encode_default(bytes, default);
        }
        RichTextCallableParameterPresence::Conditional { predicate } => {
            bytes.push(3);
            encode_predicate(bytes, predicate);
        }
    }
}

fn encode_predicate(
    bytes: &mut Vec<u8>,
    predicate: RichTextPropertyPredicate<PresentationContentCallableParameterId>,
) {
    match predicate {
        RichTextPropertyPredicate::Present(property) => {
            bytes.push(0);
            encode_parameter_id(bytes, property);
        }
        RichTextPropertyPredicate::BoolEquals { property, value } => {
            bytes.push(1);
            encode_parameter_id(bytes, property);
            bytes.push(u8::from(value));
        }
        RichTextPropertyPredicate::EnumEquals { property, variant } => {
            bytes.push(2);
            encode_parameter_id(bytes, property);
            bytes.extend_from_slice(&variant.to_le_bytes());
        }
    }
}

fn encode_default(bytes: &mut Vec<u8>, value: RichTextDefaultValue) {
    match value {
        RichTextDefaultValue::Bool(value) => bytes.extend_from_slice(&[0, u8::from(value)]),
        RichTextDefaultValue::Int(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        RichTextDefaultValue::Milli(value) => {
            bytes.push(2);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        RichTextDefaultValue::RatioMilli(value) => {
            bytes.push(3);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        RichTextDefaultValue::Length { milli, unit } => {
            bytes.push(4);
            bytes.extend_from_slice(&milli.to_le_bytes());
            bytes.push(unit_ordinal(unit));
        }
        RichTextDefaultValue::AngleMilliDegrees(value) => {
            bytes.push(5);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        RichTextDefaultValue::DurationMillis(value) => {
            bytes.push(6);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        RichTextDefaultValue::EnumVariant(value) => {
            bytes.push(7);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        RichTextDefaultValue::PublicId(value) => {
            bytes.push(8);
            push_str(bytes, value);
        }
        RichTextDefaultValue::Text(value) => {
            bytes.push(9);
            push_str(bytes, value);
        }
        RichTextDefaultValue::ColorRgba8(value) => {
            bytes.push(10);
            bytes.extend_from_slice(&value);
        }
        RichTextDefaultValue::Vec2Milli(value) => {
            bytes.push(11);
            for component in value {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
        }
        RichTextDefaultValue::Seed32(value) => {
            bytes.push(12);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn encode_limits(bytes: &mut Vec<u8>, limits: RichTextValueLimits) {
    match limits.numeric {
        Some(numeric) => {
            bytes.push(1);
            encode_option_i64(bytes, numeric.inclusive_min_milli);
            encode_option_i64(bytes, numeric.inclusive_max_milli);
            bytes.push(numeric.max_integer_digits);
            bytes.push(numeric.max_fraction_digits);
        }
        None => bytes.push(0),
    }
    push_usize(bytes, limits.units.len());
    for unit in limits.units {
        bytes.push(unit_ordinal(*unit));
    }
    push_usize(bytes, limits.enum_values.len());
    for value in limits.enum_values {
        push_str(bytes, value);
    }
    bytes.extend_from_slice(&limits.max_encoded_bytes.to_le_bytes());
    bytes.extend_from_slice(&limits.max_decoded_bytes.to_le_bytes());
}

fn encode_option_i64(bytes: &mut Vec<u8>, value: Option<i64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn push_str(bytes: &mut Vec<u8>, value: &str) {
    push_usize(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn push_usize(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(&(value as u64).to_le_bytes());
}

fn unit_ordinal(unit: RichTextUnit) -> u8 {
    match unit {
        RichTextUnit::Unitless => 0,
        RichTextUnit::Px => 1,
        RichTextUnit::Pt => 2,
        RichTextUnit::Ch => 3,
        RichTextUnit::Em => 4,
        RichTextUnit::Deg => 5,
        RichTextUnit::Ms => 6,
        RichTextUnit::S => 7,
        RichTextUnit::Cps => 8,
    }
}

fn style_property_ordinal(property: RichTextStyleProperty) -> u8 {
    match property {
        RichTextStyleProperty::Angle => 0,
        RichTextStyleProperty::Opacity => 1,
        RichTextStyleProperty::Layer => 2,
        RichTextStyleProperty::ZIndex => 3,
    }
}

fn layout_property_ordinal(property: RichTextLayoutProperty) -> u8 {
    match property {
        RichTextLayoutProperty::Direction => 0,
        RichTextLayoutProperty::Latin => 1,
        RichTextLayoutProperty::Jlreq => 2,
        RichTextLayoutProperty::ColumnGap => 3,
        RichTextLayoutProperty::RubySize => 4,
        RichTextLayoutProperty::RubyGap => 5,
        RichTextLayoutProperty::RubyOverhang => 6,
        RichTextLayoutProperty::RubyCollisionGap => 7,
    }
}

fn transform_property_ordinal(property: RichTextTransformProperty) -> u8 {
    match property {
        RichTextTransformProperty::X => 0,
        RichTextTransformProperty::Y => 1,
        RichTextTransformProperty::Angle => 2,
        RichTextTransformProperty::Target => 3,
        RichTextTransformProperty::Origin => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_exposes_typed_fx_adapter() {
        assert_eq!(PRESENTATION_CONTENT_CALLABLE_CATALOG.len(), 24);
        let fx = PRESENTATION_CONTENT_CALLABLE_CATALOG
            .get(PresentationContentCallableDefinitionId::Fx)
            .expect("Fx adapter row");
        assert_eq!(fx.emission_family(), PresentationContentEmissionFamily::Fx);
        assert_eq!(fx.output(), CheckedOutputKind::Span);
        assert_eq!(fx.parameters().len(), 1);
        assert_eq!(
            fx.parameters()[0].id,
            PresentationContentCallableParameterId::Fx
        );
        assert_eq!(fx.parameters()[0].kind, RichTextValueKind::Fx);
    }

    #[test]
    fn catalog_resolves_every_canonical_head_without_aliases() {
        for definition in PRESENTATION_CONTENT_CALLABLE_CATALOG.iter() {
            let head = PRESENTATION_CONTENT_CALLABLE_CATALOG
                .resolve_implicit_head(definition.head().source_name())
                .expect("every catalog row has a canonical implicit head");
            assert_eq!(definition.head(), head);
            assert!(
                PRESENTATION_CONTENT_CALLABLE_CATALOG
                    .rows_for_head(head)
                    .all(|row| row.head() == head)
            );
        }
        assert_eq!(
            PRESENTATION_CONTENT_CALLABLE_CATALOG.resolve_implicit_head("unknown"),
            None
        );
    }
}
