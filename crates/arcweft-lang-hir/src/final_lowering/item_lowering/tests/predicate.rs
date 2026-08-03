use super::*;

use arcweft_lang_syntax::assertion::AssertionMode;
use arcweft_lang_syntax::attachment::TypedItemNode;

use crate::expr::HirPoisonState;
use crate::identity::{LocalGeneration, TypeId};
use crate::item::{HirCallableSignature, HirContractScopes, HirPredicate, HirPredicateBody};
use crate::pattern::{HirPatternBinding, HirPatternBindingIssue, HirPatternKind};
use crate::stmt::{
    HirAssertionMode, HirStmt, HirStmtKind, HirStmtPoisonState, HirStmtRecoveryIssue,
};
use crate::type_ref::{HirGenericTypeIssue, HirType, HirTypeKind};

fn predicate(
    module: &HirModule,
    ordinal: usize,
) -> (
    crate::identity::ItemId,
    &HirItem,
    &crate::item::HirPredicate,
) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Predicate(predicate) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Predicate")
    };
    (owner, item, predicate)
}

fn source_range<I: HirTypedId>(module: &HirModule, id: I) -> SourceRange {
    match module.slots().resolve(id).unwrap().source_site() {
        HirSourceSite::Span(span) => span.range(),
        HirSourceSite::Insertion(insertion) => {
            SourceRange::new(insertion.offset(), insertion.offset())
        }
    }
}

fn assert_predicate_body_scope(
    module: &HirModule,
    parsed: &ParsedSource,
    ordinal: usize,
    owner: crate::identity::ItemId,
    predicate: &HirPredicate,
) -> ScopeId {
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .filter_map(|item| match item {
            TypedItemNode::Predicate(predicate) => predicate.semantics().ok(),
            _ => None,
        })
        .nth(ordinal)
        .expect("attached Predicate body");
    let body_syntax = attached.body().syntax();
    assert_eq!(
        body_syntax.kind(),
        arcweft_lang_syntax::grammar::SyntaxKind::PredicateBody
    );
    let body_scope = predicate.body().scope();
    let payload = module
        .arenas()
        .scopes()
        .resolve(module.slots(), body_scope)
        .unwrap();
    assert_eq!(payload.kind(), HirScopeKind::Predicate);
    assert_eq!(payload.parent(), Some(predicate.callable_scope()));
    assert_eq!(payload.owner(), &HirScopeOwner::Item(owner));
    let metadata = module.slots().resolve(body_scope).unwrap();
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Source(source) if source.syntax() == body_syntax.id()
    ));
    assert_eq!(
        metadata.source_site(),
        &HirSourceSite::Span(body_syntax.source_span())
    );
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), predicate.callable_scope())
        .unwrap();
    assert_eq!(
        callable.children(),
        [
            predicate.requires_scope(),
            predicate.ensures_scope(),
            body_scope,
        ]
    );
    body_scope
}

fn assert_predicate_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&ParsedSource, &mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-predicate-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&parsed, &mut transaction, owner);
    assert!(
        matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ),
        "Predicate freeze accepted {case}"
    );
    assert!(database.current(&key).is_none());
}

#[test]
fn receiver_shaped_predicate_parameter_commits_typed_retained_recovery() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-predicate-receiver-parameter-recovery",
        "predicate recovered(self) = true\n",
    );
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.parameter.missing_type")
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, predicate) = predicate(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingType)
    );
    let [parameter] = predicate.parameters() else {
        panic!("receiver-shaped Predicate parameter must remain in HIR")
    };
    assert!(parameter.locals().is_empty());
    let pattern = module
        .arenas()
        .patterns()
        .resolve(module.slots(), parameter.pattern())
        .unwrap();
    assert!(matches!(
        pattern.kind(),
        HirPatternKind::Binding(HirPatternBinding::Recovered {
            issue: HirPatternBindingIssue::InvalidName(
                crate::leaf::HirNameInvariantError::InvalidIdentifier
            )
        })
    ));
    assert!(pattern.state().is_poisoned());
    let ty = module
        .arenas()
        .types()
        .resolve(module.slots(), parameter.ty())
        .unwrap();
    assert!(matches!(
        ty.kind(),
        HirTypeKind::Recovery(error)
            if error.issue() == HirGenericTypeIssue::UnclassifiedSyntax
    ));
    assert!(ty.state().is_poisoned());
    assert_source_backed_child(&module, parameter.pattern());
    assert_source_backed_child(&module, parameter.ty());
    assert!(
        module
            .arenas()
            .scopes()
            .resolve(module.slots(), predicate.callable_scope())
            .unwrap()
            .locals()
            .is_empty()
    );
}

