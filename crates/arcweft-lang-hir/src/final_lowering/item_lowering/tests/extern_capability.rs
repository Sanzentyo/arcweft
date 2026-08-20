use super::*;

use std::fmt::Write as _;

use arcweft_lang_syntax::attachment::AttachedCapabilityMember;

use crate::identity::ItemId;
use crate::item::{
    HirCapabilityAssociatedType, HirCapabilityFunction, HirCapabilityMember,
    HirExternCapabilityItem, HirFunctionParameterGroup, HirParameter, HirVisibility,
};

use super::super::extern_capability::preflight_extern_capability_members;

fn capability(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirExternCapabilityItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::ExternCapability(capability) = item.kind() else {
        panic!("source-ordered item {ordinal} must be an ExternCapability")
    };
    (owner, item, capability)
}

fn lower_capability_case(case: &str, source: &str) -> Arc<HirModule> {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-extern-capability-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    lower(&mut database, &parsed, &key)
}

fn assert_capability_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut Vec<HirCapabilityMember>),
) {
    assert_capability_transaction_freeze_rejects(case, source, |transaction, owner| {
        let (scope, prefix, state, name, mut members) = {
            let (slots, arenas) = transaction.storage_mut();
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::ExternCapability(capability) = item.kind() else {
                panic!("final ExternCapability item")
            };
            (
                item.scope(),
                item.prefix().clone(),
                *item.state(),
                capability.name().clone(),
                capability.members().to_vec(),
            )
        };
        tamper(&mut members);
        let capability =
            HirExternCapabilityItem::try_new(owner.module(), name, members.into_boxed_slice())
                .unwrap();
        let replacement = HirItem::try_new_with_state(
            owner,
            scope,
            prefix,
            HirItemKind::ExternCapability(capability),
            Box::new([]),
            state,
        )
        .unwrap();
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .items()
            .revise_finalized(slots, owner, replacement)
            .unwrap();
    });
}

fn assert_capability_transaction_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-extern-capability-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction.lower_parsed_source_items(&parsed).unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&mut transaction, owner);
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the canonical capability test asserts the complete interleaved member/scope/source matrix"
)]
fn canonical_extern_capability_freezes_interleaved_members_and_callable_scope() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-extern-capability-clean",
        concat!(
            "/// Host boundary\n",
            "#[audit(external)]\n",
            "pub extern capability host {\n",
            "    /// Request payload\n",
            "    #[opaque]\n",
            "    pub type Request<T: Format> = Result<T, HostError>\n",
            "    pub fn send<T>(request: T = fallback())(retry: u32) -> Need<Result<T, HostError>>\n",
            "        effects { net.connect, net.send, }\n",
            "    type Response\n",
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
    let (owner, item, capability) = capability(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(item.prefix().visibility(), Some(HirVisibility::Public));
    assert_eq!(item.prefix().attributes().len(), 1);
    assert!(matches!(
        capability.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "host"
    ));
    let [request, send, response] = capability.members() else {
        panic!("three interleaved capability members")
    };
    let HirCapabilityMember::AssociatedType(request) = request else {
        panic!("first member must remain the Request associated type")
    };
    assert_eq!(request.prefix().visibility(), Some(HirVisibility::Public));
    assert_eq!(request.prefix().attributes().len(), 1);
    assert_eq!(request.generic_parameters().len(), 1);
    let request_value = request.value().expect("associated type default");
    assert_eq!(
        module
            .arenas()
            .types()
            .resolve(module.slots(), request_value)
            .unwrap()
            .scope(),
        item.scope()
    );

    let HirCapabilityMember::Function(send) = send else {
        panic!("second member must remain the send function")
    };
    assert_eq!(send.prefix().visibility(), Some(HirVisibility::Public));
    assert_eq!(send.generic_parameters().len(), 1);
    assert_eq!(
        send.parameter_groups()
            .iter()
            .map(|group| group.parameters().len())
            .collect::<Vec<_>>(),
        [1, 1]
    );
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), send.callable_scope())
        .unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item.scope()));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(callable.locals().len(), 2);
    let default = send.parameter_groups()[0].parameters()[0]
        .default()
        .expect("authored capability default");
    assert_eq!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), default)
            .unwrap()
            .scope(),
        send.callable_scope()
    );
    assert_eq!(send.effects().len(), 2);
    assert_eq!(
        item.kind().effect_expression_roots(),
        send.effects(),
        "associated types and non-effect members cannot enter the effect inventory"
    );
    for effect in send.effects() {
        assert_eq!(
            module
                .arenas()
                .expressions()
                .resolve(module.slots(), *effect)
                .unwrap()
                .scope(),
            send.callable_scope()
        );
    }
    let return_type = send.return_type().expect("authored capability return");
    assert_eq!(
        module
            .arenas()
            .types()
            .resolve(module.slots(), return_type)
            .unwrap()
            .scope(),
        send.callable_scope()
    );

    let HirCapabilityMember::AssociatedType(response) = response else {
        panic!("third member must remain the Response associated type")
    };
    assert!(response.value().is_none());
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_slot_whole(&module, &parsed, owner);

    let items = parsed.items().unwrap();
    let [TypedItemNode::ExternCapability(source_capability)] = items.as_slice() else {
        panic!("one attached ExternCapability item")
    };
    let attached = source_capability.semantics().unwrap();
    let [
        AttachedCapabilityMember::AssociatedType(source_request),
        AttachedCapabilityMember::Function(source_send),
        AttachedCapabilityMember::AssociatedType(source_response),
    ] = attached.body().members()
    else {
        panic!("attached members retain the same closed source order")
    };
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ItemId>(attached.syntax().id()),
        Some(owner)
    );
    for associated in [source_request, source_response] {
        assert_eq!(
            module
                .slots()
                .prepared_source_owner::<ItemId>(associated.syntax().id()),
            None
        );
        assert_eq!(
            module
                .slots()
                .prepared_source_owner::<ScopeId>(associated.syntax().id()),
            None
        );
    }
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ItemId>(source_send.syntax().id()),
        None
    );
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ScopeId>(source_send.syntax().id()),
        Some(send.callable_scope())
    );
}

