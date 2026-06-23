use crate::container::{
    BundleDigest, BundleKind as ContainerBundleKind, BundleSectionKind, BundleView,
    ContentResidency, ExternalSectionPayload, ReadBudget, SectionId, SectionInput, encode_bundle,
};
use crate::{
    ARCWEFT_BUNDLE_SCHEMA_VERSION, ArcweftBundle, BundleAdapterManifest, BundleBytecodeProgram,
    BundleCodecError, BundleImageAsset, BundleImageObject, BundleKind, BundleManifest,
    BundleSource, BundleVirtualFile,
};
use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_audio_core::graph::AudioGraph;
use arcweft_core::bytecode::{BytecodeProgram, BytecodeRuntimeLayout};
use arcweft_core::compact_bytecode::{
    CompactBytecodeFunction, CompactBytecodeProgram, CompactBytecodeValidationBudget,
    CompactCodeSlotId, CompactInstruction, CompactOpcode, CompactRuntimeSignature,
    CompactRuntimeTypeId,
};
use arcweft_core::plan::{FlowOp, FlowRuntimeId};
use arcweft_render_text::LineDisplayCatalog;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const AWFB_SECTION_SCHEMA_VERSION: u32 = 1;
const BYTECODE_SECTION_MAGIC: [u8; 8] = *b"AWBC\r\n\x1a\n";
const BYTECODE_SECTION_ENVELOPE_VERSION: u32 = 1;
const BYTECODE_SECTION_STRUCTURED_MESSAGEPACK: u32 = 1;
const BYTECODE_SECTION_STRUCTURED_WITH_COMPACT_TABLE: u32 = 2;
const COMPACT_TABLE_PAYLOAD_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProductManifest {
    schema_version: u32,
    bundle_kind: BundleKind,
    manifest: BundleManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<AgentArtifactManifest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeTypesSection {
    schema_version: u32,
    #[serde(default)]
    runtime_layout: BytecodeRuntimeLayout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EntrypointsSection {
    entry: Option<String>,
    entry_flow: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AdapterRequirementsSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    adapter_manifest_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    required_host_calls: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    adapter_manifests: Vec<BundleAdapterManifest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContentCatalogSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    virtual_files: Vec<BundleVirtualFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    image_assets: Vec<BundleImageAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audio: Option<AudioGraph>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    image_objects: Vec<BundleImageObject>,
}

pub(crate) fn to_awfb_bytes(bundle: &ArcweftBundle) -> Result<Vec<u8>, BundleCodecError> {
    let product_manifest = ProductManifest {
        schema_version: bundle.schema_version,
        bundle_kind: bundle.bundle_kind,
        manifest: bundle.manifest.clone(),
        agent: bundle.agent.clone(),
    };
    let manifest = encode_json(&product_manifest)?;
    let sections = vec![
        required_section(
            BundleSectionKind::ProgramBytecode,
            encode_program_bytecode_section(&bundle.bytecode)?,
        ),
        required_section(
            BundleSectionKind::RuntimeTypes,
            encode_json(&RuntimeTypesSection {
                schema_version: AWFB_SECTION_SCHEMA_VERSION,
                runtime_layout: bundle.bytecode.program.runtime_layout.clone(),
            })?,
        ),
        required_section(
            BundleSectionKind::Entrypoints,
            encode_json(&EntrypointsSection {
                entry: bundle.manifest.entry.clone(),
                entry_flow: bundle.manifest.runtime.entry_flow.clone(),
            })?,
        ),
        required_section(
            BundleSectionKind::AdapterRequirements,
            encode_json(&AdapterRequirementsSection {
                adapter_manifest_ids: bundle.manifest.adapter_manifest_ids.clone(),
                required_host_calls: bundle.manifest.required_host_calls.clone(),
                adapter_manifests: bundle.adapter_manifests.clone(),
            })?,
        ),
        required_section(
            BundleSectionKind::ContentCatalog,
            encode_json(&ContentCatalogSection {
                virtual_files: bundle.virtual_files.clone(),
                image_assets: bundle.image_assets.clone(),
                audio: bundle.audio.clone(),
                image_objects: bundle.image_objects.clone(),
            })?,
        ),
        optional_section(
            BundleSectionKind::DisplayCatalog,
            encode_json(&bundle.display)?,
        ),
        optional_section(
            BundleSectionKind::NormalizedSource,
            encode_json(&bundle.source)?,
        ),
    ];
    encode_bundle(container_kind(bundle.bundle_kind), &manifest, sections).map_err(|error| {
        BundleCodecError::EncodeAwfb {
            message: error.to_string(),
        }
    })
}

pub(crate) fn from_awfb_slice(bytes: &[u8]) -> Result<ArcweftBundle, BundleCodecError> {
    from_awfb_slice_with_external_sections(bytes, &[])
}

pub(crate) fn from_awfb_slice_with_external_sections(
    bytes: &[u8],
    external_sections: &[ExternalSectionPayload],
) -> Result<ArcweftBundle, BundleCodecError> {
    let view = BundleView::parse(bytes, ReadBudget::default()).map_err(|error| {
        BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        }
    })?;
    let product_manifest = decode_json::<ProductManifest>(view.manifest())?;
    if product_manifest.schema_version != ARCWEFT_BUNDLE_SCHEMA_VERSION {
        return Err(BundleCodecError::UnsupportedSchema {
            actual: product_manifest.schema_version,
            expected: ARCWEFT_BUNDLE_SCHEMA_VERSION,
        });
    }
    if view.kind() != container_kind(product_manifest.bundle_kind) {
        return Err(BundleCodecError::DecodeAwfb {
            message: "container kind does not match product manifest bundle kind".to_owned(),
        });
    }

    let runtime_types = required_payload::<RuntimeTypesSection>(
        &view,
        external_sections,
        BundleSectionKind::RuntimeTypes,
    )?;
    let _entrypoints = required_payload::<EntrypointsSection>(
        &view,
        external_sections,
        BundleSectionKind::Entrypoints,
    )?;
    let adapters = required_payload::<AdapterRequirementsSection>(
        &view,
        external_sections,
        BundleSectionKind::AdapterRequirements,
    )?;
    let content = required_payload::<ContentCatalogSection>(
        &view,
        external_sections,
        BundleSectionKind::ContentCatalog,
    )?;
    let bytecode = required_bytes(&view, external_sections, BundleSectionKind::ProgramBytecode)
        .and_then(|bytes| decode_program_bytecode_section(&bytes))?;
    if runtime_types.runtime_layout != bytecode.program.runtime_layout {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!(
                "runtime types layout `{}` does not match bytecode layout `{}`",
                runtime_types.runtime_layout.label(),
                bytecode.program.runtime_layout.label()
            ),
        });
    }
    let display = optional_payload::<LineDisplayCatalog>(
        &view,
        external_sections,
        BundleSectionKind::DisplayCatalog,
    )?
    .unwrap_or_default();
    let source = optional_payload::<BundleSource>(
        &view,
        external_sections,
        BundleSectionKind::NormalizedSource,
    )?
    .unwrap_or_else(|| BundleSource {
        label: product_manifest.manifest.source_label.clone(),
        text: String::new(),
    });

    Ok(ArcweftBundle {
        schema_version: product_manifest.schema_version,
        bundle_kind: product_manifest.bundle_kind,
        manifest: product_manifest.manifest,
        agent: product_manifest.agent,
        source,
        bytecode,
        display,
        adapter_manifests: adapters.adapter_manifests,
        virtual_files: content.virtual_files,
        image_assets: content.image_assets,
        audio: content.audio,
        image_objects: content.image_objects,
    })
}

