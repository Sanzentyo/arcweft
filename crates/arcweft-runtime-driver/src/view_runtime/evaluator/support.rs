use super::EvaluationFailure;
use crate::view_runtime::{
    BundleViewDiagnosticCode, BundleViewInstancePath, BundleViewInstancePathSegment,
    ViewOccurrenceKey,
};
use arcweft_bundle::resource_codec::ViewAwaitBranchSpan;
use arcweft_core::value::RuntimeValue;
use arcweft_presentation::fx::{FxId, FxInstanceId};
use std::collections::BTreeMap;

pub(super) fn instruction_ordinal(instruction: usize) -> Result<u32, EvaluationFailure> {
    u32::try_from(instruction).map_err(|_| {
        EvaluationFailure::new(
            BundleViewDiagnosticCode::InvalidControlFlow,
            Some(instruction),
            "View instruction index exceeds u32::MAX",
        )
    })
}

pub(super) fn branch_bounds(
    instruction: usize,
    then_span: u32,
    else_span: Option<u32>,
    enclosing_end: usize,
) -> Result<(usize, usize, usize), EvaluationFailure> {
    let then_start = instruction
        .checked_add(1)
        .ok_or_else(|| control_flow_failure(instruction, "branch start overflow"))?;
    let then_end = checked_span_end(then_start, then_span, enclosing_end, instruction)?;
    let else_end = else_span.map_or(Ok(then_end), |span| {
        checked_span_end(then_end, span, enclosing_end, instruction)
    })?;
    Ok((then_start, then_end, else_end))
}

pub(super) fn await_extent(
    instruction: usize,
    enclosing_end: usize,
    branches: [Option<&ViewAwaitBranchSpan>; 4],
) -> Result<usize, EvaluationFailure> {
    let body_start = instruction
        .checked_add(1)
        .ok_or_else(|| control_flow_failure(instruction, "await body start overflow"))?;
    branches
        .into_iter()
        .flatten()
        .try_fold(body_start, |extent, branch| {
            let start = body_start
                .checked_add(branch.start_offset as usize)
                .ok_or_else(|| control_flow_failure(instruction, "await branch start overflow"))?;
            checked_span_end(start, branch.body_span, enclosing_end, instruction)
                .map(|end| extent.max(end))
        })
}

pub(super) fn checked_span_end(
    start: usize,
    span: u32,
    enclosing_end: usize,
    instruction: usize,
) -> Result<usize, EvaluationFailure> {
    let end = start
        .checked_add(span as usize)
        .ok_or_else(|| control_flow_failure(instruction, "control-flow span overflow"))?;
    if end > enclosing_end {
        Err(control_flow_failure(
            instruction,
            "control-flow span escapes its definition or enclosing branch",
        ))
    } else {
        Ok(end)
    }
}

pub(super) fn control_flow_failure(instruction: usize, message: &str) -> EvaluationFailure {
    EvaluationFailure::new(
        BundleViewDiagnosticCode::InvalidControlFlow,
        Some(instruction),
        message,
    )
}

pub(super) fn resolve_mount_path<'a>(
    parameters: &'a BTreeMap<String, RuntimeValue>,
    roots: &'a BTreeMap<String, RuntimeValue>,
    path: &[String],
) -> Option<&'a RuntimeValue> {
    let (first, rest) = path.split_first()?;
    let root = parameters.get(first).or_else(|| roots.get(first))?;
    resolve_record_path(root, rest)
}

pub(super) fn resolve_path<'a>(
    roots: &'a BTreeMap<String, RuntimeValue>,
    path: &[String],
) -> Option<&'a RuntimeValue> {
    let (first, rest) = path.split_first()?;
    resolve_record_path(roots.get(first)?, rest)
}

fn resolve_record_path<'a>(
    mut value: &'a RuntimeValue,
    path: &[String],
) -> Option<&'a RuntimeValue> {
    for segment in path {
        let RuntimeValue::Record(fields) = value else {
            return None;
        };
        value = fields
            .iter()
            .find_map(|field| (field.name() == segment).then_some(field.value()))?;
    }
    Some(value)
}

pub(super) fn derive_fx_instance(
    fx: &FxId,
    key: &ViewOccurrenceKey,
    structural_path: &BundleViewInstancePath,
    target: &str,
    application_ordinal: u32,
    reactive_key: Option<i32>,
) -> FxInstanceId {
    let mut components = vec![
        key.handle.as_str().to_owned(),
        target.to_owned(),
        application_ordinal.to_string(),
    ];
    components.extend(
        structural_path
            .segments()
            .iter()
            .map(|segment| match segment {
                BundleViewInstancePathSegment::Call {
                    instruction,
                    authored_key,
                } => format!(
                    "call:{instruction}:{}",
                    authored_key.map_or_else(|| "_".to_owned(), |key| key.to_string())
                ),
                BundleViewInstancePathSegment::Repeat { instruction, key } => {
                    format!("repeat:{instruction}:{key}")
                }
            }),
    );
    if let Some(key) = reactive_key {
        components.push(format!("key:{key}"));
    }
    FxInstanceId::derive(fx, components.iter().map(String::as_str))
}
