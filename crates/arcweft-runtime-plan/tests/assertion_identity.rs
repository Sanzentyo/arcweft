use std::{collections::BTreeSet, sync::Arc};

use arcweft_core::{
    effect::{
        RuntimeArtifactFingerprint, RuntimeAssertion, RuntimeAssertionFailure,
        RuntimeAssertionGuardId, RuntimeAssertionProfile, RuntimeEffectExpr,
    },
    plan::{FlowOp, FlowRuntimeId},
    value::RuntimeValue,
};
use arcweft_lang_hir::{
    database::HirDatabase,
    expr::HirThreadFlowItem,
    identity::StmtId,
    item::HirItemKind,
    lowering::{HirModuleKey, LoweringRequest},
    project::{HirProject, HirProjectBuilder, HirProjectModule},
    proof_return::HirProofReturnSemanticFactSet,
    stmt::HirStmtKind,
    symbol::{
        CallableDeclarationId, CallableDeclarationOwner, CallablePackageId, ProjectSymbolRevision,
        ProjectSymbolWorldId,
    },
};
use arcweft_lang_syntax::{
    assertion::AssertionMode,
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    incremental::SyntaxDatabase,
    parser::ParseOptions,
};
use arcweft_runtime_plan::{
    assertion_identity::{
        AssertionConditionIndex, AssertionConditionIndexError, RuntimeAssertionMode,
        RuntimeAssertionModeError, RuntimeAssertionProjectionError, derive_runtime_assertion_guard,
    },
    flow::{RuntimeEntryLoweringInput, RuntimePlanLowerReport, lower_runtime_plan_with_stats},
    semantic_facts::{
        RuntimeAssertionAdmission, RuntimeNormalizedType, RuntimePlanSemanticFactInput,
        RuntimePlanSemanticFacts, RuntimeSemanticTypeId, RuntimeTypeShape,
    },
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};

fn canonical_module(segments: &[&str]) -> CanonicalModulePath {
    CanonicalModulePath::from_segments(
        segments
            .iter()
            .map(|segment| ModuleSegment::new(*segment).unwrap()),
    )
}

fn callable_id(
    package: &CallablePackageId,
    module: &CanonicalModulePath,
    owner: CallableDeclarationOwner,
    owner_path: &[&str],
    name: &str,
) -> CallableDeclarationId {
    CallableDeclarationId::try_new_in_owner_path(
        package.clone(),
        module.clone(),
        owner,
        owner_path
            .iter()
            .map(|segment| ModuleSegment::new(*segment).unwrap()),
        name,
    )
    .unwrap()
}

struct GuardFixture {
    package: CallablePackageId,
    other_package: CallablePackageId,
    module: CanonicalModulePath,
    other_module: CanonicalModulePath,
    callable: CallableDeclarationId,
    other_callable: CallableDeclarationId,
    other_owner_callable: CallableDeclarationId,
    other_owner_path_callable: CallableDeclarationId,
    first: AssertionConditionIndex,
    second: AssertionConditionIndex,
}

impl GuardFixture {
    fn new() -> Self {
        let package = CallablePackageId::try_new("story").unwrap();
        let module = canonical_module(&["chapter", "opening"]);
        let callable = callable_id(
            &package,
            &module,
            CallableDeclarationOwner::Function,
            &["scene"],
            "run",
        );
        Self {
            other_package: CallablePackageId::try_new("story.extra").unwrap(),
            other_module: canonical_module(&["chapter", "ending"]),
            other_callable: callable_id(
                &package,
                &module,
                CallableDeclarationOwner::Function,
                &["scene"],
                "resume",
            ),
            other_owner_callable: callable_id(
                &package,
                &module,
                CallableDeclarationOwner::View,
                &["scene"],
                "run",
            ),
            other_owner_path_callable: callable_id(
                &package,
                &module,
                CallableDeclarationOwner::Function,
                &["chapter", "scene"],
                "run",
            ),
            first: AssertionConditionIndex::try_new(0, 2).unwrap(),
            second: AssertionConditionIndex::try_new(1, 2).unwrap(),
            package,
            module,
            callable,
        }
    }

    fn expected(&self) -> RuntimeAssertionGuardId {
        derive_runtime_assertion_guard(
            &self.package,
            &self.module,
            &self.callable,
            7,
            self.first,
            RuntimeAssertionProfile::Always,
        )
    }

    fn changed_seed_guards(&self) -> [RuntimeAssertionGuardId; 8] {
        [
            derive_runtime_assertion_guard(
                &self.other_package,
                &self.module,
                &self.callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.other_module,
                &self.callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.other_callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.other_owner_callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.other_owner_path_callable,
                7,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.callable,
                8,
                self.first,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.callable,
                7,
                self.second,
                RuntimeAssertionProfile::Always,
            ),
            derive_runtime_assertion_guard(
                &self.package,
                &self.module,
                &self.callable,
                7,
                self.first,
                RuntimeAssertionProfile::DebugOnly,
            ),
        ]
    }
}

