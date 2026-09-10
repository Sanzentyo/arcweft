use super::*;
use crate::final_analysis::{CheckedCaptureAuthorityViolation, CheckedClosure};

#[test]
fn selected_graph_rejects_an_internally_valid_capture_receipt_for_another_interpretation() {
    let fixture = crate::final_analysis::tests::fixture(
        "fn caller() -> Unit { let offset = 0i64; let values = [42i64]; let read = || -> i64 { values[offset] }; () }\n",
        None,
    );
    let report = crate::final_analysis::tests::analyze(&fixture).unwrap();
    let closure = report
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Closure(closure) => Some(closure),
            _ => None,
        })
        .unwrap();
    let project = fixture.project.analysis_view().unwrap();
    let topology = Arc::clone(closure.topology());
    let graph = project
        .selected_expression_graph(
            &topology,
            |owner| {
                let CheckedExpressionResolution::PostfixBracket(selection) =
                    report.expression(owner)?.resolution()
                else {
                    return None;
                };
                Some(selection.candidate())
            },
            |_| Some(HirSelectedCallExpressionDisposition::Structural),
        )
        .unwrap();
    let dialogue_lines = project.seal_selected_dialogue_lines(&graph).unwrap();
    let selected = CheckedSelectedExpressionGraph {
        graph,
        dialogue_lines,
        fx_definition_declarations: BTreeSet::new(),
        fx_body_expressions: BTreeSet::new(),
    };
    closure.validate_selection(&selected).unwrap();
    let other = CheckedClosure::seal(Arc::clone(&topology), closure.owner(), |owner| {
        let module = project
            .modules()
            .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module))?;
        let HirExprKind::PostfixBracket(postfix) = module.resolve_expr(owner).ok()?.kind() else {
            return None;
        };
        let arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
            dialogue,
            ..
        } = postfix.candidates()
        else {
            return None;
        };
        Some(*dialogue)
    })
    .expect("the alternate capture projection is internally valid in its assumed context");
    assert_eq!(
        other.validate_selection(&selected),
        Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)
    );
    closure
        .validate_selection(&selected)
        .expect("failed reconciliation does not mutate the accepted selection");
}
