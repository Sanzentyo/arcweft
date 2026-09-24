//! Projection of admitted `CharacterDialogue` `RichText` properties into the
//! renderer-neutral text style algebra.

use crate::{
    Milli, RichTextAngle, RichTextColor, RichTextFontFamily, RichTextInlineDirection,
    RichTextJlreqStrictness, RichTextLayout, RichTextPresentationStyle, RichTextRubyPosition,
    RichTextStyle, RichTextTransform, RichTextTransformOrigin, RichTextVerticalLatinMode,
    RichTextWritingMode,
};
use arcweft_dialogue::{
    CharacterDialogueRichTextColor, CharacterDialogueRichTextProperties,
    CharacterDialogueRichTextProperty, CharacterDialogueRichTextPropertyValue,
};
use arcweft_presentation::{
    fx::FxTarget,
    rich_text::{
        Jlreq, LayoutDirection, PRESENTATION_CONTENT_CALLABLE_CATALOG,
        PresentationContentCallableDefinitionId as Definition,
        PresentationContentCallableParameterId as Parameter, RichTextLayoutProperty,
        RichTextLayoutSelector, RichTextStyleProperty, RichTextStyleSelector,
        RichTextTransformProperty, RichTextTransformSelector,
        TransformOrigin as PresentationTransformOrigin,
        TransformTarget as PresentationTransformTarget, VerticalLatin,
    },
};
use thiserror::Error;

/// Failure to project a typed Dialogue property group into text-model styles.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CharacterDialogueRichTextProjectionError {
    /// The coordinate carries content or a representation with no style
    /// equivalent in the text-model algebra.
    #[error("unsupported CharacterDialogue RichText coordinate {definition:?}/{parameter:?}")]
    UnsupportedCoordinate {
        definition: Definition,
        parameter: Parameter,
    },
    /// The coordinate is known, but its typed value or group shape is invalid.
    #[error("invalid CharacterDialogue RichText value at {definition:?}/{parameter:?}")]
    InvalidValue {
        definition: Definition,
        parameter: Parameter,
    },
    /// A closed enum value belongs to a different owner domain.
    #[error("closed enum domain mismatch at {definition:?}/{parameter:?}")]
    ClosedEnumDomainMismatch {
        definition: Definition,
        parameter: Parameter,
        expected: arcweft_id::closed_enum::ClosedEnumDomainId,
        actual: arcweft_id::closed_enum::ClosedEnumDomainId,
    },
}

/// Projects an admitted sparse Dialogue `RichText` policy into ordered text
/// styles. The Dialogue decoder supplies catalog-ordered typed properties;
/// this projection retains that order across emitted style rows.
pub fn project_character_dialogue_rich_text_properties(
    properties: &CharacterDialogueRichTextProperties,
) -> Result<Vec<RichTextStyle>, CharacterDialogueRichTextProjectionError> {
    let mut output = Vec::new();
    let mut current_definition = None;
    let mut current_properties = Vec::new();

    for property in properties.iter() {
        let definition = property.definition();
        if let Some(current) = current_definition.filter(|current| *current != definition) {
            output.push(project_definition(current, &current_properties)?);
            current_properties.clear();
        }
        current_definition = Some(definition);
        current_properties.push(property);
    }

    if let Some(definition) = current_definition {
        output.push(project_definition(definition, &current_properties)?);
    }

    Ok(output)
}

