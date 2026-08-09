use super::*;

use crate::item::HirItemFamily;

fn assert_clean_current_callable(item: &HirItem, expected_family: HirItemFamily) {
    assert_eq!(item.family(), expected_family);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);

    let name = match item.kind() {
        HirItemKind::Function(function) => function.name(),
        HirItemKind::Predicate(predicate) => predicate.name(),
        HirItemKind::Proof(proof) => proof.name(),
        other => panic!("expected a current callable, got {other:?}"),
    };
    assert_eq!(
        name.resolved().map(crate::leaf::HirName::as_str),
        Some("next")
    );
}

#[test]
fn ordinary_removed_form_recovery_keeps_following_final_hir_queryable_until_repaired() {
    let removed_forms = [
        "borrow resource as view: View { view }",
        "trusted axiom @axiom.legacy",
        "invariant true",
        "calc { 1 == 1 }",
    ];
    let following_declarations = [
        ("fn next() {}\n", HirItemFamily::Function),
        ("predicate next() = true\n", HirItemFamily::Predicate),
        ("proof next() = ()\n", HirItemFamily::Proof),
    ];

    for (removed_ordinal, removed_form) in removed_forms.into_iter().enumerate() {
        for (following_ordinal, (following, expected_family)) in
            following_declarations.into_iter().enumerate()
        {
            let removed_prefix = format!("{removed_form}\n");
            let initial_source = format!("{removed_prefix}{following}");
            let name = SourceName::path("proof/removed-form-recovery.arcw");
            let document_id = format!(
                "arcweft-test://proof/removed-form-recovery/{removed_ordinal}/{following_ordinal}"
            );
            let mut syntax = SyntaxDatabase::try_new().unwrap();
            let initial = syntax
                .parse_initial(
                    SourceSnapshotId::initial(name.clone()),
                    source_document(&document_id, &name, &initial_source),
                    arcweft_lang_syntax::parser::ParseOptions::default(),
                )
                .unwrap();
            let key = module_key(&initial);
            let mut database = HirDatabase::try_new().unwrap();
            let recovered = lower(&mut database, &initial, &key);

            assert!(!recovered.is_executable(), "{initial_source}");
            let following_ordinal = recovered.source_ordered_items().len() - 1;
            let following_owner = recovered.source_ordered_items()[following_ordinal];
            let following_item = resolve_item(&recovered, following_ordinal);
            assert_clean_current_callable(following_item, expected_family);
            assert_item_slot_whole(&recovered, &initial, following_owner);
            assert!(
                recovered
                    .source_ordered_items()
                    .iter()
                    .take(following_ordinal)
                    .any(|owner| matches!(
                        recovered
                            .arenas()
                            .items()
                            .resolve(recovered.slots(), *owner)
                            .unwrap()
                            .kind(),
                        HirItemKind::Error(_)
                    )),
                "{initial_source}"
            );

            let repaired = syntax
                .reparse(
                    &initial,
                    &[SourceEdit::new(
                        initial
                            .document()
                            .span(SourceRange::new(0, removed_prefix.len()))
                            .unwrap(),
                        "",
                    )],
                    arcweft_lang_syntax::parser::ParseOptions::default(),
                )
                .unwrap();
            let executable = lower(&mut database, &repaired, &key);

            assert!(executable.is_executable(), "{following}");
            assert_eq!(executable.source_ordered_items(), [following_owner]);
            let executable_item = resolve_item(&executable, 0);
            assert_clean_current_callable(executable_item, expected_family);
            assert_item_slot_whole(&executable, &repaired, following_owner);
        }
    }
}
