use super::*;
use crate::awbc::codec::AwbcDecodeBudget;
use arcweft_id::{DeclarationName, LifetimeRegistryKey, LifetimeScopeKind};

fn registry_plan() -> AwbcEffectPlan {
    AwbcEffectPlan {
        signature: AwbcSignatureId(0),
        capability: None,
        payload: AwbcEffectPayload::RegistryWrite {
            key: LifetimeRegistryKey::new(
                LifetimeScopeKind::Flow,
                [DeclarationName::try_new("seen").unwrap()].into(),
            ),
            value_type: AwbcTypeId(0),
        },
        resources: Vec::new(),
    }
}

#[test]
fn wait_duration_source_roundtrips_rejects_truncation_unknown_tag_and_old_serde_payload() {
    use crate::awbc::schema::AwbcWaitDuration;
    for source in [
        AwbcWaitDuration::Constant(AwbcConstantId(128)),
        AwbcWaitDuration::Input,
    ] {
        let plan = AwbcEffectPlan {
            signature: AwbcSignatureId(0),
            capability: None,
            payload: AwbcEffectPayload::Wait(source),
            resources: Vec::new(),
        };
        let mut writer = Writer::default();
        plan.write_wire(&mut writer).unwrap();
        let mut bytes = writer.into_bytes();
        for end in 0..bytes.len() {
            assert!(
                AwbcEffectPlan::read_wire(&mut Reader::new(
                    &bytes[..end],
                    &AwbcDecodeBudget::default()
                ))
                .is_err()
            );
        }
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(AwbcEffectPlan::read_wire(&mut reader).unwrap(), plan);
        reader.finish().unwrap();
        // Kind, signature and absent capability precede the sole Duration source.
        bytes[3] = 255;
        assert!(matches!(
            AwbcEffectPlan::read_wire(&mut Reader::new(&bytes, &AwbcDecodeBudget::default())),
            Err(AwbcCodecError::UnknownTag {
                kind: "wait Duration source",
                tag: 255,
                ..
            })
        ));
    }
    assert!(serde_json::from_str::<AwbcEffectPayload>(r#"{"wait":[0]}"#).is_err());
    assert!(AwbcEffectPayload::try_from((AwbcEffectKind::Wait, vec![])).is_err());
    assert!(
        AwbcEffectPayload::try_from((
            AwbcEffectKind::Wait,
            vec![AwbcConstantId(0), AwbcConstantId(1)]
        ))
        .is_err()
    );
}

#[test]
fn audio_effect_decoder_rejects_every_incomplete_typed_payload() {
    let plan = AwbcEffectPlan {
        signature: AwbcSignatureId(0),
        capability: None,
        payload: AwbcEffectPayload::Audio(AwbcAudioCommandId(128)),
        resources: Vec::new(),
    };
    let mut writer = Writer::default();
    plan.write_wire(&mut writer).unwrap();
    let bytes = writer.into_bytes();
    for end in 0..bytes.len() {
        let mut reader = Reader::new(&bytes[..end], &AwbcDecodeBudget::default());
        assert!(matches!(
            AwbcEffectPlan::read_wire(&mut reader),
            Err(AwbcCodecError::Truncated { .. })
        ));
    }
    let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
    assert_eq!(AwbcEffectPlan::read_wire(&mut reader).unwrap(), plan);
    reader.finish().unwrap();
}

#[test]
fn registry_effect_decoder_shares_key_string_and_collection_budgets() {
    let plan = registry_plan();
    let mut writer = Writer::default();
    plan.write_wire(&mut writer).unwrap();
    plan.write_wire(&mut writer).unwrap();
    let bytes = writer.into_bytes();
    for (budget, name, actual, limit) in [
        (
            AwbcDecodeBudget {
                string_bytes: 15,
                ..AwbcDecodeBudget::default()
            },
            "string_bytes",
            16,
            15,
        ),
        (
            AwbcDecodeBudget {
                collection_items: 1,
                ..AwbcDecodeBudget::default()
            },
            "collection_items",
            2,
            1,
        ),
    ] {
        let mut reader = Reader::new(&bytes, &budget);
        assert_eq!(AwbcEffectPlan::read_wire(&mut reader).unwrap(), plan);
        assert!(matches!(
            AwbcEffectPlan::read_wire(&mut reader),
            Err(AwbcCodecError::BudgetExceeded {
                budget: actual_name,
                actual: actual_count,
                limit: actual_limit
            }) if actual_name == name && actual_count == actual && actual_limit == limit
        ));
    }
}

#[test]
fn registry_key_bounds_precede_name_materialization_and_grammar_diagnostics() {
    let mut writer = Writer::default();
    writer.write_str("flow").unwrap();
    writer
        .write_table(&["bad.name".to_owned(), "later".to_owned()])
        .unwrap();
    let bytes = writer.into_bytes();
    let budget = AwbcDecodeBudget {
        string_bytes: 12,
        ..AwbcDecodeBudget::default()
    };
    let mut reader = Reader::new(&bytes, &budget);
    assert!(matches!(
        reader.read_registry_key(),
        Err(AwbcCodecError::BudgetExceeded {
            budget: "string_bytes",
            actual: 17,
            limit: 12
        })
    ));
    let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
    assert!(matches!(
        reader.read_registry_key(),
        Err(AwbcCodecError::InvalidRegistryKey { .. })
    ));
}
