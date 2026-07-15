use super::*;
use crate::types::TypeKind;

#[test]
fn effect_capability_parses_family_operation_and_scope() {
    let capability = EffectCapability::new("state.write(flow)");
    let parts = capability.parts();

    assert_eq!(capability.as_str(), "state.write(flow)");
    assert_eq!(parts.family(), "state");
    assert_eq!(parts.operation(), "write");
    assert_eq!(parts.scope(), Some("flow"));
}

#[test]
fn typecheck_env_stores_capabilities_as_typed_ids() {
    let env = TypeCheckEnv::new()
        .with_capability(EffectCapability::new("fs.read"))
        .with_function_effects("adapter.read", [EffectCapability::new("fs.read")]);

    assert!(env.has_capability("fs.read"));
    assert_eq!(
        env.function_effects("adapter.read").map(|effects| {
            effects
                .iter()
                .map(EffectCapability::as_str)
                .collect::<Vec<_>>()
        }),
        Some(vec!["fs.read"])
    );
}

#[test]
fn standard_env_contains_dialogue_fmt_builtin() {
    assert_eq!(
        TypeCheckEnv::standard().function_type("fmt"),
        Some(&TypeKind::DisplayText)
    );
}
