use super::*;

use std::fmt::Write as _;

use crate::item::{
    HirFunctionBody, HirImplItem, HirImplMember, HirMethodParameter, HirMethodParameterGroup,
    HirMethodReceiver, HirMethodReceiverKind, HirTraitFunction, HirTraitItem, HirTraitMember,
};
use crate::stmt::HirStmtKind;

use super::super::trait_impl::{preflight_impl_members, preflight_trait_members};

fn assert_inline_member_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    assert_inline_member_freeze_rejects_with_source(case, source, |_, transaction, owner| {
        tamper(transaction, owner);
    });
}

fn assert_inline_member_freeze_rejects_with_source(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&ParsedSource, &mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-trait-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction.lower_parsed_source_items(&parsed).unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&parsed, &mut transaction, owner);
    assert!(
        matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ),
        "Trait/Impl freeze accepted {case}"
    );
    assert!(database.current(&key).is_none());
}

fn revise_impl_members(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::ItemId,
    mutate: impl FnOnce(&mut Vec<HirImplMember>),
) {
    let (scope, prefix, state, item_members, generics, trait_ref, target, predicates, mut members) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Impl(declaration) = item.kind() else {
            panic!("final Impl item")
        };
        (
            item.scope(),
            item.prefix().clone(),
            *item.state(),
            item.members().into(),
            declaration.generic_parameters().into(),
            declaration.trait_ref(),
            declaration.target(),
            declaration.where_predicates().into(),
            declaration.members().to_vec(),
        )
    };
    mutate(&mut members);
    let declaration = HirImplItem::try_new(
        owner.module(),
        generics,
        trait_ref,
        target,
        predicates,
        members.into_boxed_slice(),
    )
    .unwrap();
    let replacement = HirItem::try_new_with_state(
        owner,
        scope,
        prefix,
        HirItemKind::Impl(declaration),
        item_members,
        state,
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
}

fn revise_trait_members(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::ItemId,
    mutate: impl FnOnce(&mut Vec<HirTraitMember>),
) {
    let (scope, prefix, state, item_members, name, generics, supertraits, predicates, mut members) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Trait(declaration) = item.kind() else {
            panic!("final Trait item")
        };
        (
            item.scope(),
            item.prefix().clone(),
            *item.state(),
            item.members().into(),
            declaration.name().clone(),
            declaration.generic_parameters().into(),
            declaration.supertraits().into(),
            declaration.where_predicates().into(),
            declaration.members().to_vec(),
        )
    };
    mutate(&mut members);
    let declaration = HirTraitItem::try_new(
        owner.module(),
        name,
        generics,
        supertraits,
        predicates,
        members.into_boxed_slice(),
    )
    .unwrap();
    let replacement = HirItem::try_new_with_state(
        owner,
        scope,
        prefix,
        HirItemKind::Trait(declaration),
        item_members,
        state,
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
}

fn revise_first_trait_receiver_local(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::ItemId,
    mutate: impl FnOnce(HirLocal, Option<crate::identity::TypeId>) -> HirLocal,
) {
    let (local, return_type) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Trait(declaration) = item.kind() else {
            panic!("final Trait item")
        };
        let HirTraitMember::Function(method) = &declaration.members()[0] else {
            panic!("Trait method")
        };
        let HirMethodParameter::Receiver(receiver) = &method.parameter_groups()[0].parameters()[0]
        else {
            panic!("method receiver")
        };
        (receiver.locals()[0], method.return_type())
    };
    let original = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .locals()
            .resolve_staged(slots, local)
            .unwrap()
            .clone()
    };
    let replacement = mutate(original, return_type);
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .locals()
        .revise_finalized(slots, local, replacement)
        .unwrap();
}

