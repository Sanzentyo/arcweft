use super::*;
use crate::style::{
    ViewAngleMilliDegrees, ViewBoxAxisMode, ViewDisplay, ViewFontFamilyList, ViewFontWeight,
    ViewLengthMilli, ViewOverflow, ViewPosition, ViewPropertyKind, ViewRatioMilli, ViewScalarMilli,
    ViewStyleTokenId, ViewStyleTransition, ViewStyleValueKind,
};
use arcweft_id::PublicId;
use arcweft_presentation::appearance::SystemColor;
use std::collections::BTreeSet;

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).expect("test identity")
}

fn token(value: &str) -> ViewStyleTokenId {
    ViewStyleTokenId::try_new(value).expect("test token")
}

fn ratio(value: u16) -> ViewRatioMilli {
    ViewRatioMilli::new(value).expect("test ratio")
}

fn color(red: u8, green: u8, blue: u8, alpha: u8) -> ViewColorValue {
    ViewColorValue::Literal {
        color: PresentationColor::rgba(red, green, blue, alpha),
    }
}

fn radii(values: [i32; 4]) -> ViewBorderRadii {
    ViewBorderRadii {
        top_left: ViewLengthMilli::new(values[0]),
        top_right: ViewLengthMilli::new(values[1]),
        bottom_right: ViewLengthMilli::new(values[2]),
        bottom_left: ViewLengthMilli::new(values[3]),
    }
}

fn shadow(values: [i32; 4], color: ViewColorValue, inset: bool) -> ViewShadow {
    ViewShadow {
        x: ViewLengthMilli::new(values[0]),
        y: ViewLengthMilli::new(values[1]),
        blur: ViewLengthMilli::new(values[2]),
        spread: ViewLengthMilli::new(values[3]),
        color,
        inset,
    }
}

fn transition(
    property: ViewPropertyKind,
    duration_millis: u32,
    delay_millis: u32,
) -> ViewStyleTransition {
    ViewStyleTransition::new(property, duration_millis, delay_millis)
        .expect("transitionable property")
}

fn families(values: Vec<ViewFontFamily>) -> ViewFontFamilyList {
    ViewFontFamilyList::new(values).expect("non-empty font family list")
}

fn representatives() -> Vec<ViewSpecifiedValue> {
    vec![
        ViewSpecifiedValue::Token {
            token: token("style.token.primary"),
            value_kind: ViewStyleValueKind::Color,
        },
        ViewSpecifiedValue::BoxAxes {
            value: ViewBoxAxisMode::HorizontalLtr,
        },
        ViewSpecifiedValue::Bool { value: false },
        ViewSpecifiedValue::Integer { value: 1 },
        ViewSpecifiedValue::Ratio { value: ratio(100) },
        ViewSpecifiedValue::Scalar {
            value: ViewScalarMilli::new(100),
        },
        ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(100),
        },
        ViewSpecifiedValue::Angle {
            value: ViewAngleMilliDegrees::new(100),
        },
        ViewSpecifiedValue::Color {
            value: color(1, 2, 3, 4),
        },
        ViewSpecifiedValue::FontFamilyList {
            value: families(vec![ViewFontFamily::System(ViewSystemFontFamily::Ui)]),
        },
        ViewSpecifiedValue::FontWeight {
            value: ViewFontWeight::new(400).expect("font weight"),
        },
        ViewSpecifiedValue::FontStyle {
            value: ViewFontStyle::Normal,
        },
        ViewSpecifiedValue::Display {
            value: ViewDisplay::Block,
        },
        ViewSpecifiedValue::Position {
            value: ViewPosition::Static,
        },
        ViewSpecifiedValue::Overflow {
            value: ViewOverflow::Visible,
        },
        ViewSpecifiedValue::FlexDirection {
            value: ViewFlexDirection::Row,
        },
        ViewSpecifiedValue::FlexWrap {
            value: ViewFlexWrap::NoWrap,
        },
        ViewSpecifiedValue::Alignment {
            value: ViewAlignment::Start,
        },
        ViewSpecifiedValue::BorderRadii {
            value: radii([1, 2, 3, 4]),
        },
        ViewSpecifiedValue::ShadowList {
            value: vec![shadow([1, 2, 3, 4], color(5, 6, 7, 8), false)],
        },
        ViewSpecifiedValue::FilterList {
            value: vec![ViewFilter::Blur {
                radius: ViewLengthMilli::new(1),
            }],
        },
        ViewSpecifiedValue::Clip {
            value: ViewClip::None,
        },
        ViewSpecifiedValue::Mask {
            value: ViewMask::None,
        },
        ViewSpecifiedValue::BlendMode {
            value: ViewBlendMode::Normal,
        },
        ViewSpecifiedValue::Transition {
            value: vec![transition(ViewPropertyKind::Opacity, 100, 10)],
        },
        ViewSpecifiedValue::Resource {
            value: public_id("resource.primary"),
        },
    ]
}

