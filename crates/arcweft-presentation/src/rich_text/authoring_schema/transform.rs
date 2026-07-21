use arcweft_rich_text_schema::{
    CheckedOutputKind, Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextEnumSchemaId,
    RichTextNumericLimits, RichTextPropertySpec, RichTextSourceForm, RichTextTagSchema,
    RichTextUnit, RichTextValueKind, RichTextValueLimits, SelectorContract, SelectorKind,
    UnknownPropertyPolicy,
};

/// Closed post-layout transform selector inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextTransformSelector {
    /// Translation transform.
    Offset,
    /// Rotation transform.
    Rotate,
    /// Scale transform.
    Scale,
    /// Skew transform.
    Skew,
}

/// Semantic properties used by post-layout transform schemas.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextTransformProperty {
    /// Horizontal component.
    X,
    /// Vertical component.
    Y,
    /// Rotation angle.
    Angle,
    /// Application target.
    Target,
    /// Transform origin.
    Origin,
}

impl RichTextTransformSelector {
    /// Deterministic complete transform-selector inventory.
    pub const ALL: [Self; 4] = [Self::Offset, Self::Rotate, Self::Scale, Self::Skew];

    /// Resolves a current grammar-owned selector spelling without its dot.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"offset" | b"pos" => Some(Self::Offset),
            b"rotate" => Some(Self::Rotate),
            b"scale" => Some(Self::Scale),
            b"skew" => Some(Self::Skew),
            _ => None,
        }
    }

    /// Canonical selector spelling without its dot.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Offset => "offset",
            Self::Rotate => "rotate",
            Self::Scale => "scale",
            Self::Skew => "skew",
        }
    }

    /// Immutable owner-typed schema for this transform selector.
    #[must_use]
    pub const fn schema(self) -> &'static RichTextTagSchema<RichTextTransformProperty> {
        match self {
            Self::Offset => &OFFSET_SCHEMA,
            Self::Rotate => &ROTATE_SCHEMA,
            Self::Scale => &SCALE_SCHEMA,
            Self::Skew => &SKEW_SCHEMA,
        }
    }
}

impl RichTextTransformProperty {
    /// Deterministic complete transform-property inventory.
    pub const ALL: [Self; 5] = [Self::X, Self::Y, Self::Angle, Self::Target, Self::Origin];

    /// Canonical source key.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Angle => "angle",
            Self::Target => "target",
            Self::Origin => "origin",
        }
    }

    /// Resolves a canonical source key without aliases or normalization.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"x" => Some(Self::X),
            b"y" => Some(Self::Y),
            b"angle" => Some(Self::Angle),
            b"target" => Some(Self::Target),
            b"origin" => Some(Self::Origin),
            _ => None,
        }
    }
}

const TARGET_ENUM: RichTextEnumSchemaId = RichTextEnumSchemaId::new("rich_text.transform.target");
const ORIGIN_ENUM: RichTextEnumSchemaId = RichTextEnumSchemaId::new("rich_text.transform.origin");
const SINGLE: Multiplicity = Multiplicity::Single;

const OFFSET_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(-1_000_000_000),
        inclusive_max_milli: Some(1_000_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Px],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const ROTATE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(-360_000_000),
        inclusive_max_milli: Some(360_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Deg],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const SCALE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(-1_000_000),
        inclusive_max_milli: Some(1_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Unitless],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const SKEW_LIMITS: RichTextValueLimits = RichTextValueLimits {
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
const TARGET_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &["node", "content", "background", "line", "glyph", "viewport"],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const ORIGIN_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &[
        "baseline_start",
        "baseline_center",
        "center",
        "glyph_center",
    ],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};

const fn scalar_property(
    id: RichTextTransformProperty,
    source_name: &'static str,
    kind: RichTextValueKind,
    limits: RichTextValueLimits,
    default: RichTextDefaultValue,
) -> RichTextPropertySpec<RichTextTransformProperty> {
    RichTextPropertySpec {
        id,
        source_name,
        kind,
        presence: PropertyPresence::Defaulted(default),
        multiplicity: SINGLE,
        limits,
        allow_empty: false,
    }
}

