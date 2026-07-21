//! Owner-neutral schema descriptors for checked rich-text authoring.
//!
//! This Sans I/O crate defines only the vocabulary that dialogue and
//! presentation owners use to publish immutable schemas. It deliberately owns
//! no tag, selector, property, registry, diagnostic, checked value, or wire
//! identity.

/// Immutable schema for one rich-text tag owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextTagSchema<P: Copy + Eq + 'static> {
    /// Source forms accepted by the owning domain enum.
    pub source_forms: &'static [RichTextSourceForm],
    /// Selector form required by this owner.
    pub selector: SelectorContract,
    /// Properties in deterministic owner-defined order.
    pub properties: &'static [RichTextPropertySpec<P>],
    /// Treatment of a key absent from `properties`.
    pub unknown_policy: UnknownPropertyPolicy,
    /// Kind of checked action constructed after validation.
    pub output: CheckedOutputKind,
}

/// Schema for one property identity owned by a dialogue or presentation enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextPropertySpec<P: Copy + Eq + 'static> {
    /// Owner-defined semantic property identity.
    pub id: P,
    /// Sole canonical source key.
    pub source_name: &'static str,
    /// Checked value kind.
    pub kind: RichTextValueKind,
    /// Whether the property is required, optional, defaulted, or conditional.
    pub presence: PropertyPresence<P>,
    /// Whether the property may occur once or as an explicitly bounded list.
    pub multiplicity: Multiplicity,
    /// Numeric, unit, enum, and byte limits.
    pub limits: RichTextValueLimits,
    /// Whether a present decoded empty text value is accepted.
    pub allow_empty: bool,
}

/// Source form advertised by an owning tag enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextSourceForm {
    /// Canonical tag spelling.
    CanonicalTag(&'static str),
    /// Grammar-owned alternate spelling with one canonical formatter output.
    GrammarSpelling {
        /// Accepted source spelling.
        source: &'static str,
        /// Canonical formatter spelling.
        canonical: &'static str,
    },
    /// Explicit family syntax such as `style .selector`.
    ExplicitFamily,
    /// Dot-selector shorthand where the selector is supplied by the tag head.
    DotSelector,
    /// Dedicated call or expression payload grammar.
    DedicatedPayload,
}

/// Selector position and identity contract for one tag schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorContract {
    /// The owner has no selector.
    None,
    /// The selector is the first positional argument.
    RequiredPositional {
        /// Identity domain used to validate the selector.
        kind: SelectorKind,
    },
    /// The dot-prefixed tag head supplies the selector.
    SuppliedByDotHead {
        /// Identity domain used to validate the selector.
        kind: SelectorKind,
    },
    /// Resolve one registered typed Fx definition.
    RegisteredFx,
    /// Resolve one registered typed text-proxy schema.
    RegisteredTextProxy,
}

/// Closed selector identity domains shared by schema owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorKind {
    /// Validated public identity.
    PublicId,
    /// Member of an owner-defined closed selector enum.
    Closed,
    /// Member of the Arcweft-owned built-in Fx inventory.
    BuiltinFx,
    /// Visible typed text-proxy schema.
    TextProxy,
}

/// Stable owner identity for a closed enum value kind.
///
/// Membership and variant ordering remain on the owning enum. This identifier
/// only lets diagnostics and checked values distinguish enum domains without a
/// global registry in this crate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RichTextEnumSchemaId(&'static str);

impl RichTextEnumSchemaId {
    /// Creates an owner-selected static enum-schema identity.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the owner-selected identity spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Value classes understood by the shared semantic validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextValueKind {
    /// Exact lowercase `true` or `false`.
    Bool,
    /// Checked signed decimal integer.
    Int,
    /// Checked decimal represented in thousandths.
    FixedMilli,
    /// Unitless ratio in inclusive thousandths `0..=1000`.
    Ratio,
    /// Fixed length with an owner-accepted unit.
    Length,
    /// Fixed angle.
    Angle,
    /// Exact duration.
    Duration,
    /// Member of a specific closed enum domain.
    ClosedEnum(RichTextEnumSchemaId),
    /// Selector belonging to a specific identity domain.
    Selector(SelectorKind),
    /// Validated Arcweft public identity.
    PublicId,
    /// Validated UTF-8 text.
    Text,
    /// Validated color.
    Color,
    /// Pair of fixed decimal components.
    Vec2,
    /// Deterministic 32-bit seed.
    Seed32,
    /// Field whose closed scalar kind comes from a typed text-proxy schema.
    TextProxyField,
}

