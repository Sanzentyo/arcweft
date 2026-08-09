use super::*;

use arcweft_lang_syntax::attachment::source_file::{
    AttachedPath, AttachedUseGroupChild, AttachedUseTree,
};
use arcweft_lang_syntax::attachment::{
    AttachedCapabilityMember, AttachedEntryId, AttachedEntryMember,
};
use arcweft_lang_syntax::patterns::PatternComponentRole;
use arcweft_source::SourceSpan;

use crate::source_index::{
    HirCallableEffectSourcePart, HirCallableParameterSourcePart, HirCallableSourceOwner,
    HirCallableSourceRole, HirDeclarationSourceRole, HirEntrySourcePart, HirItemSourceRole,
    HirSourcePresence, HirSourceQuery, HirSourceQueryError, HirUseBindingSourcePart,
    HirUseSourceRole,
};

fn item_query(owner: crate::identity::ItemId, role: HirItemSourceRole) -> HirSourceQuery {
    HirSourceQuery::Item { owner, role }
}

fn declaration_query(
    owner: crate::identity::ItemId,
    role: HirDeclarationSourceRole,
) -> HirSourceQuery {
    item_query(owner, HirItemSourceRole::Declaration(role))
}

fn use_query(owner: crate::identity::ItemId, role: HirUseSourceRole) -> HirSourceQuery {
    item_query(owner, HirItemSourceRole::Use(role))
}

fn callable_query(owner: crate::identity::ItemId, role: HirCallableSourceRole) -> HirSourceQuery {
    item_query(owner, HirItemSourceRole::Callable(role))
}

fn entry_query(owner: crate::identity::ItemId, part: HirEntrySourcePart) -> HirSourceQuery {
    item_query(owner, HirItemSourceRole::Entry(part))
}

fn assert_present_span(
    module: &HirModule,
    parsed: &ParsedSource,
    query: HirSourceQuery,
    expected: &SourceSpan,
) {
    let lookup = module
        .source_site(parsed.document().identity(), query)
        .expect("typed item source query");
    assert!(matches!(
        lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(actual)) if actual == expected
    ));
}

fn declaration_name_source(item: &TypedItemNode) -> SourceSpan {
    match item {
        TypedItemNode::Function(item) => item.semantics().unwrap().name().syntax().source_span(),
        TypedItemNode::Predicate(item) => item.semantics().unwrap().name().syntax().source_span(),
        TypedItemNode::Proof(item) => item.semantics().unwrap().name().syntax().source_span(),
        TypedItemNode::Struct(item) => item.semantics().unwrap().name().syntax().source_span(),
        TypedItemNode::Enum(item) => item.semantics().unwrap().name().syntax().source_span(),
        TypedItemNode::TypeAlias(item) => item.semantics().unwrap().name().syntax().source_span(),
        _ => panic!("fixture item must be an admitted named declaration"),
    }
}

