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