fn swap_scope_children(
    transaction: &mut StagedHirModuleTransaction<'_>,
    scope: ScopeId,
    left: usize,
    right: usize,
) {
    let original = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .scopes()
            .resolve_staged(slots, scope)
            .unwrap()
            .clone()
    };
    let mut children = original.children().to_vec();
    children.swap(left, right);
    let replacement = original
        .try_with_members(children.into_boxed_slice(), original.locals().into())
        .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .scopes()
        .revise_finalized(slots, scope, replacement)
        .unwrap();
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the canonical Trait/Impl test asserts the complete inline member, receiver, scope, and body matrix"
)]
fn trait_and_impl_lower_to_distinct_inline_members_and_one_scope_per_method() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-trait-impl-clean",
        concat!(
            "trait SourceLike<T>: Base + Iterable<T> where T: Copyable {\n",
            "    type Item<U> = Result<U, Error>\n",
            "    #[audit(callback = |value| value)]\n",
            "    fn current(&self)(fallback: T) -> T {\n",
            "        let chosen = fallback;\n",
            "        chosen\n",
            "    }\n",
            "    fn required(mut self) -> T\n",
            "}\n",
            "impl<T> SourceLike<T> for Box<T> where T: Copyable {\n",
            "    type Item<U> = U\n",
            "    fn current(&mut self)(fallback: T) -> T {\n",
            "        let chosen = fallback;\n",
            "        chosen\n",
            "    }\n",
            "    fn required(self) -> T\n",
            "}\n",
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

    let trait_owner = module.source_ordered_items()[0];
    let trait_item = resolve_item(&module, 0);
    let HirItemKind::Trait(trait_declaration) = trait_item.kind() else {
        panic!("first item must be a Trait")
    };
    assert_eq!(trait_item.state(), &HirItemPoisonState::Clean);
    assert!(trait_item.members().is_empty());
    assert!(module.declaration_members().arena(trait_owner).is_none());
    assert_eq!(trait_declaration.supertraits().len(), 2);
    let [
        HirTraitMember::AssociatedType(_),
        HirTraitMember::Function(current),
        HirTraitMember::Function(required),
    ] = trait_declaration.members()
    else {
        panic!("Trait must retain its exact inline member order")
    };
    let module_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), trait_item.scope())
        .unwrap();
    assert_eq!(module_scope.children().len(), 5);
    assert_eq!(module_scope.children()[0], current.callable_scope());
    assert!(matches!(
        module
            .arenas()
            .scopes()
            .resolve(module.slots(), module_scope.children()[1])
            .unwrap()
            .owner(),
        HirScopeOwner::Expr(_)
    ));
    assert_eq!(module_scope.children()[2], required.callable_scope());
    assert_method(
        &module,
        trait_owner,
        trait_item.scope(),
        current.callable_scope(),
        current.parameter_groups(),
        current.body(),
        HirMethodReceiverKind::SharedReference,
        true,
    );
    assert_method(
        &module,
        trait_owner,
        trait_item.scope(),
        required.callable_scope(),
        required.parameter_groups(),
        required.body(),
        HirMethodReceiverKind::Owned,
        false,
    );

    let impl_owner = module.source_ordered_items()[1];
    let impl_item = resolve_item(&module, 1);
    let HirItemKind::Impl(impl_declaration) = impl_item.kind() else {
        panic!("second item must be an Impl")
    };
    assert_eq!(impl_item.state(), &HirItemPoisonState::Clean);
    assert!(impl_item.members().is_empty());
    assert!(module.declaration_members().arena(impl_owner).is_none());
    assert!(impl_declaration.trait_ref().is_some());
    let [
        HirImplMember::AssociatedType(_),
        HirImplMember::Function(current),
        HirImplMember::Function(required),
    ] = impl_declaration.members()
    else {
        panic!("Impl must retain its exact inline member order")
    };
    assert_eq!(module_scope.children()[3], current.callable_scope());
    assert_eq!(module_scope.children()[4], required.callable_scope());
    assert_method(
        &module,
        impl_owner,
        impl_item.scope(),
        current.callable_scope(),
        current.parameter_groups(),
        current.body(),
        HirMethodReceiverKind::MutableReference,
        true,
    );
    assert_method(
        &module,
        impl_owner,
        impl_item.scope(),
        required.callable_scope(),
        required.parameter_groups(),
        required.body(),
        HirMethodReceiverKind::Owned,
        false,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "the assertion receives the complete method owner/scope/payload expectation record"
)]
fn assert_method(
    module: &HirModule,
    owner: crate::identity::ItemId,
    item_scope: ScopeId,
    callable_scope: ScopeId,
    groups: &[crate::item::HirMethodParameterGroup],
    body: Option<&HirFunctionBody>,
    expected_receiver: HirMethodReceiverKind,
    has_body: bool,
) {
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), callable_scope)
        .unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item_scope));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    let [first, ..] = groups else {
        panic!("method must retain at least one parameter group")
    };
    let [HirMethodParameter::Receiver(receiver), ..] = first.parameters() else {
        panic!("method must retain its receiver as a dedicated parameter")
    };
    assert_eq!(receiver.kind(), expected_receiver);
    assert_eq!(receiver.locals().len(), 1);
    assert_eq!(callable.locals().first(), receiver.locals().first());

    match (has_body, body) {
        (
            true,
            Some(HirFunctionBody::Block {
                scope,
                statements,
                tail,
            }),
        ) => {
            assert_eq!(*scope, callable_scope);
            let [statement] = statements.as_ref() else {
                panic!("default method must retain its one statement")
            };
            assert!(matches!(
                module
                    .arenas()
                    .statements()
                    .resolve(module.slots(), *statement)
                    .unwrap()
                    .kind(),
                HirStmtKind::Let { .. }
            ));
            assert_eq!(
                module
                    .arenas()
                    .expressions()
                    .resolve(module.slots(), *tail)
                    .unwrap()
                    .scope(),
                callable_scope
            );
            assert_eq!(callable.locals().len(), 3);
        }
        (false, None) => assert_eq!(callable.locals().len(), 1),
        _ => panic!("method body ownership mismatch"),
    }
}

