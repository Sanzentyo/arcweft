use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_box_shadow::ViewBoxShadowPassPlan;
use arcweft_render_wgpu::view_scene::{
    ViewBoxShadow, ViewBoxShadowCornerRadius, ViewBoxShadowKind, ViewBoxShadowRadii,
    ViewColorRgba8, ViewCompositingEffectClass, ViewFilter,
};
use arcweft_takumi_adapter::{DirectCssFeature, DirectCssSupport, TakumiCompositingStyle};
use takumi::prelude::Viewport;
use takumi::unstable::base::layout::style::{
    BoxShadow as TakumiBoxShadow, Color as TakumiColor, ColorInput, ComputedStyle,
    Filter as TakumiFilter, Length, SizingContext, SpacePair, TextShadow as TakumiTextShadow,
};

fn sizing_context() -> SizingContext {
    SizingContext::builder()
        .viewport(Viewport::new((320, 180)))
        .build()
}

fn current_color() -> TakumiColor {
    TakumiColor([1, 2, 3, 255])
}

fn ui_color(red: u8, green: u8, blue: u8, alpha: u8) -> ViewColorRgba8 {
    ViewColorRgba8 {
        red,
        green,
        blue,
        alpha,
    }
}

fn takumi_color(red: u8, green: u8, blue: u8, alpha: u8) -> ColorInput {
    ColorInput::Value(TakumiColor([red, green, blue, alpha]))
}

fn takumi_shadow(
    inset: bool,
    horizontal_shift_px: f32,
    vertical_shift_px: f32,
    blur_radius_px: f32,
    spread_radius_px: f32,
    color: ColorInput,
) -> TakumiBoxShadow {
    TakumiBoxShadow::builder()
        .inset(inset)
        .offset_x(Length::Px(horizontal_shift_px))
        .offset_y(Length::Px(vertical_shift_px))
        .blur_radius(Length::Px(blur_radius_px))
        .spread_radius(Length::Px(spread_radius_px))
        .color(color)
        .build()
}

fn computed_style_with_radius_and_shadows(
    radius_px: f32,
    shadows: impl IntoIterator<Item = TakumiBoxShadow>,
) -> ComputedStyle {
    let radius = SpacePair::from_single(Length::Px(radius_px));
    ComputedStyle {
        border_top_left_radius: radius,
        border_top_right_radius: radius,
        border_bottom_right_radius: radius,
        border_bottom_left_radius: radius,
        box_shadow: Some(shadows.into_iter().collect::<Vec<_>>().into_boxed_slice()),
        ..Default::default()
    }
}

fn computed_style_with_corner_radii_and_shadows(
    corners: &[SpacePair<Length>; 4],
    shadows: impl IntoIterator<Item = TakumiBoxShadow>,
) -> ComputedStyle {
    let [top_left, top_right, bottom_right, bottom_left] = *corners;
    ComputedStyle {
        border_top_left_radius: top_left,
        border_top_right_radius: top_right,
        border_bottom_right_radius: bottom_right,
        border_bottom_left_radius: bottom_left,
        box_shadow: Some(shadows.into_iter().collect::<Vec<_>>().into_boxed_slice()),
        ..Default::default()
    }
}

fn corner(x_px: f32, y_px: f32) -> SpacePair<Length> {
    SpacePair {
        x: Length::Px(x_px),
        y: Length::Px(y_px),
    }
}

fn compositing_style(style: &ComputedStyle) -> TakumiCompositingStyle {
    TakumiCompositingStyle::from_computed_style(style, &sizing_context(), current_color())
}

#[test]
fn one_outer_shadow_lowers_to_view_box_shadow_list() {
    let style = computed_style_with_radius_and_shadows(
        6.0,
        [takumi_shadow(
            false,
            4.0,
            8.0,
            12.0,
            3.0,
            takumi_color(10, 20, 30, 180),
        )],
    );

    let lowered = compositing_style(&style);

    assert_eq!(
        lowered.effects.box_shadows.shadows(),
        &[ViewBoxShadow::outer(
            4.0,
            8.0,
            12.0,
            3.0,
            6.0,
            ui_color(10, 20, 30, 180),
        )]
    );
    assert!(
        lowered
            .effects
            .requirements()
            .contains(ViewCompositingEffectClass::BoxShadow)
    );
}

#[test]
fn four_different_corner_radii_lower_to_typed_shadow_radius_contract() {
    let style = computed_style_with_corner_radii_and_shadows(
        &[
            corner(4.0, 5.0),
            corner(8.0, 9.0),
            corner(12.0, 13.0),
            corner(16.0, 17.0),
        ],
        [takumi_shadow(
            false,
            4.0,
            8.0,
            12.0,
            3.0,
            takumi_color(10, 20, 30, 180),
        )],
    );

    let lowered = compositing_style(&style);
    let shadow = lowered.effects.box_shadows.shadows()[0];

    assert_eq!(
        shadow.border_radii,
        ViewBoxShadowRadii::from_corners(
            ViewBoxShadowCornerRadius::new(4.0, 5.0),
            ViewBoxShadowCornerRadius::new(8.0, 9.0),
            ViewBoxShadowCornerRadius::new(12.0, 13.0),
            ViewBoxShadowCornerRadius::new(16.0, 17.0),
        )
    );
}

