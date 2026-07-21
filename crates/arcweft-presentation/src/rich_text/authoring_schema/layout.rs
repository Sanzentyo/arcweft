use arcweft_rich_text_schema::{
    CheckedOutputKind, Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextEnumSchemaId,
    RichTextNumericLimits, RichTextPropertySpec, RichTextSourceForm, RichTextTagSchema,
    RichTextUnit, RichTextValueKind, RichTextValueLimits, SelectorContract, SelectorKind,
    UnknownPropertyPolicy,
};

/// Closed inline-layout selector inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextLayoutSelector {
    /// Horizontal top-to-bottom writing mode.
    HorizontalTb,
    /// Vertical right-to-left writing mode.
    VerticalRl,
    /// Vertical left-to-right writing mode.
    VerticalLr,
    /// Explicit inline direction.
    Direction,
    /// Ruby above its base text.
    RubyOver,
    /// Ruby below its base text.
    RubyUnder,
    /// Inter-character ruby.
    RubyInterCharacter,
}

/// Semantic properties used by inline-layout schemas.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextLayoutProperty {
    /// Inline direction.
    Direction,
    /// Vertical Latin orientation.
    Latin,
    /// JLREQ strictness.
    Jlreq,
    /// Column gap.
    ColumnGap,
    /// Ruby font size.
    RubySize,
    /// Ruby gap.
    RubyGap,
    /// Ruby overhang.
    RubyOverhang,
    /// Ruby collision gap.
    RubyCollisionGap,
}

impl RichTextLayoutSelector {
    /// Deterministic complete layout-selector inventory.
    pub const ALL: [Self; 7] = [
        Self::HorizontalTb,
        Self::VerticalRl,
        Self::VerticalLr,
        Self::Direction,
        Self::RubyOver,
        Self::RubyUnder,
        Self::RubyInterCharacter,
    ];

    /// Resolves a current grammar-owned selector spelling without its dot.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"horizontal_tb" => Some(Self::HorizontalTb),
            b"vertical_rl" | b"vertical" => Some(Self::VerticalRl),
            b"vertical_lr" => Some(Self::VerticalLr),
            b"dir" => Some(Self::Direction),
            b"ruby_over" => Some(Self::RubyOver),
            b"ruby_under" => Some(Self::RubyUnder),
            b"ruby_inter_character" => Some(Self::RubyInterCharacter),
            _ => None,
        }
    }

    /// Canonical selector spelling without its dot.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::HorizontalTb => "horizontal_tb",
            Self::VerticalRl => "vertical_rl",
            Self::VerticalLr => "vertical_lr",
            Self::Direction => "dir",
            Self::RubyOver => "ruby_over",
            Self::RubyUnder => "ruby_under",
            Self::RubyInterCharacter => "ruby_inter_character",
        }
    }

    /// Immutable owner-typed schema for this layout selector.
    #[must_use]
    pub const fn schema(self) -> &'static RichTextTagSchema<RichTextLayoutProperty> {
        match self {
            Self::HorizontalTb => &HORIZONTAL_TB_SCHEMA,
            Self::VerticalRl => &VERTICAL_RL_SCHEMA,
            Self::VerticalLr => &VERTICAL_LR_SCHEMA,
            Self::Direction => &DIRECTION_SCHEMA,
            Self::RubyOver => &RUBY_OVER_SCHEMA,
            Self::RubyUnder => &RUBY_UNDER_SCHEMA,
            Self::RubyInterCharacter => &RUBY_INTER_CHARACTER_SCHEMA,
        }
    }
}

impl RichTextLayoutProperty {
    /// Deterministic complete layout-property inventory.
    pub const ALL: [Self; 8] = [
        Self::Direction,
        Self::Latin,
        Self::Jlreq,
        Self::ColumnGap,
        Self::RubySize,
        Self::RubyGap,
        Self::RubyOverhang,
        Self::RubyCollisionGap,
    ];

    /// Canonical source key.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Direction => "dir",
            Self::Latin => "latin",
            Self::Jlreq => "jlreq",
            Self::ColumnGap => "column_gap",
            Self::RubySize => "ruby_size",
            Self::RubyGap => "ruby_gap",
            Self::RubyOverhang => "ruby_overhang",
            Self::RubyCollisionGap => "ruby_collision_gap",
        }
    }

    /// Resolves a canonical source key without aliases or normalization.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"dir" => Some(Self::Direction),
            b"latin" => Some(Self::Latin),
            b"jlreq" => Some(Self::Jlreq),
            b"column_gap" => Some(Self::ColumnGap),
            b"ruby_size" => Some(Self::RubySize),
            b"ruby_gap" => Some(Self::RubyGap),
            b"ruby_overhang" => Some(Self::RubyOverhang),
            b"ruby_collision_gap" => Some(Self::RubyCollisionGap),
            _ => None,
        }
    }
}

const DIRECTION_ENUM: RichTextEnumSchemaId =
    RichTextEnumSchemaId::new("rich_text.layout.direction");
const LATIN_ENUM: RichTextEnumSchemaId = RichTextEnumSchemaId::new("rich_text.layout.latin");
const JLREQ_ENUM: RichTextEnumSchemaId = RichTextEnumSchemaId::new("rich_text.layout.jlreq");
const SINGLE: Multiplicity = Multiplicity::Single;

const DIRECTION_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &["auto", "ltr", "rtl"],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const LATIN_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &["mixed", "upright", "sideways"],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const JLREQ_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &["auto", "loose", "normal", "strict"],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const COLUMN_GAP_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(0),
        inclusive_max_milli: Some(4_096_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Px],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const RUBY_SIZE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(1),
        inclusive_max_milli: Some(512_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Px, RichTextUnit::Pt],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};

