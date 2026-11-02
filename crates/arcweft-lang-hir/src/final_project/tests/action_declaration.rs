use super::*;

#[test]
fn retained_action_signature_has_a_complete_evaluation_topology() {
    for declaration in [
        "pub action submit()",
        "pub action submit(value: String)",
        "pub action @action:.feedback.submit submit(value: String, count: i64)",
        "pub action submit(value: String)\npub action cancel(value: String)",
    ] {
        assert_action_signature_topology(declaration);
    }
}

fn assert_action_signature_topology(declaration: &str) {
    let package = package();
    let path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/action-signature",
        "action-signature.arcw",
        &format!("{declaration}\nflow main() -> i64 {{ return 0 }}\n"),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &path);
    assert!(
        module.diagnostics().is_empty(),
        "{:?}",
        module.diagnostics()
    );
    let retained = Arc::clone(&module);
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &path, module)],
    )
    .unwrap();
    let symbols = symbols_for_project(&project, parsed.document(), "action-signature");
    let topology = evaluation_topology(&project, &symbols);
    let module = topology.module(retained.module_id()).unwrap();
    let mut actions = 0;
    for entry in module.entries() {
        let item = retained.resolve_item(entry.item()).unwrap();
        let HirItemKind::Action(action) = item.kind() else {
            assert_eq!(entry.parameter_roots(), &[]);
            continue;
        };
        actions += 1;
        assert!(
            entry.body().is_none(),
            "an action is not an executable callable"
        );
        assert_eq!(entry.parameter_roots().len(), action.parameters().len());
        for (index, (root, parameter)) in entry
            .parameter_roots()
            .iter()
            .zip(action.parameters())
            .enumerate()
        {
            let index = u32::try_from(index).unwrap();
            let role = HirDeclarationParameterRootRole::Pattern {
                group: 0,
                parameter: index,
            };
            assert_eq!(root.role(), role);
            assert_eq!(
                root.child(),
                super::super::HirDeclarationParameterRootChild::Pattern(parameter.pattern())
            );
            assert_eq!(
                entry.paths().pattern(parameter.pattern()).unwrap().steps(),
                &[HirSemanticPathStep::ItemParameter(role)]
            );
            for local in parameter.locals() {
                assert_eq!(
                    retained.resolve_local(*local).unwrap().scope(),
                    action.callable_scope()
                );
                assert!(entry.paths().local(*local).is_some());
                let origin = module.local_origins().binding(*local).unwrap();
                assert_eq!(
                    origin.site(),
                    super::super::HirBindingSite::ItemParameter {
                        item: entry.item(),
                        group: 0,
                        parameter: index
                    }
                );
                assert_eq!(origin.pattern(), Some(parameter.pattern()));
            }
        }
    }
    assert!(actions > 0);
    assert_eq!(
        module.local_origins().rows().count(),
        retained.locals().len()
    );
}
