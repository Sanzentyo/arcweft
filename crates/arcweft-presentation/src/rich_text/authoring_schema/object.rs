use arcweft_rich_text_schema::{
    CheckedOutputKind, Multiplicity, PropertyPresence, RichTextDefaultValue, RichTextNumericLimits,
    RichTextPropertySpec, RichTextSourceForm, RichTextTagSchema, RichTextUnit, RichTextValueKind,
    RichTextValueLimits, SelectorContract, SelectorKind, UnknownPropertyPolicy,
};

/// Closed inline-object selector family.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextObjectSelector {
    /// Explicit or inferred typed text object.
    Object,
}

/// Canonical metadata properties shared by inline text objects.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextObjectProperty {
    /// Visible text-proxy schema identity.
    Type,
    /// Semantic object role.
    Role,
    /// Semantic presentation layer.
    Layer,
    /// Presentation depth.
    Depth,
    /// Whether the object contributes a hit region.
    HitTest,
}

impl RichTextObjectSelector {
    /// Deterministic complete object-selector inventory.
    pub const ALL: [Self; 1] = [Self::Object];

    /// Resolves the current explicit family spelling.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"object" => Some(Self::Object),
            _ => None,
        }
    }

    /// Canonical formatter spelling.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Object => "object",
        }
    }

    /// Immutable owner-typed schema for inline objects.
    #[must_use]
    pub const fn schema(self) -> &'static RichTextTagSchema<RichTextObjectProperty> {
        match self {
            Self::Object => &OBJECT_SCHEMA,
        }
    }
}

impl RichTextObjectProperty {
    /// Deterministic complete object-metadata property inventory.
    pub const ALL: [Self; 5] = [
        Self::Type,
        Self::Role,
        Self::Layer,
        Self::Depth,
        Self::HitTest,
    ];

    /// Canonical source key.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Role => "role",
            Self::Layer => "layer",
            Self::Depth => "depth",
            Self::HitTest => "hit_test",
        }
    }

    /// Resolves a canonical source key without aliases or normalization.
    #[must_use]
    pub const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"type" => Some(Self::Type),
            b"role" => Some(Self::Role),
            b"layer" => Some(Self::Layer),
            b"depth" => Some(Self::Depth),
            b"hit_test" => Some(Self::HitTest),
            _ => None,
        }
    }
}

const SINGLE: Multiplicity = Multiplicity::Single;
const PUBLIC_ID_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &[],
    max_encoded_bytes: 4_096,
    max_decoded_bytes: 4_096,
};
const DEPTH_LIMITS: RichTextValueLimits = RichTextValueLimits {
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
const BOOL_LIMITS: RichTextValueLimits = RichTextValueLimits {
    numeric: None,
    units: &[],
    enum_values: &["false", "true"],
    max_encoded_bytes: 64,
    max_decoded_bytes: 64,
};

const fn optional_public_id(
    id: RichTextObjectProperty,
    source_name: &'static str,
) -> RichTextPropertySpec<RichTextObjectProperty> {
    RichTextPropertySpec {
        id,
        source_name,
        kind: RichTextValueKind::PublicId,
        presence: PropertyPresence::Optional,
        multiplicity: SINGLE,
        limits: PUBLIC_ID_LIMITS,
        allow_empty: false,
    }
}

const OBJECT_PROPERTIES: [RichTextPropertySpec<RichTextObjectProperty>; 5] = [
    optional_public_id(RichTextObjectProperty::Type, "type"),
    optional_public_id(RichTextObjectProperty::Role, "role"),
    optional_public_id(RichTextObjectProperty::Layer, "layer"),
    RichTextPropertySpec {
        id: RichTextObjectProperty::Depth,
        source_name: "depth",
        kind: RichTextValueKind::Length,
        presence: PropertyPresence::Optional,
        multiplicity: SINGLE,
        limits: DEPTH_LIMITS,
        allow_empty: false,
    },
    RichTextPropertySpec {
        id: RichTextObjectProperty::HitTest,
        source_name: "hit_test",
        kind: RichTextValueKind::Bool,
        presence: PropertyPresence::Defaulted(RichTextDefaultValue::Bool(false)),
        multiplicity: SINGLE,
        limits: BOOL_LIMITS,
        allow_empty: false,
    },
];

const OBJECT_SCHEMA: RichTextTagSchema<RichTextObjectProperty> = RichTextTagSchema {
    source_forms: &[
        RichTextSourceForm::CanonicalTag("object"),
        RichTextSourceForm::DotSelector,
    ],
    selector: SelectorContract::RequiredPositional {
        kind: SelectorKind::PublicId,
    },
    properties: &OBJECT_PROPERTIES,
    unknown_policy: UnknownPropertyPolicy::Reject,
    output: CheckedOutputKind::Object,
};
