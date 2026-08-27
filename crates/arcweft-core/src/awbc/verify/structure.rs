#![allow(
    clippy::too_many_lines,
    reason = "AWBC structural verification checks many stable table families in one pass"
)]

use super::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use crate::awbc::schema::{
    AWBC_ABI_VERSION, AwbcAgentTypeShape, AwbcAudioCommandId, AwbcAudioValueRef, AwbcBlockId,
    AwbcCodeLocation, AwbcConstant, AwbcConstantId, AwbcEffectKind, AwbcEffectPlan,
    AwbcEffectSetId, AwbcEntryKind, AwbcEntryTarget, AwbcFrameSlotRole, AwbcFunctionId,
    AwbcFunctionKind, AwbcLineTaskNode, AwbcPattern, AwbcPatternId, AwbcProgram, AwbcRoute,
    AwbcRouteBindingSource, AwbcRouteSegment, AwbcRuntimeType, AwbcRuntimeTypeShape,
    AwbcSignatureId, AwbcStringId, AwbcTableRange, AwbcTraitMethod, AwbcTraitReceiverMode,
    AwbcTypeId, AwbcVariantIdentity,
};
use crate::effect::RuntimeAssertionGuardId;
use crate::entry::{RuntimeCallableRole, RuntimeEntryRoles};
use crate::pattern::RuntimeOpaqueTypeAdmission;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Verifier<'program, 'context> {
    pub(super) program: &'program AwbcProgram,
    pub(super) budget: AwbcVerifyBudget,
    pub(super) context: AwbcVerifyContext<'context>,
    pub(super) block_owner: Vec<usize>,
}

pub(super) fn verify_program(
    program: &AwbcProgram,
    budget: AwbcVerifyBudget,
    context: AwbcVerifyContext<'_>,
) -> Result<(), AwbcVerifyError> {
    verify_header(program, context)?;
    verify_strings(program)?;
    if context.require_entrypoint && program.entries.is_empty() {
        return Err(AwbcVerifyError::MissingEntrypoint);
    }
    verify_runtime_types(program)?;
    verify_constants(program)?;
    verify_effect_sets(program, context)?;
    verify_signatures(program, budget)?;
    verify_frame_layouts(program, budget)?;
    let block_owner = verify_function_blocks(program)?;
    verify_block_instructions(program)?;
    let verifier = Verifier {
        program,
        budget,
        context,
        block_owner,
    };
    verify_resume_points(&verifier)?;
    verify_patterns(&verifier)?;
    verify_runtime_tables(&verifier)?;
    super::code::verify_code(&verifier)?;
    verify_entry_runtime_contracts(&verifier)?;
    verify_entries(&verifier)?;
    verify_maps_and_resources(&verifier)?;
    Ok(())
}

fn verify_header(
    program: &AwbcProgram,
    context: AwbcVerifyContext<'_>,
) -> Result<(), AwbcVerifyError> {
    if program.header.abi_version != AWBC_ABI_VERSION {
        return Err(AwbcVerifyError::UnsupportedAbi {
            actual: program.header.abi_version,
            expected: AWBC_ABI_VERSION,
        });
    }
    if program.header.minimum_runtime_abi > context.runtime_abi_version {
        return Err(AwbcVerifyError::RuntimeAbiTooOld {
            required: program.header.minimum_runtime_abi,
            actual: context.runtime_abi_version,
        });
    }
    let unsupported = program.header.feature_bits & !context.supported_feature_bits;
    if unsupported != 0 {
        return Err(AwbcVerifyError::UnsupportedFeatureBits { unsupported });
    }
    if context
        .expected_host_abi_digest
        .is_some_and(|expected| expected != program.header.host_abi_digest)
    {
        return Err(AwbcVerifyError::HostAbiDigestMismatch);
    }
    Ok(())
}

fn verify_strings(program: &AwbcProgram) -> Result<(), AwbcVerifyError> {
    for (index, pair) in program.strings.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(AwbcVerifyError::NonCanonicalStringTable { index: index + 1 });
        }
    }
    Ok(())
}

fn verify_runtime_types(program: &AwbcProgram) -> Result<(), AwbcVerifyError> {
    let mut nominal_records = BTreeMap::new();
    let mut semantic_identities = BTreeSet::new();
    for (index, ty) in program.runtime_types.iter().enumerate() {
        let at = format!("runtime type {index}");
        if !semantic_identities.insert(ty.semantic_identity()) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "runtime semantic type identity is duplicated".to_owned(),
            });
        }
        match ty.shape() {
            AwbcRuntimeTypeShape::Tuple(items) => {
                for item in items {
                    check_index(program.runtime_types.len(), item.0, "runtime_types", &at)?;
                }
            }
            AwbcRuntimeTypeShape::Sequence(item)
            | AwbcRuntimeTypeShape::Range(item)
            | AwbcRuntimeTypeShape::Iterator(item)
            | AwbcRuntimeTypeShape::Need(item)
            | AwbcRuntimeTypeShape::Task(item)
            | AwbcRuntimeTypeShape::Shared(item)
            | AwbcRuntimeTypeShape::Reference(item)
            | AwbcRuntimeTypeShape::Array { item, .. }
            | AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(item)) => {
                check_index(program.runtime_types.len(), item.0, "runtime_types", &at)?;
            }
            AwbcRuntimeTypeShape::Map { key, value }
            | AwbcRuntimeTypeShape::Stream {
                item: key,
                error: value,
            } => {
                check_index(program.runtime_types.len(), key.0, "runtime_types", &at)?;
                check_index(program.runtime_types.len(), value.0, "runtime_types", &at)?;
            }
            AwbcRuntimeTypeShape::Function { parameters, result } => {
                for parameter in parameters {
                    check_index(
                        program.runtime_types.len(),
                        parameter.0,
                        "runtime_types",
                        &at,
                    )?;
                }
                check_index(program.runtime_types.len(), result.0, "runtime_types", &at)?;
            }
            AwbcRuntimeTypeShape::Record { public_id, fields } => {
                check_optional_string(program, *public_id, &at)?;
                let mut names = BTreeSet::new();
                for field in fields {
                    check_string(program, field.name, &at)?;
                    check_index(
                        program.runtime_types.len(),
                        field.ty.0,
                        "runtime_types",
                        &at,
                    )?;
                    if !names.insert(field.name) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "record type contains duplicate field names".to_owned(),
                        });
                    }
                }
            }
            AwbcRuntimeTypeShape::Variant {
                owner,
                arguments,
                cases,
            } => {
                if let AwbcVariantIdentity::Nominal { public_id, .. } = owner {
                    check_string(program, *public_id, &at)?;
                }
                for argument in arguments {
                    check_index(
                        program.runtime_types.len(),
                        argument.0,
                        "runtime_types",
                        &at,
                    )?;
                }
                let mut names = BTreeSet::new();
                for case in cases {
                    check_string(program, case.name, &at)?;
                    if let Some(payload) = case.payload {
                        check_index(program.runtime_types.len(), payload.0, "runtime_types", &at)?;
                    }
                    if !names.insert(case.name) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "variant type contains duplicate case names".to_owned(),
                        });
                    }
                }
                verify_builtin_variant_schema(program, owner, arguments, cases, &at)?;
            }
            AwbcRuntimeTypeShape::Choice(alternatives) => {
                for alternative in alternatives {
                    check_index(
                        program.runtime_types.len(),
                        alternative.0,
                        "runtime_types",
                        &at,
                    )?;
                }
            }
            AwbcRuntimeTypeShape::Nominal {
                public_id,
                arguments,
                ..
            } => {
                check_string(program, *public_id, &at)?;
                for argument in arguments {
                    check_index(
                        program.runtime_types.len(),
                        argument.0,
                        "runtime_types",
                        &at,
                    )?;
                }
            }
            AwbcRuntimeTypeShape::NominalRecord {
                public_id,
                layout,
                arguments,
                fields,
            } => {
                check_string(program, *public_id, &at)?;
                for argument in arguments {
                    check_index(
                        program.runtime_types.len(),
                        argument.0,
                        "runtime_types",
                        &at,
                    )?;
                }
                let key = (*public_id, ty.semantic_identity(), *layout);
                if nominal_records.insert(key, index).is_some() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "nominal record identity has more than one executable descriptor"
                            .to_owned(),
                    });
                }
                let mut names = BTreeSet::new();
                for field in fields {
                    check_string(program, field.name, &at)?;
                    check_index(
                        program.runtime_types.len(),
                        field.ty.0,
                        "runtime_types",
                        &at,
                    )?;
                    if !names.insert(field.name) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "nominal record type contains duplicate field names"
                                .to_owned(),
                        });
                    }
                }
                program
                    .nominal_record_layout(AwbcTypeId(u32::try_from(index).map_err(|_| {
                        AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "runtime type index exceeds u32".to_owned(),
                        }
                    })?))
                    .map_err(|error| AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: error.to_string(),
                    })?;
            }
            AwbcRuntimeTypeShape::Opaque {
                producer,
                arguments,
                ..
            } => {
                check_string(program, *producer, &at)?;
                for argument in arguments {
                    check_index(
                        program.runtime_types.len(),
                        argument.0,
                        "runtime_types",
                        &at,
                    )?;
                }
                ty.try_opaque_owner(&program.strings).map_err(|error| {
                    AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: error.to_string(),
                    }
                })?;
            }
            AwbcRuntimeTypeShape::Unit
            | AwbcRuntimeTypeShape::Bool
            | AwbcRuntimeTypeShape::Int(_)
            | AwbcRuntimeTypeShape::UInt(_)
            | AwbcRuntimeTypeShape::Bytes
            | AwbcRuntimeTypeShape::Never
            | AwbcRuntimeTypeShape::F32
            | AwbcRuntimeTypeShape::F64
            | AwbcRuntimeTypeShape::String
            | AwbcRuntimeTypeShape::Char
            | AwbcRuntimeTypeShape::Duration
            | AwbcRuntimeTypeShape::Progress
            | AwbcRuntimeTypeShape::EntityRef
            | AwbcRuntimeTypeShape::AgentValue
            | AwbcRuntimeTypeShape::MatrixF32
            | AwbcRuntimeTypeShape::MatrixF64
            | AwbcRuntimeTypeShape::TensorF32
            | AwbcRuntimeTypeShape::TensorF64
            | AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(_))
            | AwbcRuntimeTypeShape::Dynamic => {}
        }
    }
    Ok(())
}

