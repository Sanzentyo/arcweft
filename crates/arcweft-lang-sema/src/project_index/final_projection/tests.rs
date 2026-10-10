use super::*;
use crate::final_analysis::tests::{analyze, fixture};

const SOURCE: &str = r#"
flow opening {
    scope menu {
        choice @.pick {
            @.first "First" -> @flow.done
            @.second "Second" -> @flow.child.done
        }
    }
}
flow done() -> String { return "root" }
"#;
const CHILD: &str = "pub flow done() -> String { return \"child\" }\n";

fn public(id: &str) -> ProjectEntityId {
    ProjectEntityId::public(PublicId::try_new(id).unwrap())
}

#[test]
fn choice_index_source_rejects_a_private_foreign_flow_target() {
    let world = fixture(SOURCE, Some("flow done() -> String { return \"child\" }\n"));
    let Err(FinalSemanticAnalysisError::ValueResolutionFailed { owner }) = analyze(&world) else {
        panic!("private foreign Flow must fail checked target resolution")
    };
    let module = world.project.analysis_view().unwrap();
    let module = module
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))
        .unwrap();
    assert!(matches!(
        module.resolve_expr(owner).unwrap().kind(),
        arcweft_lang_hir::expr::HirExprKind::Choice(_)
    ));
}

#[test]
fn checked_choice_index_retains_symbols_relations_and_exact_source_provenance() {
    use arcweft_lang_hir::source_index::HirChoiceCompactArmSourcePart as Part;
    let world = fixture(SOURCE, Some(CHILD));
    let analysis = analyze(&world).unwrap();
    let project = world.project.analysis_view().unwrap();
    let index = ProjectSemanticIndex::try_from_final_project(
        ProgramHash::new("choice-source-index"),
        project,
        &world.symbols,
        &analysis,
    )
    .unwrap();
    let (owner, choice) = analysis
        .expressions()
        .find_map(|(owner, checked)| match checked.resolution() {
            CheckedExpressionResolution::Choice(choice) => Some((owner, choice)),
            _ => None,
        })
        .unwrap();
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))
        .unwrap();
    let choice_id = public("choice.opening.menu.pick");
    let first_id = public("choice.opening.menu.pick.first");
    let second_id = public("choice.opening.menu.pick.second");
    let root = index
        .entities()
        .keys()
        .find(|id| id.public_id().as_str() == "flow.opening")
        .unwrap();
    assert_eq!(index.entities().len(), 6);
    assert_eq!(
        index.entity(&choice_id).unwrap().ty().kind(),
        &EntityKind::Choice
    );
    for id in [&first_id, &second_id] {
        assert_eq!(
            index.entity(id).unwrap().ty().kind(),
            &EntityKind::ChoiceOption
        );
    }
    assert_eq!(index.relations().len(), 5);
    assert!(index.relations().contains(&ProjectGraphRelation::new(
        root.clone(),
        choice_id.clone(),
        ProjectGraphRelationKind::ContainsChoice,
    )));
    for (ordinal, id) in [&first_id, &second_id].into_iter().enumerate() {
        assert!(index.relations().contains(&ProjectGraphRelation::new(
            choice_id.clone(),
            id.clone(),
            ProjectGraphRelationKind::ContainsChoiceOption,
        )));
        let (CallableDeclarationKey::Flow(flow), _) =
            choice.gotos()[ordinal].target().flow_owner().unwrap()
        else {
            panic!("exact structural Flow owner");
        };
        assert!(index.relations().contains(&ProjectGraphRelation::new(
            id.clone(),
            ProjectEntityId::structural_flow(flow.clone()),
            ProjectGraphRelationKind::ChoiceOptionGoto,
        )));
        let site = module
            .source_site(
                module.provenance().source_identity(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::ChoiceCompactArm {
                        arm: u32::try_from(ordinal).unwrap(),
                        part: Part::Whole,
                    },
                },
            )
            .unwrap();
        let HirSourcePresence::Present(HirSourceSite::Span(span)) = site.presence() else {
            panic!("authored option span")
        };
        assert_eq!(index.entity(id).unwrap().source().span(), span);
        let site = module
            .source_site(
                module.provenance().source_identity(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::ChoiceCompactArm {
                        arm: u32::try_from(ordinal).unwrap(),
                        part: Part::GotoTarget,
                    },
                },
            )
            .unwrap();
        let HirSourcePresence::Present(HirSourceSite::Span(span)) = site.presence() else {
            panic!("authored target span")
        };
        assert_eq!(
            &SOURCE[span.range().as_range()],
            if ordinal == 0 {
                "@flow.done"
            } else {
                "@flow.child.done"
            }
        );
    }
    assert_ne!(
        choice.gotos()[0].target().flow_owner().unwrap().0,
        choice.gotos()[1].target().flow_owner().unwrap().0
    );
    let site = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Whole,
            },
        )
        .unwrap();
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = site.presence() else {
        panic!("authored Choice span")
    };
    assert_eq!(index.entity(&choice_id).unwrap().source().span(), span);
    assert!(
        index
            .entities()
            .values()
            .all(|entity| !entity.semantic_hash().as_str().is_empty())
    );
}