fn required_section(kind: BundleSectionKind, bytes: Vec<u8>) -> SectionInput {
    SectionInput::embedded(
        section_id(kind),
        kind,
        AWFB_SECTION_SCHEMA_VERSION,
        ContentResidency::Startup,
        true,
        bytes,
    )
}

fn optional_section(kind: BundleSectionKind, bytes: Vec<u8>) -> SectionInput {
    SectionInput::embedded(
        section_id(kind),
        kind,
        AWFB_SECTION_SCHEMA_VERSION,
        kind.default_residency(),
        false,
        bytes,
    )
}

fn required_payload<T>(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
    kind: BundleSectionKind,
) -> Result<T, BundleCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    optional_payload(view, external_sections, kind)?.ok_or_else(|| BundleCodecError::DecodeAwfb {
        message: format!("AWFB bundle is missing required {kind:?} section"),
    })
}

fn required_bytes(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
    kind: BundleSectionKind,
) -> Result<Vec<u8>, BundleCodecError> {
    let descriptor = required_descriptor(view, kind)?;
    view.decoded_section_with_external_payloads(descriptor.id(), external_sections)
        .map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })?
        .ok_or_else(|| BundleCodecError::DecodeAwfb {
            message: format!("AWFB {kind:?} section is external and cannot be decoded inline"),
        })
}

