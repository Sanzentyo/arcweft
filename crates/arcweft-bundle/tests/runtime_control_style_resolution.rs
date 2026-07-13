use arcweft_bundle::resource_codec::{
    SystemColorOverride, ViewRuntimeNodeStyle, ViewRuntimeStyleProjectionError,
    ViewThemeEnvironmentDefaults, ViewThemeEnvironmentError, ViewThemeResource,
};
use arcweft_presentation::appearance::{
    ColorScheme, ColorSchemePreference, EnvironmentRevision, PresentationColor,
    PresentationEnvironment, SystemColor, SystemPaletteSet,
};
use arcweft_view::style::{
    ComputedViewStyle, ComputedViewStyleBuilder, ComputedViewStyleRevision, ViewAlignment,
    ViewAngleMilliDegrees, ViewBlendMode, ViewClip, ViewColorValue, ViewDisplay, ViewFlexDirection,
    ViewFlexWrap, ViewFontFamily, ViewFontFamilyList, ViewFontStyle, ViewFontWeight,
    ViewLengthMilli, ViewMask, ViewOverflow, ViewPosition, ViewPropertyKind, ViewRatioMilli,
    ViewScalarMilli, ViewSpecifiedValue, ViewStyleAssignOp, ViewStyleContribution,
    ViewStyleContributionSource, ViewStylePriority, ViewStyleValueKind, ViewSystemFontFamily,
};

fn computed(
    entries: impl IntoIterator<Item = (ViewPropertyKind, ViewSpecifiedValue)>,
) -> ComputedViewStyle {
    let mut builder = ComputedViewStyleBuilder::default();
    for (order, (property, value)) in entries.into_iter().enumerate() {
        assert!(builder.apply(ViewStyleContribution::new(
            property,
            value,
            ViewStyleAssignOp::Replace,
            ViewStylePriority::new(1, 1, 0, 0, 0, u32::try_from(order).unwrap_or(u32::MAX),),
            ViewStyleContributionSource::Inherited,
        )));
    }
    builder.finish(ComputedViewStyleRevision::new(1))
}

fn representative_value(kind: ViewStyleValueKind) -> ViewSpecifiedValue {
    match kind {
        ViewStyleValueKind::Bool => ViewSpecifiedValue::Bool { value: true },
        ViewStyleValueKind::Integer => ViewSpecifiedValue::Integer { value: 7 },
        ViewStyleValueKind::Ratio => ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(750).unwrap(),
        },
        ViewStyleValueKind::Scalar => ViewSpecifiedValue::Scalar {
            value: ViewScalarMilli::new(1_250),
        },
        ViewStyleValueKind::Length => ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(12_000),
        },
        ViewStyleValueKind::Angle => ViewSpecifiedValue::Angle {
            value: ViewAngleMilliDegrees::new(45_000),
        },
        ViewStyleValueKind::Color => ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal {
                color: PresentationColor::rgb(10, 20, 30),
            },
        },
        ViewStyleValueKind::FontFamilyList => ViewSpecifiedValue::FontFamilyList {
            value: ViewFontFamilyList::new(vec![ViewFontFamily::system(ViewSystemFontFamily::Ui)])
                .unwrap(),
        },
        ViewStyleValueKind::FontWeight => ViewSpecifiedValue::FontWeight {
            value: ViewFontWeight::new(600).unwrap(),
        },
        ViewStyleValueKind::FontStyle => ViewSpecifiedValue::FontStyle {
            value: ViewFontStyle::Italic,
        },
        ViewStyleValueKind::Display => ViewSpecifiedValue::Display {
            value: ViewDisplay::Flex,
        },
        ViewStyleValueKind::Position => ViewSpecifiedValue::Position {
            value: ViewPosition::Relative,
        },
        ViewStyleValueKind::Overflow => ViewSpecifiedValue::Overflow {
            value: ViewOverflow::Hidden,
        },
        ViewStyleValueKind::FlexDirection => ViewSpecifiedValue::FlexDirection {
            value: ViewFlexDirection::Column,
        },
        ViewStyleValueKind::FlexWrap => ViewSpecifiedValue::FlexWrap {
            value: ViewFlexWrap::Wrap,
        },
        ViewStyleValueKind::Alignment => ViewSpecifiedValue::Alignment {
            value: ViewAlignment::Start,
        },
        ViewStyleValueKind::BorderRadii | ViewStyleValueKind::Resource => {
            unreachable!("no canonical property uses this kind")
        }
        ViewStyleValueKind::ShadowList => ViewSpecifiedValue::ShadowList { value: Vec::new() },
        ViewStyleValueKind::FilterList => ViewSpecifiedValue::FilterList { value: Vec::new() },
        ViewStyleValueKind::Clip => ViewSpecifiedValue::Clip {
            value: ViewClip::None,
        },
        ViewStyleValueKind::Mask => ViewSpecifiedValue::Mask {
            value: ViewMask::None,
        },
        ViewStyleValueKind::BlendMode => ViewSpecifiedValue::BlendMode {
            value: ViewBlendMode::Normal,
        },
        ViewStyleValueKind::Transition => ViewSpecifiedValue::Transition { value: Vec::new() },
    }
}

