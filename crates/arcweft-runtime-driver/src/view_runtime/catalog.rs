//! Immutable accepted View-program authority derived from complete-product typestate.

use std::collections::BTreeMap;

use arcweft_bundle::resource_codec::SourceSetRevision;
use arcweft_bundle::resource_codec::view::{
    ValidatedViewProduct, ViewDefinitionResource, ViewProgramInstruction, ViewProgramResource,
};
use arcweft_view::{
    AcceptedViewProgramRevision, CustomElementId, EventKind, HandlerId, ImageId, SemanticSpecId,
    TextSourceId, ViewAwait, ViewAwaitBranch, ViewBranch, ViewCall, ViewCallArgument,
    ViewCustomSpec, ViewElementSpec, ViewEvaluationSiteId, ViewEventBindingSpec,
    ViewFxApplicationInstruction, ViewFxCallArgument, ViewId, ViewImageSpec, ViewInstruction,
    ViewInstructionRange, ViewLocalBinding, ViewPartId, ViewPartStaticReachability, ViewProgram,
    ViewProgramBuildError, ViewProgramBuilder, ViewProgramId, ViewRepeat, ViewSemanticSpec,
    ViewStableKey, ViewTextSpec, ViewValueInventoryError, ViewValueProgramInventory,
};
use thiserror::Error;

use super::part::ViewPartRuntimeCatalog;

/// Dense definition position private to one immutable accepted catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ViewDefinitionIndex(u32);

#[derive(Clone, Debug)]
pub(super) struct RuntimeViewDefinition {
    pub(super) view: ViewId,
    pub(super) semantic: ViewProgram,
    execution: ViewDefinitionResource,
}

/// Canonically ordered typed View definitions accepted by the runtime.
#[derive(Clone, Debug)]
pub struct ViewProgramCatalog {
    program_id: ViewProgramId,
    revision: AcceptedViewProgramRevision,
    source_revision: SourceSetRevision,
    resource: ViewProgramResource,
    definitions: Vec<RuntimeViewDefinition>,
    by_view: BTreeMap<ViewId, ViewDefinitionIndex>,
    parts: ViewPartRuntimeCatalog,
}

/// Failure to adapt an already validated product into the immutable runtime catalog.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewProgramCatalogError {
    #[error("View definition count cannot be represented by the private catalog index")]
    DefinitionIndexOverflow,
    #[error("View program repeats public definition {0}")]
    DuplicateDefinition(ViewId),
    #[error("View definition {view} has an invalid instruction span")]
    InvalidDefinitionSpan { view: ViewId },
    #[error("View definition {view} uses unsupported handler event `{event}`")]
    UnsupportedHandlerEvent { view: ViewId, event: String },
    #[error("View semantic target `{target}` is not a valid public identity")]
    InvalidSemanticTarget { target: String },
    #[error("View instruction table index cannot be represented")]
    InstructionIndexOverflow,
    #[error(transparent)]
    ValueInventory(#[from] ViewValueInventoryError),
    #[error(transparent)]
    Program(#[from] ViewProgramBuildError),
}

impl ViewDefinitionIndex {
    fn try_from_index(index: usize) -> Result<Self, ViewProgramCatalogError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| ViewProgramCatalogError::DefinitionIndexOverflow)
    }

    pub(crate) fn index(self) -> usize {
        usize::try_from(self.0).expect("u32 View definition indexes fit the target usize")
    }
}

impl ViewProgramCatalog {
    pub(crate) fn try_from_validated(
        product: &ValidatedViewProduct,
    ) -> Result<Option<Self>, ViewProgramCatalogError> {
        let Some(validated) = product.program() else {
            return Ok(None);
        };
        let resource = validated.resource().clone();
        let inventory = ViewValueProgramInventory::from_programs(resource.value_programs.clone())?;
        let mut candidates = resource.definitions.clone();
        candidates.sort_by_key(|definition| definition.public_id.to_view_id());

        let mut definitions = Vec::with_capacity(candidates.len());
        let mut by_view = BTreeMap::new();
        for definition in candidates {
            let view = definition.public_id.to_view_id();
            let index = ViewDefinitionIndex::try_from_index(definitions.len())?;
            if by_view.insert(view.clone(), index).is_some() {
                return Err(ViewProgramCatalogError::DuplicateDefinition(view));
            }
            let semantic = build_definition(
                &resource,
                &definition,
                validated.program_id().clone(),
                &view,
                inventory.clone(),
            )?;
            definitions.push(RuntimeViewDefinition {
                view,
                semantic,
                execution: definition,
            });
        }
        let parts = ViewPartRuntimeCatalog::from_programs(&definitions);
        Ok(Some(Self {
            program_id: validated.program_id().clone(),
            revision: validated.accepted_revision(),
            source_revision: validated.source_set_revision(),
            resource,
            definitions,
            by_view,
            parts,
        }))
    }

