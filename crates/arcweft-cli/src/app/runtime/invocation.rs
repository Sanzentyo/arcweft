use arcweft_core::plan::{FlowRuntimeId, RuntimeFlowInvocation, RuntimePlan};
use arcweft_core::value::{RuntimeBinding, RuntimeFlowParameterBinding};
use std::collections::BTreeMap;
use std::process::ExitCode;

/// Resolves one external named-value inventory exactly once against the
/// selected plan-owned Flow schema and emits canonical coordinate order.
pub(in crate::app) fn seal_named_flow_invocation(
    plan: RuntimePlan,
    target: &str,
    bindings: &[RuntimeBinding],
) -> Result<RuntimeFlowInvocation, ExitCode> {
    let flow = plan.resolve_flow_target_value(target).map_err(|error| {
        eprintln!("error: cannot resolve --flow `{target}`: {error}");
        ExitCode::from(2)
    })?;
    seal_resolved_flow_invocation(plan, flow, bindings)
}

/// Binds an already selected plan-owned Flow without reinterpreting its private
/// identity as an external source name.
pub(in crate::app) fn seal_resolved_flow_invocation(
    plan: RuntimePlan,
    flow: FlowRuntimeId,
    bindings: &[RuntimeBinding],
) -> Result<RuntimeFlowInvocation, ExitCode> {
    let target = flow.canonical_label();
    let parameters = plan
        .flow_schemas()
        .iter()
        .find(|schema| schema.flow == flow)
        .map(|schema| schema.parameters.as_slice())
        .ok_or_else(|| {
            eprintln!("error: selected Flow `{target}` has no executable parameter schema");
            ExitCode::FAILURE
        })?;
    let mut parameter_names = BTreeMap::new();
    for (index, parameter) in parameters.iter().enumerate() {
        if parameter_names
            .insert(parameter.name.as_str(), index)
            .is_some()
        {
            eprintln!(
                "error: selected Flow `{target}` has duplicate executable parameter name `{}`",
                parameter.name
            );
            return Err(ExitCode::FAILURE);
        }
    }
    let mut supplied = vec![None; parameters.len()];
    for binding in bindings {
        let Some(&index) = parameter_names.get(binding.name.as_str()) else {
            eprintln!(
                "error: selected Flow `{target}` has no parameter named `{}`",
                binding.name
            );
            return Err(ExitCode::from(2));
        };
        if supplied[index].replace(&binding.value).is_some() {
            eprintln!("error: duplicate --value binding `{}`", binding.name);
            return Err(ExitCode::from(2));
        }
    }
    let mut canonical = Vec::with_capacity(parameters.len());
    for (parameter, value) in parameters.iter().zip(supplied) {
        let Some(value) = value else {
            eprintln!(
                "error: selected Flow `{target}` is missing --value `{}`",
                parameter.name
            );
            return Err(ExitCode::from(2));
        };
        canonical.push(RuntimeFlowParameterBinding {
            parameter: parameter.coordinate,
            value,
        });
    }
    plan.seal_flow_invocation_copying(
        flow,
        canonical,
        arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
    )
    .map_err(|error| {
        eprintln!("error: invalid --flow invocation: {error}");
        ExitCode::from(2)
    })
}