#[test]
fn guard_derivation_uses_typed_seed_and_is_deterministic() {
    let fixture = GuardFixture::new();
    let expected = fixture.expected();
    assert_eq!(
        expected.as_bytes(),
        &[
            0x5f, 0x3b, 0x1c, 0xcf, 0xea, 0x6b, 0xac, 0x47, 0x5e, 0xba, 0x86, 0xa0, 0x78, 0xc9,
            0xa8, 0x98,
        ]
    );
    assert_eq!(fixture.expected(), expected);

    let variants = fixture.changed_seed_guards();
    for variant in variants {
        assert_ne!(variant, expected);
    }
}

#[test]
fn check_failure_retains_exact_session_identity() {
    let project = project_fixture(
        "runtime-assertion-identity",
        "flow checks { assert.check(true, false) }\n",
    );
    let (report, statement, guards) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
        &[true, false],
    );

    assert_eq!(report.assertion_site_count(), 2);
    assert_eq!(guards.len(), 2);
    assert_ne!(guards[0], guards[1]);
    assert!(matches!(
        report.plan.flows[0].ops.as_slice(),
        [
            FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
                profile: RuntimeAssertionProfile::Always,
                ..
            }),
            FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
                profile: RuntimeAssertionProfile::Always,
                ..
            })
        ]
    ));

    let artifact = RuntimeArtifactFingerprint::try_from_bytes([9; 32]).expect("runtime artifact");
    let inventory = report.bind_assertion_inventory(artifact);
    for (index, guard) in guards.iter().copied().enumerate() {
        let site = inventory.site(guard).expect("fresh-session assertion site");
        assert_eq!(site.statement(), statement);
        assert_eq!(usize::from(site.condition().get()), index);
        assert_eq!(site.mode(), RuntimeAssertionMode::Check);
        assert_eq!(
            site.presentation().condition_label(),
            if index == 0 { "true" } else { "false" }
        );
    }

    let first = inventory.site(guards[0]).expect("first assertion site");
    let condition_span = first.condition_span().clone();
    let statement_span = first.presentation().statement_span().clone();
    let fault = inventory
        .project_failure(
            artifact,
            RuntimeAssertionFailure::new(RuntimeAssertion::new(
                guards[0],
                "false".to_owned(),
                "first condition failed".to_owned(),
                RuntimeAssertionProfile::Always,
            )),
        )
        .expect("Check failure joins the exact fresh-session site");
    assert_eq!(fault.identity().statement(), statement);
    assert_eq!(fault.identity().condition().get(), 0);
    assert_eq!(fault.identity().mode(), RuntimeAssertionMode::Check);
    assert_eq!(fault.identity().span(), &condition_span);
    assert_eq!(fault.presentation().statement_span(), &statement_span);
    assert_eq!(fault.presentation().condition_label(), "true");
}

#[test]
fn enabled_debug_failure_retains_exact_session_identity() {
    let project = project_fixture(
        "debug-assertion-enabled",
        "flow checks { assert.debug(true, false) }\n",
    );
    let (report, statement, guards) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Debug),
        &[true, false],
    );

    assert_eq!(report.assertion_site_count(), 2);
    assert_eq!(guards.len(), 2);
    assert!(matches!(
        report.plan.flows[0].ops.as_slice(),
        [
            FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
                profile: RuntimeAssertionProfile::DebugOnly,
                ..
            }),
            FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
                profile: RuntimeAssertionProfile::DebugOnly,
                ..
            })
        ]
    ));

    let artifact = RuntimeArtifactFingerprint::try_from_bytes([5; 32]).expect("artifact");
    let inventory = report.bind_assertion_inventory(artifact);
    let second = inventory.site(guards[1]).expect("second Debug site");
    assert_eq!(second.statement(), statement);
    assert_eq!(second.condition().get(), 1);
    assert_eq!(second.mode(), RuntimeAssertionMode::Debug);
    assert_eq!(second.presentation().condition_label(), "false");
    let condition_span = second.condition_span().clone();
    let statement_span = second.presentation().statement_span().clone();
    let fault = inventory
        .project_failure(
            artifact,
            RuntimeAssertionFailure::new(RuntimeAssertion::new(
                guards[1],
                "false".to_owned(),
                "second debug condition failed".to_owned(),
                RuntimeAssertionProfile::DebugOnly,
            )),
        )
        .expect("Debug failure joins the exact fresh-session site");
    assert_eq!(fault.identity().statement(), statement);
    assert_eq!(fault.identity().condition().get(), 1);
    assert_eq!(fault.identity().mode(), RuntimeAssertionMode::Debug);
    assert_eq!(fault.identity().span(), &condition_span);
    assert_eq!(fault.presentation().statement_span(), &statement_span);
    assert_eq!(fault.presentation().condition_label(), "false");
}

