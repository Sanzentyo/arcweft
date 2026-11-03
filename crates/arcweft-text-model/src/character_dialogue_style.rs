//! Projects admitted `CharacterDialogue` `RichText` properties into text styles.

use crate::{
    Milli, RichTextAngle, RichTextColor, RichTextFontFamily, RichTextInlineDirection,
    RichTextJlreqStrictness, RichTextLayoutUpdate, RichTextPresentationStyle, RichTextRubyPosition,
    RichTextStyle, RichTextTransform, RichTextTransformOrigin, RichTextVerticalLatinMode,
    RichTextWritingMode,
};
use arcweft_dialogue::{
    CharacterDialogueRichTextColor, CharacterDialogueRichTextProperties,
    CharacterDialogueRichTextProperty, CharacterDialogueRichTextPropertyValue as Value,
};
use arcweft_presentation::{
    fx::FxTarget,
    rich_text::{
        Jlreq, LayoutDirection, PresentationReusableStylePropertyId as PropertyId,
        RichTextLayoutSelector, RichTextStyleSelector,
        TransformOrigin as PresentationTransformOrigin,
        TransformTarget as PresentationTransformTarget, VerticalLatin,
    },
};
use thiserror::Error;

/// Failure to project a typed Dialogue property into text-model styles.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CharacterDialogueRichTextProjectionError {
    /// The property has no matching value representation.
    #[error("invalid CharacterDialogue RichText value at {property:?}")]
    InvalidValue { property: PropertyId },
    /// A closed enum value belongs to a different owner domain.
    #[error("closed enum domain mismatch at {property:?}")]
    ClosedEnumDomainMismatch {
        property: PropertyId,
        expected: arcweft_id::closed_enum::ClosedEnumDomainId,
        actual: arcweft_id::closed_enum::ClosedEnumDomainId,
    },
}

/// Projects admitted semantic properties. Source-order overrides have already
/// been merged by the Dialogue codec; source selector order has no role here.
pub fn project_character_dialogue_rich_text_properties(
    properties: &CharacterDialogueRichTextProperties,
) -> Result<Vec<RichTextStyle>, CharacterDialogueRichTextProjectionError> {
    let mut output = Vec::new();
    let mut layout = RichTextLayoutUpdate::default();
    let mut transform = RichTextTransform::default();
    let mut has_transform = false;
    let mut center_origin = false;
    let mut explicit_origin = None;
    for property in properties.iter() {
        match property.id() {
            PropertyId::Color
            | PropertyId::Font
            | PropertyId::Size
            | PropertyId::Italic
            | PropertyId::ObliqueAngle
            | PropertyId::Opacity
            | PropertyId::Layer
            | PropertyId::ZIndex => output.push(project_scalar(property)?),
            PropertyId::WritingMode
            | PropertyId::RubyPosition
            | PropertyId::Direction
            | PropertyId::Latin
            | PropertyId::Jlreq
            | PropertyId::ColumnGap
            | PropertyId::RubySize
            | PropertyId::RubyGap
            | PropertyId::RubyOverhang
            | PropertyId::RubyCollisionGap => apply_layout(property, &mut layout)?,
            PropertyId::TranslateX
            | PropertyId::TranslateY
            | PropertyId::Rotate
            | PropertyId::ScaleX
            | PropertyId::ScaleY
            | PropertyId::SkewX
            | PropertyId::SkewY
            | PropertyId::TransformTarget => {
                has_transform = true;
                center_origin |= matches!(
                    property.id(),
                    PropertyId::Rotate | PropertyId::ScaleX | PropertyId::ScaleY
                );
                apply_transform(property, &mut transform)?;
            }
            PropertyId::TransformOrigin => {
                has_transform = true;
                explicit_origin = Some(transform_origin(property)?);
            }
        }
    }
    if !layout.is_empty() {
        output.push(RichTextStyle::Layout { layout });
    }
    if has_transform {
        transform.origin = explicit_origin.unwrap_or(if center_origin {
            RichTextTransformOrigin::Center
        } else {
            RichTextTransformOrigin::BaselineStart
        });
        output.push(RichTextStyle::Transform { transform });
    }
    Ok(output)
}

