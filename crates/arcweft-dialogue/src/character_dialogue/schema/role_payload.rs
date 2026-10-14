//! Dialogue-owned payload codecs for authored CharacterDialogue roles.

use super::super::{CharacterDialogueValueError, PRODUCTION_CHARACTER_DIALOGUE_LIMITS};
use arcweft_core::{
    entry::RuntimeValueDigest,
    pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeCheckedType, RuntimeSemanticTypeId},
    plan::{RuntimePlanTypeProjection, RuntimePlanTypeSeed},
    value::{
        RuntimeBorrowedValue, RuntimeScalarView, RuntimeSignedIntWidth, RuntimeUInt,
        RuntimeUnsignedIntWidth, RuntimeValue,
    },
};
use arcweft_id::{PublicId, closed_enum::ClosedEnumDomainId};
use arcweft_presentation::rich_text::{
    Jlreq, LayoutDirection, PRESENTATION_CONTENT_CALLABLE_CATALOG,
    PresentationContentCallableDefinitionId, PresentationContentCallableParameterId,
    RichTextLayoutSelector, RichTextStyleSelector, RichTextTransformSelector, TransformOrigin,
    TransformTarget, VerticalLatin,
};
use arcweft_rich_text_schema::{
    RichTextEnumDomain, RichTextEnumValueConstraint, RichTextUnit, RichTextValueKind, SelectorKind,
};
use std::{collections::BTreeMap, sync::OnceLock};

const RICH_TEXT_ROLE_CODEC_TAG: u8 = 0;
const RICH_TEXT_ROLE_CODEC_DIGEST_DOMAIN: &[u8] =
    b"arcweft.character-dialogue.rich-text-properties-codec.v1\0";

/// One closed Dialogue-owned codec for a bound role payload.
///
/// Optional authored roles remain unbound until Dialogue owns a complete
/// constructor and consumer contract for them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterDialogueRolePayloadCodec {
    /// Sparse per-property values from the presentation-owned reusable style
    /// projection. Content-only arguments are never configuration fields.
    RichTextProperties,
}

/// Complete immutable Core type graph for one bound role payload.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRolePayloadSchema {
    root: RuntimeSemanticTypeId,
    types: Box<[RuntimePlanTypeSeed]>,
    schema_digest: RuntimeValueDigest,
    checked_types: BTreeMap<RuntimeSemanticTypeId, RuntimeCheckedType>,
    fields: Box<[RichTextPolicyField]>,
    no_overrides_payload: RuntimeValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RichTextPolicyField {
    definition: PresentationContentCallableDefinitionId,
    parameter: arcweft_presentation::rich_text::PresentationContentCallableParameterSpec,
    value_type: RuntimeCheckedType,
}

/// One decoded, present RichText policy property.
///
/// Coordinates use the exact presentation catalog identities. Absence is
/// represented by no row, so callers do not need to inspect wire slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRichTextProperty {
    definition: PresentationContentCallableDefinitionId,
    parameter: PresentationContentCallableParameterId,
    value: CharacterDialogueRichTextPropertyValue,
}

/// Canonically decoded RichText property overrides.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CharacterDialogueRichTextProperties {
    properties: Vec<CharacterDialogueRichTextProperty>,
}

/// Typed color accepted by the presentation Content property schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDialogueRichTextColor<I = PublicId> {
    /// Literal RGBA8 color.
    Rgba8([u8; 4]),
    /// Presentation-owned named color resource.
    Resource(I),
}

/// Typed scalar carried by one present RichText policy property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDialogueRichTextPropertyValue<S = String, I = PublicId> {
    Bool(bool),
    Int(i64),
    Milli(i32),
    Ratio(u16),
    Length {
        milli: i32,
        unit: RichTextUnit,
    },
    Angle(i32),
    Duration(u64),
    ClosedEnum {
        domain: ClosedEnumDomainId,
        variant: u16,
    },
    PublicId(I),
    Text(S),
    Color(CharacterDialogueRichTextColor<I>),
    Vec2 {
        x_milli: i32,
        y_milli: i32,
    },
    Seed32(u32),
}

