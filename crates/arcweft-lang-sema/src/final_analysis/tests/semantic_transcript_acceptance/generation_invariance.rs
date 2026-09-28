use super::*;

struct CallTranscriptObservation {
    semantic_digest: [u8; 32],
    runtime_applications: Vec<[u8; 32]>,
    fx_applications: Vec<[u8; 32]>,
    content_applications: usize,
    view_fx_applications: usize,
    evaluated_effect_statements: usize,
    dialogue_call_triggers: Vec<crate::final_analysis::CheckedDialogueEffectTrigger>,
}

fn observe_calls_below_match(source: &str) -> CallTranscriptObservation {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("checked Match call fixture");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let matches = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    let [match_owner] = matches.as_slice() else {
        panic!("fixture owns exactly one Match expression");
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, *match_owner);
    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let match_path = coordinates
        .expression(*match_owner)
        .expect("accepted Match path");
    let runtime_applications = report
        .calls()
        .filter_map(|(owner, facts)| {
            let coordinate = coordinates.expression(owner).ok()?;
            coordinate
                .is_at_or_below(&match_path)
                .then(|| {
                    facts
                        .selected_application()
                        .map(|application| *application.digest().as_bytes())
                })
                .flatten()
        })
        .collect();
    let (content_applications, view_fx_applications) =
        report
            .expressions()
            .fold((0, 0), |(content, view_fx), (owner, checked)| {
                let below_match = coordinates
                    .expression(owner)
                    .is_ok_and(|path| path.is_at_or_below(&match_path));
                (
                    content
                        + usize::from(
                            below_match
                                && matches!(
                                    checked.resolution(),
                                    CheckedExpressionResolution::ContentApplication(_)
                                ),
                        ),
                    view_fx
                        + usize::from(
                            below_match
                                && matches!(
                                    checked.resolution(),
                                    CheckedExpressionResolution::ViewFxApplication(_)
                                ),
                        ),
                )
            });
    let mut fx_applications = Vec::new();
    let mut dialogue_call_triggers = Vec::new();
    for (owner, checked) in report.expressions() {
        if !coordinates
            .expression(owner)
            .is_ok_and(|path| path.is_at_or_below(&match_path))
        {
            continue;
        }
        match checked.resolution() {
            CheckedExpressionResolution::ViewFxApplication(application) => {
                fx_applications.push(*application.semantic_digest().as_bytes());
            }
            CheckedExpressionResolution::DialogueApplication { rich_text, .. } => {
                for site in rich_text.effect_plan().effect_sites() {
                    if matches!(
                        site.operation(),
                        crate::final_analysis::CheckedDialogueEffectOperation::Call { .. }
                    ) {
                        dialogue_call_triggers.push(site.trigger().clone());
                    }
                }
                for token in rich_text.content().tokens() {
                    if let crate::checked_rich_text::CheckedDialogueToken::ContentInsert(insertion) =
                        token
                        && let crate::checked_rich_text::CheckedContentEmission::Fx(application) =
                            insertion.emission()
                    {
                        fx_applications.push(*application.semantic_digest().as_bytes());
                    }
                }
            }
            _ => {}
        }
    }
    let evaluated_effect_statements = report
        .statements()
        .filter(|(owner, checked)| {
            coordinates
                .statement(*owner)
                .is_ok_and(|coordinate| coordinate.path().is_at_or_below(&match_path))
                && matches!(
                    checked.payload(),
                    CheckedStatementPayload::EvaluatedEffect(_)
                )
        })
        .count();
    CallTranscriptObservation {
        semantic_digest: *product.semantic_digest().as_bytes(),
        runtime_applications,
        fx_applications,
        content_applications,
        view_fx_applications,
        evaluated_effect_statements,
        dialogue_call_triggers,
    }
}

fn evaluated_effect_match_source(suffix: &str, message: &str) -> String {
    format!(
        "flow root(flag: bool) {{\n    let selected = match flag {{\n        true => {{\n            log.info(\"{message}\")\n            1i64\n        }}\n        false => 0i64\n    }}\n}}\n{suffix}"
    )
}

fn dialogue_call_match_source(suffix: &str, callable: &str) -> String {
    format!(
        "pub character alice {{}}\nfn project_action() {{}}\nfn alternate_action() {{}}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{\n            alice[before [call {callable}()] [at 120ms call={callable}()]];\n            1i64\n        }}\n        false => 0i64\n    }}\n}}\n{suffix}"
    )
}

fn content_call_match_source(suffix: &str, modifier: &str) -> String {
    format!(
        "pub character alice {{}}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{\n            alice[before #{modifier}()[text]];\n            1i64\n        }}\n        false => 0i64\n    }}\n}}\n{suffix}"
    )
}

fn project_content_call_match_source(suffix: &str, callable: &str) -> String {
    format!(
        "pub character alice {{}}\nfn passthrough()[body: DialogueContent] -> DialogueContent {{ body }}\nfn alternate()[body: DialogueContent] -> DialogueContent {{ body }}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{\n            alice[before #{callable}()[nested]];\n            1i64\n        }}\n        false => 0i64\n    }}\n}}\n{suffix}"
    )
}