    pub const fn program_id(&self) -> &ViewProgramId {
        &self.program_id
    }

    pub const fn revision(&self) -> AcceptedViewProgramRevision {
        self.revision
    }

    pub const fn source_revision(&self) -> SourceSetRevision {
        self.source_revision
    }

    pub fn definition(&self, view: &ViewId) -> Option<&ViewProgram> {
        self.definition_index(view)
            .map(|index| &self.definitions[index.index()].semantic)
    }

    pub fn definitions(&self) -> impl ExactSizeIterator<Item = (&ViewId, &ViewProgram)> {
        self.definitions
            .iter()
            .map(|definition| (&definition.view, &definition.semantic))
    }

    pub(crate) fn definition_index(&self, view: &ViewId) -> Option<ViewDefinitionIndex> {
        self.by_view.get(view).copied()
    }

    pub(crate) fn execution_definition(
        &self,
        index: ViewDefinitionIndex,
    ) -> &ViewDefinitionResource {
        &self.definitions[index.index()].execution
    }

    pub(crate) const fn resource(&self) -> &ViewProgramResource {
        &self.resource
    }

    pub(crate) const fn parts(&self) -> &ViewPartRuntimeCatalog {
        &self.parts
    }

    pub(crate) fn view_ids(&self) -> impl Iterator<Item = &ViewId> {
        self.by_view.keys()
    }
}

