use arcweft_dialogue::rich_text::DialogueHostEventKind;
use arcweft_lang_hir::expr::HirExprKind;

use crate::{
    checked_rich_text::{
        CheckedDialogueHostEvent, CheckedDialogueMark, CheckedDialogueToken, CheckedRichTextAction,
        PreparedCheckedDialogueToken, PreparedCheckedRichTextAction,
    },
    final_analysis::{
        CheckedExpressionResolution, CheckedStatementPayload, CheckedTriggerView,
        FinalSemanticAnalysis,
    },
    semantic_coordinate::{
        SemanticCoordinateIndex, StableCheckedDialogueMarkCoordinate, StableCheckedValueCoordinate,
    },
};

use super::{analyze, fixture};

fn dialogue_application_resolutions(
    report: &FinalSemanticAnalysis,
) -> Vec<&CheckedExpressionResolution> {
    report
        .expressions()
        .filter_map(|(_, expression)| match expression.resolution() {
            resolution @ CheckedExpressionResolution::DialogueApplication { .. } => {
                Some(resolution)
            }
            _ => None,
        })
        .collect()
}

fn marker_rows(
    report: &FinalSemanticAnalysis,
) -> Vec<(String, StableCheckedDialogueMarkCoordinate)> {
    dialogue_application_resolutions(report)
        .into_iter()
        .flat_map(|resolution| {
            let CheckedExpressionResolution::DialogueApplication { rich_text, .. } = resolution
            else {
                unreachable!("dialogue application resolution was filtered above")
            };
            rich_text
                .content()
                .tokens()
                .iter()
                .filter_map(|token| {
                    let CheckedDialogueToken::PointAction(CheckedRichTextAction::Marker(mark)) =
                        token
                    else {
                        return None;
                    };
                    Some((
                        mark.diagnostic_name().as_str().to_owned(),
                        mark.coordinate().clone(),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn marker_semantic_rows(report: &FinalSemanticAnalysis) -> Vec<(String, CheckedDialogueMark)> {
    dialogue_application_resolutions(report)
        .into_iter()
        .flat_map(|resolution| {
            let CheckedExpressionResolution::DialogueApplication { rich_text, .. } = resolution
            else {
                unreachable!("dialogue application resolution was filtered above")
            };
            rich_text
                .content()
                .tokens()
                .iter()
                .filter_map(|token| {
                    let CheckedDialogueToken::PointAction(CheckedRichTextAction::Marker(mark)) =
                        token
                    else {
                        return None;
                    };
                    Some((mark.diagnostic_name().as_str().to_owned(), mark.clone()))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn mark_trigger_rows(report: &FinalSemanticAnalysis) -> Vec<StableCheckedDialogueMarkCoordinate> {
    report
        .statements()
        .filter_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::Trigger(trigger) => match trigger.view() {
                CheckedTriggerView::Mark(coordinate) => Some(coordinate.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn one_mark_source(mark: &str, handler: &str) -> String {
    format!(
        "pub character alice {{}}\n\
         flow main() -> String {{\n\
             alice[before [mark @.{mark}] after] with {{ on mark(@.{handler}) => return \"done\" }}\n\
             return \"done\"\n\
         }}\n"
    )
}

fn two_mark_source(first: &str, second: &str, handler: &str) -> String {
    format!(
        "pub character alice {{}}\n\
         flow main() -> String {{\n\
             alice[before [mark @.{first}] middle [mark @.{second}] after] with {{ on mark(@.{handler}) => return \"done\" }}\n\
             return \"done\"\n\
         }}\n"
    )
}

#[test]
fn p06_p11_nested_line_plan_marks_are_checked_against_the_content_catalog() {
    let fixture = fixture(
        r#"
pub character alice {}
flow main() -> String {
    alice[before [mark @.outer] middle [mark @.inner] after] with {
        on mark(@.outer) => on mark(@.inner) => return "done"
    }
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("nested marker line plan is checked");
    let applications = dialogue_application_resolutions(&report);
    let [CheckedExpressionResolution::DialogueApplication { rich_text, .. }] =
        applications.as_slice()
    else {
        panic!("nested fixture publishes one dialogue application")
    };

    let markers = marker_rows(&report);
    assert_eq!(markers.len(), 2);
    assert_eq!(markers[0].1.ordinal().get(), 0);
    assert_eq!(markers[1].1.ordinal().get(), 1);
    assert!(rich_text.effect_plan().effect_sites().is_empty());
    assert!(rich_text.is_valid());

    let triggers = mark_trigger_rows(&report);
    assert_eq!(triggers.len(), 2);
    assert_eq!(
        triggers
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        markers
            .into_iter()
            .map(|(_, coordinate)| coordinate)
            .collect()
    );
}

#[test]
fn call_and_timed_cue_actions_keep_their_checked_payload_owners() {
    let fixture = fixture(
        r#"
pub character alice {}
fn callback() {}
flow main() -> String {
    alice[hello [call callback()] [at 120ms call=callback()]]
    return "done"
}
"#,
        None,
    );
    let executable = fixture.project.analysis_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let (_, expression) = module
        .expressions()
        .find(|(_, expression)| {
            matches!(expression.kind(), HirExprKind::AttachedContentApplication(application)
            if application.content().nodes().iter().any(|node| matches!(
                node.kind(),
                arcweft_lang_hir::dialogue_application::HirDialogueNodeKind::PointAction(_)
            )))
        })
        .expect("dialogue application expression");
    let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
        unreachable!("expression was filtered as an attached content application")
    };
    let checked =
        crate::checked_rich_text::RichTextContentChecker::check(module, application.content())
            .expect("dialogue content source roles");
    assert!(checked.report().is_valid());
    let actions =
        checked
            .report()
            .content()
            .tokens()
            .iter()
            .filter_map(|token| match token {
                PreparedCheckedDialogueToken::PointAction(
                    PreparedCheckedRichTextAction::Host { owner, action, .. },
                ) => Some((*owner, action)),
                _ => None,
            })
            .collect::<Vec<_>>();
    let [
        (
            call_owner,
            CheckedDialogueHostEvent::Call {
                call: call_expression,
            },
        ),
        (
            timed_owner,
            CheckedDialogueHostEvent::TimedCue {
                at,
                call: timed_expression,
            },
        ),
    ] = actions.as_slice()
    else {
        panic!("checked call and timed cue actions retain typed payloads: {actions:?}");
    };
    assert_eq!(*call_owner, DialogueHostEventKind::Call);
    assert_eq!(*timed_owner, DialogueHostEventKind::TimedCue);
    assert_eq!(at.millis, 120);
    assert_ne!(call_expression, timed_expression);
}

#[test]
fn dialogue_effect_sites_publish_exact_free_local_capture_schemas() {
    let fixture = fixture(
        r#"
pub character alice {}
flow main() -> String {
    let message = "captured";
    alice[hello [call log.info("literal")] [at 120ms call=log.info(message)]]
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("dialogue effect captures are checked");
    let applications = dialogue_application_resolutions(&report);
    let [CheckedExpressionResolution::DialogueApplication { rich_text, .. }] =
        applications.as_slice()
    else {
        panic!("capture fixture publishes one dialogue application")
    };
    let [literal, captured] = rich_text.effect_plan().effect_sites() else {
        panic!("capture fixture publishes two checked effect sites")
    };
    for site in [literal, captured] {
        assert_eq!(
            site.effects().to_labels(),
            vec!["log.write"],
            "the callback retains the selected application's exact closed row",
        );
    }
    assert!(literal.captures().is_empty());
    let [capture] = captured.captures() else {
        panic!("local-backed effect publishes one capture")
    };
    assert_eq!(capture.ty(), &crate::types::TypeKind::String);
    let StableCheckedValueCoordinate::Expression(application) =
        captured.effect().application().coordinate()
    else {
        panic!("effect application is expression-owned")
    };
    assert_eq!(capture.origin().root(), application.root());
    assert_eq!(
        report
            .local(capture.local())
            .expect("capture local remains in the checked binding catalog")
            .ty(),
        capture.ty()
    );
}

#[test]
fn p12_equal_local_mark_names_in_two_applications_have_distinct_coordinates() {
    let fixture = fixture(
        r#"
pub character alice {}
flow main() -> Unit {
    alice[one [mark @.same]]
    alice[two [mark @.same]]
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("two dialogue applications are checked");
    let markers = marker_rows(&report);
    assert_eq!(markers.len(), 2);
    assert_eq!(markers[0].0, "same");
    assert_eq!(markers[1].0, "same");
    assert_eq!(markers[0].1.ordinal().get(), 0);
    assert_eq!(markers[1].1.ordinal().get(), 0);
    assert_ne!(markers[0].1, markers[1].1);
    assert_ne!(
        markers[0].1.application(),
        markers[1].1.application(),
        "accepted application path, not local spelling, separates marks"
    );
}

#[test]
fn p14_coordinated_mark_rename_preserves_checked_semantics_but_changes_diagnostics() {
    let first_fixture = fixture(&one_mark_source("first", "first"), None);
    let renamed_fixture = fixture(&one_mark_source("other", "other"), None);
    let first = analyze(&first_fixture).expect("original mark source is checked");
    let renamed = analyze(&renamed_fixture).expect("coordinated mark rename is checked");

    let first_resolution = dialogue_application_resolutions(&first);
    let renamed_resolution = dialogue_application_resolutions(&renamed);
    assert_eq!(first_resolution.len(), 1);
    assert_eq!(renamed_resolution.len(), 1);

    let first_markers = marker_rows(&first);
    let renamed_markers = marker_rows(&renamed);
    assert_eq!(first_markers[0].1, renamed_markers[0].1);
    assert_eq!(mark_trigger_rows(&first), mark_trigger_rows(&renamed));
    assert_ne!(first_markers[0].0, renamed_markers[0].0);

    let first_semantic = marker_semantic_rows(&first);
    let renamed_semantic = marker_semantic_rows(&renamed);
    assert_eq!(first_semantic[0].1, renamed_semantic[0].1);
    assert_ne!(first_semantic[0].0, renamed_semantic[0].0);
}

#[test]
fn p15_mark_reorder_and_trigger_reference_swap_change_checked_coordinates() {
    let ordered = fixture(&two_mark_source("first", "second", "first"), None);
    let reordered = fixture(&two_mark_source("second", "first", "first"), None);
    let ordered = analyze(&ordered).expect("ordered marks are checked");
    let reordered = analyze(&reordered).expect("reordered marks are checked");

    let ordered_markers = marker_rows(&ordered);
    let reordered_markers = marker_rows(&reordered);
    let ordered_first = ordered_markers
        .iter()
        .find(|(name, _)| name == "first")
        .expect("ordered first marker");
    let reordered_first = reordered_markers
        .iter()
        .find(|(name, _)| name == "first")
        .expect("reordered first marker");
    assert_ne!(ordered_first.1, reordered_first.1);
    assert_ne!(mark_trigger_rows(&ordered), mark_trigger_rows(&reordered));

    let same_content_first_reference = fixture(&two_mark_source("first", "second", "first"), None);
    let same_content_second_reference =
        fixture(&two_mark_source("first", "second", "second"), None);
    let first_reference =
        analyze(&same_content_first_reference).expect("first reference is checked");
    let second_reference =
        analyze(&same_content_second_reference).expect("second reference is checked");
    assert_ne!(
        mark_trigger_rows(&first_reference),
        mark_trigger_rows(&second_reference)
    );
}

#[test]
fn n17_coordinate_issuer_rejects_a_stale_hir_generation() {
    let first_fixture = fixture(&one_mark_source("first", "first"), None);
    let first_report = analyze(&first_fixture).expect("first generation is checked");
    let first_project = first_fixture
        .project
        .analysis_view()
        .expect("first project");
    let first_module = first_project
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .expect("first root module");
    let mark = first_module
        .expressions()
        .find_map(|(_, expression)| match expression.kind() {
            HirExprKind::AttachedContentApplication(application) => {
                let arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
                    target: _,
                    plan: _,
                    coordinates: _,
                } = application.family()
                else {
                    return None;
                };
                application.content().marks().first().map(|mark| mark.id())
            }
            _ => None,
        })
        .expect("first generation mark");
    let index = SemanticCoordinateIndex::new(first_report.accepted_root_catalog(), &first_report);

    let stale_fixture = fixture(&one_mark_source("first", "first"), None);
    let stale_project = stale_fixture
        .project
        .analysis_view()
        .expect("stale project");
    assert!(index.dialogue_mark(stale_project, mark).is_err());
}

#[test]
fn p06_exact_mark_catalog_publishes_all_rows() {
    let accepted_fixture = fixture(
        r#"
pub character alice {}
flow main() -> String {
    alice[zero [mark @.zero] one [mark @.one] two [mark @.two]]
    return "done"
}
"#,
        None,
    );
    let report = analyze(&accepted_fixture).expect("exact marker catalog is checked");
    let markers = marker_rows(&report);
    assert_eq!(
        markers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["zero", "one", "two"]
    );
    assert_eq!(
        markers
            .iter()
            .map(|(_, coordinate)| coordinate.ordinal().get())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}
