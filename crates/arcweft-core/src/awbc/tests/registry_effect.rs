use super::*;
use arcweft_id::{DeclarationName, LifetimeRegistryKey, LifetimeScopeKind};

fn registry_program(scope: LifetimeScopeKind, path: &[&str], declared: &str) -> AwbcProgram {
    let mut program = minimal_program();
    program
        .runtime_types
        .push(runtime_type(1, AwbcRuntimeTypeShape::Bool));
    let effects = add_effect_set(&mut program, &[declared]);
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: None,
        effects,
    });
    let capability = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push("state.write".to_owned());
    program.effect_plans.push(AwbcEffectPlan {
        signature: AwbcSignatureId(1),
        capability: Some(capability),
        payload: AwbcEffectPayload::RegistryWrite {
            key: LifetimeRegistryKey::new(
                scope,
                path.iter()
                    .map(|name| DeclarationName::try_new(*name).unwrap())
                    .collect(),
            ),
            value_type: AwbcTypeId(0),
        },
        resources: Vec::new(),
    });
    program.canonicalize_string_table();
    program
}

#[test]
fn registry_effect_round_trip_preserves_root_unicode_and_named_keys() {
    for (scope, path, declared) in [
        (
            LifetimeScopeKind::Flow,
            vec!["flags", "seen"],
            "state.write('flow)",
        ),
        (
            LifetimeScopeKind::Persistent,
            vec![],
            "state.write('persistent)",
        ),
        (
            LifetimeScopeKind::parse("章").unwrap(),
            vec!["進行", "旗"],
            "state.write('章)",
        ),
    ] {
        let program = registry_program(scope, &path, declared);
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        let encoded = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded.effect_plans, program.effect_plans);
        assert_eq!(
            decoded.effect_plans[0].kind(),
            AwbcEffectKind::RegistryWrite
        );
        decoded
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
    }
}

#[test]
fn registry_effect_requires_scoped_effect_coverage_on_segment_boundaries() {
    for declared in [
        "state.write('scene)",
        "state.write('flow.flag)",
        "log.write",
    ] {
        let program = registry_program(LifetimeScopeKind::Flow, &["flags", "seen"], declared);
        assert!(
            matches!(
                program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
                Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
            ),
            "accepted unrelated row {declared}"
        );
    }
    let program = registry_program(
        LifetimeScopeKind::Flow,
        &["flags", "seen"],
        "state.write('flow.flags)",
    );
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
}

#[test]
fn registry_effect_rejects_missing_capability_wrong_signature_and_unknown_value_type() {
    let original = registry_program(LifetimeScopeKind::Flow, &["seen"], "state.write('flow)");
    for fault in 0..4 {
        let mut program = original.clone();
        match fault {
            0 => program.effect_plans[0].capability = None,
            1 => program.signatures[1].params.clear(),
            2 => program.signatures[1].result = Some(AwbcTypeId(0)),
            3 => {
                let AwbcEffectPayload::RegistryWrite { value_type, .. } =
                    &mut program.effect_plans[0].payload
                else {
                    unreachable!()
                };
                *value_type = AwbcTypeId(999);
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
                Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
                    | Err(AwbcVerifyError::IndexOutOfBounds {
                        table: "runtime_types",
                        index: 999,
                        ..
                    })
            ),
            "accepted invalid registry plan {fault}"
        );
    }
}
