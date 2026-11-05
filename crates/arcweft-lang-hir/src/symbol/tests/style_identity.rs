use super::*;
use crate::item::HirRetainedName;
use arcweft_id::{DeclarationIdentityFamily, DeclarationPath};

fn reference(value: &str) -> HirIdRef {
    HirIdRef::absolute(HirEntityReference::try_new(value.into()).unwrap())
}

#[test]
fn styles_share_original_declaration_lookup_visibility_and_imports() {
    let (documents, project) = project_modules(&[
        ("", "use crate.palette.theme.Accent as ImportedAccent\n"),
        (
            "palette",
            "pub style theme.Accent {}\nstyle Hidden {}\npub style @style.shared.explicit {}\n",
        ),
        ("other", ""),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "style-identity"),
    )
    .expect("one shared declaration catalog")
    .into_table();
    let source = documents[0].span(SourceRange::new(0, 3)).unwrap();
    let root = CanonicalModulePath::crate_root();
    let ResolvedProjectSymbol::Retained(imported) = table
        .resolve(&root, &symbol_path("ImportedAccent"), &source)
        .unwrap()
    else {
        panic!("imported Style declaration")
    };
    assert_eq!(imported.family(), DeclarationIdentityFamily::Style);
    assert_eq!(imported.public_id().as_str(), "style.palette.theme.Accent");
    assert_eq!(
        imported.name(),
        Some(&DeclarationPath::try_new("theme.Accent").unwrap())
    );
    assert_eq!(imported.module(), &module_path("palette"));
    assert!(imported.callable().is_none());
    assert!(imported.action_payload().is_none());
    let ResolvedProjectSymbol::Retained(direct) = table
        .resolve_entity_reference(
            &root,
            &reference("style.palette.theme.Accent"),
            source.clone(),
        )
        .unwrap()
    else {
        panic!("original Style identity")
    };
    assert_eq!(direct, imported);
    assert!(matches!(
        table.resolve_entity_reference(&root, &reference("style.palette.Hidden"), source.clone()),
        Err(crate::symbol::ProjectEntityReferenceLookupError::Inaccessible { .. })
    ));
    assert!(
        matches!(table.resolve_entity_reference(&module_path("palette"), &reference("style.palette.Hidden"), source.clone()), Ok(ResolvedProjectSymbol::Retained(symbol)) if symbol.family() == DeclarationIdentityFamily::Style)
    );
    let ResolvedProjectSymbol::Retained(explicit) = table
        .resolve_entity_reference(&root, &reference("style.shared.explicit"), source)
        .unwrap()
    else {
        panic!("explicit Style identity")
    };
    assert_eq!(
        explicit.name(),
        None,
        "an explicit ID does not fabricate a local name"
    );
    assert_eq!(table.scope_bindings().filter(|(_, _, target)| matches!(target, ProjectSymbolTargetId::Retained(id) if id.as_str() == "style.shared.explicit")).count(), 0);
    let item = project
        .view()
        .items()
        .find(|item| item.id() == explicit.owner())
        .unwrap();
    assert!(
        matches!(item.item().kind(), crate::item::HirItemKind::Style(style) if matches!(style.header().name(), HirRetainedName::IdentityOnly))
    );
    assert_eq!(
        explicit.declaration_span().source(),
        documents[1].identity()
    );
}