#[test]
fn condition_indices_follow_authored_zero_based_order() {
    let source = format!(
        "flow checks {{ assert.check({}) }}\n",
        std::iter::repeat_n("true", 64)
            .collect::<Vec<_>>()
            .join(", ")
    );
    let project = project_fixture("sixty-four-assertion-conditions", &source);
    let values = [true; 64];
    let (report, statement, guards) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
        &values,
    );

    assert_eq!(report.assertion_site_count(), 64);
    assert_eq!(guards.len(), 64);
    assert_eq!(guards.iter().copied().collect::<BTreeSet<_>>().len(), 64);
    let artifact = RuntimeArtifactFingerprint::try_from_bytes([6; 32]).expect("artifact");
    let inventory = report.bind_assertion_inventory(artifact);
    for (index, guard) in guards.into_iter().enumerate() {
        let site = inventory.site(guard).expect("condition site");
        assert_eq!(site.statement(), statement);
        assert_eq!(usize::from(site.condition().get()), index);
    }
}

#[test]
fn condition_index_validation_rejects_invalid_count_and_bounds() {
    assert_eq!(AssertionConditionIndex::try_new(0, 1).unwrap().get(), 0);
    assert_eq!(AssertionConditionIndex::try_new(63, 64).unwrap().get(), 63);
    assert_eq!(
        AssertionConditionIndex::try_new(0, 0),
        Err(AssertionConditionIndexError::InvalidConditionCount { count: 0 })
    );
    assert_eq!(
        AssertionConditionIndex::try_new(0, 65),
        Err(AssertionConditionIndexError::InvalidConditionCount { count: 65 })
    );
    assert_eq!(
        AssertionConditionIndex::try_new(64, 64),
        Err(AssertionConditionIndexError::OutOfBounds {
            index: 64,
            count: 64,
        })
    );
}

#[test]
fn prove_has_no_runtime_mode_or_guard() {
    assert_eq!(
        RuntimeAssertionMode::try_from_assertion_mode(AssertionMode::Prove),
        Err(RuntimeAssertionModeError::ProveHasNoRuntimeRepresentation)
    );
    let project = project_fixture("proved-assertion", "flow checks { assert.prove(true) }\n");
    let (report, _, guards) =
        lower_assertion_project(&project, RuntimeAssertionAdmission::Discharged, &[true]);
    assert_eq!(report.assertion_site_count(), 0);
    assert!(guards.is_empty());
    assert!(report.plan.flows[0].ops.is_empty());
}

#[test]
fn release_plan_omits_debug_evaluation_and_inventory() {
    let project = project_fixture(
        "debug-assertion-profile",
        "flow checks { assert.debug(true, false) }\n",
    );
    let (debug_report, debug_statement, debug_guards) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Debug),
        &[true, false],
    );
    let (release_report, release_statement, release_guards) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::OmittedDebug,
        &[true, false],
    );

    assert_eq!(debug_statement, release_statement);
    assert_eq!(debug_report.assertion_site_count(), 2);
    assert_eq!(debug_guards.len(), 2);
    assert_eq!(release_report.assertion_site_count(), 0);
    assert!(release_guards.is_empty());
    assert!(release_report.plan.flows[0].ops.is_empty());
}

#[test]
fn runtime_fault_invalid_guard_is_typed_error() {
    let project = project_fixture(
        "unknown-runtime-assertion-guard",
        "flow checks { assert.check(true) }\n",
    );
    let (report, _, _) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
        &[true],
    );
    let artifact = RuntimeArtifactFingerprint::try_from_bytes([3; 32]).expect("artifact");
    let inventory = report.bind_assertion_inventory(artifact);
    let unknown = RuntimeAssertionGuardId::try_from_bytes([8; 16]).expect("unknown guard");

    assert_eq!(
        inventory.project_failure(artifact, failure(unknown, RuntimeAssertionProfile::Always),),
        Err(RuntimeAssertionProjectionError::UnknownGuard { guard: unknown })
    );
}

