use crate::{
    capture::{TakumiCaptureFrame, TakumiCaptureRecord},
    diagnostic::TakumiAdapterError,
    metadata::{TakumiMetadataMap, TakumiPath},
    text::ArcweftTextLayoutBridge,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2, UiBorder, UiClip, UiColorRgba8, UiGradientStop, UiImagePrimitive, UiLinearGradient,
    UiPrimitive, UiPrimitiveRange, UiRoundedRect, UiScene, UiSceneContext, UiSolidRect,
};
use num_traits::ToPrimitive;
use std::{collections::HashMap, rc::Rc, sync::Arc};
use taffy::Size;
use takumi::prelude::{Fonts, ImageSource, Node, StyleSheet, Viewport};
use takumi::unstable::base::{
    context::RenderContext,
    layout::{
        style::{Affine, ComputedStyle, SizingContext},
        tree::{LayoutResults, LayoutTree, RenderNode},
    },
    scene::{NodePaint, PaintItemKind, StackingContextNode, build_stacking_contexts},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirectPaintCatalog {
    entries: Vec<(TakumiPath, DirectBoxPaint)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectBoxPaint {
    pub background: Option<DirectBackground>,
    pub border: Option<DirectBorder>,
    pub clip: Option<DirectClip>,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DirectBackground {
    Solid {
        color: UiColorRgba8,
        radius: f32,
    },
    LinearGradient {
        angle_degrees: f32,
        stops: Vec<UiGradientStop>,
    },
    Image {
        resource_index: u32,
        opacity: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectBorder {
    pub width: f32,
    pub radius: f32,
    pub color: UiColorRgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DirectClip {
    Rect,
    RoundedRect { radius: f32 },
}

pub struct TakumiSceneInput<'a> {
    pub node: Node,
    pub stylesheet: StyleSheet,
    pub metadata: TakumiMetadataMap,
    pub direct_paint: &'a DirectPaintCatalog,
    pub text: &'a ArcweftTextLayoutBridge,
    pub fonts: &'a Fonts,
    pub images: HashMap<Arc<str>, ImageSource>,
    pub viewport: Viewport,
    pub time_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TakumiSceneOutput {
    pub scene: UiScene,
    pub capture: TakumiCaptureFrame,
}

#[derive(Clone, Debug, Default)]
pub struct TakumiSceneLowerer;

#[derive(Default)]
struct UiSceneBuild {
    viewport_width: f32,
    viewport_height: f32,
    primitives: Vec<UiPrimitive>,
    contexts: Vec<UiSceneContext>,
    capture: TakumiCaptureFrame,
}

impl DirectPaintCatalog {
    pub fn insert(&mut self, path: TakumiPath, paint: DirectBoxPaint) {
        self.entries.push((path, paint));
    }

    pub fn get(&self, path: &TakumiPath) -> Option<&DirectBoxPaint> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, paint)| paint)
    }

    pub fn entries(&self) -> &[(TakumiPath, DirectBoxPaint)] {
        &self.entries
    }
}

impl DirectBoxPaint {
    pub fn new() -> Self {
        Self {
            background: None,
            border: None,
            clip: None,
            opacity: 1.0,
        }
    }

    #[must_use]
    pub fn with_background(mut self, background: DirectBackground) -> Self {
        self.background = Some(background);
        self
    }

    #[must_use]
    pub fn with_border(mut self, border: DirectBorder) -> Self {
        self.border = Some(border);
        self
    }

    #[must_use]
    pub fn with_clip(mut self, clip: DirectClip) -> Self {
        self.clip = Some(clip);
        self
    }

    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

impl Default for DirectBoxPaint {
    fn default() -> Self {
        Self::new()
    }
}

impl TakumiSceneLowerer {
    pub fn lower(input: TakumiSceneInput<'_>) -> Result<TakumiSceneOutput, TakumiAdapterError> {
        let render_context = RenderContext::builder()
            .fonts(input.fonts.snapshot_with_fallbacks(None))
            .sizing(SizingContext::builder().viewport(input.viewport).build())
            .images(Rc::new(input.images))
            .stylesheet(Rc::new(input.stylesheet))
            .time_ms(input.time_ms)
            .draw_debug_border(false)
            .style(Box::<ComputedStyle>::default())
            .build();

        let root = RenderNode::from_node(&render_context, input.node);
        let mut tree = LayoutTree::from_render_node(&root);
        tree.compute_layout(input.viewport.into());
        let layout_results = tree.into_results();
        let root_node_id = layout_results.root_node_id();
        let contexts = build_stacking_contexts(
            &root,
            &layout_results,
            root_node_id,
            Affine::IDENTITY,
            Size {
                width: input.viewport.size.width.map(viewport_dimension_to_f32),
                height: input.viewport.size.height.map(viewport_dimension_to_f32),
            },
        )
        .map_err(|error| TakumiAdapterError::scene_extraction(error.to_string()))?;

        let mut build = UiSceneBuild::new(
            viewport_dimension_to_f32(input.viewport.size.width.unwrap_or_default()),
            viewport_dimension_to_f32(input.viewport.size.height.unwrap_or_default()),
        );
        lower_context(
            0,
            &contexts,
            &layout_results,
            &input.metadata,
            input.direct_paint,
            input.text,
            &mut build,
        )?;
        Ok(build.finish())
    }
}

fn viewport_dimension_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

impl UiSceneBuild {
    fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            primitives: Vec::new(),
            contexts: Vec::new(),
            capture: TakumiCaptureFrame::default(),
        }
    }

    fn primitive_start(&self) -> Result<u32, TakumiAdapterError> {
        u32::try_from(self.primitives.len()).map_err(|_| TakumiAdapterError::CapacityExceeded)
    }

    fn push_primitive(&mut self, primitive: UiPrimitive) {
        self.primitives.push(primitive);
    }

    fn push_context(&mut self, context: UiSceneContext) {
        self.contexts.push(context);
    }

    fn finish(self) -> TakumiSceneOutput {
        let mut scene = UiScene::new(self.viewport_width, self.viewport_height);
        for primitive in self.primitives {
            scene.push_primitive(primitive);
        }
        for context in self.contexts {
            scene.push_context(context);
        }
        TakumiSceneOutput {
            scene,
            capture: self.capture,
        }
    }
}

fn lower_context(
    context_id: usize,
    contexts: &[StackingContextNode],
    layout_results: &LayoutResults,
    metadata: &TakumiMetadataMap,
    direct_paint: &DirectPaintCatalog,
    text: &ArcweftTextLayoutBridge,
    build: &mut UiSceneBuild,
) -> Result<(), TakumiAdapterError> {
    let Some(context) = contexts.get(context_id) else {
        return Ok(());
    };
    if let Some(root) = context.root() {
        lower_node(root, layout_results, metadata, direct_paint, text, build)?;
    }
    for bucket in context.in_paint_order() {
        for item in bucket {
            match &item.kind {
                PaintItemKind::Node(node) => {
                    lower_node(node, layout_results, metadata, direct_paint, text, build)?;
                }
                PaintItemKind::Context(child_context) => {
                    lower_context(
                        *child_context,
                        contexts,
                        layout_results,
                        metadata,
                        direct_paint,
                        text,
                        build,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn lower_node(
    node: &NodePaint,
    layout_results: &LayoutResults,
    metadata: &TakumiMetadataMap,
    direct_paint: &DirectPaintCatalog,
    text: &ArcweftTextLayoutBridge,
    build: &mut UiSceneBuild,
) -> Result<(), TakumiAdapterError> {
    let layout = layout_results
        .layout(node.node_id)
        .map_err(|error| TakumiAdapterError::scene_extraction(error.to_string()))?;
    let path = TakumiPath::from(node.path.clone());
    let bounds = HitRect::new(0.0, 0.0, layout.size.width, layout.size.height);
    let transform = affine_to_ui(node.transform.to_cols_array());
    let paint = direct_paint.get(&path);
    let start = build.primitive_start()?;

    if let Some(paint) = paint {
        if let Some(background) = &paint.background {
            lower_background(background, bounds, build);
        }
        if let Some(border) = paint.border {
            build.push_primitive(UiPrimitive::Border(UiBorder {
                bounds,
                radius: border.radius,
                width: border.width,
                color: border.color,
            }));
        }
    }

    if let Some(metadata) = metadata.get_by_path(&path)
        && let Some(participant) = text.get(metadata.node())
    {
        for glyph_run in participant.glyph_runs() {
            build.push_primitive(glyph_run.clone().into_primitive());
        }
    }

    let end = build.primitive_start()?;
    if start == end {
        return Ok(());
    }

    let clip = paint.and_then(|paint| paint.clip.map(|clip| clip.to_ui_clip(bounds)));
    let opacity = paint.map_or(1.0, |paint| paint.opacity);
    let primitive_range = UiPrimitiveRange { start, end };
    build.push_context(UiSceneContext {
        transform,
        opacity,
        clip: clip.clone(),
        primitive_range,
    });
    if let Some(metadata) = metadata.get_by_path(&path) {
        build.capture.push(TakumiCaptureRecord::new(
            metadata.clone(),
            primitive_range,
            bounds,
            transform,
            clip,
        ));
    }
    Ok(())
}

fn lower_background(background: &DirectBackground, bounds: HitRect, build: &mut UiSceneBuild) {
    match background {
        DirectBackground::Solid { color, radius } if *radius > 0.0 => {
            build.push_primitive(UiPrimitive::RoundedRect(UiRoundedRect {
                bounds,
                radius: *radius,
                color: *color,
            }));
        }
        DirectBackground::Solid { color, .. } => {
            build.push_primitive(UiPrimitive::SolidRect(UiSolidRect {
                bounds,
                color: *color,
            }));
        }
        DirectBackground::LinearGradient {
            angle_degrees,
            stops,
        } => {
            build.push_primitive(UiPrimitive::LinearGradient(UiLinearGradient {
                bounds,
                angle_degrees: *angle_degrees,
                stops: stops.clone(),
            }));
        }
        DirectBackground::Image {
            resource_index,
            opacity,
        } => {
            build.push_primitive(UiPrimitive::Image(UiImagePrimitive {
                resource_index: *resource_index,
                bounds,
                opacity: *opacity,
            }));
        }
    }
}

impl DirectClip {
    fn to_ui_clip(self, bounds: HitRect) -> UiClip {
        match self {
            Self::Rect => UiClip::Rect(bounds),
            Self::RoundedRect { radius } => UiClip::RoundedRect { bounds, radius },
        }
    }
}

fn affine_to_ui(values: [f32; 6]) -> UiAffine2 {
    UiAffine2 {
        m11: values[0],
        m12: values[1],
        m21: values[2],
        m22: values[3],
        tx: values[4],
        ty: values[5],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(alpha: u8) -> UiColorRgba8 {
        UiColorRgba8 {
            red: 10,
            green: 20,
            blue: 30,
            alpha,
        }
    }

    #[test]
    fn direct_paint_catalog_returns_path_specific_paint() {
        let path = TakumiPath::root().child(1);
        let mut catalog = DirectPaintCatalog::default();
        catalog.insert(
            path.clone(),
            DirectBoxPaint::new().with_background(DirectBackground::Solid {
                color: color(255),
                radius: 4.0,
            }),
        );

        assert!(catalog.get(&path).is_some());
        assert!(catalog.get(&TakumiPath::root()).is_none());
    }
}