fn project_definition(
    definition: Definition,
    properties: &[&CharacterDialogueRichTextProperty],
) -> Result<RichTextStyle, CharacterDialogueRichTextProjectionError> {
    match definition {
        Definition::Color => {
            require_only(properties, &[Parameter::Value])?;
            let value = required_value(definition, properties, Parameter::Value)?;
            let CharacterDialogueRichTextPropertyValue::Color(color) = value else {
                return Err(invalid(definition, Parameter::Value));
            };
            Ok(RichTextStyle::Color {
                value: match color {
                    CharacterDialogueRichTextColor::Rgba8(value) => {
                        RichTextColor::Rgba8 { value: *value }
                    }
                    CharacterDialogueRichTextColor::Resource(id) => RichTextColor::Resource {
                        id: id.as_str().to_owned(),
                    },
                },
            })
        }
        Definition::Font => {
            require_only(properties, &[Parameter::Value])?;
            let value = required_value(definition, properties, Parameter::Value)?;
            let CharacterDialogueRichTextPropertyValue::Text(name) = value else {
                return Err(invalid(definition, Parameter::Value));
            };
            Ok(RichTextStyle::Font {
                family: RichTextFontFamily::Named { name: name.clone() },
            })
        }
        Definition::Size => {
            require_only(properties, &[Parameter::Value])?;
            let value = required_value(definition, properties, Parameter::Value)?;
            let CharacterDialogueRichTextPropertyValue::Length { milli, unit } = value else {
                return Err(invalid(definition, Parameter::Value));
            };
            validate_length_unit(definition, Parameter::Value, *unit)?;
            Ok(RichTextStyle::Size {
                milli_points: Milli(*milli),
            })
        }
        Definition::Style(selector) => project_style(definition, selector, properties),
        Definition::Layout(selector) => project_layout(definition, selector, properties),
        Definition::Transform(selector) => project_transform(definition, selector, properties),
        Definition::Ruby => Err(unsupported(definition, Parameter::Value)),
        Definition::Fx => Err(unsupported(definition, Parameter::Fx)),
        Definition::Strong | Definition::Em | Definition::Raw => Err(unsupported(
            definition,
            properties
                .first()
                .map_or(Parameter::Value, |property| property.parameter()),
        )),
    }
}

fn project_style(
    definition: Definition,
    selector: RichTextStyleSelector,
    properties: &[&CharacterDialogueRichTextProperty],
) -> Result<RichTextStyle, CharacterDialogueRichTextProjectionError> {
    let selector_parameter = Parameter::Selector;
    let selector_value = match selector {
        RichTextStyleSelector::Italic => None,
        RichTextStyleSelector::Oblique => Some(Parameter::Style(RichTextStyleProperty::Angle)),
        RichTextStyleSelector::Opacity => Some(Parameter::Style(RichTextStyleProperty::Opacity)),
        RichTextStyleSelector::Layer => Some(Parameter::Style(RichTextStyleProperty::Layer)),
        RichTextStyleSelector::ZIndex => Some(Parameter::Style(RichTextStyleProperty::ZIndex)),
    };
    let allowed = selector_value.map_or_else(
        || vec![selector_parameter],
        |parameter| vec![selector_parameter, parameter],
    );
    require_only(properties, &allowed)?;
    require_selector(
        definition,
        properties,
        selector_parameter,
        selector.schema_id(),
        selector.ordinal(),
    )?;

    match selector {
        RichTextStyleSelector::Italic => Ok(RichTextStyle::Italic),
        RichTextStyleSelector::Oblique => {
            let angle_parameter = Parameter::Style(RichTextStyleProperty::Angle);
            let angle = optional_value(properties, angle_parameter)
                .map(|value| match value {
                    CharacterDialogueRichTextPropertyValue::Angle(value) => Ok(*value),
                    _ => Err(invalid(definition, angle_parameter)),
                })
                .transpose()?
                .unwrap_or(0);
            Ok(RichTextStyle::Oblique {
                angle: RichTextAngle {
                    degrees: Milli(angle),
                },
            })
        }
        RichTextStyleSelector::Opacity => {
            let parameter = Parameter::Style(RichTextStyleProperty::Opacity);
            let value = required_value(definition, properties, parameter)?;
            let CharacterDialogueRichTextPropertyValue::Ratio(value) = value else {
                return Err(invalid(definition, parameter));
            };
            Ok(RichTextStyle::Presentation {
                presentation: RichTextPresentationStyle {
                    opacity: Some(Milli(i32::from(*value))),
                    ..RichTextPresentationStyle::default()
                },
            })
        }
        RichTextStyleSelector::Layer => {
            let parameter = Parameter::Style(RichTextStyleProperty::Layer);
            let value = required_value(definition, properties, parameter)?;
            let CharacterDialogueRichTextPropertyValue::PublicId(value) = value else {
                return Err(invalid(definition, parameter));
            };
            Ok(RichTextStyle::Presentation {
                presentation: RichTextPresentationStyle {
                    layer: Some(value.as_str().to_owned()),
                    ..RichTextPresentationStyle::default()
                },
            })
        }
        RichTextStyleSelector::ZIndex => {
            let parameter = Parameter::Style(RichTextStyleProperty::ZIndex);
            let value = required_value(definition, properties, parameter)?;
            let CharacterDialogueRichTextPropertyValue::Int(value) = value else {
                return Err(invalid(definition, parameter));
            };
            let Ok(value) = i16::try_from(*value) else {
                return Err(invalid(definition, parameter));
            };
            Ok(RichTextStyle::Presentation {
                presentation: RichTextPresentationStyle {
                    z_index: Some(value),
                    ..RichTextPresentationStyle::default()
                },
            })
        }
    }
}

