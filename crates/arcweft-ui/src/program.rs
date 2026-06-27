//! Bundle-owned UI program data produced by Arcweft Component DSL lowering.
//!
//! This is the retained, Sans I/O execution substrate for Arcweft-authored
//! Components. It intentionally does not evaluate expressions or allocate GPU
//! resources. Component evaluators consume `UiProgram`, props, local state, and
//! environment snapshots, then emit `ViewFragment`, `UiFrameResources`, handlers,
//! semantics, and style overlays.

use crate::{
    ComponentId, CustomElementId, EventKind, HandlerId, ImageId, SemanticSpecId, StyleId,
    TextSourceId, UiProgramId,
};
use arcweft_id::PublicId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiProgram {
    id: UiProgramId,
    component: ComponentId,
    instructions: Vec<UiInstruction>,
    exported_parts: Vec<UiPartExport>,
    handler_programs: Vec<UiHandlerProgram>,
    state_schema_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiInstruction {
    OpenElement(UiElementSpec),
    CloseElement,
    EmitText(UiTextSpec),
    EmitImage(UiImageSpec),
    EmitCustom(UiCustomSpec),
    CallComponent(UiComponentCall),
    Branch(UiBranch),
    RepeatKeyed(UiRepeat),
    ApplyStyle(UiStyleApply),
    BindEvent(UiEventBindingSpec),
    AttachSemantic(UiSemanticSpec),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiElementSpec {
    pub kind: UiElementKind,
    pub style: Option<StyleId>,
    pub part: Option<UiPartId>,
    pub key: Option<UiStableKey>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UiPartId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiPartExport {
    pub id: UiPartId,
    pub public_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiElementKind {
    Surface,
    Row,
    Column,
    Stack,
    Button,
    TextField,
    TextArea,
    SecureField,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UiStableKey(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiTextSpec {
    pub source: TextSourceId,
    pub style: Option<StyleId>,
    pub part: Option<UiPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiImageSpec {
    pub image: ImageId,
    pub style: Option<StyleId>,
    pub part: Option<UiPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiCustomSpec {
    pub element: CustomElementId,
    pub style: Option<StyleId>,
    pub part: Option<UiPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiComponentCall {
    pub component: ComponentId,
    pub props: UiExpressionId,
    pub style: Option<StyleId>,
    pub part: Option<UiPartId>,
    pub key: Option<UiStableKey>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UiExpressionId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiBranch {
    pub condition: UiExpressionId,
    pub then_range: UiInstructionRange,
    pub else_range: Option<UiInstructionRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiRepeat {
    pub source: UiExpressionId,
    pub key: UiExpressionId,
    pub body: UiInstructionRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiInstructionRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiStyleApply {
    Named(StyleId),
    InlineArcweft(UiStylePatchId),
    InlineCss(UiStylePatchId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UiStylePatchId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiEventBindingSpec {
    pub event: EventKind,
    pub handler: HandlerId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiSemanticSpec {
    pub semantic: SemanticSpecId,
    pub target: PublicId,
    pub label: Option<TextSourceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiHandlerProgram {
    pub handler: HandlerId,
    pub target_action: Option<PublicId>,
    pub body: UiExpressionId,
}

impl UiProgram {
    pub fn new(
        id: UiProgramId,
        component: ComponentId,
        state_schema_hash: u64,
        instructions: Vec<UiInstruction>,
    ) -> Self {
        Self {
            id,
            component,
            instructions,
            exported_parts: Vec::new(),
            handler_programs: Vec::new(),
            state_schema_hash,
        }
    }

    #[must_use]
    pub fn with_exported_parts(mut self, exported_parts: Vec<UiPartExport>) -> Self {
        self.exported_parts = exported_parts;
        self
    }

    #[must_use]
    pub fn with_handler_programs(mut self, handler_programs: Vec<UiHandlerProgram>) -> Self {
        self.handler_programs = handler_programs;
        self
    }

    pub const fn id(&self) -> UiProgramId {
        self.id
    }

    pub const fn component(&self) -> ComponentId {
        self.component
    }

    pub const fn state_schema_hash(&self) -> u64 {
        self.state_schema_hash
    }

    pub fn instructions(&self) -> &[UiInstruction] {
        &self.instructions
    }

    pub fn exported_parts(&self) -> &[UiPartExport] {
        &self.exported_parts
    }

    pub fn handler_programs(&self) -> &[UiHandlerProgram] {
        &self.handler_programs
    }
}