fn verify_constants(program: &AwbcProgram) -> Result<(), AwbcVerifyError> {
    for (index, constant) in program.constants.iter().enumerate() {
        let at = format!("constant {index}");
        match constant {
            AwbcConstant::String(id) => check_string(program, *id, &at)?,
            AwbcConstant::Char(value) => {
                if char::from_u32(*value).is_none() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "char constant is not a Unicode scalar value".to_owned(),
                    });
                }
            }
            AwbcConstant::Tuple(items) | AwbcConstant::Sequence(items) => {
                for item in items {
                    check_index(program.constants.len(), item.0, "constants", &at)?;
                }
            }
            AwbcConstant::Record {
                ty,
                field_names,
                fields,
            } => {
                check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                if field_names.len() != fields.len() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: format!("constant {index}"),
                        message: "record constant field name count does not match value count"
                            .to_owned(),
                    });
                }
                for field_name in field_names {
                    check_string(program, *field_name, &at)?;
                }
                for field in fields {
                    check_index(program.constants.len(), field.0, "constants", &at)?;
                }
                match program.runtime_types[ty.index()].shape() {
                    AwbcRuntimeTypeShape::Record {
                        fields: type_fields,
                        ..
                    }
                    | AwbcRuntimeTypeShape::NominalRecord {
                        fields: type_fields,
                        ..
                    } => {
                        if type_fields.len() != fields.len() {
                            return Err(AwbcVerifyError::InvalidInvariant {
                                at: format!("constant {index}"),
                                message: "record constant field count does not match type"
                                    .to_owned(),
                            });
                        }
                        for (actual, expected) in field_names.iter().zip(type_fields) {
                            if *actual != expected.name {
                                return Err(AwbcVerifyError::InvalidInvariant {
                                    at: format!("constant {index}"),
                                    message: "record constant field names do not match type"
                                        .to_owned(),
                                });
                            }
                        }
                    }
                    AwbcRuntimeTypeShape::Dynamic => {}
                    _ => {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at,
                            message: "record constant references a non-record type".to_owned(),
                        });
                    }
                }
            }
            AwbcConstant::Variant {
                ty,
                case,
                case_name,
                payload,
            } => {
                check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                check_string(program, *case_name, &at)?;
                if let Some(payload) = payload {
                    check_index(program.constants.len(), payload.0, "constants", &at)?;
                }
                match program.runtime_types[ty.index()].shape() {
                    AwbcRuntimeTypeShape::Variant { cases, .. } => {
                        let Some(case_layout) = cases.get(*case as usize) else {
                            return Err(AwbcVerifyError::IndexOutOfBounds {
                                table: "variant cases",
                                index: *case,
                                at: format!("constant {index}"),
                            });
                        };
                        if case_layout.name != *case_name {
                            return Err(AwbcVerifyError::InvalidInvariant {
                                at: format!("constant {index}"),
                                message: "variant constant case name does not match type"
                                    .to_owned(),
                            });
                        }
                        if case_layout.payload.is_some() != payload.is_some() {
                            return Err(AwbcVerifyError::InvalidInvariant {
                                at: format!("constant {index}"),
                                message: "variant constant payload shape does not match type"
                                    .to_owned(),
                            });
                        }
                    }
                    _ => {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at,
                            message: "variant constant references a non-variant type".to_owned(),
                        });
                    }
                }
            }
            AwbcConstant::Opaque { ty, payload } => {
                check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                check_index(program.constants.len(), payload.0, "constants", &at)?;
                if payload.index() >= index {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "opaque constant payload must precede its owner row".to_owned(),
                    });
                }
                let owner = program.opaque_owner(*ty).map_err(|error| {
                    AwbcVerifyError::InvalidInvariant {
                        at: format!("constant {index}"),
                        message: error.to_string(),
                    }
                })?;
                if !owner.is_some_and(|owner| {
                    owner.admission() == RuntimeOpaqueTypeAdmission::ExactIdentity
                        && owner.value_class() == crate::value::RuntimeOpaqueValueClass::Plain
                        && owner.persistence()
                            == crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot
                }) {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: format!("constant {index}"),
                        message:
                            "opaque constant requires an exact constant-admissible opaque type row"
                                .to_owned(),
                    });
                }
            }
            AwbcConstant::Range { start, end, .. } => {
                if let Some(start) = start {
                    check_index(program.constants.len(), start.0, "constants", &at)?;
                }
                if let Some(end) = end {
                    check_index(program.constants.len(), end.0, "constants", &at)?;
                }
            }
            AwbcConstant::TensorF32 { shape, values } => {
                verify_tensor_shape(shape, values.len(), &at)?;
            }
            AwbcConstant::TensorF64 { shape, values } => {
                verify_tensor_shape(shape, values.len(), &at)?;
            }
            AwbcConstant::Unit
            | AwbcConstant::Bool(_)
            | AwbcConstant::Int { .. }
            | AwbcConstant::UInt { .. }
            | AwbcConstant::F32Bits(_)
            | AwbcConstant::F64Bits(_)
            | AwbcConstant::DurationNanos(_)
            | AwbcConstant::EntityRef(_)
            | AwbcConstant::Bytes(_) => {}
        }
    }
    verify_constant_graph(program)?;
    Ok(())
}

fn verify_constant_graph(program: &AwbcProgram) -> Result<(), AwbcVerifyError> {
    let mut complete = BTreeSet::new();
    for index in 0..program.constants.len() {
        verify_constant_graph_from(
            program,
            AwbcConstantId(u32::try_from(index).map_err(|_| {
                AwbcVerifyError::InvalidInvariant {
                    at: "constants".to_owned(),
                    message: "constant table exceeds the u32 index space".to_owned(),
                }
            })?),
            0,
            &mut BTreeSet::new(),
            &mut complete,
        )?;
    }
    Ok(())
}

fn verify_constant_graph_from(
    program: &AwbcProgram,
    id: crate::awbc::schema::AwbcConstantId,
    depth: usize,
    visiting: &mut BTreeSet<crate::awbc::schema::AwbcConstantId>,
    complete: &mut BTreeSet<crate::awbc::schema::AwbcConstantId>,
) -> Result<(), AwbcVerifyError> {
    if complete.contains(&id) {
        return Ok(());
    }
    if depth > 64 {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: format!("constant {}", id.0),
            message: "constant graph exceeds depth 64".to_owned(),
        });
    }
    if !visiting.insert(id) {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: format!("constant {}", id.0),
            message: "constant graph contains a cycle".to_owned(),
        });
    }
    let constant = &program.constants[id.index()];
    let mut visit =
        |child| verify_constant_graph_from(program, child, depth + 1, visiting, complete);
    match constant {
        AwbcConstant::Tuple(items) | AwbcConstant::Sequence(items) => {
            for child in items {
                visit(*child)?;
            }
        }
        AwbcConstant::Record { fields, .. } => {
            for child in fields {
                visit(*child)?;
            }
        }
        AwbcConstant::Variant { payload, .. } => {
            if let Some(child) = payload {
                visit(*child)?;
            }
        }
        AwbcConstant::Range { start, end, .. } => {
            if let Some(child) = start {
                visit(*child)?;
            }
            if let Some(child) = end {
                visit(*child)?;
            }
        }
        AwbcConstant::Opaque { payload, .. } => visit(*payload)?,
        AwbcConstant::Unit
        | AwbcConstant::Bool(_)
        | AwbcConstant::Int { .. }
        | AwbcConstant::UInt { .. }
        | AwbcConstant::F32Bits(_)
        | AwbcConstant::F64Bits(_)
        | AwbcConstant::String(_)
        | AwbcConstant::Char(_)
        | AwbcConstant::DurationNanos(_)
        | AwbcConstant::EntityRef(_)
        | AwbcConstant::Bytes(_)
        | AwbcConstant::TensorF32 { .. }
        | AwbcConstant::TensorF64 { .. } => {}
    }
    visiting.remove(&id);
    complete.insert(id);
    Ok(())
}

fn verify_builtin_variant_schema(
    program: &AwbcProgram,
    owner: &AwbcVariantIdentity,
    arguments: &[AwbcTypeId],
    cases: &[crate::awbc::schema::AwbcVariantCase],
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let valid = match owner {
        AwbcVariantIdentity::Nominal { .. } => true,
        AwbcVariantIdentity::Builtin(owner) => {
            arguments.is_empty()
                && cases.len() == owner.cases().len()
                && cases.iter().zip(owner.cases()).all(|(case, expected)| {
                    program.strings.get(case.name.index()).map(String::as_str)
                        == Some(expected.name())
                        && case.payload.is_some() == expected.has_payload()
                })
        }
    };
    if !valid {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "builtin variant owner has a non-canonical case schema".to_owned(),
        });
    }
    Ok(())
}

fn verify_tensor_shape(shape: &[u32], value_len: usize, at: &str) -> Result<(), AwbcVerifyError> {
    let elements = shape.iter().try_fold(1_usize, |total, dimension| {
        total.checked_mul(*dimension as usize)
    });
    if elements != Some(value_len) {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "tensor shape does not match element count".to_owned(),
        });
    }
    Ok(())
}

