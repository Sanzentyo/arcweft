use super::*;
use crate::semantic_facts::RuntimeDialogueApplicationTarget;
use arcweft_core::value::RuntimeCharacterDialogueProducerId;
use arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily;

fn dialogue_type(admission: RuntimeOpaqueTypeAdmission) -> super::super::RuntimeNormalizedType {
    normalized_type(
        0x81,
        RuntimeTypeShape::Opaque {
            producer: RuntimeCharacterDialogueProducerId::get(),
            admission,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: Box::new([]),
        },
    )
}

#[test]
fn dialogue_target_admission_rejects_another_application_target_or_source_type() {
    let project = project_fixture(
        "dialogue-target-owner",
        "fn root() { alice[First]; bob[Second]; }\n",
    );
    let view = project.analysis_view().unwrap();
    let modules = view
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let applications = modules
        .values()
        .flat_map(|module| module.expressions())
        .filter_map(|(owner, expression)| match expression.kind() {
            HirExprKind::AttachedContentApplication(application) => match application.family() {
                HirAttachedContentApplicationFamily::DialogueLine { target, .. } => {
                    Some((owner, *target))
                }
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(first, target), (_, foreign)] = applications.as_slice() else {
        panic!("two authored dialogue application candidates");
    };
    let source_type = normalized_type(0x80, RuntimeTypeShape::EntityReference);
    let fact = |expression| RuntimeDialogueApplicationTarget::CharacterReference {
        expression,
        source_type: source_type.clone(),
        dialogue_type: dialogue_type(RuntimeOpaqueTypeAdmission::ExactIdentity),
    };
    assert!(
        fact(*target)
            .validate(&modules, *first, Some(&source_type))
            .is_ok()
    );
    for (candidate, supplied_type) in [
        (fact(*foreign), Some(&source_type)),
        (fact(*target), None),
        (fact(*target), Some(&unit_type())),
    ] {
        assert!(matches!(
            candidate.validate(&modules, *first, supplied_type),
            Err(RuntimeSemanticFactsError::DialogueTargetMismatch { .. })
        ));
    }
}

#[test]
fn dialogue_target_accepts_exact_and_producer_wide_values_but_not_other_opaque_domains() {
    let project = project_fixture("dialogue-target-types", "fn root() { true }\n");
    let expression = boolean_literal(&project);
    for admission in [
        RuntimeOpaqueTypeAdmission::ExactIdentity,
        RuntimeOpaqueTypeAdmission::ProducerWide,
    ] {
        let accepted = dialogue_type(admission);
        let target = RuntimeDialogueApplicationTarget::CharacterDialogue {
            expression,
            dialogue_type: accepted.clone(),
        };
        assert!(target.has_valid_types());
        let RuntimeTypeShape::Opaque {
            admission,
            value_class,
            persistence,
            ..
        } = accepted.shape()
        else {
            unreachable!()
        };
        let foreign = normalized_type(
            0x82,
            RuntimeTypeShape::Opaque {
                producer: RuntimeOpaqueTypeProducerId::try_new("fixture.other-producer").unwrap(),
                admission: *admission,
                value_class: *value_class,
                persistence: *persistence,
                arguments: Box::new([]),
            },
        );
        assert!(
            !RuntimeDialogueApplicationTarget::CharacterDialogue {
                expression,
                dialogue_type: foreign,
            }
            .has_valid_types()
        );
        assert!(
            !RuntimeDialogueApplicationTarget::CharacterReference {
                expression,
                source_type: unit_type(),
                dialogue_type: accepted,
            }
            .has_valid_types()
        );
    }
}
