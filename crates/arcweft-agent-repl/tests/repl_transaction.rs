use arcweft_agent_repl::{
    ReplBaseSnapshot, ReplCellFilter, ReplGenerationId, ReplResetOptions, ReplSession,
    ReplSessionOptions,
};
use arcweft_lang_sema::project_index::{ProgramHash, ProjectSemanticIndex};

#[test]
fn repl_transaction_reset_returns_empty_overlay() {
    let mut repl = test_repl("test.program.reset");
    let outcome = repl.reset_to_base(ReplResetOptions::default());
    assert_eq!(outcome.removed_cells, 0);
    assert_eq!(outcome.retained_generation, ReplGenerationId::base());
    assert!(repl.cells(ReplCellFilter::default()).cells.is_empty());
}

#[test]
fn repl_transaction_base_change_advances_generation() {
    let mut repl = test_repl("test.program.old");
    let outcome = repl.replace_base_snapshot(ReplBaseSnapshot::from_project(
        "new",
        ProjectSemanticIndex::new(ProgramHash::new("test.program.new")),
    ));
    assert_eq!(outcome.evidence.active_generation, ReplGenerationId::new(1));
    assert_eq!(outcome.evidence.base_program_hash, "test.program.new");
}

fn test_repl(program_hash: &str) -> ReplSession {
    ReplSession::new(
        ReplBaseSnapshot::from_project(
            "test",
            ProjectSemanticIndex::new(ProgramHash::new(program_hash)),
        ),
        ReplSessionOptions::default(),
    )
}
