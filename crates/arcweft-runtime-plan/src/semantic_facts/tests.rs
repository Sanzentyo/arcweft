use std::{collections::BTreeMap, sync::Arc};

use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId,
};
use arcweft_core::plan::{FlowRuntimeId, RuntimeLineId};
use arcweft_core::value::RuntimeValue;
use arcweft_lang_hir::database::HirDatabase;
use arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates;
use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_hir::leaf::HirLiteral;
use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
use arcweft_lang_hir::project::{HirProject, HirProjectBuilder, HirProjectModule};
use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    RuntimeCallResultShape, RuntimeCheckedTypeProjectionError, RuntimePlanSemanticFactInput,
    RuntimePlanSemanticFacts, RuntimeReductionConstructor, RuntimeRegisteredValueId,
    RuntimeResolvedCall, RuntimeResolvedCallArgument, RuntimeResolvedCallTarget,
    RuntimeResolvedValue, RuntimeResolvedVariant, RuntimeSemanticFactFamily,
    RuntimeSemanticFactsError, RuntimeSemanticTypeId, RuntimeTypeProjectionStep, RuntimeTypeShape,
    RuntimeUnsupportedTypeShape,
};

fn project_fixture(label: &str, source: &str) -> HirProject {
    let package = CallablePackageId::try_new(format!("runtime-plan-semantic-facts-{label}"))
        .expect("fixture package");
    let path = CanonicalModulePath::crate_root();
    let source_name = SourceName::path(format!("runtime-plan-semantic-facts-{label}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-test://runtime-plan/{label}"))
                .expect("fixture document ID"),
            source_name.clone(),
            source,
        )
        .expect("fixture document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(source_name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached fixture parse");
    let key = HirModuleKey::new(
        package.clone(),
        path.clone(),
        parsed.document().identity().clone(),
    );
    let mut database = HirDatabase::try_new().expect("HIR database");
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        parsed.document().identity().id().clone(),
        "runtime-plan-semantic-facts-test",
    )
    .expect("fixture symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
        .expect("fixture symbol revision");
    let transaction = database
        .stage_proof_return_project(
            [LoweringRequest::try_new(key, &parsed).expect("lower request")],
            world,
            revision,
            [parsed.document().identity()],
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .expect("final HIR project stages");
    let facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("semantic-facts fixture has no authored Proof return headers");
    let mut outputs = transaction
        .publish_with_semantic_facts(&mut database, facts)
        .expect("final HIR project publishes");
    let module = outputs
        .pop()
        .expect("one semantic-facts fixture module")
        .into_module();
    assert!(outputs.is_empty());
    let project_module = HirProjectModule::try_new(
        &database,
        &package,
        &path,
        parsed.document().identity(),
        module,
    )
    .expect("accepted module lease");
    let mut builder = HirProjectBuilder::new(&database, package);
    builder
        .insert_module(project_module)
        .expect("module insertion");
    builder.finish().expect("fixture project")
}

fn boolean_literal(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .executable_view()
        .expect("clean fixture")
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(id, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::Literal(HirLiteral::Boolean(true))
            )
            .then_some(id)
        })
        .expect("fixture boolean literal")
}

fn flow_item(project: &HirProject) -> arcweft_lang_hir::identity::ItemId {
    project
        .executable_view()
        .expect("clean fixture")
        .items()
        .find(|item| {
            matches!(
                item.item().kind(),
                arcweft_lang_hir::item::HirItemKind::Flow(_)
            )
        })
        .map(arcweft_lang_hir::project::HirProjectItemRef::id)
        .expect("fixture Flow item")
}

fn call_expression(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .executable_view()
        .expect("clean fixture")
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(id, expression)| {
            matches!(expression.kind(), HirExprKind::Call(_)).then_some(id)
        })
        .expect("fixture call expression")
}

fn entity_reference(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .executable_view()
        .expect("clean fixture")
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(id, expression)| {
            matches!(expression.kind(), HirExprKind::EntityReference(_)).then_some(id)
        })
        .expect("fixture entity-reference expression")
}

fn unit_type() -> super::RuntimeNormalizedType {
    super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x11; 32]),
        RuntimeTypeShape::Unit,
    )
}

fn complete_type_input(project: &HirProject) -> RuntimePlanSemanticFactInput {
    let mut input = RuntimePlanSemanticFactInput::new();
    for (_, module) in project
        .executable_view()
        .expect("executable type fixture")
        .modules()
    {
        for (owner, _) in module.expressions() {
            input.push_expression_type(owner, unit_type());
        }
        for (owner, _) in module.patterns() {
            input.push_pattern_type(owner, unit_type());
        }
    }
    input
}