fn required_descriptor<'a>(
    view: &'a BundleView<'_>,
    kind: BundleSectionKind,
) -> Result<&'a crate::container::SectionDescriptor, BundleCodecError> {
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.kind() == kind);
    let Some(descriptor) = matches.next() else {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB bundle is missing required {kind:?} section"),
        });
    };
    if matches.next().is_some() {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB bundle contains multiple {kind:?} sections"),
        });
    }
    Ok(descriptor)
}

fn optional_payload<T>(
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
    kind: BundleSectionKind,
) -> Result<Option<T>, BundleCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.kind() == kind);
    let Some(descriptor) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB bundle contains multiple {kind:?} sections"),
        });
    }
    let Some(bytes) = view
        .decoded_section_with_external_payloads(descriptor.id(), external_sections)
        .map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })?
    else {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!("AWFB {kind:?} section is external and cannot be decoded inline"),
        });
    };
    decode_json(&bytes).map(Some)
}

fn encode_program_bytecode_section(
    bytecode: &BundleBytecodeProgram,
) -> Result<Vec<u8>, BundleCodecError> {
    let compact = compact_validation_table(&bytecode.program);
    compact
        .verify(CompactBytecodeValidationBudget::default())
        .map_err(|error| BundleCodecError::EncodeAwfb {
            message: error.to_string(),
        })?;
    let compact_payload = encode_compact_table(&compact)?;
    let structured_payload = encode_json(&bytecode.program)?;
    let compact_len = checked_u32_len(compact_payload.len(), "compact bytecode table")?;
    let structured_len = checked_u32_len(structured_payload.len(), "structured bytecode program")?;

    let mut section = Vec::with_capacity(24 + compact_payload.len() + structured_payload.len());
    section.extend_from_slice(&BYTECODE_SECTION_MAGIC);
    section.extend_from_slice(&BYTECODE_SECTION_ENVELOPE_VERSION.to_le_bytes());
    section.extend_from_slice(&BYTECODE_SECTION_STRUCTURED_WITH_COMPACT_TABLE.to_le_bytes());
    section.extend_from_slice(&compact_len.to_le_bytes());
    section.extend_from_slice(&structured_len.to_le_bytes());
    section.extend_from_slice(&compact_payload);
    section.extend_from_slice(&structured_payload);
    Ok(section)
}

