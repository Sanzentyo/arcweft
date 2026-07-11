use arcweft_render_wgpu::view_scene::{
    PreparedTextId, ViewPrimitive, ViewScene, ViewTextPrimitive,
};
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
pub struct ArcweftPreparedText {
    text: PreparedTextId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArcweftInlineParticipant {
    node: NodeId,
    kind: ArcweftInlineParticipantKind,
    measured_size: InlineMeasuredSize,
    prepared_text: Vec<ArcweftPreparedText>,
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

impl ArcweftPreparedText {
    pub const fn new(text: PreparedTextId) -> Self {
        Self { text }
    }

    pub const fn text(&self) -> PreparedTextId {
        self.text
    }

    pub fn into_primitive(self) -> ViewPrimitive {
        ViewPrimitive::Text(ViewTextPrimitive { text: self.text })
    }
}

impl ArcweftInlineParticipant {
    pub fn new(
        node: NodeId,
        kind: ArcweftInlineParticipantKind,
        measured_size: InlineMeasuredSize,
        prepared_text: impl Into<Vec<ArcweftPreparedText>>,
    ) -> Self {
        Self {
            node,
            kind,
            measured_size,
            prepared_text: prepared_text.into(),
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

    pub fn prepared_text(&self) -> &[ArcweftPreparedText] {
        &self.prepared_text
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

    pub fn emit_text_for(&self, node: NodeId, scene: &mut ViewScene) -> bool {
        let Some(participant) = self.get(node) else {
            return false;
        };
        for text in participant.prepared_text() {
            scene.push_primitive(text.clone().into_primitive());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_text_is_emitted_from_arcweft_text_layout() {
        let mut bridge = ArcweftTextLayoutBridge::default();
        bridge.insert(ArcweftInlineParticipant::new(
            NodeId(1),
            ArcweftInlineParticipantKind::Text,
            InlineMeasuredSize::new(24.0, 12.0, Some(9.0)),
            [ArcweftPreparedText::new(PreparedTextId::from_index(3))],
        ));

        let mut scene = ViewScene::new(320.0, 180.0);
        assert!(bridge.emit_text_for(NodeId(1), &mut scene));
        assert_eq!(scene.primitives().len(), 1);
        assert_eq!(bridge.placeholder_text(NodeId(1)), Some("\u{fffc}"));
    }
}