#[test]
fn runtime_fault_artifact_mismatch_is_typed_error() {
    let project = project_fixture(
        "runtime-assertion-artifact-mismatch",
        "flow checks { assert.check(true) }\n",
    );
    let (report, _, guards) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
        &[true],
    );
    let artifact = RuntimeArtifactFingerprint::try_from_bytes([3; 32]).expect("artifact");
    let foreign = RuntimeArtifactFingerprint::try_from_bytes([4; 32]).expect("foreign artifact");
    let inventory = report.bind_assertion_inventory(artifact);

    assert!(matches!(
        inventory.project_failure(
            foreign,
            failure(guards[0], RuntimeAssertionProfile::Always),
        ),
        Err(RuntimeAssertionProjectionError::ArtifactMismatch {
            expected,
            actual,
        }) if expected == artifact && actual == foreign
    ));
}

#[test]
fn runtime_fault_profile_mismatch_is_typed_error() {
    let project = project_fixture(
        "runtime-assertion-profile-mismatch",
        "flow checks { assert.check(true) }\n",
    );
    let (report, _, guards) = lower_assertion_project(
        &project,
        RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
        &[true],
    );
    let artifact = RuntimeArtifactFingerprint::try_from_bytes([3; 32]).expect("artifact");
    let inventory = report.bind_assertion_inventory(artifact);

    assert_eq!(
        inventory.project_failure(
            artifact,
            failure(guards[0], RuntimeAssertionProfile::DebugOnly),
        ),
        Err(RuntimeAssertionProjectionError::ProfileModeMismatch {
            guard: guards[0],
            profile: RuntimeAssertionProfile::DebugOnly,
            mode: RuntimeAssertionMode::Check,
        })
    );
}

fn failure(
    guard: RuntimeAssertionGuardId,
    profile: RuntimeAssertionProfile,
) -> RuntimeAssertionFailure {
    RuntimeAssertionFailure::new(RuntimeAssertion::new(
        guard,
        "condition".to_owned(),
        "failed".to_owned(),
        profile,
    ))
}

fn lower_assertion_project(
    project: &HirProject,
    admission: RuntimeAssertionAdmission,
    values: &[bool],
) -> (RuntimePlanLowerReport, StmtId, Vec<RuntimeAssertionGuardId>) {
    let executable = project.executable_view().expect("executable fixture");
    let (flow_owner, statement, conditions) = executable
        .items()
        .find_map(|item| {
            let HirItemKind::Flow(flow) = item.item().kind() else {
                return None;
            };
            let HirThreadFlowItem::Statement(statement) =
                flow.body().items().first().expect("assertion statement")
            else {
                panic!("assertion remains an ordinary statement flow item");
            };
            let resolved = item
                .module()
                .resolve_stmt(*statement)
                .expect("assertion statement resolves");
            let HirStmtKind::Assertion { conditions, .. } = resolved.kind() else {
                panic!("statement is the typed assertion payload");
            };
            Some((item.id(), *statement, conditions.to_vec()))
        })
        .expect("Flow assertion fixture");
    assert_eq!(conditions.len(), values.len());

    let mut input = RuntimePlanSemanticFactInput::new();
    for (_, module) in executable.modules() {
        for (owner, _) in module.locals() {
            input
                .push_local_declaration(
                    owner,
                    RuntimeNormalizedType::new(
                        RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                        RuntimeTypeShape::Unit,
                    ),
                )
                .expect("fixture local identity");
        }
        for (owner, _) in module.expressions() {
            input.push_expression_type(
                owner,
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                    RuntimeTypeShape::Unit,
                ),
            );
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
    for (condition, value) in conditions.iter().copied().zip(values.iter().copied()) {
        input.push_expression_literal(condition, RuntimeValue::Bool(value));
    }
    input.push_assertion(statement, admission);
    let facts = RuntimePlanSemanticFacts::try_new(executable, input).expect("checked facts");
    let report = lower_runtime_plan_with_stats(
        executable,
        &facts,
        &RuntimeEntryLoweringInput::empty(executable),
    )
    .expect("runtime assertion fixture lowers");
    let guards = report.plan.flows[0]
        .ops
        .iter()
        .filter_map(|operation| match operation {
            FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert { guard, .. }) => Some(*guard),
            _ => None,
        })
        .collect();
    (report, statement, guards)
}

fn project_fixture(label: &str, source: &str) -> HirProject {
    let package = CallablePackageId::try_new(format!("runtime-plan-assertion-{label}"))
        .expect("fixture package");
    let path = CanonicalModulePath::crate_root();
    let source_name = SourceName::path(format!("runtime-plan-assertion-{label}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-test://runtime-plan/assertion/{label}"))
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
            ParseOptions::default(),
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
        "runtime-plan-assertion-test",
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
    .expect("runtime-plan fixture has no authored Proof return headers");
    let mut outputs = transaction
        .publish_with_semantic_facts(&mut database, facts)
        .expect("final HIR project publishes");
    let module = outputs
        .pop()
        .expect("one runtime-plan fixture module")
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
