use super::*;

use crate::expr::HirCallArgumentOrdinal;
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

fn call_source_site(
    module: &HirModule,
    parsed: &ParsedSource,
    owner: ExprId,
    role: HirExprSourceRole,
) -> HirSourceSite {
    match module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr { owner, role },
        )
        .expect("committed Call source query")
        .presence()
    {
        HirSourcePresence::Present(site) => site.clone(),
        HirSourcePresence::AbsentOptional => panic!("required Call cursor source is absent"),
    }
}

fn call_site_start(site: &HirSourceSite) -> usize {
    match site {
        HirSourceSite::Span(span) => span.range().start(),
        HirSourceSite::Insertion(insertion) => insertion.offset(),
    }
}

fn call_site_end(site: &HirSourceSite) -> usize {
    match site {
        HirSourceSite::Span(span) => span.range().end(),
        HirSourceSite::Insertion(insertion) => insertion.offset(),
    }
}

#[test]
fn t_cursor_r04_r05_r08_r09_r13_r14_use_the_committed_argument_manifest() {
    let parsed = parsed_source(
        "call-argument-active-slot-matrix",
        &["f(a, b)".into(), "f(a,)".into(), "f(a)".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let source = parsed.document().identity();

    let open = call_source_site(
        &module,
        &parsed,
        owners[0],
        HirExprSourceRole::CallArgumentListOpen,
    );
    for cursor in call_site_start(&open)..call_site_end(&open) {
        assert_eq!(
            module
                .call_active_argument_slot(source, owners[0], cursor)
                .expect("T-CURSOR-R04 query"),
            None,
            "T-CURSOR-R04 opening-token byte {cursor} must remain outside",
        );
    }
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[0], call_site_end(&open))
            .expect("T-CURSOR-R05 query"),
        Some(0),
        "T-CURSOR-R05",
    );

    let comma = call_source_site(
        &module,
        &parsed,
        owners[0],
        HirExprSourceRole::CallArgumentSeparator {
            following: HirCallArgumentOrdinal::try_new(1).expect("second argument ordinal"),
        },
    );
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[0], call_site_start(&comma))
            .expect("T-CURSOR-R08 query"),
        Some(1),
        "T-CURSOR-R08",
    );
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[0], call_site_end(&comma))
            .expect("T-CURSOR-R09 query"),
        Some(1),
        "T-CURSOR-R09",
    );

    let trailing = call_source_site(
        &module,
        &parsed,
        owners[1],
        HirExprSourceRole::CallArgumentTrailingSeparator,
    );
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[1], call_site_start(&trailing))
            .expect("T-CURSOR-R13 query"),
        Some(1),
        "T-CURSOR-R13 one-past slot",
    );

    let close = call_source_site(
        &module,
        &parsed,
        owners[2],
        HirExprSourceRole::CallArgumentListClose,
    );
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[2], call_site_start(&close))
            .expect("T-CURSOR-R14 query"),
        Some(0),
        "T-CURSOR-R14",
    );
}

