use super::*;
use crate::callable::{CallableName, CallablePath};
use crate::types::TypeKind;
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
    assert_eq!(function.signature.return_type(), &TypeKind::DisplayText);
}

#[test]
fn standard_callable_inventory_is_typed_before_publication() {
    let environment = TypeCheckEnv::standard();
    let functions = environment.standard_functions();
    let unique_paths = functions
        .iter()
        .map(|function| &function.path)
        .collect::<BTreeSet<_>>();
    assert_eq!(unique_paths.len(), functions.len());
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
        .find(|(ty, _)| ty == &TypeKind::Named("CaptureFormat".to_owned()))
        .map(|(_, variants)| variants)
        .expect("CaptureFormat has one closed environment enum inventory");
    assert_eq!(capture_format, &["png", "raw_rgba"].map(str::to_owned));

    let capture_kind = inventories
        .iter()
        .find(|(ty, _)| ty == &TypeKind::Named("CaptureKind".to_owned()))
        .map(|(_, variants)| variants)
        .expect("CaptureKind has one closed environment enum inventory");
    assert_eq!(capture_kind, &["color", "mask"].map(str::to_owned));

    let pointer_button = inventories
        .iter()
        .find(|(ty, _)| ty == &TypeKind::Named("PointerButton".to_owned()))
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
