use super::*;
use crate::callable::{CallableName, CallableParameterPresence, CallablePath};
use crate::types::{
    CompileTimeScalarKind, EntityKind, GenericParameterOwnerId, GenericTypeParameterId,
    LanguageIntrinsicGenericOwner, StandardMapFamily, TypeKind,
};
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
    let content = env
        .standard_dialogue_content_type()
        .expect("standard DialogueContent owner is accepted");
    assert_eq!(function.schema.value_type(), Some(&content));
    assert!(matches!(
        function.schema.validator(),
        crate::callable::CallableValidator::Format
    ));
    assert_eq!(
        function.schema.argument_policy().unknown_named(),
        crate::callable::UnknownNamedArgumentPolicy::Reject
    );
    let [group] = function.schema.groups() else {
        panic!("fmt has one normalized parameter group")
    };
    let parameters = group.parameters();
    assert_eq!(parameters.len(), 9);
    assert_eq!(
        parameters
            .iter()
            .map(|parameter| parameter
                .name()
                .expect("fmt parameters have source labels")
                .as_str())
            .collect::<Vec<_>>(),
        [
            "value",
            "style",
            "locale",
            "currency",
            "none",
            "color",
            "on_error",
            "fallback",
            "discard_error",
        ]
    );
    assert_eq!(
        parameters[0].presence(),
        CallableParameterPresence::Required
    );
    assert_eq!(parameters[1].declared_type(), Some(&TypeKind::String));
    assert_eq!(parameters[2].declared_type(), Some(&TypeKind::String));
    assert_eq!(parameters[3].declared_type(), Some(&TypeKind::String));
    assert_eq!(parameters[4].declared_type(), Some(&TypeKind::String));
    assert!(matches!(
        parameters[5].declared_type(),
        Some(TypeKind::CompileTimeScalar(color))
            if color.kind() == CompileTimeScalarKind::Color
    ));
    assert_eq!(
        parameters[6].declared_type(),
        Some(&TypeKind::Named("InlineFailure".to_owned()))
    );
    assert_eq!(parameters[7].declared_type(), Some(&TypeKind::String));
    assert_eq!(parameters[8].declared_type(), Some(&TypeKind::Bool));
}

