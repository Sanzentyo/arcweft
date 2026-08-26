use arcweft_core::plan::{RuntimeFlowInvocation, RuntimePlan};
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
    let parameters = plan
        .flow_schemas()
        .iter()
        .find(|schema| schema.flow == flow)
        .map(|schema| schema.parameters.clone())
        .ok_or_else(|| {
            eprintln!("error: selected Flow `{target}` has no executable parameter schema");
            ExitCode::FAILURE
        })?;
    let mut supplied = BTreeMap::new();
    for binding in bindings {
        if supplied
            .insert(binding.name.clone(), binding.value.clone())
            .is_some()
        {
            eprintln!("error: duplicate --value binding `{}`", binding.name);
            return Err(ExitCode::from(2));
        }
    }
    let mut parameter_names = BTreeMap::new();
    let mut canonical = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        if parameter_names
            .insert(parameter.name.clone(), parameter.coordinate)
            .is_some()
        {
            eprintln!(
                "error: selected Flow `{target}` has duplicate executable parameter name `{}`",
                parameter.name
            );
            return Err(ExitCode::FAILURE);
        }
        let Some(value) = supplied.remove(&parameter.name) else {
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
    if let Some((name, _)) = supplied.first_key_value() {
        eprintln!("error: selected Flow `{target}` has no parameter named `{name}`");
        return Err(ExitCode::from(2));
    }
    plan.seal_flow_invocation(flow, canonical).map_err(|error| {
        eprintln!("error: invalid --flow invocation: {error}");
        ExitCode::from(2)
    })
}