fn rebuild_predicate_return(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::ItemId,
    return_type: TypeId,
) {
    let (original, predicate) = {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
        let HirItemKind::Predicate(predicate) = original.kind() else {
            panic!("final Predicate item")
        };
        (original.clone(), predicate.clone())
    };
    let signature = HirCallableSignature::try_new(
        predicate.generic_parameters().into(),
        predicate.parameters().into(),
        predicate.where_predicates().into(),
        predicate.requires().into(),
        predicate.ensures().into(),
        return_type,
    )
    .unwrap();
    let scopes = HirContractScopes::try_new(
        predicate.callable_scope(),
        predicate.requires_scope(),
        predicate.ensures_scope(),
    )
    .unwrap();
    let predicate = HirPredicate::try_new(
        predicate.name().clone(),
        signature,
        predicate.body().clone(),
        scopes,
    )
    .unwrap();
    let replacement = HirItem::try_new_with_state(
        owner,
        original.scope(),
        original.prefix().clone(),
        HirItemKind::Predicate(predicate),
        Box::new([]),
        *original.state(),
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
}

#[derive(Clone, Copy)]
enum ParameterLocalTamper {
    Name(&'static str),
    Generation(LocalGeneration),
    Mutable(bool),
}

fn tamper_first_parameter_local(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::ItemId,
    tamper: ParameterLocalTamper,
) {
    let (local, payload) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Predicate(predicate) = item.kind() else {
            panic!("final Predicate item")
        };
        let local = predicate.parameters()[0].locals()[0];
        let payload = arenas
            .locals()
            .resolve_staged(slots, local)
            .unwrap()
            .clone();
        (local, payload)
    };
    let mut name = payload.name().clone();
    let mut generation = payload.generation();
    let mut mutable = payload.is_mutable_binding();
    match tamper {
        ParameterLocalTamper::Name(replacement) => {
            name = HirName::try_new(replacement.into()).unwrap();
        }
        ParameterLocalTamper::Generation(replacement) => generation = replacement,
        ParameterLocalTamper::Mutable(replacement) => mutable = replacement,
    }
    let replacement = crate::scope::HirLocal::try_new(
        payload.scope(),
        payload.kind(),
        name,
        generation,
        payload.pattern(),
        payload.annotation(),
        mutable,
        payload.is_poisoned(),
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .locals()
        .revise_finalized(slots, local, replacement)
        .unwrap();
}

#[test]
fn canonical_predicate_freezes_signature_contracts_assertion_body_and_synthetic_owners() {
    let source = concat!(
        "pub predicate ordered<T>((left, right): (T, T), cmp: Comparator<T>)\n",
        "where T: Ord\n",
        "requires cmp.is_total()\n",
        "ensures result\n",
        "{\n",
        "    assert.prove(cmp.ready())\n",
        "    cmp.compare(left, right) <= 0\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-predicate-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, predicate) = predicate(&module, 0);
    let body_scope = assert_predicate_body_scope(&module, &parsed, 0, owner, predicate);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    assert!(matches!(
        predicate.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "ordered"
    ));
    assert_eq!(predicate.generic_parameters().len(), 1);
    assert_eq!(predicate.parameters().len(), 2);
    assert_eq!(predicate.where_predicates().len(), 1);
    assert_eq!(predicate.requires().len(), 1);
    assert_eq!(predicate.ensures().len(), 1);
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());

    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), predicate.callable_scope())
        .unwrap();
    let requires = module
        .arenas()
        .scopes()
        .resolve(module.slots(), predicate.requires_scope())
        .unwrap();
    let ensures = module
        .arenas()
        .scopes()
        .resolve(module.slots(), predicate.ensures_scope())
        .unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item.scope()));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(callable.children()[2], body_scope);
    assert!(requires.locals().is_empty());
    let [result] = ensures.locals() else {
        panic!("exact postcondition result local")
    };
    let result = module
        .arenas()
        .locals()
        .resolve(module.slots(), *result)
        .unwrap();
    assert_eq!(result.kind(), HirLocalKind::PostconditionResult);
    assert_eq!(result.name().as_str(), "result");
    assert_eq!(result.generation(), LocalGeneration::FIRST);
    assert_eq!(result.annotation(), Some(predicate.return_type()));

    let bool_type = module
        .arenas()
        .types()
        .resolve(module.slots(), predicate.return_type())
        .unwrap();
    assert_eq!(bool_type.scope(), predicate.callable_scope());
    assert_eq!(bool_type.state(), &HirPoisonState::Clean);
    assert!(matches!(
        bool_type.kind(),
        HirTypeKind::Path(path)
            if path.root() == HirPathRoot::ImplicitCrate
                && path_spellings(path) == ["Bool"]
    ));
    let return_metadata = module.slots().resolve(predicate.return_type()).unwrap();
    assert!(matches!(
        return_metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Item(owner)
                && key.role() == SyntheticRole::PredicateBoolReturn
    ));
    let parameter_end = source.find("\nwhere").unwrap();
    assert_eq!(
        source_range(&module, predicate.return_type()),
        SourceRange::new(parameter_end, parameter_end)
    );
    assert_eq!(
        source_range(&module, predicate.requires_scope()),
        SourceRange::new(
            source.find("requires").unwrap(),
            source.find("requires").unwrap()
        )
    );
    assert_eq!(
        source_range(&module, predicate.ensures_scope()),
        SourceRange::new(
            source.find("ensures").unwrap(),
            source.find("ensures").unwrap()
        )
    );

    let HirPredicateBody::Block {
        scope,
        statements,
        tail,
    } = predicate.body()
    else {
        panic!("Predicate block body")
    };
    assert_eq!(*scope, body_scope);
    assert_eq!(statements.len(), 1);
    let assertion = module
        .arenas()
        .statements()
        .resolve(module.slots(), statements[0])
        .unwrap();
    assert_eq!(assertion.scope(), *scope);
    assert!(matches!(
        assertion.kind(),
        HirStmtKind::Assertion {
            mode: HirAssertionMode::Resolved(AssertionMode::Prove),
            conditions,
        } if conditions.len() == 1
    ));
    assert_eq!(
        assertion.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::PredicateAssertionNotAllowed)
    );
    assert_eq!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), *tail)
            .unwrap()
            .scope(),
        *scope
    );
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn predicate_lowering_allocates_headers_contracts_and_body_in_the_accepted_order() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-predicate-allocation-order",
        concat!(
            "predicate ordered<T: Bound>(value: Input)\n",
            "where T: Other\n",
            "requires ready(value)\n",
            "ensures result\n",
            "= true\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, predicate) = predicate(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);

    let [generic] = predicate.generic_parameters() else {
        panic!("one generic parameter")
    };
    let [generic_bound] = generic.bounds() else {
        panic!("one generic bound")
    };
    let [parameter] = predicate.parameters() else {
        panic!("one fixed parameter")
    };
    let [parameter_local] = parameter.locals() else {
        panic!("one fixed-parameter local")
    };
    let [where_predicate] = predicate.where_predicates() else {
        panic!("one where predicate")
    };
    let [where_bound] = where_predicate.bounds() else {
        panic!("one where bound")
    };
    let [requires] = predicate.requires() else {
        panic!("one requires condition")
    };
    let [ensures] = predicate.ensures() else {
        panic!("one ensures condition")
    };
    let [result_local] = module
        .arenas()
        .scopes()
        .resolve(module.slots(), predicate.ensures_scope())
        .unwrap()
        .locals()
    else {
        panic!("one postcondition result local")
    };
    let HirPredicateBody::Expression {
        scope: body_scope,
        expression: body,
    } = predicate.body()
    else {
        panic!("expression Predicate body")
    };

    let slots = [
        owner.raw().slot().get(),
        predicate.callable_scope().raw().slot().get(),
        predicate.requires_scope().raw().slot().get(),
        predicate.ensures_scope().raw().slot().get(),
        body_scope.raw().slot().get(),
        generic_bound.raw().slot().get(),
        parameter.ty().raw().slot().get(),
        parameter.pattern().raw().slot().get(),
        parameter_local.raw().slot().get(),
        predicate.return_type().raw().slot().get(),
        where_predicate.subject().raw().slot().get(),
        where_bound.raw().slot().get(),
        requires.raw().slot().get(),
        result_local.raw().slot().get(),
        ensures.raw().slot().get(),
        body.raw().slot().get(),
    ];
    assert!(
        slots.windows(2).all(|pair| pair[0] < pair[1]),
        "Predicate allocation order diverged from the accepted transaction: {slots:?}"
    );
}