fn verify_effect_sets(
    program: &AwbcProgram,
    context: AwbcVerifyContext<'_>,
) -> Result<(), AwbcVerifyError> {
    for (index, effect_set) in program.effect_sets.iter().enumerate() {
        for effect in &effect_set.effects {
            check_string(program, *effect, &format!("effect set {index}"))?;
            if let Some(allowed) = context.allowed_effects {
                let name = &program.strings[effect.index()];
                if !allowed.contains(name) {
                    return Err(AwbcVerifyError::EffectDenied {
                        effect: name.clone(),
                    });
                }
            }
        }
        if effect_set.effects.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(AwbcVerifyError::NonCanonicalEffectSet { effect_set: index });
        }
    }
    Ok(())
}

fn verify_signatures(
    program: &AwbcProgram,
    budget: AwbcVerifyBudget,
) -> Result<(), AwbcVerifyError> {
    for (index, signature) in program.signatures.iter().enumerate() {
        if signature.params.len() > budget.params_per_signature {
            return Err(AwbcVerifyError::SignatureBudgetExceeded {
                signature: index,
                budget: "params_per_signature",
            });
        }
        for ty in &signature.params {
            check_index(
                program.runtime_types.len(),
                ty.0,
                "runtime_types",
                &format!("signature {index}"),
            )?;
        }
        if let Some(result) = signature.result {
            check_index(
                program.runtime_types.len(),
                result.0,
                "runtime_types",
                &format!("signature {index}"),
            )?;
        }
        check_index(
            program.effect_sets.len(),
            signature.effects.0,
            "effect_sets",
            &format!("signature {index}"),
        )?;
    }
    Ok(())
}

fn verify_frame_layouts(
    program: &AwbcProgram,
    budget: AwbcVerifyBudget,
) -> Result<(), AwbcVerifyError> {
    for (index, layout) in program.frame_layouts.iter().enumerate() {
        if layout.slots.len() > budget.frame_slots_per_function {
            return Err(AwbcVerifyError::FrameBudgetExceeded {
                layout: index,
                budget: "frame_slots_per_function",
            });
        }
        for slot in &layout.slots {
            check_optional_string(program, slot.name, &format!("frame layout {index}"))?;
            check_index(
                program.runtime_types.len(),
                slot.ty.0,
                "runtime_types",
                &format!("frame layout {index}"),
            )?;
            if slot.scope_depth > layout.max_scope_depth {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: format!("frame layout {index}"),
                    message: "slot scope depth exceeds frame maximum".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn verify_function_blocks(program: &AwbcProgram) -> Result<Vec<usize>, AwbcVerifyError> {
    let mut owners = vec![None; program.blocks.len()];
    for (function_index, function) in program.functions.iter().enumerate() {
        let at = format!("function {function_index}");
        function
            .flags
            .validate_for_kind(function.kind)
            .map_err(|source| AwbcVerifyError::InvalidFunctionRoles {
                function: function_index,
                source,
            })?;
        check_optional_string(program, function.public_id, &at)?;
        check_index(
            program.signatures.len(),
            function.signature.0,
            "signatures",
            &at,
        )?;
        check_index(
            program.frame_layouts.len(),
            function.frame_layout.0,
            "frame_layouts",
            &at,
        )?;
        let range = checked_range(function.blocks, program.blocks.len(), "blocks", &at)?;
        if !range.contains(&function.entry_block.index()) {
            return Err(AwbcVerifyError::EntryBlockOutsideFunction {
                function: function_index,
                block: function.entry_block.0,
            });
        }
        for block_index in range {
            if owners[block_index].replace(function_index).is_some() {
                return Err(AwbcVerifyError::InvalidTableOwnership {
                    table: "blocks",
                    index: block_index,
                });
            }
            let actual = program.blocks[block_index].owner.0;
            if actual as usize != function_index {
                return Err(AwbcVerifyError::BlockOwnerMismatch {
                    block: block_index,
                    actual,
                    expected: function_index,
                });
            }
        }
        verify_parameter_layout(program, function_index)?;
    }
    owners
        .into_iter()
        .enumerate()
        .map(|(index, owner)| {
            owner.ok_or(AwbcVerifyError::InvalidTableOwnership {
                table: "blocks",
                index,
            })
        })
        .collect()
}

fn verify_parameter_layout(
    program: &AwbcProgram,
    function_index: usize,
) -> Result<(), AwbcVerifyError> {
    let function = &program.functions[function_index];
    let signature = &program.signatures[function.signature.index()];
    let layout = &program.frame_layouts[function.frame_layout.index()];
    let parameter_slots = layout
        .slots
        .iter()
        .take_while(|slot| slot.role == AwbcFrameSlotRole::Parameter)
        .collect::<Vec<_>>();
    if parameter_slots.len() != signature.params.len()
        || parameter_slots
            .iter()
            .zip(&signature.params)
            .any(|(slot, ty)| slot.ty != *ty)
        || layout
            .slots
            .iter()
            .skip(parameter_slots.len())
            .any(|slot| slot.role == AwbcFrameSlotRole::Parameter)
    {
        return Err(AwbcVerifyError::ParameterLayoutMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn verify_block_instructions(program: &AwbcProgram) -> Result<Vec<usize>, AwbcVerifyError> {
    let mut owners = vec![None; program.instructions.len()];
    for (block_index, block) in program.blocks.iter().enumerate() {
        check_optional_index(
            program.source_map.len(),
            block.source_map.map(|id| id.0),
            "source_map",
            &format!("block {block_index}"),
        )?;
        let range = checked_range(
            block.instructions,
            program.instructions.len(),
            "instructions",
            &format!("block {block_index}"),
        )?;
        for instruction_index in range {
            if owners[instruction_index].replace(block_index).is_some() {
                return Err(AwbcVerifyError::InvalidTableOwnership {
                    table: "instructions",
                    index: instruction_index,
                });
            }
        }
    }
    owners
        .into_iter()
        .enumerate()
        .map(|(index, owner)| {
            owner.ok_or(AwbcVerifyError::InvalidTableOwnership {
                table: "instructions",
                index,
            })
        })
        .collect()
}

fn verify_resume_points(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, resume) in program.resume_points.iter().enumerate() {
        let at = format!("resume point {index}");
        check_index(program.functions.len(), resume.function.0, "functions", &at)?;
        check_index(program.blocks.len(), resume.block.0, "blocks", &at)?;
        check_index(
            program.frame_layouts.len(),
            resume.frame_layout.0,
            "frame_layouts",
            &at,
        )?;
        let function = &program.functions[resume.function.index()];
        if verifier.block_owner[resume.block.index()] != resume.function.index()
            || function.frame_layout != resume.frame_layout
        {
            return Err(AwbcVerifyError::ResumePointMismatch {
                resume: u32::try_from(index).expect("AWBC resume indices originate from u32 ids"),
                at,
            });
        }
    }
    Ok(())
}

fn verify_patterns(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, pattern) in program.patterns.iter().enumerate() {
        let at = format!("pattern {index}");
        match pattern {
            AwbcPattern::Bind { expected, .. } => {
                if let Some(ty) = expected {
                    check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                }
            }
            AwbcPattern::Literal(constant) => {
                check_index(program.constants.len(), constant.0, "constants", &at)?;
            }
            AwbcPattern::Entity(_) => {}
            AwbcPattern::Tuple(items) | AwbcPattern::Sequence { items, .. } => {
                for child in items {
                    check_index(program.patterns.len(), child.0, "patterns", &at)?;
                }
            }
            AwbcPattern::Record { ty, fields, .. } => {
                if let Some(ty) = ty {
                    check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                }
                let mut ordinals = BTreeSet::new();
                for field in fields {
                    check_index(program.patterns.len(), field.pattern.0, "patterns", &at)?;
                    if !ordinals.insert(field.field) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "record pattern contains duplicate field ordinals".to_owned(),
                        });
                    }
                }
            }
            AwbcPattern::Variant {
                ty,
                case_name,
                payload,
                ..
            } => {
                check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                check_string(program, *case_name, &at)?;
                if let Some(payload) = payload {
                    check_index(program.patterns.len(), payload.0, "patterns", &at)?;
                }
            }
            AwbcPattern::Whole { inner, .. } => {
                check_index(program.patterns.len(), inner.0, "patterns", &at)?;
            }
            AwbcPattern::Discard => {}
        }
    }
    let mut state = vec![0_u8; program.patterns.len()];
    for index in 0..program.patterns.len() {
        visit_pattern(verifier, index, 0, &mut state)?;
    }
    Ok(())
}

fn visit_pattern(
    verifier: &Verifier<'_, '_>,
    index: usize,
    depth: usize,
    state: &mut [u8],
) -> Result<(), AwbcVerifyError> {
    if depth > verifier.budget.pattern_depth {
        return Err(AwbcVerifyError::PatternDepthExceeded {
            pattern: index,
            limit: verifier.budget.pattern_depth,
        });
    }
    match state[index] {
        1 => return Err(AwbcVerifyError::PatternCycle { pattern: index }),
        2 => return Ok(()),
        _ => {}
    }
    state[index] = 1;
    for child in pattern_children(&verifier.program.patterns[index]) {
        visit_pattern(verifier, child.index(), depth + 1, state)?;
    }
    state[index] = 2;
    Ok(())
}

fn pattern_children(pattern: &AwbcPattern) -> Vec<AwbcPatternId> {
    match pattern {
        AwbcPattern::Tuple(items) | AwbcPattern::Sequence { items, .. } => items.clone(),
        AwbcPattern::Record { fields, .. } => fields.iter().map(|field| field.pattern).collect(),
        AwbcPattern::Variant {
            payload: Some(payload),
            ..
        } => vec![*payload],
        AwbcPattern::Whole { inner, .. } => vec![*inner],
        AwbcPattern::Bind { .. }
        | AwbcPattern::Discard
        | AwbcPattern::Literal(_)
        | AwbcPattern::Entity(_)
        | AwbcPattern::Variant { payload: None, .. } => Vec::new(),
    }
}

fn verify_runtime_tables(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, intrinsic) in program.intrinsics.iter().enumerate() {
        let at = format!("intrinsic {index}");
        check_index(
            program.signatures.len(),
            intrinsic.signature.0,
            "signatures",
            &at,
        )?;
        if intrinsic.revision != 1 {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "intrinsic revision must remain 1".to_owned(),
            });
        }
    }
    for (index, call) in program.host_calls.iter().enumerate() {
        let at = format!("host call {index}");
        check_string(program, call.public_id, &at)?;
        check_capability(verifier, call.capability, &at)?;
        check_string(program, call.operation, &at)?;
        check_index(
            program.signatures.len(),
            call.signature.0,
            "signatures",
            &at,
        )?;
        for argument in &call.arguments {
            check_optional_string(program, argument.name, &at)?;
        }
        if call.arguments.len() != program.signatures[call.signature.index()].params.len() {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "host-call argument descriptors must match signature arity".to_owned(),
            });
        }
    }
    for (index, task) in program.task_plans.iter().enumerate() {
        let at = format!("task plan {index}");
        check_string(program, task.public_id, &at)?;
        check_string(program, task.need_id, &at)?;
        check_capability(verifier, task.capability, &at)?;
        check_string(program, task.operation, &at)?;
        check_string(program, task.cancel_scope, &at)?;
        check_index(
            program.signatures.len(),
            task.signature.0,
            "signatures",
            &at,
        )?;
        check_index(
            program.runtime_types.len(),
            task.payload_type.0,
            "runtime_types",
            &at,
        )?;
        for argument in &task.arguments {
            check_optional_string(program, argument.name, &at)?;
        }
        if task.arguments.len() != program.signatures[task.signature.index()].params.len() {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "task argument descriptors must match signature arity".to_owned(),
            });
        }
        if task.many.as_ref().is_some_and(|many| many.limit == 0) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "await-many task limit must be non-zero".to_owned(),
            });
        }
    }
    for (index, effect) in program.effect_plans.iter().enumerate() {
        let at = format!("effect plan {index}");
        check_index(
            program.signatures.len(),
            effect.signature.0,
            "signatures",
            &at,
        )?;
        if let Some(capability) = effect.capability {
            check_capability(verifier, capability, &at)?;
        }
        verify_effect_audio_payload(program, index, effect)?;
        for constant in &effect.static_args {
            check_index(program.constants.len(), constant.0, "constants", &at)?;
        }
        verify_effect_payload_shape(program, index, effect)?;
        for access in &effect.resources {
            check_index(program.resources.len(), access.resource.0, "resources", &at)?;
        }
    }
    for (index, choice) in program.choices.iter().enumerate() {
        check_optional_string(program, choice.public_id, &format!("choice {index}"))?;
        checked_range(
            choice.options,
            program.choice_options.len(),
            "choice_options",
            &format!("choice {index}"),
        )?;
    }
    for (index, option) in program.choice_options.iter().enumerate() {
        let at = format!("choice option {index}");
        check_optional_string(program, option.public_id, &at)?;
        check_string(program, option.label, &at)?;
        check_optional_index(
            program.functions.len(),
            option.condition.map(|id| id.0),
            "functions",
            &at,
        )?;
        check_optional_index(
            program.functions.len(),
            option.target.map(|id| id.0),
            "functions",
            &at,
        )?;
        if let Some(target) = option.target
            && program.flow_identity(target).is_none()
        {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "choice target has no exact semantic Flow binding".to_owned(),
            });
        }
        check_optional_index(
            program.effect_plans.len(),
            option.out_effect.map(|id| id.0),
            "effect_plans",
            &at,
        )?;
        for effect in &option.effects {
            check_index(program.effect_plans.len(), effect.0, "effect_plans", &at)?;
        }
    }
    verify_content_and_line_tables(verifier)?;
    verify_stream_tables(verifier)?;
    for (index, helper) in program.pure_helpers.iter().enumerate() {
        let at = format!("pure helper {index}");
        check_string(program, helper.public_id, &at)?;
        check_index(
            program.signatures.len(),
            helper.signature.0,
            "signatures",
            &at,
        )?;
        check_index(program.functions.len(), helper.function.0, "functions", &at)?;
        let function = &program.functions[helper.function.index()];
        if function.signature != helper.signature || function.kind != AwbcFunctionKind::PureHelper {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "pure-helper table does not match function signature/kind".to_owned(),
            });
        }
    }
    let mut pure_programs = std::collections::BTreeSet::new();
    let semantic_types = program
        .runtime_types
        .iter()
        .map(AwbcRuntimeType::semantic_identity)
        .collect::<BTreeSet<_>>();
    for (index, binding) in program.pure_programs.iter().enumerate() {
        let at = format!("pure program {index}");
        if !pure_programs.insert(binding.program) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "stable pure-program identity is duplicated".to_owned(),
            });
        }
        check_index(
            program.pure_helpers.len(),
            binding.helper.0,
            "pure_helpers",
            &at,
        )?;
        let helper = &program.pure_helpers[binding.helper.index()];
        let signature = &program.signatures[helper.signature.index()];
        if signature.params.len() != binding.input_types.len() || signature.result.is_none() {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "pure-program semantic signature does not match helper arity".to_owned(),
            });
        }
        for (parameter, semantic_identity) in
            signature.params.iter().zip(binding.input_types.iter())
        {
            if !semantic_types.contains(semantic_identity)
                || runtime_semantic_identity(program, *parameter) != Some(*semantic_identity)
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "pure-program input semantic identity does not match helper type"
                        .to_owned(),
                });
            }
        }
        if !semantic_types.contains(&binding.result_type)
            || runtime_semantic_identity(
                program,
                signature
                    .result
                    .expect("pure-program helper result checked above"),
            ) != Some(binding.result_type)
        {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "pure-program result semantic identity does not match helper type"
                    .to_owned(),
            });
        }
    }
    for (index, method) in program.trait_methods.iter().enumerate() {
        let at = format!("trait method {index}");
        check_string(program, method.public_id, &at)?;
        check_index(
            program.signatures.len(),
            method.signature.0,
            "signatures",
            &at,
        )?;
        check_index(program.functions.len(), method.function.0, "functions", &at)?;
        let function = &program.functions[method.function.index()];
        if function.signature != method.signature || function.kind != AwbcFunctionKind::TraitMethod
        {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "trait-method table does not match function signature/kind".to_owned(),
            });
        }
        verify_trait_method_receiver(program, index, method)?;
    }
    Ok(())
}

