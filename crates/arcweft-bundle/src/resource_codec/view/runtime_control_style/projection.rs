//! Pure projection from one canonical computed Style snapshot.

use super::{
    ViewRuntimeControlBorderStyle, ViewRuntimeControlCornerFrameStyle, ViewRuntimeControlFilter,
    ViewRuntimeControlFilterList, ViewRuntimeControlFocusRingStyle, ViewRuntimeControlVisualStyle,
    ViewRuntimeGeometryOwner, ViewRuntimeNodeStyle, ViewRuntimePhysicalNodeStyle,
    ViewRuntimeShadow, ViewRuntimeShadowKind, ViewRuntimeStyleProjectionError,
    ViewRuntimeStyleProperties,
};
use arcweft_presentation::appearance::{
    PresentationColor, PresentationEnvironment, SystemColor, SystemPaletteSet,
};
use arcweft_view::ViewStyleNodeKey;
use arcweft_view::style::{
    ComputedViewStyle, ViewColorValue, ViewFilter, ViewFontFamily, ViewLengthMilli,
    ViewPropertyKind, ViewShadow, ViewSpecifiedValue,
};

pub(super) fn project_computed_style(
    node: ViewStyleNodeKey,
    owner: ViewRuntimeGeometryOwner,
    computed: &ComputedViewStyle,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
) -> Result<ViewRuntimeNodeStyle, ViewRuntimeStyleProjectionError> {
    let mut projected = ViewRuntimeNodeStyle {
        layout: ViewRuntimeStyleProperties::default(),
        text: ViewRuntimeStyleProperties::default(),
        paint: ViewRuntimeStyleProperties::default(),
        composite: ViewRuntimeStyleProperties::default(),
        transition: ViewRuntimeStyleProperties::default(),
        physical: ViewRuntimePhysicalNodeStyle::try_from_computed(node, owner, computed)?,
        visual: ViewRuntimeControlVisualStyle::default(),
    };
    for (property, entry) in computed.properties() {
        let value = entry.value();
        let expected = property.value_kind();
        let actual = value.kind();
        if actual != expected {
            return Err(ViewRuntimeStyleProjectionError::ValueKindMismatch {
                property,
                expected,
                actual,
            });
        }
        retain_property(&mut projected, property, value.clone());
        project_visual_property(
            &mut projected.visual,
            property,
            value,
            environment,
            palettes,
        );
    }
    Ok(projected)
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed property inventory is exhaustively assigned to one runtime owner"
)]
fn retain_property(
    projected: &mut ViewRuntimeNodeStyle,
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
) {
    match property {
        ViewPropertyKind::BoxAxes
        | ViewPropertyKind::Visibility
        | ViewPropertyKind::Display
        | ViewPropertyKind::Width
        | ViewPropertyKind::Height
        | ViewPropertyKind::InlineSize
        | ViewPropertyKind::BlockSize
        | ViewPropertyKind::MinWidth
        | ViewPropertyKind::MinHeight
        | ViewPropertyKind::MinInlineSize
        | ViewPropertyKind::MinBlockSize
        | ViewPropertyKind::MaxWidth
        | ViewPropertyKind::MaxHeight
        | ViewPropertyKind::MaxInlineSize
        | ViewPropertyKind::MaxBlockSize
        | ViewPropertyKind::Padding
        | ViewPropertyKind::PaddingTop
        | ViewPropertyKind::PaddingRight
        | ViewPropertyKind::PaddingBottom
        | ViewPropertyKind::PaddingLeft
        | ViewPropertyKind::PaddingInlineStart
        | ViewPropertyKind::PaddingInlineEnd
        | ViewPropertyKind::PaddingBlockStart
        | ViewPropertyKind::PaddingBlockEnd
        | ViewPropertyKind::Margin
        | ViewPropertyKind::MarginTop
        | ViewPropertyKind::MarginRight
        | ViewPropertyKind::MarginBottom
        | ViewPropertyKind::MarginLeft
        | ViewPropertyKind::MarginInlineStart
        | ViewPropertyKind::MarginInlineEnd
        | ViewPropertyKind::MarginBlockStart
        | ViewPropertyKind::MarginBlockEnd
        | ViewPropertyKind::Gap
        | ViewPropertyKind::RowGap
        | ViewPropertyKind::ColumnGap
        | ViewPropertyKind::Position
        | ViewPropertyKind::Top
        | ViewPropertyKind::Right
        | ViewPropertyKind::Bottom
        | ViewPropertyKind::Left
        | ViewPropertyKind::InsetInlineStart
        | ViewPropertyKind::InsetInlineEnd
        | ViewPropertyKind::InsetBlockStart
        | ViewPropertyKind::InsetBlockEnd
        | ViewPropertyKind::ZIndex
        | ViewPropertyKind::Overflow
        | ViewPropertyKind::OverflowX
        | ViewPropertyKind::OverflowY
        | ViewPropertyKind::OverflowInline
        | ViewPropertyKind::OverflowBlock
        | ViewPropertyKind::FlexDirection
        | ViewPropertyKind::FlexWrap
        | ViewPropertyKind::FlexGrow
        | ViewPropertyKind::FlexShrink
        | ViewPropertyKind::FlexBasis
        | ViewPropertyKind::Order
        | ViewPropertyKind::AlignItems
        | ViewPropertyKind::AlignSelf
        | ViewPropertyKind::AlignContent
        | ViewPropertyKind::JustifyContent
        | ViewPropertyKind::JustifySelf => projected.layout.insert(property, value),
        ViewPropertyKind::Color
        | ViewPropertyKind::FontFamily
        | ViewPropertyKind::FontSize
        | ViewPropertyKind::FontWeight
        | ViewPropertyKind::FontStyle
        | ViewPropertyKind::LineHeight
        | ViewPropertyKind::LetterSpacing
        | ViewPropertyKind::TextAlign
        | ViewPropertyKind::PlaceholderColor
        | ViewPropertyKind::SelectionColor
        | ViewPropertyKind::CaretColor
        | ViewPropertyKind::CompositionUnderlineColor => projected.text.insert(property, value),
        ViewPropertyKind::BackgroundColor
        | ViewPropertyKind::BorderColor
        | ViewPropertyKind::BorderWidth
        | ViewPropertyKind::BorderRadius
        | ViewPropertyKind::OutlineColor
        | ViewPropertyKind::OutlineWidth
        | ViewPropertyKind::OutlineOffset
        | ViewPropertyKind::FocusRingColor
        | ViewPropertyKind::FocusRingWidth
        | ViewPropertyKind::CornerFrameColor
        | ViewPropertyKind::CornerFrameWidth
        | ViewPropertyKind::CornerFrameLength
        | ViewPropertyKind::CornerFrameOffset
        | ViewPropertyKind::BoxShadow => projected.paint.insert(property, value),
        ViewPropertyKind::Opacity
        | ViewPropertyKind::TranslateX
        | ViewPropertyKind::TranslateY
        | ViewPropertyKind::TranslateInline
        | ViewPropertyKind::TranslateBlock
        | ViewPropertyKind::Scale
        | ViewPropertyKind::Rotate
        | ViewPropertyKind::Filter
        | ViewPropertyKind::BackdropFilter
        | ViewPropertyKind::Clip
        | ViewPropertyKind::Mask
        | ViewPropertyKind::BlendMode => projected.composite.insert(property, value),
        ViewPropertyKind::Transition => projected.transition.insert(property, value),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the visual packet projection exhaustively distinguishes consumed and retained-only properties"
)]
fn project_visual_property(
    visual: &mut ViewRuntimeControlVisualStyle,
    property: ViewPropertyKind,
    value: &ViewSpecifiedValue,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
) {
    match property {
        ViewPropertyKind::Color => {
            visual.text =
                color_value(value).map(|value| runtime_color(value, environment, palettes));
        }
        ViewPropertyKind::BackgroundColor => {
            visual.fill =
                color_value(value).map(|value| runtime_color(value, environment, palettes));
        }
        ViewPropertyKind::PlaceholderColor => {
            visual.placeholder =
                color_value(value).map(|value| runtime_color(value, environment, palettes));
        }
        ViewPropertyKind::SelectionColor => {
            visual.selection =
                color_value(value).map(|value| runtime_color(value, environment, palettes));
        }
        ViewPropertyKind::CaretColor => {
            visual.caret =
                color_value(value).map(|value| runtime_color(value, environment, palettes));
        }
        ViewPropertyKind::CompositionUnderlineColor => {
            visual.composition_underline =
                color_value(value).map(|value| runtime_color(value, environment, palettes));
        }
        ViewPropertyKind::BorderColor => {
            if let Some(value) = color_value(value) {
                upsert_border(visual, environment, palettes, |border| {
                    border.color = runtime_color(value, environment, palettes);
                });
            }
        }
        ViewPropertyKind::BorderWidth => {
            if let Some(value) = length_value(value) {
                upsert_border(visual, environment, palettes, |border| {
                    border.width_milli = non_negative_length(value);
                });
            }
        }
        ViewPropertyKind::CornerFrameColor => {
            if let Some(value) = color_value(value) {
                upsert_corner_frame(visual, environment, palettes, |frame| {
                    frame.color = runtime_color(value, environment, palettes);
                });
            }
        }
        ViewPropertyKind::CornerFrameWidth => {
            if let Some(value) = length_value(value) {
                upsert_corner_frame(visual, environment, palettes, |frame| {
                    frame.width_milli = non_negative_length(value);
                });
            }
        }
        ViewPropertyKind::CornerFrameLength => {
            if let Some(value) = length_value(value) {
                upsert_corner_frame(visual, environment, palettes, |frame| {
                    frame.length_milli = non_negative_length(value);
                });
            }
        }
        ViewPropertyKind::CornerFrameOffset => {
            if let Some(value) = length_value(value) {
                upsert_corner_frame(visual, environment, palettes, |frame| {
                    frame.offset_milli = value.value();
                });
            }
        }
        ViewPropertyKind::FocusRingColor | ViewPropertyKind::OutlineColor => {
            if let Some(value) = color_value(value) {
                upsert_focus_ring(visual, environment, palettes, |ring| {
                    ring.color = runtime_color(value, environment, palettes);
                });
            }
        }
        ViewPropertyKind::FocusRingWidth | ViewPropertyKind::OutlineWidth => {
            if let Some(value) = length_value(value) {
                upsert_focus_ring(visual, environment, palettes, |ring| {
                    ring.width_milli = non_negative_length(value);
                });
            }
        }
        ViewPropertyKind::OutlineOffset => {
            if let Some(value) = length_value(value) {
                upsert_focus_ring(visual, environment, palettes, |ring| {
                    ring.offset_milli = value.value();
                });
            }
        }
        ViewPropertyKind::Opacity => {
            if let ViewSpecifiedValue::Ratio { value } = value {
                visual.opacity_milli = Some(value.value());
            }
        }
        ViewPropertyKind::FontFamily => {
            if let ViewSpecifiedValue::FontFamilyList { value } = value {
                visual.font_family = Some(runtime_font_family(value));
            }
        }
        ViewPropertyKind::FontSize => {
            if let Some(value) = length_value(value) {
                visual.font_size_milli = Some(non_negative_length(value));
            }
        }
        ViewPropertyKind::LineHeight => {
            if let Some(value) = length_value(value) {
                visual.line_height_milli = Some(non_negative_length(value));
            }
        }
        ViewPropertyKind::LetterSpacing => {
            if let Some(value) = length_value(value) {
                visual.letter_spacing_milli = Some(value.value());
            }
        }
        ViewPropertyKind::FontWeight => {
            if let ViewSpecifiedValue::FontWeight { value } = value {
                visual.font_weight = Some(value.value());
            }
        }
        ViewPropertyKind::BorderRadius => {
            if let Some(value) = length_value(value) {
                visual.radius_milli = Some(non_negative_length(value));
                visual.radii_milli = None;
            }
        }
        ViewPropertyKind::ZIndex => {
            if let ViewSpecifiedValue::Integer { value } = value {
                visual.depth_milli = Some(*value);
            }
        }
        ViewPropertyKind::Filter => {
            if let ViewSpecifiedValue::FilterList { value } = value {
                visual.filters = Some(runtime_filter_list(value));
            }
        }
        ViewPropertyKind::BackdropFilter => {
            if let ViewSpecifiedValue::FilterList { value } = value {
                visual.backdrop_filters = Some(runtime_filter_list(value));
            }
        }
        ViewPropertyKind::BoxShadow => {
            if let ViewSpecifiedValue::ShadowList { value } = value {
                visual.shadows = runtime_shadow_list(
                    value,
                    visual.radius_milli.unwrap_or_default(),
                    environment,
                    palettes,
                );
            }
        }
        ViewPropertyKind::BoxAxes
        | ViewPropertyKind::Visibility
        | ViewPropertyKind::Display
        | ViewPropertyKind::Width
        | ViewPropertyKind::Height
        | ViewPropertyKind::InlineSize
        | ViewPropertyKind::BlockSize
        | ViewPropertyKind::MinWidth
        | ViewPropertyKind::MinHeight
        | ViewPropertyKind::MinInlineSize
        | ViewPropertyKind::MinBlockSize
        | ViewPropertyKind::MaxWidth
        | ViewPropertyKind::MaxHeight
        | ViewPropertyKind::MaxInlineSize
        | ViewPropertyKind::MaxBlockSize
        | ViewPropertyKind::Padding
        | ViewPropertyKind::PaddingTop
        | ViewPropertyKind::PaddingRight
        | ViewPropertyKind::PaddingBottom
        | ViewPropertyKind::PaddingLeft
        | ViewPropertyKind::PaddingInlineStart
        | ViewPropertyKind::PaddingInlineEnd
        | ViewPropertyKind::PaddingBlockStart
        | ViewPropertyKind::PaddingBlockEnd
        | ViewPropertyKind::Margin
        | ViewPropertyKind::MarginTop
        | ViewPropertyKind::MarginRight
        | ViewPropertyKind::MarginBottom
        | ViewPropertyKind::MarginLeft
        | ViewPropertyKind::MarginInlineStart
        | ViewPropertyKind::MarginInlineEnd
        | ViewPropertyKind::MarginBlockStart
        | ViewPropertyKind::MarginBlockEnd
        | ViewPropertyKind::Gap
        | ViewPropertyKind::RowGap
        | ViewPropertyKind::ColumnGap
        | ViewPropertyKind::Position
        | ViewPropertyKind::Top
        | ViewPropertyKind::Right
        | ViewPropertyKind::Bottom
        | ViewPropertyKind::Left
        | ViewPropertyKind::InsetInlineStart
        | ViewPropertyKind::InsetInlineEnd
        | ViewPropertyKind::InsetBlockStart
        | ViewPropertyKind::InsetBlockEnd
        | ViewPropertyKind::Overflow
        | ViewPropertyKind::OverflowX
        | ViewPropertyKind::OverflowY
        | ViewPropertyKind::OverflowInline
        | ViewPropertyKind::OverflowBlock
        | ViewPropertyKind::FlexDirection
        | ViewPropertyKind::FlexWrap
        | ViewPropertyKind::FlexGrow
        | ViewPropertyKind::FlexShrink
        | ViewPropertyKind::FlexBasis
        | ViewPropertyKind::Order
        | ViewPropertyKind::AlignItems
        | ViewPropertyKind::AlignSelf
        | ViewPropertyKind::AlignContent
        | ViewPropertyKind::JustifyContent
        | ViewPropertyKind::JustifySelf
        | ViewPropertyKind::FontStyle
        | ViewPropertyKind::TextAlign
        | ViewPropertyKind::TranslateX
        | ViewPropertyKind::TranslateY
        | ViewPropertyKind::TranslateInline
        | ViewPropertyKind::TranslateBlock
        | ViewPropertyKind::Scale
        | ViewPropertyKind::Rotate
        | ViewPropertyKind::Clip
        | ViewPropertyKind::Mask
        | ViewPropertyKind::BlendMode
        | ViewPropertyKind::Transition => {}
    }
}

