use super::*;
use crate::semantic_coordinate::{CheckedSemanticPathStep, SemanticCoordinateIndex};
use arcweft_lang_hir::project::HirDeclarationParameterRootRole;

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