fn mutated_representatives() -> Vec<ViewSpecifiedValue> {
    vec![
        ViewSpecifiedValue::Token {
            token: token("style.token.secondary"),
            value_kind: ViewStyleValueKind::Color,
        },
        ViewSpecifiedValue::BoxAxes {
            value: ViewBoxAxisMode::HorizontalRtl,
        },
        ViewSpecifiedValue::Bool { value: true },
        ViewSpecifiedValue::Integer { value: 2 },
        ViewSpecifiedValue::Ratio { value: ratio(101) },
        ViewSpecifiedValue::Scalar {
            value: ViewScalarMilli::new(101),
        },
        ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(101),
        },
        ViewSpecifiedValue::Angle {
            value: ViewAngleMilliDegrees::new(101),
        },
        ViewSpecifiedValue::Color {
            value: color(2, 2, 3, 4),
        },
        ViewSpecifiedValue::FontFamilyList {
            value: families(vec![ViewFontFamily::System(ViewSystemFontFamily::Serif)]),
        },
        ViewSpecifiedValue::FontWeight {
            value: ViewFontWeight::new(401).expect("font weight"),
        },
        ViewSpecifiedValue::FontStyle {
            value: ViewFontStyle::Italic,
        },
        ViewSpecifiedValue::Display {
            value: ViewDisplay::Flex,
        },
        ViewSpecifiedValue::Position {
            value: ViewPosition::Relative,
        },
        ViewSpecifiedValue::Overflow {
            value: ViewOverflow::Hidden,
        },
        ViewSpecifiedValue::FlexDirection {
            value: ViewFlexDirection::RowReverse,
        },
        ViewSpecifiedValue::FlexWrap {
            value: ViewFlexWrap::Wrap,
        },
        ViewSpecifiedValue::Alignment {
            value: ViewAlignment::End,
        },
        ViewSpecifiedValue::BorderRadii {
            value: radii([2, 2, 3, 4]),
        },
        ViewSpecifiedValue::ShadowList {
            value: vec![shadow([2, 2, 3, 4], color(5, 6, 7, 8), false)],
        },
        ViewSpecifiedValue::FilterList {
            value: vec![ViewFilter::Blur {
                radius: ViewLengthMilli::new(2),
            }],
        },
        ViewSpecifiedValue::Clip {
            value: ViewClip::RoundedRect(radii([1, 2, 3, 4])),
        },
        ViewSpecifiedValue::Mask {
            value: ViewMask::Resource(public_id("resource.mask")),
        },
        ViewSpecifiedValue::BlendMode {
            value: ViewBlendMode::Multiply,
        },
        ViewSpecifiedValue::Transition {
            value: vec![transition(ViewPropertyKind::Opacity, 101, 10)],
        },
        ViewSpecifiedValue::Resource {
            value: public_id("resource.secondary"),
        },
    ]
}

#[test]
fn specified_value_outer_tags_are_unique_without_a_count_literal() {
    let values = representatives();
    let tags = values
        .iter()
        .map(ViewSpecifiedValue::semantic_tag)
        .collect::<BTreeSet<_>>();
    assert_eq!(tags.len(), values.len());
}

#[test]
fn specified_value_digest_is_deterministic_and_each_payload_is_sensitive() {
    let values = representatives();
    let same = representatives();
    let mutated = mutated_representatives();
    assert_eq!(values.len(), same.len());
    assert_eq!(values.len(), mutated.len());

    for ((value, same), mutated) in values.iter().zip(&same).zip(&mutated) {
        assert_eq!(value.semantic_digest(), same.semantic_digest());
        assert_eq!(value.semantic_tag(), mutated.semantic_tag());
        assert_ne!(value.semantic_digest(), mutated.semantic_digest());
    }
}

#[test]
fn nested_semantic_tags_are_unique() {
    fn unique(tags: impl IntoIterator<Item = u8>, expected: usize) {
        assert_eq!(tags.into_iter().collect::<BTreeSet<_>>().len(), expected);
    }

    unique(
        ViewStyleValueKind::ALL
            .iter()
            .copied()
            .map(ViewStyleValueKind::semantic_tag),
        ViewStyleValueKind::ALL.len(),
    );
    unique(
        ViewSystemFontFamily::ALL
            .iter()
            .copied()
            .map(ViewSystemFontFamily::semantic_tag),
        ViewSystemFontFamily::ALL.len(),
    );
    unique(
        ViewFlexDirection::ALL
            .iter()
            .copied()
            .map(ViewFlexDirection::semantic_tag),
        ViewFlexDirection::ALL.len(),
    );
    unique(
        ViewFlexWrap::ALL
            .iter()
            .copied()
            .map(ViewFlexWrap::semantic_tag),
        ViewFlexWrap::ALL.len(),
    );
    unique(
        ViewFontStyle::ALL
            .iter()
            .copied()
            .map(ViewFontStyle::semantic_tag),
        ViewFontStyle::ALL.len(),
    );
    unique(
        ViewAlignment::ALL
            .iter()
            .copied()
            .map(ViewAlignment::semantic_tag),
        ViewAlignment::ALL.len(),
    );
    unique(
        ViewBlendMode::ALL
            .iter()
            .copied()
            .map(ViewBlendMode::semantic_tag),
        ViewBlendMode::ALL.len(),
    );

    let filters = [
        ViewFilter::Blur {
            radius: ViewLengthMilli::new(1),
        },
        ViewFilter::Brightness {
            amount: ViewScalarMilli::new(1),
        },
        ViewFilter::Contrast {
            amount: ViewScalarMilli::new(1),
        },
        ViewFilter::Opacity { amount: ratio(1) },
    ];
    unique(
        filters.iter().copied().map(ViewFilter::semantic_tag),
        filters.len(),
    );

    let clips = [ViewClip::None, ViewClip::RoundedRect(radii([1, 2, 3, 4]))];
    unique(
        clips.iter().copied().map(ViewClip::semantic_tag),
        clips.len(),
    );

    let masks = [
        ViewMask::None,
        ViewMask::Resource(public_id("resource.mask")),
    ];
    unique(masks.iter().map(ViewMask::semantic_tag), masks.len());
}