#[test]
fn extern_capability_recovery_preserves_the_typed_family_and_primary_issue_order() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-extern-capability-recovery",
        concat!(
            "extern capability {}\n",
            "extern capability host\n",
            "extern capability recovered {\n",
            "    unsupported member\n",
            "    type\n",
            "    fn broken() effects net.read\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (_, missing_name, missing_name_payload) = capability(&module, 0);
    assert!(matches!(
        missing_name_payload.name(),
        HirRequiredName::Missing
    ));
    assert_eq!(
        missing_name.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingName)
    );

    let (_, missing_body, missing_body_payload) = capability(&module, 1);
    assert!(missing_body_payload.members().is_empty());
    assert_eq!(
        missing_body.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );

    let (_, recovered, recovered_payload) = capability(&module, 2);
    assert_eq!(recovered_payload.members().len(), 3);
    assert!(matches!(
        recovered_payload.members()[0],
        HirCapabilityMember::Error
    ));
    assert_eq!(
        recovered.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    for owner in module.source_ordered_items() {
        assert_item_owner_whole_recovery(&module, *owner);
    }
}

#[test]
fn extern_capability_member_recovery_matrix_retains_typed_children() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-extern-capability-member-recovery",
        concat!(
            "extern capability parameter_type {\n",
            "    fn send(value)\n",
            "}\n",
            "extern capability return_type {\n",
            "    fn send() ->\n",
            "}\n",
            "extern capability effects {\n",
            "    fn send() effects { net.send\n",
            "    fn finish()\n",
            "}\n",
            "extern capability outer {\n",
            "    type Request\n",
            "proof next() = ()\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (_, parameter_item, parameter_capability) = capability(&module, 0);
    assert_eq!(
        parameter_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingType)
    );
    let [HirCapabilityMember::Function(parameter_function)] = parameter_capability.members() else {
        panic!("missing parameter type remains one typed function member")
    };
    let parameter_type = parameter_function.parameter_groups()[0].parameters()[0].ty();
    assert!(
        module
            .slots()
            .resolve(parameter_type)
            .unwrap()
            .is_poisoned()
    );

    let (_, return_item, return_capability) = capability(&module, 1);
    assert_eq!(
        return_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingType)
    );
    let [HirCapabilityMember::Function(return_function)] = return_capability.members() else {
        panic!("missing return type remains one typed function member")
    };
    let return_type = return_function
        .return_type()
        .expect("authored return owns a recovered type slot");
    assert!(module.slots().resolve(return_type).unwrap().is_poisoned());

    let (_, effects_item, effects_capability) = capability(&module, 2);
    assert_eq!(
        effects_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let [
        HirCapabilityMember::Function(send),
        HirCapabilityMember::Function(finish),
    ] = effects_capability.members()
    else {
        panic!("unclosed effects recover before the next typed member")
    };
    assert_eq!(send.effects().len(), 1);
    assert!(finish.effects().is_empty());

    let (_, outer_item, outer_capability) = capability(&module, 3);
    assert_eq!(
        outer_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    assert!(matches!(
        outer_capability.members(),
        [HirCapabilityMember::AssociatedType(_)]
    ));
    assert!(matches!(
        resolve_item(&module, 4).kind(),
        HirItemKind::Proof(_)
    ));
}

#[test]
fn extern_capability_associated_type_recovery_stays_inline_and_typed() {
    let module = lower_capability_case("associated-missing-name", "extern capability c { type }\n");
    let (_, item, payload) = capability(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingName)
    );
    let [HirCapabilityMember::AssociatedType(associated)] = payload.members() else {
        panic!("missing associated-type name remains one typed inline member")
    };
    assert!(matches!(associated.name(), HirRequiredName::Missing));
    assert!(associated.value().is_none());

    let module = lower_capability_case(
        "associated-missing-value",
        "extern capability c { type T = }\n",
    );
    let (_, item, payload) = capability(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingType)
    );
    let [HirCapabilityMember::AssociatedType(associated)] = payload.members() else {
        panic!("missing associated-type value remains one typed inline member")
    };
    let value = associated
        .value()
        .expect("authored equals owns a recovered type slot");
    assert!(module.slots().resolve(value).unwrap().is_poisoned());
}

