use super::*;

#[test]
fn flow_effect_contract_preserves_typed_lifetime_scopes() {
    for scope in [
        "'frame",
        "'tick",
        "'cue",
        "'line",
        "'scene",
        "'flow",
        "'session",
        "'global",
        "'persistent",
        "'inventory",
        "'flow.flags",
    ] {
        let effect = format!("state.write({scope})");
        let source = format!("flow main() effects {{ {effect} }} {{}}");
        let fixture = fixture(&source, None);
        let report = analyze(&fixture).expect("an unused scoped permission is still a contract");
        let flows = report
            .items()
            .filter(|(_, item)| matches!(item.role(), CheckedItemRole::Flow { .. }))
            .collect::<Vec<_>>();
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].1.effects().to_labels(), [effect.as_str()]);
    }
}

#[test]
fn function_effect_contract_keeps_lifetime_scope_at_its_call() {
    let fixture = fixture(
        "fn scoped() effects { state.write('flow) } {}\nflow main() { scoped() }",
        None,
    );
    let report = analyze(&fixture).expect("typed lifetime scope in a function interface");
    let function = checked_function_facts(&report, &fixture, "scoped");
    assert_eq!(
        function
            .exposed_row()
            .closed_value()
            .expect("closed authored permission")
            .to_labels(),
        ["state.write('flow)"]
    );
    let (_, flow) = report
        .items()
        .find(|(_, item)| matches!(item.role(), CheckedItemRole::Flow { .. }))
        .expect("calling flow");
    assert_eq!(flow.effects().to_labels(), ["state.write('flow)"]);
}

#[test]
fn lifetime_scoped_permission_does_not_grant_unrelated_effects() {
    let fixture = fixture(
        "flow main() effects { state.write('flow) } { log.info(\"body\") }",
        None,
    );
    let error = analyze(&fixture).expect_err("state permission does not grant log.write");
    let FinalSemanticAnalysisError::EffectUpperBoundExceeded { missing, .. } = error else {
        panic!("expected the exact missing permission, received {error:?}");
    };
    assert_eq!(missing.to_labels(), ["log.write"]);
}

#[test]
fn external_callable_keeps_its_typed_lifetime_permission() {
    let fixture = fixture(
        "extern capability store { fn touch() -> Unit effects { state.write('flow) } }\n\
         flow main() effects { state.write('flow) } { store.touch() }",
        None,
    );
    let report = analyze(&fixture).expect("the registered callable shares the effect projection");
    let (_, flow) = report
        .items()
        .find(|(_, item)| matches!(item.role(), CheckedItemRole::Flow { .. }))
        .expect("calling flow");
    assert_eq!(flow.effects().to_labels(), ["state.write('flow)"]);
}

#[test]
fn effect_scope_rejects_optional_registry_reads() {
    let fixture = fixture("flow main() effects { state.write('flow.flags?) } {}", None);
    let project = fixture.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let root = module
        .items()
        .flat_map(|(_, item)| item.kind().effect_expression_roots())
        .next()
        .expect("authored effect");
    assert!(matches!(
        crate::effects::project_hir_effect_id(module, root),
        Err(crate::effects::HirEffectProjectionError::Unsupported { .. })
    ));
    analyze(&fixture).expect_err("optional registry reads cannot become permissions");
}

#[test]
fn dialogue_line_owned_phases_check_the_flow_registry_effect_bound() {
    let (document, catalog, _) = super::akane_character_registration();
    let write = "'flow.flags.seen <- true";
    let phases = [
        ("init", format!("init {{ {write} }}")),
        ("mark", format!("on mark(@.write_here) {{ {write} }}")),
        (
            "cancellation",
            format!("cancel on input(.SkipLine) {{ {write}; out \"Skipped\" }}"),
        ),
        ("thread", format!("thread {{ {write} }}")),
        (
            "defer completed",
            format!("defer on completed {{ {write}; log.info(\"completed\") }}"),
        ),
        (
            "defer cancelled",
            format!("defer on cancelled {{ {write}; log.info(\"cancelled\") }}"),
        ),
        (
            "defer always",
            format!("defer {{ {write}; log.info(\"cleanup\") }}"),
        ),
    ];
    for (phase, action) in phases {
        for (permission, authorized) in [("flags.seen", true), ("other", false)] {
            let source = format!(
                r#"
pub character akane {{}}
flow main() -> String effects {{ state.write('flow.{permission}), control.spawn, log.write }} {{
    akane(voice=auto):
        本文。[mark @.write_here][p]
    with:
        {action}
        out "Completed"
    return "done"
}}
"#,
            );
            let fixture = super::fixture_with_all_registration_inputs_and_base(
                &source,
                None,
                Vec::new(),
                vec![(document.clone(), catalog.clone())],
                Vec::new(),
                super::TypeCheckEnv::standard(),
            );
            let result = analyze(&fixture);
            if authorized {
                result.unwrap_or_else(|error| panic!("{phase}: {error:?}"));
            } else {
                let error = result.expect_err("line cannot exceed the source Flow effect bound");
                let FinalSemanticAnalysisError::EffectUpperBoundExceeded { missing, .. } = error
                else {
                    panic!("{phase}: {error:?}");
                };
                assert_eq!(missing.to_labels(), ["state.write('flow.flags.seen)"]);
            }
        }
    }
}

#[test]
fn dialogue_line_checks_selected_call_effects_under_the_enclosing_bound() {
    let (document, catalog, _) = super::akane_character_registration();
    for (phase, action) in [
        ("init", "init { write_flag() }"),
        ("mark", "on mark(@.write_here) { write_flag() }"),
    ] {
        for (permission, authorized) in [("flags.seen", true), ("other", false)] {
            let source = format!(
                r#"
pub character akane {{}}
fn write_flag() -> Unit {{
    'flow.flags.seen <- true;
    return ()
}}
flow main() -> String effects {{ state.write('flow.{permission}) }} {{
    akane()[本文。[mark @.write_here][p]] with {{
        {action}
        out "Completed"
    }}
    return "done"
}}
"#,
            );
            let fixture = super::fixture_with_all_registration_inputs_and_base(
                &source,
                None,
                Vec::new(),
                vec![(document.clone(), catalog.clone())],
                Vec::new(),
                super::TypeCheckEnv::standard(),
            );
            let result = analyze(&fixture);
            if authorized {
                result.unwrap_or_else(|error| panic!("{phase}: {error:?}"));
            } else {
                let error = result.expect_err("selected helper exceeds the source Flow bound");
                let FinalSemanticAnalysisError::EffectUpperBoundExceeded { missing, .. } = error
                else {
                    panic!("{phase}: {error:?}");
                };
                assert_eq!(missing.to_labels(), ["state.write('flow.flags.seen)"]);
            }
        }
    }
}
