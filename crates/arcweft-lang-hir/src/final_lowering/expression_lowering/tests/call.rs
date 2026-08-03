use super::*;
use crate::expr::{
    HirAssociatedCallSyntax, HirAssociatedReceiver, HirCallArgument, HirCallArgumentListTerminator,
    HirCallCallee, HirCallIssue, HirCallTypeApplication, HirCallTypeApplicationSpelling,
    HirCallTypeApplicationTerminator, HirCallTypeArgument, HirCallTypeArgumentOrdinal,
    HirCallValue, HirRecoveredName, HirRecoveryIssue,
};
use crate::source_index::{
    HirCallArgumentSourcePart, HirCallTypeApplicationSourceRole, HirCallTypeArgumentSourcePart,
    HirSourcePresence,
};
use crate::type_ref::HirTypeKind;

fn positional_call(argument_count: usize) -> String {
    let mut source = String::from("callee(");
    for ordinal in 0..argument_count {
        if ordinal != 0 {
            source.push_str(", ");
        }
        source.push_str("value");
    }
    source.push(')');
    source
}

#[test]
fn attached_callback_block_lowers_as_one_positional_closure_argument() {
    let parsed = parsed_source(
        "call-callback-block",
        &["items.map { item: Label, index => item.text }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::Call(call) = expression(&module, owner).kind() else {
        panic!("callback block must publish the central Call payload");
    };
    assert!(matches!(
        expression(
            &module,
            call.callee().value_expression().expect("callback callee")
        )
        .kind(),
        HirExprKind::Select(_)
    ));
    assert!(matches!(
        call.explicit_type_application(),
        HirCallTypeApplication::Absent
    ));
    let [
        HirCallArgument::Positional {
            value: HirCallValue::Present { value: callback },
        },
    ] = call.arguments()
    else {
        panic!("callback Call must own exactly one positional Closure argument");
    };
    let HirExprKind::Closure(closure) = expression(&module, *callback).kind() else {
        panic!("callback argument must use the central Closure payload");
    };
    assert_eq!(closure.parameters().len(), 2);
    assert!(closure.parameters()[0].ty().is_some());
    assert!(closure.parameters()[1].ty().is_none());
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), closure.scope())
        .expect("callback Closure lexical scope");
    assert_eq!(scope.kind(), HirScopeKind::Closure);
    assert_eq!(scope.owner(), &HirScopeOwner::Expr(*callback));
    assert!(scope.locals().iter().all(|local| {
        module
            .arenas()
            .locals()
            .resolve(module.slots(), *local)
            .is_ok_and(|local| local.kind() == HirLocalKind::ClosureParameter)
    }));

    let argument = HirCallArgumentOrdinal::try_new(0).expect("callback argument ordinal");
    for role in [
        HirExprSourceRole::CallCallee,
        HirExprSourceRole::CallArgumentListOpen,
        HirExprSourceRole::CallArgumentListClose,
        HirExprSourceRole::CallArgument {
            argument,
            part: HirCallArgumentSourcePart::Whole,
        },
        HirExprSourceRole::CallArgument {
            argument,
            part: HirCallArgumentSourcePart::Value,
        },
    ] {
        assert!(matches!(
            module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner, role },
                )
                .expect("callback Call source query")
                .presence(),
            HirSourcePresence::Present(HirSourceSite::Span(_))
        ));
    }
}

