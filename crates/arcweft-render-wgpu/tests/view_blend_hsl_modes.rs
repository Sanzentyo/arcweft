use arcweft_render_wgpu::view_blend::{
    ViewBlendPassPlan, ViewBlendShaderMode, supported_blend_modes,
};
use arcweft_render_wgpu::view_scene::ViewBlendMode;

#[test]
fn hsl_family_blend_modes_are_planned_for_shader_execution() {
    let cases = [
        (ViewBlendMode::Hue, ViewBlendShaderMode::Hue),
        (ViewBlendMode::Saturation, ViewBlendShaderMode::Saturation),
        (ViewBlendMode::Color, ViewBlendShaderMode::Color),
        (ViewBlendMode::Luminosity, ViewBlendShaderMode::Luminosity),
    ];

    for (mode, shader_mode) in cases {
        let plan = ViewBlendPassPlan::from_mode(mode).expect("mode is production-supported");
        assert_eq!(plan.shader_mode, shader_mode);
        assert!(plan.samples_backdrop);
    }
}

#[test]
fn supported_blend_modes_matrix_includes_css_color_family() {
    for mode in [
        ViewBlendMode::Hue,
        ViewBlendMode::Saturation,
        ViewBlendMode::Color,
        ViewBlendMode::Luminosity,
    ] {
        assert!(
            supported_blend_modes().contains(&mode),
            "{mode:?} is listed"
        );
    }
}