const fn target_property() -> RichTextPropertySpec<RichTextTransformProperty> {
    scalar_property(
        RichTextTransformProperty::Target,
        "target",
        RichTextValueKind::ClosedEnum(TARGET_ENUM),
        TARGET_LIMITS,
        RichTextDefaultValue::EnumVariant(1),
    )
}

const fn origin_property(default_variant: u16) -> RichTextPropertySpec<RichTextTransformProperty> {
    scalar_property(
        RichTextTransformProperty::Origin,
        "origin",
        RichTextValueKind::ClosedEnum(ORIGIN_ENUM),
        ORIGIN_LIMITS,
        RichTextDefaultValue::EnumVariant(default_variant),
    )
}

const OFFSET_PROPERTIES: [RichTextPropertySpec<RichTextTransformProperty>; 4] = [
    scalar_property(
        RichTextTransformProperty::X,
        "x",
        RichTextValueKind::Length,
        OFFSET_LIMITS,
        RichTextDefaultValue::Length {
            milli: 0,
            unit: RichTextUnit::Px,
        },
    ),
    scalar_property(
        RichTextTransformProperty::Y,
        "y",
        RichTextValueKind::Length,
        OFFSET_LIMITS,
        RichTextDefaultValue::Length {
            milli: 0,
            unit: RichTextUnit::Px,
        },
    ),
    target_property(),
    origin_property(0),
];
const ROTATE_PROPERTIES: [RichTextPropertySpec<RichTextTransformProperty>; 3] = [
    scalar_property(
        RichTextTransformProperty::Angle,
        "angle",
        RichTextValueKind::Angle,
        ROTATE_LIMITS,
        RichTextDefaultValue::AngleMilliDegrees(0),
    ),
    target_property(),
    origin_property(2),
];
const SCALE_PROPERTIES: [RichTextPropertySpec<RichTextTransformProperty>; 4] = [
    scalar_property(
        RichTextTransformProperty::X,
        "x",
        RichTextValueKind::FixedMilli,
        SCALE_LIMITS,
        RichTextDefaultValue::Milli(1_000),
    ),
    scalar_property(
        RichTextTransformProperty::Y,
        "y",
        RichTextValueKind::FixedMilli,
        SCALE_LIMITS,
        RichTextDefaultValue::Milli(1_000),
    ),
    target_property(),
    origin_property(2),
];
const SKEW_PROPERTIES: [RichTextPropertySpec<RichTextTransformProperty>; 4] = [
    scalar_property(
        RichTextTransformProperty::X,
        "x",
        RichTextValueKind::Angle,
        SKEW_LIMITS,
        RichTextDefaultValue::AngleMilliDegrees(0),
    ),
    scalar_property(
        RichTextTransformProperty::Y,
        "y",
        RichTextValueKind::Angle,
        SKEW_LIMITS,
        RichTextDefaultValue::AngleMilliDegrees(0),
    ),
    target_property(),
    origin_property(0),
];

const fn selector_schema(
    source_forms: &'static [RichTextSourceForm],
    properties: &'static [RichTextPropertySpec<RichTextTransformProperty>],
) -> RichTextTagSchema<RichTextTransformProperty> {
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

const OFFSET_SCHEMA: RichTextTagSchema<RichTextTransformProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
        RichTextSourceForm::GrammarSpelling {
            source: ".pos",
            canonical: ".offset",
        },
    ],
    &OFFSET_PROPERTIES,
);
const ROTATE_SCHEMA: RichTextTagSchema<RichTextTransformProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &ROTATE_PROPERTIES,
);
const SCALE_SCHEMA: RichTextTagSchema<RichTextTransformProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &SCALE_PROPERTIES,
);
const SKEW_SCHEMA: RichTextTagSchema<RichTextTransformProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &SKEW_PROPERTIES,
);
