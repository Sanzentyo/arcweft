//! Owner-neutral schema descriptors for checked rich-text authoring.
//!
//! This Sans I/O crate defines only the vocabulary that dialogue and
//! presentation owners use to publish immutable schemas. It deliberately owns
//! no source parser, selector, property, registry, diagnostic, checked value, or wire
//! identity.

/// Immutable schema for one zero-width dialogue point action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextPointActionSchema<P: Copy + Eq + 'static> {
    /// Typed source identity and canonical spelling for the point action.
    pub source: RichTextPointActionSource,
    /// Selector contract required by this point action.
    pub selector: SelectorContract,
    /// Properties in deterministic owner-defined order.
    pub properties: &'static [RichTextPropertySpec<P>],
    /// Treatment of a key absent from `properties`.
    pub unknown_policy: UnknownPropertyPolicy,
    /// Kind of checked action constructed after validation.
    pub output: CheckedOutputKind,
}

/// Immutable metadata for one presentation property set.
///
/// Body-bearing presentation calls are described by the presentation Content
/// callable catalog. This type carries only the property rows needed to
/// project those callable parameters; it has no source spelling or output
/// grammar authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextPropertySetSchema<P: Copy + Eq + 'static> {
    /// Properties in deterministic owner-defined order.
    pub properties: &'static [RichTextPropertySpec<P>],
}

/// One point-action source identity and its canonical authored spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextPointActionSource {
    /// Stable owner identity used by the point-action schema.
    pub identity: &'static str,
    /// Canonical source spelling accepted by the grammar.
    pub spelling: &'static str,
}

impl RichTextPointActionSource {
    /// Constructs one canonical point-action source identity.
    #[must_use]
    pub const fn canonical(spelling: &'static str) -> Self {
        Self {
            identity: spelling,
            spelling,
        }
    }

    /// Returns the point-action source identity.
    #[must_use]
    pub const fn identity(self) -> &'static str {
        self.identity
    }

    /// Returns the canonical authored spelling.
    #[must_use]
    pub const fn spelling(self) -> &'static str {
        self.spelling
    }
}

/// Schema for one property identity owned by a dialogue point action or a
/// presentation property set.
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

/// Selector position and identity contract for one point-action schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorContract {
    /// The owner has no selector.
    None,
    /// The selector is the first positional argument.
    RequiredPositional {
        /// Identity domain used to validate the selector.
        kind: SelectorKind,
    },
    /// The dot-prefixed point-action head supplies the selector.
    SuppliedByDotHead {
        /// Identity domain used to validate the selector.
        kind: SelectorKind,
    },
}

/// Closed selector identity domains shared by schema owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorKind {
    /// Validated public identity.
    PublicId,
    /// Member of an owner-defined closed selector enum.
    Closed,
}

/// Closed identity of a presentation-owned enum domain.
///
/// The enum's members and their order remain on the owning presentation enum;
/// this lower-layer identity only distinguishes the domains in checked values
/// and schema digests. Keeping this set closed prevents a source string from
/// becoming an accidental second enum registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RichTextEnumDomain {
    /// Style-call selector domain.
    StyleSelector,
    /// Layout-call selector domain.
    LayoutSelector,
    /// Transform-call selector domain.
    TransformSelector,
    /// Inline layout direction domain.
    LayoutDirection,
    /// Vertical Latin orientation domain.
    VerticalLatin,
    /// Japanese layout requirement preset domain.
    Jlreq,
    /// Transform target domain.
    TransformTarget,
    /// Transform origin domain.
    TransformOrigin,
}

impl RichTextEnumDomain {
    /// Complete deterministic domain inventory.
    pub const ALL: [Self; 8] = [
        Self::StyleSelector,
        Self::LayoutSelector,
        Self::TransformSelector,
        Self::LayoutDirection,
        Self::VerticalLatin,
        Self::Jlreq,
        Self::TransformTarget,
        Self::TransformOrigin,
    ];

    /// Stable zero-based domain discriminant used by canonical transcripts.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::StyleSelector => 0,
            Self::LayoutSelector => 1,
            Self::TransformSelector => 2,
            Self::LayoutDirection => 3,
            Self::VerticalLatin => 4,
            Self::Jlreq => 5,
            Self::TransformTarget => 6,
            Self::TransformOrigin => 7,
        }
    }

    /// Canonical diagnostic/schema label for this domain.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StyleSelector => "style_selector",
            Self::LayoutSelector => "layout_selector",
            Self::TransformSelector => "transform_selector",
            Self::LayoutDirection => "layout_direction",
            Self::VerticalLatin => "vertical_latin",
            Self::Jlreq => "jlreq",
            Self::TransformTarget => "transform_target",
            Self::TransformOrigin => "transform_origin",
        }
    }

    #[must_use]
    pub const fn domain_id(self) -> arcweft_id::closed_enum::ClosedEnumDomainId {
        arcweft_id::closed_enum::ClosedEnumDomainId::new(
            arcweft_id::closed_enum::ClosedEnumOwnerId::RichText,
            self.semantic_tag(),
        )
    }
}