#[test]
fn semantic_facts_are_bound_to_the_exact_accepted_generation() {
    let first = project_fixture("generation-first", "fn root() {}\n");
    let second = project_fixture("generation-second", "fn root() {}\n");
    let facts = RuntimePlanSemanticFacts::try_new(
        first.executable_view().expect("first executable view"),
        complete_type_input(&first),
    )
    .expect("complete checked fact set");

    assert_eq!(
        facts.validate_generation(first.executable_view().expect("same generation")),
        Ok(())
    );
    assert_eq!(
        facts.validate_generation(second.executable_view().expect("foreign generation")),
        Err(RuntimeSemanticFactsError::WrongProjectGeneration)
    );
}

#[test]
fn checked_literal_fact_uses_the_qualified_expression_owner() {
    let project = project_fixture("literal-owner", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_literal(owner, RuntimeValue::Bool(true));

    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("literal fact");
    assert_eq!(
        facts.expression_literal(owner),
        Some(&RuntimeValue::Bool(true))
    );
}

#[test]
fn checked_flow_identity_uses_the_qualified_item_owner() {
    let project = project_fixture("flow-owner", "flow opening {}\n");
    let owner = flow_item(&project);
    let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
    let mut input = complete_type_input(&project);
    input.push_flow(owner, identity.clone());

    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("Flow identity fact");
    assert_eq!(facts.flow(owner), Some(&identity));
}

#[test]
fn wrong_expression_family_is_not_reinterpreted() {
    let project = project_fixture("wrong-family", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_value(
        owner,
        RuntimeResolvedValue::Constant(RuntimeValue::Bool(true)),
    );

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("literal cannot masquerade as a resolved path"),
        RuntimeSemanticFactsError::WrongExpressionFamily {
            expression: owner,
            expected: RuntimeSemanticFactFamily::Value,
        }
    );
}

#[test]
fn dialogue_line_fact_owns_the_checked_path_only_runtime_identity() {
    let project = project_fixture(
        "dialogue-line",
        "fn root() {\n    let line: Ref<DialogueLine> = @say.story.greeting\n}\n",
    );
    let owner = entity_reference(&project);
    let line = RuntimeLineId::from_source_entity_body("say.story.greeting")
        .expect("checked dialogue line conversion");
    let mut input = complete_type_input(&project);
    input.push_value(owner, RuntimeResolvedValue::DialogueLine(line.clone()));

    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("typed dialogue-line runtime fact");
    assert_eq!(line.canonical_label(), "story.greeting");
    assert_eq!(
        facts.value(owner),
        Some(&RuntimeResolvedValue::DialogueLine(line))
    );
}

#[test]
fn duplicate_facts_are_rejected_before_publication() {
    let project = project_fixture("duplicate", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_literal(owner, RuntimeValue::Bool(true));
    input.push_expression_literal(owner, RuntimeValue::Bool(false));

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("duplicate fact must fail atomically"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::ExpressionLiteral,
        }
    );
}

#[test]
fn accepted_expression_and_pattern_types_are_complete_and_exact() {
    let project = project_fixture(
        "complete-types",
        "fn root(value: bool) {\n    match value { true => (), false => () }\n}\n",
    );
    let input = complete_type_input(&project);
    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("complete type facts");

    for (_, module) in project
        .executable_view()
        .expect("executable fixture")
        .modules()
    {
        for (owner, _) in module.expressions() {
            assert_eq!(facts.expression_type(owner), Some(&unit_type()));
        }
        for (owner, _) in module.patterns() {
            assert_eq!(facts.pattern_type(owner), Some(&unit_type()));
        }
    }
}

#[test]
fn missing_expression_type_is_rejected_before_publication() {
    let project = project_fixture("missing-expression-type", "fn root() { true }\n");
    let owner = boolean_literal(&project);

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            RuntimePlanSemanticFactInput::new(),
        )
        .expect_err("an accepted expression cannot omit its type"),
        RuntimeSemanticFactsError::MissingExpressionType { expression: owner },
    );
}

#[test]
fn missing_pattern_type_is_rejected_before_publication() {
    let project = project_fixture(
        "missing-pattern-type",
        "fn root(value: bool) {\n    match value { true => (), false => () }\n}\n",
    );
    let mut input = RuntimePlanSemanticFactInput::new();
    let executable = project.executable_view().expect("executable fixture");
    for (_, module) in executable.modules() {
        for (owner, _) in module.expressions() {
            input.push_expression_type(owner, unit_type());
        }
    }
    let pattern = executable
        .modules()
        .flat_map(|(_, module)| module.patterns())
        .map(|(owner, _)| owner)
        .next()
        .expect("pattern fixture");

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, input)
            .expect_err("an accepted pattern cannot omit its type"),
        RuntimeSemanticFactsError::MissingPatternType { pattern },
    );
}