#[test]
fn standard_need_producers_publish_typed_operation_and_join_policy() {
    let environment = TypeCheckEnv::standard();
    for (path, expected, expected_ok, expected_error) in [
        (
            ["asset", "image"],
            crate::callable::CallableNeedProducerRole::asset_image(),
            "ImageHandle",
            "AssetError",
        ),
        (
            ["voice", "load"],
            crate::callable::CallableNeedProducerRole::voice_load(),
            "AudioHandle",
            "VoiceError",
        ),
    ] {
        let path = CallablePath::try_new(
            path.into_iter()
                .map(CallableName::try_new)
                .collect::<Result<Vec<_>, _>>()
                .expect("typed standard producer path"),
        )
        .expect("standard path is nonempty");
        let function = environment
            .standard_functions()
            .iter()
            .find(|function| function.path == path)
            .expect("standard producer schema is registered");
        let crate::callable::CallableValidator::NeedProducer(role) = function.schema.validator()
        else {
            panic!("standard Need producer must retain its typed role")
        };
        assert_eq!(*role, expected);
        let Some(TypeKind::Need(payload)) = function.schema.value_type() else {
            panic!("standard asset and voice producers return Need<Result<_, _>>")
        };
        let TypeKind::Result { ok, error } = payload.as_ref() else {
            panic!("standard asset and voice Need payloads are Result values")
        };
        let TypeKind::AcceptedNominal(ok) = ok.as_ref() else {
            panic!("standard asset and voice successes are accepted opaque nominals")
        };
        let TypeKind::AcceptedNominal(error) = error.as_ref() else {
            panic!("standard asset and voice failures are accepted opaque nominals")
        };
        assert_eq!(
            ok.declaration()
                .canonical_path()
                .segments()
                .last()
                .expect("accepted path has one type segment")
                .as_str(),
            expected_ok
        );
        assert_eq!(
            error
                .declaration()
                .canonical_path()
                .segments()
                .last()
                .expect("accepted path has one type segment")
                .as_str(),
            expected_error
        );
    }

    let load_bg = CallablePath::try_new(vec![CallableName::try_new("load_bg").unwrap()])
        .expect("load_bg standard path");
    let unbacked = environment
        .standard_functions()
        .iter()
        .find(|function| function.path == load_bg)
        .expect("legacy typed declaration remains visible to sema");
    assert!(matches!(
        unbacked.schema.validator(),
        crate::callable::CallableValidator::Ordinary
    ));
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
fn entity_write_schemas_match_signal_watch_and_metric_payloads() {
    let environment = TypeCheckEnv::standard();
    let signal_value = TypeKind::generic_parameter(GenericTypeParameterId::new(
        GenericParameterOwnerId::LanguageIntrinsic(LanguageIntrinsicGenericOwner::SignalWrite),
        0,
    ));
    let metric_value = TypeKind::generic_parameter(GenericTypeParameterId::new(
        GenericParameterOwnerId::LanguageIntrinsic(LanguageIntrinsicGenericOwner::MetricWrite),
        0,
    ));
    let watch_record = environment
        .nominal_catalog
        .exact(super::nominal::standard_nominal_id("Watch").canonical_path())
        .expect("Watch is one accepted standard nominal");
    let signal_target_value = watch_record
        .try_instantiate([signal_value.clone()])
        .expect("Watch has one payload type parameter");
    let callable = |namespace: &str| {
        CallablePath::try_new(vec![
            CallableName::try_new(namespace).expect("callable namespace"),
            CallableName::try_new("set").expect("callable member"),
        ])
        .expect("entity write callable path")
    };
    let signal = environment
        .standard_functions()
        .iter()
        .find(|function| function.path == callable("signal"))
        .expect("signal.set has one typed standard schema");
    let signal_parameters = signal.schema.groups()[0].parameters();
    assert_eq!(
        signal_parameters[0].declared_type(),
        Some(&TypeKind::entity_ref_with_value(
            EntityKind::Signal,
            signal_target_value,
        ))
    );
    assert_eq!(signal_parameters[1].declared_type(), Some(&signal_value));

    let metric = environment
        .standard_functions()
        .iter()
        .find(|function| function.path == callable("metric"))
        .expect("metric.set has one typed standard schema");
    let metric_parameters = metric.schema.groups()[0].parameters();
    assert_eq!(
        metric_parameters[0].declared_type(),
        Some(&TypeKind::entity_ref_with_value(
            EntityKind::Metric,
            metric_value.clone(),
        ))
    );
    assert_eq!(metric_parameters[1].declared_type(), Some(&metric_value));
}

#[test]
fn data_standard_callables_publish_only_the_typed_contracts() {
    let environment = TypeCheckEnv::standard();
    let data_path = |member: &str| {
        CallablePath::try_new(vec![
            CallableName::try_new("data").unwrap(),
            CallableName::try_new(member).unwrap(),
        ])
        .unwrap()
    };
    let functions = environment.standard_functions();
    let shape = functions
        .iter()
        .find(|function| function.path == data_path("shape"))
        .expect("data.shape has one typed standard schema");
    assert!(matches!(
        shape.schema.result_schema().value_type(),
        Some(TypeKind::DataShape(_))
    ));
    assert_eq!(shape.schema.generic_inventory().types().len(), 1);
    assert_eq!(shape.schema.groups()[0].parameters().len(), 1);
    assert_eq!(
        shape.schema.groups()[0].parameters()[0].presence(),
        CallableParameterPresence::Optional
    );

    let encode = functions
        .iter()
        .find(|function| function.path == data_path("encode"))
        .expect("data.encode has one typed standard schema");
    assert_eq!(encode.schema.generic_inventory().types().len(), 1);
    assert_eq!(encode.schema.groups()[0].parameters().len(), 3);
    assert_eq!(
        encode.schema.groups()[0].parameters()[2].presence(),
        CallableParameterPresence::Optional
    );
    assert_eq!(
        encode.schema.result_schema().value_type(),
        Some(&TypeKind::Result {
            ok: Box::new(TypeKind::Bytes),
            error: Box::new(TypeKind::DataError),
        })
    );

    let decode = functions
        .iter()
        .filter(|function| function.path == data_path("decode"))
        .collect::<Vec<_>>();
    assert_eq!(decode.len(), 2);
    let untyped = decode
        .iter()
        .find(|function| function.schema.groups()[0].parameters().len() == 2)
        .expect("two-argument decode has a closed DataValue result");
    assert!(untyped.schema.generic_inventory().types().is_empty());
    assert_eq!(
        untyped.schema.result_schema().value_type(),
        Some(&TypeKind::Result {
            ok: Box::new(TypeKind::DataValue),
            error: Box::new(TypeKind::DataError),
        })
    );
    let typed = decode
        .iter()
        .find(|function| function.schema.groups()[0].parameters().len() == 3)
        .expect("three-argument decode has a shape-inferred result");
    assert_eq!(typed.schema.generic_inventory().types().len(), 1);
    assert!(matches!(
        typed.schema.groups()[0].parameters()[2].declared_type(),
        Some(TypeKind::DataShape(_))
    ));
    assert!(matches!(
        typed.schema.result_schema().value_type(),
        Some(TypeKind::Result { ok, error })
            if matches!(ok.as_ref(), TypeKind::GenericParam(_))
                && error.as_ref() == &TypeKind::DataError
    ));
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
fn data_value_and_error_enums_keep_the_complete_ordered_payload_algebra() {
    let environment = TypeCheckEnv::standard();
    let data_value = environment
        .closed_enum(&TypeKind::DataValue)
        .expect("DataValue is a closed source ADT");
    assert_eq!(
        data_value
            .variants()
            .iter()
            .map(|variant| variant.name())
            .collect::<Vec<_>>(),
        [
            "Unit", "Bool", "I128", "U128", "F32", "F64", "String", "Char", "Bytes", "Option",
            "Seq", "Tuple", "Map", "Record", "Enum",
        ]
    );
    assert_eq!(
        data_value.variants()[0].payload(),
        &EnumVariantPayload::Unit
    );
    assert_eq!(
        data_value.variants()[1].payload(),
        &EnumVariantPayload::Tuple(vec![TypeKind::Bool])
    );
    assert_eq!(
        data_value.variants()[2].payload(),
        &EnumVariantPayload::Tuple(vec![TypeKind::I128])
    );
    assert_eq!(
        data_value.variants()[3].payload(),
        &EnumVariantPayload::Tuple(vec![TypeKind::U128])
    );
    let EnumVariantPayload::Record(map_fields) = data_value.variants()[12].payload() else {
        panic!("DataValue::Map preserves its map kind and typed entries");
    };
    assert_eq!(map_fields[0].name(), "kind");
    assert_eq!(map_fields[0].ty(), &TypeKind::DataMapKind);
    assert_eq!(map_fields[1].name(), "entries");
    assert_eq!(
        map_fields[1].ty(),
        &TypeKind::Vec(Box::new(TypeKind::Tuple(vec![
            TypeKind::DataValue,
            TypeKind::DataValue,
        ])))
    );

    let data_error_kind = environment
        .closed_enum(&TypeKind::DataErrorKind)
        .expect("DataErrorKind is closed");
    assert_eq!(
        data_error_kind
            .variants()
            .iter()
            .map(|variant| variant.name())
            .collect::<Vec<_>>(),
        [
            "MissingField",
            "UnknownField",
            "DuplicateField",
            "InvalidType",
            "InvalidEnumTag",
            "NumberOutOfRange",
            "InvalidEncoding",
            "TrailingData",
            "LimitExceeded",
            "UnsupportedFormat",
            "Io",
            "Custom",
        ]
    );
    assert!(
        data_error_kind
            .variants()
            .iter()
            .all(|variant| variant.payload() == &EnumVariantPayload::Unit)
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
            if matches!(fields.as_ref(), [field]
                if field.name() == "fade" && field.ty() == &TypeKind::Duration)
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
            StandardDropPolicyValue::Stop {
                fade: arcweft_core::time::LogicalDuration::from_nanos(0)
            }
        ))
    );
}

