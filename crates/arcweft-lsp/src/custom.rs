/// Arcweft-specific LSP extension requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcweftCustomRequest {
    GetNodeAtPosition,
    GetGraphSlice,
    GetNodeHistory,
    PreviewGraphPatch,
    ApplyGraphPatch,
    GetRagContext,
    RenderRouteMap,
    ParseInput,
    ShaderPreview,
    AudioCuePreview,
    ReplCommand,
}

impl ArcweftCustomRequest {
    /// Stable request method sent over LSP.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GetNodeAtPosition => "arcweft/getNodeAtPosition",
            Self::GetGraphSlice => "arcweft/getGraphSlice",
            Self::GetNodeHistory => "arcweft/getNodeHistory",
            Self::PreviewGraphPatch => "arcweft/previewGraphPatch",
            Self::ApplyGraphPatch => "arcweft/applyGraphPatch",
            Self::GetRagContext => "arcweft/getRagContext",
            Self::RenderRouteMap => "arcweft/renderRouteMap",
            Self::ParseInput => "arcweft/parseInput",
            Self::ShaderPreview => "arcweft/shaderPreview",
            Self::AudioCuePreview => "arcweft/audioCuePreview",
            Self::ReplCommand => "arcweft/replCommand",
        }
    }
}
