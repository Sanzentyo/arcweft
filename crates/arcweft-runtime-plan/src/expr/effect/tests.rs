use super::*;
use arcweft_lang_syntax::expr::parse_expr;
use std::collections::BTreeMap;

fn lower(source: &str) -> Result<LoweredRuntimeEffect, String> {
    let expr = parse_expr(source).expect("effect fixture parses");
    let ids = BTreeMap::new();
    let helpers = Vec::new();
    lower_runtime_effect_strict_with_pure(&expr, RuntimePureHelperLookup::new(&ids, &helpers))
}

#[test]
fn fixed_effect_arguments_bind_by_name_in_authored_order() {
    let lowered = lower("signal.set(value = score, target = @signal.current)")
        .expect("typed signal effect lowers");

    assert!(matches!(
        lowered,
        LoweredRuntimeEffect::Evaluated(RuntimeEffectExpr::SignalWrite {
            target: RuntimeExpr::EntityRef(target),
            value: RuntimeExpr::Local(value),
        }) if target == "signal.current" && value == "score"
    ));
}

#[test]
fn field_effect_keeps_named_fields_distinct_from_its_head_argument() {
    let lowered =
        lower("log.info(user = user_id, message = message)").expect("typed log effect lowers");

    let LoweredRuntimeEffect::Evaluated(RuntimeEffectExpr::Log {
        level,
        message,
        fields,
    }) = lowered
    else {
        panic!("expected an evaluated log effect");
    };
    assert_eq!(level, "info");
    assert_eq!(message, RuntimeExpr::Local("message".to_owned()));
    assert_eq!(
        fields,
        [RuntimeEffectFieldExpr {
            name: "user".to_owned(),
            value: RuntimeExpr::Local("user_id".to_owned()),
        }]
    );
}

#[test]
fn closed_generic_effect_preserves_named_and_spread_source_shape() {
    let lowered = lower("analytics.track(name = \"open\", [1i64, 2i64]...)")
        .expect("closed generic effect remains a static host call");

    let LoweredRuntimeEffect::Static(LineEffectRequest::Call(call)) = lowered else {
        panic!("expected a static generic effect call");
    };
    assert_eq!(call.callee, "analytics.track");
    assert_eq!(call.args[0], "name = \"open\"");
    assert!(call.args[1].ends_with("..."), "spread marker was lost");
}

#[test]
fn runtime_valued_generic_effect_requires_a_typed_boundary() {
    let error = lower("analytics.track(name = current_name)")
        .expect_err("dynamic generic calls must not flatten named arguments");

    assert_eq!(
        error,
        "generic effect call `analytics.track` has runtime-valued arguments but no typed effect boundary"
    );
}

#[test]
fn old_assertion_call_spellings_are_not_builtin_effects() {
    for callee in ["assert", "debug_assert"] {
        let lowered = lower(&format!("{callee}(true)"))
            .expect("closed ordinary call remains a generic static host call");

        let LoweredRuntimeEffect::Static(LineEffectRequest::Call(call)) = lowered else {
            panic!("old assertion spelling must not produce an assertion effect");
        };
        assert_eq!(call.callee, callee);
        assert_eq!(call.args, ["true"]);
    }
}

#[test]
fn typed_effect_rejects_spread_at_its_boundary() {
    let error = lower("signal.set([@signal.current, score]...)")
        .expect_err("typed effect spread requires an explicit signature expansion");

    assert_eq!(
        error,
        "signal.set does not accept spread arguments at the effect boundary"
    );
}
