use arcweft_player_scene::fonts::DEFAULT_PLAYER_FONT_RESOURCE_BYTES;
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, RenderScene, SharedFramePlanContext,
};

pub fn prepare(scene: &RenderScene) -> Result<PreparedFrame, FramePlanError> {
    let mut planner = SharedFramePlanContext::new();
    for bytes in DEFAULT_PLAYER_FONT_RESOURCE_BYTES {
        planner.register_font_bytes(bytes.to_vec())?;
    }
    planner.prepare(scene)
}
