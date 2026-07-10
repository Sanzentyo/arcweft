//! Bundle-owned View program data produced by Arcweft View DSL lowering.
//!
//! This is the retained, Sans I/O execution substrate for Arcweft-authored
//! Views. It intentionally does not evaluate expressions or allocate GPU
//! resources. View evaluators consume `ViewProgram`, props, local state, and
//! environment snapshots, then emit `ViewFragment`, `ViewFrameResources`,
//! handlers, semantics, and style overlays.

use crate::{
    CustomElementId, EventKind, HandlerId, ImageId, SemanticSpecId, StyleId, TextSourceId, ViewId,
    ViewProgramId,
};
use arcweft_id::PublicId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewProgram {
    id: ViewProgramId,
    view: ViewId,
    instructions: Vec<ViewInstruction>,
    exported_parts: Vec<ViewPartExport>,
    handler_programs: Vec<ViewHandlerProgram>,
    state_schema_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewProgramBuilder {
    id: ViewProgramId,
    view: ViewId,
    instructions: Vec<ViewInstruction>,
    exported_parts: Vec<ViewPartExport>,
    handler_programs: Vec<ViewHandlerProgram>,
    state_schema_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewInstruction {
    OpenElement(ViewElementSpec),
    CloseElement,
    EmitText(ViewTextSpec),
    EmitImage(ViewImageSpec),
    EmitCustom(ViewCustomSpec),
    CallView(ViewCall),
    Branch(ViewBranch),
    RepeatKeyed(ViewRepeat),
    ApplyStyle(ViewStyleApply),
    BindEvent(ViewEventBindingSpec),
    AttachSemantic(ViewSemanticSpec),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewElementSpec {
    pub kind: ViewElementKind,
    pub style: Option<StyleId>,
    pub part: Option<ViewPartId>,
    pub key: Option<ViewStableKey>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPartId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartExport {
    pub id: ViewPartId,
    pub public_name: String,
}

/// Canonical inventory of built-in Arcweft View elements.
///
/// Source parsing, bundle codecs, runtime labels, and element classification
/// all use this type so adding an element cannot leave a parallel inventory
/// out of sync.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewElementKind {
    Panel,
    Box,
    Scroll,
    Row,
    Column,
    Stack,
    Button,
    TextField,
    TextArea,
    SecureField,
}

/// Layout strategy owned by a built-in View container.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewElementLayoutKind {
    Stack,
    Scroll,
    Row,
    Column,
}

/// Primary layout axis owned by a virtualized retained list.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewVirtualAxis {
    Horizontal,
    Vertical,
}

/// Text-input control represented by a built-in View element.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewElementTextInputKind {
    TextField,
    TextArea,
    SecureField,
}

impl ViewElementKind {
    pub const ALL: [Self; 10] = [
        Self::Panel,
        Self::Box,
        Self::Scroll,
        Self::Row,
        Self::Column,
        Self::Stack,
        Self::Button,
        Self::TextField,
        Self::TextArea,
        Self::SecureField,
    ];