#[test]
fn duplicate_expression_types_are_rejected_before_publication() {
    let project = project_fixture("duplicate-expression-type", "fn root() { true }\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_type(owner, unit_type());

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("one expression cannot own two accepted types"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::ExpressionType,
        },
    );
}

#[test]
fn duplicate_pattern_types_are_rejected_before_publication() {
    let project = project_fixture(
        "duplicate-pattern-type",
        "fn root(value: bool) { match value { true => (), false => () } }\n",
    );
    let pattern = project
        .executable_view()
        .expect("executable fixture")
        .modules()
        .flat_map(|(_, module)| module.patterns())
        .map(|(owner, _)| owner)
        .next()
        .expect("pattern fixture");
    let mut input = complete_type_input(&project);
    input.push_pattern_type(pattern, unit_type());

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("one pattern cannot own two accepted types"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::PatternType,
        },
    );
}

#[test]
fn nested_operational_expression_type_is_retained_without_reconstruction() {
    let project = project_fixture("nested-operational-type", "fn root() { true }\n");
    let owner = boolean_literal(&project);
    let leaf = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x22; 32]),
        RuntimeTypeShape::Unit,
    );
    let range = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x33; 32]),
        RuntimeTypeShape::Range(Box::new(leaf)),
    );
    let nested = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x44; 32]),
        RuntimeTypeShape::Option(Box::new(range)),
    );
    let mut input = RuntimePlanSemanticFactInput::new();
    for (_, module) in project
        .executable_view()
        .expect("executable fixture")
        .modules()
    {
        for (expression, _) in module.expressions() {
            input.push_expression_type(expression, nested.clone());
        }
        for (pattern, _) in module.patterns() {
            input.push_pattern_type(pattern, nested.clone());
        }
    }
    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("nested operational fact remains accepted semantic data");

    assert_eq!(facts.expression_type(owner), Some(&nested));
    assert_eq!(
        facts
            .expression_type(owner)
            .expect("exact retained type")
            .checked_type(),
        Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
            semantic_identity: RuntimeSemanticTypeId::from_bytes([0x33; 32]),
            path: super::RuntimeTypeProjectionPath::root()
                .pushed(RuntimeTypeProjectionStep::OptionItem),
            shape: RuntimeUnsupportedTypeShape::Range,
        }),
    );
}

#[test]
fn postfix_type_completeness_keeps_only_the_selected_candidate_expression_tree() {
    let project = project_fixture(
        "postfix-selected-types",
        "fn root(items: Vec<i64>, subject: i64) {\n    items[{ match subject { value => value }; 0 }]\n}\n",
    );
    let executable = project.executable_view().expect("executable fixture");
    let modules = executable
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let (postfix_owner, target, index, dialogue) = modules
        .values()
        .flat_map(|module| module.expressions())
        .find_map(|(owner, expression)| {
            let HirExprKind::PostfixBracket(postfix) = expression.kind() else {
                return None;
            };
            let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
            else {
                return None;
            };
            Some((owner, postfix.target(), *index, *dialogue))
        })
        .expect("ambiguous postfix fixture");
    assert!(
        modules
            .values()
            .flat_map(|module| module.patterns())
            .next()
            .is_some(),
        "the ordinary candidate retains a Match pattern"
    );

    let postfix_candidates = BTreeMap::from([(postfix_owner, dialogue)]);
    let accepted = executable
        .selected_expression_owners(|owner| postfix_candidates.get(&owner).copied())
        .expect("selected candidate inventory");
    assert!(accepted.contains(&postfix_owner));
    assert!(accepted.contains(&target));
    assert!(accepted.contains(&dialogue));
    assert!(!accepted.contains(&index));

    let complete_selected_input = || {
        let mut input = RuntimePlanSemanticFactInput::new();
        for owner in &accepted {
            input.push_expression_type(*owner, unit_type());
        }
        for module in modules.values() {
            for (owner, _) in module.patterns() {
                input.push_pattern_type(owner, unit_type());
            }
        }
        input.push_postfix_candidate(postfix_owner, dialogue);
        input
    };
    let facts = RuntimePlanSemanticFacts::try_new(executable, complete_selected_input())
        .expect("the rolled-back expression candidate needs no type fact");
    assert!(facts.expression_type(dialogue).is_some());
    assert!(facts.expression_type(index).is_none());

    let mut input = complete_selected_input();
    input.push_expression_type(index, unit_type());
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, input)
            .expect_err("an unselected candidate cannot publish an expression type"),
        RuntimeSemanticFactsError::InactiveExpressionType { expression: index },
    );
}