fn decode_program_bytecode_section(
    bytes: &[u8],
) -> Result<BundleBytecodeProgram, BundleCodecError> {
    if !bytes.starts_with(&BYTECODE_SECTION_MAGIC) {
        return decode_json(bytes);
    }
    if bytes.len() < 16 {
        return Err(BundleCodecError::DecodeAwfb {
            message: "AWFB ProgramBytecode envelope is truncated".to_owned(),
        });
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().expect("slice length checked"));
    if version != BYTECODE_SECTION_ENVELOPE_VERSION {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!(
                "unsupported AWFB ProgramBytecode envelope version {version}; expected {BYTECODE_SECTION_ENVELOPE_VERSION}"
            ),
        });
    }
    let encoding = u32::from_le_bytes(bytes[12..16].try_into().expect("slice length checked"));
    let program = match encoding {
        BYTECODE_SECTION_STRUCTURED_MESSAGEPACK => {
            decode_structured_messagepack_program(&bytes[16..])?
        }
        BYTECODE_SECTION_STRUCTURED_WITH_COMPACT_TABLE => {
            decode_structured_program_with_compact_table(&bytes[16..])?
        }
        _ => {
            return Err(BundleCodecError::DecodeAwfb {
                message: format!("unknown AWFB ProgramBytecode encoding tag {encoding}"),
            });
        }
    };
    Ok(BundleBytecodeProgram {
        encoding: crate::BundleBytecodeEncoding::StructuredJson,
        program,
    })
}

fn decode_structured_program_with_compact_table(
    bytes: &[u8],
) -> Result<BytecodeProgram, BundleCodecError> {
    if bytes.len() < 8 {
        return Err(BundleCodecError::DecodeAwfb {
            message: "AWFB ProgramBytecode compact payload header is truncated".to_owned(),
        });
    }
    let compact_len =
        u32::from_le_bytes(bytes[..4].try_into().expect("slice length checked")) as usize;
    let structured_len =
        u32::from_le_bytes(bytes[4..8].try_into().expect("slice length checked")) as usize;
    let payload_len = compact_len
        .checked_add(structured_len)
        .and_then(|len| len.checked_add(8))
        .ok_or_else(|| BundleCodecError::DecodeAwfb {
            message: "AWFB ProgramBytecode compact payload lengths overflow".to_owned(),
        })?;
    if bytes.len() != payload_len {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!(
                "AWFB ProgramBytecode compact payload length mismatch: header declares {payload_len} bytes, section has {} bytes",
                bytes.len()
            ),
        });
    }
    let compact_end = 8 + compact_len;
    let compact = decode_compact_table(&bytes[8..compact_end])?;
    compact
        .verify(CompactBytecodeValidationBudget::default())
        .map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })?;
    decode_json(&bytes[compact_end..])
}

#[cfg(feature = "format-messagepack")]
fn decode_structured_messagepack_program(
    bytes: &[u8],
) -> Result<arcweft_core::bytecode::BytecodeProgram, BundleCodecError> {
    rmp_serde::from_slice(bytes).map_err(|error| BundleCodecError::DecodeAwfb {
        message: error.to_string(),
    })
}

#[cfg(not(feature = "format-messagepack"))]
fn decode_structured_messagepack_program(
    _bytes: &[u8],
) -> Result<arcweft_core::bytecode::BytecodeProgram, BundleCodecError> {
    Err(BundleCodecError::DecodeAwfb {
        message: "AWFB ProgramBytecode MessagePack support is not enabled".to_owned(),
    })
}

fn compact_validation_table(program: &BytecodeProgram) -> CompactBytecodeProgram {
    let flow_slots = program
        .flows
        .iter()
        .enumerate()
        .map(|(index, flow)| (flow.id.clone(), u32::try_from(index).unwrap_or(u32::MAX)))
        .collect::<BTreeMap<_, _>>();
    let functions = program
        .flows
        .iter()
        .enumerate()
        .map(|(index, flow)| CompactBytecodeFunction {
            slot: CompactCodeSlotId(u32::try_from(index).unwrap_or(u32::MAX)),
            signature: CompactRuntimeSignature::default(),
            instructions: compact_instructions(&flow.instructions, &flow_slots),
        })
        .collect();
    CompactBytecodeProgram {
        abi_version: program.abi_version,
        runtime_type_count: 1,
        constant_count: 0,
        content_unit_count: u32::try_from(program.line_task_groups.len()).unwrap_or(u32::MAX),
        functions,
    }
}