const fn color_value(value: &ViewSpecifiedValue) -> Option<ViewColorValue> {
    if let ViewSpecifiedValue::Color { value } = value {
        Some(*value)
    } else {
        None
    }
}

const fn length_value(value: &ViewSpecifiedValue) -> Option<ViewLengthMilli> {
    if let ViewSpecifiedValue::Length { value } = value {
        Some(*value)
    } else {
        None
    }
}

const fn runtime_color(
    value: ViewColorValue,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
) -> PresentationColor {
    match value {
        ViewColorValue::Literal { color } => color,
        ViewColorValue::System { role } => palettes.color(environment.color_scheme(), role),
    }
}

fn runtime_font_family(value: &arcweft_view::style::ViewFontFamilyList) -> String {
    value
        .as_slice()
        .iter()
        .map(|family| match family {
            ViewFontFamily::Named(name) => name.as_str(),
            ViewFontFamily::System(system) => system.source_name(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn runtime_shadow_list(
    values: &[ViewShadow],
    fallback_radius_milli: u32,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
) -> Vec<ViewRuntimeShadow> {
    values
        .iter()
        .map(|shadow| ViewRuntimeShadow {
            offset_x_milli: shadow.x.value(),
            offset_y_milli: shadow.y.value(),
            blur_milli: non_negative_length(shadow.blur),
            spread_milli: shadow.spread.value(),
            radius_milli: fallback_radius_milli,
            color: runtime_color(shadow.color, environment, palettes),
            kind: if shadow.inset {
                ViewRuntimeShadowKind::Inset
            } else {
                ViewRuntimeShadowKind::Outer
            },
        })
        .collect()
}

fn runtime_filter_list(values: &[ViewFilter]) -> ViewRuntimeControlFilterList {
    ViewRuntimeControlFilterList {
        filters: values
            .iter()
            .map(|filter| match filter {
                ViewFilter::Blur { radius } => ViewRuntimeControlFilter::Blur {
                    radius_milli: non_negative_length(*radius),
                },
                ViewFilter::Brightness { amount } => ViewRuntimeControlFilter::Brightness {
                    factor_milli: amount.value(),
                },
                ViewFilter::Contrast { amount } => ViewRuntimeControlFilter::Contrast {
                    factor_milli: amount.value(),
                },
                ViewFilter::Opacity { amount } => ViewRuntimeControlFilter::Opacity {
                    amount_milli: amount.value(),
                },
            })
            .collect(),
    }
}

fn non_negative_length(value: ViewLengthMilli) -> u32 {
    u32::try_from(value.value().max(0)).unwrap_or_default()
}

fn upsert_border(
    visual: &mut ViewRuntimeControlVisualStyle,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
    update: impl FnOnce(&mut ViewRuntimeControlBorderStyle),
) {
    let border = visual.border.get_or_insert(ViewRuntimeControlBorderStyle {
        color: palettes.color(environment.color_scheme(), SystemColor::Border),
        width_milli: 1_000,
    });
    update(border);
}

fn upsert_corner_frame(
    visual: &mut ViewRuntimeControlVisualStyle,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
    update: impl FnOnce(&mut ViewRuntimeControlCornerFrameStyle),
) {
    let frame = visual
        .corner_frame
        .get_or_insert(ViewRuntimeControlCornerFrameStyle {
            color: palettes.color(environment.color_scheme(), SystemColor::FocusRing),
            width_milli: 2_000,
            length_milli: 18_000,
            offset_milli: 0,
        });
    update(frame);
}

fn upsert_focus_ring(
    visual: &mut ViewRuntimeControlVisualStyle,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
    update: impl FnOnce(&mut ViewRuntimeControlFocusRingStyle),
) {
    let ring = visual
        .focus_ring
        .get_or_insert(ViewRuntimeControlFocusRingStyle {
            color: palettes.color(environment.color_scheme(), SystemColor::FocusRing),
            width_milli: 2_000,
            offset_milli: 2_000,
        });
    update(ring);
}