#[test]
fn extern_capability_function_recovery_stays_inline_and_typed() {
    let module = lower_capability_case("function-missing-name", "extern capability c { fn () }\n");
    let (_, item, payload) = capability(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingName)
    );
    let [HirCapabilityMember::Function(function)] = payload.members() else {
        panic!("missing function name remains one typed inline member")
    };
    assert!(matches!(function.name(), HirRequiredName::Missing));

    let module = lower_capability_case(
        "function-missing-default",
        "extern capability c { fn f(value: Int = ) }\n",
    );
    let (_, item, payload) = capability(&module, 0);
    assert!(item.state().is_poisoned());
    let [HirCapabilityMember::Function(function)] = payload.members() else {
        panic!("missing default remains one typed inline function")
    };
    let default = function.parameter_groups()[0].parameters()[0]
        .default()
        .expect("authored equals owns a recovered expression slot");
    assert!(module.slots().resolve(default).unwrap().is_poisoned());
}

#[test]
fn extern_capability_rest_shape_recovery_retains_typed_parameters_and_defaults() {
    let module = lower_capability_case(
        "rest-shape-recovery",
        concat!(
            "extern capability host {\n",
            "    fn misplaced(rest: ...Int)(later: Int)\n",
            "    fn duplicate(first: ...Int, second: ...Int)\n",
            "    fn defaulted(rest: ...Int = 1)\n",
            "}\n",
        ),
    );
    let (_, item, capability) = capability(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    let [
        HirCapabilityMember::Function(misplaced),
        HirCapabilityMember::Function(duplicate),
        HirCapabilityMember::Function(defaulted),
    ] = capability.members()
    else {
        panic!("three capability functions")
    };

    assert_eq!(
        misplaced.parameter_groups()[0].parameters()[0].kind(),
        HirParameterKind::RestPositional
    );
    assert!(
        duplicate.parameter_groups()[0]
            .parameters()
            .iter()
            .all(|parameter| parameter.kind() == HirParameterKind::RestPositional)
    );
    let rest = &defaulted.parameter_groups()[0].parameters()[0];
    assert_eq!(rest.kind(), HirParameterKind::RestPositional);
    assert!(rest.default().is_some());
}

#[test]
fn extern_capability_header_effect_and_tail_recovery_remain_current_grammar() {
    for (case, source) in [
        (
            "malformed-header",
            "extern capability c unexpected { type T }\n",
        ),
        (
            "unbraced-effects",
            "extern capability c { fn f() effects net.read }\n",
        ),
        (
            "associated-tail",
            "extern capability c { type T policy legacy }\n",
        ),
        (
            "function-tail",
            "extern capability c { fn f() policy legacy }\n",
        ),
    ] {
        let module = lower_capability_case(case, source);
        let (_, item, capability) = capability(&module, 0);
        assert!(item.state().is_poisoned(), "{case}");
        assert!(
            matches!(
                capability.members(),
                [HirCapabilityMember::AssociatedType(_) | HirCapabilityMember::Function(_)]
            ),
            "{case}: {:?}",
            capability.members()
        );
    }
}

#[test]
fn extern_capability_freeze_rejects_inline_member_reordering() {
    assert_capability_freeze_rejects(
        "member-reordering",
        concat!(
            "extern capability host {\n",
            "    type Request\n",
            "    fn send(request: Request) -> Unit\n",
            "}\n",
        ),
        |members| members.swap(0, 1),
    );
}

#[test]
fn extern_capability_freeze_rejects_function_effect_substitution() {
    assert_capability_freeze_rejects(
        "effect-substitution",
        concat!(
            "extern capability host {\n",
            "    fn send() effects { net.send, log.write }\n",
            "}\n",
        ),
        |members| {
            let HirCapabilityMember::Function(function) = &members[0] else {
                panic!("one capability function")
            };
            let mut effects = function.effects().to_vec();
            effects.swap(0, 1);
            members[0] = HirCapabilityMember::Function(
                HirCapabilityFunction::try_new(
                    function.prefix().clone(),
                    function.name().clone(),
                    function.generic_parameters().into(),
                    function.parameter_groups().into(),
                    function.return_type(),
                    function.callable_scope(),
                    effects.into_boxed_slice(),
                )
                .unwrap(),
            );
        },
    );
}

#[test]
fn extern_capability_freeze_rejects_associated_type_value_substitution() {
    assert_capability_freeze_rejects(
        "associated-value-substitution",
        concat!(
            "extern capability host {\n",
            "    type First = Alpha\n",
            "    type Second = Beta\n",
            "}\n",
        ),
        |members| {
            let HirCapabilityMember::AssociatedType(second) = &members[1] else {
                panic!("second associated type")
            };
            let second_value = second.value();
            let HirCapabilityMember::AssociatedType(first) = &members[0] else {
                panic!("first associated type")
            };
            members[0] = HirCapabilityMember::AssociatedType(
                HirCapabilityAssociatedType::try_new(
                    first.value().expect("first associated type value").module(),
                    first.prefix().clone(),
                    first.name().clone(),
                    first.generic_parameters().into(),
                    second_value,
                )
                .unwrap(),
            );
        },
    );
}

#[test]
fn extern_capability_freeze_rejects_parameter_type_default_and_return_substitution() {
    let source = concat!(
        "extern capability host {\n",
        "    fn first(value: First = make_first()) -> First\n",
        "    fn second(value: Second = make_second()) -> Second\n",
        "}\n",
    );
    assert_capability_freeze_rejects("parameter-type-substitution", source, |members| {
        let HirCapabilityMember::Function(second) = &members[1] else {
            panic!("second capability function")
        };
        let second_type = second.parameter_groups()[0].parameters()[0].ty();
        let HirCapabilityMember::Function(first) = &members[0] else {
            panic!("first capability function")
        };
        let parameter = &first.parameter_groups()[0].parameters()[0];
        let replacement = HirParameter::try_new(
            parameter.pattern(),
            second_type,
            parameter.kind(),
            parameter.default(),
            parameter.locals().into(),
        )
        .unwrap();
        let group = HirFunctionParameterGroup::try_new(
            first.callable_scope().module(),
            vec![replacement].into_boxed_slice(),
        )
        .unwrap();
        members[0] = HirCapabilityMember::Function(
            HirCapabilityFunction::try_new(
                first.prefix().clone(),
                first.name().clone(),
                first.generic_parameters().into(),
                vec![group].into_boxed_slice(),
                first.return_type(),
                first.callable_scope(),
                first.effects().into(),
            )
            .unwrap(),
        );
    });

    assert_capability_freeze_rejects("parameter-default-substitution", source, |members| {
        let HirCapabilityMember::Function(second) = &members[1] else {
            panic!("second capability function")
        };
        let second_default = second.parameter_groups()[0].parameters()[0].default();
        let HirCapabilityMember::Function(first) = &members[0] else {
            panic!("first capability function")
        };
        let parameter = &first.parameter_groups()[0].parameters()[0];
        let replacement = HirParameter::try_new(
            parameter.pattern(),
            parameter.ty(),
            parameter.kind(),
            second_default,
            parameter.locals().into(),
        )
        .unwrap();
        let group = HirFunctionParameterGroup::try_new(
            first.callable_scope().module(),
            vec![replacement].into_boxed_slice(),
        )
        .unwrap();
        members[0] = HirCapabilityMember::Function(
            HirCapabilityFunction::try_new(
                first.prefix().clone(),
                first.name().clone(),
                first.generic_parameters().into(),
                vec![group].into_boxed_slice(),
                first.return_type(),
                first.callable_scope(),
                first.effects().into(),
            )
            .unwrap(),
        );
    });

    assert_capability_freeze_rejects("return-type-substitution", source, |members| {
        let HirCapabilityMember::Function(second) = &members[1] else {
            panic!("second capability function")
        };
        let second_return = second.return_type();
        let HirCapabilityMember::Function(first) = &members[0] else {
            panic!("first capability function")
        };
        members[0] = HirCapabilityMember::Function(
            HirCapabilityFunction::try_new(
                first.prefix().clone(),
                first.name().clone(),
                first.generic_parameters().into(),
                first.parameter_groups().into(),
                second_return,
                first.callable_scope(),
                first.effects().into(),
            )
            .unwrap(),
        );
    });
}

#[test]
fn extern_capability_freeze_rejects_callable_scope_substitution_and_reordering() {
    let source = concat!(
        "extern capability host {\n",
        "    fn first(value: Int) -> Unit\n",
        "    fn second(value: Int) -> Unit\n",
        "}\n",
    );
    assert_capability_freeze_rejects("callable-scope-substitution", source, |members| {
        let HirCapabilityMember::Function(second) = &members[1] else {
            panic!("second capability function")
        };
        let second_scope = second.callable_scope();
        let HirCapabilityMember::Function(first) = &members[0] else {
            panic!("first capability function")
        };
        members[0] = HirCapabilityMember::Function(
            HirCapabilityFunction::try_new(
                first.prefix().clone(),
                first.name().clone(),
                first.generic_parameters().into(),
                first.parameter_groups().into(),
                first.return_type(),
                second_scope,
                first.effects().into(),
            )
            .unwrap(),
        );
    });

    assert_capability_transaction_freeze_rejects(
        "callable-scope-reordering",
        source,
        |transaction, owner| {
            let (item_scope, first_scope, second_scope) = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::ExternCapability(capability) = item.kind() else {
                    panic!("final ExternCapability item")
                };
                let [
                    HirCapabilityMember::Function(first),
                    HirCapabilityMember::Function(second),
                ] = capability.members()
                else {
                    panic!("two capability functions")
                };
                (
                    item.scope(),
                    first.callable_scope(),
                    second.callable_scope(),
                )
            };
            let (slots, arenas) = transaction.storage_mut();
            let scope = arenas
                .scopes()
                .resolve_staged(slots, item_scope)
                .unwrap()
                .clone();
            let mut children = scope.children().to_vec();
            let first_position = children
                .iter()
                .position(|child| *child == first_scope)
                .unwrap();
            let second_position = children
                .iter()
                .position(|child| *child == second_scope)
                .unwrap();
            children.swap(first_position, second_position);
            let replacement = scope
                .try_with_members(children.into_boxed_slice(), scope.locals().into())
                .unwrap();
            arenas
                .scopes()
                .revise_finalized(slots, item_scope, replacement)
                .unwrap();
        },
    );
}

