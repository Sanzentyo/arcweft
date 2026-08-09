use super::*;

use crate::item::{HirDeclarationMemberIssue, HirViewDeclaration};
use crate::source_index::{
    HirCallableSourceOwner, HirCallableSourceRole, HirItemSourceRole, HirSourcePresence,
    HirSourceQuery, HirViewBodySourcePart, HirViewExportSourcePart, HirViewSourceRole,
};

use super::super::retained::preflight_view_exports;

fn assert_view_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-view-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction.lower_parsed_source_items(&parsed).unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&mut transaction, owner);
    assert!(
        matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ),
        "View freeze accepted {case}"
    );
    assert!(database.current(&key).is_none());
}

fn revise_view(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::ItemId,
    mutate: impl FnOnce(&mut Vec<crate::item::HirParameter>, &mut Vec<ExprId>),
) {
    let (
        scope,
        prefix,
        state,
        members,
        header,
        callable_scope,
        mut parameters,
        exports,
        mut values,
    ) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::View(view) = item.kind() else {
            panic!("final View item")
        };
        (
            item.scope(),
            item.prefix().clone(),
            *item.state(),
            item.members().into(),
            view.header().clone(),
            view.callable_scope(),
            view.parameters().to_vec(),
            view.exports().into(),
            view.values().to_vec(),
        )
    };
    mutate(&mut parameters, &mut values);
    let declaration = HirViewDeclaration::try_new(
        owner,
        header,
        callable_scope,
        parameters.into_boxed_slice(),
        exports,
        values.into_boxed_slice(),
    )
    .unwrap();
    let replacement = HirItem::try_new_with_state(
        owner,
        scope,
        prefix,
        HirItemKind::View(declaration),
        members,
        state,
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
}

fn view(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirViewDeclaration) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::View(view) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a View")
    };
    (owner, item, view)
}

fn member(
    module: &HirModule,
    id: crate::item::HirDeclarationMemberId,
) -> &crate::item::HirDeclarationMember {
    module.declaration_members().resolve(id).unwrap()
}

fn view_source_query(owner: crate::identity::ItemId, role: HirViewSourceRole) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::View(role),
    }
}

fn callable_source_query(
    owner: crate::identity::ItemId,
    role: HirCallableSourceRole,
) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Callable(role),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the canonical View test asserts one complete parameter, export, value, scope, and source graph"
)]
fn canonical_view_freezes_callable_parameters_exports_and_value_owners() {
    let source = concat!(
        "/// Main View\n",
        "pub view Main(count: u32 = 1) {\n",
        "    export part panel as public.panel\n",
        "    Panel {}\n",
        "    Text(count)\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-view-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = view(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert!(matches!(
        declaration.header().name(),
        HirRetainedName::Resolved(name) if name.as_str() == "Main"
    ));
    assert_eq!(declaration.parameters().len(), 1);
    let parameter = &declaration.parameters()[0];
    assert_eq!(parameter.kind(), HirParameterKind::Fixed);
    assert!(parameter.default().is_some());
    assert_source_backed_child(&module, parameter.pattern());
    assert_source_backed_child(&module, parameter.ty());
    assert_source_backed_child(&module, parameter.default().unwrap());

    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), declaration.callable_scope())
        .unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item.scope()));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(callable.locals(), parameter.locals());
    let callable_metadata = module
        .slots()
        .resolve(declaration.callable_scope())
        .unwrap();
    assert!(matches!(callable_metadata.origin(), HirOrigin::Source(_)));

    let [export_id] = declaration.exports() else {
        panic!("canonical View must retain one export")
    };
    assert_eq!(item.members(), declaration.exports());
    let export = member(&module, *export_id);
    assert_eq!(export.state(), HirDeclarationMemberPoisonState::Clean);
    let HirDeclarationMemberKind::ViewExport(export) = export.kind() else {
        panic!("View member must retain the export payload")
    };
    assert_eq!(
        path_spellings(resolved_path(export.local_part())),
        ["panel"]
    );
    assert_eq!(
        path_spellings(resolved_path(export.public_part())),
        ["public", "panel"]
    );
    assert_eq!(
        module
            .declaration_members()
            .arena(owner)
            .unwrap()
            .members()
            .len(),
        1
    );

    assert_eq!(declaration.values().len(), 2);
    for value in declaration.values().iter().copied() {
        let expression = module
            .arenas()
            .expressions()
            .resolve(module.slots(), value)
            .unwrap();
        assert_eq!(expression.scope(), declaration.callable_scope());
        assert_source_backed_child(&module, value);
    }
    assert_item_slot_whole(&module, &parsed, owner);

    let attached_items = parsed.items().unwrap();
    let [TypedItemNode::View(attached_node)] = attached_items.as_slice() else {
        panic!("canonical source must retain exactly one attached View")
    };
    let attached = attached_node.semantics().unwrap();
    let arcweft_lang_syntax::attachment::AttachedViewBody::Braced {
        open,
        close,
        fragment,
        ..
    } = attached.body()
    else {
        panic!("canonical View must retain a braced body")
    };

    let whole = module
        .source_site(
            parsed.document().identity(),
            view_source_query(owner, HirViewSourceRole::Whole),
        )
        .unwrap();
    assert!(matches!(
        whole.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(span))
            if span == &attached.syntax().source_span()
    ));
    assert_eq!(
        module
            .source_site(
                parsed.document().identity(),
                view_source_query(owner, HirViewSourceRole::ItemId),
            )
            .unwrap()
            .presence(),
        HirSourcePresence::AbsentOptional
    );
    for (part, expected) in [
        (HirViewBodySourcePart::OpenDelimiter, open.source_span()),
        (HirViewBodySourcePart::CloseDelimiter, close.source_span()),
        (
            HirViewBodySourcePart::Fragment,
            fragment.syntax().source_span(),
        ),
    ] {
        let lookup = module
            .source_site(
                parsed.document().identity(),
                view_source_query(owner, HirViewSourceRole::Body(part)),
            )
            .unwrap();
        assert!(matches!(
            lookup.presence(),
            HirSourcePresence::Present(HirSourceSite::Span(actual)) if actual == &expected
        ));
    }
    let name = match attached.header().name() {
        arcweft_lang_syntax::attachment::AttachedRetainedName::Resolved { syntax, .. }
        | arcweft_lang_syntax::attachment::AttachedRetainedName::Missing { syntax }
        | arcweft_lang_syntax::attachment::AttachedRetainedName::Invalid { syntax } => {
            syntax.source_span()
        }
    };
    let name_lookup = module
        .source_site(
            parsed.document().identity(),
            callable_source_query(
                owner,
                HirCallableSourceRole::Name {
                    owner: HirCallableSourceOwner::ViewItem,
                },
            ),
        )
        .unwrap();
    assert!(matches!(
        name_lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(actual)) if actual == &name
    ));
    let export = attached.exports().next().unwrap();
    let export_lookup = module
        .source_site(
            parsed.document().identity(),
            view_source_query(
                owner,
                HirViewSourceRole::Export {
                    ordinal: 0,
                    part: HirViewExportSourcePart::LocalPart,
                },
            ),
        )
        .unwrap();
    let expected_export = match export.local_part() {
        arcweft_lang_syntax::attachment::AttachedViewPartPath::Path(path) => {
            path.syntax().source_span()
        }
        arcweft_lang_syntax::attachment::AttachedViewPartPath::Missing(syntax) => {
            syntax.source_span()
        }
    };
    assert!(matches!(
        export_lookup.presence(),
        HirSourcePresence::Present(HirSourceSite::Span(actual)) if actual == &expected_export
    ));
}

