use arcweft_compiler::{project::ProjectCompileStage, source::compile_source};

#[test]
fn compile_source_rejects_unresolved_prove_before_emitting_a_plan() {
    let error = compile_source(
        r"
flow assertions {
    assert.prove(true)
}
",
    )
    .expect_err("undischarged prove assertion blocks code generation");

    let project = error.project();
    assert_eq!(
        project.stage(),
        ProjectCompileStage::RuntimePlanLower.as_str()
    );
    let errors = project.diagnostics();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0]
            .diagnostic()
            .code()
            .expect("proof rejection has a stable code")
            .as_str(),
        "verify.proof.unresolved"
    );
}