#[test]
fn extern_capability_freezes_nested_callable_scopes_in_source_order() {
    let source = concat!(
        "extern capability host {\n",
        "    #[audit(callback = |value| value)]\n",
        "    fn run(handler: Callback = |x| x) effects { |a| a, |b| b }\n",
        "}\n",
    );
    let parsed = parse(
        "arcweft-test://proof/final-hir-extern-capability-nested-scope-order",
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
    let (_, _, capability) = capability(&module, 0);
    let [HirCapabilityMember::Function(function)] = capability.members() else {
        panic!("one capability function")
    };
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), function.callable_scope())
        .unwrap();
    assert_eq!(callable.children().len(), 3);
    let source_offsets = callable
        .children()
        .iter()
        .map(|child| {
            let scope = module
                .arenas()
                .scopes()
                .resolve(module.slots(), *child)
                .unwrap();
            assert_eq!(scope.kind(), HirScopeKind::Closure);
            assert_eq!(scope.parent(), Some(function.callable_scope()));
            match module.slots().resolve(*child).unwrap().source_site() {
                HirSourceSite::Span(span) => span.range().start(),
                HirSourceSite::Insertion(insertion) => insertion.offset(),
            }
        })
        .collect::<Vec<_>>();
    assert!(source_offsets.windows(2).all(|pair| pair[0] < pair[1]));

    assert_capability_transaction_freeze_rejects(
        "nested-callable-scope-reordering",
        source,
        |transaction, owner| {
            let callable_scope = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::ExternCapability(capability) = item.kind() else {
                    panic!("final ExternCapability item")
                };
                let [HirCapabilityMember::Function(function)] = capability.members() else {
                    panic!("one capability function")
                };
                function.callable_scope()
            };
            let (slots, arenas) = transaction.storage_mut();
            let callable = arenas
                .scopes()
                .resolve_staged(slots, callable_scope)
                .unwrap()
                .clone();
            let mut children = callable.children().to_vec();
            children.swap(0, 2);
            let replacement = callable
                .try_with_members(children.into_boxed_slice(), callable.locals().into())
                .unwrap();
            arenas
                .scopes()
                .revise_finalized(slots, callable_scope, replacement)
                .unwrap();
        },
    );
}