#[test]
fn semantic_identities_round_trip_without_display_labels() {
    let type_bytes = [0x5a; 32];
    let registered_bytes = [0xa5; 32];
    assert_eq!(
        RuntimeSemanticTypeId::from_bytes(type_bytes).as_bytes(),
        &type_bytes
    );
    assert_eq!(
        RuntimeRegisteredValueId::from_bytes(registered_bytes).as_bytes(),
        &registered_bytes
    );
}

#[test]
fn reduction_constructor_fact_requires_one_authored_value_argument() {
    let project = project_fixture(
        "reduction-constructor",
        "fn root(value: i32) {\n    identity(value)\n}\n",
    );
    let owner = call_expression(&project);
    let valid = RuntimeResolvedCall::new(
        RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged),
        [RuntimeResolvedCallArgument::Authored { ordinal: 0 }],
        RuntimeCallResultShape::Value,
    );
    let mut input = complete_type_input(&project);
    input.push_call(owner, valid.clone());
    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("typed Reduction constructor fact");
    assert_eq!(facts.call(owner), Some(&valid));

    for invalid in [
        RuntimeResolvedCall::new(
            RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged),
            [],
            RuntimeCallResultShape::Value,
        ),
        RuntimeResolvedCall::new(
            RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged),
            [RuntimeResolvedCallArgument::Authored { ordinal: 0 }],
            RuntimeCallResultShape::PartialFunction,
        ),
    ] {
        let mut input = complete_type_input(&project);
        input.push_call(owner, invalid);
        assert_eq!(
            RuntimePlanSemanticFacts::try_new(
                project.executable_view().expect("executable fixture"),
                input,
            )
            .expect_err("fabricated Reduction constructor fact must fail"),
            RuntimeSemanticFactsError::InvalidReductionConstructorCall,
        );
    }
}

#[test]
fn opaque_composite_projection_preserves_complete_owner_and_first_error_path() {
    let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.runtime-plan.opaque")
        .expect("valid fixture producer");
    let opaque = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([1; 32]),
        RuntimeTypeShape::Opaque {
            producer: producer.clone(),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
        },
    );
    let closed = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([2; 32]),
        RuntimeTypeShape::Result {
            value: Box::new(opaque.clone()),
            error: Box::new(opaque),
        },
    );
    assert!(matches!(
        closed.checked_type().expect("complete opaque Result owner"),
        RuntimeCheckedType::Result { ok, error }
            if matches!(*ok, RuntimeCheckedType::Opaque { .. })
                && matches!(*error, RuntimeCheckedType::Opaque { .. })
    ));

    let unsupported = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([3; 32]),
        RuntimeTypeShape::Result {
            value: Box::new(super::RuntimeNormalizedType::new(
                RuntimeSemanticTypeId::from_bytes([4; 32]),
                RuntimeTypeShape::Range(Box::new(super::RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([5; 32]),
                    RuntimeTypeShape::Unit,
                ))),
            )),
            error: Box::new(super::RuntimeNormalizedType::new(
                RuntimeSemanticTypeId::from_bytes([6; 32]),
                RuntimeTypeShape::Function {
                    parameters: Box::new([]),
                    result: Box::new(super::RuntimeNormalizedType::new(
                        RuntimeSemanticTypeId::from_bytes([7; 32]),
                        RuntimeTypeShape::Unit,
                    )),
                },
            )),
        },
    );
    assert_eq!(
        unsupported.checked_type(),
        Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
            semantic_identity: RuntimeSemanticTypeId::from_bytes([4; 32]),
            path: super::RuntimeTypeProjectionPath::root()
                .pushed(RuntimeTypeProjectionStep::ResultOk),
            shape: RuntimeUnsupportedTypeShape::Range,
        })
    );
}

#[test]
fn checked_variant_selection_retains_both_result_branches() {
    let ok = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([8; 32]),
        RuntimeTypeShape::Unit,
    );
    let error = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([9; 32]),
        RuntimeTypeShape::String,
    );
    let selection = RuntimeResolvedVariant::result_ok(ok, error)
        .checked_selection()
        .expect("complete Result selection");
    assert_eq!(selection.ordinal(), 0);
    assert_eq!(selection.name(), "Ok");
    assert_eq!(selection.payload(), Some(&RuntimeCheckedType::Unit));
    assert_eq!(
        selection.owner(),
        &RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Unit),
            error: Box::new(RuntimeCheckedType::String),
        }
    );
}