#[test]
fn named_declaration_whole_and_name_roles_are_exact_for_every_admitted_family() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-item-declaration-sources",
        concat!(
            "fn routine() {}\n",
            "predicate logical() = true\n",
            "proof evidence() = ()\n",
            "struct Record {}\n",
            "enum Choice {}\n",
            "type Alias = Int\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let attached = parsed.items().unwrap();
    assert_eq!(attached.len(), 6);

    for (ordinal, item) in attached.iter().enumerate() {
        let owner = module.source_ordered_items()[ordinal];
        let whole = module
            .source_site(
                parsed.document().identity(),
                declaration_query(owner, HirDeclarationSourceRole::Whole),
            )
            .unwrap();
        assert_present_span(
            &module,
            &parsed,
            declaration_query(owner, HirDeclarationSourceRole::Whole),
            &item.source_span(),
        );
        let name = declaration_name_source(item);
        let lookup = module
            .source_site(
                parsed.document().identity(),
                declaration_query(owner, HirDeclarationSourceRole::Name),
            )
            .unwrap();
        assert_eq!(lookup.owner_status(), whole.owner_status());
        let HirSourcePresence::Present(HirSourceSite::Span(actual)) = lookup.presence() else {
            panic!(
                "declaration {ordinal} must publish an exact name span, got {:?}",
                lookup.presence()
            )
        };
        assert_eq!(actual, &name, "declaration {ordinal}");
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the callable source test exhausts final item and inline-member ownership roles"
)]
fn callable_roles_preserve_final_item_and_inline_member_ownership() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-callable-sources",
        concat!(
            "fn configured(first: Int = 1)(rest: ...String) -> Unit { first }\n",
            "extern capability host {\n",
            "    fn send(value: Bytes = 1)(count: Int) -> Bool\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let attached = parsed.items().unwrap();
    let ordinary_owner = module.source_ordered_items()[0];
    let capability_owner = module.source_ordered_items()[1];

    let TypedItemNode::Function(function) = &attached[0] else {
        panic!("first item must be the ordinary function")
    };
    let function = function.semantics().unwrap();
    let ordinary = HirCallableSourceOwner::Item;
    assert_present_span(
        &module,
        &parsed,
        callable_query(
            ordinary_owner,
            HirCallableSourceRole::Name { owner: ordinary },
        ),
        &function.name().syntax().source_span(),
    );
    let first = &function.parameter_groups()[0].parameters()[0];
    for (part, expected) in [
        (
            HirCallableParameterSourcePart::Whole,
            first.syntax().source_span(),
        ),
        (
            HirCallableParameterSourcePart::Name,
            first
                .pattern()
                .component(PatternComponentRole::Name)
                .expect("simple binding name"),
        ),
        (
            HirCallableParameterSourcePart::Type,
            first.ty().syntax().source_span(),
        ),
        (
            HirCallableParameterSourcePart::Default,
            first
                .default()
                .expect("authored default")
                .value()
                .syntax()
                .source_span(),
        ),
    ] {
        assert_present_span(
            &module,
            &parsed,
            callable_query(
                ordinary_owner,
                HirCallableSourceRole::Parameter {
                    owner: ordinary,
                    group: 0,
                    parameter: 0,
                    part,
                },
            ),
            &expected,
        );
    }

    let TypedItemNode::ExternCapability(capability) = &attached[1] else {
        panic!("second item must be the external capability")
    };
    let capability = capability.semantics().unwrap();
    let [AttachedCapabilityMember::Function(function)] = capability.body().members() else {
        panic!("fixture capability must own one inline function")
    };
    let external = HirCallableSourceOwner::ExternCapabilityFunction { member: 0 };
    assert_present_span(
        &module,
        &parsed,
        callable_query(
            capability_owner,
            HirCallableSourceRole::Name { owner: external },
        ),
        &function.name().syntax().source_span(),
    );
    assert!(matches!(
        module.source_site(
            parsed.document().identity(),
            callable_query(
                ordinary_owner,
                HirCallableSourceRole::Name { owner: external },
            ),
        ),
        Err(HirSourceQueryError::ItemRoleNotApplicable { owner, .. })
            if owner == ordinary_owner
    ));
    assert!(matches!(
        module.source_site(
            parsed.document().identity(),
            callable_query(
                capability_owner,
                HirCallableSourceRole::Name {
                    owner: HirCallableSourceOwner::ExternCapabilityFunction {
                        member: u16::MAX,
                    },
                },
            ),
        ),
        Err(HirSourceQueryError::ItemRoleNotApplicable { owner, .. })
            if owner == capability_owner
    ));
}

