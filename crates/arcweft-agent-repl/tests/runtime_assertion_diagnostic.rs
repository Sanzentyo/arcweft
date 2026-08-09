use std::sync::Arc;

use arcweft_agent_repl::diagnostics::{
    RuntimeAssertionDebugContext, project_runtime_assertion_debug_diagnostic,
};
use arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext;
use arcweft_core::{
    effect::{RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionProfile},
    plan::{FlowOp, FlowRuntimeId},
    value::RuntimeValue,
};
use arcweft_lang_hir::{
    database::HirDatabase,
    expr::HirThreadFlowItem,
    item::HirItemKind,
    lowering::{HirModuleKey, LoweringRequest},
    project::{HirProject, HirProjectModule},
    proof_return::HirProofReturnSemanticFactSet,
    stmt::HirStmtKind,
    symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_project::{
    artifact::{ArtifactKeyInput, RuntimePlanArtifactKey},
    fingerprint::BuildDigest,
    incremental::QueryKind,
};
use arcweft_runtime_plan::{
    assertion_identity::{RuntimeAssertionMode, RuntimeAssertionProjectionError},
    flow::{RuntimeEntryLoweringInput, lower_runtime_plan_with_stats},
    semantic_facts::{
        RuntimeAssertionAdmission, RuntimePlanSemanticFactInput, RuntimePlanSemanticFacts,
    },
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};
use arcweft_tooling::runtime_diagnostic::{
    RuntimeAssertionDiagnosticIdentity, project_runtime_assertion_fault,
};

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the assertion diagnostic identity test verifies one end-to-end session projection matrix"
)]
fn agent_debug_diagnostic_projects_fresh_session_fault() {
    let source = "flow checks { assert.check(true) }\n";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://agent/runtime-assertion")
                .expect("fixture document ID"),
            SourceName::path("agent-runtime-assertion.arcw"),
            source,
        )
        .expect("fixture document"),
    );
    let package = CallablePackageId::try_new("agent.runtime-assertion").expect("fixture package");
    let module_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("attached source");
    let module_key = HirModuleKey::new(
        package.clone(),
        module_path.clone(),
        parsed.document().identity().id().clone(),
    );
    let mut hir = HirDatabase::try_new().expect("HIR database");
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        parsed.document().identity().id().clone(),
        "agent-runtime-assertion-test",
    )
    .expect("symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
        .expect("symbol revision");
    let transaction = hir
        .stage_proof_return_project(
            [LoweringRequest::try_new(module_key, &parsed).expect("lowering request")],
            world,
            revision,
            [parsed.document().identity()],
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .expect("HIR transaction");
    let proof_returns = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("fixture has no Proof return headers");
    let mut lowered = transaction
        .publish_with_semantic_facts(&mut hir, proof_returns)
        .expect("published HIR module");
    let module = lowered.pop().expect("one module").into_module();
    assert!(lowered.is_empty());
    let project_module = HirProjectModule::try_new(
        &hir,
        &package,
        &module_path,
        parsed.document().identity(),
        module,
    )
    .expect("accepted module lease");
    let project = HirProject::try_new(&hir, package, [project_module]).expect("HIR project");
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
    input.push_flow(
        flow_owner,
        FlowRuntimeId::canonical("checks").expect("runtime flow identity"),
    );
    input.push_expression_literal(condition, RuntimeValue::Bool(false));
    input.push_assertion(
        statement,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
    );
    let facts = RuntimePlanSemanticFacts::try_new(executable, input).expect("runtime facts");
    let report = lower_runtime_plan_with_stats(
        executable,
        &facts,
        &RuntimeEntryLoweringInput::empty(executable),
    )
    .expect("runtime plan");
    let guard = report.plan.flows[0]
        .ops
        .iter()
        .find_map(|operation| match operation {
            FlowOp::EvaluatedEffect(arcweft_core::effect::RuntimeEffectExpr::Assert {
                guard,
                ..
            }) => Some(*guard),
            _ => None,
        })
        .expect("runtime assertion guard");
    let artifact_key = RuntimePlanArtifactKey::try_derive(&ArtifactKeyInput {
        compiler_build_id: "agent-runtime-assertion-test".to_owned(),
        query: QueryKind::RuntimePlan,
        artifact_kind: QueryKind::RuntimePlan.artifact_kind(),
        target_triple: "native".to_owned(),
        target_features: Vec::new(),
        profile: "debug".to_owned(),
        package: "agent.runtime-assertion".to_owned(),
        logical_item: "runtime-plan".to_owned(),
        source_digest: BuildDigest::from(document.identity().revision()),
        dependency_interface_digests: Vec::new(),
        dependency_body_digests: Vec::new(),
        adapter_environment_digest: BuildDigest::ZERO,
        launch_profile_digest: BuildDigest::ZERO,
        declared_environment_digest: BuildDigest::ZERO,
        format_options_digest: BuildDigest::of(QueryKind::RuntimePlan.cache_namespace().as_bytes()),
    })
    .expect("typed runtime-plan artifact key");
    let context = ExecutionDiagnosticContext::try_from_runtime_plan_artifact(artifact_key, &report)
        .expect("canonical runtime-plan key binds the exact inventory");
    let failure = RuntimeAssertionFailure::new(RuntimeAssertion::new(
        guard,
        "text that must not be parsed for identity".to_owned(),
        String::new(),
        RuntimeAssertionProfile::Always,
    ));
    let fault = context
        .project_assertion_failure(failure.clone())
        .expect("failure joins the fresh session inventory");
    let diagnostic = project_runtime_assertion_fault(&fault);
    assert_eq!(
        diagnostic.identity(),
        &RuntimeAssertionDiagnosticIdentity::Session {
            mode: RuntimeAssertionMode::Check,
            condition_index: 0,
        }
    );
    assert_eq!(diagnostic.message(), "assertion condition 0 failed");
    assert_eq!(
        diagnostic.primary().expect("condition label").message(),
        "true"
    );
    assert_eq!(diagnostic.secondary().len(), 1);

    let projected = project_runtime_assertion_debug_diagnostic(
        RuntimeAssertionDebugContext::new(
            "diagnostic.agent.runtime-assertion.1",
            None,
            None,
            None,
            Some(1),
            0,
        ),
        context.artifact(),
        &failure,
        &diagnostic,
    );
    assert_eq!(projected.code.as_deref(), Some("runtime.assertion_failed"));
    assert_eq!(projected.message, "assertion condition 0 failed");
    assert_eq!(
        projected.payload["source_evidence"]["primary"]["message"],
        "true"
    );
    assert_eq!(
        projected.payload["source_evidence"]["secondary"][0]["message"],
        "assertion statement"
    );

    let unknown = RuntimeAssertionFailure::new(RuntimeAssertion::new(
        arcweft_core::effect::RuntimeAssertionGuardId::try_from_bytes([0x7f; 16])
            .expect("non-zero unknown guard"),
        "true".to_owned(),
        String::new(),
        RuntimeAssertionProfile::Always,
    ));
    assert!(matches!(
        context.project_assertion_failure(unknown),
        Err(RuntimeAssertionProjectionError::UnknownGuard { .. })
    ));
}
