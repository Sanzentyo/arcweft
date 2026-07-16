use arcweft_bundle::resource_codec::{
    SystemColorOverride, ViewRuntimeNodeStyle, ViewRuntimeStyleProjectionError, ViewThemeResource,
};
use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationColor, PresentationEnvironment,
    PresentationEnvironmentOverrides, PresentationEnvironmentValue, PresentationEnvironmentValues,
    SystemColor, SystemPaletteSet, TextScaleMilli,
};

fn environment(color_scheme: ColorScheme) -> PresentationEnvironment {
    PresentationEnvironment::initial(PresentationEnvironmentValues::new(
        color_scheme,
        ContrastPreference::Standard,
        false,
        TextScaleMilli::ONE,
    ))
}
use arcweft_view::style::{
    ComputedViewStyle, ComputedViewStyleBuilder, ComputedViewStyleRevision, ViewAlignment,
    ViewAngleMilliDegrees, ViewBlendMode, ViewBoxAxisMode, ViewClip, ViewColorValue, ViewDisplay,
    ViewFlexDirection, ViewFlexWrap, ViewFontFamily, ViewFontFamilyList, ViewFontStyle,
    ViewFontWeight, ViewLengthMilli, ViewMask, ViewOverflow, ViewPosition, ViewPropertyKind,
    ViewRatioMilli, ViewScalarMilli, ViewSpecifiedValue, ViewStyleAssignOp, ViewStyleContribution,
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
        ViewStyleValueKind::BoxAxes => ViewSpecifiedValue::BoxAxes {
            value: ViewBoxAxisMode::HorizontalLtr,
        },
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
            .filter(|property| property.is_computed_canonical())
            .map(|property| (property, representative_value(property.value_kind()))),
    );
    let projected = ViewRuntimeNodeStyle::try_from_computed(
        &computed,
        &environment(ColorScheme::Dark),
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
        ViewPropertyKind::ALL
            .iter()
            .filter(|property| property.is_computed_canonical())
            .count()
    );
    for property in ViewPropertyKind::ALL
        .iter()
        .filter(|property| property.is_computed_canonical())
    {
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
        &environment(ColorScheme::Dark),
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
            &environment(ColorScheme::Light),
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
    let mut environment_overrides = PresentationEnvironmentOverrides::empty();
    environment_overrides.insert(PresentationEnvironmentValue::TextScale(
        TextScaleMilli::try_new(1_250).unwrap(),
    ));
    let theme = ViewThemeResource {
        palette_overrides: vec![SystemColorOverride {
            color: SystemColor::Accent,
            light: None,
            dark: Some(accent),
            source: None,
        }],
        environment: environment_overrides,
        dark_mode_visual_golden_ids: Vec::new(),
    };
    let effective = theme
        .environment_overrides()
        .apply_to(PresentationEnvironmentValues::new(
            ColorScheme::Dark,
            ContrastPreference::Standard,
            false,
            TextScaleMilli::ONE,
        ));

    assert_eq!(effective.color_scheme(), ColorScheme::Dark);
    assert_eq!(effective.text_scale().value(), 1_250);
    assert_eq!(
        theme
            .system_palette_set()
            .color(ColorScheme::Dark, SystemColor::Accent),
        accent
    );

    let invalid = r#"{
        "palette_overrides": [],
        "environment": { "text_scale": 65536 },
        "dark_mode_visual_golden_ids": []
    }"#;
    assert!(serde_json::from_str::<ViewThemeResource>(invalid).is_err());
}
