use arcweft_compiler::{
    hir::resolve_hir_references,
    link::missing_entry_names,
    lower::{
        lower_source_pure_helper_candidate, lower_source_runtime_plan_with_options,
        lower_source_runtime_plan_with_typecheck_and_options,
    },
    reachability::ReachabilityReport,
};

fn removed_report_accessor(report: &ReachabilityReport) {
    let _ = report.all_domains();
}

fn main() {}
