//! Per-definition semantic fingerprints used by replacement invalidation.

use std::collections::BTreeSet;

use arcweft_bundle::resource_codec::view::{
    ViewDefinitionResource, ViewExportedPart, ViewHandlerRef, ViewProgramInstruction,
    ViewProgramResource, ViewValueInputNamespace, ViewValueInputResource,
};
use arcweft_view::{ViewId, ViewValueProgram, ViewValueProgramId};
use serde::Serialize;

use super::ViewProgramCatalogError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ViewDefinitionFingerprints {
    local: ViewSemanticFingerprint,
    exports: ViewSemanticFingerprint,
    direct_calls: BTreeSet<ViewId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ViewSemanticFingerprint([u8; 32]);

#[derive(Serialize)]
struct LocalTranscript<'a> {
    schema: &'static str,
    definition: &'a ViewDefinitionResource,
    instructions: Vec<ViewProgramInstruction>,
    value_programs: Vec<&'a ViewValueProgram>,
    value_inputs: Vec<&'a ViewValueInputResource>,
    handlers: Vec<&'a ViewHandlerRef>,
}

#[derive(Serialize)]
struct ExportTranscript<'a> {
    schema: &'static str,
    owner: &'a ViewId,
    exports: Vec<SemanticExport<'a>>,
}

#[derive(Serialize)]
struct SemanticExport<'a> {
    local_name: &'a arcweft_view::ViewPartLocalName,
    public_name: &'a arcweft_view::ViewPartName,
}