fn compact_instructions(
    instructions: &[arcweft_core::bytecode::BytecodeInstruction],
    flow_slots: &BTreeMap<FlowRuntimeId, u32>,
) -> Vec<CompactInstruction> {
    instructions
        .iter()
        .flat_map(|instruction| match instruction {
            arcweft_core::bytecode::BytecodeInstruction::Flow(op) => {
                compact_flow_op(op, flow_slots)
            }
        })
        .collect()
}

fn compact_flow_op(
    op: &FlowOp,
    flow_slots: &BTreeMap<FlowRuntimeId, u32>,
) -> Vec<CompactInstruction> {
    let mut instructions = Vec::new();
    collect_compact_flow_op(op, flow_slots, &mut instructions);
    instructions
}

fn collect_compact_flow_ops(
    ops: &[FlowOp],
    flow_slots: &BTreeMap<FlowRuntimeId, u32>,
    out: &mut Vec<CompactInstruction>,
) {
    for op in ops {
        collect_compact_flow_op(op, flow_slots, out);
    }
}

fn collect_compact_flow_op(
    op: &FlowOp,
    flow_slots: &BTreeMap<FlowRuntimeId, u32>,
    out: &mut Vec<CompactInstruction>,
) {
    match op {
        FlowOp::Dialogue { task_group, .. } => out.push(CompactInstruction {
            opcode: CompactOpcode::EnsureContent.encoded(),
            operand: u32::try_from(*task_group).unwrap_or(u32::MAX),
        }),
        FlowOp::Choice { options, .. } => options
            .iter()
            .filter_map(|option| option.target.as_ref())
            .for_each(|target| push_flow_call(target, flow_slots, out)),
        FlowOp::Goto(target) => push_flow_call(target, flow_slots, out),
        FlowOp::LetElse { else_ops, .. } => collect_compact_flow_ops(else_ops, flow_slots, out),
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => {
            collect_compact_flow_ops(then_ops, flow_slots, out);
            collect_compact_flow_ops(else_ops, flow_slots, out);
        }
        FlowOp::Match { arms, .. } => arms
            .iter()
            .for_each(|arm| collect_compact_flow_ops(&arm.ops, flow_slots, out)),
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::For { body, .. }
        | FlowOp::Thread { body, .. } => collect_compact_flow_ops(body, flow_slots, out),
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. }
        | FlowOp::ForNext { body, .. } => collect_compact_flow_ops(body, flow_slots, out),
        FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => {
            collect_compact_flow_ops(ops, flow_slots, out);
        }
        FlowOp::Return(_) | FlowOp::ReturnExpr(_) => out.push(CompactInstruction {
            opcode: CompactOpcode::Return.encoded(),
            operand: 0,
        }),
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::Await { .. }
        | FlowOp::AwaitMany { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::GotoExpr(_)
        | FlowOp::Effect(_)
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => {}
    }
}

fn push_flow_call(
    target: &FlowRuntimeId,
    flow_slots: &BTreeMap<FlowRuntimeId, u32>,
    out: &mut Vec<CompactInstruction>,
) {
    out.push(CompactInstruction {
        opcode: CompactOpcode::Call.encoded(),
        operand: flow_slots.get(target).copied().unwrap_or(u32::MAX),
    });
}

fn encode_compact_table(program: &CompactBytecodeProgram) -> Result<Vec<u8>, BundleCodecError> {
    let function_count = checked_u32_len(program.functions.len(), "compact bytecode functions")?;
    let mut bytes = Vec::new();
    push_u32(&mut bytes, COMPACT_TABLE_PAYLOAD_VERSION);
    push_u32(&mut bytes, program.abi_version);
    push_u32(&mut bytes, program.runtime_type_count);
    push_u32(&mut bytes, program.constant_count);
    push_u32(&mut bytes, program.content_unit_count);
    push_u32(&mut bytes, function_count);
    for function in &program.functions {
        let param_count =
            checked_u32_len(function.signature.params.len(), "compact signature params")?;
        let instruction_count =
            checked_u32_len(function.instructions.len(), "compact instructions")?;
        push_u32(&mut bytes, function.slot.0);
        push_u32(&mut bytes, param_count);
        function
            .signature
            .params
            .iter()
            .for_each(|ty| push_u32(&mut bytes, ty.0));
        push_u32(&mut bytes, function.signature.result.0);
        bytes.extend_from_slice(&function.signature.effects.0);
        push_u32(&mut bytes, instruction_count);
        function.instructions.iter().for_each(|instruction| {
            bytes.push(instruction.opcode);
            push_u32(&mut bytes, instruction.operand);
        });
    }
    Ok(bytes)
}