#[test]
fn predicate_missing_and_omitted_tails_retain_typed_recovery_while_scoped_expression_publishes() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-predicate-body-matrix",
        concat!(
            "predicate missing(value: Bool)\n",
            "predicate omitted() { assert.check(true) }\n",
            "predicate scoped(value: Bool) = if value { true } else { false }\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (missing_owner, missing_item, missing) = predicate(&module, 0);
    let missing_scope = assert_predicate_body_scope(&module, &parsed, 0, missing_owner, missing);
    assert_eq!(
        missing_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert!(matches!(
        missing.body(),
        HirPredicateBody::Error { scope, .. } if *scope == missing_scope
    ));

    let (omitted_owner, omitted_item, omitted) = predicate(&module, 1);
    let omitted_scope = assert_predicate_body_scope(&module, &parsed, 1, omitted_owner, omitted);
    assert_eq!(
        omitted_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let HirPredicateBody::Block { scope, tail, .. } = omitted.body() else {
        panic!("omitted Predicate block")
    };
    assert_eq!(*scope, omitted_scope);
    assert!(module.slots().resolve(*tail).unwrap().is_poisoned());

    let (scoped_owner, scoped_item, scoped) = predicate(&module, 2);
    let scoped_body_scope = assert_predicate_body_scope(&module, &parsed, 2, scoped_owner, scoped);
    assert_eq!(scoped_item.state(), &HirItemPoisonState::Clean);
    let HirPredicateBody::Expression { scope, expression } = scoped.body() else {
        panic!("scoped Predicate expression")
    };
    assert_eq!(*scope, scoped_body_scope);
    assert_eq!(scoped.body().scope(), *scope);
    assert!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), *expression)
            .is_ok()
    );
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), scoped.callable_scope())
        .unwrap();
    assert!(callable.children().len() > 2);
}

