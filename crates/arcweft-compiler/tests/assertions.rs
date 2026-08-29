use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_compiler::{
    project::{
        AssertionBuildProfile, CompiledProject, ProjectCompilationContext,
        ProjectCompilationSession, ProjectCompileStage, compile_project,
    },
    source::compile_source,
};
use arcweft_core::{
    effect::{
        RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionGuardId, RuntimeAssertionProfile,
    },
    plan::FlowOp,
};
use arcweft_lang_hir::{
    item::{HirItemKind, HirPredicateBody, HirProofBody},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    assertion::AssertionRuntimePolicy,
    env::TypeCheckEnv,
    final_analysis::{CheckedAssertionDisposition, CheckedItemRole, CheckedStatementPayload},
    registration::ProjectRegistrationFacts,
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::{
    artifact::{ArtifactKeyInput, RuntimePlanArtifactKey},
    fingerprint::BuildDigest,
    incremental::QueryKind,
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};
use arcweft_verify::ProofObligationKind;

#[test]
fn compile_source_rejects_unresolved_prove_before_emitting_a_plan() {
    let error = compile_source(
        r"
flow assertions {
    assert.prove(true, false)
}
",
    )
    .expect_err("undischarged prove assertion blocks code generation");

    let project = error.project();
    assert_eq!(
        project.stage(),
        ProjectCompileStage::RuntimePlanLower.as_str()
    );
    let errors = project.diagnostics();
    assert_eq!(errors.len(), 2);
    for error in errors {
        assert_eq!(
            error
                .diagnostic()
                .code()
                .expect("proof rejection has a stable code")
                .as_str(),
            "verify.proof.unresolved"
        );
    }
    assert!(errors[0].diagnostic().message().contains("condition 0"));
    assert!(errors[1].diagnostic().message().contains("condition 1"));
}

#[test]
fn proof_and_predicate_assertion_context_errors_are_final_sema_diagnostics() {
    for source in [
        "proof invalid() { assert.check(true) }\n",
        "proof invalid() { assert.debug(true) }\n",
        "predicate invalid() { assert.prove(true); true }\n",
    ] {
        let error = compile_source(source)
            .expect_err("invalid assertion context must fail before verification and lowering");
        let project = error.project();
        assert_eq!(project.stage(), ProjectCompileStage::TypeCheck.as_str());
        let [diagnostic] = project.diagnostics() else {
            panic!(
                "one exact assertion-context diagnostic: {:?}",
                project.diagnostics()
            )
        };
        assert_eq!(
            diagnostic
                .diagnostic()
                .code()
                .expect("assertion context has a stable code")
                .as_str(),
            "sema.assert.context"
        );
    }
}

#[test]
fn predicate_and_non_unit_proof_missing_tails_are_exact_semantic_diagnostics() {
    for (source, code) in [
        (
            "predicate missing_boolean() {}\n",
            "sema.predicate.missing_boolean_tail",
        ),
        (
            "proof missing_value() -> i64 { let value: i64 = 1; }\n",
            "sema.proof.missing_value_tail",
        ),
    ] {
        let error = compile_source(source)
            .expect_err("a required Predicate/Proof value tail blocks executable publication");
        let project = error.project();
        assert_eq!(project.stage(), ProjectCompileStage::TypeCheck.as_str());
        let [diagnostic] = project.diagnostics() else {
            panic!(
                "one terminal missing-tail diagnostic without propagated-item duplication: {:?}",
                project.diagnostics()
            )
        };
        assert_eq!(diagnostic.stage(), ProjectCompileStage::TypeCheck);
        assert_eq!(
            diagnostic
                .diagnostic()
                .code()
                .expect("missing tail has a stable semantic code")
                .as_str(),
            code
        );
        let close = source.rfind('}').expect("fixture close delimiter");
        assert_eq!(
            diagnostic
                .diagnostic()
                .span()
                .expect("missing-tail insertion is the primary span")
                .range(),
            SourceRange::new(close, close)
        );
        assert_eq!(diagnostic.diagnostic().labels().len(), 2);
        assert_eq!(
            diagnostic.diagnostic().labels()[0].span().range(),
            SourceRange::new(close, close)
        );
        assert!(
            !diagnostic.diagnostic().labels()[1]
                .span()
                .range()
                .is_empty()
        );

        let tooling = project
            .tooling_lease()
            .expect("semantic recovery retains the exact tooling generation");
        let [module] = tooling.modules() else {
            panic!("single-source fixture retains one compiled module")
        };
        let accepted = tooling
            .hir_project()
            .view()
            .module(module.module())
            .expect("tooling project retains the recovered module");
        assert!(Arc::ptr_eq(module.hir(), accepted));
        assert!(project.diagnostics().iter().all(|candidate| {
            candidate
                .diagnostic()
                .code()
                .is_none_or(|candidate| candidate.as_str() != "hir.project.execution")
        }));
    }
}

#[test]
fn unit_proof_omitted_tail_remains_implicit_unit() {
    compile_source("proof implicit_unit() {}\n")
        .expect("Unit Proof may omit its block tail without recovery");
}

#[test]
fn proof_body_prove_is_admitted_until_the_verifier_requires_discharge() {
    let error = compile_source("proof pending() { assert.prove(true) }\n")
        .expect_err("undischarged Proof-body Prove assertion blocks code generation");
    let project = error.project();
    assert_eq!(
        project.stage(),
        ProjectCompileStage::RuntimePlanLower.as_str()
    );
    let [diagnostic] = project.diagnostics() else {
        panic!(
            "one exact unresolved proof diagnostic: {:?}",
            project.diagnostics()
        )
    };
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .expect("unresolved proof has a stable code")
            .as_str(),
        "verify.proof.unresolved"
    );
}