fn decode_compact_table(bytes: &[u8]) -> Result<CompactBytecodeProgram, BundleCodecError> {
    let mut reader = CompactTableReader::new(bytes);
    let version = reader.read_u32()?;
    if version != COMPACT_TABLE_PAYLOAD_VERSION {
        return Err(BundleCodecError::DecodeAwfb {
            message: format!(
                "unsupported AWFB compact bytecode table version {version}; expected {COMPACT_TABLE_PAYLOAD_VERSION}"
            ),
        });
    }
    let abi_version = reader.read_u32()?;
    let runtime_type_count = reader.read_u32()?;
    let constant_count = reader.read_u32()?;
    let content_unit_count = reader.read_u32()?;
    let function_count = reader.read_u32()? as usize;
    let functions = (0..function_count)
        .map(|_| decode_compact_function(&mut reader))
        .collect::<Result<Vec<_>, _>>()?;
    reader.finish()?;
    Ok(CompactBytecodeProgram {
        abi_version,
        runtime_type_count,
        constant_count,
        content_unit_count,
        functions,
    })
}

fn decode_compact_function(
    reader: &mut CompactTableReader<'_>,
) -> Result<CompactBytecodeFunction, BundleCodecError> {
    let slot = CompactCodeSlotId(reader.read_u32()?);
    let param_count = reader.read_u32()? as usize;
    let params = (0..param_count)
        .map(|_| reader.read_u32().map(CompactRuntimeTypeId))
        .collect::<Result<Vec<_>, _>>()?;
    let result = CompactRuntimeTypeId(reader.read_u32()?);
    let effects = reader.read_effect_digest()?;
    let instruction_count = reader.read_u32()? as usize;
    let instructions = (0..instruction_count)
        .map(|_| {
            let opcode = reader.read_u8()?;
            let operand = reader.read_u32()?;
            Ok(CompactInstruction { opcode, operand })
        })
        .collect::<Result<Vec<_>, BundleCodecError>>()?;
    Ok(CompactBytecodeFunction {
        slot,
        signature: CompactRuntimeSignature {
            params,
            result,
            effects,
        },
        instructions,
    })
}

struct CompactTableReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> CompactTableReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, BundleCodecError> {
        let Some(value) = self.bytes.get(self.offset).copied() else {
            return Err(Self::truncated_error());
        };
        self.offset += 1;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, BundleCodecError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn read_effect_digest(
        &mut self,
    ) -> Result<arcweft_core::compact_bytecode::CompactEffectDigest, BundleCodecError> {
        let bytes = self.read_exact(32)?;
        let digest = bytes.try_into().expect("slice length checked");
        Ok(arcweft_core::compact_bytecode::CompactEffectDigest(digest))
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], BundleCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(Self::truncated_error)?;
        if end > self.bytes.len() {
            return Err(Self::truncated_error());
        }
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), BundleCodecError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(BundleCodecError::DecodeAwfb {
                message: format!(
                    "AWFB compact bytecode table has {} trailing bytes",
                    self.bytes.len() - self.offset
                ),
            })
        }
    }

    fn truncated_error() -> BundleCodecError {
        BundleCodecError::DecodeAwfb {
            message: "AWFB compact bytecode table is truncated".to_owned(),
        }
    }
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn checked_u32_len(len: usize, label: &'static str) -> Result<u32, BundleCodecError> {
    u32::try_from(len).map_err(|_| BundleCodecError::EncodeAwfb {
        message: format!("{label} exceed u32 length limit"),
    })
}

