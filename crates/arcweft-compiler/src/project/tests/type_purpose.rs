use super::*;

use arcweft_lang_sema::nominal::ResolvedTypeNodePurpose;
use arcweft_lang_sema::types::{EntityKind, TypeKind};
use arcweft_runtime_plan::semantic_facts::{
    RuntimeProjectFunctionFactError, RuntimeProjectFunctionInstanceSemanticFacts,
    RuntimeProjectFunctionTypeOwner, RuntimeProjectFunctionTypeProjection, RuntimeTypeShape,
};

#[test]
fn ref_flow_signature_retains_every_type_owner_and_rejects_wrong_purpose_projection() {
    assert_entity_family_type_partition(
        "entry cli @entry.main { goto @flow.main }\nfn choose(flag: bool) -> Ref<Flow> { match flag { true => @flow.main, false => @flow.other } }\nflow other() -> String { return \"other\" }\nflow main() -> String { return choose(true).id }",
        EntityKind::Flow,
    );
}

#[test]
fn ref_asset_signature_retains_every_type_owner_and_rejects_wrong_purpose_projection() {
    assert_entity_family_type_partition(
        "entry cli @entry.main { goto @flow.main }\nfn choose(flag: bool) -> Ref<Asset> { match flag { true => @asset:.bg.pulse, false => @asset:.bg.room } }\nflow main() -> String { return choose(true).id }",
        EntityKind::Asset,
    );
}

fn assert_entity_family_type_partition(source: &str, family: EntityKind) {
    let (project, context) = removed_role_project(source);
    let (mut session, sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &sources, &context)
        .expect("ordinary callable entity-reference ABI closes without fabricating a type");
    let analysis = compiled.analysis_lease().final_analysis();
    let executable = compiled
        .analysis_lease()
        .hir_project()
        .analysis_view()
        .unwrap();
    let (_, module) = executable.modules().next().unwrap();
    let owner = module
        .items()
        .find_map(|(owner, item)| match item.kind() {
            HirItemKind::Function(function)
                if function.name().resolved().map(|name| name.as_str()) == Some("choose") =>
            {
                Some(owner)
            }
            _ => None,
        })
        .unwrap();
    let instances = compiled
        .runtime_facts()
        .project_function_instances()
        .filter(|instance| instance.callable().owner() == owner)
        .collect::<Vec<_>>();
    let [instance] = instances.as_slice() else {
        panic!("one closed ordinary choose instance")
    };
    let semantics = instance.semantics();
    let partition = semantics.partition();
    let reachability = crate::lower::project_runtime_reachability(
        executable,
        compiled.analysis_lease().project_symbols(),
        analysis,
        analysis.checked_entries(),
        crate::lower::RuntimeEmissionMode::CheckAll,
    )
    .unwrap();
    let expected = reachability
        .executable_owners(partition.executable())
        .unwrap();
    assert_eq!(
        partition
            .types()
            .iter()
            .map(|row| row.owner())
            .collect::<Vec<_>>(),
        expected.types().collect::<Vec<_>>(),
        "semantic-only arguments remain in the complete HIR executable inventory",
    );
    for row in partition.types() {
        assert_eq!(
            row.purpose(),
            &analysis
                .type_resolution_node(row.owner())
                .unwrap()
                .purpose()
        );
    }
    let marker = partition
        .types()
        .iter()
        .find(|row| row.purpose() == &ResolvedTypeNodePurpose::EntityFamilyArgument(family.clone()))
        .expect("the Ref constructor retains its exact admitted family argument");
    let reference = partition
        .types()
        .iter()
        .find(|row| analysis.ty(row.owner()) == Some(&TypeKind::entity_ref(family.clone())))
        .expect("the Ref constructor is a runtime value type");
    assert!(reference.has_runtime_type());
    assert!(!marker.has_runtime_type());
    assert!(matches!(
        semantics
            .ty(RuntimeProjectFunctionTypeOwner::Type(reference.owner()))
            .unwrap()
            .shape(),
        RuntimeTypeShape::EntityReference,
    ));
    assert_eq!(
        semantics.ty(RuntimeProjectFunctionTypeOwner::Type(marker.owner())),
        None
    );
    assert_eq!(analysis.ty(marker.owner()), None);
    assert_strict_type_projection(semantics, family, marker.owner(), reference.owner());
}

fn assert_strict_type_projection(
    semantics: &RuntimeProjectFunctionInstanceSemanticFacts,
    family: EntityKind,
    marker: arcweft_lang_hir::identity::TypeId,
    reference: arcweft_lang_hir::identity::TypeId,
) {
    let partition = semantics.partition();
    let marker_owner = RuntimeProjectFunctionTypeOwner::Type(marker);
    let reference_owner = RuntimeProjectFunctionTypeOwner::Type(reference);
    let marker_index = semantics
        .type_projection()
        .iter()
        .position(|row| row.owner() == marker_owner)
        .unwrap();
    let reference_index = semantics
        .type_projection()
        .iter()
        .position(|row| row.owner() == reference_owner)
        .unwrap();
    assert!(matches!(
        &semantics.type_projection()[marker_index],
        RuntimeProjectFunctionTypeProjection::SemanticOnlyType {
            purpose, ..
        } if purpose == &ResolvedTypeNodePurpose::EntityFamilyArgument(family.clone())
    ));
    let rebuild = |projection: Box<[RuntimeProjectFunctionTypeProjection]>| {
        RuntimeProjectFunctionInstanceSemanticFacts::try_new(
            partition.clone(),
            semantics.local_uses().clone(),
            projection,
            semantics.expressions().into(),
            semantics.patterns().into(),
            semantics.statements().into(),
            semantics.captures().into(),
        )
    };
    let assert_incomplete = |projection| {
        assert!(matches!(
            rebuild(projection),
            Err(RuntimeProjectFunctionFactError::IncompleteTypeProjection)
        ));
    };
    let value_type = semantics.ty(reference_owner).unwrap().clone();
    let mut forged = semantics.type_projection().to_vec();
    forged[marker_index] = RuntimeProjectFunctionTypeProjection::value(marker_owner, value_type);
    assert_incomplete(forged.into_boxed_slice());
    for purpose in [
        ResolvedTypeNodePurpose::EntityFamilyArgument(EntityKind::Character),
        ResolvedTypeNodePurpose::ConstantArgument,
        ResolvedTypeNodePurpose::ValueType,
    ] {
        let mut forged = semantics.type_projection().to_vec();
        forged[marker_index] =
            RuntimeProjectFunctionTypeProjection::semantic_only_type(marker, purpose);
        assert_incomplete(forged.into_boxed_slice());
    }
    let mut missing = semantics.type_projection().to_vec();
    missing.remove(marker_index);
    assert_incomplete(missing.into_boxed_slice());
    let mut forged = semantics.type_projection().to_vec();
    forged[reference_index] = RuntimeProjectFunctionTypeProjection::semantic_only_type(
        reference,
        ResolvedTypeNodePurpose::EntityFamilyArgument(family.clone()),
    );
    assert_incomplete(forged.into_boxed_slice());
}