#[test]
fn t_cursor_c_empty_missing_and_out_use_structural_terminators() {
    let parsed = parsed_source(
        "call-argument-active-slot-recovery",
        &["f()".into(), "f(a)".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let source = parsed.document().identity();

    let empty_open = call_source_site(
        &module,
        &parsed,
        owners[0],
        HirExprSourceRole::CallArgumentListOpen,
    );
    let empty_close = call_source_site(
        &module,
        &parsed,
        owners[0],
        HirExprSourceRole::CallArgumentListClose,
    );
    assert_eq!(call_site_end(&empty_open), call_site_start(&empty_close));
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[0], call_site_end(&empty_open))
            .expect("T-CURSOR-C-EMPTY query"),
        Some(0),
        "T-CURSOR-C-EMPTY",
    );

    let missing = parsed_source("call-argument-active-slot-missing-close", &["f(a".into()]);
    let (missing_module, missing_owners, _) = lower_and_publish(&missing);
    let recovery_end = call_source_site(
        &missing_module,
        &missing,
        missing_owners[0],
        HirExprSourceRole::CallArgumentListRecoveryEnd,
    );
    assert_eq!(
        missing_module
            .call_active_argument_slot(
                missing.document().identity(),
                missing_owners[0],
                call_site_start(&recovery_end),
            )
            .expect("T-CURSOR-C-MISSING query"),
        Some(0),
        "T-CURSOR-C-MISSING",
    );

    let close = call_source_site(
        &module,
        &parsed,
        owners[1],
        HirExprSourceRole::CallArgumentListClose,
    );
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[1], call_site_end(&close))
            .expect("T-CURSOR-C-OUT query"),
        None,
        "T-CURSOR-C-OUT",
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test is the closed type-application cursor acceptance matrix and asserts cross-arena ordering in one fixture"
)]
fn t_cursor_type_application_rows_use_the_independent_committed_type_manifest() {
    let parsed = parsed_source(
        "call-type-active-slot-matrix",
        &[
            "value.collect<T>()".into(),
            "foo::<T>()".into(),
            "foo::<T, U>()".into(),
            "foo::<T,>()".into(),
            "foo::<>()".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let source = parsed.document().identity();
    let type_role = |role| HirExprSourceRole::CallTypeApplication(role);

    let direct_open = call_source_site(
        &module,
        &parsed,
        owners[0],
        type_role(HirCallTypeApplicationSourceRole::OpenAngle),
    );
    for cursor in call_site_start(&direct_open)..call_site_end(&direct_open) {
        assert_eq!(
            module
                .call_active_type_argument_slot(source, owners[0], cursor)
                .expect("T-CURSOR-T-DIRECT-OPEN query"),
            None,
            "T-CURSOR-T-DIRECT-OPEN byte {cursor}",
        );
    }
    assert_eq!(
        module
            .call_active_type_argument_slot(source, owners[0], call_site_end(&direct_open))
            .expect("T-CURSOR-T-DIRECT-S0 query"),
        Some(0),
        "T-CURSOR-T-DIRECT-S0",
    );

    let turbofish = call_source_site(
        &module,
        &parsed,
        owners[1],
        type_role(HirCallTypeApplicationSourceRole::TurbofishSeparator),
    );
    for cursor in call_site_start(&turbofish)..call_site_end(&turbofish) {
        assert_eq!(
            module
                .call_active_type_argument_slot(source, owners[1], cursor)
                .expect("T-CURSOR-T-TURBO-PREFIX query"),
            None,
            "T-CURSOR-T-TURBO-PREFIX byte {cursor}",
        );
    }

    let comma = call_source_site(
        &module,
        &parsed,
        owners[2],
        type_role(HirCallTypeApplicationSourceRole::Separator {
            following: HirCallTypeArgumentOrdinal::try_new(1)
                .expect("second type argument ordinal"),
        }),
    );
    assert_eq!(
        module
            .call_active_type_argument_slot(source, owners[2], call_site_start(&comma))
            .expect("T-CURSOR-T-COMMA query"),
        Some(1),
        "T-CURSOR-T-COMMA",
    );

    let trailing = call_source_site(
        &module,
        &parsed,
        owners[3],
        type_role(HirCallTypeApplicationSourceRole::TrailingSeparator),
    );
    assert_eq!(
        module
            .call_active_type_argument_slot(source, owners[3], call_site_start(&trailing))
            .expect("T-CURSOR-T-TRAIL query"),
        Some(1),
        "T-CURSOR-T-TRAIL one-past slot",
    );

    let close = call_source_site(
        &module,
        &parsed,
        owners[1],
        type_role(HirCallTypeApplicationSourceRole::CloseAngle),
    );
    assert_eq!(
        module
            .call_active_type_argument_slot(source, owners[1], call_site_start(&close))
            .expect("T-CURSOR-T-CLOSE query"),
        Some(0),
        "T-CURSOR-T-CLOSE",
    );

    let empty_open = call_source_site(
        &module,
        &parsed,
        owners[4],
        type_role(HirCallTypeApplicationSourceRole::OpenAngle),
    );
    let empty_close = call_source_site(
        &module,
        &parsed,
        owners[4],
        type_role(HirCallTypeApplicationSourceRole::CloseAngle),
    );
    assert_eq!(call_site_end(&empty_open), call_site_start(&empty_close));
    assert_eq!(
        module
            .call_active_type_argument_slot(source, owners[4], call_site_end(&empty_open))
            .expect("T-CURSOR-T-EMPTY query"),
        Some(0),
        "T-CURSOR-T-EMPTY",
    );

    let missing = parsed_source("call-type-active-slot-missing-close", &["foo::<T()".into()]);
    let (missing_module, missing_owners, _) = lower_and_publish(&missing);
    let recovery_end = call_source_site(
        &missing_module,
        &missing,
        missing_owners[0],
        type_role(HirCallTypeApplicationSourceRole::RecoveryEnd),
    );
    assert_eq!(
        missing_module
            .call_active_type_argument_slot(
                missing.document().identity(),
                missing_owners[0],
                call_site_start(&recovery_end),
            )
            .expect("T-CURSOR-T-MISSING-CLOSE query"),
        Some(0),
        "T-CURSOR-T-MISSING-CLOSE",
    );
    assert_eq!(
        module
            .call_active_type_argument_slot(source, owners[1], call_site_end(&close))
            .expect("T-CURSOR-T-OUT query"),
        None,
        "T-CURSOR-T-OUT",
    );

    let argument_open = call_source_site(
        &module,
        &parsed,
        owners[1],
        HirExprSourceRole::CallArgumentListOpen,
    );
    assert_eq!(
        module
            .call_active_argument_slot(source, owners[1], call_site_end(&argument_open))
            .expect("ordinary argument slot query"),
        Some(0),
    );
    assert_eq!(
        module
            .call_active_type_argument_slot(source, owners[1], call_site_end(&argument_open))
            .expect("independent type-argument slot query"),
        None,
        "type-application cursor must remain independent from ordinary arguments",
    );
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
fn qualified_value_path_remains_the_ordinary_call_callee_authority() {
    let parsed = parsed_source(
        "call-qualified-value-path",
        &["pkg::service::invoke(x)".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::Call(call) = expression(&module, owners[0]).kind() else {
        panic!("qualified ordinary Call");
    };
    let HirCallCallee::Value { value } = call.callee() else {
        panic!("qualified value path must not become an associated type callee");
    };
    assert!(matches!(
        expression(&module, *value).kind(),
        HirExprKind::Path(HirPathValue::Resolved(path))
            if matches!(
                path.segments(),
                [
                    HirPathSegment::Identifier(package),
                    HirPathSegment::Identifier(service),
                    HirPathSegment::Identifier(invoke),
                ] if package.as_str() == "pkg"
                    && service.as_str() == "service"
                    && invoke.as_str() == "invoke"
            )
    ));
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
                    ..
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
                    separator:
                        HirAssociatedSeparator::Present(HirAssociatedCallSyntax::ExplicitDoubleColon),
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
fn attached_e12_admitted_associated_recovery_lowers_exact_typed_states_and_source_sites() {
    let parsed = parsed_source(
        "call-associated-recovery-matrix",
        &[
            "Bad<>::member(x)".into(),
            "Vec.with_capacity(8)".into(),
            "Vec<I32>. (8)".into(),
            "Vec<I32>.9bad(8)".into(),
        ],
    );
    let (module, owners, _attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let calls = owners
        .iter()
        .map(|owner| {
            let expression = expression(&module, *owner);
            let HirExprKind::Call(call) = expression.kind() else {
                panic!("associated recovery E12 Call");
            };
            (expression, call)
        })
        .collect::<Vec<_>>();

    let HirCallCallee::Associated {
        receiver: HirAssociatedReceiver::InvalidPresent { poisoned },
        ..
    } = calls[0].1.callee()
    else {
        panic!("invalid receiver retains one poisoned TypeId");
    };
    assert!(
        module
            .arenas()
            .types()
            .resolve(module.slots(), *poisoned)
            .expect("invalid associated receiver type")
            .is_poisoned()
    );
    assert_eq!(
        calls[0].0.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(
            HirCallIssue::InvalidAssociatedReceiver,
        ))
    );

    assert!(matches!(
        calls[1].1.callee(),
        HirCallCallee::UnresolvedDot {
            nominal_receiver: HirAssociatedReceiver::Resolved { .. },
            separator: HirAssociatedSeparator::Present(HirAssociatedCallSyntax::DotFallback),
            member: HirRecoveredName::Valid(member),
            ..
        } if member.as_str() == "with_capacity"
    ));
    assert_eq!(calls[1].0.state(), &HirPoisonState::Clean);

    assert!(matches!(
        calls[2].1.callee(),
        HirCallCallee::Associated {
            member: HirRecoveredName::Missing,
            ..
        }
    ));
    assert_eq!(
        calls[2].0.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(
            HirCallIssue::MissingAssociatedMember,
        ))
    );

    assert!(matches!(
        calls[3].1.callee(),
        HirCallCallee::Associated {
            member: HirRecoveredName::InvalidPresent,
            ..
        }
    ));
    assert_eq!(
        calls[3].0.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(
            HirCallIssue::InvalidAssociatedMember,
        ))
    );

    assert!(matches!(
        module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[2],
                    role: HirExprSourceRole::CallAssociatedMember,
                },
            )
            .expect("associated member recovery source query")
            .presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
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
