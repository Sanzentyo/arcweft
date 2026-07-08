//! Takumi computed-style to Arcweft direct-paint extraction.
//!
//! This module is intentionally an adapter-layer contract. It consumes Takumi
//! computed style and emits Arcweft-owned `DirectPaintCatalog` data that the
//! existing `TakumiSceneLowerer` can lower into `ViewScene` primitives. It does
//! not render, rasterize, read files, fetch URLs, or allocate GPU resources.

use crate::{
    diagnostic::{TakumiDiagnostic, TakumiDiagnosticCode},
    lowering::{DirectBoxPaint, DirectPaintCatalog},
    metadata::{ArcweftNodeMetadata, TakumiMetadataMap, TakumiPath},
};
use arcweft_render_wgpu::view_scene::{
    ViewColorRgba8, ViewCornerRadii, ViewCornerRadius, ViewGradientStop, ViewPrimitiveRange,
    ViewSurfaceBackground, ViewSurfaceBorder, ViewSurfaceClip,
};
use num_traits::ToPrimitive;
use std::{collections::BTreeMap, sync::Arc};
use takumi::unstable::base::layout::{
    style::{
        BackgroundImage, BorderStyle, Color as TakumiColor, ComputedStyle, GradientStop, Length,
        LineWidth, LinearGradient, LinearGradientDirection, SizingContext, SpacePair,
    },
    tree::RenderNode,
};

/// Input for deterministic computed-style direct-paint extraction.
#[derive(Clone, Copy)]
pub struct ComputedDirectPaintInput<'a> {
    /// Takumi render tree after CSS cascade and computed style creation.
    pub root: &'a RenderNode,
    /// Arcweft metadata keyed by Takumi render path.
    pub metadata: &'a TakumiMetadataMap,
    /// Stable resource references supplied by adapter/player layers.
    pub resources: &'a DirectPaintResourceTable,
}

/// Complete direct-paint extraction output for one Takumi render tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputedDirectPaintFrame {
    /// Catalog consumed by `TakumiSceneLowerer`.
    pub catalog: DirectPaintCatalog,
    /// Structured diagnostics for values outside the direct-wgpu subset.
    pub diagnostics: Vec<TakumiDiagnostic>,
    /// Evidence records keyed by Takumi path and Arcweft metadata.
    pub evidence: DirectPaintEvidenceFrame,
    /// Resource references that must be fulfilled outside this adapter crate.
    pub resource_requirements: Vec<DirectPaintResourceRequirement>,
}

/// Stable resource table for image backgrounds.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectPaintResourceTable {
    entries: BTreeMap<Arc<str>, u32>,
}

/// A resource URL required by direct paint extraction but not present in the
/// resource table supplied by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectPaintResourceRequirement {
    path: TakumiPath,
    url: Arc<str>,
}

/// Evidence for all extracted direct-paint records in one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirectPaintEvidenceFrame {
    records: Vec<DirectPaintEvidenceRecord>,
}

/// Evidence for one Takumi path.
#[derive(Clone, Debug, PartialEq)]
pub struct DirectPaintEvidenceRecord {
    path: TakumiPath,
    metadata: Option<ArcweftNodeMetadata>,
    layers: Vec<DirectPaintLayerEvidence>,
    primitive_range: Option<ViewPrimitiveRange>,
}

/// Evidence for one extracted paint layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectPaintLayerEvidence {
    kind: DirectPaintLayerKind,
    source: DirectPaintSource,
}

/// Kind of direct-paint layer extracted from computed style.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPaintLayerKind {
    BackgroundColor,
    LinearGradientBackground,
    ImageBackground,
    Border,
    RoundedClip,
    Opacity,
}

/// Source family for an extracted paint layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPaintSource {
    CssComputedStyle,
    ResourceTable,
}

/// Deterministic Takumi computed-style extractor.
#[derive(Clone, Debug, Default)]
pub struct ComputedDirectPaintExtractor;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BorderExtractionState {
    None,
    Supported,
    Unsupported,
}

impl ComputedDirectPaintExtractor {
    /// Extracts direct paint from a Takumi render tree into a catalog consumed by
    /// the existing `TakumiSceneLowerer`.
    pub fn extract(input: ComputedDirectPaintInput<'_>) -> ComputedDirectPaintFrame {
        let mut frame = ComputedDirectPaintFrame::default();
        let root = TakumiPath::root();
        Self::extract_node(
            &root,
            input.root,
            input.metadata,
            input.resources,
            &mut frame,
        );
        frame
    }

