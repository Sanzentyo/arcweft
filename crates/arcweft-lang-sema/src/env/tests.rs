use super::*;
use crate::callable::{CallableName, CallablePath};
use crate::types::{StandardMapFamily, TypeKind};
use arcweft_data::DataFormat;
use std::collections::BTreeSet;

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
    let env = TypeCheckEnv::new().with_capability(EffectCapability::new("fs.read"));

    assert!(env.has_capability("fs.read"));
}

#[test]
fn standard_env_contains_dialogue_fmt_builtin() {
    let fmt = CallablePath::try_new(vec![CallableName::try_new("fmt").unwrap()]).unwrap();
    let env = TypeCheckEnv::standard();
    let function = env
        .standard_functions()
        .iter()
        .find(|function| function.path == fmt)
        .expect("fmt has one typed standard callable record");
    assert_eq!(function.schema.result(), &TypeKind::DisplayText);
}

#[test]
fn standard_callable_inventory_is_typed_before_publication() {
    let environment = TypeCheckEnv::standard();
    let functions = environment.standard_functions();
    let unique_identities = functions
        .iter()
        .map(|function| (&function.path, function.overload))
        .collect::<BTreeSet<_>>();
    assert_eq!(unique_identities.len(), functions.len());
    assert!(functions.iter().all(|function| {
        function
            .path
            .segments()
            .iter()
            .all(|segment| !segment.as_str().contains('.'))
    }));

    let data_encode = CallablePath::try_new(vec![
        CallableName::try_new("data").unwrap(),
        CallableName::try_new("encode").unwrap(),
    ])
    .unwrap();
    assert!(
        functions
            .iter()
            .any(|function| function.path == data_encode)
    );

    let drop = CallablePath::try_new(vec![CallableName::try_new("drop").unwrap()]).unwrap();
    let drop_overloads = functions
        .iter()
        .filter(|function| function.path == drop)
        .collect::<Vec<_>>();
    assert_eq!(drop_overloads.len(), 2);
    assert!(matches!(
        drop_overloads[0].schema.validator(),
        crate::callable::CallableValidator::Drop(crate::callable::DropCallableId::Drop)
    ));
    assert!(matches!(
        drop_overloads[1].schema.validator(),
        crate::callable::CallableValidator::Drop(crate::callable::DropCallableId::DropWithPolicy)
    ));
    assert_eq!(drop_overloads[0].schema.groups().len(), 1);
    assert_eq!(drop_overloads[1].schema.groups().len(), 2);
    assert_eq!(
        drop_overloads[1].schema.groups()[0].parameters()[0].declared_type(),
        Some(&TypeKind::Named("DropPolicy".to_owned()))
    );

    let methods = environment.standard_methods();
    for (index, method) in methods.iter().enumerate() {
        assert!(
            methods[index + 1..].iter().all(|other| {
                method.receiver != other.receiver || method.member != other.member
            })
        );
    }
    assert!(
        methods
            .iter()
            .all(|method| !method.member.as_str().contains('.'))
    );
}

#[test]
fn standard_map_publishes_only_eager_families_from_one_typed_inventory() {
    let environment = TypeCheckEnv::standard();
    let map = CallablePath::try_new(vec![CallableName::try_new("map").unwrap()]).unwrap();
    let rows = environment
        .standard_functions()
        .iter()
        .filter(|function| function.path == map)
        .collect::<Vec<_>>();

    assert_eq!(rows.len(), StandardMapFamily::PUBLISHED.len());
    assert_eq!(
        rows.iter()
            .map(|row| match row.schema.validator() {
                crate::callable::CallableValidator::StandardMap(family) => *family,
                validator => panic!("map row has a foreign validator: {validator:?}"),
            })
            .collect::<Vec<_>>(),
        StandardMapFamily::PUBLISHED
    );
    assert!(rows.iter().all(|row| {
        let crate::callable::CallableValidator::StandardMap(family) = row.schema.validator() else {
            return false;
        };
        row.overload == family.overload()
            && row.schema.extension_receiver()
                == Some(crate::callable::CallableExtensionReceiver::new(
                    crate::callable::CallableGroupIndex::try_from_usize(1).expect("second group"),
                    crate::callable::CallableParameterIndex::try_from_usize(0)
                        .expect("first parameter"),
                ))
    }));
    assert!(
        rows.iter().all(|row| !matches!(
            row.schema.validator(),
            crate::callable::CallableValidator::StandardMap(
                StandardMapFamily::Need | StandardMapFamily::Parser | StandardMapFamily::Stream
            )
        )),
        "producer/parser map families must stay unpublished until their runtime owners exist"
    );
    assert!(
        environment
            .standard_methods()
            .iter()
            .all(|method| method.member.as_str() != "map"),
        "dot map lookup is derived from the explicit receiver row, not a shadow method"
    );
}