fn project_layout(
    definition: Definition,
    selector: RichTextLayoutSelector,
    properties: &[&CharacterDialogueRichTextProperty],
) -> Result<RichTextStyle, CharacterDialogueRichTextProjectionError> {
    let mut allowed = vec![Parameter::Selector];
    allowed.extend(RichTextLayoutProperty::ALL.map(Parameter::Layout));
    require_only(properties, &allowed)?;
    require_selector(
        definition,
        properties,
        Parameter::Selector,
        selector.schema_id(),
        selector.ordinal(),
    )?;

    let mut layout = RichTextLayout::default();
    match selector {
        RichTextLayoutSelector::HorizontalTb => {
            layout.writing_mode = RichTextWritingMode::HorizontalTb;
        }
        RichTextLayoutSelector::VerticalRl => {
            layout.writing_mode = RichTextWritingMode::VerticalRl;
        }
        RichTextLayoutSelector::VerticalLr => {
            layout.writing_mode = RichTextWritingMode::VerticalLr;
        }
        RichTextLayoutSelector::Direction => {
            let parameter = Parameter::Layout(RichTextLayoutProperty::Direction);
            layout.direction = layout_direction(
                definition,
                parameter,
                required_value(definition, properties, parameter)?,
            )?;
        }
        RichTextLayoutSelector::RubyOver => layout.ruby_position = RichTextRubyPosition::Over,
        RichTextLayoutSelector::RubyUnder => layout.ruby_position = RichTextRubyPosition::Under,
        RichTextLayoutSelector::RubyInterCharacter => {
            layout.ruby_position = RichTextRubyPosition::InterCharacter;
        }
    }

    for property in properties {
        match property.parameter() {
            Parameter::Selector => {}
            Parameter::Layout(RichTextLayoutProperty::Direction) => {
                layout.direction =
                    layout_direction(definition, property.parameter(), property.value())?;
            }
            Parameter::Layout(RichTextLayoutProperty::Latin) => {
                layout.vertical_latin =
                    vertical_latin(definition, property.parameter(), property.value())?;
            }
            Parameter::Layout(RichTextLayoutProperty::Jlreq) => {
                layout.jlreq_strictness =
                    jlreq(definition, property.parameter(), property.value())?;
            }
            Parameter::Layout(RichTextLayoutProperty::ColumnGap) => {
                layout.column_gap = Milli(length_milli(
                    definition,
                    property.parameter(),
                    property.value(),
                )?);
            }
            Parameter::Layout(RichTextLayoutProperty::RubySize) => {
                layout.ruby_font_size = Some(Milli(length_milli(
                    definition,
                    property.parameter(),
                    property.value(),
                )?));
            }
            Parameter::Layout(RichTextLayoutProperty::RubyGap) => {
                layout.ruby_gap = Some(Milli(length_milli(
                    definition,
                    property.parameter(),
                    property.value(),
                )?));
            }
            Parameter::Layout(RichTextLayoutProperty::RubyOverhang) => {
                layout.ruby_overhang = Some(Milli(length_milli(
                    definition,
                    property.parameter(),
                    property.value(),
                )?));
            }
            Parameter::Layout(RichTextLayoutProperty::RubyCollisionGap) => {
                layout.ruby_collision_gap = Some(Milli(length_milli(
                    definition,
                    property.parameter(),
                    property.value(),
                )?));
            }
            parameter => return Err(unsupported(definition, parameter)),
        }
    }

    Ok(RichTextStyle::Layout { layout })
}