impl CharacterDialogueRolePayloadCodec {
    /// Stable role-codec tag included in the generation declaration digest.
    #[must_use]
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::RichTextProperties => RICH_TEXT_ROLE_CODEC_TAG,
        }
    }

    /// Returns the complete Core graph and value contract for this codec.
    pub fn payload_schema(
        self,
    ) -> Result<&'static CharacterDialogueRolePayloadSchema, CharacterDialogueValueError> {
        static RICH_TEXT_SCHEMA: OnceLock<
            Result<CharacterDialogueRolePayloadSchema, CharacterDialogueValueError>,
        > = OnceLock::new();

        let schema = match self {
            Self::RichTextProperties => RICH_TEXT_SCHEMA
                .get_or_init(|| CharacterDialogueRolePayloadSchema::try_rich_text_properties()),
        };
        schema.as_ref().map_err(Clone::clone)
    }

    /// Returns the canonical no-overrides payload for this codec.
    pub fn no_overrides_payload(self) -> Result<RuntimeValue, CharacterDialogueValueError> {
        Ok(self.payload_schema()?.no_overrides_payload.clone())
    }

    /// Decodes only values admitted by this codec's exact property contract.
    pub fn decode_properties<'a>(
        self,
        payload: impl Into<RuntimeBorrowedValue<'a>>,
    ) -> Result<CharacterDialogueRichTextProperties, CharacterDialogueValueError> {
        match self {
            Self::RichTextProperties => self.payload_schema()?.decode_properties(payload.into()),
        }
    }

    /// Checks the same property grammar on live/inert borrowed fields without
    /// building owned property rows or copying their strings.
    pub fn validate_properties<'a>(
        self,
        payload: impl Into<RuntimeBorrowedValue<'a>>,
    ) -> Result<(), Box<CharacterDialogueValueError>> {
        self.payload_schema()?
            .visit_properties(payload.into(), |_, _| Ok(()))
    }
}