#[test]
fn standard_closed_enum_inventories_preserve_owner_authored_order() {
    let environment = TypeCheckEnv::standard();
    let inventories = environment.enum_variant_sets();

    let data_format = inventories
        .iter()
        .find(|(ty, _)| ty == &TypeKind::DataFormat)
        .map(|(_, variants)| variants)
        .expect("DataFormat has one closed environment enum inventory");
    assert_eq!(
        data_format,
        &DataFormat::ALL
            .map(DataFormat::variant_name)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    );

    let presentation = inventories
        .iter()
        .find(|(ty, _)| ty == &TypeKind::Named("PresentationLifetime".to_owned()))
        .map(|(_, variants)| variants)
        .expect("PresentationLifetime has one closed environment enum inventory");
    assert_eq!(
        presentation,
        &[
            "frame",
            "tick",
            "cue",
            "line",
            "scene",
            "flow",
            "session",
            "global",
            "persistent",
        ]
        .map(str::to_owned)
    );

    let capture_format = inventories
        .iter()
        .find(|(ty, _)| {
            ty == &TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureFormat)
        })
        .map(|(_, variants)| variants)
        .expect("CaptureFormat has one closed environment enum inventory");
    assert_eq!(capture_format, &["png", "raw_rgba"].map(str::to_owned));

    let capture_kind = inventories
        .iter()
        .find(|(ty, _)| ty == &TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureKind))
        .map(|(_, variants)| variants)
        .expect("CaptureKind has one closed environment enum inventory");
    assert_eq!(capture_kind, &["color", "mask"].map(str::to_owned));

    let pointer_button = inventories
        .iter()
        .find(|(ty, _)| {
            ty == &TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::PointerButton)
        })
        .map(|(_, variants)| variants)
        .expect("PointerButton has one closed environment enum inventory");
    assert_eq!(
        pointer_button,
        &["primary", "secondary", "middle"].map(str::to_owned)
    );
    assert!(
        inventories
            .iter()
            .all(|(ty, _)| ty != &TypeKind::ActionName),
        "open Agent action names must not be collapsed into a pointer-button enum"
    );
}

#[test]
fn standard_drop_policy_keeps_payload_case_and_zero_fade_alias_distinct() {
    let environment = TypeCheckEnv::standard();
    let policy_type = TypeKind::Named("DropPolicy".to_owned());
    let policy = environment
        .closed_enum(&policy_type)
        .expect("DropPolicy has one closed typed inventory");
    assert_eq!(
        policy
            .variants()
            .iter()
            .map(super::base::EnvironmentEnumVariant::name)
            .collect::<Vec<_>>(),
        ["Cancel", "Stop", "Finish", "Release", "Detach"]
    );
    assert!(matches!(
        policy.variants()[0].payload(),
        EnumVariantPayload::Unit
    ));
    assert!(matches!(
        policy.variants()[1].payload(),
        EnumVariantPayload::Record(fields)
            if fields.get("fade") == Some(&TypeKind::Duration) && fields.len() == 1
    ));
    assert!(
        policy.variants()[2..]
            .iter()
            .all(|variant| matches!(variant.payload(), EnumVariantPayload::Unit))
    );
    for (ordinal, expected) in [
        StandardDropPolicyCase::Cancel,
        StandardDropPolicyCase::Stop,
        StandardDropPolicyCase::Finish,
        StandardDropPolicyCase::Release,
        StandardDropPolicyCase::Detach,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            environment.standard_drop_policy_case(
                policy.owner(),
                u32::try_from(ordinal).expect("standard policy ordinal fits u32"),
            ),
            Some(expected)
        );
    }
    let stop_now =
        identity::EnvironmentBindingId::try_new("stop_now").expect("standard value binding");
    assert_eq!(
        environment.standard_environment_value(&stop_now),
        Some(StandardEnvironmentValue::DropPolicy(
            StandardDropPolicyValue::Stop { fade_nanos: 0 }
        ))
    );
}

#[test]
fn standard_dialogue_voice_owns_auto_variant() {
    let environment = TypeCheckEnv::standard();
    let schema = environment
        .closed_enum(&TypeKind::Named("DialogueVoice".to_owned()))
        .expect("DialogueVoice is a closed standard enum");
    assert_eq!(
        schema
            .variants()
            .iter()
            .map(|variant| variant.name())
            .collect::<Vec<_>>(),
        ["auto"]
    );
}

#[test]
fn closed_enum_construction_rejects_duplicate_cases_and_conflicting_owners() {
    let first = identity::EnvironmentBindingId::try_new("First").expect("valid owner");
    let second = identity::EnvironmentBindingId::try_new("Second").expect("valid owner");
    let ty = TypeKind::Named("Closed".to_owned());

    assert!(matches!(
        TypeCheckEnv::new().try_with_enum_variants(
            first.clone(),
            ty.clone(),
            ["second", "first", "second"],
        ),
        Err(TypeCheckEnvBuildError::DuplicateEnumVariant { owner, variant })
            if owner == first && variant == "second"
    ));

    let environment = TypeCheckEnv::new()
        .try_with_enum_variants(first.clone(), ty.clone(), ["second", "first"])
        .expect("first owner is accepted");
    assert!(matches!(
        environment.try_with_enum_variants(second.clone(), ty.clone(), ["third"]),
        Err(TypeCheckEnvBuildError::ConflictingEnumTypeOwner {
            ty: conflicting,
            existing,
            requested,
        }) if *conflicting == ty && existing == first && requested == second
    ));
}