#[test]
fn verifier_consumes_predicate_proof_arena_records() {
    let compiled = compile_assertion_project(
        concat!(
            "predicate ready(value: bool) = value\n",
            "proof readiness() = ()\n",
            "flow opening {}\n",
        ),
        AssertionBuildProfile::Debug,
    );
    let executable = compiled
        .hir_project()
        .executable_view()
        .expect("compiled project remains executable");
    let mut predicate_owner = None;
    let mut proof_owner = None;
    for item in executable.items() {
        match item.item().kind() {
            HirItemKind::Predicate(predicate) => {
                predicate_owner = Some(item.id());
                let HirPredicateBody::Expression { expression, .. } = predicate.body() else {
                    panic!("fixture Predicate retains its expression body")
                };
                assert!(compiled.final_analysis().expression(*expression).is_some());
            }
            HirItemKind::Proof(proof) => {
                proof_owner = Some(item.id());
                let HirProofBody::Expression { expression, .. } = proof.body() else {
                    panic!("fixture Proof retains its expression body")
                };
                assert!(compiled.final_analysis().expression(*expression).is_some());
            }
            _ => {}
        }
    }
    let predicate_owner = predicate_owner.expect("typed Predicate item");
    let proof_owner = proof_owner.expect("typed Proof item");
    assert!(matches!(
        compiled
            .final_analysis()
            .item(predicate_owner)
            .expect("Predicate semantic fact")
            .role(),
        CheckedItemRole::Predicate
    ));
    assert!(matches!(
        compiled
            .final_analysis()
            .item(proof_owner)
            .expect("Proof semantic fact")
            .role(),
        CheckedItemRole::Proof
    ));

    let [artifact] = compiled.verification().proof_artifacts.as_slice() else {
        panic!(
            "one Proof arena record produces one typed artifact: {:?}",
            compiled.verification().proof_artifacts
        )
    };
    assert_eq!(artifact.item(), proof_owner);
    assert_eq!(artifact.snapshot().module(), proof_owner.module());
    let obligation = compiled
        .verification()
        .obligations
        .iter()
        .find(|obligation| obligation.kind == ProofObligationKind::ProofBody)
        .expect("Proof body obligation");
    assert_eq!(obligation.proof_artifact.as_ref(), Some(artifact));
}

#[test]
fn project_assertion_profile_is_the_only_debug_runtime_admission_owner() {
    let source = r"
flow assertions {
    assert.debug(true)
    assert.check(true)
}
";
    let debug = compile_assertion_project(source, AssertionBuildProfile::Debug);
    let release = compile_assertion_project(source, AssertionBuildProfile::Release);

    assert_eq!(
        debug.assertion_build_profile(),
        AssertionBuildProfile::Debug
    );
    assert_eq!(
        release.assertion_build_profile(),
        AssertionBuildProfile::Release
    );
    assert_eq!(
        assertion_dispositions(&debug),
        vec![
            CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::DebugGuard),
            CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::AlwaysGuard),
        ]
    );
    assert_eq!(
        assertion_dispositions(&release),
        vec![
            CheckedAssertionDisposition::OmittedDebug,
            CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::AlwaysGuard),
        ]
    );
    assert_eq!(debug.runtime_plan().assertion_site_count(), 2);
    assert_eq!(release.runtime_plan().assertion_site_count(), 1);
    assert_eq!(
        assertion_profiles(&debug),
        vec![
            RuntimeAssertionProfile::DebugOnly,
            RuntimeAssertionProfile::Always
        ]
    );
    assert_eq!(
        assertion_profiles(&release),
        vec![RuntimeAssertionProfile::Always]
    );
}