#[test]
fn predicate_freeze_rejects_bool_contract_result_and_assertion_tampering_atomically() {
    let source = concat!(
        "predicate guarded(value: Bool)\n",
        "requires value\n",
        "ensures result\n",
        "{\n",
        "    assert.prove(value)\n",
        "    value\n",
        "}\n",
    );

    assert_predicate_freeze_rejects("bool-name", source, |_, transaction, owner| {
        let (return_type, scope) = {
            let (slots, arenas) = transaction.storage_mut();
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::Predicate(predicate) = item.kind() else {
                panic!("final Predicate item")
            };
            (predicate.return_type(), predicate.callable_scope())
        };
        let path = HirPath::try_new(
            HirPathRoot::ImplicitCrate,
            Box::new([HirPathSegment::Identifier(
                HirName::try_new("NotBool".into()).unwrap(),
            )]),
        )
        .unwrap();
        let replacement = HirType::try_new(
            return_type,
            HirTypeKind::Path(path),
            scope,
            HirPoisonState::Clean,
            transaction,
        )
        .unwrap();
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .types()
            .revise_finalized(slots, return_type, replacement)
            .unwrap();
    });

    assert_predicate_freeze_rejects("contract-scope", source, |_, transaction, owner| {
        let requires_scope = {
            let (slots, arenas) = transaction.storage_mut();
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::Predicate(predicate) = item.kind() else {
                panic!("final Predicate item")
            };
            predicate.requires_scope()
        };
        let replacement = {
            let (slots, arenas) = transaction.storage_mut();
            let original = arenas
                .scopes()
                .resolve_staged(slots, requires_scope)
                .unwrap();
            HirScope::try_new(
                requires_scope.module(),
                HirScopeKind::Block,
                original.parent(),
                *original.owner(),
                original.children().into(),
                original.locals().into(),
            )
            .unwrap()
        };
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .scopes()
            .revise_finalized(slots, requires_scope, replacement)
            .unwrap();
    });

    assert_predicate_freeze_rejects("result-generation", source, |_, transaction, owner| {
        let (local, payload) = {
            let (slots, arenas) = transaction.storage_mut();
            let ensures_scope = {
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Predicate(predicate) = item.kind() else {
                    panic!("final Predicate item")
                };
                predicate.ensures_scope()
            };
            let scope = arenas
                .scopes()
                .resolve_staged(slots, ensures_scope)
                .unwrap();
            let local = scope.locals()[0];
            let payload = arenas
                .locals()
                .resolve_staged(slots, local)
                .unwrap()
                .clone();
            (local, payload)
        };
        let replacement = crate::scope::HirLocal::try_new(
            payload.scope(),
            payload.kind(),
            payload.name().clone(),
            LocalGeneration::try_new(2).unwrap(),
            payload.pattern(),
            payload.annotation(),
            payload.is_mutable_binding(),
            payload.is_poisoned(),
        )
        .unwrap();
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .locals()
            .revise_finalized(slots, local, replacement)
            .unwrap();
    });

    assert_predicate_freeze_rejects("assertion-mode", source, |_, transaction, owner| {
        let statement = {
            let (slots, arenas) = transaction.storage_mut();
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::Predicate(predicate) = item.kind() else {
                panic!("final Predicate item")
            };
            let HirPredicateBody::Block { statements, .. } = predicate.body() else {
                panic!("Predicate block")
            };
            statements[0]
        };
        let replacement = {
            let (slots, arenas) = transaction.storage_mut();
            let original = arenas
                .statements()
                .resolve_staged(slots, statement)
                .unwrap();
            let HirStmtKind::Assertion { conditions, .. } = original.kind() else {
                panic!("Assertion statement")
            };
            HirStmt::try_new_with_state(
                original.scope(),
                HirStmtKind::Assertion {
                    mode: HirAssertionMode::Resolved(AssertionMode::Check),
                    conditions: conditions.clone(),
                },
                original.state().clone(),
            )
            .unwrap()
        };
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .statements()
            .revise_finalized(slots, statement, replacement)
            .unwrap();
    });
}

