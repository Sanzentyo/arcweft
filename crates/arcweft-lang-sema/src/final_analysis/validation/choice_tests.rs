use super::*;
use crate::final_analysis::tests::{analyze, fixture};

const SOURCE: &str = r#"
flow main {
    scope menu {
        choice @.pick {
            @.first "First" -> @flow.first
            @.second "Second" -> @flow.second
        }
    }
}
flow first() -> String { return "first" }
flow second() -> String { return "second" }
"#;

#[test]
fn choice_seal_binds_header_options_and_complete_gotos_to_the_authored_owner() {
    let world = fixture(SOURCE, None);
    let analysis = analyze(&world).unwrap();
    let (owner, choice) = analysis
        .expressions()
        .find_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::Choice(choice) => Some((owner, choice)),
            _ => None,
        })
        .unwrap();
    let project = world.project.analysis_view().unwrap();
    let modules = project
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    validate_choice(&world.symbols, &modules, owner, choice).unwrap();
    let unrelated = arcweft_id::PublicId::try_new("choice.unrelated").unwrap();
    let header = CheckedChoice::new(
        Some(unrelated.clone()),
        choice.option_ids(),
        choice.gotos(),
        choice.plan().cloned(),
    );
    let mut options = choice.option_ids().to_vec();
    options.swap(0, 1);
    let swapped_options = CheckedChoice::new(
        choice.public_id().cloned(),
        options,
        choice.gotos(),
        choice.plan().cloned(),
    );
    let omitted_goto = CheckedChoice::new(
        choice.public_id().cloned(),
        choice.option_ids(),
        &choice.gotos()[..1],
        choice.plan().cloned(),
    );
    let changed_target = CheckedChoice::new(
        choice.public_id().cloned(),
        choice.option_ids(),
        vec![
            super::super::CheckedChoiceGoto::new(0, choice.gotos()[1].target().clone()),
            choice.gotos()[1].clone(),
        ],
        choice.plan().cloned(),
    );
    for invalid in [header, swapped_options, omitted_goto, changed_target] {
        assert_eq!(
            validate_choice(&world.symbols, &modules, owner, &invalid),
            Err(FinalSemanticAnalysisError::WrongPayloadFamily)
        );
    }
}

#[test]
fn choice_seal_rejects_foreign_same_source_flow_target_generation() {
    let world = fixture(SOURCE, None);
    let foreign = fixture(SOURCE, None);
    let analysis = analyze(&world).unwrap();
    let foreign_analysis = analyze(&foreign).unwrap();
    let (owner, choice) = analysis
        .expressions()
        .find_map(|(owner, checked)| match checked.resolution() {
            CheckedExpressionResolution::Choice(choice) => Some((owner, choice)),
            _ => None,
        })
        .unwrap();
    let foreign_choice = foreign_analysis
        .expressions()
        .find_map(|(_, checked)| match checked.resolution() {
            CheckedExpressionResolution::Choice(choice) => Some(choice),
            _ => None,
        })
        .unwrap();
    let project = world.project.analysis_view().unwrap();
    let modules = project
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    validate_choice(&world.symbols, &modules, owner, choice).unwrap();
    assert_ne!(
        choice.gotos(),
        foreign_choice.gotos(),
        "equal readable ids do not substitute the item generation"
    );
    let forged = CheckedChoice::new(
        choice.public_id().cloned(),
        choice.option_ids(),
        foreign_choice.gotos(),
        choice.plan().cloned(),
    );
    assert_eq!(
        validate_choice(&world.symbols, &modules, owner, &forged),
        Err(FinalSemanticAnalysisError::WrongPayloadFamily)
    );
}

#[test]
fn choice_admission_refuses_duplicate_options_and_wrong_target_family() {
    for source in [
        "flow main { choice @choice.menu {\n @choice.same \"A\" -> @flow.done\n @choice.same \"B\" -> @flow.done\n } }\nflow done() -> String { return \"done\" }\n",
        "signal ready: Watch<bool>\nflow main { choice @choice.menu {\n @choice.first \"A\" -> @signal.ready\n } }\n",
    ] {
        let world = fixture(source, None);
        assert!(matches!(
            analyze(&world),
            Err(FinalSemanticAnalysisError::WrongPayloadFamily)
        ));
    }
}
