use arcweft_compiler::{
    error::ValidateHirError,
    hir::{
        resolve_hir_references, resolve_hir_references_with_env, typecheck_hir_with_env,
        validate_hir_with_env,
    },
    link::missing_entry_names,
    lower::{
        lower_source_pure_helper_candidate, lower_source_runtime_plan_with_options,
        lower_source_runtime_plan_with_stats_and_options,
        lower_source_runtime_plan_with_typecheck_and_options,
    },
    reachability::ReachabilityReport,
};

fn removed_report_accessor(report: &ReachabilityReport) {
    let _ = report.all_domains();
}

fn removed_validation_error(_: ValidateHirError) {}

fn main() {}