    /// Canonical case-sensitive spelling in Arcweft View source.
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Panel => "Panel",
            Self::Box => "Box",
            Self::Scroll => "Scroll",
            Self::Row => "Row",
            Self::Column => "Column",
            Self::Stack => "Stack",
            Self::Button => "Button",
            Self::TextField => "TextField",
            Self::TextArea => "TextArea",
            Self::SecureField => "SecureField",
        }
    }

    /// Stable snake-case label used by runtime targets and serialized codecs.
    pub const fn runtime_label(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Box => "box",
            Self::Scroll => "scroll",
            Self::Row => "row",
            Self::Column => "column",
            Self::Stack => "stack",
            Self::Button => "button",
            Self::TextField => "text_field",
            Self::TextArea => "text_area",
            Self::SecureField => "secure_field",
        }
    }

    /// Looks up an exact canonical source spelling without accepting aliases.
    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|element| element.source_name() == value)
    }

    /// Looks up an exact runtime/codec label without accepting aliases.
    pub fn from_runtime_label(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|element| element.runtime_label() == value)
    }

    /// Returns the container layout strategy, or `None` for leaf controls.
    pub const fn layout_kind(self) -> Option<ViewElementLayoutKind> {
        match self {
            Self::Panel | Self::Box | Self::Stack => Some(ViewElementLayoutKind::Stack),
            Self::Scroll => Some(ViewElementLayoutKind::Scroll),
            Self::Row => Some(ViewElementLayoutKind::Row),
            Self::Column => Some(ViewElementLayoutKind::Column),
            Self::Button | Self::TextField | Self::TextArea | Self::SecureField => None,
        }
    }

    /// Returns the text-input role owned by this element, when applicable.
    pub const fn text_input_kind(self) -> Option<ViewElementTextInputKind> {
        match self {
            Self::TextField => Some(ViewElementTextInputKind::TextField),
            Self::TextArea => Some(ViewElementTextInputKind::TextArea),
            Self::SecureField => Some(ViewElementTextInputKind::SecureField),
            Self::Panel
            | Self::Box
            | Self::Scroll
            | Self::Row
            | Self::Column
            | Self::Stack
            | Self::Button => None,
        }
    }

    /// Whether the element lays out child View nodes.
    pub const fn is_layout_container(self) -> bool {
        self.layout_kind().is_some()
    }

    /// Whether this element owns a text-input control.
    pub const fn is_text_input(self) -> bool {
        self.text_input_kind().is_some()
    }

    /// Whether this element owns an action-button control.
    pub const fn is_action_control(self) -> bool {
        matches!(self, Self::Button)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewStableKey(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewTextSpec {
    pub source: TextSourceId,
    pub style: Option<StyleId>,
    pub part: Option<ViewPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewImageSpec {
    pub image: ImageId,
    pub style: Option<StyleId>,
    pub part: Option<ViewPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewCustomSpec {
    pub element: CustomElementId,
    pub style: Option<StyleId>,
    pub part: Option<ViewPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewCall {
    pub view: ViewId,
    pub props: ViewExpressionId,
    pub style: Option<StyleId>,
    pub part: Option<ViewPartId>,
    pub key: Option<ViewStableKey>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewExpressionId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewBranch {
    pub condition: ViewExpressionId,
    pub then_range: ViewInstructionRange,
    pub else_range: Option<ViewInstructionRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRepeat {
    pub source: ViewExpressionId,
    pub key: ViewExpressionId,
    pub body: ViewInstructionRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewInstructionRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewStyleApply {
    Named(StyleId),
    InlineArcweft(ViewStylePatchId),
    InlineCss(ViewStylePatchId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewStylePatchId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewEventBindingSpec {
    pub event: EventKind,
    pub handler: HandlerId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewSemanticSpec {
    pub semantic: SemanticSpecId,
    pub target: PublicId,
    pub label: Option<TextSourceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewHandlerProgram {
    pub handler: HandlerId,
    pub target_action: Option<PublicId>,
    pub body: ViewExpressionId,
}

impl ViewInstructionRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

impl ViewPartExport {
    pub fn new(id: ViewPartId, public_name: impl Into<String>) -> Self {
        Self {
            id,
            public_name: public_name.into(),
        }
    }
}

impl ViewProgramBuilder {
    pub fn new(id: ViewProgramId, view: ViewId, state_schema_hash: u64) -> Self {
        Self {
            id,
            view,
            instructions: Vec::new(),
            exported_parts: Vec::new(),
            handler_programs: Vec::new(),
            state_schema_hash,
        }
    }

    pub fn push(&mut self, instruction: ViewInstruction) -> u32 {
        let index = u32::try_from(self.instructions.len()).unwrap_or(u32::MAX);
        self.instructions.push(instruction);
        index
    }

    pub fn export_part(&mut self, export: ViewPartExport) {
        self.exported_parts.push(export);
    }

    pub fn push_handler_program(&mut self, handler: ViewHandlerProgram) {
        self.handler_programs.push(handler);
    }

    pub fn finish(self) -> ViewProgram {
        ViewProgram::new(
            self.id,
            self.view,
            self.state_schema_hash,
            self.instructions,
        )
        .with_exported_parts(self.exported_parts)
        .with_handler_programs(self.handler_programs)
    }
}

impl ViewProgram {
    pub fn new(
        id: ViewProgramId,
        view: ViewId,
        state_schema_hash: u64,
        instructions: Vec<ViewInstruction>,
    ) -> Self {
        Self {
            id,
            view,
            instructions,
            exported_parts: Vec::new(),
            handler_programs: Vec::new(),
            state_schema_hash,
        }
    }

    #[must_use]
    pub fn with_exported_parts(mut self, exported_parts: Vec<ViewPartExport>) -> Self {
        self.exported_parts = exported_parts;
        self
    }

    #[must_use]
    pub fn with_handler_programs(mut self, handler_programs: Vec<ViewHandlerProgram>) -> Self {
        self.handler_programs = handler_programs;
        self
    }

    pub const fn id(&self) -> ViewProgramId {
        self.id
    }

    pub const fn view(&self) -> ViewId {
        self.view
    }

    pub const fn state_schema_hash(&self) -> u64 {
        self.state_schema_hash
    }

    pub fn instructions(&self) -> &[ViewInstruction] {
        &self.instructions
    }

    pub fn exported_parts(&self) -> &[ViewPartExport] {
        &self.exported_parts
    }

    pub fn handler_programs(&self) -> &[ViewHandlerProgram] {
        &self.handler_programs
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ViewElementKind, ViewElementLayoutKind, ViewElementSpec, ViewElementTextInputKind,
        ViewInstruction, ViewPartExport, ViewPartId, ViewProgramBuilder,
    };
    use crate::{ViewId, ViewProgramId};
    use std::collections::BTreeSet;

    #[test]
    fn view_program_builder_preserves_instruction_order_before_fragment_lowering() {
        let mut builder = ViewProgramBuilder::new(ViewProgramId(1), ViewId(2), 0xCAFE);
        builder.push(ViewInstruction::OpenElement(ViewElementSpec {
            kind: ViewElementKind::TextField,
            style: None,
            part: Some(ViewPartId(1)),
            key: None,
        }));
        builder.push(ViewInstruction::CloseElement);
        builder.export_part(ViewPartExport::new(ViewPartId(1), "field"));

        let program = builder.finish();

        assert_eq!(program.instructions().len(), 2);
        assert_eq!(program.exported_parts()[0].public_name, "field");
        assert_eq!(program.state_schema_hash(), 0xCAFE);
    }

    #[test]
    fn element_inventory_owns_unique_source_and_runtime_names() {
        let source_names = ViewElementKind::ALL
            .into_iter()
            .map(ViewElementKind::source_name)
            .collect::<BTreeSet<_>>();
        let runtime_labels = ViewElementKind::ALL
            .into_iter()
            .map(ViewElementKind::runtime_label)
            .collect::<BTreeSet<_>>();

        assert_eq!(source_names.len(), ViewElementKind::ALL.len());
        assert_eq!(runtime_labels.len(), ViewElementKind::ALL.len());
        for element in ViewElementKind::ALL {
            assert_eq!(
                ViewElementKind::from_source_name(element.source_name()),
                Some(element)
            );
            assert_eq!(
                ViewElementKind::from_runtime_label(element.runtime_label()),
                Some(element)
            );
        }
        assert_eq!(ViewElementKind::from_source_name("panel"), None);
        assert_eq!(ViewElementKind::from_runtime_label("LazyRow"), None);
    }

    #[test]
    fn element_inventory_classifies_layout_and_control_roles() {
        assert_eq!(
            ViewElementKind::Panel.layout_kind(),
            Some(ViewElementLayoutKind::Stack)
        );
        assert_eq!(
            ViewElementKind::Row.layout_kind(),
            Some(ViewElementLayoutKind::Row)
        );
        assert!(ViewElementKind::Scroll.is_layout_container());
        assert!(ViewElementKind::Button.is_action_control());
        assert_eq!(
            ViewElementKind::SecureField.text_input_kind(),
            Some(ViewElementTextInputKind::SecureField)
        );
        assert!(ViewElementKind::TextArea.is_text_input());
        assert!(!ViewElementKind::Button.is_text_input());
    }
}