#[test]
fn callback_missing_body_uses_the_required_tail_owner_and_poisons_the_argument() {
    let parsed = parsed_source("call-callback-missing-body", &["items.map {}".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::Call(call) = expression(&module, owner).kind() else {
        panic!("callback block Call payload");
    };
    let argument = HirCallArgumentOrdinal::try_new(0).expect("callback argument ordinal");
    assert_eq!(
        expression(&module, owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(
            HirCallIssue::InvalidArgumentValue { argument }
        ))
    );
    let [
        HirCallArgument::Positional {
            value: HirCallValue::Present { value: callback },
        },
    ] = call.arguments()
    else {
        panic!("callback argument");
    };
    let HirExprKind::Closure(closure) = expression(&module, *callback).kind() else {
        panic!("callback Closure payload");
    };
    assert_eq!(
        expression(&module, *callback).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
    );
    assert!(matches!(
        module
            .slots()
            .resolve(closure.body())
            .expect("missing callback body slot")
            .origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(*callback)
                && key.role() == SyntheticRole::MissingRequiredTail
    ));
}

#[test]
fn attached_e12_call_publishes_one_typed_argument_inventory_and_exact_sources() {
    let parsed = parsed_source(
        "call-ordinary-matrix",
        &["callee(first, limit = second, rest...,)".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::Call(call) = expression(&module, owner).kind() else {
        panic!("E12 Call payload");
    };
    assert_eq!(call.arguments().len(), 3);
    assert_eq!(call.terminator(), HirCallArgumentListTerminator::Closed);
    assert!(matches!(
        expression(
            &module,
            call.callee().value_expression().expect("ordinary callee")
        )
        .kind(),
        HirExprKind::Path(HirPathValue::Resolved(path))
            if matches!(path.segments(), [HirPathSegment::Identifier(name)] if name.as_str() == "callee")
    ));
    assert!(matches!(
        &call.arguments()[0],
        HirCallArgument::Positional {
            value: HirCallValue::Present { .. }
        }
    ));
    assert!(matches!(
        &call.arguments()[1],
        HirCallArgument::Named {
            name: HirRecoveredName::Valid(name),
            value: HirCallValue::Present { .. },
            ..
        } if name.as_str() == "limit"
    ));
    assert!(matches!(
        &call.arguments()[2],
        HirCallArgument::Spread {
            value: HirCallValue::Present { .. },
            ..
        }
    ));

    for role in [
        HirExprSourceRole::CallCallee,
        HirExprSourceRole::CallArgumentListOpen,
        HirExprSourceRole::CallArgumentListClose,
        HirExprSourceRole::CallArgumentSeparator {
            following: HirCallArgumentOrdinal::try_new(1).expect("argument ordinal"),
        },
        HirExprSourceRole::CallArgumentSeparator {
            following: HirCallArgumentOrdinal::try_new(2).expect("argument ordinal"),
        },
        HirExprSourceRole::CallArgumentTrailingSeparator,
        HirExprSourceRole::CallArgument {
            argument: HirCallArgumentOrdinal::try_new(1).expect("argument ordinal"),
            part: HirCallArgumentSourcePart::Name,
        },
        HirExprSourceRole::CallArgument {
            argument: HirCallArgumentOrdinal::try_new(1).expect("argument ordinal"),
            part: HirCallArgumentSourcePart::Equals,
        },
        HirExprSourceRole::CallArgument {
            argument: HirCallArgumentOrdinal::try_new(2).expect("argument ordinal"),
            part: HirCallArgumentSourcePart::Spread,
        },
    ] {
        let source = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr { owner, role },
            )
            .expect("Call source query");
        assert!(matches!(
            source.presence(),
            HirSourcePresence::Present(HirSourceSite::Span(_))
        ));
    }
}

#[test]
fn attached_e12_associated_calls_publish_one_revision_bound_receiver_authority() {
    let parsed = parsed_source(
        "call-associated-matrix",
        &[
            "Vec<I32>.with_capacity(8)".into(),
            "Vec<I32>::with_capacity(8)".into(),
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    for ((owner, attached), explicit) in owners.into_iter().zip(&attached).zip([false, true]) {
        let HirExprKind::Call(call) = expression(&module, owner).kind() else {
            panic!("associated E12 Call payload");
        };
        let receiver = match (call.callee(), explicit) {
            (
                HirCallCallee::UnresolvedDot {
                    value_receiver,
                    nominal_receiver,
                    member: HirRecoveredName::Valid(member),
                },
                false,
            ) => {
                assert_eq!(member.as_str(), "with_capacity");
                assert!(matches!(
                    expression(&module, *value_receiver).kind(),
                    HirExprKind::Path(HirPathValue::Resolved(path))
                        if matches!(path.segments(), [HirPathSegment::Identifier(name)] if name.as_str() == "Vec")
                ));
                nominal_receiver
            }
            (
                HirCallCallee::Associated {
                    receiver,
                    member: HirRecoveredName::Valid(member),
                    syntax: HirAssociatedCallSyntax::ExplicitDoubleColon,
                },
                true,
            ) => {
                assert_eq!(member.as_str(), "with_capacity");
                receiver
            }
            _ => panic!("wrong associated E12 callee family"),
        };
        let HirAssociatedReceiver::Resolved { receiver } = receiver else {
            panic!("valid associated receiver remains resolved");
        };
        assert!(matches!(
            module
                .arenas()
                .types()
                .resolve(module.slots(), *receiver)
                .expect("published associated receiver type")
                .kind(),
            HirTypeKind::Generic(generic)
                if matches!(generic.base().segments(), [HirPathSegment::Identifier(name)] if name.as_str() == "Vec")
                    && generic.arguments().len() == 1
        ));
        let [attached_receiver] = attached.call_type_children() else {
            panic!("one attached associated receiver relation");
        };
        assert!(matches!(
            module
                .slots()
                .resolve(*receiver)
                .expect("associated receiver slot")
                .origin(),
            HirOrigin::Source(source) if source.syntax() == attached_receiver.node().id()
        ));

        for role in [
            HirExprSourceRole::CallAssociatedReceiver,
            HirExprSourceRole::CallAssociatedSeparator,
            HirExprSourceRole::CallAssociatedMember,
        ] {
            assert!(matches!(
                module
                    .source_site(
                        parsed.document().identity(),
                        HirSourceQuery::Expr { owner, role },
                    )
                    .expect("associated Call source query")
                    .presence(),
                HirSourcePresence::Present(HirSourceSite::Span(_))
            ));
        }
    }
}

#[test]
fn attached_e12_call_type_applications_publish_distinct_ordered_type_authority() {
    let parsed = parsed_source(
        "call-type-application-matrix",
        &[
            "foo::<T>()".into(),
            "value.collect<Vec<I32>>()".into(),
            "Vec<T>::member::<U>(x)".into(),
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    for ((owner, attached), expected_types) in owners.iter().zip(&attached).zip([1, 2, 2]) {
        let HirExprKind::Call(call) = expression(&module, *owner).kind() else {
            panic!("typed E12 Call payload");
        };
        let HirCallTypeApplication::Present {
            arguments,
            terminator: HirCallTypeApplicationTerminator::Closed,
            ..
        } = call.explicit_type_application()
        else {
            panic!("closed explicit type application");
        };
        assert!(matches!(
            arguments.as_ref(),
            [HirCallTypeArgument::Resolved { .. }]
        ));
        assert_eq!(attached.call_type_children().len(), expected_types);
        let type_argument = HirCallTypeArgumentOrdinal::try_new(0).expect("type ordinal");
        for role in [
            HirCallTypeApplicationSourceRole::Whole,
            HirCallTypeApplicationSourceRole::OpenAngle,
            HirCallTypeApplicationSourceRole::CloseAngle,
            HirCallTypeApplicationSourceRole::Argument {
                argument: type_argument,
                part: HirCallTypeArgumentSourcePart::Whole,
            },
            HirCallTypeApplicationSourceRole::Argument {
                argument: type_argument,
                part: HirCallTypeArgumentSourcePart::Type,
            },
        ] {
            assert!(matches!(
                module
                    .source_site(
                        parsed.document().identity(),
                        HirSourceQuery::Expr {
                            owner: *owner,
                            role: HirExprSourceRole::CallTypeApplication(role),
                        },
                    )
                    .expect("Call type-application source query")
                    .presence(),
                HirSourcePresence::Present(HirSourceSite::Span(_))
            ));
        }
    }

    let HirExprKind::Call(free) = expression(&module, owners[0]).kind() else {
        panic!("free typed Call");
    };
    assert_eq!(
        free.explicit_type_application().spelling(),
        Some(HirCallTypeApplicationSpelling::Turbofish)
    );
    let HirExprKind::Call(direct) = expression(&module, owners[1]).kind() else {
        panic!("direct member typed Call");
    };
    assert_eq!(
        direct.explicit_type_application().spelling(),
        Some(HirCallTypeApplicationSpelling::DirectAngle)
    );

    let HirExprKind::Call(associated) = expression(&module, owners[2]).kind() else {
        panic!("associated typed Call");
    };
    let HirCallCallee::Associated {
        receiver: HirAssociatedReceiver::Resolved { receiver },
        ..
    } = associated.callee()
    else {
        panic!("associated receiver");
    };
    let HirCallTypeApplication::Present { arguments, .. } = associated.explicit_type_application()
    else {
        panic!("associated member application");
    };
    let [
        HirCallTypeArgument::Resolved {
            ty: member_argument,
        },
    ] = arguments.as_ref()
    else {
        panic!("associated member type argument");
    };
    assert_ne!(receiver, member_argument);
}

#[test]
fn attached_e12_call_type_application_recovery_keeps_structural_slots() {
    let parsed = parsed_source(
        "call-type-application-recovery",
        &[
            "foo::<>()".into(),
            "foo::<9bad>()".into(),
            "foo::<T()".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let type_argument = HirCallTypeArgumentOrdinal::try_new(0).expect("type ordinal");
    let expected = [
        HirCallIssue::MissingTypeArgument {
            argument: type_argument,
        },
        HirCallIssue::InvalidTypeArgument {
            argument: type_argument,
        },
        HirCallIssue::MissingTypeApplicationClose,
    ];
    for (owner, expected) in owners.into_iter().zip(expected) {
        assert_eq!(
            expression(&module, owner).state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(expected))
        );
    }
}

#[test]
fn attached_e12_missing_argument_value_retains_one_synthetic_operand() {
    let parsed = parsed_source("call-missing-value", &["callee(name =)".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let owner = owners[0];
    let HirExprKind::Call(call) = expression(&module, owner).kind() else {
        panic!("recovered E12 Call payload");
    };
    let [
        HirCallArgument::Named {
            value: HirCallValue::Missing { recovery },
            ..
        },
    ] = call.arguments()
    else {
        panic!("one recovered named argument");
    };
    let argument = HirCallArgumentOrdinal::try_new(0).expect("argument ordinal");
    let role = HirExprSourceRole::CallArgument {
        argument,
        part: HirCallArgumentSourcePart::Value,
    };
    assert_synthetic_recovery_child(&module, owner, *recovery, 1, role);
    assert!(matches!(
        expression(&module, owner).state(),
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(
            HirCallIssue::MissingArgumentValue { argument: actual }
        )) if *actual == argument
    ));
    let value_source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr { owner, role },
        )
        .expect("missing Call value source");
    assert!(matches!(
        value_source.presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
}

#[test]
fn attached_e12_call_recovery_order_is_derived_from_the_final_argument_inventory() {
    let parsed = parsed_source(
        "call-ordered-recovery",
        &[
            "callee(name = first, second)".into(),
            "callee(rest..., second)".into(),
            "callee(name = first, name = second)".into(),
            "callee(first".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);

    let expected = [
        HirCallIssue::PositionalAfterNamed {
            argument: HirCallArgumentOrdinal::try_new(1).expect("argument ordinal"),
        },
        HirCallIssue::SpreadNotLast {
            argument: HirCallArgumentOrdinal::try_new(0).expect("argument ordinal"),
        },
        HirCallIssue::DuplicateNamedArgument {
            first: HirCallArgumentOrdinal::try_new(0).expect("argument ordinal"),
            duplicate: HirCallArgumentOrdinal::try_new(1).expect("argument ordinal"),
        },
        HirCallIssue::MissingArgumentListClose,
    ];
    for (owner, expected) in owners.into_iter().zip(expected) {
        assert_eq!(
            expression(&module, owner).state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(expected))
        );
    }
}

#[test]
fn attached_e12_call_argument_limit_accepts_exact_and_rolls_back_one_over() {
    let maximum = HirLimit::CallArguments.maximum();
    let exact = parsed_source("call-arguments-exact", &[positional_call(maximum)]);
    let (module, owners, _) = lower_and_publish(&exact);
    let HirExprKind::Call(call) = expression(&module, owners[0]).kind() else {
        panic!("exact-limit E12 Call payload");
    };
    assert_eq!(call.arguments().len(), maximum);

    let one_over = parsed_source("call-arguments-one-over", &[positional_call(maximum + 1)]);
    let attached = attached_expressions(&one_over);
    let [attached] = attached.as_slice() else {
        panic!("one-over fixture must retain exactly one expression");
    };
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    assert_eq!(
        transaction.lower_attached_expression(attached, scope),
        Err(HirLowerFailure::Limit(HirLimitError::with_maximum(
            HirLimit::CallArguments,
            maximum + 1,
            maximum,
        )))
    );
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}
