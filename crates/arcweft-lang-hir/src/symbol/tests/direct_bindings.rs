use super::*;

#[test]
fn project_direct_binding_retains_exact_typed_path_and_rejects_explicit_roots() {
    let (document, _) = project("fn main() -> Unit { () }\n");
    let source = document.span(SourceRange::new(0, 2)).expect("source span");
    let path = binding_path(["character", "akane"]);
    let binding = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        path.clone(),
        Some(Visibility::Public),
        source.clone(),
        true,
    )
    .expect("implicit direct binding");

    assert_eq!(binding.path(), &path);
    assert_eq!(binding.visibility(), Some(Visibility::Public));
    assert_eq!(binding.source(), &source);
    assert!(binding.authored_alias());

    for root in [
        ModulePathRoot::Crate,
        ModulePathRoot::SelfModule,
        ModulePathRoot::Super(1),
    ] {
        let explicit = ProjectSymbolPath::new(
            root,
            [ProjectSymbolSegment::try_new("akane").expect("valid segment")],
        )
        .expect("explicit path");
        assert_eq!(
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                explicit,
                None,
                source.clone(),
                false,
            ),
            Err(ProjectDirectBindingError::ExplicitRoot { root })
        );
    }
}

#[test]
fn external_seed_keeps_canonical_identity_and_exact_binding_paths_distinct() {
    let (document, _) = project("fn main() -> Unit { () }\n");
    let source = document.span(SourceRange::new(0, 2)).expect("source span");
    let qualified = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        binding_path(["character", "akane"]),
        Some(Visibility::Public),
        source.clone(),
        false,
    )
    .expect("qualified binding");
    let compact = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        binding_path(["akane"]),
        Some(Visibility::Public),
        source.clone(),
        false,
    )
    .expect("compact binding");
    let alias = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        binding_path(["hero"]),
        Some(Visibility::Public),
        source.clone(),
        true,
    )
    .expect("authored alias");
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "character.akane")
            .expect("opaque canonical path"),
        Some(Visibility::Public),
        source,
        vec![qualified.clone(), compact, alias, qualified],
    )
    .expect("external seed");

    assert!(seed.canonical_path().qualifiers().is_empty());
    assert_eq!(seed.canonical_path().leaf(), "character.akane");
    assert_eq!(seed.direct_bindings().len(), 3);
    assert_eq!(
        seed.direct_bindings()
            .iter()
            .map(|binding| binding.path().to_string())
            .collect::<Vec<_>>(),
        ["akane", "character.akane", "hero"]
    );
}

#[test]
fn direct_external_paths_survive_linking_and_resolve_to_one_target() {
    let (document, project) = project("fn main() -> Unit { () }\n");
    let declarations = declarations(
        &document,
        vec![external_seed(
            &document,
            "character.akane",
            [
                (binding_path(["character", "akane"]), false),
                (binding_path(["akane"]), false),
                (binding_path(["hero"]), true),
            ],
        )],
        "typed-direct-paths",
    );
    let link =
        ProjectSymbolTable::link(project.view(), &declarations).expect("external paths link");
    let declaration = link
        .seed_declarations()
        .next()
        .expect("external declaration")
        .1;
    let expected = ProjectSymbolTargetId::External(declaration);
    let rows = scope_rows(link.table(), &CanonicalModulePath::crate_root())
        .into_iter()
        .filter(|(_, target)| target == &expected)
        .collect::<Vec<_>>();

    assert_eq!(
        rows.iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        [
            vec!["akane".to_owned()],
            vec!["character".to_owned(), "akane".to_owned()],
            vec!["hero".to_owned()],
        ]
    );
    let source = document.span(SourceRange::new(0, 2)).expect("source span");
    for path in [
        binding_path(["character", "akane"]),
        binding_path(["akane"]),
        binding_path(["hero"]),
    ] {
        let reference = SymbolPath::try_from(&path).expect("typed resolution reference");
        assert!(matches!(
            link.table().resolve(
                &CanonicalModulePath::crate_root(),
                &reference,
                &source
            ),
            Ok(ResolvedProjectSymbol::External(symbol))
                if symbol.declaration() == declaration
        ));
    }
}

#[test]
fn unaliased_and_explicit_alias_imports_use_typed_destination_paths() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        (
            "consumer",
            "use character.akane\nuse character.akane as hero\n",
        ),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed(
            &documents[0],
            "character.akane",
            [(binding_path(["character", "akane"]), false)],
        )],
        "typed-path-imports",
    );
    let link = ProjectSymbolTable::link(project.view(), &declarations).expect("imports link");
    let target = ProjectSymbolTargetId::External(
        link.seed_declarations()
            .next()
            .expect("external declaration")
            .1,
    );
    let rows = scope_rows(link.table(), &module_path("consumer"))
        .into_iter()
        .filter(|(_, candidate)| candidate == &target)
        .collect::<Vec<_>>();

    assert_eq!(
        rows.iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        [vec!["akane".to_owned()], vec!["hero".to_owned()]]
    );
    assert!(
        scope_rows(link.table(), &CanonicalModulePath::crate_root())
            .iter()
            .any(|(path, candidate)| {
                path == &["character".to_owned(), "akane".to_owned()] && candidate == &target
            }),
        "the source qualified binding remains independent"
    );
}

