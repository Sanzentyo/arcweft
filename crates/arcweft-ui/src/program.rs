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
pub struct UiProgramBuilder {
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

impl UiInstructionRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

impl UiPartExport {
    pub fn new(id: UiPartId, public_name: impl Into<String>) -> Self {
        Self {
            id,
            public_name: public_name.into(),
        }
    }
}

impl UiProgramBuilder {
    pub fn new(id: UiProgramId, component: ComponentId, state_schema_hash: u64) -> Self {
        Self {
            id,
            component,
            instructions: Vec::new(),
            exported_parts: Vec::new(),
            handler_programs: Vec::new(),
            state_schema_hash,
        }
    }

    pub fn push(&mut self, instruction: UiInstruction) -> u32 {
        let index = u32::try_from(self.instructions.len()).unwrap_or(u32::MAX);
        self.instructions.push(instruction);
        index
    }

    pub fn export_part(&mut self, export: UiPartExport) {
        self.exported_parts.push(export);
    }

    pub fn push_handler_program(&mut self, handler: UiHandlerProgram) {
        self.handler_programs.push(handler);
    }

    pub fn finish(self) -> UiProgram {
        UiProgram::new(
            self.id,
            self.component,
            self.state_schema_hash,
            self.instructions,
        )
        .with_exported_parts(self.exported_parts)
        .with_handler_programs(self.handler_programs)
    }
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

#[cfg(test)]
mod tests {
    use super::{
        UiElementKind, UiElementSpec, UiInstruction, UiPartExport, UiPartId, UiProgramBuilder,
    };
    use crate::{ComponentId, UiProgramId};

    #[test]
    fn ui_program_builder_preserves_instruction_order_before_fragment_lowering() {
        let mut builder = UiProgramBuilder::new(UiProgramId(1), ComponentId(2), 0xCAFE);
        builder.push(UiInstruction::OpenElement(UiElementSpec {
            kind: UiElementKind::TextField,
            style: None,
            part: Some(UiPartId(1)),
            key: None,
        }));
        builder.push(UiInstruction::CloseElement);
        builder.export_part(UiPartExport::new(UiPartId(1), "field"));

        let program = builder.finish();

        assert_eq!(program.instructions().len(), 2);
        assert_eq!(program.exported_parts()[0].public_name, "field");
        assert_eq!(program.state_schema_hash(), 0xCAFE);
    }
}
