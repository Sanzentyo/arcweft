use super::compositing::ViewPaintNode;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_view::{TextEditorPart, TextFieldVisualBuffer};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewScene {
    viewport_width: f32,
    viewport_height: f32,
    contexts: Vec<ViewSceneContext>,
    primitives: Vec<ViewPrimitive>,
    paint_nodes: Vec<ViewPaintNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewSceneContext {
    pub transform: ViewAffine2D,
    pub opacity: f32,
    pub clip: Option<ViewClip>,
    pub primitive_range: ViewPrimitiveRange,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewPrimitiveRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewPrimitive {
    SolidRect(ViewSolidRect),
    RoundedRect(ViewRoundedRect),
    Border(ViewBorder),
    LinearGradient(ViewLinearGradient),
    Image(ViewImagePrimitive),
    GlyphRun(ViewGlyphRun),
    Selection(ViewSelectionPrimitive),
    Caret(ViewCaretPrimitive),
    CompositionUnderline(ViewCompositionUnderline),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewTextFieldSceneStyle {
    pub background: Option<ViewColorRgba8>,
    pub selection: ViewColorRgba8,
    pub caret: ViewColorRgba8,
    pub composition: ViewColorRgba8,
    pub composition_thickness: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewAffine2D {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewClip {
    Rect(HitRect),
    RoundedRect { bounds: HitRect, radius: f32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewColorRgba8 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewSolidRect {
    pub bounds: HitRect,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewRoundedRect {
    pub bounds: HitRect,
    pub radius: f32,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewBorder {
    pub bounds: HitRect,
    pub radius: f32,
    pub width: f32,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewLinearGradient {
    pub bounds: HitRect,
    pub angle_degrees: f32,
    pub stops: Vec<ViewGradientStop>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewGradientStop {
    pub offset: f32,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewImagePrimitive {
    pub resource_index: u32,
    pub bounds: HitRect,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewGlyphRun {
    pub run_index: u32,
    pub bounds: HitRect,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewSelectionPrimitive {
    pub target: Option<InteractionTarget>,
    pub bounds: HitRect,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewCaretPrimitive {
    pub target: Option<InteractionTarget>,
    pub bounds: HitRect,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewCompositionUnderline {
    pub target: Option<InteractionTarget>,
    pub bounds: HitRect,
    pub color: ViewColorRgba8,
    pub thickness: f32,
    pub style: ViewUnderlineStyle,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewUnderlineStyle {
    #[default]
    Solid,
    Dotted,
    Dashed,
}

impl Default for ViewAffine2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl ViewAffine2D {
    pub const IDENTITY: Self = Self {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        tx: 0.0,
        ty: 0.0,
    };
}

impl Default for ViewTextFieldSceneStyle {
    fn default() -> Self {
        Self {
            background: None,
            selection: ViewColorRgba8 {
                red: 0x33,
                green: 0x99,
                blue: 0xff,
                alpha: 0x66,
            },
            caret: ViewColorRgba8 {
                red: 0xff,
                green: 0xff,
                blue: 0xff,
                alpha: 0xff,
            },
            composition: ViewColorRgba8 {
                red: 0xff,
                green: 0xff,
                blue: 0xff,
                alpha: 0xff,
            },
            composition_thickness: 2.0,
        }
    }
}

impl ViewScene {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            contexts: Vec::new(),
            primitives: Vec::new(),
            paint_nodes: Vec::new(),
        }
    }

    pub fn push_context(&mut self, context: ViewSceneContext) {
        self.paint_nodes
            .push(ViewPaintNode::Direct(context.clone()));
        self.contexts.push(context);
    }

    pub fn push_primitive(&mut self, primitive: ViewPrimitive) {
        self.primitives.push(primitive);
    }

    pub fn push_paint_node(&mut self, node: ViewPaintNode) {
        self.paint_nodes.push(node);
    }

    pub fn replace_paint_nodes(&mut self, paint_nodes: Vec<ViewPaintNode>) {
        self.paint_nodes = paint_nodes;
    }

    pub const fn viewport_width(&self) -> f32 {
        self.viewport_width
    }

    pub const fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    pub fn contexts(&self) -> &[ViewSceneContext] {
        &self.contexts
    }

    pub fn primitives(&self) -> &[ViewPrimitive] {
        &self.primitives
    }

    pub fn paint_nodes(&self) -> &[ViewPaintNode] {
        &self.paint_nodes
    }

    pub fn push_text_field_parts(
        &mut self,
        buffer: &TextFieldVisualBuffer,
        style: &ViewTextFieldSceneStyle,
    ) -> ViewPrimitiveRange {
        let start = u32::try_from(self.primitives.len()).unwrap_or(u32::MAX);
        if let Some(color) = style.background {
            self.push_primitive(ViewPrimitive::SolidRect(ViewSolidRect {
                bounds: buffer.bounds(),
                color,
            }));
        }
        for part in buffer.parts() {
            match part.part() {
                TextEditorPart::Selection => {
                    self.push_primitive(ViewPrimitive::Selection(ViewSelectionPrimitive {
                        target: buffer.target().cloned(),
                        bounds: part.bounds(),
                        color: style.selection,
                    }));
                }
                TextEditorPart::Caret => {
                    self.push_primitive(ViewPrimitive::Caret(ViewCaretPrimitive {
                        target: buffer.target().cloned(),
                        bounds: part.bounds(),
                        color: style.caret,
                    }));
                }
                TextEditorPart::Composition | TextEditorPart::CompositionTarget => {
                    self.push_primitive(ViewPrimitive::CompositionUnderline(
                        ViewCompositionUnderline {
                            target: buffer.target().cloned(),
                            bounds: part.bounds(),
                            color: style.composition,
                            thickness: style.composition_thickness,
                            style: ViewUnderlineStyle::Solid,
                        },
                    ));
                }
                TextEditorPart::Root
                | TextEditorPart::Content
                | TextEditorPart::Placeholder
                | TextEditorPart::Leading
                | TextEditorPart::Trailing
                | TextEditorPart::ClearButton => {}
            }
        }
        let end = u32::try_from(self.primitives.len()).unwrap_or(u32::MAX);
        let range = ViewPrimitiveRange { start, end };
        self.push_context(ViewSceneContext {
            transform: ViewAffine2D::IDENTITY,
            opacity: 1.0,
            clip: Some(ViewClip::Rect(buffer.bounds())),
            primitive_range: range,
        });
        range
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ViewAffine2D, ViewColorRgba8, ViewPrimitive, ViewPrimitiveRange, ViewScene,
        ViewSceneContext, ViewSolidRect, ViewTextFieldSceneStyle,
    };
    use crate::view_scene::ViewPaintNode;
    use arcweft_presentation::hit::HitRect;
    use arcweft_view::{TextEditState, TextEditorPart, TextFieldMetrics};

    #[test]
    fn view_scene_preserves_context_primitive_and_paint_node_order() {
        let mut scene = ViewScene::new(320.0, 180.0);
        scene.push_primitive(ViewPrimitive::SolidRect(ViewSolidRect {
            bounds: HitRect::new(0.0, 0.0, 10.0, 10.0),
            color: ViewColorRgba8 {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 255,
            },
        }));
        scene.push_context(ViewSceneContext {
            transform: ViewAffine2D::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
        });

        assert!((scene.viewport_width() - 320.0).abs() < f32::EPSILON);
        assert!((scene.viewport_height() - 180.0).abs() < f32::EPSILON);
        assert_eq!(scene.primitives().len(), 1);
        assert_eq!(
            scene.contexts()[0].primitive_range,
            ViewPrimitiveRange { start: 0, end: 1 }
        );
        assert!(matches!(scene.paint_nodes()[0], ViewPaintNode::Direct(_)));
    }

    #[test]
    fn text_field_parts_lower_to_selection_caret_and_composition_primitives() {
        let mut state = TextEditState::new("abc");
        state.set_composition(
            arcweft_presentation::text_input::TextCompositionUpdate::new(
                "かな",
                arcweft_presentation::text_input::TextRange::new(
                    arcweft_presentation::text_input::TextByteOffset(0),
                    arcweft_presentation::text_input::TextByteOffset(6),
                ),
            ),
        );
        let buffer = state.visual_buffer(
            None,
            HitRect::new(0.0, 0.0, 120.0, 24.0),
            TextFieldMetrics::default(),
            false,
        );
        assert!(
            buffer
                .parts()
                .iter()
                .any(|part| part.part() == TextEditorPart::Composition)
        );

        let mut scene = ViewScene::new(320.0, 180.0);
        let range = scene.push_text_field_parts(&buffer, &ViewTextFieldSceneStyle::default());

        assert_eq!(range, scene.contexts()[0].primitive_range);
        assert!(matches!(scene.paint_nodes()[0], ViewPaintNode::Direct(_)));
        assert!(
            scene
                .primitives()
                .iter()
                .any(|primitive| matches!(primitive, ViewPrimitive::Caret(_)))
        );
        assert!(
            scene
                .primitives()
                .iter()
                .any(|primitive| { matches!(primitive, ViewPrimitive::CompositionUnderline(_)) })
        );
    }
}
