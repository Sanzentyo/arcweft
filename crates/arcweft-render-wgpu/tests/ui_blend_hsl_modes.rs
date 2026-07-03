use arcweft_render_wgpu::ui_blend::{UiBlendPassPlan, UiBlendShaderMode, supported_blend_modes};
use arcweft_render_wgpu::ui_scene::UiBlendMode;

#[test]
fn hsl_family_blend_modes_are_planned_for_shader_execution() {
    let cases = [
        (UiBlendMode::Hue, UiBlendShaderMode::Hue),
        (UiBlendMode::Saturation, UiBlendShaderMode::Saturation),
        (UiBlendMode::Color, UiBlendShaderMode::Color),
        (UiBlendMode::Luminosity, UiBlendShaderMode::Luminosity),
    ];

    for (mode, shader_mode) in cases {
        let plan = UiBlendPassPlan::from_mode(mode).expect("mode is production-supported");
        assert_eq!(plan.shader_mode, shader_mode);
        assert!(plan.samples_backdrop);
    }
}

#[test]
fn supported_blend_modes_matrix_includes_css_color_family() {
    for mode in [
        UiBlendMode::Hue,
        UiBlendMode::Saturation,
        UiBlendMode::Color,
        UiBlendMode::Luminosity,
    ] {
        assert!(
            supported_blend_modes().contains(&mode),
            "{mode:?} is listed"
        );
    }
}