/// Bounds attached to one property value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextValueLimits {
    /// Optional fixed/integer numeric limits.
    pub numeric: Option<RichTextNumericLimits>,
    /// Units accepted by this property, in deterministic display order.
    pub units: &'static [RichTextUnit],
    /// Closed enum spellings, in owner-defined variant order.
    pub enum_values: &'static [&'static str],
    /// Maximum authored token bytes.
    pub max_encoded_bytes: u16,
    /// Maximum decoded value bytes.
    pub max_decoded_bytes: u16,
}

impl RichTextValueLimits {
    /// Limits for a property that accepts no encoded value.
    pub const NONE: Self = Self {
        numeric: None,
        units: &[],
        enum_values: &[],
        max_encoded_bytes: 0,
        max_decoded_bytes: 0,
    };
}

/// Fixed/integer numeric limits expressed without floating point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextNumericLimits {
    /// Optional inclusive minimum in thousandths.
    pub inclusive_min_milli: Option<i64>,
    /// Optional inclusive maximum in thousandths.
    pub inclusive_max_milli: Option<i64>,
    /// Maximum decimal digits before the fractional part.
    pub max_integer_digits: u8,
    /// Maximum fractional digits.
    pub max_fraction_digits: u8,
}

/// Units accepted by owner-defined rich-text property schemas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextUnit {
    /// No authored unit.
    Unitless,
    /// Logical pixels.
    Px,
    /// Typographic points.
    Pt,
    /// Character advance unit.
    Ch,
    /// Font-relative em unit.
    Em,
    /// Degrees.
    Deg,
    /// Milliseconds.
    Ms,
    /// Seconds.
    S,
    /// Characters per second.
    Cps,
}

/// Presence and defaulting policy for one owner-defined property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyPresence<P: Copy + Eq + 'static> {
    /// The property must be authored.
    Required,
    /// The property may be absent and has no materialized default.
    Optional,
    /// The owner materializes this default only when the property is absent.
    Defaulted(RichTextDefaultValue),
    /// Presence depends on another property in the same owner schema.
    Conditional {
        /// Owner-defined deterministic predicate.
        predicate: RichTextPropertyPredicate<P>,
    },
}

/// Cross-property predicate used by conditional presence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextPropertyPredicate<P: Copy + Eq + 'static> {
    /// True when the referenced property is present.
    Present(P),
    /// True when the referenced Boolean property has this value.
    BoolEquals {
        /// Referenced owner property.
        property: P,
        /// Required Boolean value.
        value: bool,
    },
    /// True when the referenced closed-enum property has this variant.
    EnumEquals {
        /// Referenced owner property.
        property: P,
        /// Required owner-defined variant index.
        variant: u16,
    },
}

/// Closed materializable defaults supported by schema descriptors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextDefaultValue {
    /// Boolean default.
    Bool(bool),
    /// Signed integer default.
    Int(i64),
    /// Fixed decimal default in thousandths.
    Milli(i32),
    /// Ratio default in inclusive thousandths.
    RatioMilli(u16),
    /// Length default.
    Length {
        /// Magnitude in thousandths.
        milli: i32,
        /// Length unit.
        unit: RichTextUnit,
    },
    /// Angle default in milli-degrees.
    AngleMilliDegrees(i32),
    /// Duration default in milliseconds.
    DurationMillis(u64),
    /// Owner-defined closed-enum variant index.
    EnumVariant(u16),
    /// Public identity default.
    PublicId(&'static str),
    /// Text default.
    Text(&'static str),
    /// Color default as RGBA8.
    ColorRgba8([u8; 4]),
    /// Two fixed components in thousandths.
    Vec2Milli([i32; 2]),
    /// Deterministic seed default.
    Seed32(u32),
}

/// Multiplicity of one semantic property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Multiplicity {
    /// At most one occurrence.
    Single,
    /// Explicitly repeatable property with an owner-selected upper bound.
    Repeated {
        /// Maximum retained occurrences.
        max: u16,
    },
}

/// Policy for keys absent from an owner schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownPropertyPolicy {
    /// Reject the containing tag; unknown data never disappears.
    Reject,
}

