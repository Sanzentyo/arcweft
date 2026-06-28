use arcweft_core::bytecode::BytecodeVerificationBudget;
use arcweft_lang_sema::project_index::{ProgramHash, ProjectSemanticIndex};

#[test]
fn repl_cell_synthetic_agent_source_compiles_to_verified_bytecode() {
    let source = r#"
#[agent(version = 1)]
agent @agent.repl.cell_0 repl_cell_0()
effects { agent.observe, debug.record }
{
    return "ok"
}
"#;
    let project = ProjectSemanticIndex::new(ProgramHash::new("test.program.repl_cell"));
    let compiled = arcweft_compiler::agent::compile_agent_bundle_with_project(source, &project)
        .expect("synthetic REPL cell source should compile");
    compiled
        .bundle
        .bytecode
        .program
        .verify(BytecodeVerificationBudget::default())
        .expect("compiled REPL cell bytecode should pass verifier");
}