    fn extract_node(
        path: &TakumiPath,
        render_node: &RenderNode,
        metadata: &TakumiMetadataMap,
        resources: &DirectPaintResourceTable,
        frame: &mut ComputedDirectPaintFrame,
    ) {
        let style = render_node.context.style.as_ref();
        let sizing = &render_node.context.sizing;
        let current_color = render_node.context.current_color;
        let mut paint = DirectBoxPaint::new().with_opacity(style.opacity.0.clamp(0.0, 1.0));
        let mut evidence =
            DirectPaintEvidenceRecord::new(path.clone(), metadata.get_by_path(path).cloned());

        if paint.opacity < 1.0 {
            evidence.push_layer(
                DirectPaintLayerKind::Opacity,
                DirectPaintSource::CssComputedStyle,
            );
        }

        let radii = direct_corner_radii(style, sizing);
        let uniform_radius = radii.uniform_circular_radius();
        if let Some(radius) = uniform_radius.filter(|value| *value > 0.0) {
            paint = paint.with_clip(ViewSurfaceClip::RoundedRect { radius });
            evidence.push_layer(
                DirectPaintLayerKind::RoundedClip,
                DirectPaintSource::CssComputedStyle,
            );
        }

        if let Some(background) = solid_background(style, current_color, radii) {
            paint = paint.with_background(background);
            evidence.push_layer(
                DirectPaintLayerKind::BackgroundColor,
                DirectPaintSource::CssComputedStyle,
            );
        }

        for background in
            supported_background_images(style, sizing, current_color, path, resources, frame)
        {
            let layer_kind = match background {
                ViewSurfaceBackground::LinearGradient { .. } => {
                    DirectPaintLayerKind::LinearGradientBackground
                }
                ViewSurfaceBackground::Image { .. } => DirectPaintLayerKind::ImageBackground,
                ViewSurfaceBackground::Solid { .. } => DirectPaintLayerKind::BackgroundColor,
            };
            paint = paint.with_background(background);
            evidence.push_layer(layer_kind, DirectPaintSource::CssComputedStyle);
        }

        if let (BorderExtractionState::Supported, Some(border)) = supported_border(
            style,
            sizing,
            current_color,
            uniform_radius.unwrap_or_default(),
            path,
            frame,
        ) {
            paint = paint.with_border(border);
            evidence.push_layer(
                DirectPaintLayerKind::Border,
                DirectPaintSource::CssComputedStyle,
            );
        }

        if paint.has_visible_direct_paint() {
            frame.catalog.insert(path.clone(), paint);
            frame.evidence.push(evidence);
        }

        if let Some(children) = render_node.children.as_deref() {
            for (index, child) in children.iter().enumerate() {
                Self::extract_node(&path.child(index), child, metadata, resources, frame);
            }
        }
    }
}

impl DirectPaintResourceTable {
    /// Creates a deterministic resource table from `(url, resource_index)` pairs.
    pub fn new(entries: impl IntoIterator<Item = (impl Into<Arc<str>>, u32)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(url, index)| (url.into(), index))
                .collect(),
        }
    }

    /// Returns the resource index for a URL when the adapter/player layer has
    /// already provided it.
    pub fn get(&self, url: &str) -> Option<u32> {
        self.entries.get(url).copied()
    }

    /// Returns all known resource mappings in stable URL order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, u32)> {
        self.entries
            .iter()
            .map(|(url, index)| (url.as_ref(), *index))
    }
}

impl DirectPaintResourceRequirement {
    pub fn new(path: TakumiPath, url: Arc<str>) -> Self {
        Self { path, url }
    }