fn runtime_semantic_identity(
    program: &AwbcProgram,
    ty: AwbcTypeId,
) -> Option<crate::pattern::RuntimeSemanticTypeId> {
    program
        .runtime_types
        .get(ty.index())
        .map(AwbcRuntimeType::semantic_identity)
}

fn verify_trait_method_receiver(
    program: &AwbcProgram,
    index: usize,
    method: &AwbcTraitMethod,
) -> Result<(), AwbcVerifyError> {
    let at = format!("trait method {index}");
    let signature = &program.signatures[method.signature.index()];
    let Some(receiver_ty) = signature.params.first().copied() else {
        return Err(AwbcVerifyError::InvalidInvariant {
            at,
            message: "trait method signature must include receiver parameter".to_owned(),
        });
    };
    match method.receiver {
        AwbcTraitReceiverMode::Owned | AwbcTraitReceiverMode::SharedRef => {
            if method.receiver_state_slot.is_some() {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "non-mut trait receiver cannot declare receiver_state_slot".to_owned(),
                });
            }
        }
        AwbcTraitReceiverMode::MutRef => {
            let Some(slot) = method.receiver_state_slot else {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "mut trait receiver must declare receiver_state_slot".to_owned(),
                });
            };
            let function = &program.functions[method.function.index()];
            let layout = &program.frame_layouts[function.frame_layout.index()];
            check_index(layout.slots.len(), slot.0, "frame_slots", &at)?;
            let slot = &layout.slots[slot.index()];
            if slot.role != AwbcFrameSlotRole::Parameter || slot.ty != receiver_ty {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "mut receiver state slot must be a receiver parameter slot".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn verify_effect_audio_payload(
    program: &AwbcProgram,
    effect_index: usize,
    effect: &AwbcEffectPlan,
) -> Result<(), AwbcVerifyError> {
    match (effect.kind, effect.audio) {
        (AwbcEffectKind::Audio, Some(command)) => {
            if !effect.static_args.is_empty() {
                return Err(AwbcVerifyError::MalformedAudioPayload {
                    effect: effect_index,
                    message: "audio effect must use typed payload rows, not legacy static args"
                        .to_owned(),
                });
            }
            verify_audio_command_refs(program, effect_index, effect.signature, command)
        }
        (AwbcEffectKind::Audio, None) => Err(AwbcVerifyError::MalformedAudioPayload {
            effect: effect_index,
            message: "audio effect is missing typed payload row".to_owned(),
        }),
        (_, Some(_)) => Err(AwbcVerifyError::MalformedAudioPayload {
            effect: effect_index,
            message: "non-audio effect carries an audio payload row".to_owned(),
        }),
        (_, None) => Ok(()),
    }
}

fn verify_effect_payload_shape(
    program: &AwbcProgram,
    effect_index: usize,
    effect: &AwbcEffectPlan,
) -> Result<(), AwbcVerifyError> {
    let static_count = effect.static_args.len();
    let parameter_count = program.signatures[effect.signature.index()].params.len();
    match effect.kind {
        AwbcEffectKind::Audio => Ok(()),
        AwbcEffectKind::Call => {
            require_effect_static_minimum(effect_index, effect.kind, static_count, 1)?;
            require_effect_parameter_count(effect_index, effect.kind, parameter_count, &[0])
        }
        AwbcEffectKind::Log => {
            require_effect_static_minimum(effect_index, effect.kind, static_count, 2)?;
            if !(static_count - 2).is_multiple_of(2) {
                return Err(malformed_effect_payload(
                    effect_index,
                    effect.kind,
                    "log fields must use name/value static-argument pairs",
                ));
            }
            let evaluated_count = 1 + (static_count - 2) / 2;
            require_effect_parameter_count(
                effect_index,
                effect.kind,
                parameter_count,
                &[0, evaluated_count],
            )
        }
        AwbcEffectKind::SignalWrite | AwbcEffectKind::MetricWrite | AwbcEffectKind::Ensure => {
            require_effect_static_count(effect_index, effect.kind, static_count, 2)?;
            require_effect_parameter_count(effect_index, effect.kind, parameter_count, &[0, 2])
        }
        AwbcEffectKind::EmitEvent => {
            require_effect_static_minimum(effect_index, effect.kind, static_count, 1)?;
            if !(static_count - 1).is_multiple_of(2) {
                return Err(malformed_effect_payload(
                    effect_index,
                    effect.kind,
                    "event fields must use name/value static-argument pairs",
                ));
            }
            let evaluated_count = 1 + (static_count - 1) / 2;
            require_effect_parameter_count(
                effect_index,
                effect.kind,
                parameter_count,
                &[0, evaluated_count],
            )
        }
        AwbcEffectKind::Panic | AwbcEffectKind::Fail | AwbcEffectKind::Bail => {
            require_effect_static_count(effect_index, effect.kind, static_count, 1)?;
            require_effect_parameter_count(effect_index, effect.kind, parameter_count, &[0, 1])
        }
        AwbcEffectKind::Assert => {
            require_effect_static_count(effect_index, effect.kind, static_count, 4)?;
            require_effect_parameter_count(effect_index, effect.kind, parameter_count, &[0, 1])?;
            let guard = effect_static_bytes(program, effect, 0).ok_or_else(|| {
                malformed_effect_payload(
                    effect_index,
                    effect.kind,
                    "assert guard must be a byte-array constant",
                )
            })?;
            let guard: [u8; 16] = guard.try_into().map_err(|_| {
                malformed_effect_payload(
                    effect_index,
                    effect.kind,
                    "assert guard must contain exactly 16 bytes",
                )
            })?;
            RuntimeAssertionGuardId::try_from_bytes(guard).map_err(|error| {
                malformed_effect_payload(
                    effect_index,
                    effect.kind,
                    format!("invalid assert guard: {error}"),
                )
            })?;
            let profile = effect_static_string(program, effect, 3).ok_or_else(|| {
                malformed_effect_payload(
                    effect_index,
                    effect.kind,
                    "assert profile must be a string constant",
                )
            })?;
            if !matches!(profile, "always" | "debug_only") {
                return Err(malformed_effect_payload(
                    effect_index,
                    effect.kind,
                    format!("unknown assert profile `{profile}`"),
                ));
            }
            Ok(())
        }
        AwbcEffectKind::Out | AwbcEffectKind::Break => {
            require_static_only_effect(effect_index, effect.kind, static_count, parameter_count, 2)
        }
        AwbcEffectKind::Wait
        | AwbcEffectKind::Return
        | AwbcEffectKind::Goto
        | AwbcEffectKind::Close
        | AwbcEffectKind::Select
        | AwbcEffectKind::Continue => {
            require_static_only_effect(effect_index, effect.kind, static_count, parameter_count, 1)
        }
    }
}

fn require_static_only_effect(
    effect_index: usize,
    kind: AwbcEffectKind,
    static_count: usize,
    parameter_count: usize,
    expected_static_count: usize,
) -> Result<(), AwbcVerifyError> {
    require_effect_static_count(effect_index, kind, static_count, expected_static_count)?;
    require_effect_parameter_count(effect_index, kind, parameter_count, &[0])
}

fn require_effect_static_count(
    effect_index: usize,
    kind: AwbcEffectKind,
    actual: usize,
    expected: usize,
) -> Result<(), AwbcVerifyError> {
    if actual == expected {
        return Ok(());
    }
    Err(malformed_effect_payload(
        effect_index,
        kind,
        format!("expected {expected} static arguments, found {actual}"),
    ))
}

fn require_effect_static_minimum(
    effect_index: usize,
    kind: AwbcEffectKind,
    actual: usize,
    minimum: usize,
) -> Result<(), AwbcVerifyError> {
    if actual >= minimum {
        return Ok(());
    }
    Err(malformed_effect_payload(
        effect_index,
        kind,
        format!("expected at least {minimum} static arguments, found {actual}"),
    ))
}

fn require_effect_parameter_count(
    effect_index: usize,
    kind: AwbcEffectKind,
    actual: usize,
    allowed: &[usize],
) -> Result<(), AwbcVerifyError> {
    if allowed.contains(&actual) {
        return Ok(());
    }
    Err(malformed_effect_payload(
        effect_index,
        kind,
        format!("expected evaluated argument count in {allowed:?}, found {actual}"),
    ))
}

fn malformed_effect_payload(
    effect: usize,
    kind: AwbcEffectKind,
    message: impl Into<String>,
) -> AwbcVerifyError {
    AwbcVerifyError::MalformedEffectPayload {
        effect,
        message: format!("{kind:?}: {}", message.into()),
    }
}

fn effect_static_string<'a>(
    program: &'a AwbcProgram,
    effect: &AwbcEffectPlan,
    index: usize,
) -> Option<&'a str> {
    let constant = program
        .constants
        .get(effect.static_args.get(index)?.index())?;
    let AwbcConstant::String(string) = constant else {
        return None;
    };
    program.strings.get(string.index()).map(String::as_str)
}