fn project_transform(
    definition: Definition,
    selector: RichTextTransformSelector,
    properties: &[&CharacterDialogueRichTextProperty],
) -> Result<RichTextStyle, CharacterDialogueRichTextProjectionError> {
    let mut allowed = vec![Parameter::Selector];
    let selector_properties: &[RichTextTransformProperty] = match selector {
        RichTextTransformSelector::Rotate => &[
            RichTextTransformProperty::Angle,
            RichTextTransformProperty::Target,
            RichTextTransformProperty::Origin,
        ],
        RichTextTransformSelector::Offset
        | RichTextTransformSelector::Scale
        | RichTextTransformSelector::Skew => &[
            RichTextTransformProperty::X,
            RichTextTransformProperty::Y,
            RichTextTransformProperty::Target,
            RichTextTransformProperty::Origin,
        ],
    };
    allowed.extend(
        selector_properties
            .iter()
            .copied()
            .map(Parameter::Transform),
    );
    require_only(properties, &allowed)?;
    require_selector(
        definition,
        properties,
        Parameter::Selector,
        selector.schema_id(),
        selector.ordinal(),
    )?;

    let mut transform = RichTextTransform::default();
    if matches!(
        selector,
        RichTextTransformSelector::Rotate | RichTextTransformSelector::Scale
    ) {
        transform.origin = RichTextTransformOrigin::Center;
    }

    for property in properties {
        apply_transform_property(definition, selector, property, &mut transform)?;
    }

    Ok(RichTextStyle::Transform { transform })
}

fn apply_transform_property(
    definition: Definition,
    selector: RichTextTransformSelector,
    property: &CharacterDialogueRichTextProperty,
    transform: &mut RichTextTransform,
) -> Result<(), CharacterDialogueRichTextProjectionError> {
    let parameter = property.parameter();
    let value = property.value();
    match parameter {
        Parameter::Selector => {}
        Parameter::Transform(RichTextTransformProperty::X) => match selector {
            RichTextTransformSelector::Offset => {
                transform.translate.x = Milli(length_milli(definition, parameter, value)?);
            }
            RichTextTransformSelector::Scale => {
                transform.scale.x = Milli(milli(definition, parameter, value)?);
            }
            RichTextTransformSelector::Skew => {
                transform.skew.x = Milli(angle(definition, parameter, value)?);
            }
            RichTextTransformSelector::Rotate => {
                return Err(unsupported(definition, parameter));
            }
        },
        Parameter::Transform(RichTextTransformProperty::Y) => match selector {
            RichTextTransformSelector::Offset => {
                transform.translate.y = Milli(length_milli(definition, parameter, value)?);
            }
            RichTextTransformSelector::Scale => {
                transform.scale.y = Milli(milli(definition, parameter, value)?);
            }
            RichTextTransformSelector::Skew => {
                transform.skew.y = Milli(angle(definition, parameter, value)?);
            }
            RichTextTransformSelector::Rotate => {
                return Err(unsupported(definition, parameter));
            }
        },
        Parameter::Transform(RichTextTransformProperty::Angle) => {
            if selector != RichTextTransformSelector::Rotate {
                return Err(unsupported(definition, parameter));
            }
            transform.rotate = RichTextAngle {
                degrees: Milli(angle(definition, parameter, value)?),
            };
        }
        Parameter::Transform(RichTextTransformProperty::Target) => {
            transform.target = transform_target(definition, parameter, value)?;
        }
        Parameter::Transform(RichTextTransformProperty::Origin) => {
            transform.origin = transform_origin(definition, parameter, value)?;
        }
        parameter => return Err(unsupported(definition, parameter)),
    }
    Ok(())
}

fn require_selector(
    definition: Definition,
    properties: &[&CharacterDialogueRichTextProperty],
    parameter: Parameter,
    expected_domain: arcweft_id::closed_enum::ClosedEnumDomainId,
    expected_variant: u16,
) -> Result<(), CharacterDialogueRichTextProjectionError> {
    let value = required_value(definition, properties, parameter)?;
    let CharacterDialogueRichTextPropertyValue::ClosedEnum { domain, variant } = value else {
        return Err(invalid(definition, parameter));
    };
    if *domain != expected_domain {
        return Err(
            CharacterDialogueRichTextProjectionError::ClosedEnumDomainMismatch {
                definition,
                parameter,
                expected: expected_domain,
                actual: *domain,
            },
        );
    }
    if *variant != expected_variant {
        return Err(invalid(definition, parameter));
    }
    Ok(())
}