#[test]
fn every_canonical_property_is_retained_in_exactly_one_runtime_partition() {
    let computed = computed(
        ViewPropertyKind::ALL
            .iter()
            .copied()
            .map(|property| (property, representative_value(property.value_kind()))),
    );
    let projected = ViewRuntimeNodeStyle::try_from_computed(
        &computed,
        &PresentationEnvironment::new(ColorScheme::Dark),
        &SystemPaletteSet::ENGINE_DEFAULT,
    )
    .unwrap();
    let partitions = [
        projected.layout(),
        projected.text(),
        projected.paint(),
        projected.composite(),
        projected.transition(),
    ];

    assert_eq!(
        partitions
            .iter()
            .map(|partition| partition.iter().len())
            .sum::<usize>(),
        ViewPropertyKind::ALL.len()
    );
    for property in ViewPropertyKind::ALL {
        assert_eq!(
            partitions
                .iter()
                .filter(|partition| partition.value(*property).is_some())
                .count(),
            1,
            "{property:?} must have one runtime owner"
        );
    }

    let visual = projected.visual();
    assert_eq!(visual.fill, Some(PresentationColor::rgb(10, 20, 30)));
    assert_eq!(visual.text, Some(PresentationColor::rgb(10, 20, 30)));
    assert_eq!(visual.placeholder, Some(PresentationColor::rgb(10, 20, 30)));
    assert_eq!(
        visual.composition_underline,
        Some(PresentationColor::rgb(10, 20, 30))
    );
    assert_eq!(visual.font_family.as_deref(), Some("Ui"));
    assert_eq!(visual.font_size_milli, Some(12_000));
    assert_eq!(visual.letter_spacing_milli, Some(12_000));
    assert_eq!(visual.depth_milli, Some(7));
}

#[test]
fn projection_uses_the_supplied_environment_palette_for_system_colors() {
    let expected = PresentationColor::rgb(101, 102, 103);
    let mut palettes = SystemPaletteSet::ENGINE_DEFAULT;
    palettes.dark.accent = expected;
    let computed = computed([(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Color {
            value: ViewColorValue::System {
                role: SystemColor::Accent,
            },
        },
    )]);

    let projected = ViewRuntimeNodeStyle::try_from_computed(
        &computed,
        &PresentationEnvironment::new(ColorScheme::Dark),
        &palettes,
    )
    .unwrap();

    assert_eq!(projected.visual().fill, Some(expected));
}

#[test]
fn malformed_computed_property_value_is_a_typed_error() {
    let computed = computed([(
        ViewPropertyKind::Width,
        ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal {
                color: PresentationColor::rgb(1, 2, 3),
            },
        },
    )]);

    assert_eq!(
        ViewRuntimeNodeStyle::try_from_computed(
            &computed,
            &PresentationEnvironment::new(ColorScheme::Light),
            &SystemPaletteSet::ENGINE_DEFAULT,
        ),
        Err(ViewRuntimeStyleProjectionError::ValueKindMismatch {
            property: ViewPropertyKind::Width,
            expected: ViewStyleValueKind::Length,
            actual: ViewStyleValueKind::Color,
        })
    );
}

#[test]
fn theme_owns_checked_environment_and_palette_resolution() {
    let accent = PresentationColor::rgb(41, 42, 43);
    let theme = ViewThemeResource {
        palette_overrides: vec![SystemColorOverride {
            color: SystemColor::Accent,
            light: None,
            dark: Some(accent),
            source: None,
        }],
        defaults: ViewThemeEnvironmentDefaults {
            color_scheme: ColorSchemePreference::System,
            text_scale_milli: 1_250,
            ..ViewThemeEnvironmentDefaults::default()
        },
        dark_mode_visual_golden_ids: Vec::new(),
    };
    let environment = theme
        .presentation_environment(ColorScheme::Dark, None, EnvironmentRevision(9))
        .unwrap();

    assert_eq!(environment.color_scheme(), ColorScheme::Dark);
    assert_eq!(environment.text_scale().value(), 1_250);
    assert_eq!(environment.revision(), EnvironmentRevision(9));
    assert_eq!(
        theme
            .system_palette_set()
            .color(ColorScheme::Dark, SystemColor::Accent),
        accent
    );

    let invalid = ViewThemeResource {
        defaults: ViewThemeEnvironmentDefaults {
            text_scale_milli: u32::from(u16::MAX) + 1,
            ..ViewThemeEnvironmentDefaults::default()
        },
        ..ViewThemeResource::default()
    };
    assert_eq!(
        invalid.presentation_environment(ColorScheme::Light, None, EnvironmentRevision::default(),),
        Err(ViewThemeEnvironmentError::TextScaleOutOfRange {
            value: u32::from(u16::MAX) + 1,
        })
    );
}
