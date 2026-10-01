use super::*;
use crate::{callable::CheckedCallableExecution, types::CompileTimeCallableType};
use arcweft_lang_hir::symbol::CallableDeclarationOwner;

fn assert_view_calls(world: &Fixture, expected: usize) {
    let report = analyze(world).unwrap_or_else(|error| {
        panic!(
            "View callable fixture: {error:?} source: {}",
            world.root_document.text()
        )
    });
    let calls = report
        .calls()
        .filter_map(|(owner, facts)| {
            let application = facts.selected_application()?;
            let candidate = application.core().candidates().selected();
            let CallableCandidateId::Project(declaration) = candidate.id() else {
                return None;
            };
            (declaration.owner() == CallableDeclarationOwner::View).then_some((
                owner,
                application,
                declaration,
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), expected);
    for (owner, application, declaration) in calls {
        assert_eq!(
            report
                .expression(owner)
                .expect("call expression")
                .value_type(),
            Some(&TypeKind::ViewValue)
        );
        let checked = report
            .checked_callables()
            .project_callable(declaration)
            .expect("exact View record");
        assert_eq!(checked.execution(), &CheckedCallableExecution::RetainedView);
        assert!(checked.ordinary_function_emission().is_none());
        assert_eq!(
            application.core().candidates().selected().id(),
            checked.record().id()
        );
    }
    let classifier = RuntimeProducerArgumentClassifier::try_new(&report, &world.registered)
        .expect("registered ownership authority");
    let tokens = report
        .expressions()
        .filter(|(_, expression)| {
            matches!(
                expression.value_type(),
                Some(TypeKind::CompileTimeCallable(
                    CompileTimeCallableType::ProjectView(_)
                ))
            )
        })
        .inspect(|(_, expression)| {
            assert_eq!(
                classifier
                    .classify(expression.value_type().expect("typed View token"))
                    .expect_err("semantic callee has no Core snapshot")
                    .rejection(),
                Some(RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner)
            );
        })
        .count();
    assert!(
        tokens >= expected,
        "View callees retain semantic token facts"
    );
}

#[test]
fn retained_view_calls_use_selected_catalog_binding_for_direct_and_alias_heads() {
    for head in [
        "Child(\"hello\")",
        "Child(repeat = true, value = \"hello\")",
        "{ let saved = Child; saved(\"hello\") }",
        "{ let saved = Child; { saved }(\"hello\") }",
    ] {
        let source = format!(
            "view Child(value: String, repeat: bool = false) {{ Text(value) }}\nview Main() {{ {head} }}\n"
        );
        assert_view_calls(&fixture(&source, None), 1);
    }
}

#[test]
fn retained_view_calls_follow_imports_qualified_paths_and_builtin_shadowing() {
    assert_view_calls(
        &fixture(
            "use crate.child.Child as Imported\nview Main() { Imported(\"hello\"); crate.child.Child(\"world\") }\n",
            Some("pub view Child(value: String) { Text(value) }\n"),
        ),
        2,
    );
    assert_view_calls(
        &fixture(
            "view Text(value: bool) { Panel() }\nview Main() { Text(true) }\n",
            None,
        ),
        1,
    );
    assert_view_calls(
        &fixture(
            "view Child(value: String) { Text(value) }\nview Main() { { let Text = Child; Text(\"hello\") } }\n",
            None,
        ),
        1,
    );
}

#[test]
fn retained_view_calls_reject_invalid_supply_without_selected_application() {
    for head in [
        "Child()",
        "Child(true)",
        "Child(unknown = \"hello\")",
        "Child(\"a\", \"b\")",
    ] {
        let source =
            format!("view Child(value: String) {{ Text(value) }}\nview Main() {{ {head} }}\n");
        let world = fixture(&source, None);
        match analyze(&world) {
            Err(_) => {}
            Ok(report) => assert!(
                report
                    .calls()
                    .all(|(_, facts)| facts.selected_application().is_none())
            ),
        }
    }
}

#[test]
fn retained_view_calls_preserve_supplied_order_default_omission_and_spread_projection() {
    for (head, destinations) in [
        ("Child(second = 2i64, first = 1i64)", vec![1, 0]),
        ("Child(1i64)", vec![0]),
        ("Child([1i64, 2i64]...)", vec![0, 1]),
    ] {
        let source = format!(
            "view Child(first: i64, second: i64 = 0i64) {{ Text(first) }}\nview Main() {{ {head} }}\n"
        );
        let world = fixture(&source, None);
        let report = analyze(&world).expect("View argument projection");
        let application = report
            .calls()
            .find_map(|(_, facts)| facts.selected_application())
            .expect("selected View call");
        let arguments = application.core().execution().arguments();
        for (ordinal, argument) in arguments.iter().enumerate() {
            assert_eq!(usize::from(argument.argument().get()), ordinal);
        }
        let slots = arguments
            .iter()
            .flat_map(|argument| argument.slots())
            .collect::<Vec<_>>();
        let actual = slots
            .iter()
            .map(|slot| {
                let crate::callable::CheckedCallOperandDestination::Parameter(parameter) =
                    slot.destination()
                else {
                    panic!("declared View parameter");
                };
                assert_eq!(parameter.group(), CallableGroupIndex::ZERO);
                assert_eq!(slot.inferred(), &TypeKind::I64);
                assert_eq!(slot.expected(), Some(&TypeKind::I64));
                parameter.parameter().get()
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, destinations);
        if head.contains("...") {
            assert_eq!(arguments.len(), 1);
            for (ordinal, slot) in slots.iter().enumerate() {
                assert!(
                    matches!(slot.source().raw(), CheckedCallArgumentSlotSource::CompactNumericElement { ordinal: found, .. } if found as usize == ordinal)
                );
            }
        }
    }
}

#[test]
fn retained_view_call_transcript_tracks_selected_identity_and_ignores_source_allocation() {
    let source = |target: &str| {
        format!(
            "view Child(value: String) {{ Text(value) }}\nview Other(value: String) {{ Text(value) }}\nview Main(flag: bool) {{\n    match flag {{\n        true => {target}(\"hello\")\n        false => Child(\"world\")\n    }}\n}}\n"
        )
    };
    let first = source("Child");
    let digest = source_match_digest(&first);
    assert_ne!(digest, source_match_digest(&source("Other")));
    assert_eq!(
        digest,
        source_match_digest(&format!("fn unrelated() -> i64 {{ 9i64 }}\n{first}"))
    );
    assert_eq!(
        digest,
        source_match_digest(&first.replace("Child(\"hello\")", "Child ( \"hello\" )"))
    );
}