fn layout_direction(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<RichTextInlineDirection, CharacterDialogueRichTextProjectionError> {
    let variant = closed_enum_variant(
        definition,
        parameter,
        value,
        LayoutDirection::Auto.schema_id(),
    )?;
    Ok(match variant {
        0 => RichTextInlineDirection::Auto,
        1 => RichTextInlineDirection::Ltr,
        2 => RichTextInlineDirection::Rtl,
        _ => return Err(invalid(definition, parameter)),
    })
}

fn vertical_latin(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<RichTextVerticalLatinMode, CharacterDialogueRichTextProjectionError> {
    let variant = closed_enum_variant(
        definition,
        parameter,
        value,
        VerticalLatin::Mixed.schema_id(),
    )?;
    Ok(match variant {
        0 => RichTextVerticalLatinMode::Mixed,
        1 => RichTextVerticalLatinMode::Upright,
        2 => RichTextVerticalLatinMode::Sideways,
        _ => return Err(invalid(definition, parameter)),
    })
}

fn jlreq(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<RichTextJlreqStrictness, CharacterDialogueRichTextProjectionError> {
    let variant = closed_enum_variant(definition, parameter, value, Jlreq::Auto.schema_id())?;
    Ok(match variant {
        0 => RichTextJlreqStrictness::Auto,
        1 => RichTextJlreqStrictness::Loose,
        2 => RichTextJlreqStrictness::Normal,
        3 => RichTextJlreqStrictness::Strict,
        _ => return Err(invalid(definition, parameter)),
    })
}

fn transform_target(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<FxTarget, CharacterDialogueRichTextProjectionError> {
    let variant = closed_enum_variant(
        definition,
        parameter,
        value,
        PresentationTransformTarget::Content.schema_id(),
    )?;
    Ok(match variant {
        0 => FxTarget::Node,
        1 => FxTarget::Content,
        2 => FxTarget::Background,
        3 => FxTarget::Line,
        4 => FxTarget::Glyph,
        5 => FxTarget::Viewport,
        _ => return Err(invalid(definition, parameter)),
    })
}

fn transform_origin(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<RichTextTransformOrigin, CharacterDialogueRichTextProjectionError> {
    let variant = closed_enum_variant(
        definition,
        parameter,
        value,
        PresentationTransformOrigin::BaselineStart.schema_id(),
    )?;
    Ok(match variant {
        0 => RichTextTransformOrigin::BaselineStart,
        1 => RichTextTransformOrigin::BaselineCenter,
        2 => RichTextTransformOrigin::Center,
        3 => RichTextTransformOrigin::GlyphCenter,
        _ => return Err(invalid(definition, parameter)),
    })
}

fn closed_enum_variant(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
    expected_domain: arcweft_id::closed_enum::ClosedEnumDomainId,
) -> Result<u16, CharacterDialogueRichTextProjectionError> {
    let CharacterDialogueRichTextPropertyValue::ClosedEnum { domain, variant } = value else {
        return Err(invalid(definition, parameter));
    };
    if *domain != expected_domain {
        return Err(
            CharacterDialogueRichTextProjectionError::ClosedEnumDomainMismatch {
                definition,
                parameter,
                expected: expected_domain,
                actual: *domain,
            },
        );
    }
    Ok(*variant)
}

fn length_milli(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<i32, CharacterDialogueRichTextProjectionError> {
    let CharacterDialogueRichTextPropertyValue::Length { milli, unit } = value else {
        return Err(invalid(definition, parameter));
    };
    validate_length_unit(definition, parameter, *unit)?;
    Ok(*milli)
}

fn validate_length_unit(
    definition: Definition,
    parameter: Parameter,
    unit: arcweft_rich_text_schema::RichTextUnit,
) -> Result<(), CharacterDialogueRichTextProjectionError> {
    let accepted = PRESENTATION_CONTENT_CALLABLE_CATALOG
        .get(definition)
        .and_then(|row| {
            row.parameters()
                .iter()
                .find(|candidate| candidate.id == parameter)
        })
        .is_some_and(|spec| spec.limits.units.contains(&unit));
    if accepted {
        Ok(())
    } else {
        Err(invalid(definition, parameter))
    }
}

fn angle(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<i32, CharacterDialogueRichTextProjectionError> {
    let CharacterDialogueRichTextPropertyValue::Angle(value) = value else {
        return Err(invalid(definition, parameter));
    };
    Ok(*value)
}

fn milli(
    definition: Definition,
    parameter: Parameter,
    value: &CharacterDialogueRichTextPropertyValue,
) -> Result<i32, CharacterDialogueRichTextProjectionError> {
    let CharacterDialogueRichTextPropertyValue::Milli(value) = value else {
        return Err(invalid(definition, parameter));
    };
    Ok(*value)
}

fn require_only(
    properties: &[&CharacterDialogueRichTextProperty],
    allowed: &[Parameter],
) -> Result<(), CharacterDialogueRichTextProjectionError> {
    for property in properties {
        if !allowed.contains(&property.parameter()) {
            return Err(unsupported(property.definition(), property.parameter()));
        }
    }
    Ok(())
}

fn required_value<'a>(
    definition: Definition,
    properties: &[&'a CharacterDialogueRichTextProperty],
    parameter: Parameter,
) -> Result<&'a CharacterDialogueRichTextPropertyValue, CharacterDialogueRichTextProjectionError> {
    optional_value(properties, parameter).ok_or_else(|| invalid(definition, parameter))
}

fn optional_value<'a>(
    properties: &[&'a CharacterDialogueRichTextProperty],
    parameter: Parameter,
) -> Option<&'a CharacterDialogueRichTextPropertyValue> {
    properties
        .iter()
        .find(|property| property.parameter() == parameter)
        .map(|property| property.value())
}

const fn invalid(
    definition: Definition,
    parameter: Parameter,
) -> CharacterDialogueRichTextProjectionError {
    CharacterDialogueRichTextProjectionError::InvalidValue {
        definition,
        parameter,
    }
}

const fn unsupported(
    definition: Definition,
    parameter: Parameter,
) -> CharacterDialogueRichTextProjectionError {
    CharacterDialogueRichTextProjectionError::UnsupportedCoordinate {
        definition,
        parameter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::value::{RuntimeInt, RuntimeUInt, RuntimeValue};
    use arcweft_dialogue::CharacterDialogueRolePayloadCodec;

    fn policy_with(
        values: impl IntoIterator<Item = (Definition, Parameter, RuntimeValue)>,
    ) -> CharacterDialogueRichTextProperties {
        let mut payload = CharacterDialogueRolePayloadCodec::RichTextProperties
            .no_overrides_payload()
            .expect("RichText payload codec has a no-overrides value");
        let RuntimeValue::Tuple(slots) = &mut payload else {
            panic!("RichText policy root is a tuple")
        };

        for (definition, parameter, value) in values {
            let slot_index = PRESENTATION_CONTENT_CALLABLE_CATALOG
                .reusable_style_parameters()
                .map(|(definition, spec)| (definition, spec.id))
                .position(|coordinate| coordinate == (definition, parameter))
                .expect("typed coordinate exists in the presentation catalog");
            slots[slot_index] = RuntimeValue::option_some(value);
        }

        CharacterDialogueRolePayloadCodec::RichTextProperties
            .decode_properties(&payload)
            .expect("the payload uses the Dialogue-owned sparse policy codec")
    }

    fn closed_enum(variant: u16) -> RuntimeValue {
        RuntimeValue::UInt(RuntimeUInt::U16(variant))
    }

    fn int_i32(value: i32) -> RuntimeValue {
        RuntimeValue::Int(RuntimeInt::I32(value))
    }

    fn length(milli: i32, unit_ordinal: u8) -> RuntimeValue {
        RuntimeValue::Tuple(vec![
            int_i32(milli),
            RuntimeValue::UInt(RuntimeUInt::U8(unit_ordinal)),
        ])
    }

    fn representative_projection_properties() -> CharacterDialogueRichTextProperties {
        policy_with([
            (
                Definition::Color,
                Parameter::Value,
                RuntimeValue::Tuple(vec![
                    RuntimeValue::UInt(RuntimeUInt::U8(8)),
                    RuntimeValue::UInt(RuntimeUInt::U8(16)),
                    RuntimeValue::UInt(RuntimeUInt::U8(32)),
                    RuntimeValue::UInt(RuntimeUInt::U8(255)),
                ]),
            ),
            (
                Definition::Font,
                Parameter::Value,
                RuntimeValue::String("Yu Gothic".to_owned()),
            ),
            (Definition::Size, Parameter::Value, length(18_000, 2)),
            (
                Definition::Style(RichTextStyleSelector::Italic),
                Parameter::Selector,
                closed_enum(0),
            ),
            (
                Definition::Style(RichTextStyleSelector::Opacity),
                Parameter::Selector,
                closed_enum(2),
            ),
            (
                Definition::Style(RichTextStyleSelector::Opacity),
                Parameter::Style(RichTextStyleProperty::Opacity),
                RuntimeValue::UInt(RuntimeUInt::U16(625)),
            ),
            (
                Definition::Layout(RichTextLayoutSelector::VerticalRl),
                Parameter::Selector,
                closed_enum(1),
            ),
            (
                Definition::Layout(RichTextLayoutSelector::VerticalRl),
                Parameter::Layout(RichTextLayoutProperty::Latin),
                closed_enum(VerticalLatin::Upright.ordinal()),
            ),
            (
                Definition::Layout(RichTextLayoutSelector::VerticalRl),
                Parameter::Layout(RichTextLayoutProperty::ColumnGap),
                length(9_500, 1),
            ),
            (
                Definition::Transform(RichTextTransformSelector::Rotate),
                Parameter::Selector,
                closed_enum(1),
            ),
            (
                Definition::Transform(RichTextTransformSelector::Rotate),
                Parameter::Transform(RichTextTransformProperty::Angle),
                int_i32(45_000),
            ),
            (
                Definition::Transform(RichTextTransformSelector::Rotate),
                Parameter::Transform(RichTextTransformProperty::Target),
                closed_enum(PresentationTransformTarget::Glyph.ordinal()),
            ),
        ])
    }

    fn expected_representative_styles() -> Vec<RichTextStyle> {
        vec![
            RichTextStyle::Color {
                value: RichTextColor::Rgba8 {
                    value: [8, 16, 32, 255],
                },
            },
            RichTextStyle::Font {
                family: RichTextFontFamily::Named {
                    name: "Yu Gothic".to_owned(),
                },
            },
            RichTextStyle::Size {
                milli_points: Milli(18_000),
            },
            RichTextStyle::Italic,
            RichTextStyle::Presentation {
                presentation: RichTextPresentationStyle {
                    opacity: Some(Milli(625)),
                    ..RichTextPresentationStyle::default()
                },
            },
            RichTextStyle::Layout {
                layout: RichTextLayout {
                    writing_mode: RichTextWritingMode::VerticalRl,
                    vertical_latin: RichTextVerticalLatinMode::Upright,
                    column_gap: Milli(9_500),
                    ..RichTextLayout::default()
                },
            },
            RichTextStyle::Transform {
                transform: RichTextTransform {
                    rotate: RichTextAngle {
                        degrees: Milli(45_000),
                    },
                    origin: RichTextTransformOrigin::Center,
                    target: FxTarget::Glyph,
                    ..RichTextTransform::default()
                },
            },
        ]
    }

    #[test]
    fn projection_preserves_catalog_order_and_typed_style_values() {
        let properties = representative_projection_properties();
        let styles = project_character_dialogue_rich_text_properties(&properties).unwrap();
        assert_eq!(styles, expected_representative_styles());
    }

    #[test]
    fn projection_rejects_ruby_reading_text_as_content_not_style() {
        assert_eq!(
            project_definition(Definition::Ruby, &[]),
            Err(
                CharacterDialogueRichTextProjectionError::UnsupportedCoordinate {
                    definition: Definition::Ruby,
                    parameter: Parameter::Value,
                }
            )
        );
    }

    #[test]
    fn projection_rejects_an_incomplete_required_layout_direction() {
        let properties = policy_with([(
            Definition::Layout(RichTextLayoutSelector::Direction),
            Parameter::Selector,
            closed_enum(3),
        )]);

        assert_eq!(
            project_character_dialogue_rich_text_properties(&properties),
            Err(CharacterDialogueRichTextProjectionError::InvalidValue {
                definition: Definition::Layout(RichTextLayoutSelector::Direction),
                parameter: Parameter::Layout(RichTextLayoutProperty::Direction),
            })
        );
    }
}