/// Version marker for the owner-neutral rich-text schema transcript.
pub const RICH_TEXT_SCHEMA_VERSION: u8 = 1;

/// Stable digest of one complete typed content-call schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RichTextCallableSchemaDigest([u8; 32]);

impl RichTextCallableSchemaDigest {
    /// Hashes canonical schema bytes with the version-one rich-text domain.
    #[must_use]
    pub fn derive(canonical_bytes: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key("arcweft.rich-text-callable-schema.v1");
        hasher.update(canonical_bytes);
        Self(*hasher.finalize().as_bytes())
    }

    /// Constructs a digest from already verified bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Consumes the digest and returns its exact bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
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
    ClosedEnum(arcweft_id::closed_enum::ClosedEnumDomainId),
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
    /// One exact typed Fx application.
    Fx,
}

/// Typed membership constraint for a closed enum parameter.
///
/// The owning presentation enum remains the sole authority for membership,
/// names, and ordinals. `Allowed` carries only those owner-issued ordinals so
/// a branch can narrow a domain without introducing a second source-string
/// inventory in a lower-layer DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextEnumValueConstraint {
    /// Every member of the parameter's closed enum domain is accepted.
    All,
    /// Exactly one owner-issued ordinal is accepted for a branch row.
    Exact(u16),
    /// Only the listed owner-issued ordinals are accepted.
    Allowed(&'static [u16]),
}

/// Passing mode of a callable parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextCallableParameterPassing {
    /// The parameter may occur only positionally.
    PositionalOnly,
    /// The parameter may occur only by its canonical name.
    NamedOnly,
}

/// Presence/default policy of a callable parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextCallableParameterPresence<P: Copy + Eq + 'static> {
    /// The parameter must be supplied.
    Required,
    /// The parameter may be omitted without materializing a value.
    Optional,
    /// The owner materializes this value when omitted.
    Defaulted(RichTextDefaultValue),
    /// Presence depends on another parameter in this callable row.
    Conditional {
        /// Owner-defined predicate.
        predicate: RichTextPropertyPredicate<P>,
    },
}

/// One typed parameter in a content callable row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextCallableParameterSpec<P: Copy + Eq + 'static> {
    /// Typed parameter identity owned by the row's domain.
    pub id: P,
    /// Canonical source name (when named passing is allowed).
    pub source_name: &'static str,
    /// Exact checked value kind.
    pub kind: RichTextValueKind,
    /// Positional/named admission mode.
    pub passing: RichTextCallableParameterPassing,
    /// Requiredness, optionality, or owner default.
    pub presence: RichTextCallableParameterPresence<P>,
    /// Materialized default when a conditional predicate is true and the
    /// parameter is omitted.
    pub conditional_default: Option<RichTextDefaultValue>,
    /// Typed closed-enum membership, when the value kind is `ClosedEnum`.
    pub enum_constraint: RichTextEnumValueConstraint,
    /// Numeric, unit, enum, and byte limits.
    pub limits: RichTextValueLimits,
    /// Whether a present decoded empty text value is accepted.
    pub allow_empty: bool,
}

/// Bounds attached to one property value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichTextValueLimits {
    /// Optional fixed/integer numeric limits.
    pub numeric: Option<RichTextNumericLimits>,
    /// Units accepted by this property, in deterministic display order.
    pub units: &'static [RichTextUnit],
    /// Optional diagnostic spellings; never a closed-enum membership authority.
    ///
    /// Callable DTOs carry typed ordinal constraints separately. Legacy
    /// property descriptors may retain these spellings only for diagnostics
    /// while their owning enum remains authoritative.
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

/// Typed value view consumed by owner-defined conditional property rules.
///
/// The schema owns predicate semantics while each consumer owns its checked
/// value algebra. Keeping this tiny view at the schema boundary prevents
/// point-action, builtin-effect, and Content-call checkers from developing
/// separate predicate interpreters.
pub trait RichTextPredicateValueView {
    /// Returns the Boolean projection when this value is Boolean.
    fn predicate_bool(&self) -> Option<bool>;

    /// Returns the closed-enum ordinal when this value is an enum.
    fn predicate_enum_variant(&self) -> Option<u16>;
}

impl<P: Copy + Eq + 'static> RichTextPropertyPredicate<P> {
    /// Evaluates this schema predicate against an owner-provided typed value
    /// lookup. The lookup is called only for the referenced property, and
    /// `Present` means that any typed value exists for that property.
    #[must_use]
    pub fn holds<'a, V: RichTextPredicateValueView + ?Sized + 'a>(
        self,
        mut value_for: impl FnMut(P) -> Option<&'a V>,
    ) -> bool {
        match self {
            Self::Present(property) => value_for(property).is_some(),
            Self::BoolEquals { property, value } => {
                value_for(property).and_then(V::predicate_bool) == Some(value)
            }
            Self::EnumEquals { property, variant } => {
                value_for(property).and_then(V::predicate_enum_variant) == Some(variant)
            }
        }
    }
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
    /// Reject the containing point action; unknown data never disappears.
    Reject,
}