#[test]
fn trait_and_impl_recovery_stays_typed_and_preserves_following_members() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-trait-impl-recovery",
        concat!(
            "trait Broken {\n",
            "    const unsupported = 1\n",
            "    type Item\n",
            "}\n",
            "impl Broken for Target {\n",
            "    type Item\n",
            "    fn current(self) -> Item\n",
            "}\n",
        ),
    );
    assert_eq!(parsed.diagnostics().len(), 2);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let trait_owner = module.source_ordered_items()[0];
    let trait_item = resolve_item(&module, 0);
    let HirItemKind::Trait(trait_declaration) = trait_item.kind() else {
        panic!("recovered Trait item")
    };
    assert_eq!(
        trait_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert!(matches!(
        trait_declaration.members(),
        [HirTraitMember::Error, HirTraitMember::AssociatedType(_)]
    ));
    assert_item_owner_whole_recovery(&module, trait_owner);

    let impl_owner = module.source_ordered_items()[1];
    let impl_item = resolve_item(&module, 1);
    let HirItemKind::Impl(impl_declaration) = impl_item.kind() else {
        panic!("recovered Impl item")
    };
    assert_eq!(
        impl_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingType)
    );
    assert!(matches!(
        impl_declaration.members(),
        [HirImplMember::AssociatedType(_), HirImplMember::Function(_)]
    ));
    assert_item_owner_whole_recovery(&module, impl_owner);
}