fn effect_static_bytes<'a>(
    program: &'a AwbcProgram,
    effect: &AwbcEffectPlan,
    index: usize,
) -> Option<&'a [u8]> {
    let constant = program
        .constants
        .get(effect.static_args.get(index)?.index())?;
    let AwbcConstant::Bytes(bytes) = constant else {
        return None;
    };
    Some(bytes)
}

fn verify_audio_command_refs(
    program: &AwbcProgram,
    effect_index: usize,
    signature: AwbcSignatureId,
    command: AwbcAudioCommandId,
) -> Result<(), AwbcVerifyError> {
    check_index(
        program.audio_commands.len(),
        command.0,
        "audio_commands",
        &format!("effect plan {effect_index}"),
    )?;
    let arg_count = program.signatures[signature.index()].params.len();
    for value in program.audio_commands[command.index()].value_refs() {
        match value {
            AwbcAudioValueRef::Arg(arg) if arg.index() >= arg_count => {
                return Err(AwbcVerifyError::MalformedAudioPayload {
                    effect: effect_index,
                    message: format!(
                        "audio arg {} exceeds effect signature arity {arg_count}",
                        arg.0
                    ),
                });
            }
            AwbcAudioValueRef::Arg(_) => {}
            AwbcAudioValueRef::Const(constant) => check_index(
                program.constants.len(),
                constant.0,
                "constants",
                &format!("effect plan {effect_index} audio payload"),
            )?,
        }
    }
    Ok(())
}

