use super::*;
use crate::semantic_coordinate::{CheckedSemanticPathStep, SemanticCoordinateIndex};
use arcweft_lang_hir::project::HirDeclarationParameterRootRole;

#[test]
fn action_emit_owns_the_original_nominal_payload_and_exact_effect() {
    let world = fixture(
        "pub action @action:.feedback.submit submit(value: i64)\nflow main() -> i64 { let channel = submit; emit(channel(value = 42i64)); return 0 }\n",
        None,
    );
    let report = analyze(&world).expect("typed Action emission is an ordinary checked call");
    let effect = report
        .expressions()
        .filter_map(|(_, expression)| expression.evaluated_effect())
        .find(|effect| {
            matches!(
                effect.operation(),
                CheckedEvaluatedEffectOperation::EmitAction { .. }
            )
        })
        .expect("the expression retains its checked emission");
    let CheckedEvaluatedEffectOperation::EmitAction { payload } = effect.operation() else {
        unreachable!()
    };
    let TypeKind::ProjectNominal(nominal) = payload.ty() else {
        panic!("Action emission retains its exact nominal payload");
    };
    assert_eq!(
        nominal
            .declaration_identity()
            .action()
            .map(arcweft_id::PublicId::as_str),
        Some("action.feedback.submit"),
    );
    assert_eq!(
        effect.disposition(),
        crate::callable::CallableEvaluatedEffect::EmitAction
    );
    assert_eq!(effect.result(), &TypeKind::Unit);
}

#[test]
fn action_emit_rejects_a_non_action_value_at_its_exact_operand_source() {
    let source = "flow main() -> i64 { emit(42i64); return 0 }\n";
    let world = fixture(source, None);
    let error = analyze(&world).expect_err("ordinary values cannot impersonate Action payloads");
    let FinalSemanticAnalysisError::ActionPayloadRequired {
        actual,
        source_span,
        ..
    } = &error
    else {
        panic!("emission retains a source-backed payload rejection: {error:?}");
    };
    assert_eq!(actual.as_ref(), &TypeKind::I64);
    let start = source.find("42i64").unwrap();
    assert_eq!(
        source_span.range(),
        arcweft_source::SourceRange::new(start, start + 5)
    );
    assert_eq!(error.diagnostic_code(), "sema.action.payload_type");
    assert!(error.source_diagnostic().is_some());
}

#[test]
fn action_receive_returns_the_original_payload_in_a_typed_need() {
    let world = fixture(
        "pub action @action:.feedback.submit submit(value: i64)\nfn next() -> Need<submit> { receive(submit) }\nflow main() -> i64 { let pending = receive(submit); return 0 }\n",
        None,
    );
    let report =
        analyze(&world).expect("receive binds its payload through the ordinary call schema");
    let payloads = report
        .expressions()
        .filter_map(|(_, expression)| {
            let Some(TypeKind::Need(payload)) = expression.value_type() else {
                return None;
            };
            let TypeKind::ProjectNominal(nominal) = payload.as_ref() else {
                return None;
            };
            Some(nominal)
        })
        .collect::<Vec<_>>();
    assert!(!payloads.is_empty());
    for payload in payloads {
        assert_eq!(
            payload
                .declaration_identity()
                .action()
                .map(arcweft_id::PublicId::as_str),
            Some("action.feedback.submit"),
        );
    }
}

#[test]
fn qualified_action_shadowing_reports_the_exact_unknown_member() {
    let source = "fn invalid(value: i64) -> Unit { let ignored = value.submit; () }\n";
    let world = fixture(source, None);
    let error = analyze(&world).expect_err("a lexical value owns its member lookup");
    let FinalSemanticAnalysisError::UnknownValueMember {
        receiver,
        member,
        member_source,
        ..
    } = &error
    else {
        panic!("a source rejection retains the selected member: {error:?}")
    };
    assert_eq!(receiver.as_ref(), &TypeKind::I64);
    assert_eq!(member.as_str(), "submit");
    let start = source.find("submit").unwrap();
    assert_eq!(
        member_source.range(),
        arcweft_source::SourceRange::new(start, start + 6)
    );
    assert_eq!(error.diagnostic_code(), "sema.value.unknown_member");
    assert!(error.source_diagnostic().is_some());
}

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