#[test]
fn grouped_imports_use_selected_and_alias_segments() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        (
            "cast",
            "pub fn akane() -> Unit { () }\npub fn hero() -> Unit { () }\n",
        ),
        ("consumer", "use crate.cast.{akane, hero as lead}\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "typed-group-imports"),
    )
    .expect("grouped imports link")
    .into_table();
    let rows = scope_rows(&table, &module_path("consumer"));

    assert!(rows.iter().any(|(path, target)| {
        path == &["akane".to_owned()] && matches!(target, ProjectSymbolTargetId::Callable(_))
    }));
    assert!(rows.iter().any(|(path, target)| {
        path == &["lead".to_owned()] && matches!(target, ProjectSymbolTargetId::Callable(_))
    }));
}

#[test]
fn glob_and_fixed_point_reexport_preserve_qualified_external_segments() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        ("origin", ""),
        ("middle", "pub use crate.origin.*\n"),
        ("consumer", "use crate.middle.*\n"),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed_in_module(
            &documents[0],
            "character.akane",
            &module_path("origin"),
            [(binding_path(["character", "akane"]), false)],
        )],
        "typed-glob-reexport",
    );
    let link = ProjectSymbolTable::link(project.view(), &declarations).expect("glob chain links");
    let target = ProjectSymbolTargetId::External(
        link.seed_declarations()
            .next()
            .expect("external declaration")
            .1,
    );

    for module in ["origin", "middle", "consumer"] {
        assert!(
            scope_rows(link.table(), &module_path(module))
                .iter()
                .any(|(path, candidate)| {
                    path == &["character".to_owned(), "akane".to_owned()] && candidate == &target
                }),
            "{module} must retain the exact qualified external path"
        );
    }
}

#[test]
fn external_only_qualifier_import_retains_the_full_typed_binding() {
    let (documents, project) = project_modules(&[
        ("", "predicate seed() = true\n"),
        ("consumer", "use character.hero-pack.2d\n"),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed(
            &documents[0],
            "character.hero-pack.2d",
            [(binding_path(["character", "hero-pack", "2d"]), false)],
        )],
        "external-only-import",
    );
    let link = ProjectSymbolTable::link(project.view(), &declarations)
        .expect("external-only import links");
    let target = ProjectSymbolTargetId::External(
        link.seed_declarations()
            .next()
            .expect("external declaration")
            .1,
    );

    assert!(
        scope_rows(link.table(), &module_path("consumer"))
            .iter()
            .any(|(path, candidate)| {
                path == &[
                    "character".to_owned(),
                    "hero-pack".to_owned(),
                    "2d".to_owned(),
                ] && candidate == &target
            })
    );
}

#[test]
fn typed_scope_iterator_is_insertion_order_independent_and_mixes_target_kinds() {
    let (documents, project) =
        project_modules(&[("", "fn main() -> Unit { () }\n"), ("child", "")]);
    let forward_seed = external_seed(
        &documents[0],
        "character.akane",
        [
            (binding_path(["character", "akane"]), false),
            (binding_path(["akane"]), false),
        ],
    );
    let reverse_seed = external_seed(
        &documents[0],
        "character.akane",
        [
            (binding_path(["akane"]), false),
            (binding_path(["character", "akane"]), false),
        ],
    );
    let forward = ProjectSymbolTable::link(
        project.view(),
        &declarations(
            &documents[0],
            vec![forward_seed],
            "typed-iterator-determinism",
        ),
    )
    .expect("forward facts link");
    let reverse = ProjectSymbolTable::link(
        project.view(),
        &declarations(
            &documents[0],
            vec![reverse_seed],
            "typed-iterator-determinism",
        ),
    )
    .expect("reverse facts link");
    let rows = |table: &ProjectSymbolTable| {
        table
            .scope_bindings()
            .map(|(module, path, target)| (module.clone(), path.clone(), target.clone()))
            .collect::<Vec<_>>()
    };

    assert_eq!(rows(forward.table()), rows(reverse.table()));
    let root_rows = scope_rows(forward.table(), &CanonicalModulePath::crate_root());
    assert!(
        root_rows
            .iter()
            .any(|(_, target)| matches!(target, ProjectSymbolTargetId::Callable(_)))
    );
    assert!(
        root_rows
            .iter()
            .any(|(_, target)| matches!(target, ProjectSymbolTargetId::Module(_)))
    );
    assert!(
        root_rows
            .iter()
            .any(|(_, target)| matches!(target, ProjectSymbolTargetId::External(_)))
    );
}