#[test]
fn reloaded_artifact_uses_fresh_inventory_without_old_stmt_equality() {
    let source = "flow assertions { assert.check(true) }\n";
    let old = compile_assertion_project(source, AssertionBuildProfile::Debug);
    let fresh = compile_assertion_project(source, AssertionBuildProfile::Debug);
    let old_guards = assertion_guards(&old);
    let fresh_guards = assertion_guards(&fresh);
    assert_eq!(old_guards, fresh_guards);
    let guard = fresh_guards[0];

    let artifact = runtime_plan_artifact_key();
    let old_context = old
        .execution_diagnostic_context(artifact)
        .expect("old session inventory matches the persisted artifact");
    let fresh_context = fresh
        .execution_diagnostic_context(artifact)
        .expect("fresh session inventory matches the persisted artifact");
    let old_statement = old_context
        .assertions()
        .site(guard)
        .expect("old session assertion site")
        .statement();
    let fresh_statement = fresh_context
        .assertions()
        .site(guard)
        .expect("fresh session assertion site")
        .statement();
    assert_ne!(old_statement, fresh_statement);

    let fault = fresh_context
        .project_assertion_failure(RuntimeAssertionFailure::new(RuntimeAssertion::new(
            guard,
            "false".to_owned(),
            "failed".to_owned(),
            RuntimeAssertionProfile::Always,
        )))
        .expect("persisted failure joins only the fresh inventory");
    assert_eq!(fault.identity().statement(), fresh_statement);
    assert_ne!(fault.identity().statement(), old_statement);
}

fn assertion_dispositions(project: &CompiledProject) -> Vec<CheckedAssertionDisposition> {
    project
        .final_analysis()
        .statements()
        .filter_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::Assertion(disposition) => Some(*disposition),
            _ => None,
        })
        .collect()
}

fn assertion_profiles(project: &CompiledProject) -> Vec<RuntimeAssertionProfile> {
    project
        .runtime_plan()
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .filter_map(|operation| match operation {
            FlowOp::EvaluatedEffect(arcweft_core::effect::RuntimeEffectExpr::Assert {
                profile,
                ..
            }) => Some(*profile),
            _ => None,
        })
        .collect()
}

fn assertion_guards(project: &CompiledProject) -> Vec<RuntimeAssertionGuardId> {
    project
        .runtime_plan()
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .filter_map(|operation| match operation {
            FlowOp::EvaluatedEffect(arcweft_core::effect::RuntimeEffectExpr::Assert {
                guard,
                ..
            }) => Some(*guard),
            _ => None,
        })
        .collect()
}

fn runtime_plan_artifact_key() -> RuntimePlanArtifactKey {
    RuntimePlanArtifactKey::try_derive(&ArtifactKeyInput {
        compiler_build_id: "compiler".to_owned(),
        query: QueryKind::RuntimePlan,
        artifact_kind: QueryKind::RuntimePlan.artifact_kind(),
        target_triple: "native".to_owned(),
        target_features: vec!["base".to_owned()],
        profile: "debug".to_owned(),
        package: "local.arcweft.assertion-admission".to_owned(),
        logical_item: "runtime-plan".to_owned(),
        source_digest: BuildDigest::of(b"flow assertions { assert.check(true) }\n"),
        dependency_interface_digests: Vec::new(),
        dependency_body_digests: Vec::new(),
        adapter_environment_digest: BuildDigest::of(b"adapter"),
        launch_profile_digest: BuildDigest::of(b"launch"),
        declared_environment_digest: BuildDigest::of(b"environment"),
        format_options_digest: BuildDigest::of(b"options"),
    })
    .expect("typed runtime-plan artifact key")
}

fn compile_assertion_project(source: &str, profile: AssertionBuildProfile) -> CompiledProject {
    const PACKAGE: &str = "local.arcweft.assertion-admission";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-source://src/main.arcw").expect("document ID"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let manifest = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-source://arcw.toml").expect("manifest ID"),
            SourceName::path("arcw.toml"),
            "",
        )
        .expect("manifest document"),
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new(PACKAGE).expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::clone(&manifest),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .expect("single-module project");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE).expect("callable package ID"),
        document.identity().id().clone(),
        "assertion-admission",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&manifest), Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    )
    .with_assertion_build_profile(profile);
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("parsed source");
    let parsed_sources = BTreeMap::from([(CanonicalModulePath::crate_root(), parsed)]);
    let mut compiler = ProjectCompilationSession::try_new().expect("HIR database");
    compile_project(&mut compiler, &project, &parsed_sources, &context)
        .expect("assertion profile project compiles")
}