#[test]
fn view_recovery_preserves_family_export_ordinals_and_primary_issue_order() {
    let source = concat!(
        "view Broken() {\n",
        "    export part first as public_first\n",
        "    Panel {}\n",
        "    export late\n",
        "    Text(1)\n",
        "}\n",
        "view Missing\n",
        "view Header() -> View { Panel {} } trailing\n",
        "view Tuple((left, right): Pair) { Panel {} }\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-view-recovery", source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (_, broken_item, broken) = view(&module, 0);
    assert_eq!(
        broken_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert_eq!(broken.exports().len(), 2);
    assert_eq!(broken.values().len(), 2);
    assert_eq!(broken.exports()[0].ordinal(), 0);
    assert_eq!(broken.exports()[1].ordinal(), 1);
    assert_eq!(
        member(&module, broken.exports()[0]).state(),
        HirDeclarationMemberPoisonState::Clean
    );
    assert_eq!(
        member(&module, broken.exports()[1]).state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
    );

    let (missing_owner, missing_item, missing) = view(&module, 1);
    assert_eq!(
        missing_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    assert!(missing.parameters().is_empty());
    assert!(missing.exports().is_empty());
    assert!(missing.values().is_empty());
    assert!(matches!(
        module
            .source_site(
                parsed.document().identity(),
                view_source_query(
                    missing_owner,
                    HirViewSourceRole::Body(HirViewBodySourcePart::Whole),
                ),
            )
            .unwrap()
            .presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
    assert_eq!(
        module
            .source_site(
                parsed.document().identity(),
                view_source_query(
                    missing_owner,
                    HirViewSourceRole::Body(HirViewBodySourcePart::OpenDelimiter),
                ),
            )
            .unwrap()
            .presence(),
        HirSourcePresence::AbsentOptional
    );

    let (_, header_item, _) = view(&module, 2);
    assert_eq!(
        header_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );

    let (_, tuple_item, tuple) = view(&module, 3);
    assert_eq!(
        tuple_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert_eq!(tuple.parameters().len(), 1);
    assert_eq!(tuple.parameters()[0].locals().len(), 2);
}

#[test]
fn view_export_preflight_accepts_exact_and_rejects_first_one_over() {
    assert!(preflight_view_exports(HirLimit::DeclarationMembers.maximum()).is_ok());
    let observed = HirLimit::DeclarationMembers.maximum() + 1;
    let Err(HirLowerFailure::Limit(error)) = preflight_view_exports(observed) else {
        panic!("first one-over View export count must fail the shared declaration-member limit")
    };
    assert_eq!(error.limit(), HirLimit::DeclarationMembers);
    assert_eq!(error.observed(), observed);
    assert_eq!(error.maximum(), HirLimit::DeclarationMembers.maximum());
}

#[test]
fn view_freeze_rejects_value_order_and_parameter_default_substitution() {
    let source = concat!(
        "view Main(count: u32 = 1) {\n",
        "    Panel {}\n",
        "    Text(count)\n",
        "}\n",
    );
    assert_view_freeze_rejects("value-order", source, |transaction, owner| {
        revise_view(transaction, owner, |_, values| values.reverse());
    });
    assert_view_freeze_rejects("parameter-default", source, |transaction, owner| {
        revise_view(transaction, owner, |parameters, values| {
            let parameter = &parameters[0];
            parameters[0] = crate::item::HirParameter::try_new(
                parameter.pattern(),
                parameter.ty(),
                parameter.kind(),
                Some(values[0]),
                parameter.locals().into(),
            )
            .unwrap();
        });
    });
}