fn verify_content_and_line_tables(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, content) in program.content_units.iter().enumerate() {
        let at = format!("content unit {index}");
        check_string(program, content.public_id, &at)?;
        let mut marks = BTreeSet::new();
        for mark in &content.marks {
            check_string(program, mark.label, &at)?;
            if !marks.insert(mark.id) {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "content unit repeats a dialogue mark identity".to_owned(),
                });
            }
        }
        check_optional_index(
            program.line_task_groups.len(),
            content.line_task_group.map(|id| id.0),
            "line_task_groups",
            &at,
        )?;
        check_optional_index(
            program.display_map.len(),
            content.display.map(|id| id.0),
            "display_map",
            &at,
        )?;
        check_optional_index(
            program.source_map.len(),
            content.source.map(|id| id.0),
            "source_map",
            &at,
        )?;
        for resource in &content.resources {
            check_index(program.resources.len(), resource.0, "resources", &at)?;
        }
    }
    for (index, group) in program.line_task_groups.iter().enumerate() {
        let at = format!("line task group {index}");
        let activation = program
            .functions
            .get(group.activation.index())
            .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                at: at.clone(),
                message: "line task group activation function is absent".to_owned(),
            })?;
        if activation.kind != AwbcFunctionKind::LineActivation {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: at.clone(),
                message: "line task group activation must target a LineActivation function"
                    .to_owned(),
            });
        }
        let capture_types = program
            .signatures
            .get(activation.signature.index())
            .map(|signature| signature.params.as_slice())
            .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                at: at.clone(),
                message: "line task group activation signature is absent".to_owned(),
            })?;
        if capture_types.len() != group.captures.len()
            || capture_types
                .iter()
                .any(|ty| !super::code::runtime_type_permits_copy(program, *ty, 0))
        {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: at.clone(),
                message: "line task group capture ABI must be exact and recursively unrestricted"
                    .to_owned(),
            });
        }
        check_index(
            program.line_task_nodes.len(),
            group.root.0,
            "line_task_nodes",
            &at,
        )?;
        let node_end =
            group
                .nodes
                .checked_end()
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line task node range overflows".to_owned(),
                })?;
        if group.nodes.start > group.root.0
            || group.root.0 >= node_end
            || node_end as usize > program.line_task_nodes.len()
        {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: at.clone(),
                message: "line task root is outside its dense node range".to_owned(),
            });
        }
        let mut scheduled_children = BTreeSet::new();
        for (site_index, site) in group.handle_sites.iter().enumerate() {
            let Some(child) = site.scheduled_child else {
                continue;
            };
            let site_index =
                u32::try_from(site_index).map_err(|_| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line handle site index exceeds the AWBC identity domain".to_owned(),
                })?;
            if child.0 < group.nodes.start
                || child.0 >= node_end
                || !scheduled_children.insert(child)
                || !matches!(
                    program.line_task_nodes.get(child.index()),
                    Some(AwbcLineTaskNode::Child {
                        trigger: crate::awbc::schema::AwbcLineTaskTrigger::Scheduled(trigger),
                        join: crate::awbc::schema::AwbcChildJoinPolicy::Join,
                        cancel: crate::awbc::schema::AwbcChildCancelPolicy::CancelAndJoin,
                        scope,
                    }) if trigger.0 == site_index
                        && matches!(
                            program.line_task_nodes.get(scope.index()),
                            Some(AwbcLineTaskNode::Action(_))
                        )
                )
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message:
                        "scheduled handle site must pair one-to-one with a joined action child"
                            .to_owned(),
                });
            }
        }
        for (node_index, node) in program.line_task_nodes
            [group.nodes.start as usize..node_end as usize]
            .iter()
            .enumerate()
        {
            let AwbcLineTaskNode::Child {
                trigger: crate::awbc::schema::AwbcLineTaskTrigger::Scheduled(site),
                ..
            } = node
            else {
                continue;
            };
            let global = group
                .nodes
                .start
                .checked_add(u32::try_from(node_index).map_err(|_| {
                    AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "scheduled child index exceeds the AWBC identity domain"
                            .to_owned(),
                    }
                })?)
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "scheduled child identity overflows".to_owned(),
                })?;
            if group
                .handle_sites
                .get(site.index())
                .and_then(|site| site.scheduled_child)
                != Some(crate::awbc::schema::AwbcLineTaskNodeId(global))
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "scheduled child does not point back to its sole handle site"
                        .to_owned(),
                });
            }
        }
        for handler in &group.cancel_handlers {
            check_index(
                program.functions.len(),
                handler.function.0,
                "functions",
                &at,
            )?;
            if program.functions[handler.function.index()].kind != AwbcFunctionKind::LineTask {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line cancellation handler must target a LineTask function".to_owned(),
                });
            }
            if program
                .signatures
                .get(
                    program.functions[handler.function.index()]
                        .signature
                        .index(),
                )
                .is_none_or(|signature| signature.params.as_slice() != capture_types)
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line cancellation handler capture signature disagrees with its group"
                        .to_owned(),
                });
            }
        }
        for node in &program.line_task_nodes[group.nodes.start as usize..node_end as usize] {
            let contained = |node: crate::awbc::schema::AwbcLineTaskNodeId| {
                group.nodes.start <= node.0 && node.0 < node_end
            };
            let children = match node {
                AwbcLineTaskNode::Sequence(children)
                | AwbcLineTaskNode::Start(children)
                | AwbcLineTaskNode::Parallel { children, .. } => Some(children.as_slice()),
                AwbcLineTaskNode::Child { scope, .. } => {
                    if !contained(*scope) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "line child scope escapes its dense group node range"
                                .to_owned(),
                        });
                    }
                    None
                }
                AwbcLineTaskNode::Action(_) => None,
            };
            if children.is_some_and(|children| children.iter().any(|child| !contained(*child))) {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line node child escapes its dense group node range".to_owned(),
                });
            }
            if let AwbcLineTaskNode::Action(function) = node
                && program
                    .functions
                    .get(function.index())
                    .and_then(|function| program.signatures.get(function.signature.index()))
                    .is_none_or(|signature| signature.params.as_slice() != capture_types)
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line action capture signature disagrees with its group".to_owned(),
                });
            }
        }
        for cleanup in [
            group.cleanup_completed,
            group.cleanup_cancelled,
            group.cleanup_failed,
        ]
        .into_iter()
        .flatten()
        {
            let signature = program
                .functions
                .get(cleanup.index())
                .and_then(|function| program.signatures.get(function.signature.index()));
            if signature.is_none_or(|signature| signature.params.as_slice() != capture_types) {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line cleanup capture signature disagrees with its group".to_owned(),
                });
            }
        }
    }
    for (index, node) in program.line_task_nodes.iter().enumerate() {
        let at = format!("line task node {index}");
        match node {
            AwbcLineTaskNode::Sequence(nodes)
            | AwbcLineTaskNode::Start(nodes)
            | AwbcLineTaskNode::Parallel {
                children: nodes, ..
            } => {
                for node in nodes {
                    check_index(
                        program.line_task_nodes.len(),
                        node.0,
                        "line_task_nodes",
                        &at,
                    )?;
                }
            }
            AwbcLineTaskNode::Child {
                trigger: _,
                cancel,
                scope,
                ..
            } => {
                check_index(
                    program.line_task_nodes.len(),
                    scope.0,
                    "line_task_nodes",
                    &at,
                )?;
                if matches!(cancel, crate::awbc::schema::AwbcChildCancelPolicy::Detach) {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "line child detach policy has no verified ownership boundary"
                            .to_owned(),
                    });
                }
            }
            AwbcLineTaskNode::Action(function) => {
                check_index(program.functions.len(), function.0, "functions", &at)?;
                if program.functions[function.index()].kind != AwbcFunctionKind::LineTask {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "line action must target a LineTask function".to_owned(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn verify_stream_tables(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, stream) in program.stream_plans.iter().enumerate() {
        let at = format!("stream plan {index}");
        check_string(program, stream.public_id, &at)?;
        check_index(
            program.runtime_types.len(),
            stream.item_type.0,
            "runtime_types",
            &at,
        )?;
        check_index(
            program.runtime_types.len(),
            stream.error_type.0,
            "runtime_types",
            &at,
        )?;
        check_index(
            program.functions.len(),
            stream.transform.0,
            "functions",
            &at,
        )?;
        if program.functions[stream.transform.index()].kind != AwbcFunctionKind::StreamTransform {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "stream transform references wrong function kind".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_entries(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let mut ids = BTreeSet::new();
    let mut runtime_ids = BTreeSet::new();
    for (entry_index, entry) in program.entries.iter().enumerate() {
        let at = format!("entry {entry_index}");
        check_string(program, entry.public_id, &at)?;
        if !runtime_ids.insert(entry.runtime_id.clone()) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: at.clone(),
                message: "duplicate semantic runtime entry identity".to_owned(),
            });
        }
        if !ids.insert(entry.public_id) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "duplicate public entry id".to_owned(),
            });
        }
        if let AwbcEntryKind::Custom(kind) = entry.kind {
            check_string(program, kind, &format!("entry {entry_index}"))?;
        }
        match &entry.target {
            AwbcEntryTarget::Function { function } => {
                verify_entry_function(program, entry_index, *function)?;
            }
            AwbcEntryTarget::Routes(routes) => {
                if routes.is_empty() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "route entry must contain at least one route".to_owned(),
                    });
                }
                for (route_index, route) in routes.iter().enumerate() {
                    let capture_count =
                        verify_route_segments(program, entry_index, route_index, route)?;
                    if let Some(previous) = route_index
                        .checked_sub(1)
                        .and_then(|index| routes.get(index))
                        && previous
                            .method
                            .cmp(&route.method)
                            .then_with(|| route_dispatch_cmp(program, previous, route))
                            != Ordering::Less
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: format!("entry {entry_index} route {route_index}"),
                            message: "routes are not in canonical method/dispatch order".to_owned(),
                        });
                    }
                    if routes[..route_index].iter().any(|previous| {
                        previous.method == route.method
                            && route_dispatch_overlaps(program, previous, route)
                    }) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: format!("entry {entry_index} route {route_index}"),
                            message: "same-method route dispatch patterns overlap".to_owned(),
                        });
                    }
                    verify_entry_function(program, entry_index, route.target)?;
                    let layout = &program.frame_layouts
                        [program.functions[route.target.index()].frame_layout.index()];
                    let signature = &program.signatures
                        [program.functions[route.target.index()].signature.index()];
                    if route.bindings.len() != signature.params.len()
                        || capture_count != route.bindings.len()
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: format!("entry {entry_index}"),
                            message: "route bindings must cover every target parameter and capture"
                                .to_owned(),
                        });
                    }
                    let mut captures = BTreeSet::new();
                    for (index, binding) in route.bindings.iter().enumerate() {
                        if binding.parameter.index().ok() != Some(index) {
                            return Err(AwbcVerifyError::InvalidInvariant {
                                at: format!("entry {entry_index} route {route_index}"),
                                message: "route parameter coordinates are not complete and ordered"
                                    .to_owned(),
                            });
                        }
                        let parameter_registers = layout
                            .slots
                            .iter()
                            .filter(|slot| slot.role == AwbcFrameSlotRole::Parameter)
                            .count();
                        if parameter_registers != signature.params.len() {
                            return Err(AwbcVerifyError::InvalidInvariant {
                                at: format!("entry {entry_index} route {route_index}"),
                                message: "route target frame parameters differ from its signature"
                                    .to_owned(),
                            });
                        }
                        match binding.source {
                            AwbcRouteBindingSource::PathCapture(capture) => {
                                let capture = capture.index().map_err(|_| {
                                    AwbcVerifyError::InvalidInvariant {
                                        at: format!("entry {entry_index}"),
                                        message: "route capture coordinate is invalid".to_owned(),
                                    }
                                })?;
                                if capture >= capture_count || !captures.insert(capture) {
                                    return Err(AwbcVerifyError::InvalidInvariant {
                                        at: format!("entry {entry_index}"),
                                        message:
                                            "route capture coordinates are not complete and unique"
                                                .to_owned(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn verify_entry_runtime_contracts(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let mut callable_ids = BTreeSet::new();
    for (index, executable) in program.callable_executables.iter().enumerate() {
        let at = format!("callable executable {index}");
        if !callable_ids.insert(executable.role.callable.clone()) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "duplicate stable callable executable identity".to_owned(),
            });
        }
        check_index(
            program.functions.len(),
            executable.function.0,
            "functions",
            &at,
        )?;
        if !matches!(
            program.functions[executable.function.index()].kind,
            AwbcFunctionKind::PureHelper | AwbcFunctionKind::Flow
        ) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "role callable maps to a non-callable Product AWBC function".to_owned(),
            });
        }
    }

    let mut bound_flow_ids = BTreeSet::new();
    let mut bound_flow_functions = BTreeSet::new();
    for (index, binding) in program.flow_bindings.iter().enumerate() {
        let at = format!("flow binding {index}");
        if !bound_flow_ids.insert(binding.flow.clone()) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "duplicate stable Flow binding identity".to_owned(),
            });
        }
        if !bound_flow_functions.insert(binding.function) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "multiple semantic Flow identities map to one Product function".to_owned(),
            });
        }
        check_index(
            program.functions.len(),
            binding.function.0,
            "functions",
            &at,
        )?;
        let function = &program.functions[binding.function.index()];
        if function.kind != AwbcFunctionKind::Flow {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "semantic Flow binding maps to a non-flow Product function".to_owned(),
            });
        }
    }
    for (index, function) in program.functions.iter().enumerate() {
        let function_id = AwbcFunctionId(u32::try_from(index).unwrap_or(u32::MAX));
        if function.kind == AwbcFunctionKind::Flow && !bound_flow_functions.contains(&function_id) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: format!("function {index}"),
                message: "Flow function has no exact semantic Flow binding".to_owned(),
            });
        }
    }

    let mut flow_ids = BTreeSet::new();
    for (index, executable) in program.flow_executables.iter().enumerate() {
        let at = format!("flow executable {index}");
        if !flow_ids.insert(executable.metadata.flow.clone()) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "duplicate stable flow executable identity".to_owned(),
            });
        }
        check_index(
            program.functions.len(),
            executable.function.0,
            "functions",
            &at,
        )?;
        let function = &program.functions[executable.function.index()];
        if function.kind != AwbcFunctionKind::Flow {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "flow executable maps to a non-flow Product AWBC function".to_owned(),
            });
        }
        if program.flow_function(&executable.metadata.flow) != Some(executable.function) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "flow executable metadata differs from the typed Flow binding".to_owned(),
            });
        }
        if let Some(controller) = executable.metadata.controller.as_ref() {
            let Some(callable) = find_callable_executable(program, controller) else {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "flow controller role has no exact callable executable".to_owned(),
                });
            };
            if callable.function != executable.function {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "flow controller and flow metadata map to different functions"
                        .to_owned(),
                });
            }
        }
    }

    let mut referenced_callables = BTreeSet::new();
    let mut referenced_flows = BTreeSet::new();
    for (entry_index, entry) in program.entries.iter().enumerate() {
        let at = format!("entry {entry_index} runtime contract");
        if let AwbcEntryTarget::Function {
            function: target, ..
        } = &entry.target
        {
            check_index(program.functions.len(), target.0, "functions", &at)?;
        }
        match (&entry.kind, &entry.target, &entry.roles) {
            (
                AwbcEntryKind::Game | AwbcEntryKind::Editor | AwbcEntryKind::Test,
                AwbcEntryTarget::Function {
                    function: target, ..
                },
                RuntimeEntryRoles::Stateful(roles),
            ) => {
                if entry.binding != roles.binding {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "top-level entry binding differs from stateful role binding"
                            .to_owned(),
                    });
                }
                if !roles.command_policy.root_limits.is_valid() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "stateful entry has invalid durable-root execution limits"
                            .to_owned(),
                    });
                }
                for (label, role, arity) in [
                    ("initializer", &roles.initializer, 0_usize),
                    ("reducer", &roles.reducer, 2_usize),
                ] {
                    let Some(executable) = find_callable_executable(program, role) else {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: format!("{label} role has no exact callable executable"),
                        });
                    };
                    let function = &program.functions[executable.function.index()];
                    let signature = &program.signatures[function.signature.index()];
                    if function.kind != AwbcFunctionKind::PureHelper
                        || signature.params.len() != arity
                        || signature.result.is_none()
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: format!(
                                "{label} role does not map to the required pure callable shape"
                            ),
                        });
                    }
                    referenced_callables.insert((role.callable.clone(), role.contract));
                }
                let Some(flow) = program.flow_executables.iter().find(|executable| {
                    executable.metadata.flow == roles.initial_flow.flow
                        && executable.metadata.contract == roles.initial_flow.contract
                }) else {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "initial-flow role has no exact flow executable".to_owned(),
                    });
                };
                if flow.function != *target {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "stateful target differs from its bound initial flow".to_owned(),
                    });
                }
                let function = &program.functions[flow.function.index()];
                let signature = &program.signatures[function.signature.index()];
                let [parameter] = signature.params.as_slice() else {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "stateful initial flow must have exactly one parameter".to_owned(),
                    });
                };
                let parameter_type = &program.runtime_types[parameter.index()];
                if parameter_type.semantic_identity() != roles.state.semantic_identity {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "initial flow does not receive the selected owned state role"
                            .to_owned(),
                    });
                }
                verify_role_schema(&at, "state", &roles.state.schema, roles.state.layout)?;
                verify_role_schema(&at, "event", &roles.event.schema, roles.event.layout)?;
                referenced_flows
                    .insert((roles.initial_flow.flow.clone(), roles.initial_flow.contract));
            }
            (
                AwbcEntryKind::Agent,
                AwbcEntryTarget::Function {
                    function: target, ..
                },
                RuntimeEntryRoles::Agent(roles),
            ) => {
                if entry.binding != roles.binding {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "top-level entry binding differs from Agent role binding"
                            .to_owned(),
                    });
                }
                let Some(callable) = find_callable_executable(program, &roles.controller) else {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "Agent controller role has no exact callable executable"
                            .to_owned(),
                    });
                };
                if callable.function != *target
                    || program.functions[target.index()].kind != AwbcFunctionKind::Flow
                {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "Agent controller target differs from its callable executable"
                            .to_owned(),
                    });
                }
                let Some(flow) = program.flow_executables.iter().find(|flow| {
                    flow.function == *target
                        && flow.metadata.controller.as_ref() == Some(&roles.controller)
                }) else {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "Agent controller target has no exact flow executable".to_owned(),
                    });
                };
                referenced_callables
                    .insert((roles.controller.callable.clone(), roles.controller.contract));
                referenced_flows.insert((flow.metadata.flow.clone(), flow.metadata.contract));
            }
            (
                AwbcEntryKind::Cli
                | AwbcEntryKind::Server
                | AwbcEntryKind::Activity
                | AwbcEntryKind::Bench
                | AwbcEntryKind::Custom(_),
                AwbcEntryTarget::Function { .. } | AwbcEntryTarget::Routes(_),
                RuntimeEntryRoles::None,
            ) => {
                let functions = match &entry.target {
                    AwbcEntryTarget::Function { function, .. } => std::slice::from_ref(function),
                    AwbcEntryTarget::Routes(routes) => {
                        for route in routes {
                            let Some(flow) = program
                                .flow_executables
                                .iter()
                                .find(|flow| flow.function == route.target)
                            else {
                                return Err(AwbcVerifyError::InvalidInvariant {
                                    at: at.clone(),
                                    message: "route target has no exact flow executable".to_owned(),
                                });
                            };
                            referenced_flows
                                .insert((flow.metadata.flow.clone(), flow.metadata.contract));
                        }
                        &[]
                    }
                };
                for function in functions {
                    let Some(flow) = program
                        .flow_executables
                        .iter()
                        .find(|flow| flow.function == *function)
                    else {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "entry target has no exact flow executable".to_owned(),
                        });
                    };
                    referenced_flows.insert((flow.metadata.flow.clone(), flow.metadata.contract));
                }
            }
            _ => {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "entry kind, target, and semantic roles are incompatible".to_owned(),
                });
            }
        }
    }

    for executable in &program.callable_executables {
        if !referenced_callables
            .contains(&(executable.role.callable.clone(), executable.role.contract))
        {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: executable.role.callable.as_str().to_owned(),
                message: "callable executable is not reachable from an entry role".to_owned(),
            });
        }
    }
    for executable in &program.flow_executables {
        if !referenced_flows.contains(&(
            executable.metadata.flow.clone(),
            executable.metadata.contract,
        )) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: executable.metadata.flow.canonical_label(),
                message: "flow executable is not reachable from an entry role".to_owned(),
            });
        }
    }
    Ok(())
}