fn project_scalar(
    property: &CharacterDialogueRichTextProperty,
) -> Result<RichTextStyle, CharacterDialogueRichTextProjectionError> {
    Ok(match (property.id(), property.value()) {
        (PropertyId::Color, Value::Color(color)) => RichTextStyle::Color {
            value: match color {
                CharacterDialogueRichTextColor::Rgba8(value) => {
                    RichTextColor::Rgba8 { value: *value }
                }
                CharacterDialogueRichTextColor::Resource(id) => RichTextColor::Resource {
                    id: id.as_str().to_owned(),
                },
            },
        },
        (PropertyId::Font, Value::Text(name)) => RichTextStyle::Font {
            family: RichTextFontFamily::Named { name: name.clone() },
        },
        (PropertyId::Size, Value::Length { milli, .. }) => RichTextStyle::Size {
            milli_points: Milli(*milli),
        },
        (PropertyId::Italic, Value::ClosedEnum { .. })
            if closed_enum_variant(property, RichTextStyleSelector::Italic.schema_id())?
                == RichTextStyleSelector::Italic.ordinal() =>
        {
            RichTextStyle::Italic
        }
        (PropertyId::ObliqueAngle, Value::Angle(angle)) => RichTextStyle::Oblique {
            angle: RichTextAngle {
                degrees: Milli(*angle),
            },
        },
        (PropertyId::Opacity, Value::Ratio(opacity)) => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: Some(Milli(i32::from(*opacity))),
                ..RichTextPresentationStyle::default()
            },
        },
        (PropertyId::Layer, Value::PublicId(layer)) => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                layer: Some(layer.as_str().to_owned()),
                ..RichTextPresentationStyle::default()
            },
        },
        (PropertyId::ZIndex, Value::Int(z_index)) => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                z_index: Some(i16::try_from(*z_index).map_err(|_| invalid(property.id()))?),
                ..RichTextPresentationStyle::default()
            },
        },
        _ => return Err(invalid(property.id())),
    })
}

fn apply_layout(
    property: &CharacterDialogueRichTextProperty,
    layout: &mut RichTextLayoutUpdate,
) -> Result<(), CharacterDialogueRichTextProjectionError> {
    match property.id() {
        PropertyId::WritingMode => {
            let variant =
                closed_enum_variant(property, RichTextLayoutSelector::HorizontalTb.schema_id())?;
            layout.writing_mode = Some(match variant {
                0 => RichTextWritingMode::HorizontalTb,
                1 => RichTextWritingMode::VerticalRl,
                2 => RichTextWritingMode::VerticalLr,
                _ => return Err(invalid(property.id())),
            });
        }
        PropertyId::RubyPosition => {
            let variant =
                closed_enum_variant(property, RichTextLayoutSelector::RubyOver.schema_id())?;
            layout.ruby_position = Some(match variant {
                4 => RichTextRubyPosition::Over,
                5 => RichTextRubyPosition::Under,
                6 => RichTextRubyPosition::InterCharacter,
                _ => return Err(invalid(property.id())),
            });
        }
        PropertyId::Direction => {
            let variant = closed_enum_variant(property, LayoutDirection::Auto.schema_id())?;
            layout.direction = Some(match variant {
                0 => RichTextInlineDirection::Auto,
                1 => RichTextInlineDirection::Ltr,
                2 => RichTextInlineDirection::Rtl,
                _ => return Err(invalid(property.id())),
            });
        }
        PropertyId::Latin => {
            let variant = closed_enum_variant(property, VerticalLatin::Mixed.schema_id())?;
            layout.vertical_latin = Some(match variant {
                0 => RichTextVerticalLatinMode::Mixed,
                1 => RichTextVerticalLatinMode::Upright,
                2 => RichTextVerticalLatinMode::Sideways,
                _ => return Err(invalid(property.id())),
            });
        }
        PropertyId::Jlreq => {
            let variant = closed_enum_variant(property, Jlreq::Auto.schema_id())?;
            layout.jlreq_strictness = Some(match variant {
                0 => RichTextJlreqStrictness::Auto,
                1 => RichTextJlreqStrictness::Loose,
                2 => RichTextJlreqStrictness::Normal,
                3 => RichTextJlreqStrictness::Strict,
                _ => return Err(invalid(property.id())),
            });
        }
        PropertyId::ColumnGap => layout.column_gap = Some(Milli(length_milli(property)?)),
        PropertyId::RubySize => layout.ruby_font_size = Some(Milli(length_milli(property)?)),
        PropertyId::RubyGap => layout.ruby_gap = Some(Milli(length_milli(property)?)),
        PropertyId::RubyOverhang => layout.ruby_overhang = Some(Milli(length_milli(property)?)),
        PropertyId::RubyCollisionGap => {
            layout.ruby_collision_gap = Some(Milli(length_milli(property)?));
        }
        _ => return Err(invalid(property.id())),
    }
    Ok(())
}

