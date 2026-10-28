use super::*;
use arcweft_lang_sema::final_analysis::CheckedSuspensionStatement;

fn fixture() -> (HirProject, RuntimePlanSemanticFactInput) {
    let project = project_fixture(
        "wait-operand-admission",
        "flow main {\nwait(1s)\nwait(2s)\n}\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let mut input = complete_type_input(&project);
    for (owner, statement) in analysis.statements() {
        if let arcweft_lang_sema::final_analysis::CheckedStatementPayload::Suspension(suspension) =
            statement.payload()
        {
            let CheckedSuspensionStatement::Wait { target } = suspension.as_ref();
            let duration = super::super::RuntimeNormalizedType::new(
                RuntimeSemanticTypeId::from_bytes(
                    *arcweft_lang_sema::types::TypeKind::Duration
                        .semantic_identity_digest()
                        .unwrap()
                        .as_bytes(),
                ),
                RuntimeTypeShape::Duration,
            );
            input
                .expression_facts
                .iter_mut()
                .find(|(id, _)| id == target)
                .unwrap()
                .1
                .ty = Some(duration);
            input.push_suspension(owner, suspension.as_ref().clone());
        }
    }
    assert_eq!(input.suspensions.len(), 2);
    (project, input)
}

#[test]
fn wait_admission_requires_complete_unique_exact_operands_and_duration_types() {
    let (project, input) = fixture();
    let expected = input.suspensions.clone();
    let facts = runtime_facts(&project, input).unwrap();
    let view = super::super::RuntimeExecutableSemanticFactView::Global(&facts);
    for (owner, suspension) in expected {
        assert_eq!(view.suspension(owner), Some(&suspension));
    }

    let (project, mut input) = fixture();
    let (owner, _) = input.suspensions.remove(0);
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::InvalidSuspension { statement: owner }
    );

    let (project, mut input) = fixture();
    input.suspensions.push(input.suspensions[0].clone());
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::Suspension
        }
    );

    let (project, mut input) = fixture();
    let owner = input.suspensions[0].0;
    input.suspensions[0].1 = input.suspensions[1].1.clone();
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::InvalidSuspension { statement: owner }
    );

    let (project, mut input) = fixture();
    let (owner, CheckedSuspensionStatement::Wait { target }) = input.suspensions[0];
    input
        .expression_facts
        .iter_mut()
        .find(|(id, _)| *id == target)
        .unwrap()
        .1
        .ty = Some(unit_type());
    assert_eq!(
        runtime_facts(&project, input).unwrap_err(),
        RuntimeSemanticFactsError::InvalidSuspension { statement: owner }
    );
}
