use std::sync::Arc;

use arcweft_core::plan::FlowRuntimeId;
use arcweft_core::value::RuntimeValue;
use arcweft_lang_hir::database::HirDatabase;
use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_hir::leaf::HirLiteral;
use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
use arcweft_lang_hir::project::{HirProject, HirProjectModule};
use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    RuntimeCallResultShape, RuntimePlanSemanticFactInput, RuntimePlanSemanticFacts,
    RuntimeReductionConstructor, RuntimeRegisteredValueId, RuntimeResolvedCall,
    RuntimeResolvedCallArgument, RuntimeResolvedCallTarget, RuntimeResolvedValue,
    RuntimeSemanticFactFamily, RuntimeSemanticFactsError, RuntimeSemanticTypeId,
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
    HirProject::try_new(&database, package, [project_module]).expect("fixture project")
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

#[test]
fn semantic_facts_are_bound_to_the_exact_accepted_generation() {
    let first = project_fixture("generation-first", "fn root() {}\n");
    let second = project_fixture("generation-second", "fn root() {}\n");
    let facts = RuntimePlanSemanticFacts::try_new(
        first.executable_view().expect("first executable view"),
        RuntimePlanSemanticFactInput::new(),
    )
    .expect("empty checked fact set");

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
    let mut input = RuntimePlanSemanticFactInput::new();
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
    let mut input = RuntimePlanSemanticFactInput::new();
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
    let mut input = RuntimePlanSemanticFactInput::new();
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
fn duplicate_facts_are_rejected_before_publication() {
    let project = project_fixture("duplicate", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = RuntimePlanSemanticFactInput::new();
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
    let mut input = RuntimePlanSemanticFactInput::new();
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
        let mut input = RuntimePlanSemanticFactInput::new();
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