#[test]
fn trait_receiver_kind_tampering_is_rejected_before_publication() {
    assert_inline_member_freeze_rejects(
        "receiver-tamper",
        "trait Readable { fn read(&self) -> Self }\n",
        |transaction, owner| {
            revise_trait_members(transaction, owner, |members| {
                let HirTraitMember::Function(method) = &members[0] else {
                    panic!("Trait method")
                };
                let mut groups = method.parameter_groups().to_vec();
                let mut parameters = groups[0].parameters().to_vec();
                let HirMethodParameter::Receiver(receiver) = &parameters[0] else {
                    panic!("method receiver")
                };
                parameters[0] = HirMethodParameter::Receiver(
                    HirMethodReceiver::try_new(
                        HirMethodReceiverKind::MutableReference,
                        receiver.pattern(),
                        receiver.locals().into(),
                    )
                    .unwrap(),
                );
                groups[0] =
                    HirMethodParameterGroup::try_new(owner.module(), parameters.into_boxed_slice())
                        .unwrap();
                members[0] = HirTraitMember::Function(
                    HirTraitFunction::try_new(
                        owner.module(),
                        method.prefix().clone(),
                        method.name().clone(),
                        method.generic_parameters().into(),
                        groups.into_boxed_slice(),
                        method.where_predicates().into(),
                        method.return_type(),
                        method.callable_scope(),
                        method.body().cloned(),
                    )
                    .unwrap(),
                );
            });
        },
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Trait freeze test exhausts member and callable-scope corruption cases"
)]
fn trait_freeze_rejects_member_and_callable_scope_corruption() {
    assert_inline_member_freeze_rejects(
        "member-reorder",
        "trait Ordered { type First\n type Second\n }\n",
        |transaction, owner| {
            revise_trait_members(transaction, owner, |members| members.swap(0, 1));
        },
    );

    assert_inline_member_freeze_rejects(
        "impl-member-reorder",
        concat!(
            "impl Base for Target {\n",
            "    type First = I32\n",
            "    type Second = I64\n",
            "}\n",
        ),
        |transaction, owner| {
            revise_impl_members(transaction, owner, |members| members.swap(0, 1));
        },
    );

    assert_inline_member_freeze_rejects(
        "callable-scope-order",
        concat!(
            "trait Ordered {\n",
            "    fn first(&self) -> Self\n",
            "    fn second(&self) -> Self\n",
            "}\n",
        ),
        |transaction, owner| {
            let item_scope = {
                let (slots, arenas) = transaction.storage_mut();
                arenas.items().resolve_staged(slots, owner).unwrap().scope()
            };
            swap_scope_children(transaction, item_scope, 0, 1);
        },
    );

    assert_inline_member_freeze_rejects(
        "prefix-expression-method-scope-order",
        concat!(
            "trait OrderedPrefix {\n",
            "    #[audit(callback = |value| value)]\n",
            "    fn run(self) -> Self\n",
            "}\n",
        ),
        |transaction, owner| {
            let item_scope = {
                let (slots, arenas) = transaction.storage_mut();
                arenas.items().resolve_staged(slots, owner).unwrap().scope()
            };
            let child_count = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .scopes()
                    .resolve_staged(slots, item_scope)
                    .unwrap()
                    .children()
                    .len()
            };
            assert_eq!(
                child_count, 2,
                "member-prefix closure and method own two direct child scopes"
            );
            swap_scope_children(transaction, item_scope, 0, 1);
        },
    );

    assert_inline_member_freeze_rejects(
        "method-child-scope-order",
        concat!(
            "trait OrderedChildren {\n",
            "    fn run(\n",
            "        self,\n",
            "        first: Callback = |value| value,\n",
            "        second: Callback = |value| value,\n",
            "    ) -> Callback\n",
            "}\n",
        ),
        |transaction, owner| {
            let callable_scope = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Trait(declaration) = item.kind() else {
                    panic!("final Trait item")
                };
                let HirTraitMember::Function(method) = &declaration.members()[0] else {
                    panic!("Trait method")
                };
                method.callable_scope()
            };
            let child_count = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .scopes()
                    .resolve_staged(slots, callable_scope)
                    .unwrap()
                    .children()
                    .len()
            };
            assert_eq!(child_count, 2, "two default closures own two scopes");
            swap_scope_children(transaction, callable_scope, 0, 1);
        },
    );

    assert_inline_member_freeze_rejects_with_source(
        "unreferenced-item-owned-method-scope",
        "trait ExtraScope { fn run(self) -> Self }\n",
        |parsed, transaction, owner| {
            let (callable_scope, source) = {
                let items = parsed.items().unwrap();
                let [TypedItemNode::Trait(node)] = items.as_slice() else {
                    panic!("one attached Trait")
                };
                let attached = node.semantics().unwrap();
                let [arcweft_lang_syntax::attachment::AttachedTraitMember::Function(method)] =
                    attached.body().members()
                else {
                    panic!("one attached Trait method")
                };
                let return_type = method.authored_return().unwrap().ty().syntax().clone();
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Trait(declaration) = item.kind() else {
                    panic!("final Trait item")
                };
                let HirTraitMember::Function(method) = &declaration.members()[0] else {
                    panic!("final Trait method")
                };
                (method.callable_scope(), return_type)
            };
            let extra_scope = {
                let (slots, arenas) = transaction.storage_mut();
                let reservation = arenas
                    .scopes()
                    .reserve_source(
                        slots,
                        source.id(),
                        HirSourceSite::Span(source.source_span()),
                    )
                    .unwrap();
                assert!(reservation.is_first_touch());
                let extra_scope = reservation.id();
                let payload = HirScope::try_new(
                    owner.module(),
                    HirScopeKind::Block,
                    Some(callable_scope),
                    HirScopeOwner::Item(owner),
                    Box::new([]),
                    Box::new([]),
                )
                .unwrap();
                arenas
                    .scopes()
                    .finalize(slots, reservation, payload)
                    .unwrap();
                extra_scope
            };
            let original = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .scopes()
                    .resolve_staged(slots, callable_scope)
                    .unwrap()
                    .clone()
            };
            let mut children = original.children().to_vec();
            children.push(extra_scope);
            let replacement = original
                .try_with_members(children.into_boxed_slice(), original.locals().into())
                .unwrap();
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .scopes()
                .revise_finalized(slots, callable_scope, replacement)
                .unwrap();
        },
    );

    assert_inline_member_freeze_rejects(
        "callable-scope-owner",
        concat!(
            "trait First { fn read(&self) -> Self }\n",
            "trait Second {}\n",
        ),
        |transaction, owner| {
            let other_owner = transaction.source_ordered_items[1];
            let callable_scope = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Trait(declaration) = item.kind() else {
                    panic!("final Trait item")
                };
                let HirTraitMember::Function(method) = &declaration.members()[0] else {
                    panic!("Trait method")
                };
                method.callable_scope()
            };
            let original = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .scopes()
                    .resolve_staged(slots, callable_scope)
                    .unwrap()
                    .clone()
            };
            let replacement = HirScope::try_new(
                callable_scope.module(),
                original.kind(),
                original.parent(),
                HirScopeOwner::Item(other_owner),
                original.children().into(),
                original.locals().into(),
            )
            .unwrap();
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .scopes()
                .revise_finalized(slots, callable_scope, replacement)
                .unwrap();
        },
    );

    assert_inline_member_freeze_rejects(
        "receiver-local-annotation",
        "trait Mutable { fn update(mut self) -> Self }\n",
        |transaction, owner| {
            revise_first_trait_receiver_local(transaction, owner, |original, return_type| {
                HirLocal::try_new(
                    original.scope(),
                    original.kind(),
                    original.name().clone(),
                    original.generation(),
                    original.pattern(),
                    Some(return_type.expect("method return type")),
                    original.is_mutable_binding(),
                    original.is_poisoned(),
                )
                .unwrap()
            });
        },
    );

    assert_inline_member_freeze_rejects(
        "receiver-local-mutability",
        "trait Mutable { fn update(mut self) -> Self }\n",
        |transaction, owner| {
            revise_first_trait_receiver_local(transaction, owner, |original, _| {
                HirLocal::try_new(
                    original.scope(),
                    original.kind(),
                    original.name().clone(),
                    original.generation(),
                    original.pattern(),
                    original.annotation(),
                    !original.is_mutable_binding(),
                    original.is_poisoned(),
                )
                .unwrap()
            });
        },
    );

    assert_inline_member_freeze_rejects(
        "method-statement-order",
        concat!(
            "trait OrderedBody {\n",
            "    fn run(self) -> I32 {\n",
            "        let first = 1;\n",
            "        let second = 2;\n",
            "        second\n",
            "    }\n",
            "}\n",
        ),
        |transaction, owner| {
            revise_trait_members(transaction, owner, |members| {
                let HirTraitMember::Function(method) = &members[0] else {
                    panic!("Trait method")
                };
                let Some(HirFunctionBody::Block {
                    scope,
                    statements,
                    tail,
                }) = method.body()
                else {
                    panic!("method block")
                };
                let mut reordered = statements.to_vec();
                reordered.swap(0, 1);
                members[0] = HirTraitMember::Function(
                    HirTraitFunction::try_new(
                        owner.module(),
                        method.prefix().clone(),
                        method.name().clone(),
                        method.generic_parameters().into(),
                        method.parameter_groups().into(),
                        method.where_predicates().into(),
                        method.return_type(),
                        method.callable_scope(),
                        Some(HirFunctionBody::Block {
                            scope: *scope,
                            statements: reordered.into_boxed_slice(),
                            tail: *tail,
                        }),
                    )
                    .unwrap(),
                );
            });
        },
    );
}

#[test]
fn trait_and_impl_member_limits_accept_exact_and_reject_one_over_preflight() {
    let maximum = HirLimit::DeclarationMembers.maximum();
    let mut source = String::from("trait Large {\n");
    for ordinal in 0..maximum {
        writeln!(source, "    type T{ordinal}").unwrap();
    }
    source.push_str("}\n");
    let parsed = parse("arcweft-test://proof/final-hir-trait-member-limit", &source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);
    let HirItemKind::Trait(declaration) = item.kind() else {
        panic!("exact-limit Trait")
    };
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(declaration.members().len(), maximum);
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());

    assert!(preflight_trait_members(maximum).is_ok());
    assert!(preflight_impl_members(maximum).is_ok());
    let observed = maximum + 1;
    for result in [
        preflight_trait_members(observed),
        preflight_impl_members(observed),
    ] {
        let Err(HirLowerFailure::Limit(error)) = result else {
            panic!("one-over Trait/Impl member inventory must fail before lowering")
        };
        assert_eq!(error.limit(), HirLimit::DeclarationMembers);
        assert_eq!(error.observed(), observed);
        assert_eq!(error.maximum(), maximum);
    }
}