impl CharacterDialogueRolePayloadSchema {
    fn try_rich_text_properties() -> Result<Self, CharacterDialogueValueError> {
        let fields = PRESENTATION_CONTENT_CALLABLE_CATALOG
            .reusable_style_parameters()
            .map(|(definition, parameter)| {
                Ok(RichTextPolicyField {
                    definition,
                    parameter: *parameter,
                    value_type: policy_value_type(parameter.kind)?,
                })
            })
            .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?;

        if fields.is_empty() {
            return Err(schema_error("presentation property inventory is empty"));
        }
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_leaves);
        if fields.len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "rich_text_policy_fields",
                maximum,
            });
        }

        let root_type = RuntimeCheckedType::Tuple(
            fields
                .iter()
                .map(|field| RuntimeCheckedType::Option(Box::new(field.value_type.clone())))
                .collect(),
        );
        let root = root_type.semantic_identity_digest();
        let mut checked_types = BTreeMap::new();
        append_checked_type_graph(&root_type, &mut checked_types)?;
        if checked_types.len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "rich_text_policy_type_seeds",
                maximum,
            });
        }
        let types = checked_types
            .iter()
            .map(|(identity, checked)| {
                Ok(RuntimePlanTypeSeed::new(
                    *identity,
                    plan_projection(checked)?,
                ))
            })
            .collect::<Result<Vec<_>, CharacterDialogueValueError>>()?
            .into_boxed_slice();

        let no_overrides_payload =
            RuntimeValue::Tuple(fields.iter().map(|_| RuntimeValue::option_none()).collect());
        no_overrides_payload.try_canonical_bytes(
            PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
        )?;

        let mut hasher = blake3::Hasher::new();
        hasher.update(RICH_TEXT_ROLE_CODEC_DIGEST_DOMAIN);
        hasher.update(&[RICH_TEXT_ROLE_CODEC_TAG]);
        hasher.update(
            PRESENTATION_CONTENT_CALLABLE_CATALOG
                .reusable_style_schema_digest()
                .as_bytes(),
        );
        hasher.update(&(fields.len() as u32).to_le_bytes());
        for field in &fields {
            hasher.update(field.value_type.semantic_identity_digest().as_bytes());
        }
        let schema_digest = RuntimeValueDigest::from_bytes(*hasher.finalize().as_bytes());

        Ok(Self {
            root,
            types,
            schema_digest,
            checked_types,
            fields: fields.into_boxed_slice(),
            no_overrides_payload,
        })
    }

    /// Canonical semantic identity of the payload root.
    #[must_use]
    pub const fn root(&self) -> RuntimeSemanticTypeId {
        self.root
    }

    /// Complete root and descendant graph for RuntimePlan/AWBC projection.
    #[must_use]
    pub fn types(&self) -> &[RuntimePlanTypeSeed] {
        &self.types
    }

    /// Version-one digest of the property inventory and codec contract.
    #[must_use]
    pub const fn schema_digest(&self) -> RuntimeValueDigest {
        self.schema_digest
    }

    pub(super) fn checked_type(
        &self,
        identity: RuntimeSemanticTypeId,
    ) -> Option<&RuntimeCheckedType> {
        self.checked_types.get(&identity)
    }

    fn decode_properties<'a>(
        &self,
        payload: impl Into<RuntimeBorrowedValue<'a>>,
    ) -> Result<CharacterDialogueRichTextProperties, CharacterDialogueValueError> {
        let payload = payload.into();
        self.visit_properties(payload, |_, _| Ok(()))
            .map_err(|error| *error)?;
        let mut properties = Vec::new();
        self.visit_properties(payload, |field, value| {
            properties.push(CharacterDialogueRichTextProperty {
                definition: field.definition,
                parameter: field.parameter.id,
                value: value.into_owned()?,
            });
            Ok(())
        })
        .map_err(|error| *error)?;
        Ok(CharacterDialogueRichTextProperties { properties })
    }

    fn visit_properties<'a>(
        &self,
        payload: RuntimeBorrowedValue<'a>,
        mut visit: impl FnMut(
            &RichTextPolicyField,
            CharacterDialogueRichTextPropertyValue<&'a str, &'a str>,
        ) -> Result<(), Box<CharacterDialogueValueError>>,
    ) -> Result<(), Box<CharacterDialogueValueError>> {
        let Some(slots) = payload.tuple() else {
            return Err(Box::new(schema_error("payload root must be a tuple")));
        };
        if slots.len() != self.fields.len() {
            return Err(Box::new(schema_error(
                "payload tuple has the wrong field count",
            )));
        }
        for (field, slot) in self.fields.iter().zip(slots.iter()) {
            match slot.builtin_variant_case() {
                Some((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => {}
                Some((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => {
                    visit(field, decode_property_value(field.parameter, value)?)?;
                }
                _ => {
                    return Err(Box::new(schema_error(
                        "payload property slot is not a canonical Option",
                    )));
                }
            }
        }
        Ok(())
    }
}

impl CharacterDialogueRichTextProperties {
    /// Present properties in the canonical presentation catalog order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CharacterDialogueRichTextProperty> {
        self.properties.iter()
    }

    /// Number of present property overrides.
    #[must_use]
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Whether this policy contains no overrides.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }
}

impl CharacterDialogueRichTextProperty {
    #[must_use]
    pub const fn definition(&self) -> PresentationContentCallableDefinitionId {
        self.definition
    }

    #[must_use]
    pub const fn parameter(&self) -> PresentationContentCallableParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn value(&self) -> &CharacterDialogueRichTextPropertyValue {
        &self.value
    }
}

fn policy_value_type(
    kind: RichTextValueKind,
) -> Result<RuntimeCheckedType, CharacterDialogueValueError> {
    use RuntimeCheckedType as Checked;
    use RuntimeSignedIntWidth::I32;
    use RuntimeUnsignedIntWidth::{U8, U16, U32, U64};

    Ok(match kind {
        RichTextValueKind::Bool => Checked::Bool,
        RichTextValueKind::Int => Checked::Signed(RuntimeSignedIntWidth::I64),
        RichTextValueKind::FixedMilli | RichTextValueKind::Angle => Checked::Signed(I32),
        RichTextValueKind::Ratio => Checked::Unsigned(U16),
        RichTextValueKind::Length => {
            Checked::Tuple(vec![Checked::Signed(I32), Checked::Unsigned(U8)])
        }
        RichTextValueKind::Duration => Checked::Unsigned(U64),
        RichTextValueKind::ClosedEnum(_) => Checked::Unsigned(U16),
        RichTextValueKind::Selector(SelectorKind::PublicId) | RichTextValueKind::PublicId => {
            Checked::String
        }
        RichTextValueKind::Selector(SelectorKind::Closed) => {
            return Err(schema_error("closed selector lacks an owning enum domain"));
        }
        RichTextValueKind::Text => Checked::String,
        RichTextValueKind::Color => Checked::Choice(vec![
            Checked::String,
            Checked::Tuple(vec![
                Checked::Unsigned(U8),
                Checked::Unsigned(U8),
                Checked::Unsigned(U8),
                Checked::Unsigned(U8),
            ]),
        ]),
        RichTextValueKind::Vec2 => Checked::Tuple(vec![Checked::Signed(I32), Checked::Signed(I32)]),
        RichTextValueKind::Seed32 => Checked::Unsigned(U32),
        RichTextValueKind::Fx => {
            return Err(schema_error(
                "Fx is an effect input, not a stored policy property",
            ));
        }
    })
}

fn append_checked_type_graph(
    checked: &RuntimeCheckedType,
    graph: &mut BTreeMap<RuntimeSemanticTypeId, RuntimeCheckedType>,
) -> Result<(), CharacterDialogueValueError> {
    match checked {
        RuntimeCheckedType::Tuple(items) | RuntimeCheckedType::Choice(items) => {
            for item in items {
                append_checked_type_graph(item, graph)?;
            }
        }
        RuntimeCheckedType::Option(item) => {
            append_checked_type_graph(item, graph)?;
            append_checked_type_graph(&RuntimeCheckedType::Tuple(vec![(**item).clone()]), graph)?;
        }
        RuntimeCheckedType::Bool
        | RuntimeCheckedType::Signed(_)
        | RuntimeCheckedType::Unsigned(_)
        | RuntimeCheckedType::String => {}
        _ => {
            return Err(schema_error(
                "codec generated an unsupported Core type shape",
            ));
        }
    }
    let identity = checked.semantic_identity_digest();
    if let Some(previous) = graph.get(&identity) {
        if previous != checked {
            return Err(schema_error(
                "Core semantic type identity collision in policy graph",
            ));
        }
    } else {
        graph.insert(identity, checked.clone());
    }
    Ok(())
}

fn plan_projection(
    checked: &RuntimeCheckedType,
) -> Result<RuntimePlanTypeProjection<RuntimeSemanticTypeId>, CharacterDialogueValueError> {
    use RuntimePlanTypeProjection as Projection;

    Ok(match checked {
        RuntimeCheckedType::Bool => Projection::Bool,
        RuntimeCheckedType::Signed(width) => Projection::Signed(*width),
        RuntimeCheckedType::Unsigned(width) => Projection::Unsigned(*width),
        RuntimeCheckedType::String => Projection::String,
        RuntimeCheckedType::Tuple(items) => Projection::Tuple(
            items
                .iter()
                .map(RuntimeCheckedType::semantic_identity_digest)
                .collect(),
        ),
        RuntimeCheckedType::Choice(items) => Projection::Choice(
            items
                .iter()
                .map(RuntimeCheckedType::semantic_identity_digest)
                .collect(),
        ),
        RuntimeCheckedType::Option(item) => {
            let some_payload =
                RuntimeCheckedType::Tuple(vec![(**item).clone()]).semantic_identity_digest();
            Projection::Option {
                item: item.semantic_identity_digest(),
                some_payload,
            }
        }
        _ => {
            return Err(schema_error(
                "codec generated an unsupported Core type shape",
            ));
        }
    })
}

fn decode_property_value(
    parameter: arcweft_presentation::rich_text::PresentationContentCallableParameterSpec,
    value: RuntimeBorrowedValue<'_>,
) -> Result<CharacterDialogueRichTextPropertyValue<&str, &str>, CharacterDialogueValueError> {
    use CharacterDialogueRichTextPropertyValue as PropertyValue;
    use RuntimeScalarView as Scalar;
    let invalid = || schema_error("property value differs from its presentation schema");
    Ok(match parameter.kind {
        RichTextValueKind::Bool => {
            let Some(Scalar::Bool(value)) = value.scalar() else {
                return Err(invalid());
            };
            PropertyValue::Bool(value)
        }
        RichTextValueKind::Int => {
            let Some(Scalar::Int(value)) = value.scalar() else {
                return Err(invalid());
            };
            let Some(value) = value.exact_i64() else {
                return Err(invalid());
            };
            validate_numeric(parameter, value)?;
            PropertyValue::Int(value)
        }
        RichTextValueKind::FixedMilli | RichTextValueKind::Angle => {
            let Some(Scalar::Int(value)) = value.scalar() else {
                return Err(invalid());
            };
            let Some(value) = value.exact_i32() else {
                return Err(invalid());
            };
            validate_numeric(parameter, i64::from(value))?;
            if parameter.kind == RichTextValueKind::FixedMilli {
                PropertyValue::Milli(value)
            } else {
                PropertyValue::Angle(value)
            }
        }
        RichTextValueKind::Ratio => {
            let Some(Scalar::UInt(RuntimeUInt::U16(value))) = value.scalar() else {
                return Err(invalid());
            };
            validate_numeric(parameter, i64::from(value))?;
            PropertyValue::Ratio(value)
        }
        RichTextValueKind::Length => {
            let Some(fields) = value.tuple().filter(|fields| fields.len() == 2) else {
                return Err(invalid());
            };
            let [
                Some(Scalar::Int(milli)),
                Some(Scalar::UInt(RuntimeUInt::U8(unit))),
            ] = [
                fields.get(0).and_then(RuntimeBorrowedValue::scalar),
                fields.get(1).and_then(RuntimeBorrowedValue::scalar),
            ]
            else {
                return Err(invalid());
            };
            let Some(milli) = milli.exact_i32() else {
                return Err(invalid());
            };
            let Some(unit) = rich_text_unit(unit) else {
                return Err(invalid());
            };
            if !parameter.limits.units.contains(&unit) {
                return Err(invalid());
            }
            validate_numeric(parameter, i64::from(milli))?;
            PropertyValue::Length { milli, unit }
        }
        RichTextValueKind::Duration => {
            let Some(Scalar::UInt(RuntimeUInt::U64(value))) = value.scalar() else {
                return Err(invalid());
            };
            i64::try_from(value)
                .ok()
                .map(|value| validate_numeric(parameter, value))
                .transpose()?;
            PropertyValue::Duration(value)
        }
        RichTextValueKind::ClosedEnum(domain) => {
            let Some(Scalar::UInt(RuntimeUInt::U16(variant))) = value.scalar() else {
                return Err(invalid());
            };
            if !enum_variant_is_accepted(parameter, domain, variant) {
                return Err(invalid());
            }
            PropertyValue::ClosedEnum { domain, variant }
        }
        RichTextValueKind::Selector(SelectorKind::PublicId) | RichTextValueKind::PublicId => {
            let Some(Scalar::String(value)) = value.scalar() else {
                return Err(invalid());
            };
            validate_text(parameter, value)?;
            PublicId::validate_text(value).map_err(|_| invalid())?;
            PropertyValue::PublicId(value)
        }
        RichTextValueKind::Text => {
            let Some(Scalar::String(value)) = value.scalar() else {
                return Err(invalid());
            };
            validate_text(parameter, value)?;
            PropertyValue::Text(value)
        }
        RichTextValueKind::Color => PropertyValue::Color(decode_color(parameter, value)?),
        RichTextValueKind::Vec2 => {
            let Some(fields) = value.tuple().filter(|fields| fields.len() == 2) else {
                return Err(invalid());
            };
            let [Some(Scalar::Int(x)), Some(Scalar::Int(y))] = [
                fields.get(0).and_then(RuntimeBorrowedValue::scalar),
                fields.get(1).and_then(RuntimeBorrowedValue::scalar),
            ] else {
                return Err(invalid());
            };
            let (Some(x_milli), Some(y_milli)) = (x.exact_i32(), y.exact_i32()) else {
                return Err(invalid());
            };
            validate_numeric(parameter, i64::from(x_milli))?;
            validate_numeric(parameter, i64::from(y_milli))?;
            PropertyValue::Vec2 { x_milli, y_milli }
        }
        RichTextValueKind::Seed32 => {
            let Some(Scalar::UInt(RuntimeUInt::U32(value))) = value.scalar() else {
                return Err(invalid());
            };
            PropertyValue::Seed32(value)
        }
        RichTextValueKind::Selector(SelectorKind::Closed) | RichTextValueKind::Fx => {
            return Err(invalid());
        }
    })
}

fn decode_color(
    parameter: arcweft_presentation::rich_text::PresentationContentCallableParameterSpec,
    value: RuntimeBorrowedValue<'_>,
) -> Result<CharacterDialogueRichTextColor<&str>, CharacterDialogueValueError> {
    let invalid = || schema_error("Color value differs from its presentation schema");
    if let Some(RuntimeScalarView::String(value)) = value.scalar() {
        validate_text(parameter, value)?;
        PublicId::validate_text(value).map_err(|_| invalid())?;
        return Ok(CharacterDialogueRichTextColor::Resource(value));
    }
    let Some(fields) = value.tuple().filter(|fields| fields.len() == 4) else {
        return Err(invalid());
    };
    let channels =
        std::array::from_fn(|index| fields.get(index).and_then(RuntimeBorrowedValue::scalar));
    let [
        Some(RuntimeScalarView::UInt(RuntimeUInt::U8(r))),
        Some(RuntimeScalarView::UInt(RuntimeUInt::U8(g))),
        Some(RuntimeScalarView::UInt(RuntimeUInt::U8(b))),
        Some(RuntimeScalarView::UInt(RuntimeUInt::U8(a))),
    ] = channels
    else {
        return Err(invalid());
    };
    Ok(CharacterDialogueRichTextColor::Rgba8([r, g, b, a]))
}

impl CharacterDialogueRichTextPropertyValue<&str, &str> {
    fn into_owned(
        self,
    ) -> Result<CharacterDialogueRichTextPropertyValue, Box<CharacterDialogueValueError>> {
        use CharacterDialogueRichTextPropertyValue as Value;
        Ok(match self {
            Self::Bool(value) => Value::Bool(value),
            Self::Int(value) => Value::Int(value),
            Self::Milli(value) => Value::Milli(value),
            Self::Ratio(value) => Value::Ratio(value),
            Self::Length { milli, unit } => Value::Length { milli, unit },
            Self::Angle(value) => Value::Angle(value),
            Self::Duration(value) => Value::Duration(value),
            Self::ClosedEnum { domain, variant } => Value::ClosedEnum { domain, variant },
            Self::PublicId(value) => Value::PublicId(
                PublicId::try_new(value)
                    .map_err(|_| schema_error("invalid admitted public identity"))?,
            ),
            Self::Text(value) => Value::Text(value.to_owned()),
            Self::Color(CharacterDialogueRichTextColor::Rgba8(value)) => {
                Value::Color(CharacterDialogueRichTextColor::Rgba8(value))
            }
            Self::Color(CharacterDialogueRichTextColor::Resource(value)) => {
                Value::Color(CharacterDialogueRichTextColor::Resource(
                    PublicId::try_new(value)
                        .map_err(|_| schema_error("invalid admitted color identity"))?,
                ))
            }
            Self::Vec2 { x_milli, y_milli } => Value::Vec2 { x_milli, y_milli },
            Self::Seed32(value) => Value::Seed32(value),
        })
    }
}

fn validate_text(
    parameter: arcweft_presentation::rich_text::PresentationContentCallableParameterSpec,
    value: &str,
) -> Result<(), CharacterDialogueValueError> {
    if value.is_empty() && !parameter.allow_empty {
        return Err(schema_error("empty text value is not accepted"));
    }
    if value.len() > usize::from(parameter.limits.max_decoded_bytes) {
        return Err(CharacterDialogueValueError::Limit {
            limit: "rich_text_property_bytes",
            maximum: usize::from(parameter.limits.max_decoded_bytes),
        });
    }
    Ok(())
}

fn validate_numeric(
    parameter: arcweft_presentation::rich_text::PresentationContentCallableParameterSpec,
    value: i64,
) -> Result<(), CharacterDialogueValueError> {
    if parameter.limits.numeric.is_some_and(|limits| {
        limits
            .inclusive_min_milli
            .is_some_and(|minimum| value < minimum)
            || limits
                .inclusive_max_milli
                .is_some_and(|maximum| value > maximum)
    }) {
        return Err(schema_error("numeric value is outside presentation limits"));
    }
    Ok(())
}

fn enum_variant_is_accepted(
    parameter: arcweft_presentation::rich_text::PresentationContentCallableParameterSpec,
    domain: ClosedEnumDomainId,
    variant: u16,
) -> bool {
    let domain_has_variant =
        closed_enum_domain_len(domain).is_some_and(|count| usize::from(variant) < count);
    domain_has_variant
        && match parameter.enum_constraint {
            RichTextEnumValueConstraint::All => true,
            RichTextEnumValueConstraint::Exact(expected) => variant == expected,
            RichTextEnumValueConstraint::Allowed(allowed) => allowed.contains(&variant),
        }
}

fn closed_enum_domain_len(domain: ClosedEnumDomainId) -> Option<usize> {
    let counts = [
        (
            RichTextEnumDomain::StyleSelector.domain_id(),
            RichTextStyleSelector::ALL.len(),
        ),
        (
            RichTextEnumDomain::LayoutSelector.domain_id(),
            RichTextLayoutSelector::ALL.len(),
        ),
        (
            RichTextEnumDomain::TransformSelector.domain_id(),
            RichTextTransformSelector::ALL.len(),
        ),
        (
            RichTextEnumDomain::LayoutDirection.domain_id(),
            LayoutDirection::ALL.len(),
        ),
        (
            RichTextEnumDomain::VerticalLatin.domain_id(),
            VerticalLatin::ALL.len(),
        ),
        (RichTextEnumDomain::Jlreq.domain_id(), Jlreq::ALL.len()),
        (
            RichTextEnumDomain::TransformTarget.domain_id(),
            TransformTarget::ALL.len(),
        ),
        (
            RichTextEnumDomain::TransformOrigin.domain_id(),
            TransformOrigin::ALL.len(),
        ),
    ];
    counts
        .into_iter()
        .find_map(|(candidate, count)| (candidate == domain).then_some(count))
}

fn rich_text_unit(value: u8) -> Option<RichTextUnit> {
    Some(match value {
        0 => RichTextUnit::Unitless,
        1 => RichTextUnit::Px,
        2 => RichTextUnit::Pt,
        3 => RichTextUnit::Ch,
        4 => RichTextUnit::Em,
        5 => RichTextUnit::Deg,
        6 => RichTextUnit::Ms,
        7 => RichTextUnit::S,
        8 => RichTextUnit::Cps,
        _ => return None,
    })
}

fn schema_error(reason: &'static str) -> CharacterDialogueValueError {
    CharacterDialogueValueError::Field {
        field: "rich_text",
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::plan::RuntimePlanTypeProjection;
    use arcweft_core::value::RuntimeInt;
    use arcweft_presentation::rich_text::RichTextLayoutProperty;

    fn schema() -> &'static CharacterDialogueRolePayloadSchema {
        CharacterDialogueRolePayloadCodec::RichTextProperties
            .payload_schema()
            .expect("accepted presentation catalog builds its RichText codec")
    }

    #[test]
    fn sparse_schema_has_a_complete_option_graph_and_typed_empty_value() {
        let schema = schema();
        let RuntimeCheckedType::Tuple(slots) = schema.checked_type(schema.root()).unwrap() else {
            panic!("sparse policy root is a tuple")
        };
        let RuntimeValue::Tuple(empty) = schema.no_overrides_payload.clone() else {
            panic!("no-overrides value preserves the typed tuple")
        };
        assert!(!slots.is_empty());
        assert_eq!(slots.len(), schema.fields.len());
        assert_eq!(empty.len(), slots.len());
        assert!(empty.iter().all(|slot| matches!(
            slot.builtin_variant_case(),
            Some((RuntimeBuiltinVariantCaseIdentity::OptionNone, None))
        )));

        for (slot_type, field) in slots.iter().zip(&schema.fields) {
            let RuntimeCheckedType::Option(item) = slot_type else {
                panic!("every property slot is optional")
            };
            assert_eq!(item.as_ref(), &field.value_type);
            let option_seed = schema
                .types()
                .iter()
                .find(|seed| seed.semantic_identity() == slot_type.semantic_identity_digest())
                .expect("Option type has a Core seed");
            let RuntimePlanTypeProjection::Option { item, some_payload } = option_seed.projection()
            else {
                panic!("Option seed retains both Core child references")
            };
            assert_eq!(*item, field.value_type.semantic_identity_digest());
            let payload = RuntimeCheckedType::Tuple(vec![field.value_type.clone()]);
            assert_eq!(*some_payload, payload.semantic_identity_digest());
            assert!(schema.checked_type(*item).is_some());
            assert!(schema.checked_type(*some_payload).is_some());
        }
    }

    #[test]
    fn rich_text_decoder_uses_catalog_coordinates_and_rejects_tampering() {
        let schema = schema();
        let (index, field) = schema
            .fields
            .iter()
            .enumerate()
            .find(|(_, field)| {
                field.definition == PresentationContentCallableDefinitionId::Color
                    && field.parameter.id == PresentationContentCallableParameterId::Value
            })
            .expect("presentation catalog exposes the Color value property");
        let rgba = RuntimeValue::Tuple(vec![
            RuntimeValue::UInt(RuntimeUInt::U8(1)),
            RuntimeValue::UInt(RuntimeUInt::U8(2)),
            RuntimeValue::UInt(RuntimeUInt::U8(3)),
            RuntimeValue::UInt(RuntimeUInt::U8(4)),
        ]);
        let mut payload = schema.no_overrides_payload.clone();
        let RuntimeValue::Tuple(slots) = &mut payload else {
            panic!("sparse policy root is a tuple")
        };
        slots[index] = RuntimeValue::option_some(rgba);

        let decoded = schema.decode_properties(&payload).unwrap();
        let snapshot =
            arcweft_core::value::AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(
                &payload,
            )
            .unwrap();
        let codec = CharacterDialogueRolePayloadCodec::RichTextProperties;
        codec.validate_properties(&payload).unwrap();
        codec.validate_properties(&snapshot).unwrap();
        assert_eq!(codec.decode_properties(&snapshot).unwrap(), decoded);
        assert_eq!(decoded.len(), 1);
        let property = decoded.iter().next().expect("one decoded property");
        assert_eq!(property.definition(), field.definition);
        assert_eq!(property.parameter(), field.parameter.id);
        assert_eq!(
            property.value(),
            &CharacterDialogueRichTextPropertyValue::Color(CharacterDialogueRichTextColor::Rgba8(
                [1, 2, 3, 4]
            ))
        );

        let mut tampered = payload.clone();
        let RuntimeValue::Tuple(slots) = &mut tampered else {
            unreachable!()
        };
        slots[index] = RuntimeValue::option_some(RuntimeValue::Bool(true));
        assert!(schema.decode_properties(&tampered).is_err());
        let tampered_snapshot =
            arcweft_core::value::AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(
                &tampered,
            )
            .unwrap();
        assert!(codec.validate_properties(&tampered_snapshot).is_err());
        assert!(codec.decode_properties(&tampered_snapshot).is_err());

        let mut truncated = payload;
        let RuntimeValue::Tuple(slots) = &mut truncated else {
            unreachable!()
        };
        slots.pop();
        assert!(schema.decode_properties(&truncated).is_err());
    }

    #[test]
    fn rich_text_borrowed_codec_rejects_domain_invalid_inert_fields() {
        let schema = schema();
        let codec = CharacterDialogueRolePayloadCodec::RichTextProperties;
        for kind in [
            RichTextValueKind::ClosedEnum(RichTextEnumDomain::LayoutDirection.domain_id()),
            RichTextValueKind::Color,
        ] {
            let index = schema
                .fields
                .iter()
                .rposition(|field| field.parameter.kind == kind)
                .expect("reusable Style contains the domain field");
            let mut payload = schema.no_overrides_payload.clone();
            let RuntimeValue::Tuple(slots) = &mut payload else {
                unreachable!()
            };
            slots[index] = RuntimeValue::option_some(match kind {
                RichTextValueKind::ClosedEnum(_) => RuntimeValue::UInt(RuntimeUInt::U16(u16::MAX)),
                RichTextValueKind::Color => RuntimeValue::String("@reserved".to_owned()),
                _ => unreachable!(),
            });
            let snapshot =
                arcweft_core::value::AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(
                    &payload,
                )
                .unwrap();
            assert!(codec.validate_properties(&payload).is_err());
            assert!(codec.validate_properties(&snapshot).is_err());
            assert!(codec.decode_properties(&snapshot).is_err());
        }
    }

    #[test]
    fn rich_text_configuration_rejects_content_reading_slots() {
        let schema = schema();
        assert!(
            schema
                .fields
                .iter()
                .all(|field| { field.definition != PresentationContentCallableDefinitionId::Ruby })
        );
        let content_parameters = PRESENTATION_CONTENT_CALLABLE_CATALOG
            .iter()
            .flat_map(|definition| {
                definition.parameters().iter().filter_map(move |parameter| {
                    (parameter.id != PresentationContentCallableParameterId::Fx)
                        .then_some((definition.id(), parameter.id))
                })
            })
            .collect::<Vec<_>>();
        let reading = content_parameters
            .iter()
            .position(|coordinate| {
                *coordinate
                    == (
                        PresentationContentCallableDefinitionId::Ruby,
                        PresentationContentCallableParameterId::Value,
                    )
            })
            .expect("the full Content catalog retains the Ruby reading argument");
        let mut slots = vec![RuntimeValue::option_none(); content_parameters.len()];
        slots[reading] = RuntimeValue::option_some(RuntimeValue::String("よみ".to_owned()));
        assert!(matches!(
            schema.decode_properties(&RuntimeValue::Tuple(slots)),
            Err(CharacterDialogueValueError::Field {
                field: "rich_text",
                ..
            })
        ));
    }

    #[test]
    fn rich_text_configuration_preserves_ruby_typography_properties() {
        let schema = schema();
        let mut payload = schema.no_overrides_payload.clone();
        let RuntimeValue::Tuple(slots) = &mut payload else {
            panic!("sparse policy root is a tuple")
        };
        let mut expected = Vec::new();
        for selector in [
            RichTextLayoutSelector::RubyOver,
            RichTextLayoutSelector::RubyUnder,
            RichTextLayoutSelector::RubyInterCharacter,
        ] {
            for property in [
                RichTextLayoutProperty::RubySize,
                RichTextLayoutProperty::RubyGap,
                RichTextLayoutProperty::RubyOverhang,
                RichTextLayoutProperty::RubyCollisionGap,
            ] {
                let definition = PresentationContentCallableDefinitionId::Layout(selector);
                let parameter = PresentationContentCallableParameterId::Layout(property);
                let index = schema
                    .fields
                    .iter()
                    .position(|field| {
                        field.definition == definition && field.parameter.id == parameter
                    })
                    .expect("Ruby typography remains an accepted reusable style coordinate");
                slots[index] = RuntimeValue::option_some(RuntimeValue::Tuple(vec![
                    RuntimeValue::Int(RuntimeInt::I32(1_000)),
                    RuntimeValue::UInt(RuntimeUInt::U8(1)),
                ]));
                expected.push((definition, parameter));
            }
        }
        let decoded = schema.decode_properties(&payload).unwrap();
        assert_eq!(decoded.len(), expected.len());
        for property in decoded.iter() {
            assert!(expected.contains(&(property.definition(), property.parameter())));
            assert_eq!(
                property.value(),
                &CharacterDialogueRichTextPropertyValue::Length {
                    milli: 1_000,
                    unit: RichTextUnit::Px,
                }
            );
        }
    }
}