#[test]
fn predicate_freeze_rejects_wrong_synthetic_return_key_site_and_authored_return_allocation() {
    let source = "predicate guarded(value: Bool) = value\n";
    assert_predicate_freeze_rejects("bool-key-site", source, |parsed, transaction, owner| {
        let callable_scope = {
            let (slots, arenas) = transaction.storage_mut();
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::Predicate(predicate) = item.kind() else {
                panic!("final Predicate item")
            };
            predicate.callable_scope()
        };
        let key = SyntheticKey::try_new(
            SyntheticOwner::Item(owner),
            SyntheticRole::ProofUnitReturn,
            0,
        )
        .unwrap();
        let reservation = {
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .types()
                .reserve_synthetic(
                    slots,
                    key,
                    HirSourceSite::Span(parsed.root_syntax().source_span()),
                )
                .unwrap()
        };
        let replacement_id = reservation.id();
        let path = HirPath::try_new(
            HirPathRoot::ImplicitCrate,
            Box::new([HirPathSegment::Identifier(
                HirName::try_new("Bool".into()).unwrap(),
            )]),
        )
        .unwrap();
        let payload = HirType::try_new(
            replacement_id,
            HirTypeKind::Path(path),
            callable_scope,
            HirPoisonState::Clean,
            transaction,
        )
        .unwrap();
        {
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .types()
                .finalize(slots, reservation, payload)
                .unwrap();
        }
        rebuild_predicate_return(transaction, owner, replacement_id);
    });

    let authored = "predicate guarded(value: Bool) -> Bool = value\n";
    assert_predicate_freeze_rejects(
        "authored-return-owner",
        authored,
        |parsed, transaction, owner| {
            let callable_scope = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Predicate(predicate) = item.kind() else {
                    panic!("final Predicate item")
                };
                predicate.callable_scope()
            };
            let attached = parsed
                .tree()
                .items()
                .unwrap()
                .into_iter()
                .find_map(|item| match item {
                    TypedItemNode::Predicate(predicate) => predicate.semantics().ok(),
                    _ => None,
                })
                .unwrap();
            transaction
                .lower_attached_type(attached.authored_return().unwrap().ty(), callable_scope)
                .unwrap();
        },
    );
}