#[test]
fn anonymous_choice_indexes_absolute_options_without_an_invented_choice_identity() {
    let world = fixture(
        "flow opening { choice {\n @choice.absolute \"Only\" -> @flow.done\n } }\nflow done() -> String { return \"done\" }\n",
        None,
    );
    let analysis = analyze(&world).unwrap();
    let index = ProjectSemanticIndex::try_from_final_project(
        ProgramHash::new("anonymous-choice"),
        world.project.analysis_view().unwrap(),
        &world.symbols,
        &analysis,
    )
    .unwrap();
    assert_eq!(index.entities().len(), 3);
    assert_eq!(
        index
            .entity(&public("choice.absolute"))
            .unwrap()
            .ty()
            .kind(),
        &EntityKind::ChoiceOption
    );
    assert!(
        index
            .entities()
            .values()
            .all(|entity| entity.ty().kind() != &EntityKind::Choice)
    );
    assert_eq!(index.relations().len(), 1);
    assert_eq!(
        index.relations()[0].edge_kind(),
        ProjectGraphRelationKind::ChoiceOptionGoto
    );
}

#[test]
fn choice_index_keeps_out_arms_and_nonzero_goto_ordinals_distinct() {
    let source = r#"
flow opening() -> i64 {
    return choice @choice.menu {
        @choice.answer "Answer" => 42i64
        @choice.next "Next" -> @flow.done
    }
}
flow done() -> String { return "done" }
"#;
    let world = fixture(source, None);
    let analysis = analyze(&world).unwrap();
    let index = ProjectSemanticIndex::try_from_final_project(
        ProgramHash::new("mixed-choice"),
        world.project.analysis_view().unwrap(),
        &world.symbols,
        &analysis,
    )
    .unwrap();
    let choice = analysis
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Choice(choice) => Some((expression, choice)),
            _ => None,
        })
        .unwrap();
    assert_eq!(choice.0.value_type(), Some(&TypeKind::I64));
    assert_eq!(choice.1.option_ids().len(), 2);
    assert_eq!(choice.1.gotos().len(), 1);
    assert_eq!(choice.1.gotos()[0].arm(), 1);
    assert_eq!(index.entities().len(), 5);
    assert_eq!(index.relations().len(), 4);
    for id in [public("choice.answer"), public("choice.next")] {
        assert_eq!(
            index.entity(&id).unwrap().ty().kind(),
            &EntityKind::ChoiceOption
        );
        assert!(index.relations().contains(&ProjectGraphRelation::new(
            public("choice.menu"),
            id,
            ProjectGraphRelationKind::ContainsChoiceOption,
        )));
    }
    let gotos = index
        .relations()
        .iter()
        .filter(|relation| relation.edge_kind() == ProjectGraphRelationKind::ChoiceOptionGoto)
        .collect::<Vec<_>>();
    let [goto] = gotos.as_slice() else {
        panic!("one exact static target relation")
    };
    assert_eq!(goto.from(), &public("choice.next"));
    let (CallableDeclarationKey::Flow(flow), _) =
        choice.1.gotos()[0].target().flow_owner().unwrap()
    else {
        panic!("original Flow owner")
    };
    assert_eq!(goto.to(), &ProjectEntityId::structural_flow(flow.clone()));
}

#[test]
fn choice_index_refuses_foreign_generation_and_global_option_identity_collision() {
    let world = fixture(SOURCE, Some(CHILD));
    let foreign = fixture(SOURCE, Some(CHILD));
    let foreign_analysis = analyze(&foreign).unwrap();
    assert!(matches!(
        ProjectSemanticIndex::try_from_final_project(
            ProgramHash::new("foreign-choice"), world.project.analysis_view().unwrap(), &world.symbols, &foreign_analysis,
        ),
        Err(ProjectSemanticIndexError::FinalAnalysis(error))
            if *error == FinalSemanticAnalysisError::GenerationMismatch
    ));
    let source = "flow first { choice @choice.first {\n @choice.shared \"A\" -> @flow.done\n } }\nflow second { choice @choice.second {\n @choice.shared \"B\" -> @flow.done\n } }\nflow done() -> String { return \"done\" }\n";
    let collision = fixture(source, None);
    let analysis = analyze(&collision).unwrap();
    assert_eq!(
        ProjectSemanticIndex::try_from_final_project(
            ProgramHash::new("choice-collision"),
            collision.project.analysis_view().unwrap(),
            &collision.symbols,
            &analysis,
        ),
        Err(ProjectSemanticIndexError::DuplicateEntity {
            id: public("choice.shared")
        })
    );
}