fn apply_transform(
    property: &CharacterDialogueRichTextProperty,
    transform: &mut RichTextTransform,
) -> Result<(), CharacterDialogueRichTextProjectionError> {
    match (property.id(), property.value()) {
        (PropertyId::TranslateX, Value::Length { milli, .. }) => {
            transform.translate.x = Milli(*milli);
        }
        (PropertyId::TranslateY, Value::Length { milli, .. }) => {
            transform.translate.y = Milli(*milli);
        }
        (PropertyId::Rotate, Value::Angle(value)) => transform.rotate.degrees = Milli(*value),
        (PropertyId::ScaleX, Value::Milli(value)) => transform.scale.x = Milli(*value),
        (PropertyId::ScaleY, Value::Milli(value)) => transform.scale.y = Milli(*value),
        (PropertyId::SkewX, Value::Angle(value)) => transform.skew.x = Milli(*value),
        (PropertyId::SkewY, Value::Angle(value)) => transform.skew.y = Milli(*value),
        (PropertyId::TransformTarget, Value::ClosedEnum { .. }) => {
            transform.target = match closed_enum_variant(
                property,
                PresentationTransformTarget::Content.schema_id(),
            )? {
                0 => FxTarget::Node,
                1 => FxTarget::Content,
                2 => FxTarget::Background,
                3 => FxTarget::Line,
                4 => FxTarget::Glyph,
                5 => FxTarget::Viewport,
                _ => return Err(invalid(property.id())),
            };
        }
        _ => return Err(invalid(property.id())),
    }
    Ok(())
}

fn transform_origin(
    property: &CharacterDialogueRichTextProperty,
) -> Result<RichTextTransformOrigin, CharacterDialogueRichTextProjectionError> {
    Ok(
        match closed_enum_variant(
            property,
            PresentationTransformOrigin::BaselineStart.schema_id(),
        )? {
            0 => RichTextTransformOrigin::BaselineStart,
            1 => RichTextTransformOrigin::BaselineCenter,
            2 => RichTextTransformOrigin::Center,
            3 => RichTextTransformOrigin::GlyphCenter,
            _ => return Err(invalid(property.id())),
        },
    )
}

fn closed_enum_variant(
    property: &CharacterDialogueRichTextProperty,
    expected: arcweft_id::closed_enum::ClosedEnumDomainId,
) -> Result<u16, CharacterDialogueRichTextProjectionError> {
    let Value::ClosedEnum { domain, variant } = property.value() else {
        return Err(invalid(property.id()));
    };
    if *domain != expected {
        return Err(
            CharacterDialogueRichTextProjectionError::ClosedEnumDomainMismatch {
                property: property.id(),
                expected,
                actual: *domain,
            },
        );
    }
    Ok(*variant)
}

fn length_milli(
    property: &CharacterDialogueRichTextProperty,
) -> Result<i32, CharacterDialogueRichTextProjectionError> {
    let Value::Length { milli, .. } = property.value() else {
        return Err(invalid(property.id()));
    };
    Ok(*milli)
}