fn find_callable_executable<'a>(
    program: &'a AwbcProgram,
    role: &RuntimeCallableRole,
) -> Option<&'a crate::awbc::schema::AwbcCallableExecutable> {
    program
        .callable_executables
        .iter()
        .find(|executable| executable.role == *role)
}

fn verify_role_schema(
    at: &str,
    role: &'static str,
    schema: &crate::entry::RuntimeTypeSchema,
    layout: crate::entry::TypeLayoutHash,
) -> Result<(), AwbcVerifyError> {
    let actual = schema
        .try_layout_hash()
        .map_err(|error| AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: format!("{role} schema is invalid: {error}"),
        })?;
    if actual != layout {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: format!("{role} schema layout does not match checked metadata"),
        });
    }
    Ok(())
}

fn verify_route_segments(
    program: &AwbcProgram,
    entry_index: usize,
    route_index: usize,
    route: &AwbcRoute,
) -> Result<usize, AwbcVerifyError> {
    let at = format!("entry {entry_index} route {route_index}");
    let mut capture_count = 0usize;
    for segment in &route.segments {
        match segment {
            AwbcRouteSegment::Literal(literal) => {
                check_string(program, *literal, &at)?;
                let value = &program.strings[literal.index()];
                if value.is_empty()
                    || value.starts_with(':')
                    || value.contains('/')
                    || value.chars().any(char::is_control)
                {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "route literal segment is not canonical".to_owned(),
                    });
                }
            }
            AwbcRouteSegment::Capture(coordinate) => {
                if coordinate.index().ok() != Some(capture_count) {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "route capture coordinates are not complete and ordered"
                            .to_owned(),
                    });
                }
                capture_count += 1;
            }
        }
    }
    Ok(capture_count)
}