const fn enum_property(
    id: RichTextLayoutProperty,
    source_name: &'static str,
    enum_id: RichTextEnumSchemaId,
    limits: RichTextValueLimits,
    presence: PropertyPresence<RichTextLayoutProperty>,
) -> RichTextPropertySpec<RichTextLayoutProperty> {
    RichTextPropertySpec {
        id,
        source_name,
        kind: RichTextValueKind::ClosedEnum(enum_id),
        presence,
        multiplicity: SINGLE,
        limits,
        allow_empty: false,
    }
}

const fn length_property(
    id: RichTextLayoutProperty,
    source_name: &'static str,
    limits: RichTextValueLimits,
    presence: PropertyPresence<RichTextLayoutProperty>,
) -> RichTextPropertySpec<RichTextLayoutProperty> {
    RichTextPropertySpec {
        id,
        source_name,
        kind: RichTextValueKind::Length,
        presence,
        multiplicity: SINGLE,
        limits,
        allow_empty: false,
    }
}

const DIRECTION_DEFAULT: RichTextPropertySpec<RichTextLayoutProperty> = enum_property(
    RichTextLayoutProperty::Direction,
    "dir",
    DIRECTION_ENUM,
    DIRECTION_LIMITS,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const DIRECTION_REQUIRED: RichTextPropertySpec<RichTextLayoutProperty> = enum_property(
    RichTextLayoutProperty::Direction,
    "dir",
    DIRECTION_ENUM,
    DIRECTION_LIMITS,
    PropertyPresence::Required,
);
const LATIN: RichTextPropertySpec<RichTextLayoutProperty> = enum_property(
    RichTextLayoutProperty::Latin,
    "latin",
    LATIN_ENUM,
    LATIN_LIMITS,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const JLREQ: RichTextPropertySpec<RichTextLayoutProperty> = enum_property(
    RichTextLayoutProperty::Jlreq,
    "jlreq",
    JLREQ_ENUM,
    JLREQ_LIMITS,
    PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
);
const COLUMN_GAP: RichTextPropertySpec<RichTextLayoutProperty> = length_property(
    RichTextLayoutProperty::ColumnGap,
    "column_gap",
    COLUMN_GAP_LIMITS,
    PropertyPresence::Defaulted(RichTextDefaultValue::Length {
        milli: 8_000,
        unit: RichTextUnit::Px,
    }),
);
const RUBY_SIZE: RichTextPropertySpec<RichTextLayoutProperty> = length_property(
    RichTextLayoutProperty::RubySize,
    "ruby_size",
    RUBY_SIZE_LIMITS,
    PropertyPresence::Optional,
);
const RUBY_GAP: RichTextPropertySpec<RichTextLayoutProperty> = length_property(
    RichTextLayoutProperty::RubyGap,
    "ruby_gap",
    COLUMN_GAP_LIMITS,
    PropertyPresence::Optional,
);
const RUBY_OVERHANG: RichTextPropertySpec<RichTextLayoutProperty> = length_property(
    RichTextLayoutProperty::RubyOverhang,
    "ruby_overhang",
    COLUMN_GAP_LIMITS,
    PropertyPresence::Optional,
);
const RUBY_COLLISION_GAP: RichTextPropertySpec<RichTextLayoutProperty> = length_property(
    RichTextLayoutProperty::RubyCollisionGap,
    "ruby_collision_gap",
    COLUMN_GAP_LIMITS,
    PropertyPresence::Optional,
);

const COMMON_PROPERTIES: [RichTextPropertySpec<RichTextLayoutProperty>; 8] = [
    DIRECTION_DEFAULT,
    LATIN,
    JLREQ,
    COLUMN_GAP,
    RUBY_SIZE,
    RUBY_GAP,
    RUBY_OVERHANG,
    RUBY_COLLISION_GAP,
];
const DIRECTION_PROPERTIES: [RichTextPropertySpec<RichTextLayoutProperty>; 8] = [
    DIRECTION_REQUIRED,
    LATIN,
    JLREQ,
    COLUMN_GAP,
    RUBY_SIZE,
    RUBY_GAP,
    RUBY_OVERHANG,
    RUBY_COLLISION_GAP,
];

const fn selector_schema(
    source_forms: &'static [RichTextSourceForm],
    properties: &'static [RichTextPropertySpec<RichTextLayoutProperty>],
) -> RichTextTagSchema<RichTextLayoutProperty> {
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

const HORIZONTAL_TB_SCHEMA: RichTextTagSchema<RichTextLayoutProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &COMMON_PROPERTIES,
);
const VERTICAL_RL_SCHEMA: RichTextTagSchema<RichTextLayoutProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
        RichTextSourceForm::GrammarSpelling {
            source: ".vertical",
            canonical: ".vertical_rl",
        },
    ],
    &COMMON_PROPERTIES,
);
const VERTICAL_LR_SCHEMA: RichTextTagSchema<RichTextLayoutProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &COMMON_PROPERTIES,
);
const DIRECTION_SCHEMA: RichTextTagSchema<RichTextLayoutProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &DIRECTION_PROPERTIES,
);
const RUBY_OVER_SCHEMA: RichTextTagSchema<RichTextLayoutProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &COMMON_PROPERTIES,
);
const RUBY_UNDER_SCHEMA: RichTextTagSchema<RichTextLayoutProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &COMMON_PROPERTIES,
);
const RUBY_INTER_CHARACTER_SCHEMA: RichTextTagSchema<RichTextLayoutProperty> = selector_schema(
    &[
        RichTextSourceForm::ExplicitFamily,
        RichTextSourceForm::DotSelector,
    ],
    &COMMON_PROPERTIES,
);