#[test]
fn extern_capability_member_limit_accepts_exact_source_and_rejects_one_over_preflight() {
    let maximum = HirLimit::DeclarationMembers.maximum();
    let mut source = String::from("extern capability host {\n");
    for ordinal in 0..maximum {
        writeln!(source, "    type T{ordinal}").unwrap();
    }
    source.push_str("}\n");
    let parsed = parse(
        "arcweft-test://proof/final-hir-extern-capability-member-limit",
        &source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, capability) = capability(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(capability.members().len(), maximum);
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    let published = database.current(&key).expect("exact-limit module commits");
    assert!(Arc::ptr_eq(&module, &published));

    assert!(preflight_extern_capability_members(maximum).is_ok());
    let observed = maximum + 1;
    let Err(HirLowerFailure::Limit(error)) = preflight_extern_capability_members(observed) else {
        panic!("one-over capability member inventory must fail before child lowering")
    };
    assert_eq!(error.limit(), HirLimit::DeclarationMembers);
    assert_eq!(error.observed(), observed);
    assert_eq!(error.maximum(), HirLimit::DeclarationMembers.maximum());
}

#[test]
fn local_scope_limit_is_inclusive_and_atomic() {
    let source = |local_count: usize| {
        let maximum_descendants = HirLimit::SyntheticDescendantsPerOwner.maximum();
        let mut source = String::from("extern capability host { fn run(");
        for (parameter, start) in (0..local_count).step_by(maximum_descendants).enumerate() {
            if parameter != 0 {
                source.push_str(", ");
            }
            source.push('(');
            let end = (start + maximum_descendants).min(local_count);
            for ordinal in start..end {
                if ordinal != start {
                    source.push_str(", ");
                }
                write!(source, "p{ordinal}").unwrap();
            }
            source.push_str("): Tuple");
        }
        source.push_str(") }\n");
        source
    };
    let maximum = HirLimit::LocalsPerScope.maximum();
    let parsed = parse(
        "arcweft-test://proof/final-hir-extern-capability-locals-exact",
        &source(maximum),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, capability) = capability(&module, 0);
    let [HirCapabilityMember::Function(function)] = capability.members() else {
        panic!("one capability function")
    };
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), function.callable_scope())
        .unwrap();
    assert_eq!(callable.locals().len(), maximum);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert!(database.current(&key).is_some());

    let observed = maximum + 1;
    let parsed = parse(
        "arcweft-test://proof/final-hir-extern-capability-locals-one-over",
        &source(observed),
    );
    let key = module_key(&parsed);
    let database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    let Err(HirLowerFailure::Limit(error)) = transaction.lower_parsed_source_items(&parsed) else {
        panic!("one-over callable locals must fail in the staged HIR transaction")
    };
    assert_eq!(error.limit(), HirLimit::LocalsPerScope);
    assert_eq!(error.observed(), observed);
    assert_eq!(error.maximum(), maximum);
    assert!(database.current(&key).is_none());
}