#[test]
fn compound_payload_field_order_and_values_are_semantic() {
    let base_radii = ViewSpecifiedValue::BorderRadii {
        value: radii([1, 2, 3, 4]),
    };
    for values in [[9, 2, 3, 4], [1, 9, 3, 4], [1, 2, 9, 4], [1, 2, 3, 9]] {
        assert_ne!(
            base_radii.semantic_digest(),
            ViewSpecifiedValue::BorderRadii {
                value: radii(values)
            }
            .semantic_digest()
        );
    }

    let base_shadow = ViewSpecifiedValue::ShadowList {
        value: vec![shadow([1, 2, 3, 4], color(5, 6, 7, 8), false)],
    };
    let shadow_mutations = [
        shadow([9, 2, 3, 4], color(5, 6, 7, 8), false),
        shadow([1, 9, 3, 4], color(5, 6, 7, 8), false),
        shadow([1, 2, 9, 4], color(5, 6, 7, 8), false),
        shadow([1, 2, 3, 9], color(5, 6, 7, 8), false),
        shadow([1, 2, 3, 4], color(9, 6, 7, 8), false),
        shadow([1, 2, 3, 4], color(5, 6, 7, 8), true),
    ];
    for mutation in shadow_mutations {
        assert_ne!(
            base_shadow.semantic_digest(),
            ViewSpecifiedValue::ShadowList {
                value: vec![mutation]
            }
            .semantic_digest()
        );
    }

    let base_transition = ViewSpecifiedValue::Transition {
        value: vec![transition(ViewPropertyKind::Opacity, 100, 10)],
    };
    for mutation in [
        transition(ViewPropertyKind::Rotate, 100, 10),
        transition(ViewPropertyKind::Opacity, 101, 10),
        transition(ViewPropertyKind::Opacity, 100, 11),
    ] {
        assert_ne!(
            base_transition.semantic_digest(),
            ViewSpecifiedValue::Transition {
                value: vec![mutation]
            }
            .semantic_digest()
        );
    }
}

#[test]
fn color_kind_list_order_and_nested_payloads_are_semantic() {
    let literal = ViewSpecifiedValue::Color {
        value: color(1, 2, 3, 4),
    };
    for mutation in [
        color(9, 2, 3, 4),
        color(1, 9, 3, 4),
        color(1, 2, 9, 4),
        color(1, 2, 3, 9),
        ViewColorValue::System {
            role: SystemColor::Canvas,
        },
    ] {
        assert_ne!(
            literal.semantic_digest(),
            ViewSpecifiedValue::Color { value: mutation }.semantic_digest()
        );
    }
    assert_ne!(
        ViewSpecifiedValue::Color {
            value: ViewColorValue::System {
                role: SystemColor::Canvas,
            }
        }
        .semantic_digest(),
        ViewSpecifiedValue::Color {
            value: ViewColorValue::System {
                role: SystemColor::CanvasText,
            }
        }
        .semantic_digest()
    );

    let token_value = ViewSpecifiedValue::Token {
        token: token("style.token.primary"),
        value_kind: ViewStyleValueKind::Color,
    };
    assert_ne!(
        token_value.semantic_digest(),
        ViewSpecifiedValue::Token {
            token: token("style.token.primary"),
            value_kind: ViewStyleValueKind::Length,
        }
        .semantic_digest()
    );

    let first = ViewFontFamily::System(ViewSystemFontFamily::Ui);
    let second = ViewFontFamily::Named("Narrative".to_owned());
    assert_ne!(
        ViewSpecifiedValue::FontFamilyList {
            value: families(vec![first.clone(), second.clone()])
        }
        .semantic_digest(),
        ViewSpecifiedValue::FontFamilyList {
            value: families(vec![second, first])
        }
        .semantic_digest()
    );
}
