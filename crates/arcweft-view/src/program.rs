//! Bundle-owned View program data produced by Arcweft View DSL lowering.
//!
//! This is the retained, Sans I/O execution substrate for Arcweft-authored
//! Views. It intentionally does not evaluate expressions or allocate GPU
//! resources. View evaluators consume `ViewProgram`, props, local state, and
//! environment snapshots, then emit `ViewFragment`, `ViewFrameResources`,
//! handlers, semantics, and style overlays.

use crate::style::{ViewPhysicalFlow, ViewStyleApplicationTarget};
use crate::{
    CustomElementId, EventKind, HandlerId, ImageId, SemanticSpecId, TextSourceId,
    ViewEvaluationSiteId, ViewId, ViewInstructionIndex, ViewPartExport, ViewPartId,
    ViewPartInstructionKind, ViewPartLocalName, ViewPartName, ViewPartStaticReachability,
    ViewProgramBuildError, ViewProgramId, ViewStaticPart, ViewValueProgramId,
    ViewValueProgramInventory,
};
use arcweft_id::PublicId;
use arcweft_presentation::fx::FxId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewProgram {
    id: ViewProgramId,
    view: ViewId,
    value_programs: ViewValueProgramInventory,
    instructions: Vec<ViewInstruction>,
    static_parts: Vec<ViewStaticPart>,
    exported_parts: Vec<ViewPartExport>,
    handler_programs: Vec<ViewHandlerProgram>,
    state_schema_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewProgramBuilder {
    id: ViewProgramId,
    view: ViewId,
    value_programs: ViewValueProgramInventory,
    instructions: Vec<ViewInstruction>,
    static_parts: Vec<ViewStaticPart>,
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
    Await(ViewAwait),
    BindLocal(ViewLocalBinding),
    ApplyFx(ViewFxApplicationInstruction),
    BindEvent(ViewEventBindingSpec),
    AttachSemantic(ViewSemanticSpec),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewElementSpec {
    pub kind: ViewElementKind,
    /// Ordered named-sheet and inline-patch applications authored on this node.
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub part: Option<ViewPartId>,
    pub key: Option<ViewStableKey>,
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

    /// Default executable physical flow owned by this built-in element.
    pub const fn default_physical_flow(self) -> Option<ViewPhysicalFlow> {
        match self {
            Self::Row => Some(ViewPhysicalFlow::Row),
            Self::Column => Some(ViewPhysicalFlow::Column),
            Self::Panel | Self::Box | Self::Scroll | Self::Stack => Some(ViewPhysicalFlow::Overlay),
            Self::Button | Self::TextField | Self::TextArea | Self::SecureField => None,
        }
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
    /// Ordered named-sheet and inline-patch applications authored on this node.
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub part: Option<ViewPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewImageSpec {
    pub image: ImageId,
    /// Ordered named-sheet and inline-patch applications authored on this node.
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub part: Option<ViewPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewCustomSpec {
    pub element: CustomElementId,
    /// Ordered named-sheet and inline-patch applications authored on this node.
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub part: Option<ViewPartId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewCall {
    pub view: ViewId,
    pub arguments: Vec<ViewCallArgument>,
    /// Ordered applications established before the nested View is evaluated.
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub part: Option<ViewPartId>,
    pub key: Option<ViewStableKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewCallArgument {
    pub ordinal: u16,
    pub name: Option<String>,
    pub value: ViewValueProgramId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewBranch {
    pub condition: ViewValueProgramId,
    pub then_range: ViewInstructionRange,
    pub else_range: Option<ViewInstructionRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRepeat {
    pub source: ViewValueProgramId,
    pub key: ViewValueProgramId,
    pub body: ViewInstructionRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewAwaitBranch {
    pub start_offset: u32,
    pub body_span: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewAwait {
    pub source: ViewValueProgramId,
    pub pending: Option<ViewAwaitBranch>,
    pub ready: Option<ViewAwaitBranch>,
    pub error: Option<ViewAwaitBranch>,
    pub denied: Option<ViewAwaitBranch>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewLocalBinding {
    pub binding: String,
    pub value: ViewValueProgramId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFxCallArgument {
    pub parameter: String,
    pub value: ViewValueProgramId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFxApplicationInstruction {
    pub fx: FxId,
    pub arguments: Vec<ViewFxCallArgument>,
    pub key: Option<ViewValueProgramId>,
    pub application_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewInstructionRange {
    pub start: u32,
    pub end: u32,
}

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
    pub body: ViewValueProgramId,
}

impl ViewInstructionRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

impl ViewProgramBuilder {
    pub fn new(id: ViewProgramId, view: ViewId, state_schema_hash: u64) -> Self {
        Self {
            id,
            view,
            value_programs: ViewValueProgramInventory::default(),
            instructions: Vec::new(),
            static_parts: Vec::new(),
            exported_parts: Vec::new(),
            handler_programs: Vec::new(),
            state_schema_hash,
        }
    }

    pub fn push(
        &mut self,
        instruction: ViewInstruction,
    ) -> Result<ViewInstructionIndex, ViewProgramBuildError> {
        let index =
            ViewInstructionIndex::try_from_index(self.instructions.len()).map_err(|_| {
                ViewProgramBuildError::InstructionIndexOverflow {
                    length: self.instructions.len(),
                }
            })?;
        self.instructions.push(instruction);
        Ok(index)
    }

    pub fn set_value_programs(&mut self, value_programs: ViewValueProgramInventory) {
        self.value_programs = value_programs;
    }

    pub fn register_part(
        &mut self,
        local_name: ViewPartLocalName,
        instruction: ViewInstructionIndex,
        reachability: ViewPartStaticReachability,
        site: ViewEvaluationSiteId,
    ) -> Result<ViewPartId, ViewProgramBuildError> {
        let instruction_value = self
            .instructions
            .get(instruction.index())
            .ok_or(ViewProgramBuildError::UnknownInstruction { instruction })?;
        let kind = instruction_value
            .part_kind()
            .ok_or(ViewProgramBuildError::UnsupportedInstruction { instruction })?;

        if let Some(first) = self
            .static_parts
            .iter()
            .find(|part| part.local_name() == &local_name)
        {
            return Err(ViewProgramBuildError::DuplicateLocalName {
                name: local_name,
                first: first.id(),
                duplicate_instruction: instruction,
            });
        }

        if let Some(previous) = self.static_parts.last().map(ViewStaticPart::local_name)
            && previous >= &local_name
        {
            return Err(ViewProgramBuildError::NonCanonicalLocalOrder {
                previous: previous.clone(),
                next: local_name,
            });
        }

        if let Some(first) = self
            .static_parts
            .iter()
            .find(|part| part.instruction() == instruction)
        {
            return Err(ViewProgramBuildError::DuplicateInstructionTarget {
                instruction,
                first: first.id(),
            });
        }
        if let Some(first) = instruction_value.part_id() {
            return Err(ViewProgramBuildError::DuplicateInstructionTarget { instruction, first });
        }
        if let Some(first) = self.static_parts.iter().find(|part| part.site() == site) {
            return Err(ViewProgramBuildError::DuplicateEvaluationSite {
                site,
                first: first.instruction(),
                duplicate: instruction,
            });
        }

        let id = ViewPartId::try_from_index(self.static_parts.len()).map_err(|_| {
            ViewProgramBuildError::PartIdOverflow {
                count: self.static_parts.len(),
            }
        })?;
        let part = ViewStaticPart::new(id, local_name, instruction, kind, reachability, site);

        self.static_parts.push(part);
        self.instructions[instruction.index()].set_part_id(id);
        Ok(id)
    }

    pub fn export_part(
        &mut self,
        part: ViewPartId,
        public_name: ViewPartName,
    ) -> Result<(), ViewProgramBuildError> {
        let target = self
            .static_parts
            .get(part.index())
            .filter(|target| target.id() == part)
            .ok_or(ViewProgramBuildError::UnknownPart { part })?;
        if !target.kind().is_exportable() {
            return Err(ViewProgramBuildError::UnsupportedCallViewExport {
                part,
                instruction: target.instruction(),
            });
        }
        if let Some(existing) = self
            .exported_parts
            .iter()
            .find(|export| export.part() == part)
        {
            return Err(ViewProgramBuildError::TargetAlreadyExported {
                part,
                existing: existing.public_name().clone(),
            });
        }
        if let Some(first) = self
            .exported_parts
            .iter()
            .find(|export| export.public_name() == &public_name)
        {
            return Err(ViewProgramBuildError::DuplicatePublicName {
                name: public_name,
                first: first.part(),
                duplicate: part,
            });
        }
        if let Some(previous) = self.exported_parts.last().map(ViewPartExport::public_name)
            && previous >= &public_name
        {
            return Err(ViewProgramBuildError::NonCanonicalExportOrder {
                previous: previous.clone(),
                next: public_name,
            });
        }

        self.exported_parts
            .push(ViewPartExport::new(part, public_name));
        Ok(())
    }

    pub fn push_handler_program(&mut self, handler: ViewHandlerProgram) {
        self.handler_programs.push(handler);
    }

    pub fn finish(self) -> Result<ViewProgram, ViewProgramBuildError> {
        validate_parts(&self.instructions, &self.static_parts, &self.exported_parts)?;
        Ok(ViewProgram {
            id: self.id,
            view: self.view,
            value_programs: self.value_programs,
            instructions: self.instructions,
            static_parts: self.static_parts,
            exported_parts: self.exported_parts,
            handler_programs: self.handler_programs,
            state_schema_hash: self.state_schema_hash,
        })
    }
}

impl ViewProgram {
    pub const fn id(&self) -> &ViewProgramId {
        &self.id
    }

    pub const fn view(&self) -> &ViewId {
        &self.view
    }

    pub const fn state_schema_hash(&self) -> u64 {
        self.state_schema_hash
    }

    pub fn instructions(&self) -> &[ViewInstruction] {
        &self.instructions
    }

    pub fn static_parts(&self) -> &[ViewStaticPart] {
        &self.static_parts
    }

    pub const fn value_programs(&self) -> &ViewValueProgramInventory {
        &self.value_programs
    }

    pub fn exported_parts(&self) -> &[ViewPartExport] {
        &self.exported_parts
    }

    pub fn handler_programs(&self) -> &[ViewHandlerProgram] {
        &self.handler_programs
    }
}

impl ViewInstruction {
    pub const fn part_kind(&self) -> Option<ViewPartInstructionKind> {
        match self {
            Self::OpenElement(_) => Some(ViewPartInstructionKind::OpenElement),
            Self::EmitText(_) => Some(ViewPartInstructionKind::EmitText),
            Self::EmitImage(_) => Some(ViewPartInstructionKind::EmitImage),
            Self::EmitCustom(_) => Some(ViewPartInstructionKind::EmitCustom),
            Self::CallView(_) => Some(ViewPartInstructionKind::CallView),
            Self::CloseElement
            | Self::Branch(_)
            | Self::RepeatKeyed(_)
            | Self::Await(_)
            | Self::BindLocal(_)
            | Self::ApplyFx(_)
            | Self::BindEvent(_)
            | Self::AttachSemantic(_) => None,
        }
    }

    pub const fn part_id(&self) -> Option<ViewPartId> {
        match self {
            Self::OpenElement(spec) => spec.part,
            Self::EmitText(spec) => spec.part,
            Self::EmitImage(spec) => spec.part,
            Self::EmitCustom(spec) => spec.part,
            Self::CallView(call) => call.part,
            Self::CloseElement
            | Self::Branch(_)
            | Self::RepeatKeyed(_)
            | Self::Await(_)
            | Self::BindLocal(_)
            | Self::ApplyFx(_)
            | Self::BindEvent(_)
            | Self::AttachSemantic(_) => None,
        }
    }

    fn set_part_id(&mut self, part: ViewPartId) {
        match self {
            Self::OpenElement(spec) => spec.part = Some(part),
            Self::EmitText(spec) => spec.part = Some(part),
            Self::EmitImage(spec) => spec.part = Some(part),
            Self::EmitCustom(spec) => spec.part = Some(part),
            Self::CallView(call) => call.part = Some(part),
            Self::CloseElement
            | Self::Branch(_)
            | Self::RepeatKeyed(_)
            | Self::Await(_)
            | Self::BindLocal(_)
            | Self::ApplyFx(_)
            | Self::BindEvent(_)
            | Self::AttachSemantic(_) => {
                unreachable!("part IDs are assigned only to node-producing instructions")
            }
        }
    }
}

fn validate_parts(
    instructions: &[ViewInstruction],
    static_parts: &[ViewStaticPart],
    exports: &[ViewPartExport],
) -> Result<(), ViewProgramBuildError> {
    if ViewInstructionIndex::try_from_index(instructions.len()).is_err() {
        return Err(ViewProgramBuildError::InstructionIndexOverflow {
            length: instructions.len(),
        });
    }
    if ViewPartId::try_from_index(static_parts.len()).is_err() {
        return Err(ViewProgramBuildError::PartIdOverflow {
            count: static_parts.len(),
        });
    }

    validate_static_parts(instructions, static_parts)?;
    validate_instruction_part_links(instructions, static_parts)?;
    validate_exports(static_parts, exports)
}

fn validate_static_parts(
    instructions: &[ViewInstruction],
    static_parts: &[ViewStaticPart],
) -> Result<(), ViewProgramBuildError> {
    let mut local_names = BTreeMap::new();
    let mut instruction_targets = BTreeMap::new();
    let mut sites = BTreeMap::new();
    let mut previous_local: Option<&ViewPartLocalName> = None;
    for (expected_index, part) in static_parts.iter().enumerate() {
        let expected_id = ViewPartId::try_from_index(expected_index).map_err(|_| {
            ViewProgramBuildError::PartIdOverflow {
                count: static_parts.len(),
            }
        })?;
        if part.id() != expected_id {
            return Err(ViewProgramBuildError::UnknownPart { part: part.id() });
        }
        if let Some(first) = local_names.insert(part.local_name(), part.id()) {
            return Err(ViewProgramBuildError::DuplicateLocalName {
                name: part.local_name().clone(),
                first,
                duplicate_instruction: part.instruction(),
            });
        }
        if let Some(previous) = previous_local
            && previous >= part.local_name()
        {
            return Err(ViewProgramBuildError::NonCanonicalLocalOrder {
                previous: previous.clone(),
                next: part.local_name().clone(),
            });
        }
        previous_local = Some(part.local_name());

        let instruction = instructions.get(part.instruction().index()).ok_or(
            ViewProgramBuildError::StalePartTarget {
                part: part.id(),
                instruction: part.instruction(),
            },
        )?;
        let actual =
            instruction
                .part_kind()
                .ok_or(ViewProgramBuildError::UnsupportedInstruction {
                    instruction: part.instruction(),
                })?;
        if actual != part.kind() {
            return Err(ViewProgramBuildError::InstructionKindMismatch {
                instruction: part.instruction(),
                expected: part.kind(),
                actual,
            });
        }
        if instruction.part_id() != Some(part.id()) {
            return Err(ViewProgramBuildError::StalePartTarget {
                part: part.id(),
                instruction: part.instruction(),
            });
        }
        if let Some(first) = instruction_targets.insert(part.instruction(), part.id()) {
            return Err(ViewProgramBuildError::DuplicateInstructionTarget {
                instruction: part.instruction(),
                first,
            });
        }
        if let Some(first) = sites.insert(part.site(), part.instruction()) {
            return Err(ViewProgramBuildError::DuplicateEvaluationSite {
                site: part.site(),
                first,
                duplicate: part.instruction(),
            });
        }
    }

    Ok(())
}

fn validate_instruction_part_links(
    instructions: &[ViewInstruction],
    static_parts: &[ViewStaticPart],
) -> Result<(), ViewProgramBuildError> {
    let instruction_targets = static_parts
        .iter()
        .map(|part| (part.instruction(), part.id()))
        .collect::<BTreeMap<_, _>>();
    for (index, instruction) in instructions.iter().enumerate() {
        if let Some(part) = instruction.part_id() {
            let instruction_index = ViewInstructionIndex::try_from_index(index).map_err(|_| {
                ViewProgramBuildError::InstructionIndexOverflow {
                    length: instructions.len(),
                }
            })?;
            if instruction_targets.get(&instruction_index) != Some(&part) {
                return Err(ViewProgramBuildError::StalePartTarget {
                    part,
                    instruction: instruction_index,
                });
            }
        }
    }

    Ok(())
}

fn validate_exports(
    static_parts: &[ViewStaticPart],
    exports: &[ViewPartExport],
) -> Result<(), ViewProgramBuildError> {
    let mut public_names = BTreeMap::new();
    let mut exported_targets = BTreeMap::new();
    let mut previous_public: Option<&ViewPartName> = None;
    for export in exports {
        let target = static_parts
            .get(export.part().index())
            .filter(|target| target.id() == export.part())
            .ok_or(ViewProgramBuildError::UnknownPart {
                part: export.part(),
            })?;
        if !target.kind().is_exportable() {
            return Err(ViewProgramBuildError::UnsupportedCallViewExport {
                part: export.part(),
                instruction: target.instruction(),
            });
        }
        if let Some(existing) = exported_targets.insert(export.part(), export.public_name().clone())
        {
            return Err(ViewProgramBuildError::TargetAlreadyExported {
                part: export.part(),
                existing,
            });
        }
        if let Some(first) = public_names.insert(export.public_name(), export.part()) {
            return Err(ViewProgramBuildError::DuplicatePublicName {
                name: export.public_name().clone(),
                first,
                duplicate: export.part(),
            });
        }
        if let Some(previous) = previous_public
            && previous >= export.public_name()
        {
            return Err(ViewProgramBuildError::NonCanonicalExportOrder {
                previous: previous.clone(),
                next: export.public_name().clone(),
            });
        }
        previous_public = Some(export.public_name());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ViewCall, ViewCustomSpec, ViewElementKind, ViewElementLayoutKind, ViewElementSpec,
        ViewElementTextInputKind, ViewImageSpec, ViewInstruction, ViewProgramBuilder, ViewTextSpec,
    };
    use crate::style::{ViewStyleApplicationTarget, ViewStylePatchId, ViewStyleSheetId};
    use crate::{
        CustomElementId, ImageId, TextSourceId, ViewEvaluationSiteId, ViewId, ViewPartLocalName,
        ViewPartName, ViewPartStaticReachability, ViewProgramBuildError, ViewProgramId,
    };
    use std::collections::BTreeSet;

    fn view_id(value: &str) -> ViewId {
        ViewId::try_new(value).unwrap()
    }

    fn program_id(value: &str) -> ViewProgramId {
        ViewProgramId::try_new(value).unwrap()
    }

    #[test]
    fn view_program_builder_preserves_instruction_order_before_fragment_lowering() {
        let mut builder = ViewProgramBuilder::new(
            program_id("view-program.test"),
            view_id("view.test"),
            0xCAFE,
        );
        let instruction = builder
            .push(ViewInstruction::OpenElement(ViewElementSpec {
                kind: ViewElementKind::TextField,
                styles: Vec::new(),
                part: None,
                key: None,
            }))
            .unwrap();
        let part = builder
            .register_part(
                ViewPartLocalName::try_new("field").unwrap(),
                instruction,
                ViewPartStaticReachability::Reachable,
                ViewEvaluationSiteId::from_bytes([1; 32]),
            )
            .unwrap();
        builder.push(ViewInstruction::CloseElement).unwrap();
        builder
            .export_part(part, ViewPartName::try_new("field").unwrap())
            .unwrap();

        let program = builder.finish().unwrap();

        assert_eq!(program.instructions().len(), 2);
        assert_eq!(
            program.exported_parts()[0]
                .public_name()
                .as_public_id()
                .as_str(),
            "field"
        );
        assert_eq!(program.static_parts()[0].id(), part);
        assert_eq!(program.state_schema_hash(), 0xCAFE);
    }

    #[test]
    fn view_program_builder_failure_preserves_exact_state() {
        let mut builder =
            ViewProgramBuilder::new(program_id("view-program.test"), view_id("view.test"), 0);
        let first_instruction = builder
            .push(ViewInstruction::EmitText(ViewTextSpec {
                source: TextSourceId(1),
                styles: Vec::new(),
                part: None,
            }))
            .unwrap();
        let second_instruction = builder
            .push(ViewInstruction::EmitText(ViewTextSpec {
                source: TextSourceId(2),
                styles: Vec::new(),
                part: None,
            }))
            .unwrap();
        let first = builder
            .register_part(
                ViewPartLocalName::try_new("alpha").unwrap(),
                first_instruction,
                ViewPartStaticReachability::Reachable,
                ViewEvaluationSiteId::from_bytes([1; 32]),
            )
            .unwrap();
        let before = builder.clone();

        let error = builder
            .register_part(
                ViewPartLocalName::try_new("alpha").unwrap(),
                second_instruction,
                ViewPartStaticReachability::Reachable,
                ViewEvaluationSiteId::from_bytes([2; 32]),
            )
            .unwrap_err();

        assert_eq!(
            error,
            ViewProgramBuildError::DuplicateLocalName {
                name: ViewPartLocalName::try_new("alpha").unwrap(),
                first,
                duplicate_instruction: second_instruction,
            }
        );
        assert_eq!(builder, before);
    }

    #[test]
    fn call_view_can_be_private_but_cannot_be_exported() {
        let mut builder =
            ViewProgramBuilder::new(program_id("view-program.test"), view_id("view.test"), 0);
        let instruction = builder
            .push(ViewInstruction::CallView(ViewCall {
                view: view_id("view.nested"),
                arguments: Vec::new(),
                styles: Vec::new(),
                part: None,
                key: None,
            }))
            .unwrap();
        let part = builder
            .register_part(
                ViewPartLocalName::try_new("nested").unwrap(),
                instruction,
                ViewPartStaticReachability::Reachable,
                ViewEvaluationSiteId::from_bytes([3; 32]),
            )
            .unwrap();
        let before = builder.clone();

        assert_eq!(
            builder
                .export_part(part, ViewPartName::try_new("nested").unwrap())
                .unwrap_err(),
            ViewProgramBuildError::UnsupportedCallViewExport { part, instruction }
        );
        assert_eq!(builder, before);
        assert_eq!(builder.finish().unwrap().static_parts().len(), 1);
    }

    #[test]
    fn every_node_producer_preserves_ordered_typed_style_applications() {
        let applications = vec![
            ViewStyleApplicationTarget::named(
                ViewStyleSheetId::try_new("style.app.primary")
                    .expect("test sheet ID must be valid"),
            ),
            ViewStyleApplicationTarget::inline(ViewStylePatchId::new(7)),
        ];
        let instructions = [
            ViewInstruction::OpenElement(ViewElementSpec {
                kind: ViewElementKind::Panel,
                styles: applications.clone(),
                part: None,
                key: None,
            }),
            ViewInstruction::EmitText(ViewTextSpec {
                source: TextSourceId(1),
                styles: applications.clone(),
                part: None,
            }),
            ViewInstruction::EmitImage(ViewImageSpec {
                image: ImageId(2),
                styles: applications.clone(),
                part: None,
            }),
            ViewInstruction::EmitCustom(ViewCustomSpec {
                element: CustomElementId(3),
                styles: applications.clone(),
                part: None,
            }),
            ViewInstruction::CallView(ViewCall {
                view: view_id("view.nested"),
                arguments: Vec::new(),
                styles: applications.clone(),
                part: None,
                key: None,
            }),
        ];

        for instruction in &instructions {
            let styles = match instruction {
                ViewInstruction::OpenElement(spec) => &spec.styles,
                ViewInstruction::EmitText(spec) => &spec.styles,
                ViewInstruction::EmitImage(spec) => &spec.styles,
                ViewInstruction::EmitCustom(spec) => &spec.styles,
                ViewInstruction::CallView(call) => &call.styles,
                _ => panic!("test inventory contains only node-producing instructions"),
            };
            assert_eq!(styles, &applications);
        }
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
