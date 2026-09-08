use arcweft_rich_text_schema::{
    Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextNumericLimits,
    RichTextPropertySetSchema, RichTextPropertySpec, RichTextUnit, RichTextValueKind,
    RichTextValueLimits,
};

/// Closed presentation-owned direct span inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextDirectStyle {
    /// Emphasis span.
    Emphasis,
    /// Strong-emphasis span.
    Strong,
    /// Italic span.
    Italic,
    /// Oblique span with an optional angle.
    Oblique,
    /// Foreground-color span.
    Color,
    /// Font-family span.
    Font,
    /// Font-size span.
    Size,
    /// Ruby annotation span.
    Ruby,
}

/// Semantic properties used by direct `RichText` span schemas.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextDirectStyleProperty {
    /// Oblique angle.
    Angle,
    /// Family-specific scalar value.
    Value,
    /// Ruby annotation text, authored as `rt`.
    RubyText,
}

impl RichTextDirectStyle {
    /// Deterministic complete direct-style inventory.
    pub const ALL: [Self; 8] = [
        Self::Emphasis,
        Self::Strong,
        Self::Italic,
        Self::Oblique,
        Self::Color,
        Self::Font,
        Self::Size,
        Self::Ruby,
    ];

    /// Resolves a current grammar-owned source spelling.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"em" => Some(Self::Emphasis),
            b"strong" => Some(Self::Strong),
            b"italic" => Some(Self::Italic),
            b"oblique" => Some(Self::Oblique),
            b"color" => Some(Self::Color),
            b"font" => Some(Self::Font),
            b"size" => Some(Self::Size),
            b"ruby" => Some(Self::Ruby),
            _ => None,
        }
    }

    /// Canonical formatter spelling for this direct style.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Emphasis => "em",
            Self::Strong => "strong",
            Self::Italic => "italic",
            Self::Oblique => "oblique",
            Self::Color => "color",
            Self::Font => "font",
            Self::Size => "size",
            Self::Ruby => "ruby",
        }
    }

    /// Immutable owner-typed schema for this direct style.
    #[must_use]
    pub const fn property_schema(
        self,
    ) -> &'static RichTextPropertySetSchema<RichTextDirectStyleProperty> {
        match self {
            Self::Emphasis => &EMPHASIS_PROPERTY_SET,
            Self::Strong => &STRONG_PROPERTY_SET,
            Self::Italic => &ITALIC_PROPERTY_SET,
            Self::Oblique => &OBLIQUE_PROPERTY_SET,
            Self::Color => &COLOR_PROPERTY_SET,
            Self::Font => &FONT_PROPERTY_SET,
            Self::Size => &SIZE_PROPERTY_SET,
            Self::Ruby => &RUBY_PROPERTY_SET,
        }
    }
}

impl RichTextDirectStyleProperty {
    /// Deterministic complete direct-style property inventory.
    pub const ALL: [Self; 3] = [Self::Angle, Self::Value, Self::RubyText];

    /// Canonical source key.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Angle => "angle",
            Self::Value => "value",
            Self::RubyText => "rt",
        }
    }

    /// Resolves a canonical source key without aliases or normalization.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"angle" => Some(Self::Angle),
            b"value" => Some(Self::Value),
            b"rt" => Some(Self::RubyText),
            _ => None,
        }
    }
}

const SINGLE: Multiplicity = Multiplicity::Single;
const NO_PROPERTIES: &[RichTextPropertySpec<RichTextDirectStyleProperty>] = &[];
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
const COLOR_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &[],
    max_encoded_bytes: 4_096,
    max_decoded_bytes: 4_096,
};
const FONT_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &["serif", "sans-serif", "monospace", "cursive", "fantasy"],
    max_encoded_bytes: 4_096,
    max_decoded_bytes: 256,
};
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
const RUBY_TEXT_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &[],
    max_encoded_bytes: 4_096,
    max_decoded_bytes: 4_096,
};

const ANGLE: RichTextPropertySpec<RichTextDirectStyleProperty> = RichTextPropertySpec {
    id: RichTextDirectStyleProperty::Angle,
    source_name: "angle",
    kind: RichTextValueKind::Angle,
    presence: PropertyPresence::Defaulted(RichTextDefaultValue::AngleMilliDegrees(0)),
    multiplicity: SINGLE,
    limits: ANGLE_LIMITS,
    allow_empty: false,
};
const COLOR_VALUE: RichTextPropertySpec<RichTextDirectStyleProperty> = RichTextPropertySpec {
    id: RichTextDirectStyleProperty::Value,
    source_name: "value",
    kind: RichTextValueKind::Color,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: COLOR_LIMITS,
    allow_empty: false,
};
const FONT_VALUE: RichTextPropertySpec<RichTextDirectStyleProperty> = RichTextPropertySpec {
    id: RichTextDirectStyleProperty::Value,
    source_name: "value",
    kind: RichTextValueKind::Text,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: FONT_LIMITS,
    allow_empty: false,
};
const SIZE_VALUE: RichTextPropertySpec<RichTextDirectStyleProperty> = RichTextPropertySpec {
    id: RichTextDirectStyleProperty::Value,
    source_name: "value",
    kind: RichTextValueKind::Length,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: SIZE_LIMITS,
    allow_empty: false,
};
const RUBY_TEXT: RichTextPropertySpec<RichTextDirectStyleProperty> = RichTextPropertySpec {
    id: RichTextDirectStyleProperty::RubyText,
    source_name: "rt",
    kind: RichTextValueKind::Text,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: RUBY_TEXT_LIMITS,
    allow_empty: false,
};

const fn property_set(
    properties: &'static [RichTextPropertySpec<RichTextDirectStyleProperty>],
) -> RichTextPropertySetSchema<RichTextDirectStyleProperty> {
    RichTextPropertySetSchema { properties }
}

const EMPHASIS_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(NO_PROPERTIES);
const STRONG_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(NO_PROPERTIES);
const ITALIC_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(NO_PROPERTIES);
const OBLIQUE_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(&[ANGLE]);
const COLOR_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(&[COLOR_VALUE]);
const FONT_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(&[FONT_VALUE]);
const SIZE_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(&[SIZE_VALUE]);
const RUBY_PROPERTY_SET: RichTextPropertySetSchema<RichTextDirectStyleProperty> =
    property_set(&[RUBY_TEXT]);