#[test]
fn function_effect_clause_roles_preserve_omitted_empty_and_nonempty_source_states() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-function-effect-sources",
        concat!(
            "fn inferred() {}\n",
            "fn empty() effects {} {}\n",
            "fn bounded() effects { fs.read, debug.record } {}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let attached = parsed.items().unwrap();
    let owners = module.source_ordered_items();
    assert_eq!(owners.len(), 3);

    let omitted = callable_query(
        owners[0],
        HirCallableSourceRole::EffectClause {
            owner: HirCallableSourceOwner::Item,
            clause: 0,
            part: HirCallableEffectSourcePart::Whole,
        },
    );
    assert!(matches!(
        module.source_site(parsed.document().identity(), omitted),
        Err(HirSourceQueryError::ItemRoleNotApplicable { owner, .. }) if owner == owners[0]
    ));

    for item_index in [1_usize, 2] {
        let TypedItemNode::Function(function) = &attached[item_index] else {
            panic!("effect fixture item must be an ordinary function")
        };
        let function = function.semantics().unwrap();
        let effects = function
            .contracts()
            .iter()
            .filter(|clause| clause.is_effects())
            .collect::<Vec<_>>();
        assert_eq!(effects.len(), 1);
        let owner = owners[item_index];
        assert_present_span(
            &module,
            &parsed,
            callable_query(
                owner,
                HirCallableSourceRole::EffectClause {
                    owner: HirCallableSourceOwner::Item,
                    clause: 0,
                    part: HirCallableEffectSourcePart::Whole,
                },
            ),
            &effects[0].syntax_source_span(),
        );
        assert_present_span(
            &module,
            &parsed,
            callable_query(
                owner,
                HirCallableSourceRole::EffectClause {
                    owner: HirCallableSourceOwner::Item,
                    clause: 0,
                    part: HirCallableEffectSourcePart::Keyword,
                },
            ),
            &effects[0].keyword_source_span(),
        );
    }
}

#[test]
fn entry_whole_and_id_roles_are_exact_without_a_kind_source_alias() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-sources",
        "entry game @entry.main {}\nfn ordinary() {}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let attached = parsed.items().unwrap();
    let entry_owner = module.source_ordered_items()[0];
    let function_owner = module.source_ordered_items()[1];
    let TypedItemNode::Entry(entry) = &attached[0] else {
        panic!("first item must be Entry")
    };
    let entry = entry.semantics().unwrap();
    let AttachedEntryId::Authored { expression, .. } = entry.id() else {
        panic!("fixture Entry must own one authored ID")
    };
    assert_present_span(
        &module,
        &parsed,
        entry_query(entry_owner, HirEntrySourcePart::Whole),
        &entry.syntax().source_span(),
    );
    assert_present_span(
        &module,
        &parsed,
        entry_query(entry_owner, HirEntrySourcePart::Id),
        &expression.syntax().source_span(),
    );
    assert!(matches!(
        module.source_site(
            parsed.document().identity(),
            entry_query(function_owner, HirEntrySourcePart::Id),
        ),
        Err(HirSourceQueryError::ItemRoleNotApplicable { owner, .. })
            if owner == function_owner
    ));
}

#[test]
fn entry_member_value_roles_preserve_each_final_role_rhs_without_sidecar_reconstruction() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-member-value-sources",
        concat!(
            "entry server @entry.http {\n",
            "    state = ServerState\n",
            "    initializer = server.initial_state\n",
            "    event = ServerEvent\n",
            "    reducer = server.reduce\n",
            "    controller = server.control\n",
            "    goto @flow.start\n",
            "    route GET \"/\" -> @flow.home\n",
            "    budget = policy(1)\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let attached = parsed.items().unwrap();
    let TypedItemNode::Entry(entry) = &attached[0] else {
        panic!("fixture item must be Entry")
    };
    let entry = entry.semantics().unwrap();
    let members = entry.body().members();
    assert_eq!(members.len(), 8);

    let expected = members
        .iter()
        .filter_map(|member| {
            let span = match member {
                AttachedEntryMember::StateType(binding)
                | AttachedEntryMember::EventType(binding) => {
                    binding.value().value().unwrap().syntax().source_span()
                }
                AttachedEntryMember::Initializer(binding)
                | AttachedEntryMember::Reducer(binding)
                | AttachedEntryMember::Controller(binding) => {
                    binding.value().value().unwrap().syntax().source_span()
                }
                AttachedEntryMember::Goto { target, .. } => {
                    target.value().unwrap().whole_source_span()
                }
                AttachedEntryMember::Route { .. }
                | AttachedEntryMember::Option { .. }
                | AttachedEntryMember::Error { .. } => return None,
            };
            Some((member.source_ordinal(), span))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected
            .iter()
            .map(|(ordinal, _)| *ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );

    for (member, span) in expected {
        assert_present_span(
            &module,
            &parsed,
            entry_query(owner, HirEntrySourcePart::MemberValue { member }),
            &span,
        );
    }

    let route_value = module.source_site(
        parsed.document().identity(),
        entry_query(owner, HirEntrySourcePart::MemberValue { member: 6 }),
    );
    assert!(matches!(
        route_value,
        Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
            owner: actual,
            length: 8,
            ..
        }) if actual == owner
    ));
}

