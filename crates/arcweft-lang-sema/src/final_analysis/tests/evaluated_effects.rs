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
        .filter_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::EvaluatedEffect(reference) => report
                .expression(reference.site_root())
                .and_then(|expression| expression.evaluated_effect()),
            _ => None,
        })
        .filter_map(|effect| match effect.operation() {
            CheckedEvaluatedEffectOperation::Drop { invocation, .. } => Some(invocation),
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
        .filter_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::EvaluatedEffect(reference) => report
                .expression(reference.site_root())
                .and_then(|expression| expression.evaluated_effect()),
            _ => None,
        })
        .filter_map(|effect| match effect.operation() {
            CheckedEvaluatedEffectOperation::Drop { target, invocation } => {
                Some((target, invocation))
            }
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
            .value_type()
            .expect("compact numeric source value")
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
    let executable = fixture.project.analysis_view().expect("executable HIR");
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
                matches!(
                    application.result().value_type(),
                    Some(TypeKind::ProjectNominal(_))
                )
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
    for application in report
        .calls()
        .filter_map(|(_, call)| call.selected_application())
    {
        let selected = application.core().candidates().selected();
        if !matches!(
            selected.instantiation(),
            crate::callable::ResolvedCallableBaseInstantiation::EnumConstructor
        ) {
            continue;
        }
        let variant = report
            .execution_projection()
            .variant_constructor(
                fixture.project.analysis_view().expect("executable fixture"),
                application,
            )
            .expect("completed constructor projection")
            .expect("enum constructor");
        let [group] = selected.schema().groups() else {
            panic!("enum constructor retains one parameter group")
        };
        let [parameter] = group.parameters() else {
            panic!("fixture enum constructor retains one payload field")
        };
        let parameter_type = application
            .core()
            .solution()
            .instantiate_template(parameter.declared_type().expect("payload parameter type"))
            .expect("completed payload type");
        match variant.owner().ty() {
            TypeKind::Option(item) if item.as_ref() == &TypeKind::I64 => {
                saw_some = parameter_type == TypeKind::I64
                    && parameter.passing() == CallableParameterPassing::PositionalOnly;
            }
            TypeKind::Result { ok, error }
                if ok.as_ref() == &TypeKind::I64 && error.as_ref() == &TypeKind::String =>
            {
                saw_ok |= parameter_type == TypeKind::I64
                    && parameter.passing() == CallableParameterPassing::PositionalOnly;
                saw_err |= parameter_type == TypeKind::String
                    && parameter.passing() == CallableParameterPassing::PositionalOnly;
            }
            _ => {
                saw_closed_record |= parameter.name().is_some_and(|name| name.as_str() == "fade")
                    && parameter_type == TypeKind::Duration
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
        .filter_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::EvaluatedEffect(reference) => report
                .expression(reference.site_root())
                .and_then(|expression| expression.evaluated_effect())
                .map(|effect| {
                    assert_eq!(effect.reference(), *reference);
                    effect
                }),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(effects.len(), 2, "log and event effects are both retained");
    assert_eq!(
        report
            .expressions()
            .filter(|(_, expression)| expression.evaluated_effect().is_some())
            .count(),
        effects.len(),
        "each statement references one unique expression-owned operation"
    );

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
fn value_position_effect_is_owned_by_its_expression_site() {
    let fixture = fixture(
        r#"
fn tail_log(message: String) -> Unit {
    log.info(message)
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("value-position effect-call analysis");
    let effects = report
        .expressions()
        .filter_map(|(owner, expression)| {
            expression.evaluated_effect().map(|effect| (owner, effect))
        })
        .collect::<Vec<_>>();
    let [(owner, effect)] = effects.as_slice() else {
        panic!("one operation is owned by one checked expression root")
    };

    assert_eq!(effect.site_root(), *owner);
    assert_eq!(effect.result(), &TypeKind::Unit);
    let projection = report.execution_projection();
    let execution = projection
        .plan(*owner)
        .expect("effect expression execution plan");
    assert!(!execution.executes_as_runtime_call());
    assert!(execution.is_evaluated_effect_carrier());
    assert!(execution.evaluated_effect_roles().iter().any(|role| {
        matches!(
            role,
            crate::final_analysis::CheckedEvaluatedEffectRole::ExpressionRoot { root }
                if root == owner
        )
    }));
    assert!(report.statements().all(|(_, statement)| !matches!(
        statement.payload(),
        CheckedStatementPayload::EvaluatedEffect(_)
    )));
}

#[test]
fn evaluated_effect_does_not_fabricate_positional_field_identity() {
    let fixture = fixture(
        "fn positional_field() { log.info(\"started\", 1i64); }\n",
        None,
    );
    assert!(analyze(&fixture).is_err());
}