fn project_fx_match_source(suffix: &str, accent: &str) -> String {
    format!(
        "pub character alice {{}}\n#[fx]\nfn emphasis(accent: Color = rgb(\"#ffd060\")) -> Fx {{\n    Fx.text(color = accent)\n}}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{\n            alice[#fx(emphasis(accent=rgb(\"{accent}\")))[text]];\n            1i64\n        }}\n        false => 0i64\n    }}\n}}\n{suffix}"
    )
}

fn view_fx_match_source(suffix: &str, producer: &str) -> String {
    format!(
        "view Main(speed: f32) {{\n    match true {{\n        true => Button().fx({producer})\n        false => Button()\n    }}\n}}\n{suffix}"
    )
}

#[test]
fn evaluated_effect_statement_match_digest_ignores_unrelated_source_revision() {
    let first = observe_calls_below_match(&evaluated_effect_match_source("", "started"));
    let unrelated = observe_calls_below_match(&evaluated_effect_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "started",
    ));
    let changed = observe_calls_below_match(&evaluated_effect_match_source("", "stopped"));
    assert_eq!(first.runtime_applications.len(), 1);
    assert_eq!(unrelated.runtime_applications.len(), 1);
    assert_eq!(first.evaluated_effect_statements, 1);
    assert_ne!(first.runtime_applications, unrelated.runtime_applications);
    assert_eq!(first.semantic_digest, unrelated.semantic_digest);
    assert_ne!(first.semantic_digest, changed.semantic_digest);
}

#[test]
fn dialogue_effect_call_match_digest_ignores_unrelated_source_revision() {
    let first = observe_calls_below_match(&dialogue_call_match_source("", "project_action"));
    let unrelated = observe_calls_below_match(&dialogue_call_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "project_action",
    ));
    let changed = observe_calls_below_match(&dialogue_call_match_source("", "alternate_action"));
    assert_eq!(first.runtime_applications.len(), 3);
    assert_eq!(first.dialogue_call_triggers.len(), 2);
    assert!(matches!(
        first.dialogue_call_triggers[0],
        crate::final_analysis::CheckedDialogueEffectTrigger::Content
    ));
    assert!(matches!(
        first.dialogue_call_triggers[1],
        crate::final_analysis::CheckedDialogueEffectTrigger::Delay(_)
    ));
    assert_ne!(first.runtime_applications, unrelated.runtime_applications);
    assert_eq!(first.semantic_digest, unrelated.semantic_digest);
    assert_ne!(first.semantic_digest, changed.semantic_digest);
}

#[test]
fn content_call_match_digest_ignores_unrelated_source_revision() {
    let first = observe_calls_below_match(&content_call_match_source("", "strong"));
    let unrelated = observe_calls_below_match(&content_call_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "strong",
    ));
    let changed = observe_calls_below_match(&content_call_match_source("", "em"));
    assert_eq!(first.content_applications, 1);
    assert_eq!(first.runtime_applications, unrelated.runtime_applications);
    assert_eq!(first.semantic_digest, unrelated.semantic_digest);
    assert_ne!(first.semantic_digest, changed.semantic_digest);
}

#[test]
fn project_content_call_match_digest_ignores_unrelated_source_revision() {
    let first = observe_calls_below_match(&project_content_call_match_source("", "passthrough"));
    let unrelated = observe_calls_below_match(&project_content_call_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "passthrough",
    ));
    let changed = observe_calls_below_match(&project_content_call_match_source("", "alternate"));
    assert_eq!(first.content_applications, 1);
    assert_ne!(first.runtime_applications, unrelated.runtime_applications);
    assert_eq!(first.semantic_digest, unrelated.semantic_digest);
    assert_ne!(first.semantic_digest, changed.semantic_digest);
}

#[test]
fn project_fx_content_match_digest_ignores_unrelated_source_revision() {
    let first = observe_calls_below_match(&project_fx_match_source("", "#ff6b8a"));
    let unrelated = observe_calls_below_match(&project_fx_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "#ff6b8a",
    ));
    let changed = observe_calls_below_match(&project_fx_match_source("", "#ffd060"));
    assert_eq!(first.content_applications, 1);
    assert!(first.runtime_applications.len() >= 2);
    assert_ne!(first.runtime_applications, unrelated.runtime_applications);
    assert_eq!(first.fx_applications.len(), 1);
    assert_eq!(first.fx_applications, unrelated.fx_applications);
    assert_ne!(first.fx_applications, changed.fx_applications);
    assert_eq!(first.semantic_digest, unrelated.semantic_digest);
    assert_ne!(first.semantic_digest, changed.semantic_digest);
}

#[test]
fn view_fx_match_digest_ignores_unrelated_source_revision() {
    let first = observe_calls_below_match(&view_fx_match_source("", "wave(speed = speed)"));
    let unrelated = observe_calls_below_match(&view_fx_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "wave(speed = speed)",
    ));
    let changed = observe_calls_below_match(&view_fx_match_source("", "wave(speed = 1.0)"));
    assert_eq!(first.view_fx_applications, 1);
    assert!(first.runtime_applications.len() >= 2);
    assert_ne!(first.runtime_applications, unrelated.runtime_applications);
    assert_eq!(first.fx_applications.len(), 1);
    assert_eq!(first.fx_applications, unrelated.fx_applications);
    assert_ne!(first.fx_applications, changed.fx_applications);
    assert_eq!(first.semantic_digest, unrelated.semantic_digest);
    assert_ne!(first.semantic_digest, changed.semantic_digest);
}