#[test]
fn item_role_validation_rejects_wrong_families_and_flattened_use_overflow() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-item-source-wrong-family",
        "use crate.alpha.target\nfn routine() {}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let import = module.source_ordered_items()[0];
    let function = module.source_ordered_items()[1];
    let HirItemKind::Use(import_payload) = resolve_item(&module, 0).kind() else {
        panic!(
            "first source item must remain a Use declaration, got {:?}",
            resolve_item(&module, 0).kind()
        )
    };
    assert_eq!(import_payload.bindings().len(), 1);

    assert!(matches!(
        module.source_site(
            parsed.document().identity(),
            declaration_query(import, HirDeclarationSourceRole::Name),
        ),
        Err(HirSourceQueryError::ItemRoleNotApplicable { owner, .. }) if owner == import
    ));
    assert!(matches!(
        module.source_site(
            parsed.document().identity(),
            use_query(function, HirUseSourceRole::Whole),
        ),
        Err(HirSourceQueryError::ItemRoleNotApplicable { owner, .. }) if owner == function
    ));
    let overflow = module.source_site(
        parsed.document().identity(),
        use_query(
            import,
            HirUseSourceRole::Binding {
                ordinal: 1,
                part: HirUseBindingSourcePart::TerminalReference,
            },
        ),
    );
    assert!(
        matches!(
            overflow,
            Err(HirSourceQueryError::ItemOrdinalOutOfBounds { owner, length: 1, .. })
                if owner == import
        ),
        "{overflow:?}"
    );
}

#[test]
fn use_binding_manifest_preserves_path_terminal_alias_and_group_recovery_order() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-use-binding-sources",
        concat!(
            "use crate.game.route\n",
            "use self.widgets.*\n",
            "use parent.data.{alice, bob as narrator, , carol as}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let attached = parsed.items().unwrap();

    for (item_ordinal, item) in attached.iter().enumerate() {
        let TypedItemNode::Use(item) = item else {
            panic!("fixture item must be Use")
        };
        let owner = module.source_ordered_items()[item_ordinal];
        assert_present_span(
            &module,
            &parsed,
            use_query(owner, HirUseSourceRole::Whole),
            &item.source_span(),
        );
        let projected = use_binding_sources(item.tree().unwrap());
        for (ordinal, (path, terminal, alias)) in projected.into_iter().enumerate() {
            let ordinal = u32::try_from(ordinal).unwrap();
            for (part, expected) in [
                (HirUseBindingSourcePart::Path, path),
                (HirUseBindingSourcePart::TerminalReference, terminal),
            ] {
                assert_present_span(
                    &module,
                    &parsed,
                    use_query(owner, HirUseSourceRole::Binding { ordinal, part }),
                    &expected,
                );
            }
            let alias_lookup = module
                .source_site(
                    parsed.document().identity(),
                    use_query(
                        owner,
                        HirUseSourceRole::Binding {
                            ordinal,
                            part: HirUseBindingSourcePart::Alias,
                        },
                    ),
                )
                .unwrap();
            match alias {
                Some(expected) => assert!(matches!(
                    alias_lookup.presence(),
                    HirSourcePresence::Present(HirSourceSite::Span(actual)) if actual == &expected
                )),
                None => assert_eq!(alias_lookup.presence(), HirSourcePresence::AbsentOptional),
            }
        }
    }
}

