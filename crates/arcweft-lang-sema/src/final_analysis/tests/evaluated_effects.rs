use super::*;

#[test]
fn drop_policy_overload_is_checked_for_free_pipe_and_dot_surfaces() {
    let fixture = fixture(
        r"
fn optional_value() -> Option<i64> { .Some(7i64) }

fn dispose(value: i64) {
    drop(value);
    drop_optional(optional_value());
    drop(stop_now)(value);
    value |> drop(stop_now);
    value.drop(stop_now);
    let retained = on_drop(stop_now)(value);
    retained;
}
",
        None,
    );
    let report = analyze(&fixture).expect("typed drop policy overload analysis");
    let drops = report
        .statements()
        .filter_map(|(_, statement)| match statement.role() {
            CheckedStatementRole::EvaluatedEffect(effect) => match effect.operation() {
                CheckedEvaluatedEffectOperation::Drop { invocation, .. } => Some(invocation),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(drops.len(), 5);
    assert!(matches!(drops[0], CheckedDropInvocation::Drop));
    assert!(matches!(drops[1], CheckedDropInvocation::DropOptional));
    for invocation in &drops[2..] {
        let CheckedDropInvocation::DropWithPolicy { source, policy } = invocation else {
            panic!("policy overload seals an explicit invocation")
        };
        assert!(matches!(
            source.operand().source().raw(),
            CheckedCallArgumentSlotSource::Expression(_)
        ));
        assert!(matches!(
            policy,
            CheckedExplicitDropPolicy::Stop {
                fade: CheckedDropFade::Constant(value)
            } if *value == arcweft_core::time::LogicalDuration::from_nanos(0)
        ));
    }
}

#[test]
fn runtime_flow_publishes_explicit_drop_policy_expressions() {
    let fixture = fixture(
        r"
flow main() -> Unit {
    drop(.Cancel)([1i64]...)
    drop(.Stop(fade = 120ms))([1i64]...)
}
",
        None,
    );
    let report = analyze(&fixture).expect("runtime flow explicit drop policy analysis");
    let policies = report
        .statements()
        .filter_map(|(_, statement)| match statement.role() {
            CheckedStatementRole::EvaluatedEffect(effect) => match effect.operation() {
                CheckedEvaluatedEffectOperation::Drop { target, invocation } => {
                    Some((target, invocation))
                }
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        policies.as_slice(),
        [
            (
                cancel_target,
                CheckedDropInvocation::DropWithPolicy {
                    policy: CheckedExplicitDropPolicy::Cancel,
                    ..
                }
            ),
            (
                stop_target,
                CheckedDropInvocation::DropWithPolicy {
                    policy: CheckedExplicitDropPolicy::Stop {
                        fade: CheckedDropFade::Operand(fade)
                    },
                    ..
                }
            )
        ] if cancel_target.ty() == &TypeKind::I64
            && stop_target.ty() == &TypeKind::I64
            && fade.operand().ty() == &TypeKind::Duration
    ));
    for (target, _) in &policies {
        let CheckedCallArgumentSlotSource::CompactNumericElement { sequence, .. } =
            target.source().raw()
        else {
            panic!("Drop target retains its compact numeric element source")
        };
        let TypeKind::Vec(item) = report
            .expression(sequence)
            .expect("compact numeric source expression")
            .ty()
        else {
            panic!("Drop compact source retains its Vec type")
        };
        assert_eq!(item.as_ref(), &TypeKind::I64);
        assert_eq!(target.ty(), &TypeKind::I64);
    }
}

#[test]
fn contextual_short_variant_constructor_head_uses_expected_project_enum() {
    let fixture = fixture(
        r"
enum ConstructorProbe {
    Payload(i64),
}

fn make_probe() -> ConstructorProbe {
    .Payload(7i64)
}
",
        None,
    );
    let report = analyze(&fixture).expect("contextual short-variant constructor analysis");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root module");
    assert!(report.calls().any(|(owner, call)| {
        let Ok(expression) = module.resolve_expr(owner) else {
            return false;
        };
        let HirExprKind::Call(call_expression) = expression.kind() else {
            return false;
        };
        let HirCallCallee::Value { value } = call_expression.callee() else {
            return false;
        };
        module
            .resolve_expr(*value)
            .is_ok_and(|expression| matches!(expression.kind(), HirExprKind::ShortVariant(_)))
            && call.selected_application().is_some_and(|application| {
                matches!(application.result().ty(), TypeKind::ProjectNominal(_))
            })
    }));
}

#[test]
fn contextual_short_variant_constructor_heads_preserve_non_project_payload_shapes() {
    let fixture = fixture(
        r#"
fn some_value() -> Option<i64> { .Some(7i64) }
fn ok_value() -> Result<i64, String> { .Ok(7i64) }
fn err_value() -> Result<i64, String> { .Err("failed") }
flow stop_policy() -> Unit { drop(.Stop(fade = 120ms))([1i64]...) }
"#,
        None,
    );
    let report = analyze(&fixture).expect("contextual builtin-variant constructor analysis");
    let mut saw_some = false;
    let mut saw_ok = false;
    let mut saw_err = false;
    let mut saw_closed_record = false;
    for selected in report.calls().filter_map(|(_, call)| {
        call.selected_application()
            .map(|application| application.core().candidates().selected())
    }) {
        let crate::callable::ResolvedCallableBaseInstantiation::ExpectedEnum { expected } =
            selected.instantiation()
        else {
            continue;
        };
        let [group] = selected.schema().groups() else {
            panic!("enum constructor retains one parameter group")
        };
        let [parameter] = group.parameters() else {
            panic!("fixture enum constructor retains one payload field")
        };
        match expected {
            TypeKind::Option(item) if item.as_ref() == &TypeKind::I64 => {
                saw_some = parameter.declared_type() == Some(&TypeKind::I64)
                    && parameter.passing() == CallableParameterPassing::PositionalOnly;
            }
            TypeKind::Result { ok, error }
                if ok.as_ref() == &TypeKind::I64 && error.as_ref() == &TypeKind::String =>
            {
                saw_ok |= parameter.declared_type() == Some(&TypeKind::I64)
                    && parameter.passing() == CallableParameterPassing::PositionalOnly;
                saw_err |= parameter.declared_type() == Some(&TypeKind::String)
                    && parameter.passing() == CallableParameterPassing::PositionalOnly;
            }
            _ => {
                saw_closed_record |= parameter.name().is_some_and(|name| name.as_str() == "fade")
                    && parameter.declared_type() == Some(&TypeKind::Duration)
                    && parameter.passing() == CallableParameterPassing::NamedOnly;
            }
        }
    }
    assert!(saw_some && saw_ok && saw_err && saw_closed_record);
}

#[test]
fn evaluated_effect_fields_use_selected_open_argument_identity() {
    let fixture = fixture(
        r#"
fn effect_fields(zeta_value: String, alpha_value: String, payload_value: String) {
    log.info("started", zeta = zeta_value, alpha = alpha_value);
    event.emit("opened", payload = payload_value);
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("evaluated-effect field analysis");
    let effects = report
        .statements()
        .filter_map(|(_, statement)| match statement.role() {
            CheckedStatementRole::EvaluatedEffect(effect) => Some(effect.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(effects.len(), 2, "log and event effects are both retained");

    for (effect, expected_bindings) in
        effects
            .into_iter()
            .filter_map(|effect| match effect.operation() {
                CheckedEvaluatedEffectOperation::Log { .. } => {
                    Some((effect, &["zeta", "alpha"][..]))
                }
                CheckedEvaluatedEffectOperation::EmitEvent { .. } => {
                    Some((effect, &["payload"][..]))
                }
                _ => None,
            })
    {
        let schema = report
            .calls()
            .find_map(|(_, call)| {
                let application = call.selected_application()?;
                (application
                    .core()
                    .candidates()
                    .selected()
                    .schema()
                    .evaluated_effect()
                    == Some(effect.disposition()))
                .then_some(
                    application
                        .core()
                        .candidates()
                        .selected()
                        .schema()
                        .semantic_digest(),
                )
            })
            .expect("selected callable schema for evaluated effect");
        let (CheckedEvaluatedEffectOperation::Log { fields, .. }
        | CheckedEvaluatedEffectOperation::EmitEvent { fields, .. }) = effect.operation()
        else {
            unreachable!("effect filtered above")
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.open_argument().binding().as_str())
                .collect::<Vec<_>>(),
            expected_bindings.to_vec(),
        );
        assert!(
            fields
                .iter()
                .all(|field| field.open_argument().schema() == schema)
        );
        assert!(
            fields
                .iter()
                .all(|field| !field.open_argument().binding().as_str().starts_with("arg"))
        );
    }
}

#[test]
fn evaluated_effect_does_not_fabricate_positional_field_identity() {
    let fixture = fixture(
        "fn positional_field() { log.info(\"started\", 1i64); }\n",
        None,
    );
    assert!(analyze(&fixture).is_err());
}
