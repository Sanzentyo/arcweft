use super::*;

#[test]
fn out_tail_in_init_stays_in_final_hir_for_semantic_rejection() {
    let source = concat!(
        "pub character alice { display = \"Alice\" }\n",
        "flow row() -> Unit {\n",
        "    alice()[本文。[p]] with {\n",
        "        init { out (); log.info(\"unreachable\") }\n",
        "    }\n",
        "    return ()\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/dialogue-init-out-tail", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:#?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, _, flow) = resolve_flow(&module, 1);
    assert_eq!(
        module.status(),
        HirModuleStatus::Clean,
        "diagnostics={:#?}; recovered={:?}",
        module.diagnostics(),
        module.recovered_owners().collect::<Vec<_>>(),
    );

    let HirThreadFlowItem::DialogueApplication(dialogue) = flow.body().items()[0] else {
        panic!("the Flow retains its Dialogue line")
    };
    let HirExprKind::AttachedContentApplication(application) =
        module.resolve_expr(dialogue).unwrap().kind()
    else {
        panic!("the Flow Dialogue line remains typed")
    };
    let crate::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
        plan: Some(plan),
        ..
    } = application.family()
    else {
        panic!("the Dialogue line retains its plan")
    };
    let [HirLinePlanItem::Init { scope, statements }] = plan.items() else {
        panic!("the plan retains its direct Init item")
    };
    assert_eq!(statements.len(), 2);
    assert!(statements.iter().all(|statement| {
        module
            .resolve_stmt(*statement)
            .is_ok_and(|statement| statement.scope() == *scope)
    }));
}