/// Family-specific checked output selected by one owner schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedOutputKind {
    /// Zero-width dialogue control.
    PointControl,
    /// Paired rich-text span.
    Span,
    /// Typed retained text object/proxy.
    Object,
    /// Arcweft-owned built-in effect.
    BuiltinFx,
    /// Declared typed effect.
    DeclaredFx,
    /// Typed renderer-neutral host event.
    Host,
    /// Explicit zero-width marker.
    Marker,
}

#[cfg(test)]
mod tests {
    use super::{
        CheckedOutputKind, Multiplicity, PropertyPresence, RichTextDefaultValue,
        RichTextEnumSchemaId, RichTextNumericLimits, RichTextPropertyPredicate,
        RichTextPropertySpec, RichTextSourceForm, RichTextTagSchema, RichTextUnit,
        RichTextValueKind, RichTextValueLimits, SelectorContract, UnknownPropertyPolicy,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ExampleProperty {
        Mode,
        Enabled,
    }

    const ENUM_ID: RichTextEnumSchemaId = RichTextEnumSchemaId::new("example.mode");
    const PROPERTIES: &[RichTextPropertySpec<ExampleProperty>] = &[
        RichTextPropertySpec {
            id: ExampleProperty::Mode,
            source_name: "mode",
            kind: RichTextValueKind::ClosedEnum(ENUM_ID),
            presence: PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0)),
            multiplicity: Multiplicity::Single,
            limits: RichTextValueLimits {
                numeric: None,
                units: &[],
                enum_values: &["normal", "strict"],
                max_encoded_bytes: 64,
                max_decoded_bytes: 64,
            },
            allow_empty: false,
        },
        RichTextPropertySpec {
            id: ExampleProperty::Enabled,
            source_name: "enabled",
            kind: RichTextValueKind::Bool,
            presence: PropertyPresence::Conditional {
                predicate: RichTextPropertyPredicate::EnumEquals {
                    property: ExampleProperty::Mode,
                    variant: 1,
                },
            },
            multiplicity: Multiplicity::Single,
            limits: RichTextValueLimits {
                numeric: None,
                units: &[],
                enum_values: &["false", "true"],
                max_encoded_bytes: 64,
                max_decoded_bytes: 64,
            },
            allow_empty: false,
        },
    ];
    const SCHEMA: RichTextTagSchema<ExampleProperty> = RichTextTagSchema {
        source_forms: &[
            RichTextSourceForm::CanonicalTag("example"),
            RichTextSourceForm::GrammarSpelling {
                source: "sample",
                canonical: "example",
            },
        ],
        selector: SelectorContract::None,
        properties: PROPERTIES,
        unknown_policy: UnknownPropertyPolicy::Reject,
        output: CheckedOutputKind::Span,
    };

    #[test]
    fn descriptor_preserves_owner_property_identity_and_order() {
        assert_eq!(SCHEMA.properties[0].id, ExampleProperty::Mode);
        assert_eq!(SCHEMA.properties[1].id, ExampleProperty::Enabled);
        assert_eq!(SCHEMA.properties[0].source_name, "mode");
        assert_eq!(ENUM_ID.as_str(), "example.mode");
    }

    #[test]
    fn defaults_and_conditions_remain_distinct_schema_states() {
        assert_eq!(
            SCHEMA.properties[0].presence,
            PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0))
        );
        assert_eq!(
            SCHEMA.properties[1].presence,
            PropertyPresence::Conditional {
                predicate: RichTextPropertyPredicate::EnumEquals {
                    property: ExampleProperty::Mode,
                    variant: 1,
                },
            }
        );
    }

    #[test]
    fn numeric_and_unit_limits_are_integer_only() {
        let limits = RichTextValueLimits {
            numeric: Some(RichTextNumericLimits {
                inclusive_min_milli: Some(-1_000),
                inclusive_max_milli: Some(1_000),
                max_integer_digits: 19,
                max_fraction_digits: 3,
            }),
            units: &[RichTextUnit::Px],
            enum_values: &[],
            max_encoded_bytes: 64,
            max_decoded_bytes: 64,
        };

        assert_eq!(
            limits.numeric.expect("numeric limits").inclusive_min_milli,
            Some(-1_000)
        );
        assert_eq!(limits.units, [RichTextUnit::Px]);
    }
}