fn use_binding_sources(tree: AttachedUseTree) -> Vec<(SourceSpan, SourceSpan, Option<SourceSpan>)> {
    match tree {
        AttachedUseTree::Path { path, alias } => vec![(
            path.syntax().source_span(),
            path_terminal(&path),
            alias.map(|alias| alias.source_span().clone()),
        )],
        AttachedUseTree::Glob {
            module,
            marker,
            alias,
        } => vec![(
            module.syntax().source_span(),
            marker,
            alias.map(|alias| alias.source_span().clone()),
        )],
        AttachedUseTree::Group {
            module, children, ..
        } => children
            .into_vec()
            .into_iter()
            .filter_map(|child| match child {
                AttachedUseGroupChild::Binding(binding) => Some((
                    module.syntax().source_span(),
                    binding.name().source_span(),
                    binding.alias().map(|alias| alias.source_span().clone()),
                )),
                AttachedUseGroupChild::Recovery { .. } => None,
            })
            .collect(),
    }
}

fn path_terminal(path: &AttachedPath) -> SourceSpan {
    path.missing_name()
        .map(arcweft_lang_syntax::attachment::family::FamilyNode::source_span)
        .or_else(|| {
            path.segments()
                .last()
                .map(arcweft_lang_syntax::attachment::source_file::AttachedPathSegment::source_span)
        })
        .expect("attached use path owns a terminal reference")
}

#[test]
fn declaration_queries_reject_foreign_documents_and_stale_revisions() {
    let name = SourceName::path("proof/final-hir-declaration-source-query.arcw");
    let document_id = "arcweft-test://proof/final-hir-declaration-source-query";
    let source = "fn stable() {}\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(source.len(), source.len()))
                    .unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&revised);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &revised, &key);
    let owner = module.source_ordered_items()[0];
    let query = declaration_query(owner, HirDeclarationSourceRole::Name);

    assert!(matches!(
        module.source_site(initial.document().identity(), query.clone()),
        Err(HirSourceQueryError::StaleSourceRevision { expected, actual })
            if expected == revised.document().identity().revision()
                && actual == initial.document().identity().revision()
    ));
    let foreign = parse(
        "arcweft-test://proof/final-hir-declaration-source-query-foreign",
        source,
    );
    assert!(matches!(
        module.source_site(foreign.document().identity(), query),
        Err(HirSourceQueryError::WrongSourceDocument { expected, actual })
            if expected == *revised.document().identity().id()
                && actual == *foreign.document().identity().id()
    ));
}

#[test]
fn missing_required_name_component_rolls_back_without_publication_and_retries() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-declaration-source-rollback",
        "fn stable() {}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction.lower_parsed_source_items(&parsed).unwrap();
    let owner = transaction.staged_source_ordered_items()[0];
    let snapshot = transaction.snapshot_id();
    let query = declaration_query(owner, HirDeclarationSourceRole::Name);
    assert!(transaction.source_components().remove_staged_query(&query));
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());

    let mut retry = stage(&database, &parsed, &key);
    assert_eq!(retry.snapshot_id(), snapshot);
    retry.lower_parsed_source_items(&parsed).unwrap();
    assert_eq!(retry.staged_source_ordered_items(), [owner]);
    let accepted = retry.finish(&mut database).unwrap().into_module();
    assert_present_span(
        &accepted,
        &parsed,
        query,
        &declaration_name_source(&parsed.items().unwrap()[0]),
    );
}