fn encode_json<T>(value: &T) -> Result<Vec<u8>, BundleCodecError>
where
    T: Serialize,
{
    serde_json::to_vec(value).map_err(|error| BundleCodecError::EncodeAwfb {
        message: error.to_string(),
    })
}

fn decode_json<T>(bytes: &[u8]) -> Result<T, BundleCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_slice(bytes).map_err(|error| BundleCodecError::DecodeAwfb {
        message: error.to_string(),
    })
}

fn container_kind(kind: BundleKind) -> ContainerBundleKind {
    match kind {
        BundleKind::Game => ContainerBundleKind::Program,
        BundleKind::AgentController => ContainerBundleKind::AgentController,
    }
}

fn section_id(kind: BundleSectionKind) -> SectionId {
    let mut id = [0_u8; 16];
    id[..4].copy_from_slice(&kind.encoded().to_le_bytes());
    id[4..].copy_from_slice(&BundleDigest::of(b"arcweft-awfb-v1-section").as_bytes()[..12]);
    SectionId::from_bytes(id)
}

#[cfg(test)]
mod tests {
    use super::{
        AWFB_SECTION_SCHEMA_VERSION, AdapterRequirementsSection, BYTECODE_SECTION_MAGIC,
        BYTECODE_SECTION_STRUCTURED_WITH_COMPACT_TABLE, ContentCatalogSection, EntrypointsSection,
        ProductManifest, RuntimeTypesSection, container_kind, decode_compact_table, encode_json,
        encode_program_bytecode_section, optional_section, required_section, section_id,
    };
    use crate::container::{
        BundleDigest, BundleSectionKind, BundleView, ExternalSectionPayload, ReadBudget,
        SectionInput, encode_bundle,
    };
    use crate::{
        ArcweftBundle, BundleCodecError, BundleFormat, BundleManifest, BundleRuntimeSummary,
        BundleSource,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_render_text::LineDisplayCatalog;
    use std::path::Path;

    #[test]
    fn awfb_product_decodes_verified_external_program_bytecode_section() {
        let bundle = empty_bundle();
        let (bytes, payload) = awfb_with_external_bytecode(&bundle);

        let inline_error =
            super::from_awfb_slice(&bytes).expect_err("external bytecode requires payload");
        assert!(matches!(inline_error, BundleCodecError::DecodeAwfb { .. }));

        let decoded = ArcweftBundle::from_awfb_slice_with_external_sections(&bytes, &[payload])
            .expect("external bytecode payload decodes");

        assert_eq!(decoded, bundle);
    }

    #[test]
    fn awfb_product_path_decode_accepts_external_sections() {
        let bundle = empty_bundle();
        let (bytes, payload) = awfb_with_external_bytecode(&bundle);

        let decoded = ArcweftBundle::from_product_path_slice_with_external_sections(
            Path::new("game.awfb"),
            &bytes,
            &[payload],
        )
        .expect("external product payload decodes");

        assert_eq!(decoded, bundle);
    }

    #[test]
    fn awfb_product_encodes_program_bytecode_as_binary_envelope() {
        let bundle = empty_bundle();
        let bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect("AWFB encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        let descriptor = view
            .sections()
            .iter()
            .find(|section| section.kind() == BundleSectionKind::ProgramBytecode)
            .expect("program bytecode section exists");
        let bytecode_bytes = view
            .decoded_section(descriptor.id())
            .expect("bytecode section decodes")
            .expect("bytecode section is embedded");

        assert!(bytecode_bytes.starts_with(&BYTECODE_SECTION_MAGIC));
        assert_ne!(bytecode_bytes.first(), Some(&b'{'));
        assert_eq!(
            u32::from_le_bytes(bytecode_bytes[12..16].try_into().expect("encoding tag")),
            BYTECODE_SECTION_STRUCTURED_WITH_COMPACT_TABLE
        );
        assert_eq!(
            super::from_awfb_slice(&bytes).expect("binary bytecode AWFB decodes"),
            bundle
        );
    }

    #[test]
    fn awfb_product_embeds_verified_compact_bytecode_table() {
        let bundle = empty_bundle();
        let bytecode_bytes =
            encode_program_bytecode_section(&bundle.bytecode).expect("bytecode encodes");
        let compact_len =
            u32::from_le_bytes(bytecode_bytes[16..20].try_into().expect("compact length")) as usize;
        let compact = decode_compact_table(&bytecode_bytes[24..24 + compact_len])
            .expect("compact table decodes");

        compact
            .verify(arcweft_core::compact_bytecode::CompactBytecodeValidationBudget::default())
            .expect("compact table verifies");
        assert_eq!(compact.abi_version, bundle.bytecode.program.abi_version);
    }

    fn awfb_with_external_bytecode(bundle: &ArcweftBundle) -> (Vec<u8>, ExternalSectionPayload) {
        let product_manifest = ProductManifest {
            schema_version: bundle.schema_version,
            bundle_kind: bundle.bundle_kind,
            manifest: bundle.manifest.clone(),
            agent: bundle.agent.clone(),
        };
        let manifest = encode_json(&product_manifest).expect("manifest encodes");
        let bytecode_bytes =
            encode_program_bytecode_section(&bundle.bytecode).expect("bytecode encodes");
        let bytecode_id = section_id(BundleSectionKind::ProgramBytecode);
        let sections = vec![
            SectionInput::external_ref(
                bytecode_id,
                BundleSectionKind::ProgramBytecode,
                AWFB_SECTION_SCHEMA_VERSION,
                BundleSectionKind::ProgramBytecode.default_residency(),
                true,
                bytecode_bytes.len() as u64,
                BundleDigest::of(&bytecode_bytes),
            ),
            required_section(
                BundleSectionKind::RuntimeTypes,
                encode_json(&RuntimeTypesSection {
                    schema_version: AWFB_SECTION_SCHEMA_VERSION,
                    runtime_layout: bundle.bytecode.program.runtime_layout.clone(),
                })
                .expect("runtime types encode"),
            ),
            required_section(
                BundleSectionKind::Entrypoints,
                encode_json(&EntrypointsSection {
                    entry: bundle.manifest.entry.clone(),
                    entry_flow: bundle.manifest.runtime.entry_flow.clone(),
                })
                .expect("entrypoints encode"),
            ),
            required_section(
                BundleSectionKind::AdapterRequirements,
                encode_json(&AdapterRequirementsSection {
                    adapter_manifest_ids: bundle.manifest.adapter_manifest_ids.clone(),
                    required_host_calls: bundle.manifest.required_host_calls.clone(),
                    adapter_manifests: bundle.adapter_manifests.clone(),
                })
                .expect("adapter requirements encode"),
            ),
            required_section(
                BundleSectionKind::ContentCatalog,
                encode_json(&ContentCatalogSection {
                    virtual_files: bundle.virtual_files.clone(),
                    image_assets: bundle.image_assets.clone(),
                    audio: bundle.audio.clone(),
                    image_objects: bundle.image_objects.clone(),
                })
                .expect("content catalog encode"),
            ),
            optional_section(
                BundleSectionKind::DisplayCatalog,
                encode_json(&bundle.display).expect("display catalog encode"),
            ),
            optional_section(
                BundleSectionKind::NormalizedSource,
                encode_json(&bundle.source).expect("source encode"),
            ),
        ];
        let bytes = encode_bundle(container_kind(bundle.bundle_kind), &manifest, sections)
            .expect("AWFB encodes");
        (
            bytes,
            ExternalSectionPayload::new(bytecode_id, bytecode_bytes),
        )
    }

    fn empty_bundle() -> ArcweftBundle {
        ArcweftBundle::new(
            BundleManifest {
                source_label: "main.arcw".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: Some("main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 0,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            BundleSource {
                label: "main.arcw".to_owned(),
                text: "flow @flow.main main { return \"ok\" }".to_owned(),
            },
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
    }
}
