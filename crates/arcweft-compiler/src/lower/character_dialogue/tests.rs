use super::*;
use arcweft_interaction_model::dialogue::CharacterDialogueFieldCoordinate as Field;

mod application;
mod fixtures;
mod generation;

fn project_calls(source: &str) -> Vec<(CharacterDialogueOperation, RuntimeResolvedCall)> {
    let compiled = crate::source::compile_source(source)
        .expect("CharacterDialogue calls compile through their typed producer operation");
    let lease = &compiled.analysis;
    let analysis = lease.final_analysis().as_ref();
    analysis
        .expressions()
        .filter_map(|(owner, checked)| match checked.resolution() {
            CheckedExpressionResolution::CharacterDialogueFactory(_)
            | CheckedExpressionResolution::CharacterDialogueReconfigure(_) => Some(owner),
            _ => None,
        })
        .map(|owner| {
            let selected = analysis
                .call(owner)
                .unwrap()
                .selected_application()
                .unwrap();
            let call = runtime_character_dialogue_call(
                owner,
                selected,
                lease.project_symbols(),
                lease.registered_world(),
                analysis,
                None,
            )
            .expect("selected factory/reconfigure projection")
            .expect("the Dialogue branch is retained");
            let RuntimeResolvedCallDispatch::Static(
                RuntimeResolvedStaticCallTarget::CharacterDialogue(dialogue),
            ) = call.dispatch()
            else {
                panic!("CharacterDialogue must not become an ordinary intrinsic");
            };
            (dialogue.operation(), call)
        })
        .collect()
}

#[test]
fn factory_projection_evaluates_callee_before_authored_patch_and_clear() {
    let calls = project_calls(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
fn locale() -> String { "ja-JP" }
flow main() -> i64 {
    let configured = alice(source_locale = locale(), view = None)
    return 42i64
}

"#,
    );
    let [(CharacterDialogueOperation::Factory, call)] = calls.as_slice() else {
        panic!("one selected factory call");
    };
    assert!(call.requires_specialized_operand_anf());
    assert!(matches!(
        call.operands()[0].origin(),
        RuntimeResolvedCallOperandOrigin::Callee
    ));
    assert_eq!(call.operands().len(), 3);
    let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::CharacterDialogue(
        dialogue,
    )) = call.dispatch()
    else {
        unreachable!()
    };
    assert_eq!(
        dialogue.fields(),
        [
            CharacterDialoguePatchField {
                coordinate: Field::SourceLocale,
                operation: CharacterDialoguePatchOperation::Set(1)
            },
            CharacterDialoguePatchField {
                coordinate: Field::View,
                operation: CharacterDialoguePatchOperation::Clear
            },
        ]
    );
    assert_eq!(call.operands()[1].ty().shape(), &RuntimeTypeShape::String);
}

#[test]
fn lowered_factory_materializes_every_operand_before_producing_the_value() {
    use arcweft_core::{plan::FlowOp, value::RuntimeExprKind};

    let compiled = crate::source::compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
flow main() -> i64 {
    let configured = alice(source_locale = "ja-JP", view = None)
    return 42i64
}
"#,
    )
    .expect("factory source-order program compiles");
    let [flow] = compiled.plan.flows() else {
        panic!("one main flow")
    };
    let expression = flow
        .body()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            FlowOp::Let { expr, .. } => Some(expr),
            _ => None,
        })
        .expect("configuration binding is retained");
    let mut body = expression;
    let mut bindings = Vec::new();
    let mut sources = Vec::new();
    while let RuntimeExprKind::Let {
        binding,
        expr,
        body: next,
    } = body.kind()
    {
        bindings.push(*binding);
        sources.push(expr.kind());
        body = next;
    }
    assert_eq!(
        sources.len(),
        3,
        "callee, Set operand and Clear operand are each materialized"
    );
    assert!(matches!(
        sources[0],
        RuntimeExprKind::EntityRef(arcweft_core::value::RuntimeEntityReference::Project {
            family: arcweft_id::DeclarationIdentityFamily::Character,
            ..
        })
    ));
    assert!(
        matches!(sources[1], RuntimeExprKind::Value(RuntimeValue::String(value)) if value == "ja-JP")
    );
    assert!(matches!(
        sources[2],
        RuntimeExprKind::Variant {
            ordinal: 1,
            payload: None
        }
    ));
    let RuntimeExprKind::CharacterDialogue {
        operation,
        target,
        fields,
    } = body.kind()
    else {
        panic!("typed producer operation follows source-order materialization");
    };
    assert_eq!(*operation, CharacterDialogueOperation::Factory);
    assert!(matches!(target.kind(), RuntimeExprKind::Local(local) if *local == bindings[0]));
    let [locale, view] = fields.as_slice() else {
        panic!("both patch contributions remain")
    };
    assert!(
        matches!(&locale.operation, CharacterDialoguePatchOperation::Set(value)
        if matches!(value.kind(), RuntimeExprKind::Local(local) if *local == bindings[1]))
    );
    assert_eq!(view.operation, CharacterDialoguePatchOperation::Clear);
}

