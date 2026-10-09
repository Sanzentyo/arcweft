use super::*;

use crate::nominal::{ResolvedTypeNodePurpose, TypeNameResolution};

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
