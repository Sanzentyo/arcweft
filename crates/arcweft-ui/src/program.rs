//! Bundle-owned View program data produced by Arcweft View DSL lowering.
//!
//! This is the retained, Sans I/O execution substrate for Arcweft-authored
//! Views. It intentionally does not evaluate expressions or allocate GPU
//! resources. View evaluators consume `ViewProgram`, props, local state, and
//! environment snapshots, then emit `ViewFragment`, `UiFrameResources`,
//! handlers, semantics, and style overlays.

use crate::{
    CustomElementId, EventKind, HandlerId, ImageId, SemanticSpecId, StyleId, TextSourceId, ViewId,
    ViewProgramId,
};
use arcweft_id::PublicId;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewElementKind {
    Surface,
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
        ViewElementKind, ViewElementSpec, ViewInstruction, ViewPartExport, ViewPartId,
        ViewProgramBuilder,
    };
    use crate::{ViewId, ViewProgramId};

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
}