#[test]
fn branch_factory_projection_preserves_both_selected_character_targets() {
    let calls = project_calls(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
pub character bob {}
flow main() -> i64 {
    let configured = if true { alice() } else { bob() }
    return 42i64
}
"#,
    );
    assert_eq!(calls.len(), 2);
    let targets = calls
        .iter()
        .map(|(operation, call)| {
            assert_eq!(*operation, CharacterDialogueOperation::Factory);
            assert_eq!(call.operands().len(), 1);
            assert_eq!(
                call.operands()[0].ty().shape(),
                &RuntimeTypeShape::EntityReference
            );
            call.operands()[0].source()
        })
        .collect::<Vec<_>>();
    assert_ne!(targets[0], targets[1]);
}

#[test]
fn factory_projection_accepts_a_character_reference_value_through_a_local() {
    let calls = project_calls(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
flow main() -> i64 {
    let person: Ref<Character> = alice
    let configured = person(source_locale = "ja-JP")
    return 42i64
}
"#,
    );
    let [(CharacterDialogueOperation::Factory, call)] = calls.as_slice() else {
        panic!("one factory selected from a runtime Character reference");
    };
    assert_eq!(call.operands().len(), 2);
    assert_eq!(
        call.operands()[0].ty().shape(),
        &RuntimeTypeShape::EntityReference
    );
}

#[test]
fn reconfigure_projection_keeps_dynamic_character_dialogue_target_type() {
    let calls = project_calls(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
pub character bob {}
flow main() -> i64 {
    let configured = if true { alice() } else { bob() }
    let changed = configured(source_locale = "en-US")
    return 42i64
}
"#,
    );
    let (_, call) = calls
        .iter()
        .find(|(operation, _)| *operation == CharacterDialogueOperation::Reconfigure)
        .expect("dynamic reconfiguration remains selected");
    assert_eq!(
        call.operands()[0].ty().identity(),
        arcweft_dialogue::CharacterDialogueType::new(
            arcweft_dialogue::CharacterDialogueCharacterType::Any
        )
        .runtime_opaque_owner()
        .semantic_identity(),
    );
    assert!(matches!(
        call.operands()[0].origin(),
        RuntimeResolvedCallOperandOrigin::Callee
    ));
}

#[test]
fn factory_projection_preserves_the_accepted_source_voice_enum() {
    let calls = project_calls(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
flow main() -> i64 {
    let configured = alice(voice = auto)
    return 42i64
}
"#,
    );
    let [(CharacterDialogueOperation::Factory, call)] = calls.as_slice() else {
        panic!("one selected factory call");
    };
    let voice = call.operands()[1].ty();
    let RuntimeTypeShape::Nominal { nominal, arguments } = voice.shape() else {
        panic!("source DialogueVoice remains a closed enum");
    };
    let arcweft_runtime_plan::semantic_facts::RuntimeResolvedNominalSource::ClosedVariant { proof } =
        nominal.source()
    else {
        panic!("source voice retains its accepted variant owner");
    };
    assert!(arguments.is_empty());
    assert_eq!(proof.ty(), TypeKind::Named("DialogueVoice".to_owned()));
    let CheckedVariantOwnerKind::BuiltinClosed { nominal: owner, .. } = proof.kind() else {
        panic!("source voice is owned by its environment binding");
    };
    assert_eq!(nominal.runtime_nominal_id().as_str(), owner.as_str());
    assert_eq!(proof.cases()[0].diagnostic_name(), Some("auto"));
    assert_eq!(
        voice.identity().as_bytes(),
        proof.semantic_type().as_bytes()
    );
}