    pub fn path(&self) -> &TakumiPath {
        &self.path
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl DirectPaintEvidenceFrame {
    pub fn push(&mut self, record: DirectPaintEvidenceRecord) {
        self.records.push(record);
    }

    pub fn records(&self) -> &[DirectPaintEvidenceRecord] {
        &self.records
    }

    /// Attaches a primitive range observed during `TakumiSceneLowerer` capture.
    /// This keeps extraction evidence separate from renderer integration while
    /// still letting seq06.11b join paint decisions to final frame primitives.
    pub fn attach_primitive_range(
        &mut self,
        path: &TakumiPath,
        primitive_range: ViewPrimitiveRange,
    ) -> bool {
        if let Some(record) = self.records.iter_mut().find(|record| record.path() == path) {
            record.primitive_range = Some(primitive_range);
            return true;
        }
        false
    }
}

impl DirectPaintEvidenceRecord {
    pub fn new(path: TakumiPath, metadata: Option<ArcweftNodeMetadata>) -> Self {
        Self {
            path,
            metadata,
            layers: Vec::new(),
            primitive_range: None,
        }
    }

    pub fn path(&self) -> &TakumiPath {
        &self.path
    }

    pub fn metadata(&self) -> Option<&ArcweftNodeMetadata> {
        self.metadata.as_ref()
    }

    pub fn layers(&self) -> &[DirectPaintLayerEvidence] {
        &self.layers
    }

    pub fn primitive_range(&self) -> Option<ViewPrimitiveRange> {
        self.primitive_range
    }

    fn push_layer(&mut self, kind: DirectPaintLayerKind, source: DirectPaintSource) {
        self.layers.push(DirectPaintLayerEvidence { kind, source });
    }
}

impl DirectPaintLayerEvidence {
    pub fn kind(&self) -> DirectPaintLayerKind {
        self.kind
    }

    pub fn source(&self) -> DirectPaintSource {
        self.source
    }
}

fn solid_background(
    style: &ComputedStyle,
    current_color: TakumiColor,
    radii: ViewCornerRadii,
) -> Option<ViewSurfaceBackground> {
    let color = view_color(style.background_color.resolve(current_color));
    (color.alpha > 0).then_some(ViewSurfaceBackground::Solid { color, radii })
}

fn supported_background_images(
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: TakumiColor,
    path: &TakumiPath,
    resources: &DirectPaintResourceTable,
    frame: &mut ComputedDirectPaintFrame,
) -> Vec<ViewSurfaceBackground> {
    let Some(images) = style.background_image.as_deref() else {
        return Vec::new();
    };

    images
        .iter()
        .rev()
        .filter_map(|image| {
            supported_background_image(image, sizing, current_color, path, resources, frame)
        })
        .collect()
}

fn supported_background_image(
    image: &BackgroundImage,
    sizing: &SizingContext,
    current_color: TakumiColor,
    path: &TakumiPath,
    resources: &DirectPaintResourceTable,
    frame: &mut ComputedDirectPaintFrame,
) -> Option<ViewSurfaceBackground> {
    match image {
        BackgroundImage::None => None,
        BackgroundImage::Linear(gradient) => {
            linear_gradient_background(gradient, sizing, current_color, path, frame)
        }
        BackgroundImage::Url(url) => {
            if let Some(resource_index) = resources.get(url) {
                return Some(ViewSurfaceBackground::Image {
                    resource_index,
                    opacity: 1.0,
                });
            }
            frame
                .resource_requirements
                .push(DirectPaintResourceRequirement::new(
                    path.clone(),
                    url.clone(),
                ));
            frame.diagnostics.push(unsupported(
                path,
                format!("background-image: missing resource `{url}`"),
            ));
            None
        }
        BackgroundImage::Radial(_) => {
            frame
                .diagnostics
                .push(unsupported(path, "background-image: radial-gradient"));
            None
        }
        BackgroundImage::Conic(_) => {
            frame
                .diagnostics
                .push(unsupported(path, "background-image: conic-gradient"));
            None
        }
    }
}

fn linear_gradient_background(
    gradient: &LinearGradient,
    sizing: &SizingContext,
    current_color: TakumiColor,
    path: &TakumiPath,
    frame: &mut ComputedDirectPaintFrame,
) -> Option<ViewSurfaceBackground> {
    if gradient.repeating {
        frame.diagnostics.push(unsupported(
            path,
            "background-image: repeating-linear-gradient",
        ));
        return None;
    }

    let stops = gradient_stops(&gradient.stops, sizing, current_color, path, frame)?;
    Some(ViewSurfaceBackground::LinearGradient {
        angle_degrees: gradient_angle_degrees(gradient.direction),
        stops,
    })
}

fn gradient_stops(
    stops: &[GradientStop],
    sizing: &SizingContext,
    current_color: TakumiColor,
    path: &TakumiPath,
    frame: &mut ComputedDirectPaintFrame,
) -> Option<Vec<ViewGradientStop>> {
    let color_stop_count = stops
        .iter()
        .filter(|stop| matches!(stop, GradientStop::ColorHint { .. }))
        .count();
    if color_stop_count < 2 {
        frame.diagnostics.push(unsupported(
            path,
            "linear-gradient: fewer than two color stops",
        ));
        return None;
    }

    let mut color_index = 0usize;
    let mut result = Vec::with_capacity(color_stop_count);
    for stop in stops {
        match stop {
            GradientStop::ColorHint { color, hint } => {
                let fallback_offset = fallback_gradient_offset(color_index, color_stop_count);
                let offset = match hint {
                    Some(hint) => {
                        if let Some(value) = stop_position_to_offset(hint.0, sizing) {
                            value
                        } else {
                            frame.diagnostics.push(unsupported(
                                path,
                                "linear-gradient: non-normalizable stop position",
                            ));
                            return None;
                        }
                    }
                    None => fallback_offset,
                };
                result.push(ViewGradientStop {
                    offset: offset.clamp(0.0, 1.0),
                    color: view_color(color.resolve(current_color)),
                });
                color_index += 1;
            }
            GradientStop::Hint(_) => {
                frame.diagnostics.push(unsupported(
                    path,
                    "linear-gradient: color hint without color stop",
                ));
                return None;
            }
            _ => {
                frame.diagnostics.push(unsupported(
                    path,
                    "linear-gradient: unsupported gradient stop",
                ));
                return None;
            }
        }
    }

    Some(result)
}

fn fallback_gradient_offset(index: usize, len: usize) -> f32 {
    if len <= 1 {
        return 0.0;
    }
    let Some(index) = index.to_f32() else {
        return 1.0;
    };
    let Some(denominator) = (len - 1).to_f32() else {
        return 1.0;
    };
    index / denominator
}

fn stop_position_to_offset(length: Length, sizing: &SizingContext) -> Option<f32> {
    match length {
        Length::Percentage(value) => Some(value / 100.0),
        Length::Px(value) if (0.0..=1.0).contains(&value) => Some(value),
        Length::Calc(_) => Some(length.to_px(sizing, 1.0)),
        _ => None,
    }
}

fn gradient_angle_degrees(direction: LinearGradientDirection) -> f32 {
    match direction {
        LinearGradientDirection::Angle(angle) => *angle,
        LinearGradientDirection::Keyword(keyword) => *keyword.to_angle(),
    }
}

fn direct_corner_radii(style: &ComputedStyle, sizing: &SizingContext) -> ViewCornerRadii {
    ViewCornerRadii::from_corners(
        direct_corner_radius(style.border_top_left_radius, sizing),
        direct_corner_radius(style.border_top_right_radius, sizing),
        direct_corner_radius(style.border_bottom_right_radius, sizing),
        direct_corner_radius(style.border_bottom_left_radius, sizing),
    )
}

fn direct_corner_radius(radius: SpacePair<Length>, sizing: &SizingContext) -> ViewCornerRadius {
    ViewCornerRadius::new(
        radius.x.to_px(sizing, 0.0).max(0.0),
        radius.y.to_px(sizing, 0.0).max(0.0),
    )
}

fn supported_border(
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: TakumiColor,
    radius: f32,
    path: &TakumiPath,
    frame: &mut ComputedDirectPaintFrame,
) -> (BorderExtractionState, Option<ViewSurfaceBorder>) {
    let widths = [
        line_width_px(style.border_top_width, sizing),
        line_width_px(style.border_right_width, sizing),
        line_width_px(style.border_bottom_width, sizing),
        line_width_px(style.border_left_width, sizing),
    ];
    let styles = [
        style.border_top_style,
        style.border_right_style,
        style.border_bottom_style,
        style.border_left_style,
    ];
    let colors = [
        view_color(style.border_top_color.resolve(current_color)),
        view_color(style.border_right_color.resolve(current_color)),
        view_color(style.border_bottom_color.resolve(current_color)),
        view_color(style.border_left_color.resolve(current_color)),
    ];

    let visible = widths
        .into_iter()
        .zip(styles)
        .enumerate()
        .filter(|(_, (width, style))| *width > 0.0 && border_style_is_visible(*style))
        .collect::<Vec<_>>();

    if visible.is_empty() {
        return (BorderExtractionState::None, None);
    }

    if visible
        .iter()
        .any(|(_, (_, style))| !matches!(style, BorderStyle::Solid))
    {
        frame
            .diagnostics
            .push(unsupported(path, "border-style: non-solid visible side"));
        return (BorderExtractionState::Unsupported, None);
    }

    let first_width = widths[visible[0].0];
    if visible
        .iter()
        .any(|(side, _)| !same_px(widths[*side], first_width))
    {
        frame
            .diagnostics
            .push(unsupported(path, "border-width: mixed visible side widths"));
        return (BorderExtractionState::Unsupported, None);
    }

    let first_color = colors[visible[0].0];
    if visible.iter().any(|(side, _)| colors[*side] != first_color) {
        frame
            .diagnostics
            .push(unsupported(path, "border-color: mixed visible side colors"));
        return (BorderExtractionState::Unsupported, None);
    }

    (
        BorderExtractionState::Supported,
        Some(ViewSurfaceBorder {
            width: first_width,
            radius,
            color: first_color,
        }),
    )
}

fn border_style_is_visible(style: BorderStyle) -> bool {
    style.is_rendered()
}

fn line_width_px(width: LineWidth, sizing: &SizingContext) -> f32 {
    Length::from(width).to_px(sizing, 0.0).max(0.0)
}

fn view_color(color: TakumiColor) -> ViewColorRgba8 {
    let [red, green, blue, alpha] = color.0;
    ViewColorRgba8 {
        red,
        green,
        blue,
        alpha,
    }
}

fn unsupported(path: &TakumiPath, message: impl Into<String>) -> TakumiDiagnostic {
    TakumiDiagnostic::new(TakumiDiagnosticCode::UnsupportedDirectCss, message)
        .with_path(path.clone())
}

fn same_px(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.001
}