#[test]
fn predicate_freeze_rejects_parameter_local_payload_tampering() {
    let source = "predicate guarded(value: Bool) = value\n";
    assert_predicate_freeze_rejects("parameter-local-name", source, |_, transaction, owner| {
        tamper_first_parameter_local(transaction, owner, ParameterLocalTamper::Name("renamed"));
    });
    assert_predicate_freeze_rejects(
        "parameter-local-generation",
        source,
        |_, transaction, owner| {
            tamper_first_parameter_local(
                transaction,
                owner,
                ParameterLocalTamper::Generation(LocalGeneration::try_new(2).unwrap()),
            );
        },
    );
    assert_predicate_freeze_rejects(
        "parameter-local-mutability",
        source,
        |_, transaction, owner| {
            tamper_first_parameter_local(transaction, owner, ParameterLocalTamper::Mutable(true));
        },
    );
    assert_predicate_freeze_rejects(
        "forbidden-parameter-local-mutability",
        "predicate guarded(mut value: Bool) = value\n",
        |_, transaction, owner| {
            tamper_first_parameter_local(transaction, owner, ParameterLocalTamper::Mutable(false));
        },
    );
}

#[test]
fn predicate_parameter_local_poison_is_rederived_and_sealed() {
    let source = "predicate duplicate((same, same): (Bool, Bool)) = same\n";
    let parsed = parse(
        "arcweft-test://proof/final-hir-predicate-parameter-poison",
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, predicate) = predicate(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    let locals = predicate.parameters()[0].locals();
    assert_eq!(locals.len(), 2);
    let first = module
        .arenas()
        .locals()
        .resolve(module.slots(), locals[0])
        .unwrap();
    let duplicate = module
        .arenas()
        .locals()
        .resolve(module.slots(), locals[1])
        .unwrap();
    assert_eq!(first.name().as_str(), "same");
    assert_eq!(duplicate.name().as_str(), "same");
    assert_eq!(first.generation(), LocalGeneration::FIRST);
    assert_eq!(duplicate.generation(), LocalGeneration::FIRST);
    assert!(!first.is_poisoned());
    assert!(duplicate.is_poisoned());

    let clean_source = "predicate guarded(value: Bool) = value\n";
    let parsed = parse(
        "arcweft-test://proof/final-hir-predicate-parameter-poison-seal",
        clean_source,
    );
    let key = module_key(&parsed);
    let database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    let (local, payload) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Predicate(predicate) = item.kind() else {
            panic!("final Predicate item")
        };
        let local = predicate.parameters()[0].locals()[0];
        let payload = arenas
            .locals()
            .resolve_staged(slots, local)
            .unwrap()
            .clone();
        (local, payload)
    };
    let poisoned = crate::scope::HirLocal::try_new(
        payload.scope(),
        payload.kind(),
        payload.name().clone(),
        payload.generation(),
        payload.pattern(),
        payload.annotation(),
        payload.is_mutable_binding(),
        true,
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    assert!(
        arenas
            .locals()
            .revise_finalized(slots, local, poisoned)
            .is_err(),
        "a finalized Local slot must reject a contradictory poison view"
    );
}

#[test]
fn predicate_parameter_policy_retains_typed_locals_and_poisoned_owners() {
    let source = concat!(
        "predicate clean(value: Bool) = value\n",
        "predicate mutable(mut value: Bool) = value\n",
        "predicate reserved(result: Bool) = result\n",
        "predicate refutable(.Some(value): Option<Bool>) = value\n",
    );
    let parsed = parse(
        "arcweft-test://proof/final-hir-predicate-parameter-policy",
        source,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (_, clean_item, clean) = predicate(&module, 0);
    let (_, mutable_item, mutable) = predicate(&module, 1);
    let (_, reserved_item, reserved) = predicate(&module, 2);
    let (_, refutable_item, refutable) = predicate(&module, 3);
    assert_eq!(clean_item.state(), &HirItemPoisonState::Clean);
    for item in [mutable_item, reserved_item, refutable_item] {
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
        );
    }

    let local = |predicate: &HirPredicate| {
        module
            .arenas()
            .locals()
            .resolve(module.slots(), predicate.parameters()[0].locals()[0])
            .unwrap()
    };
    assert!(!local(clean).is_poisoned());
    assert!(!local(clean).is_mutable_binding());
    assert!(local(mutable).is_poisoned());
    assert!(local(mutable).is_mutable_binding());
    assert!(local(reserved).is_poisoned());
    assert_eq!(local(reserved).name().as_str(), "result");
    assert!(local(refutable).is_poisoned());
}

#[test]
fn predicate_let_policy_propagates_local_poison_to_statements_and_item() {
    let source = concat!(
        "predicate bindings(input: Bool) {\n",
        "    let clean = input;\n",
        "    let mut mutable = input;\n",
        "    let result = input;\n",
        "    let .Some(value) = input;\n",
        "    input\n",
        "}\n",
    );
    let parsed = parse(
        "arcweft-test://proof/final-hir-predicate-let-policy",
        source,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, predicate) = predicate(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let HirPredicateBody::Block { statements, .. } = predicate.body() else {
        panic!("Predicate block body")
    };
    assert_eq!(statements.len(), 4);
    for (ordinal, statement) in statements.iter().copied().enumerate() {
        let statement = module
            .arenas()
            .statements()
            .resolve(module.slots(), statement)
            .unwrap();
        let HirStmtKind::Let { locals, .. } = statement.kind() else {
            panic!("pure let statement")
        };
        let [local] = locals.as_ref() else {
            panic!("one exact let Local")
        };
        let local = module
            .arenas()
            .locals()
            .resolve(module.slots(), *local)
            .unwrap();
        if ordinal == 0 {
            assert_eq!(statement.state(), &HirStmtPoisonState::Clean);
            assert!(!local.is_poisoned());
        } else {
            assert_eq!(
                statement.state(),
                &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                    role: crate::stmt::HirStmtChildRole::Pattern,
                })
            );
            assert!(local.is_poisoned());
        }
    }
}