/// Family-specific checked output selected by one owner schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedOutputKind {
    /// Zero-width dialogue control.
    PointControl,
    /// Structural rich-text content modifier scope.
    Span,
    /// Typed retained text object/proxy.
    Object,
    /// Typed renderer-neutral host event.
    Host,
    /// Explicit zero-width marker.
    Marker,
}

#[cfg(test)]
mod tests {
    use super::{
        CheckedOutputKind, Multiplicity, PropertyPresence, RICH_TEXT_SCHEMA_VERSION,
        RichTextCallableSchemaDigest, RichTextDefaultValue, RichTextEnumDomain,
        RichTextNumericLimits, RichTextPointActionSchema, RichTextPointActionSource,
        RichTextPredicateValueView, RichTextPropertyPredicate, RichTextPropertySetSchema,
        RichTextPropertySpec, RichTextUnit, RichTextValueKind, RichTextValueLimits,
        SelectorContract, UnknownPropertyPolicy,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ExampleProperty {
        Mode,
        Enabled,
    }

    const ENUM_ID: arcweft_id::closed_enum::ClosedEnumDomainId =
        RichTextEnumDomain::LayoutDirection.domain_id();
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
    const POINT_ACTION_SCHEMA: RichTextPointActionSchema<ExampleProperty> =
        RichTextPointActionSchema {
            source: RichTextPointActionSource::canonical("example"),
            selector: SelectorContract::None,
            properties: PROPERTIES,
            unknown_policy: UnknownPropertyPolicy::Reject,
            output: CheckedOutputKind::PointControl,
        };
    const PROPERTY_SET_SCHEMA: RichTextPropertySetSchema<ExampleProperty> =
        RichTextPropertySetSchema {
            properties: PROPERTIES,
        };

    struct PredicateValue {
        boolean: Option<bool>,
        variant: Option<u16>,
    }

    impl RichTextPredicateValueView for PredicateValue {
        fn predicate_bool(&self) -> Option<bool> {
            self.boolean
        }

        fn predicate_enum_variant(&self) -> Option<u16> {
            self.variant
        }
    }

    #[test]
    fn descriptor_preserves_owner_property_identity_and_order() {
        assert_eq!(POINT_ACTION_SCHEMA.properties[0].id, ExampleProperty::Mode);
        assert_eq!(
            POINT_ACTION_SCHEMA.properties[1].id,
            ExampleProperty::Enabled
        );
        assert_eq!(POINT_ACTION_SCHEMA.properties[0].source_name, "mode");
        assert_eq!(PROPERTY_SET_SCHEMA.properties, PROPERTIES);
        assert_eq!(POINT_ACTION_SCHEMA.source.identity(), "example");
        assert_eq!(POINT_ACTION_SCHEMA.source.spelling(), "example");
        assert_eq!(
            RichTextEnumDomain::LayoutDirection.as_str(),
            "layout_direction"
        );
    }

    #[test]
    fn defaults_and_conditions_remain_distinct_schema_states() {
        assert_eq!(
            POINT_ACTION_SCHEMA.properties[0].presence,
            PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0))
        );
        assert_eq!(
            POINT_ACTION_SCHEMA.properties[1].presence,
            PropertyPresence::Conditional {
                predicate: RichTextPropertyPredicate::EnumEquals {
                    property: ExampleProperty::Mode,
                    variant: 1,
                },
            }
        );
    }

    #[test]
    fn predicates_use_the_shared_typed_value_view() {
        let enabled = PredicateValue {
            boolean: Some(true),
            variant: None,
        };
        assert!(
            RichTextPropertyPredicate::BoolEquals {
                property: ExampleProperty::Enabled,
                value: true,
            }
            .holds(|property| (property == ExampleProperty::Enabled).then_some(&enabled))
        );
        assert!(
            !RichTextPropertyPredicate::BoolEquals {
                property: ExampleProperty::Enabled,
                value: false,
            }
            .holds(|property| (property == ExampleProperty::Enabled).then_some(&enabled))
        );
        assert!(
            !RichTextPropertyPredicate::EnumEquals {
                property: ExampleProperty::Mode,
                variant: 1,
            }
            .holds(|property| (property == ExampleProperty::Enabled).then_some(&enabled))
        );
        assert!(
            !RichTextPropertyPredicate::Present(ExampleProperty::Mode)
                .holds(|_| None::<&PredicateValue>)
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

    #[test]
    fn enum_domain_inventory_is_closed_and_stable() {
        assert_eq!(RichTextEnumDomain::ALL.len(), 8);
        for domain in RichTextEnumDomain::ALL {
            assert_eq!(domain.domain_id().local_tag(), domain.semantic_tag());
            assert!(!domain.as_str().is_empty());
        }
        assert_eq!(RICH_TEXT_SCHEMA_VERSION, 1);
        assert_ne!(
            RichTextCallableSchemaDigest::derive(b"strong"),
            RichTextCallableSchemaDigest::derive(b"em")
        );
    }
}
