use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::{ViewColorRgba8, ViewGlyphRun, ViewPrimitive, ViewScene};
use arcweft_view::{NodeId, TextFieldId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineMeasuredSize {
    width: f32,
    height: f32,
    baseline: Option<f32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcweftInlineParticipantKind {
    Text,
    RichText,
    TextField(TextFieldId),
    InlineView,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArcweftGlyphRun {
    run_index: u32,
    bounds: HitRect,
    color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArcweftInlineParticipant {
    node: NodeId,
    kind: ArcweftInlineParticipantKind,
    measured_size: InlineMeasuredSize,
    glyph_runs: Vec<ArcweftGlyphRun>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArcweftTextLayoutBridge {
    participants: Vec<ArcweftInlineParticipant>,
}

impl InlineMeasuredSize {
    pub fn new(width: f32, height: f32, baseline: Option<f32>) -> Self {
        Self {
            width,
            height,
            baseline,
        }
    }

    pub fn width(self) -> f32 {
        self.width
    }

    pub fn height(self) -> f32 {
        self.height
    }

    pub fn baseline(self) -> Option<f32> {
        self.baseline
    }
}

impl ArcweftGlyphRun {
    pub fn new(run_index: u32, bounds: HitRect, color: ViewColorRgba8) -> Self {
        Self {
            run_index,
            bounds,
            color,
        }
    }

    pub fn run_index(&self) -> u32 {
        self.run_index
    }

    pub fn bounds(&self) -> HitRect {
        self.bounds
    }

    pub fn color(&self) -> ViewColorRgba8 {
        self.color
    }

    pub fn into_primitive(self) -> ViewPrimitive {
        ViewPrimitive::GlyphRun(ViewGlyphRun {
            run_index: self.run_index,
            bounds: self.bounds,
            color: self.color,
        })
    }
}

impl ArcweftInlineParticipant {
    pub fn new(
        node: NodeId,
        kind: ArcweftInlineParticipantKind,
        measured_size: InlineMeasuredSize,
        glyph_runs: impl Into<Vec<ArcweftGlyphRun>>,
    ) -> Self {
        Self {
            node,
            kind,
            measured_size,
            glyph_runs: glyph_runs.into(),
        }
    }

    pub fn node(&self) -> NodeId {
        self.node
    }

    pub fn kind(&self) -> ArcweftInlineParticipantKind {
        self.kind
    }

    pub fn measured_size(&self) -> InlineMeasuredSize {
        self.measured_size
    }

    pub fn glyph_runs(&self) -> &[ArcweftGlyphRun] {
        &self.glyph_runs
    }

    pub fn object_replacement_text(&self) -> &'static str {
        "\u{fffc}"
    }
}

impl ArcweftTextLayoutBridge {
    pub fn insert(&mut self, participant: ArcweftInlineParticipant) {
        self.participants.push(participant);
    }

    pub fn participants(&self) -> &[ArcweftInlineParticipant] {
        &self.participants
    }

    pub fn get(&self, node: NodeId) -> Option<&ArcweftInlineParticipant> {
        self.participants
            .iter()
            .find(|participant| participant.node() == node)
    }

    pub fn placeholder_text(&self, node: NodeId) -> Option<&'static str> {
        self.get(node)
            .map(ArcweftInlineParticipant::object_replacement_text)
    }

    pub fn emit_glyph_runs_for(&self, node: NodeId, scene: &mut ViewScene) -> bool {
        let Some(participant) = self.get(node) else {
            return false;
        };
        for run in participant.glyph_runs() {
            scene.push_primitive(run.clone().into_primitive());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white() -> ViewColorRgba8 {
        ViewColorRgba8 {
            red: 255,
            green: 255,
            blue: 255,
            alpha: 255,
        }
    }

    #[test]
    fn glyph_runs_are_emitted_from_arcweft_text_layout() {
        let mut bridge = ArcweftTextLayoutBridge::default();
        bridge.insert(ArcweftInlineParticipant::new(
            NodeId(1),
            ArcweftInlineParticipantKind::Text,
            InlineMeasuredSize::new(24.0, 12.0, Some(9.0)),
            [ArcweftGlyphRun::new(
                3,
                HitRect::new(0.0, 0.0, 24.0, 12.0),
                white(),
            )],
        ));

        let mut scene = ViewScene::new(320.0, 180.0);
        assert!(bridge.emit_glyph_runs_for(NodeId(1), &mut scene));
        assert_eq!(scene.primitives().len(), 1);
        assert_eq!(bridge.placeholder_text(NodeId(1)), Some("\u{fffc}"));
    }
}
