use arcweft_rich_text_schema::{
    Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextNumericLimits,
    RichTextPropertySetSchema, RichTextPropertySpec, RichTextUnit, RichTextValueKind,
    RichTextValueLimits,
};

/// Closed presentation-style selector inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
            b"italic" => Some(Self::Italic),
            b"oblique" => Some(Self::Oblique),
            b"opacity" => Some(Self::Opacity),
            b"layer" => Some(Self::Layer),
            b"z_index" => Some(Self::ZIndex),
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
    pub const fn property_schema(
        self,
    ) -> &'static RichTextPropertySetSchema<RichTextStyleProperty> {
        match self {
            Self::Italic => &ITALIC_PROPERTY_SET,
            Self::Oblique => &OBLIQUE_PROPERTY_SET,
            Self::Opacity => &OPACITY_PROPERTY_SET,
            Self::Layer => &LAYER_PROPERTY_SET,
            Self::ZIndex => &Z_INDEX_PROPERTY_SET,
        }
    }

    /// Closed enum domain carried by selector-valued callable parameters.
    #[must_use]
    pub const fn schema_id(self) -> arcweft_id::closed_enum::ClosedEnumDomainId {
        arcweft_rich_text_schema::RichTextEnumDomain::StyleSelector.domain_id()
    }

    /// Stable zero-based selector ordinal.
    #[must_use]
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Italic => 0,
            Self::Oblique => 1,
            Self::Opacity => 2,
            Self::Layer => 3,
            Self::ZIndex => 4,
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

const fn property_set(
    properties: &'static [RichTextPropertySpec<RichTextStyleProperty>],
) -> RichTextPropertySetSchema<RichTextStyleProperty> {
    RichTextPropertySetSchema { properties }
}

const ITALIC_PROPERTY_SET: RichTextPropertySetSchema<RichTextStyleProperty> =
    property_set(NO_PROPERTIES);
const OBLIQUE_PROPERTY_SET: RichTextPropertySetSchema<RichTextStyleProperty> =
    property_set(&[ANGLE]);
const OPACITY_PROPERTY_SET: RichTextPropertySetSchema<RichTextStyleProperty> =
    property_set(&[OPACITY]);
const LAYER_PROPERTY_SET: RichTextPropertySetSchema<RichTextStyleProperty> = property_set(&[LAYER]);
const Z_INDEX_PROPERTY_SET: RichTextPropertySetSchema<RichTextStyleProperty> =
    property_set(&[Z_INDEX]);
