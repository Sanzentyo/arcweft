use std::sync::Arc;

use super::{compilation_state, removed_role_project};
use crate::project::{
    CompiledProjectModule, ProjectCompilationContext, ProjectCompilationLease, ProjectCompileCache,
    ProjectCompileStage, ProjectCompileUnitFingerprint, ProjectEntrySelection,
    ProjectEntrySelectionKind, compile_project, compile_project_with_cache,
};

const VALID: &str = "fn identity(value: i64) -> i64 { value }\nflow main() -> i64 { return identity(42i64) }\nentry cli @entry.main { goto @flow.main }\n";
const REJECTED: &str =
    "fn identity(value: i64) -> i64 { value }\nfn caller() { identity(1i64, 2i64); }\n";

#[test]
fn semantic_lease_is_the_compiled_projects_exact_ancestor() {
    let (project, context) = removed_role_project(VALID);
    let (mut session, parsed) = compilation_state(&project);
    let compiled = Arc::new(compile_project(&mut session, &project, &parsed, &context).unwrap());
    let lease = ProjectCompilationLease::Compiled(Arc::clone(&compiled));
    let analysis = compiled.analysis_lease();
    assert!(Arc::ptr_eq(lease.analysis_lease().unwrap(), analysis));
    assert!(Arc::ptr_eq(lease.tooling_lease(), analysis.tooling_lease()));
    assert!(Arc::ptr_eq(
        analysis.final_analysis().checked_callables(),
        analysis.semantic_index().checked_callables(),
    ));
    analysis
        .final_analysis()
        .validate_generation(
            analysis.hir_project().analysis_view().unwrap(),
            analysis.registered_world().symbols(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(lease.compiled().unwrap(), &compiled));
}

#[derive(Default)]
struct RecordingCache {
    stores: usize,
}

impl ProjectCompileCache for RecordingCache {
    fn load(&mut self, _key: ProjectCompileUnitFingerprint) -> Option<Vec<CompiledProjectModule>> {
        None
    }

    fn store(&mut self, _key: ProjectCompileUnitFingerprint, _modules: &[CompiledProjectModule]) {
        self.stores += 1;
    }
}

#[test]
fn semantic_lease_survives_call_rejection_without_committing_compile_cache() {
    let (project, context) = removed_role_project(REJECTED);
    let (mut session, parsed) = compilation_state(&project);
    let mut cache = RecordingCache::default();
    let error = compile_project_with_cache(&mut session, &project, &parsed, &context, &mut cache)
        .expect_err("invalid call cannot publish an executable");
    assert_eq!(error.stage(), ProjectCompileStage::TypeCheck.as_str());
    let lease = error.compilation_lease().unwrap();
    let ProjectCompilationLease::Analyzed(analysis) = lease else {
        panic!("completed analysis survives semantic admission failure");
    };
    assert!(lease.compiled().is_none());
    assert_eq!(analysis.final_analysis().call_diagnostics().count(), 1);
    assert!(
        analysis
            .final_analysis()
            .calls()
            .any(|(_, call)| call.selected_application().is_none())
    );
    assert!(Arc::ptr_eq(
        analysis.final_analysis().checked_callables(),
        analysis.semantic_index().checked_callables(),
    ));
    assert_eq!(cache.stores, 0);
}

#[test]
fn semantic_lease_is_absent_when_analysis_does_not_complete() {
    let (project, context) = removed_role_project("fn caller() { unknown(); }\n");
    let (mut session, parsed) = compilation_state(&project);
    let error = compile_project(&mut session, &project, &parsed, &context).unwrap_err();
    let lease = error.compilation_lease().unwrap();
    assert!(matches!(lease, ProjectCompilationLease::Hir(_)));
    assert!(lease.analysis_lease().is_none());
    assert!(lease.compiled().is_none());
}

#[test]
fn semantic_lease_survives_later_entry_selection_failure() {
    let (project, context) = removed_role_project(VALID);
    let context = ProjectCompilationContext::new(
        Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
        Arc::new(context.facts().clone()),
        Arc::clone(context.resource_types()),
        None,
        Some(ProjectEntrySelection::new(
            arcweft_id::PublicId::try_new("entry.missing").unwrap(),
            ProjectEntrySelectionKind::Cli,
        )),
    );
    let (mut session, parsed) = compilation_state(&project);
    let error = compile_project(&mut session, &project, &parsed, &context).unwrap_err();
    let lease = error.compilation_lease().unwrap();
    assert!(matches!(lease, ProjectCompilationLease::Analyzed(_)));
    assert_eq!(
        lease
            .analysis_lease()
            .unwrap()
            .final_analysis()
            .call_diagnostics()
            .count(),
        0
    );
    assert!(lease.compiled().is_none());
}