#[test]
fn enum_record_payload_preserves_declaration_order_and_rejects_duplicates() {
    let payload =
        EnumVariantPayload::record([("second", TypeKind::String), ("first", TypeKind::Bool)])
            .expect("distinct declaration-ordered fields");
    let EnumVariantPayload::Record(fields) = payload else {
        panic!("record payload remains a record");
    };
    assert_eq!(
        fields
            .iter()
            .map(super::enums::EnvironmentRecordField::name)
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    assert!(matches!(
        EnumVariantPayload::record([
            ("duplicate", TypeKind::Bool),
            ("duplicate", TypeKind::String),
        ]),
        Err(super::EnumVariantPayloadBuildError::DuplicateRecordField { name })
            if name == "duplicate"
    ));
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

#[test]
fn record_variant_payload_preserves_declaration_order_and_rejects_duplicate_names() {
    let payload = EnumVariantPayload::record([("z", TypeKind::I64), ("a", TypeKind::Bool)])
        .expect("distinct record fields are accepted in declaration order");
    let EnumVariantPayload::Record(fields) = payload else {
        panic!("record constructor retains the record payload family")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.name(), field.ty()))
            .collect::<Vec<_>>(),
        vec![("z", &TypeKind::I64), ("a", &TypeKind::Bool)]
    );

    for duplicate_type in [TypeKind::I64, TypeKind::Bool] {
        assert_eq!(
            EnumVariantPayload::record([("field", TypeKind::I64), ("field", duplicate_type),]),
            Err(EnumVariantPayloadBuildError::DuplicateRecordField {
                name: "field".to_owned(),
            })
        );
    }
}

#[test]
fn standard_drop_policy_retains_the_exact_record_payload_schema() {
    let environment = TypeCheckEnv::standard();
    let schema = environment
        .closed_enum(&TypeKind::Named("DropPolicy".to_owned()))
        .expect("DropPolicy is one closed environment enum");
    let stop = &schema.variants()[1];
    assert_eq!(stop.name(), "Stop");
    assert!(matches!(
        stop.payload(),
        EnumVariantPayload::Record(fields)
            if matches!(fields.as_ref(), [field]
                if field.name() == "fade" && field.ty() == &TypeKind::Duration)
    ));
}