impl ViewDefinitionFingerprints {
    pub(super) fn try_new(
        resource: &ViewProgramResource,
        definition: &ViewDefinitionResource,
        view: &ViewId,
    ) -> Result<Self, ViewProgramCatalogError> {
        let body = definition_body(resource, definition, view)?;
        let value_program_ids = referenced_value_programs(definition, body);
        let value_programs = resource
            .value_programs
            .iter()
            .filter(|program| value_program_ids.contains(&program.id()))
            .collect::<Vec<_>>();
        let parameter_slots = value_programs
            .iter()
            .flat_map(|program| program.parameter_dependencies())
            .copied()
            .collect::<BTreeSet<_>>();
        let state_slots = value_programs
            .iter()
            .flat_map(|program| program.state_dependencies())
            .copied()
            .collect::<BTreeSet<_>>();
        let value_inputs = resource
            .value_inputs
            .iter()
            .filter(|input| match input.namespace {
                ViewValueInputNamespace::Parameter => parameter_slots.contains(&input.slot),
                ViewValueInputNamespace::State => state_slots.contains(&input.slot),
            })
            .collect::<Vec<_>>();
        let handler_ids = body
            .iter()
            .filter_map(|instruction| match instruction {
                ViewProgramInstruction::BindHandler { handler, .. } => Some(handler.as_str()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let handlers = resource
            .handlers
            .iter()
            .filter(|handler| handler_ids.contains(handler.handler_id.as_str()))
            .collect::<Vec<_>>();
        let local = LocalTranscript {
            schema: "arcweft.view.definition-local.v1",
            definition,
            instructions: body
                .iter()
                .cloned()
                .map(without_instruction_source)
                .collect(),
            value_programs,
            value_inputs,
            handlers,
        };
        let exports = exports_for(resource, definition);
        let export_transcript = ExportTranscript {
            schema: "arcweft.view.definition-exports.v1",
            owner: view,
            exports: exports
                .iter()
                .map(|export| SemanticExport {
                    local_name: &export.target.part,
                    public_name: &export.public_name,
                })
                .collect(),
        };

        Ok(Self {
            local: fingerprint("arcweft.view.definition-local-fingerprint.v1", &local)?,
            exports: fingerprint(
                "arcweft.view.definition-export-fingerprint.v1",
                &export_transcript,
            )?,
            direct_calls: body
                .iter()
                .filter_map(|instruction| match instruction {
                    ViewProgramInstruction::CallView { view, .. } => Some(view.view_id().clone()),
                    _ => None,
                })
                .collect(),
        })
    }

    pub(super) fn local_changed(&self, other: &Self) -> bool {
        self.local.0 != other.local.0
    }

    pub(super) fn exports_changed(&self, other: &Self) -> bool {
        self.exports.0 != other.exports.0
    }

    pub(super) const fn direct_calls(&self) -> &BTreeSet<ViewId> {
        &self.direct_calls
    }
}

fn definition_body<'a>(
    resource: &'a ViewProgramResource,
    definition: &ViewDefinitionResource,
    view: &ViewId,
) -> Result<&'a [ViewProgramInstruction], ViewProgramCatalogError> {
    let start = usize::try_from(definition.body.start_instruction)
        .map_err(|_| ViewProgramCatalogError::InvalidDefinitionSpan { view: view.clone() })?;
    let end = usize::try_from(definition.body.end_instruction)
        .map_err(|_| ViewProgramCatalogError::InvalidDefinitionSpan { view: view.clone() })?;
    resource
        .instructions
        .get(start..end)
        .ok_or_else(|| ViewProgramCatalogError::InvalidDefinitionSpan { view: view.clone() })
}

fn referenced_value_programs(
    definition: &ViewDefinitionResource,
    body: &[ViewProgramInstruction],
) -> BTreeSet<ViewValueProgramId> {
    let mut programs = definition
        .parameters
        .iter()
        .filter_map(|parameter| parameter.default_program)
        .collect::<BTreeSet<_>>();
    for instruction in body {
        match instruction {
            ViewProgramInstruction::CallView { arguments, .. } => {
                programs.extend(arguments.iter().map(|argument| argument.value_program));
            }
            ViewProgramInstruction::Branch {
                condition_program, ..
            } => {
                programs.insert(*condition_program);
            }
            ViewProgramInstruction::RepeatKeyed {
                source_program,
                key_program,
                ..
            } => {
                programs.extend([*source_program, *key_program]);
            }
            ViewProgramInstruction::Await { source_program, .. } => {
                programs.insert(*source_program);
            }
            ViewProgramInstruction::BindLocal { value_program, .. } => {
                programs.insert(*value_program);
            }
            ViewProgramInstruction::ApplyFx {
                arguments,
                key_program,
                ..
            } => {
                programs.extend(arguments.iter().map(|argument| argument.value_program));
                programs.extend(key_program);
            }
            ViewProgramInstruction::OpenElement { .. }
            | ViewProgramInstruction::CloseElement
            | ViewProgramInstruction::EmitText { .. }
            | ViewProgramInstruction::EmitImage { .. }
            | ViewProgramInstruction::EmitCustom { .. }
            | ViewProgramInstruction::BindHandler { .. }
            | ViewProgramInstruction::AttachSemantic { .. } => {}
        }
    }
    programs
}

fn exports_for<'a>(
    resource: &'a ViewProgramResource,
    definition: &ViewDefinitionResource,
) -> Vec<&'a ViewExportedPart> {
    resource
        .exported_parts
        .iter()
        .filter(|export| export.target.view == definition.public_id)
        .collect()
}

fn without_instruction_source(mut instruction: ViewProgramInstruction) -> ViewProgramInstruction {
    match &mut instruction {
        ViewProgramInstruction::OpenElement { source, .. }
        | ViewProgramInstruction::EmitText { source, .. }
        | ViewProgramInstruction::EmitImage { source, .. }
        | ViewProgramInstruction::EmitCustom { source, .. }
        | ViewProgramInstruction::CallView { source, .. }
        | ViewProgramInstruction::Branch { source, .. }
        | ViewProgramInstruction::RepeatKeyed { source, .. }
        | ViewProgramInstruction::Await { source, .. }
        | ViewProgramInstruction::BindLocal { source, .. }
        | ViewProgramInstruction::ApplyFx { source, .. }
        | ViewProgramInstruction::BindHandler { source, .. }
        | ViewProgramInstruction::AttachSemantic { source, .. } => *source = None,
        ViewProgramInstruction::CloseElement => {}
    }
    instruction
}

fn fingerprint(
    domain: &'static str,
    transcript: &impl Serialize,
) -> Result<ViewSemanticFingerprint, ViewProgramCatalogError> {
    let bytes =
        serde_json::to_vec(transcript).map_err(|_| ViewProgramCatalogError::FingerprintEncoding)?;
    let mut hasher = blake3::Hasher::new_derive_key(domain);
    hasher.update(&bytes);
    Ok(ViewSemanticFingerprint(*hasher.finalize().as_bytes()))
}