#[test]
fn predicate_freeze_rejects_callable_children_outside_source_order() {
    let source = "predicate scoped(value: Bool) = value\n";
    assert_predicate_freeze_rejects("callable-child-order", source, |_, transaction, owner| {
        let (callable_scope, reordered) = {
            let (slots, arenas) = transaction.storage_mut();
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::Predicate(predicate) = item.kind() else {
                panic!("final Predicate item")
            };
            (
                predicate.callable_scope(),
                Box::new([
                    predicate.body().scope(),
                    predicate.requires_scope(),
                    predicate.ensures_scope(),
                ]),
            )
        };
        let (slots, arenas) = transaction.storage_mut();
        let callable = arenas
            .scopes()
            .resolve_staged(slots, callable_scope)
            .unwrap()
            .clone();
        let replacement = callable
            .try_with_members(reordered, callable.locals().into())
            .unwrap();
        arenas
            .scopes()
            .revise_finalized(slots, callable_scope, replacement)
            .unwrap();
    });
}

#[test]
fn incremental_predicate_proof_reorder_keeps_body_scopes_but_reorders_scope_children() {
    let name = SourceName::path("proof/predicate-proof-scope-order.arcw");
    let document_id = "arcweft-test://proof/predicate-proof-scope-order";
    let initial_source = "predicate First() = true\nproof Second() = ()\n";
    let reordered_source = "proof Second() = ()\npredicate First() = true\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let predicate_item = first.source_ordered_items()[0];
    let proof_item = first.source_ordered_items()[1];
    let (predicate_callable, predicate_body) = {
        let item = first
            .arenas()
            .items()
            .resolve(first.slots(), predicate_item)
            .unwrap();
        let HirItemKind::Predicate(predicate) = item.kind() else {
            panic!("first item must be Predicate")
        };
        (predicate.callable_scope(), predicate.body().scope())
    };
    let (proof_callable, proof_body) = {
        let item = first
            .arenas()
            .items()
            .resolve(first.slots(), proof_item)
            .unwrap();
        let HirItemKind::Proof(proof) = item.kind() else {
            panic!("second item must be Proof")
        };
        (proof.callable_scope(), proof.body().scope())
    };

    let reordered = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, initial_source.len()))
                    .unwrap(),
                reordered_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second = lower(&mut database, &reordered, &key);
    assert_eq!(second.source_ordered_items(), [proof_item, predicate_item]);

    let (second_proof_callable, second_proof_body) = {
        let item = second
            .arenas()
            .items()
            .resolve(second.slots(), proof_item)
            .unwrap();
        let HirItemKind::Proof(proof) = item.kind() else {
            panic!("retained Proof item")
        };
        (proof.callable_scope(), proof.body().scope())
    };
    let (second_predicate_callable, second_predicate_body) = {
        let item = second
            .arenas()
            .items()
            .resolve(second.slots(), predicate_item)
            .unwrap();
        let HirItemKind::Predicate(predicate) = item.kind() else {
            panic!("retained Predicate item")
        };
        (predicate.callable_scope(), predicate.body().scope())
    };
    assert_eq!(second_proof_callable, proof_callable);
    assert_eq!(second_proof_body, proof_body);
    assert_eq!(second_predicate_callable, predicate_callable);
    assert_eq!(second_predicate_body, predicate_body);

    let root_scope = second
        .arenas()
        .items()
        .resolve(second.slots(), proof_item)
        .unwrap()
        .scope();
    assert_eq!(
        second
            .arenas()
            .scopes()
            .resolve(second.slots(), root_scope)
            .unwrap()
            .children(),
        [proof_callable, predicate_callable]
    );
    let raw_backlink_order = second
        .arenas()
        .scopes()
        .try_iter(second.slots())
        .unwrap()
        .filter_map(|(scope, payload)| (payload.parent() == Some(root_scope)).then_some(scope))
        .collect::<Vec<_>>();
    assert_eq!(raw_backlink_order, [predicate_callable, proof_callable]);
}
