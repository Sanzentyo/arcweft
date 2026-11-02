use super::*;
use crate::semantic_coordinate::{CheckedSemanticPathStep, SemanticCoordinateIndex};
use arcweft_lang_hir::project::HirDeclarationParameterRootRole;

#[test]
fn action_payload_is_a_checked_nominal_record_with_persistent_channel_identity() {
    let check = |source: &str, name: &str| {
        let world = fixture(source, None);
        let report =
            analyze(&world).expect("Action payload resolves through the retained type binding");
        let ty = project_nominal_expression_type(&report, name);
        let TypeKind::ProjectNominal(nominal) = &ty else {
            panic!("nominal Action payload");
        };
        let declaration = world.symbols.nominal(nominal.declaration()).unwrap();
        assert_eq!(declaration.type_parameters().len(), 0);
        assert!(matches!(
            declaration.id().kind(),
            arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Action { .. }
        ));
        ty
    };
    let original = check(
        "pub action @action:.feedback.submit submit(value: String, count: i64)\nfn hold(value: submit) -> submit { value }\n",
        "submit",
    );
    let renamed = check(
        "pub action @action:.feedback.submit renamed(value: String, count: i64)\nfn hold(value: renamed) -> renamed { value }\n",
        "renamed",
    );
    let other = check(
        "pub action @action:.feedback.cancel submit(value: String, count: i64)\nfn hold(value: submit) -> submit { value }\n",
        "submit",
    );
    assert_eq!(
        original.semantic_identity_digest().unwrap(),
        renamed.semantic_identity_digest().unwrap()
    );
    assert_ne!(
        original.semantic_identity_digest().unwrap(),
        other.semantic_identity_digest().unwrap()
    );
}

#[test]
fn action_payload_record_fields_use_the_existing_checked_record_projection() {
    let world = fixture(
        "pub action submit(value: String, count: i64)\nflow main() -> i64 { let message: submit = { value = \"hello\", count = 42i64 }; return message.count }\n",
        None,
    );
    let report = analyze(&world).expect("Action payload fields follow the common record machinery");
    let ty = project_nominal_expression_type(&report, "submit");
    assert!(matches!(ty, TypeKind::ProjectNominal(_)));
}

#[test]
fn action_channel_value_retains_its_exact_nominal_payload_type() {
    let world = fixture(
        "pub action submit(value: String)\nflow main() -> i64 { let channel = submit; return 0 }\n",
        None,
    );
    let report = analyze(&world).expect("a first-class Action value retains its payload type");
    let channel = report
        .expressions()
        .find_map(|(_, expression)| match expression.value_type() {
            Some(TypeKind::Ref(entity)) if entity.kind() == &EntityKind::Action => Some(entity),
            _ => None,
        })
        .expect("checked Action channel value");
    let Some(TypeKind::ProjectNominal(payload)) = channel.value() else {
        panic!("nominal Action payload");
    };
    let action = world.symbols.retained_symbols().next().unwrap();
    assert_eq!(Some(payload.declaration()), action.action_payload());
}

#[test]
fn action_payload_uses_the_shared_persistent_field_admission() {
    let world = fixture(
        "pub action submit(handle: CueHandle)\nfn hold(value: submit) -> submit { value }\n",
        None,
    );
    assert!(matches!(analyze(&world),
        Err(FinalSemanticAnalysisError::NominalSchemaProjection(
            super::super::NominalSchemaProjectionError::UnsupportedLeaf { ty, .. })) if *ty == TypeKind::CueHandle));
}

#[test]
fn action_payload_signature_retains_checked_types_and_item_coordinates() {
    let world = fixture(
        "pub action @action:.feedback.submit submit(value: String, count: i64)\nflow main() -> i64 { return 0 }\n",
        None,
    );
    let report = analyze(&world).expect("Action signature checks independently of its use");
    let project = world.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let action = module
        .items()
        .find_map(|(_, item)| match item.kind() {
            HirItemKind::Action(action) => Some(action),
            _ => None,
        })
        .unwrap();
    for (index, parameter) in action.parameters().iter().enumerate() {
        let pattern = report
            .pattern(parameter.pattern())
            .expect("checked action parameter");
        let expected = if index == 0 {
            TypeKind::String
        } else {
            TypeKind::I64
        };
        assert_eq!(pattern.ty(), &expected);
        let path = coordinates
            .pattern(parameter.pattern())
            .expect("item parameter coordinate");
        assert_eq!(
            path.path().steps(),
            &[CheckedSemanticPathStep::ItemParameter(
                HirDeclarationParameterRootRole::Pattern {
                    group: 0,
                    parameter: u32::try_from(index).unwrap()
                }
            )]
        );
        for local in parameter.locals() {
            assert_eq!(
                report
                    .local(*local)
                    .expect("checked action payload binding")
                    .ty(),
                &expected
            );
            let binding = coordinates
                .binding(*local)
                .expect("payload binding coordinate");
            assert_eq!(binding.root(), path.path().root());
        }
    }
}
