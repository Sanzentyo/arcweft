use std::sync::Arc;

use arcweft_core::{
    effect::{
        RuntimeArtifactFingerprint, RuntimeAssertion, RuntimeAssertionFailure,
        RuntimeAssertionGuardId, RuntimeAssertionProfile,
    },
    plan::{FlowOp, FlowRuntimeId},
    value::RuntimeValue,
};
use arcweft_lang_hir::{
    database::HirDatabase,
    expr::HirThreadFlowItem,
    item::HirItemKind,
    lowering::{HirModuleKey, LoweringRequest},
    project::{HirProjectBuilder, HirProjectModule},
    proof_return::HirProofReturnSemanticFactSet,
    stmt::HirStmtKind,
    symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_runtime_plan::{
    assertion_identity::RuntimeAssertionMode,
    flow::{RuntimeEntryLoweringInput, lower_runtime_plan_with_stats},
    semantic_facts::{
        RuntimeAssertionAdmission, RuntimeNormalizedType, RuntimePlanSemanticFactInput,
        RuntimePlanSemanticFacts, RuntimeSemanticTypeId, RuntimeTypeShape,
    },
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};
use arcweft_tooling::runtime_diagnostic::{
    RUNTIME_ASSERTION_FAILED_CODE, RuntimeAssertionDiagnosticIdentity,
    project_persisted_assertion_failure, project_runtime_assertion_fault,
};

#[test]
fn reloaded_artifact_without_exact_source_association_stays_unassociated() {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://runtime-diagnostic").unwrap(),
        SourceName::path("runtime-diagnostic.arcw"),
        "assert.check(ready)",
    )
    .unwrap();
    let span = source.span(SourceRange::new(13, 18)).unwrap();
    let failure = RuntimeAssertionFailure::new(RuntimeAssertion::new(
        RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap(),
        "ready".to_owned(),
        "not ready".to_owned(),
        RuntimeAssertionProfile::Always,
    ));

    let diagnostic = project_persisted_assertion_failure(&failure, Some(span.clone()));
    assert_eq!(diagnostic.code(), RUNTIME_ASSERTION_FAILED_CODE);
    assert_eq!(diagnostic.message(), "not ready");
    assert_eq!(diagnostic.primary().unwrap().span(), &span);
    assert_eq!(diagnostic.primary().unwrap().message(), "ready");
    assert!(diagnostic.secondary().is_empty());
    assert_eq!(
        diagnostic.identity(),
        &RuntimeAssertionDiagnosticIdentity::PersistedOnly
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test follows one assertion identity through syntax, HIR, runtime lowering, and tooling projection"
)]
fn runtime_projection_emits_stable_diagnostic_without_message_parsing() {
    let source = "flow checks { assert.check(true) }\n";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/runtime-assertion")
                .expect("fixture document ID"),
            SourceName::path("runtime-assertion.arcw"),
            source,
        )
        .expect("fixture document"),
    );
    let package = CallablePackageId::try_new("tooling.runtime-assertion").expect("fixture package");
    let path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("attached fixture parse");
    let key = HirModuleKey::new(
        package.clone(),
        path.clone(),
        parsed.document().identity().clone(),
    );
    let mut hir = HirDatabase::try_new().expect("HIR database");
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        parsed.document().identity().id().clone(),
        "tooling-runtime-assertion-test",
    )
    .expect("fixture symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
        .expect("fixture symbol revision");
    let transaction = hir
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
    .expect("tooling fixture has no authored Proof return headers");
    let mut outputs = transaction
        .publish_with_semantic_facts(&mut hir, facts)
        .expect("final HIR project publishes");
    let module = outputs
        .pop()
        .expect("one tooling fixture module")
        .into_module();
    assert!(outputs.is_empty());
    let project_module =
        HirProjectModule::try_new(&hir, &package, &path, parsed.document().identity(), module)
            .expect("accepted module lease");
    let mut builder = HirProjectBuilder::new(&hir, package);
    builder
        .insert_module(project_module)
        .expect("module insertion");
    let project = builder.finish().expect("fixture project");
    let executable = project.executable_view().expect("executable project");
    let (flow_owner, statement, condition) = executable
        .items()
        .find_map(|item| {
            let HirItemKind::Flow(flow) = item.item().kind() else {
                return None;
            };
            let HirThreadFlowItem::Statement(statement) = flow.body().items().first()? else {
                return None;
            };
            let HirStmtKind::Assertion { conditions, .. } =
                item.module().resolve_stmt(*statement).ok()?.kind()
            else {
                return None;
            };
            Some((item.id(), *statement, *conditions.first()?))
        })
        .expect("typed assertion statement");
    let mut input = RuntimePlanSemanticFactInput::new();
    for (_, module) in executable.modules() {
        for (owner, _) in module.locals() {
            input.push_local_declaration(
                owner,
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                    RuntimeTypeShape::Unit,
                ),
            );
        }
        for (owner, _) in module.expressions() {
            let expression_type = if owner == condition {
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x12; 32]),
                    RuntimeTypeShape::Bool,
                )
            } else {
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                    RuntimeTypeShape::Unit,
                )
            };
            input.push_expression_type(owner, expression_type);
        }
        for (owner, _) in module.patterns() {
            input.push_pattern_type(
                owner,
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                    RuntimeTypeShape::Unit,
                ),
            );
        }
    }
    input.push_flow(
        flow_owner,
        FlowRuntimeId::canonical("checks").expect("runtime Flow identity"),
    );
    input.push_expression_literal(condition, RuntimeValue::Bool(false));
    input.push_assertion(
        statement,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
    );
    let facts =
        RuntimePlanSemanticFacts::try_new(executable, input).expect("checked runtime facts");
    let report = lower_runtime_plan_with_stats(
        executable,
        &facts,
        &RuntimeEntryLoweringInput::empty(executable),
    )
    .expect("runtime assertion lowers");
    let guard = report.plan.flows()[0]
        .ops
        .iter()
        .find_map(|operation| match operation {
            FlowOp::EvaluatedEffect(arcweft_core::effect::RuntimeEffectExpr::Assert {
                guard,
                ..
            }) => Some(*guard),
            _ => None,
        })
        .expect("lowered assertion guard");
    let artifact =
        RuntimeArtifactFingerprint::try_from_bytes([0x51; 32]).expect("artifact fingerprint");
    let fault = report
        .bind_assertion_inventory(artifact)
        .project_failure(
            artifact,
            RuntimeAssertionFailure::new(RuntimeAssertion::new(
                guard,
                "deliberately unrelated observed condition text".to_owned(),
                String::new(),
                RuntimeAssertionProfile::Always,
            )),
        )
        .expect("exact fresh-session association uses the guard, not message text");

    let diagnostic = project_runtime_assertion_fault(&fault);
    assert_eq!(diagnostic.code(), RUNTIME_ASSERTION_FAILED_CODE);
    assert_eq!(diagnostic.message(), "assertion condition 0 failed");
    assert_eq!(
        diagnostic.identity(),
        &RuntimeAssertionDiagnosticIdentity::Session {
            mode: RuntimeAssertionMode::Check,
            condition_index: 0,
        }
    );
    assert_eq!(
        diagnostic.primary().expect("condition label").message(),
        "true"
    );
    assert_eq!(diagnostic.secondary().len(), 1);
    assert_eq!(diagnostic.secondary()[0].message(), "assertion statement");
    let shared = diagnostic.to_source_diagnostic();
    assert_eq!(
        shared.code().map(arcweft_source::DiagnosticCode::as_str),
        Some(RUNTIME_ASSERTION_FAILED_CODE)
    );
    assert_eq!(shared.labels().len(), 2);
}