#[test]
fn elliptical_corner_radii_lower_without_scalar_collapse() {
    let style = computed_style_with_corner_radii_and_shadows(
        &[
            corner(18.0, 6.0),
            corner(10.0, 22.0),
            corner(14.0, 8.0),
            corner(30.0, 12.0),
        ],
        [takumi_shadow(
            true,
            0.0,
            2.0,
            6.0,
            1.0,
            takumi_color(0, 0, 0, 192),
        )],
    );

    let lowered = compositing_style(&style);
    let shadow = lowered.effects.box_shadows.shadows()[0];

    assert_eq!(shadow.kind, ViewBoxShadowKind::Inset);
    assert_eq!(
        shadow.border_radii.top_left,
        ViewBoxShadowCornerRadius::new(18.0, 6.0)
    );
    assert_ne!(shadow.border_radii, ViewBoxShadowRadii::uniform(6.0));
    assert_ne!(shadow.border_radii, ViewBoxShadowRadii::uniform(18.0));
}

#[test]
fn multiple_shadows_preserve_css_order_and_plan_back_to_front() {
    let style = computed_style_with_radius_and_shadows(
        4.0,
        [
            takumi_shadow(false, 1.0, 0.0, 2.0, 0.0, takumi_color(0, 0, 0, 96)),
            takumi_shadow(false, 2.0, 0.0, 3.0, 0.0, takumi_color(0, 0, 0, 128)),
            takumi_shadow(false, 3.0, 0.0, 4.0, 0.0, takumi_color(0, 0, 0, 160)),
        ],
    );

    let lowered = compositing_style(&style);

    assert_eq!(
        lowered
            .effects
            .box_shadows
            .shadows()
            .iter()
            .map(|shadow| shadow.offset_x_px)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );

    let plan = ViewBoxShadowPassPlan::from_shadows(
        &lowered.effects.box_shadows,
        HitRect::new(0.0, 0.0, 80.0, 40.0),
    )
    .expect("outer shadows plan");

    assert_eq!(
        plan.passes()
            .iter()
            .map(|pass| pass.shadow_index)
            .collect::<Vec<_>>(),
        vec![2, 1, 0]
    );
}

#[test]
fn negative_spread_lowers_and_plans_deterministically() {
    let style = computed_style_with_radius_and_shadows(
        8.0,
        [takumi_shadow(
            false,
            0.0,
            0.0,
            4.0,
            -3.0,
            takumi_color(20, 30, 40, 180),
        )],
    );

    let lowered = compositing_style(&style);
    let shadow = lowered.effects.box_shadows.shadows()[0];

    assert_eq!(shadow.spread_radius_px.to_bits(), (-3.0_f32).to_bits());

    let plan = ViewBoxShadowPassPlan::from_shadows(
        &lowered.effects.box_shadows,
        HitRect::new(0.0, 0.0, 20.0, 20.0),
    )
    .expect("negative spread plans");

    assert_eq!(
        plan.passes()[0].shadow_rect,
        HitRect::new(3.0, 3.0, 14.0, 14.0)
    );
}

#[test]
fn transparent_shadow_canonicalizes_to_empty_list() {
    let style = computed_style_with_radius_and_shadows(
        5.0,
        [takumi_shadow(
            false,
            8.0,
            10.0,
            12.0,
            2.0,
            takumi_color(0, 0, 0, 0),
        )],
    );

    let lowered = compositing_style(&style);

    assert!(lowered.effects.box_shadows.shadows().is_empty());
}

#[test]
fn inset_shadow_reaches_renderer_plan_as_typed_inset_pass() {
    let style = computed_style_with_radius_and_shadows(
        6.0,
        [takumi_shadow(
            true,
            0.0,
            2.0,
            6.0,
            0.0,
            takumi_color(0, 0, 0, 192),
        )],
    );

    let lowered = compositing_style(&style);

    assert_eq!(
        lowered.effects.box_shadows.shadows()[0].kind,
        ViewBoxShadowKind::Inset
    );

    let plan = ViewBoxShadowPassPlan::from_shadows(
        &lowered.effects.box_shadows,
        HitRect::new(0.0, 0.0, 80.0, 40.0),
    )
    .expect("seq06.13e renderer accepts typed inset shadows");

    assert_eq!(plan.passes()[0].shadow.kind, ViewBoxShadowKind::Inset);
    assert!(plan.visual_inset_px() > 0.0);
}

#[test]
fn filter_drop_shadow_remains_distinct_from_css_box_shadow() {
    let style = ComputedStyle {
        filter: vec![TakumiFilter::DropShadow(
            TakumiTextShadow::builder()
                .offset_x(Length::Px(3.0))
                .offset_y(Length::Px(5.0))
                .blur_radius(Length::Px(7.0))
                .color(takumi_color(1, 2, 3, 200))
                .build(),
        )],
        ..Default::default()
    };

    let lowered = compositing_style(&style);

    assert!(lowered.effects.box_shadows.shadows().is_empty());
    let [
        ViewFilter::DropShadow {
            offset_x_px,
            offset_y_px,
            blur_radius_px,
            color,
        },
    ] = lowered.effects.filters.filters()
    else {
        panic!("drop-shadow should remain a filter pass");
    };
    assert_eq!(
        (*offset_x_px, *offset_y_px, *blur_radius_px),
        (3.0, 5.0, 7.0)
    );
    assert_eq!(*color, ui_color(1, 2, 3, 200));
}

#[test]
fn box_shadow_is_advertised_as_direct_ready_after_typed_lowering() {
    assert!(
        DirectCssSupport::implementation_ready_features().contains(&DirectCssFeature::BoxShadow)
    );

    let support = DirectCssSupport::diagnose_css(
        ".card { box-shadow: 0 12px 24px 2px rgba(0, 0, 0, 0.24); }",
    );

    assert!(support.is_direct_wgpu_ready());
    assert!(support.diagnostics().is_empty());
}
