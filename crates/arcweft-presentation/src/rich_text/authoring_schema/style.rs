use arcweft_rich_text_schema::{
    CheckedOutputKind, Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextNumericLimits,
    RichTextPropertySpec, RichTextSourceForm, RichTextTagSchema, RichTextUnit, RichTextValueKind,
    RichTextValueLimits, SelectorContract, SelectorKind, UnknownPropertyPolicy,
};

/// Closed presentation-style selector inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextStyleSelector {
    /// Italic presentation.
    Italic,
    /// Oblique presentation.
    Oblique,
    /// Opacity contribution.
    Opacity,
    /// Semantic presentation layer.
    Layer,
    /// Signed presentation ordering value.
    ZIndex,
}

/// Semantic properties used by presentation-style selector schemas.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextStyleProperty {
    /// Oblique angle.
    Angle,
    /// Opacity ratio.
    Opacity,
    /// Presentation layer identity.
    Layer,
    /// Signed presentation ordering value.
    ZIndex,
}

impl RichTextStyleSelector {
    /// Deterministic complete style-selector inventory.
    pub const ALL: [Self; 5] = [
        Self::Italic,
        Self::Oblique,
        Self::Opacity,
        Self::Layer,
        Self::ZIndex,
    ];

    /// Resolves a current grammar-owned selector spelling without its dot.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"italic" | b"i" => Some(Self::Italic),
            b"oblique" | b"slant" => Some(Self::Oblique),
            b"opacity" | b"alpha" => Some(Self::Opacity),
            b"layer" | b"object_layer" => Some(Self::Layer),
            b"z_index" | b"z" => Some(Self::ZIndex),
            _ => None,
        }
    }

    /// Canonical selector spelling without its dot.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Italic => "italic",
            Self::Oblique => "oblique",
            Self::Opacity => "opacity",
            Self::Layer => "layer",
            Self::ZIndex => "z_index",
        }
    }

    /// Immutable owner-typed schema for this style selector.
    #[must_use]
    pub const fn schema(self) -> &'static RichTextTagSchema<RichTextStyleProperty> {
        match self {
            Self::Italic => &ITALIC_SCHEMA,
            Self::Oblique => &OBLIQUE_SCHEMA,
            Self::Opacity => &OPACITY_SCHEMA,
            Self::Layer => &LAYER_SCHEMA,
            Self::ZIndex => &Z_INDEX_SCHEMA,
        }
    }
}

impl RichTextStyleProperty {
    /// Deterministic complete style-property inventory.
    pub const ALL: [Self; 4] = [Self::Angle, Self::Opacity, Self::Layer, Self::ZIndex];

    /// Canonical source key.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Angle => "angle",
            Self::Opacity => "opacity",
            Self::Layer => "layer",
            Self::ZIndex => "z_index",
        }
    }

    /// Resolves a canonical source key without aliases or normalization.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"angle" => Some(Self::Angle),
            b"opacity" => Some(Self::Opacity),
            b"layer" => Some(Self::Layer),
            b"z_index" => Some(Self::ZIndex),
            _ => None,
        }
    }
}

const SINGLE: Multiplicity = Multiplicity::Single;
const NO_PROPERTIES: &[RichTextPropertySpec<RichTextStyleProperty>] = &[];
const ANGLE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(-89_999),
        inclusive_max_milli: Some(89_999),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Deg],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const RATIO_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(0),
        inclusive_max_milli: Some(1_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Unitless],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const PUBLIC_ID_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &[],
    max_encoded_bytes: 4_096,
    max_decoded_bytes: 4_096,
};
const Z_INDEX_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(i16::MIN as i64),
        inclusive_max_milli: Some(i16::MAX as i64),
        max_integer_digits: 19,
        max_fraction_digits: 0,
    }),
    units: &[RichTextUnit::Unitless],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};

const ANGLE: RichTextPropertySpec<RichTextStyleProperty> = RichTextPropertySpec {
    id: RichTextStyleProperty::Angle,
    source_name: "angle",
    kind: RichTextValueKind::Angle,
    presence: PropertyPresence::Defaulted(RichTextDefaultValue::AngleMilliDegrees(0)),
    multiplicity: SINGLE,
    limits: ANGLE_LIMITS,
    allow_empty: false,
};
const OPACITY: RichTextPropertySpec<RichTextStyleProperty> = RichTextPropertySpec {
    id: RichTextStyleProperty::Opacity,
    source_name: "opacity",
    kind: RichTextValueKind::Ratio,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: RATIO_LIMITS,
    allow_empty: false,
};
const LAYER: RichTextPropertySpec<RichTextStyleProperty> = RichTextPropertySpec {
    id: RichTextStyleProperty::Layer,
    source_name: "layer",
    kind: RichTextValueKind::PublicId,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: PUBLIC_ID_LIMITS,
    allow_empty: false,
};
const Z_INDEX: RichTextPropertySpec<RichTextStyleProperty> = RichTextPropertySpec {
    id: RichTextStyleProperty::ZIndex,
    source_name: "z_index",
    kind: RichTextValueKind::Int,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: Z_INDEX_LIMITS,
    allow_empty: false,
};

const fn selector_schema(
    source_forms: &'static [RichTextSourceForm],
    properties: &'static [RichTextPropertySpec<RichTextStyleProperty>],
) -> RichTextTagSchema<RichTextStyleProperty> {
    RichTextTagSchema {
        source_forms,
        selector: SelectorContract::RequiredPositional {
            kind: SelectorKind::Closed,
        },
        properties,
        unknown_policy: UnknownPropertyPolicy::Reject,
        output: CheckedOutputKind::Span,
    }
}

const ITALIC_SCHEMA: RichTextTagSchema<RichTextStyleProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
        RichTextSourceForm::GrammarSpelling {
            source: ".i",
            canonical: ".italic",
        },
    ],
    NO_PROPERTIES,
);
const OBLIQUE_SCHEMA: RichTextTagSchema<RichTextStyleProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
        RichTextSourceForm::GrammarSpelling {
            source: ".slant",
            canonical: ".oblique",
        },
    ],
    &[ANGLE],
);
const OPACITY_SCHEMA: RichTextTagSchema<RichTextStyleProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
        RichTextSourceForm::GrammarSpelling {
            source: ".alpha",
            canonical: ".opacity",
        },
    ],
    &[OPACITY],
);
const LAYER_SCHEMA: RichTextTagSchema<RichTextStyleProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
        RichTextSourceForm::GrammarSpelling {
            source: ".object_layer",
            canonical: ".layer",
        },
    ],
    &[LAYER],
);
const Z_INDEX_SCHEMA: RichTextTagSchema<RichTextStyleProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
        RichTextSourceForm::GrammarSpelling {
            source: ".z",
            canonical: ".z_index",
        },
    ],
    &[Z_INDEX],
);