const fn invalid(property: PropertyId) -> CharacterDialogueRichTextProjectionError {
    CharacterDialogueRichTextProjectionError::InvalidValue { property }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::value::{RuntimeInt, RuntimeUInt, RuntimeValue};
    use arcweft_dialogue::CharacterDialogueRolePayloadCodec;
    use arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG;

    fn policy_with(
        values: impl IntoIterator<Item = (PropertyId, RuntimeValue)>,
    ) -> CharacterDialogueRichTextProperties {
        let codec = CharacterDialogueRolePayloadCodec::RichTextProperties;
        let mut payload = codec.no_overrides_payload().unwrap();
        let RuntimeValue::Tuple(slots) = &mut payload else {
            unreachable!()
        };
        let schema = PRESENTATION_CONTENT_CALLABLE_CATALOG
            .reusable_style_schema()
            .unwrap();
        for (id, value) in values {
            let index = schema
                .properties()
                .iter()
                .position(|field| field.id == id)
                .unwrap();
            slots[index] = RuntimeValue::option_some(value);
        }
        codec.decode_properties(&payload).unwrap()
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
                PropertyId::Color,
                RuntimeValue::Tuple(
                    [8, 16, 32, 255]
                        .into_iter()
                        .map(|v| RuntimeValue::UInt(RuntimeUInt::U8(v)))
                        .collect(),
                ),
            ),
            (
                PropertyId::Font,
                RuntimeValue::String("Yu Gothic".to_owned()),
            ),
            (PropertyId::Size, length(18_000, 2)),
            (
                PropertyId::Italic,
                closed_enum(RichTextStyleSelector::Italic.ordinal()),
            ),
            (
                PropertyId::Opacity,
                RuntimeValue::UInt(RuntimeUInt::U16(625)),
            ),
            (
                PropertyId::WritingMode,
                closed_enum(RichTextLayoutSelector::VerticalRl.ordinal()),
            ),
            (
                PropertyId::Latin,
                closed_enum(VerticalLatin::Upright.ordinal()),
            ),
            (PropertyId::ColumnGap, length(9_500, 1)),
            (PropertyId::Rotate, int_i32(45_000)),
            (
                PropertyId::TransformTarget,
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
                layout: RichTextLayoutUpdate {
                    writing_mode: Some(RichTextWritingMode::VerticalRl),
                    vertical_latin: Some(RichTextVerticalLatinMode::Upright),
                    column_gap: Some(Milli(9_500)),
                    ..RichTextLayoutUpdate::default()
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
    fn projection_preserves_semantic_order_and_typed_style_values() {
        let properties = representative_projection_properties();
        assert_eq!(
            project_character_dialogue_rich_text_properties(&properties).unwrap(),
            expected_representative_styles()
        );
    }

    #[test]
    fn transform_properties_combine_without_resetting_other_operations() {
        let mut values = vec![
            (PropertyId::TranslateX, length(3_000, 1)),
            (PropertyId::TranslateY, length(-2_000, 1)),
            (PropertyId::Rotate, int_i32(45_000)),
            (PropertyId::ScaleX, int_i32(1_500)),
            (PropertyId::SkewY, int_i32(10_000)),
        ];
        let check = |values, expected_origin| {
            let properties = policy_with(values);
            let styles = project_character_dialogue_rich_text_properties(&properties).unwrap();
            let [RichTextStyle::Transform { transform }] = styles.as_slice() else {
                panic!("one composed transform")
            };
            assert_eq!(transform.translate.x, Milli(3_000));
            assert_eq!(transform.translate.y, Milli(-2_000));
            assert_eq!(transform.rotate.degrees, Milli(45_000));
            assert_eq!(transform.scale.x, Milli(1_500));
            assert_eq!(transform.scale.y, Milli(1_000));
            assert_eq!(transform.skew.y, Milli(10_000));
            assert_eq!(transform.origin, expected_origin);
        };
        check(values.clone(), RichTextTransformOrigin::Center);
        values.push((
            PropertyId::TransformOrigin,
            closed_enum(PresentationTransformOrigin::BaselineStart.ordinal()),
        ));
        check(values, RichTextTransformOrigin::BaselineStart);
    }

    #[test]
    fn sparse_ruby_size_preserves_inherited_placement_and_other_layout_properties() {
        let properties = policy_with([(PropertyId::RubySize, length(11_000, 1))]);
        let styles = project_character_dialogue_rich_text_properties(&properties).unwrap();
        assert_eq!(
            styles,
            vec![RichTextStyle::Layout {
                layout: RichTextLayoutUpdate {
                    ruby_font_size: Some(Milli(11_000)),
                    ..RichTextLayoutUpdate::default()
                }
            }]
        );
        for position in [
            RichTextRubyPosition::Under,
            RichTextRubyPosition::InterCharacter,
        ] {
            let inherited = RichTextStyle::Layout {
                layout: RichTextLayoutUpdate {
                    writing_mode: Some(RichTextWritingMode::VerticalLr),
                    ruby_position: Some(position),
                    ruby_gap: Some(Milli(3_000)),
                    column_gap: Some(Milli(15_000)),
                    ..RichTextLayoutUpdate::default()
                },
            };
            let presentation =
                crate::presentation_from_styles(std::iter::once(&inherited).chain(&styles));
            let resolved = presentation.layout.unwrap();
            assert_eq!(resolved.writing_mode, RichTextWritingMode::VerticalLr);
            assert_eq!(resolved.ruby_position, position);
            assert_eq!(resolved.ruby_gap, Some(Milli(3_000)));
            assert_eq!(resolved.column_gap, Milli(15_000));
            assert_eq!(resolved.ruby_font_size, Some(Milli(11_000)));
        }
    }
}