fn route_dispatch_overlaps(program: &AwbcProgram, left: &AwbcRoute, right: &AwbcRoute) -> bool {
    left.segments.len() == right.segments.len()
        && left
            .segments
            .iter()
            .zip(right.segments.iter())
            .all(|(left, right)| match (left, right) {
                (AwbcRouteSegment::Literal(left), AwbcRouteSegment::Literal(right)) => {
                    program.strings[left.index()] == program.strings[right.index()]
                }
                (
                    AwbcRouteSegment::Literal(_) | AwbcRouteSegment::Capture(_),
                    AwbcRouteSegment::Capture(_),
                )
                | (AwbcRouteSegment::Capture(_), AwbcRouteSegment::Literal(_)) => true,
            })
}

fn route_dispatch_cmp(program: &AwbcProgram, left: &AwbcRoute, right: &AwbcRoute) -> Ordering {
    for (left, right) in left.segments.iter().zip(right.segments.iter()) {
        let ordering = match (left, right) {
            (AwbcRouteSegment::Literal(left), AwbcRouteSegment::Literal(right)) => {
                program.strings[left.index()].cmp(&program.strings[right.index()])
            }
            (AwbcRouteSegment::Literal(_), AwbcRouteSegment::Capture(_)) => Ordering::Less,
            (AwbcRouteSegment::Capture(_), AwbcRouteSegment::Literal(_)) => Ordering::Greater,
            (AwbcRouteSegment::Capture(_), AwbcRouteSegment::Capture(_)) => Ordering::Equal,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.segments.len().cmp(&right.segments.len())
}

fn verify_entry_function(
    program: &AwbcProgram,
    entry_index: usize,
    function: AwbcFunctionId,
) -> Result<(), AwbcVerifyError> {
    check_index(
        program.functions.len(),
        function.0,
        "functions",
        &format!("entry {entry_index}"),
    )?;
    if program.flow_identity(function).is_none() {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: format!("entry {entry_index}"),
            message: "entry target has no exact semantic Flow binding".to_owned(),
        });
    }
    Ok(())
}

fn verify_maps_and_resources(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let mut displays = BTreeSet::new();
    for (index, display) in program.display_map.iter().enumerate() {
        check_index(
            program.content_units.len(),
            display.content.0,
            "content_units",
            &format!("display map {index}"),
        )?;
        check_string(
            program,
            display.display_key,
            &format!("display map {index}"),
        )?;
        if !displays.insert(display.content) {
            return Err(AwbcVerifyError::DuplicateMapIdentity { entry: index });
        }
    }
    let mut sources = BTreeSet::new();
    for (index, source) in program.source_map.iter().enumerate() {
        check_string(program, source.source_file, &format!("source map {index}"))?;
        check_optional_string(program, source.anchor, &format!("source map {index}"))?;
        if source.start > source.end
            || source.end - source.start > verifier.budget.source_span_bytes
        {
            return Err(AwbcVerifyError::InvalidSourceSpan {
                entry: index,
                start: source.start,
                end: source.end,
            });
        }
        let identity = match source.location {
            AwbcCodeLocation::Instruction(id) => {
                check_index(
                    program.instructions.len(),
                    id.0,
                    "instructions",
                    &format!("source map {index}"),
                )?;
                (0_u8, id.0)
            }
            AwbcCodeLocation::Block(id) => {
                check_index(
                    program.blocks.len(),
                    id.0,
                    "blocks",
                    &format!("source map {index}"),
                )?;
                (1_u8, id.0)
            }
            AwbcCodeLocation::ResumePoint(id) => {
                check_index(
                    program.resume_points.len(),
                    id.0,
                    "resume_points",
                    &format!("source map {index}"),
                )?;
                (2_u8, id.0)
            }
        };
        if !sources.insert(identity) {
            return Err(AwbcVerifyError::DuplicateMapIdentity { entry: index });
        }
    }
    let mut resources = BTreeSet::new();
    for (index, resource) in program.resources.iter().enumerate() {
        let at = format!("resource {index}");
        check_string(program, resource.public_id, &at)?;
        check_string(program, resource.kind, &at)?;
        if !resources.insert(resource.public_id) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "duplicate resource public id".to_owned(),
            });
        }
    }
    Ok(())
}

fn check_capability(
    verifier: &Verifier<'_, '_>,
    capability: AwbcStringId,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    check_string(verifier.program, capability, at)?;
    if let Some(allowed) = verifier.context.allowed_capabilities {
        let name = &verifier.program.strings[capability.index()];
        if !allowed.contains(name) {
            return Err(AwbcVerifyError::CapabilityDenied {
                capability: name.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn check_index(
    len: usize,
    index: u32,
    table: &'static str,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    if index as usize >= len {
        Err(AwbcVerifyError::IndexOutOfBounds {
            table,
            index,
            at: at.to_owned(),
        })
    } else {
        Ok(())
    }
}

pub(super) fn check_optional_index(
    len: usize,
    index: Option<u32>,
    table: &'static str,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    if let Some(index) = index {
        check_index(len, index, table, at)?;
    }
    Ok(())
}

pub(super) fn checked_range(
    range: AwbcTableRange,
    len: usize,
    table: &'static str,
    at: &str,
) -> Result<std::ops::Range<usize>, AwbcVerifyError> {
    let Some(end) = range.checked_end() else {
        return Err(AwbcVerifyError::RangeOutOfBounds {
            table,
            start: range.start,
            len: range.len,
            at: at.to_owned(),
        });
    };
    if end as usize > len {
        return Err(AwbcVerifyError::RangeOutOfBounds {
            table,
            start: range.start,
            len: range.len,
            at: at.to_owned(),
        });
    }
    Ok(range.start as usize..end as usize)
}

pub(super) fn check_string(
    program: &AwbcProgram,
    id: AwbcStringId,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    check_index(program.strings.len(), id.0, "strings", at)
}

pub(super) fn check_optional_string(
    program: &AwbcProgram,
    id: Option<AwbcStringId>,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    check_optional_index(program.strings.len(), id.map(|id| id.0), "strings", at)
}

pub(super) fn block_is_in_function(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: AwbcBlockId,
) -> bool {
    block.index() < verifier.block_owner.len() && verifier.block_owner[block.index()] == function
}

pub(crate) fn types_compatible(
    program: &AwbcProgram,
    expected: AwbcTypeId,
    actual: AwbcTypeId,
) -> bool {
    types_compatible_inner(program, expected, actual, &mut BTreeSet::new())
}

fn types_compatible_inner(
    program: &AwbcProgram,
    expected: AwbcTypeId,
    actual: AwbcTypeId,
    visiting: &mut BTreeSet<(AwbcTypeId, AwbcTypeId)>,
) -> bool {
    if expected == actual {
        return true;
    }
    let Some(expected_row) = program.runtime_types.get(expected.index()) else {
        return false;
    };
    let Some(actual_row) = program.runtime_types.get(actual.index()) else {
        return false;
    };
    let expected_type = expected_row.shape();
    let actual_type = actual_row.shape();
    if matches!(expected_type, AwbcRuntimeTypeShape::Dynamic)
        || matches!(actual_type, AwbcRuntimeTypeShape::Dynamic)
    {
        return true;
    }
    if !visiting.insert((expected, actual)) {
        return false;
    }
    let compatible = match (expected_type, actual_type) {
        (
            AwbcRuntimeTypeShape::Opaque {
                arguments: expected_arguments,
                ..
            },
            AwbcRuntimeTypeShape::Opaque {
                arguments: actual_arguments,
                ..
            },
        ) => expected_row
            .try_opaque_owner(&program.strings)
            .ok()
            .flatten()
            .zip(actual_row.try_opaque_owner(&program.strings).ok().flatten())
            .is_some_and(|(expected, actual)| {
                expected.accepts_owner(&actual)
                    && expected_arguments.len() == actual_arguments.len()
                    && expected_arguments
                        .iter()
                        .zip(actual_arguments)
                        .all(|(expected, actual)| {
                            types_compatible_inner(program, *expected, *actual, visiting)
                        })
            }),
        (AwbcRuntimeTypeShape::Choice(expected), AwbcRuntimeTypeShape::Choice(actual)) => {
            actual.iter().all(|actual| {
                expected
                    .iter()
                    .any(|expected| types_compatible_inner(program, *expected, *actual, visiting))
            })
        }
        (AwbcRuntimeTypeShape::Choice(expected), _) => expected
            .iter()
            .any(|expected| types_compatible_inner(program, *expected, actual, visiting)),
        (_, AwbcRuntimeTypeShape::Choice(actual)) => actual
            .iter()
            .all(|actual| types_compatible_inner(program, expected, *actual, visiting)),
        _ => false,
    };
    visiting.remove(&(expected, actual));
    compatible
}

pub(super) fn effect_set_is_subset(
    program: &AwbcProgram,
    subset: AwbcEffectSetId,
    superset: AwbcEffectSetId,
) -> bool {
    let Some(subset) = program.effect_sets.get(subset.index()) else {
        return false;
    };
    let Some(superset) = program.effect_sets.get(superset.index()) else {
        return false;
    };
    subset
        .effects
        .iter()
        .all(|effect| superset.effects.binary_search(effect).is_ok())
}
