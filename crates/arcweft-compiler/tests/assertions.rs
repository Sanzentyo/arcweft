use arcweft_compiler::{error::CompileSourceError, source::compile_source};
use arcweft_runtime_plan::errors::RuntimePlanLowerErrorKind;

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

    let CompileSourceError::RuntimePlan(errors) = error else {
        panic!("expected runtime-plan proof rejection, got {error:?}");
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind(), RuntimePlanLowerErrorKind::UnresolvedProof);
    assert_eq!(
        errors[0]
            .diagnostic()
            .code()
            .expect("proof rejection has a stable code")
            .as_str(),
        "verify.proof.unresolved"
    );
}
