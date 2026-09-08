use arcweft_rich_text_schema::{
    CheckedOutputKind, Multiplicity, PropertyPresence, RichTextNumericLimits,
    RichTextPointActionSchema, RichTextPointActionSource, RichTextPropertySpec, RichTextUnit,
    RichTextValueKind, RichTextValueLimits, SelectorContract, SelectorKind, UnknownPropertyPolicy,
};

/// Closed dialogue-owned point and reveal-control inventory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DialogueRichTextControl {
    /// Page boundary and advance wait.
    Page,
    /// Line-local advance wait.
    LineWait,
    /// Hard display-line break.
    HardBreak,
    /// Time-bounded wait.
    TimedWait,
    /// Clears the active dialogue display.
    Clear,
    /// Resets active rich-text presentation.
    Reset,
    /// Changes the reveal rate.
    RevealRate,
    /// Emits an explicit zero-width line marker.
    Marker,
}

/// Semantic properties used by dialogue control schemas.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DialogueControlProperty {
    /// Timed-wait duration.
    Time,
    /// Reveal characters per second.
    Cps,
}

impl DialogueRichTextControl {
    /// Deterministic complete control inventory.
    pub const ALL: [Self; 8] = [
        Self::Page,
        Self::LineWait,
        Self::HardBreak,
        Self::TimedWait,
        Self::Clear,
        Self::Reset,
        Self::RevealRate,
        Self::Marker,
    ];

    /// Resolves a current grammar-owned source spelling.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"p" => Some(Self::Page),
            b"l" => Some(Self::LineWait),
            b"r" => Some(Self::HardBreak),
            b"w" => Some(Self::TimedWait),
            b"clear" => Some(Self::Clear),
            b"reset" => Some(Self::Reset),
            b"speed" => Some(Self::RevealRate),
            b"mark" => Some(Self::Marker),
            _ => None,
        }
    }

    /// Canonical formatter spelling for this control.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Page => "p",
            Self::LineWait => "l",
            Self::HardBreak => "r",
            Self::TimedWait => "w",
            Self::Clear => "clear",
            Self::Reset => "reset",
            Self::RevealRate => "speed",
            Self::Marker => "mark",
        }
    }

    /// Immutable owner-typed schema for this control.
    #[must_use]
    pub const fn schema(self) -> &'static RichTextPointActionSchema<DialogueControlProperty> {
        match self {
            Self::Page => &PAGE_SCHEMA,
            Self::LineWait => &LINE_WAIT_SCHEMA,
            Self::HardBreak => &HARD_BREAK_SCHEMA,
            Self::TimedWait => &TIMED_WAIT_SCHEMA,
            Self::Clear => &CLEAR_SCHEMA,
            Self::Reset => &RESET_SCHEMA,
            Self::RevealRate => &REVEAL_RATE_SCHEMA,
            Self::Marker => &MARKER_SCHEMA,
        }
    }
}

impl DialogueControlProperty {
    /// Deterministic complete property inventory.
    pub const ALL: [Self; 2] = [Self::Time, Self::Cps];

    /// Canonical source key.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Cps => "cps",
        }
    }

    /// Resolves a canonical source key without aliases or normalization.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"time" => Some(Self::Time),
            b"cps" => Some(Self::Cps),
            _ => None,
        }
    }
}

const NO_PROPERTIES: &[RichTextPropertySpec<DialogueControlProperty>] = &[];
const SINGLE: Multiplicity = Multiplicity::Single;
const DURATION_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(1_000),
        inclusive_max_milli: Some(86_400_000_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Ms, RichTextUnit::S],
    enum_values: &[],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};
const REVEAL_RATE_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: Some(RichTextNumericLimits {
        inclusive_min_milli: Some(1_000),
        inclusive_max_milli: Some(240_000),
        max_integer_digits: 19,
        max_fraction_digits: 3,
    }),
    units: &[RichTextUnit::Cps],
    enum_values: &["slow", "normal", "fast"],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};

const TIME_PROPERTY: RichTextPropertySpec<DialogueControlProperty> = RichTextPropertySpec {
    id: DialogueControlProperty::Time,
    source_name: "time",
    kind: RichTextValueKind::Duration,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: DURATION_LIMITS,
    allow_empty: false,
};
const CPS_PROPERTY: RichTextPropertySpec<DialogueControlProperty> = RichTextPropertySpec {
    id: DialogueControlProperty::Cps,
    source_name: "cps",
    kind: RichTextValueKind::FixedMilli,
    presence: PropertyPresence::Required,
    multiplicity: SINGLE,
    limits: REVEAL_RATE_LIMITS,
    allow_empty: false,
};

const PAGE_SOURCE: RichTextPointActionSource = RichTextPointActionSource::canonical("p");
const LINE_WAIT_SOURCE: RichTextPointActionSource = RichTextPointActionSource::canonical("l");
const HARD_BREAK_SOURCE: RichTextPointActionSource = RichTextPointActionSource::canonical("r");
const CLEAR_SOURCE: RichTextPointActionSource = RichTextPointActionSource::canonical("clear");

const fn point_schema(
    source: RichTextPointActionSource,
) -> RichTextPointActionSchema<DialogueControlProperty> {
    RichTextPointActionSchema {
        source,
        selector: SelectorContract::None,
        properties: NO_PROPERTIES,
        unknown_policy: UnknownPropertyPolicy::Reject,
        output: CheckedOutputKind::PointControl,
    }
}

const PAGE_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> = point_schema(PAGE_SOURCE);
const LINE_WAIT_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> =
    point_schema(LINE_WAIT_SOURCE);
const HARD_BREAK_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> =
    point_schema(HARD_BREAK_SOURCE);
const CLEAR_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> = point_schema(CLEAR_SOURCE);
const RESET_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> =
    point_schema(RichTextPointActionSource::canonical("reset"));

const TIMED_WAIT_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> =
    RichTextPointActionSchema {
        source: RichTextPointActionSource::canonical("w"),
        selector: SelectorContract::None,
        properties: &[TIME_PROPERTY],
        unknown_policy: UnknownPropertyPolicy::Reject,
        output: CheckedOutputKind::PointControl,
    };
const REVEAL_RATE_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> =
    RichTextPointActionSchema {
        source: RichTextPointActionSource::canonical("speed"),
        selector: SelectorContract::None,
        properties: &[CPS_PROPERTY],
        unknown_policy: UnknownPropertyPolicy::Reject,
        output: CheckedOutputKind::PointControl,
    };
const MARKER_SCHEMA: RichTextPointActionSchema<DialogueControlProperty> =
    RichTextPointActionSchema {
        source: RichTextPointActionSource::canonical("mark"),
        selector: SelectorContract::RequiredPositional {
            kind: SelectorKind::PublicId,
        },
        properties: NO_PROPERTIES,
        unknown_policy: UnknownPropertyPolicy::Reject,
        output: CheckedOutputKind::Marker,
    };