#[test]
fn presentation_lifetime_projects_as_its_closed_environment_enum() {
    let compiled = crate::source::compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> i64 { return 0i64 }
"#,
    )
    .expect("presentation lifetime source compiles");
    let lease = &compiled.analysis;
    let projected = runtime_type(
        &TypeKind::Named("PresentationLifetime".to_owned()),
        lease.project_symbols(),
        lease.registered_world(),
        lease.final_analysis().as_ref(),
    )
    .expect("presentation lifetime retains its checked enum owner");
    let RuntimeTypeShape::Nominal { nominal, arguments } = projected.shape() else {
        panic!("presentation lifetime is a closed enum");
    };
    let arcweft_runtime_plan::semantic_facts::RuntimeResolvedNominalSource::ClosedVariant { proof } =
        nominal.source()
    else {
        panic!("presentation lifetime has a closed variant proof");
    };
    assert!(arguments.is_empty());
    assert_eq!(proof.ty(), TypeKind::Named("PresentationLifetime".to_owned()));
    assert_eq!(proof.cases()[3].diagnostic_name(), Some("line"));
}

#[test]
fn factory_and_reconfigure_project_manifest_owned_look_cases() {
    let compiled = fixtures::compile_with_character_manifest(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
flow main() -> i64 {
    let configured = alice(look = .normal)
    let changed = configured(look = .bright)
    return 42i64
}
"#,
    );
    let expected = TypeKind::character_look(
        arcweft_character::id::CharacterId::try_new("character.alice").unwrap(),
    );
    let expected_identity = expected.semantic_identity_digest().unwrap();
    let calls = compiled
        .runtime_facts()
        .calls()
        .filter_map(|(_, call)| match call.dispatch() {
            RuntimeResolvedCallDispatch::Static(
                RuntimeResolvedStaticCallTarget::CharacterDialogue(dialogue),
            ) => Some((dialogue, call)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for (dialogue, call) in &calls {
        assert_eq!(dialogue.fields()[0].coordinate, Field::Look);
        let look = call.operands()[1].ty();
        assert_eq!(look.identity().as_bytes(), expected_identity.as_bytes());
        let RuntimeTypeShape::Nominal { nominal, arguments } = look.shape() else {
            panic!("Look<Character> is a manifest-owned enum");
        };
        let arcweft_runtime_plan::semantic_facts::RuntimeResolvedNominalSource::ClosedVariant {
            proof,
        } = nominal.source()
        else {
            panic!("Look<Character> retains accepted source authority");
        };
        assert!(arguments.is_empty());
        assert_eq!(proof.ty(), expected);
        assert_eq!(
            nominal.runtime_nominal_id(),
            RuntimeNominalTypeId::from_checked_digest(*expected_identity.as_bytes())
        );
        assert_eq!(
            proof
                .cases()
                .iter()
                .map(|case| case.diagnostic_name().unwrap())
                .collect::<Vec<_>>(),
            ["normal", "bright"]
        );
    }
    assert!(
        calls
            .iter()
            .any(|(dialogue, _)| { dialogue.operation() == CharacterDialogueOperation::Factory })
    );
    assert!(
        calls.iter().any(|(dialogue, _)| {
            dialogue.operation() == CharacterDialogueOperation::Reconfigure
        })
    );
}

#[test]
fn character_look_value_projects_inside_a_closed_function_instance() {
    fixtures::compile_with_character_manifest(
        r#"
entry cli @entry.main { goto @flow.main }
pub character alice {}
fn configure() -> i64 {
    let configured = alice(look = .normal)
    42i64
}
flow main() -> i64 {
    return configure()
}
"#,
    );
}