fn build_definition(
    resource: &ViewProgramResource,
    definition: &ViewDefinitionResource,
    program_id: ViewProgramId,
    view: &ViewId,
    inventory: ViewValueProgramInventory,
) -> Result<ViewProgram, ViewProgramCatalogError> {
    let start = usize::try_from(definition.body.start_instruction)
        .map_err(|_| ViewProgramCatalogError::InvalidDefinitionSpan { view: view.clone() })?;
    let end = usize::try_from(definition.body.end_instruction)
        .map_err(|_| ViewProgramCatalogError::InvalidDefinitionSpan { view: view.clone() })?;
    let body = resource
        .instructions
        .get(start..end)
        .ok_or_else(|| ViewProgramCatalogError::InvalidDefinitionSpan { view: view.clone() })?;

    let text_ids = canonical_ids(body.iter().filter_map(|instruction| match instruction {
        ViewProgramInstruction::EmitText { text_source, .. } => Some(text_source.as_str()),
        ViewProgramInstruction::AttachSemantic {
            label_text_source: Some(label),
            ..
        } => Some(label.as_str()),
        _ => None,
    }))?;
    let image_ids = canonical_ids(body.iter().filter_map(|instruction| match instruction {
        ViewProgramInstruction::EmitImage { image, .. } => Some(image.as_str()),
        _ => None,
    }))?;
    let custom_ids = canonical_ids(body.iter().filter_map(|instruction| match instruction {
        ViewProgramInstruction::EmitCustom { element, .. } => Some(element.as_str()),
        _ => None,
    }))?;
    let semantic_ids = canonical_ids(body.iter().filter_map(|instruction| match instruction {
        ViewProgramInstruction::AttachSemantic { target, .. } => Some(target.as_str()),
        _ => None,
    }))?;
    let handler_ids = canonical_ids(body.iter().filter_map(|instruction| match instruction {
        ViewProgramInstruction::BindHandler { handler, .. } => Some(handler.as_str()),
        _ => None,
    }))?;

    let mut builder =
        ViewProgramBuilder::new(program_id, view.clone(), definition.state_schema_hash);
    builder.set_value_programs(inventory);
    let mut authored_parts = Vec::new();
    for (local_index, instruction) in body.iter().enumerate() {
        let mapped = map_instruction(
            instruction,
            local_index,
            view,
            &text_ids,
            &image_ids,
            &custom_ids,
            &semantic_ids,
            &handler_ids,
        )?;
        let index = builder.push(mapped)?;
        if let Some(local) = instruction.part() {
            authored_parts.push((local.clone(), index));
        }
    }
    authored_parts.sort_by(|left, right| left.0.cmp(&right.0));
    let mut part_ids = BTreeMap::new();
    for (local, instruction) in authored_parts {
        let kind = match &body[instruction.index()] {
            ViewProgramInstruction::OpenElement { .. } => {
                arcweft_view::ViewPartInstructionKind::OpenElement
            }
            ViewProgramInstruction::EmitText { .. } => {
                arcweft_view::ViewPartInstructionKind::EmitText
            }
            ViewProgramInstruction::EmitImage { .. } => {
                arcweft_view::ViewPartInstructionKind::EmitImage
            }
            ViewProgramInstruction::EmitCustom { .. } => {
                arcweft_view::ViewPartInstructionKind::EmitCustom
            }
            ViewProgramInstruction::CallView { .. } => {
                arcweft_view::ViewPartInstructionKind::CallView
            }
            _ => unreachable!("only node-producing instructions carry parts"),
        };
        let site = ViewEvaluationSiteId::from_part(view, &local, kind);
        let id = builder.register_part(
            local.clone(),
            instruction,
            ViewPartStaticReachability::Reachable,
            site,
        )?;
        part_ids.insert(local, id);
    }
    let mut exports = resource
        .exported_parts
        .iter()
        .filter(|export| export.target.view == definition.public_id)
        .collect::<Vec<_>>();
    exports.sort_by(|left, right| left.public_name.cmp(&right.public_name));
    for export in exports {
        let part = part_ids.get(&export.target.part).copied().ok_or(
            ViewProgramBuildError::UnknownPart {
                part: ViewPartId::try_from_index(part_ids.len())
                    .map_err(|_| ViewProgramCatalogError::InstructionIndexOverflow)?,
            },
        )?;
        builder.export_part(part, export.public_name.clone())?;
    }
    builder.finish().map_err(Into::into)
}

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "this exhaustive adapter keeps every product instruction variant visibly mapped to its semantic View instruction; Increment 6 splits fingerprint and replacement responsibilities into sibling modules"
)]
fn map_instruction(
    instruction: &ViewProgramInstruction,
    local_index: usize,
    view: &ViewId,
    text_ids: &BTreeMap<String, u32>,
    image_ids: &BTreeMap<String, u32>,
    custom_ids: &BTreeMap<String, u32>,
    semantic_ids: &BTreeMap<String, u32>,
    handler_ids: &BTreeMap<String, u32>,
) -> Result<ViewInstruction, ViewProgramCatalogError> {
    Ok(match instruction {
        ViewProgramInstruction::OpenElement {
            element,
            styles,
            key,
            ..
        } => ViewInstruction::OpenElement(ViewElementSpec {
            kind: *element,
            styles: styles.clone(),
            part: None,
            key: key.map(ViewStableKey),
        }),
        ViewProgramInstruction::CloseElement => ViewInstruction::CloseElement,
        ViewProgramInstruction::EmitText {
            text_source,
            styles,
            ..
        } => ViewInstruction::EmitText(ViewTextSpec {
            source: TextSourceId(text_ids[text_source]),
            styles: styles.clone(),
            part: None,
        }),
        ViewProgramInstruction::EmitImage { image, styles, .. } => {
            ViewInstruction::EmitImage(ViewImageSpec {
                image: ImageId(image_ids[image]),
                styles: styles.clone(),
                part: None,
            })
        }
        ViewProgramInstruction::EmitCustom {
            element, styles, ..
        } => ViewInstruction::EmitCustom(ViewCustomSpec {
            element: CustomElementId(custom_ids[element]),
            styles: styles.clone(),
            part: None,
        }),
        ViewProgramInstruction::CallView {
            view: target,
            arguments,
            styles,
            key,
            ..
        } => ViewInstruction::CallView(ViewCall {
            view: target.to_view_id(),
            arguments: arguments
                .iter()
                .map(|argument| ViewCallArgument {
                    ordinal: argument.ordinal,
                    name: argument.name.clone(),
                    value: argument.value_program,
                })
                .collect(),
            styles: styles.clone(),
            part: None,
            key: key.map(ViewStableKey),
        }),
        ViewProgramInstruction::Branch {
            condition_program,
            then_span,
            else_span,
            ..
        } => {
            let start = u32::try_from(local_index + 1)
                .map_err(|_| ViewProgramCatalogError::InstructionIndexOverflow)?;
            let then_end = start
                .checked_add(*then_span)
                .ok_or(ViewProgramCatalogError::InstructionIndexOverflow)?;
            let else_range = else_span
                .map(|span| {
                    then_end
                        .checked_add(span)
                        .map(|end| ViewInstructionRange::new(then_end, end))
                        .ok_or(ViewProgramCatalogError::InstructionIndexOverflow)
                })
                .transpose()?;
            ViewInstruction::Branch(ViewBranch {
                condition: *condition_program,
                then_range: ViewInstructionRange::new(start, then_end),
                else_range,
            })
        }
        ViewProgramInstruction::RepeatKeyed {
            source_program,
            key_program,
            body_span,
            ..
        } => {
            let start = u32::try_from(local_index + 1)
                .map_err(|_| ViewProgramCatalogError::InstructionIndexOverflow)?;
            let end = start
                .checked_add(*body_span)
                .ok_or(ViewProgramCatalogError::InstructionIndexOverflow)?;
            ViewInstruction::RepeatKeyed(ViewRepeat {
                source: *source_program,
                key: *key_program,
                body: ViewInstructionRange::new(start, end),
            })
        }
        ViewProgramInstruction::Await {
            source_program,
            pending_branch,
            ready_branch,
            error_branch,
            denied_branch,
            ..
        } => ViewInstruction::Await(ViewAwait {
            source: *source_program,
            pending: pending_branch.as_ref().map(map_await_branch),
            ready: ready_branch.as_ref().map(map_await_branch),
            error: error_branch.as_ref().map(map_await_branch),
            denied: denied_branch.as_ref().map(map_await_branch),
        }),
        ViewProgramInstruction::BindLocal {
            binding,
            value_program,
            ..
        } => ViewInstruction::BindLocal(ViewLocalBinding {
            binding: binding.clone(),
            value: *value_program,
        }),
        ViewProgramInstruction::ApplyFx {
            fx,
            arguments,
            key_program,
            application_ordinal,
            ..
        } => ViewInstruction::ApplyFx(ViewFxApplicationInstruction {
            fx: fx.clone(),
            arguments: arguments
                .iter()
                .map(|argument| ViewFxCallArgument {
                    parameter: argument.parameter.clone(),
                    value: argument.value_program,
                })
                .collect(),
            key: *key_program,
            application_ordinal: *application_ordinal,
        }),
        ViewProgramInstruction::BindHandler { event, handler, .. } => {
            ViewInstruction::BindEvent(ViewEventBindingSpec {
                event: parse_event(event).ok_or_else(|| {
                    ViewProgramCatalogError::UnsupportedHandlerEvent {
                        view: view.clone(),
                        event: event.clone(),
                    }
                })?,
                handler: HandlerId(handler_ids[handler]),
            })
        }
        ViewProgramInstruction::AttachSemantic {
            target,
            label_text_source,
            ..
        } => ViewInstruction::AttachSemantic(ViewSemanticSpec {
            semantic: SemanticSpecId(semantic_ids[target]),
            target: arcweft_bundle::resource_codec::view::ViewDefinitionRef::try_new(
                target.clone(),
            )
            .map_err(|_| ViewProgramCatalogError::InvalidSemanticTarget {
                target: target.clone(),
            })?
            .public_id()
            .clone(),
            label: label_text_source
                .as_ref()
                .map(|label| TextSourceId(text_ids[label])),
        }),
    })
}

fn canonical_ids<'a>(
    values: impl Iterator<Item = &'a str>,
) -> Result<BTreeMap<String, u32>, ViewProgramCatalogError> {
    let mut values = values.map(str::to_owned).collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            u32::try_from(index)
                .map(|index| (value, index))
                .map_err(|_| ViewProgramCatalogError::InstructionIndexOverflow)
        })
        .collect()
}

fn map_await_branch(
    branch: &arcweft_bundle::resource_codec::view::ViewAwaitBranchSpan,
) -> ViewAwaitBranch {
    ViewAwaitBranch {
        start_offset: branch.start_offset,
        body_span: branch.body_span,
    }
}

fn parse_event(event: &str) -> Option<EventKind> {
    match event {
        "activate" => Some(EventKind::Activate),
        "pointer_down" => Some(EventKind::PointerDown),
        "pointer_up" => Some(EventKind::PointerUp),
        "pointer_move" => Some(EventKind::PointerMove),
        "focus" => Some(EventKind::Focus),
        "blur" => Some(EventKind::Blur),
        _ => None,
    }
}
