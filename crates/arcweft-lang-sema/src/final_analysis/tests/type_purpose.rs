use super::*;

use crate::nominal::{ResolvedTypeNodePurpose, TypeNameResolution};
use arcweft_interaction_model::presentation::PresentationResourceKind;

#[test]
fn explicit_call_type_arguments_keep_checked_types_without_runtime_value_rows() {
    for (name, expected) in [
        ("i64", TypeKind::I64),
        (
            "ImageSurface",
            TypeKind::PresentationResource(PresentationResourceKind::Image),
        ),
        (
            "Handle<ImageSurface>",
            TypeKind::presentation_handle(TypeKind::PresentationResource(
                PresentationResourceKind::Image,
            )),
        ),
    ] {
        let world = fixture(
            &format!(
                "fn identity<T>(value: T) -> T {{ value }}\nfn apply(value: {name}) -> {name} {{ identity::<{name}>(value) }}\n"
            ),
            None,
        );
        let analysis =
            analyze(&world).expect("explicit type arguments retain their semantic binding");
        let module = world
            .project
            .analysis_view()
            .unwrap()
            .module(&CanonicalModulePath::crate_root())
            .unwrap();
        let calls = module
            .expressions()
            .filter_map(|(owner, expression)| match expression.kind() {
                HirExprKind::Call(call) => Some((owner, call)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(owner, call)] = calls.as_slice() else {
            panic!("one explicit generic call")
        };
        let [arcweft_lang_hir::expr::HirCallTypeArgument::Resolved { ty }] =
            call.explicit_type_application().arguments()
        else {
            panic!("one checked explicit type argument")
        };
        let node = analysis.type_resolution_node(*ty).unwrap();
        assert_eq!(node.purpose(), ResolvedTypeNodePurpose::TypeArgument);
        assert_eq!(node.recovered(), Some(&expected));
        assert_eq!(analysis.ty(*ty), None);
        assert_eq!(
            analysis
                .call(*owner)
                .unwrap()
                .selected_application()
                .unwrap()
                .result()
                .value_type(),
            Some(&expected)
        );
    }
}

#[test]
fn generic_constructor_arguments_retain_semantic_types_and_specific_nonvalue_roles() {
    for ty in [
        "Vec<i64>",
        "Array<i64, 3>",
        "OrderedMap<String, i64>",
        "Option<bool>",
        "Result<i64, String>",
        "Shared<String>",
        "Need<i64>",
        "Lease<ImageSurface>",
    ] {
        let world = fixture(
            &format!(
                "pub type Lease<T> = Handle<T>\nfn identity(value: {ty}) -> {ty} {{ value }}\n"
            ),
            None,
        );
        let analysis = analyze(&world).unwrap_or_else(|error| panic!("{ty}: {error:?}"));
        let module = world
            .project
            .analysis_view()
            .unwrap()
            .module(&CanonicalModulePath::crate_root())
            .unwrap();
        let mut count = 0;
        for (_, root) in module.types() {
            let HirTypeKind::Generic(generic) = root.kind() else {
                continue;
            };
            for argument in generic.arguments() {
                let node = analysis
                    .type_resolution_node(*argument)
                    .expect("exact generic argument node");
                assert!(matches!(
                    node.purpose(),
                    ResolvedTypeNodePurpose::TypeArgument
                        | ResolvedTypeNodePurpose::ConstantArgument
                ));
                assert_eq!(analysis.ty(*argument), None);
                if node.purpose() == ResolvedTypeNodePurpose::TypeArgument {
                    assert!(node.recovered().is_some());
                }
                count += 1;
            }
        }
        assert!(count > 0, "{ty}: checked generic arguments");
    }
}

#[test]
fn presentation_resource_arguments_preserve_direct_and_alias_resolution_without_runtime_rows() {
    for resource in PresentationResourceKind::ALL {
        for constructor in ["Handle", "SlotRef"] {
            for aliased in [false, true] {
                let name = resource.type_name();
                let (prefix, argument) = if aliased {
                    (format!("pub type SurfaceType = {name}\n"), "SurfaceType")
                } else {
                    (String::new(), name)
                };
                let source = format!(
                    "{prefix}fn identity(value: {constructor}<{argument}>) -> {constructor}<{argument}> {{ value }}\n"
                );
                let world = fixture(&source, None);
                let analysis = analyze(&world).expect("typed presentation resource arguments");
                let module = world
                    .project
                    .analysis_view()
                    .unwrap()
                    .module(&CanonicalModulePath::crate_root())
                    .unwrap();
                let arguments = module
                    .types()
                    .filter_map(|(_, ty)| match ty.kind() {
                        HirTypeKind::Generic(generic) => Some(generic.arguments()[0]),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(arguments.len(), 2);
                for argument in arguments {
                    let node = analysis.type_resolution_node(argument).unwrap();
                    assert_eq!(node.purpose(), ResolvedTypeNodePurpose::TypeArgument);
                    assert_eq!(
                        node.recovered(),
                        Some(&TypeKind::PresentationResource(resource))
                    );
                    assert_eq!(analysis.ty(argument), None);
                    assert_eq!(
                        matches!(node.outcome(), TypeNameResolution::Alias(_)),
                        aliased
                    );
                }
            }
        }
    }
}

#[test]
fn generic_presentation_resource_arguments_keep_their_semantic_binding() {
    let world = fixture(
        "fn identity<T>(value: Handle<T>) -> Handle<T> { value }\n",
        None,
    );
    let analysis = analyze(&world).expect("generic handle argument role");
    let arguments = analysis
        .type_resolutions()
        .flat_map(|(_, report)| report.outcome().product().nodes())
        .filter(|node| node.purpose() == ResolvedTypeNodePurpose::TypeArgument)
        .collect::<Vec<_>>();
    assert_eq!(arguments.len(), 2);
    for node in arguments {
        assert!(matches!(node.recovered(), Some(TypeKind::GenericParam(_))));
        assert!(matches!(node.outcome(), TypeNameResolution::Generic(_)));
        assert_eq!(analysis.ty(node.node()), None);
    }
}

#[test]
fn standalone_presentation_resource_types_remain_value_type_nodes() {
    let world = fixture(
        "fn identity(value: ImageSurface) -> ImageSurface { value }\n",
        None,
    );
    let analysis = analyze(&world).expect("resource body value types retain their own role");
    for (_, report) in analysis.type_resolutions() {
        for node in report.outcome().product().nodes() {
            assert_eq!(node.purpose(), ResolvedTypeNodePurpose::ValueType);
            assert_eq!(
                analysis.ty(node.node()),
                Some(&TypeKind::PresentationResource(
                    PresentationResourceKind::Image
                ))
            );
        }
    }
}

#[test]
fn admitted_entity_family_arguments_keep_exact_semantic_purpose() {
    for family in EntityKind::AUTHORED_FAMILIES {
        let name = family
            .authored_type_name()
            .expect("registered authored family");
        let source = format!("fn identity(value: Ref<{name}>) -> Ref<{name}> {{ value }}\n");
        let world = fixture(&source, None);
        let analysis = analyze(&world).expect("Ref family constructor is admitted");
        let module = world
            .project
            .analysis_view()
            .unwrap()
            .module(&CanonicalModulePath::crate_root())
            .unwrap();
        let reference_roots = module
            .types()
            .filter_map(|(owner, ty)| match ty.kind() {
                HirTypeKind::Generic(generic) => Some((owner, generic.arguments()[0])),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reference_roots.len(), 2);
        for (root, argument) in reference_roots {
            assert_eq!(
                analysis.ty(root),
                Some(&TypeKind::entity_ref(family.clone()))
            );
            let root_node = analysis
                .type_resolution_node(root)
                .expect("accepted root node");
            assert_eq!(root_node.purpose(), ResolvedTypeNodePurpose::ValueType);
            let node = analysis
                .type_resolution_node(argument)
                .expect("accepted argument node");
            assert_eq!(
                node.outcome(),
                &TypeNameResolution::EntityFamily(family.clone())
            );
            assert_eq!(
                node.purpose(),
                ResolvedTypeNodePurpose::EntityFamilyArgument(family.clone())
            );
            assert!(!node.purpose().has_runtime_type());
            assert_eq!(analysis.ty(argument), None);
            assert!(!node.is_contextual_alias_target());
        }
    }
}

#[test]
fn constant_arguments_and_alias_targets_retain_source_owned_node_purpose() {
    let world = fixture(
        "pub type Coordinates = Array<i32, 3>\nfn identity(value: Coordinates) -> Coordinates { value }\n",
        None,
    );
    let analysis = analyze(&world).expect("accepted array alias and ordinary function");
    let arguments = analysis
        .type_resolutions()
        .flat_map(|(_, report)| report.outcome().product().nodes())
        .filter(|node| !node.is_contextual_alias_target())
        .filter(|node| node.purpose() == ResolvedTypeNodePurpose::ConstantArgument)
        .map(|node| node.node())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        arguments.len(),
        1,
        "alias uses share the declaration's constant owner"
    );
    let owner = *arguments.first().unwrap();
    let node = analysis
        .type_resolution_node(owner)
        .expect("exact declaration node");
    assert_eq!(node.purpose(), ResolvedTypeNodePurpose::ConstantArgument);
    assert_eq!(node.recovered(), None);
    assert_eq!(analysis.ty(owner), None);
    assert!(!node.is_contextual_alias_target());
    let aliases = analysis
        .type_resolutions()
        .filter(|(_, report)| {
            matches!(
                report
                    .outcome()
                    .product()
                    .nodes()
                    .iter()
                    .find(|node| node.node() == report.outcome().product().root())
                    .map(|node| node.outcome()),
                Some(TypeNameResolution::Alias(_))
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(aliases.len(), 2);
    for (owner, _) in aliases {
        assert_eq!(
            analysis.type_resolution_node(owner).unwrap().purpose(),
            ResolvedTypeNodePurpose::ValueType
        );
        assert!(matches!(analysis.ty(owner), Some(TypeKind::Array { .. })));
    }
}
