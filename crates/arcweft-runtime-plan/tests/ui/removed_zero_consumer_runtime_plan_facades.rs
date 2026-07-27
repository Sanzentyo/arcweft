use arcweft_runtime_plan::{
    audio, expr,
    flow::{RuntimePlanLowerOptions, lower_runtime_plan},
    fx::lower_fx_definitions,
    host_request, labels, pattern, render_text, source,
    typed_evidence::RuntimeTypedExpressionId,
};

fn removed_options(options: &RuntimePlanLowerOptions) {
    let _ = RuntimePlanLowerOptions::new();
    let _ = options.trait_methods();
}

fn removed_expression_id(id: RuntimeTypedExpressionId) {
    let _ = id.index();
}

fn main() {}
