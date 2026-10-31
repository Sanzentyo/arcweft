use super::*;
use arcweft_interaction_model::presentation::PresentationResourceKind;

#[test]
fn presentation_handle_projection_retains_exact_resource_and_affine_snapshot_owner() {
    let compiled = crate::source::compile_source(
        "entry cli @entry.main { goto @flow.main }\nflow main() -> i64 { return 0i64 }",
    )
    .expect("projection fixture");
    let lease = &compiled.analysis;
    for resource in PresentationResourceKind::ALL {
        let ty = TypeKind::presentation_handle(TypeKind::PresentationResource(resource));
        let projected = runtime_type(
            &ty,
            lease.project_symbols(),
            lease.registered_world(),
            lease.final_analysis().as_ref(),
        )
        .expect("closed presentation handle carrier");
        let expected = arcweft_core::pattern::RuntimeOpaqueTypeOwner::presentation_handle(
            resource,
            arcweft_core::pattern::RuntimeSemanticTypeId::from(
                ty.semantic_identity_digest().unwrap(),
            ),
        );
        assert_eq!(
            projected.checked_type().expect("exact runtime predicate"),
            arcweft_core::pattern::RuntimeCheckedType::Opaque { owner: expected }
        );
    }
}

#[test]
fn presentation_handle_source_function_keeps_the_owned_parameter_and_return_type() {
    for resource in PresentationResourceKind::ALL {
        for (prefix, constructor, item, body) in [
            (
                String::new(),
                "Handle",
                resource.type_name(),
                "handle".to_owned(),
            ),
            (
                "fn generic<T>(value: Handle<T>) -> Handle<T> { value }\n".to_owned(),
                "Handle",
                resource.type_name(),
                "generic(handle)".to_owned(),
            ),
            (
                "fn generic<T>(value: Handle<T>) -> Handle<T> { value }\n".to_owned(),
                "Handle",
                resource.type_name(),
                format!("generic::<{}>(handle)", resource.type_name()),
            ),
            (
                format!("pub type SurfaceType = {}\n", resource.type_name()),
                "Handle",
                "SurfaceType",
                "handle".to_owned(),
            ),
            (
                "pub type Lease<T> = Handle<T>\n".to_owned(),
                "Lease",
                resource.type_name(),
                "handle".to_owned(),
            ),
            (
                format!(
                    "pub type SurfaceType = {}\nfn generic<T>(value: Handle<T>) -> Handle<T> {{ value }}\n",
                    resource.type_name()
                ),
                "Handle",
                resource.type_name(),
                "generic::<SurfaceType>(handle)".to_owned(),
            ),
            (
                "fn generic<T>(value: T) -> T { value }\n".to_owned(),
                "Handle",
                resource.type_name(),
                format!("generic::<Handle<{}>>(handle)", resource.type_name()),
            ),
        ] {
            let compiled = crate::source::compile_source(&format!(
            "entry cli @entry.main {{ goto @flow.main }}\n{prefix}pub fn retain(handle: {constructor}<{item}>) -> {constructor}<{item}> {{ {body} }}\nflow main() -> i64 {{ let keep = retain; return 0i64 }}",
        ))
        .unwrap_or_else(|error| panic!("{resource:?}/{prefix}: {error:?}"));
            let ty = TypeKind::presentation_handle(TypeKind::PresentationResource(resource));
            let semantic = arcweft_core::pattern::RuntimeSemanticTypeId::from(
                ty.semantic_identity_digest().unwrap(),
            );
            let handle = compiled
                .plan
                .type_table()
                .id_for_semantic(semantic)
                .expect("source handle type is emitted into the admitted Core table");
            let function = compiled
                .plan
                .function_sites()
                .iter_with_ids()
                .find_map(|(_, function)| (function.result() == handle).then_some(function))
                .expect("the source identity function retains its exact handle result ABI");
            assert_eq!(function.parameter_inputs().count(), 1);
        }
    }
}
