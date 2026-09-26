use super::*;
use crate::final_analysis::{CheckedCaptureAuthorityViolation, CheckedClosure};

#[test]
fn dialogue_semantic_id_preserves_checked_call_edges() {
    let fixture = crate::final_analysis::tests::fixture(
        r#"
pub character alice {}
flow opening() -> String {
    alice(id=@say.story.greeting, text_key=@text.story.greeting)[hello[p]]
    return "ok"
}
"#,
        None,
    );
    let report = crate::final_analysis::tests::analyze(&fixture).unwrap();
    let (owner, application) = report
        .expressions()
        .find_map(|(owner, _)| {
            let application = report.call(owner)?.selected_application()?;
            application.core().execution().semantic_operands().iter().any(|operand| {
                matches!(
                    operand.source(),
                    crate::callable::CheckedCallSemanticOperandSource::DialogueApplicationId { .. }
                )
            }).then_some((owner, application))
        })
        .expect("dialogue id is retained as a checked semantic operand");
    let semantic = application
        .core()
        .execution()
        .semantic_operands()
        .iter()
        .filter_map(|operand| match operand.source() {
            crate::callable::CheckedCallSemanticOperandSource::DialogueApplicationId {
                argument,
                source,
                ..
            }
            | crate::callable::CheckedCallSemanticOperandSource::DialogueApplicationTextKey {
                argument,
                source,
                ..
            } => Some((u32::from(argument.get()), *source)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(semantic.len(), 2);
    let edges = report
        .checked_child_edges(owner)
        .expect("semantic-only id and text_key have exact checked argument edges");
    for (ordinal, source) in semantic {
        assert!(
            application.core().execution().arguments()[usize::try_from(ordinal).unwrap()]
                .slots()
                .is_empty()
        );
        assert!(edges.iter().any(|edge| {
            edge.child() == source
                && matches!(edge.role(), CheckedExpressionChildRole::Argument { ordinal: actual } if *actual == ordinal)
        }));
    }
    report
        .checked_callable_join(owner)
        .expect("callable join remains available to the compiler");
}

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
        declaration_only_trait_receiver_owners: BTreeSet::new(),
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
