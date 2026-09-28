use std::sync::{Arc, atomic::AtomicBool};

use arcweft_lang_hir::{
    database::HirDatabase,
    expr::HirExprKind,
    identity::ExprId,
    lowering::{HirLoweringControl, HirModuleKey, LoweringRequest},
    project::{HirProject, HirProjectBuilder, HirProjectModule},
    proof_return::HirProofReturnSemanticFactSet,
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    final_analysis::{
        CheckedGuardSemantic, CheckedMatchLimitKind, CheckedMatchLimits, CheckedMatchQueryError,
        CheckedMatchWitnessKind, CheckedMatchWitnessView, CheckedPatternCoordinateStep,
        CheckedUnreachableReason, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
        FinalSemanticAnalysisError, FinalSemanticCatalogs, analyze_final_project,
    },
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        RegisteredSemanticWorld,
    },
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};

struct Fixture {
    project: HirProject,
    registered: RegisteredSemanticWorld,
}

fn fixture(source: &str) -> Fixture {
    let package = CallablePackageId::try_new("checked-match-query-tests").expect("package");
    let path = CanonicalModulePath::crate_root();
    let name = SourceName::path("root.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://sema/checked-match-query")
                .expect("document ID"),
            name.clone(),
            source,
        )
        .expect("document"),
    );
    let parsed = SyntaxDatabase::try_new()
        .expect("syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("parsed module");
    let world =
        ProjectSymbolWorldId::try_new(package.clone(), document.identity().id().clone(), "test")
            .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world.clone(),
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let mut database = HirDatabase::try_new().expect("HIR database");
    let transaction = database
        .stage_proof_return_project(
            [LoweringRequest::try_new(
                HirModuleKey::new(package.clone(), path.clone(), document.identity().clone()),
                &parsed,
            )
            .expect("lowering request")],
            world,
            *facts.symbol_revision(),
            facts.documents().map(|document| document.identity()),
            HirLoweringControl::new(),
        )
        .expect("staged HIR");
    let semantic_facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("proof-return facts");
    let module = transaction
        .publish_with_semantic_facts(&mut database, semantic_facts)
        .expect("published HIR")
        .into_iter()
        .next()
        .expect("root module")
        .into_module();
    let project_module = HirProjectModule::try_new(
        &database,
        &package,
        &path,
        module.provenance().source_identity(),
        Arc::clone(&module),
    )
    .expect("project module");
    let mut builder = HirProjectBuilder::new(&database, package);
    builder
        .insert_module(project_module)
        .expect("insert module");
    let project = builder.finish().expect("HIR project");
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        project.view(),
        &facts,
        None,
    ))
    .expect("registered world");
    Fixture {
        project,
        registered,
    }
}

fn analyze(fixture: &Fixture) -> FinalSemanticAnalysis {
    let cancelled = AtomicBool::new(false);
    analyze_final_project(
        fixture.project.analysis_view().expect("executable HIR"),
        fixture.registered.symbols(),
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancelled),
    )
    .expect("checked final analysis")
}

fn match_owner(fixture: &Fixture) -> ExprId {
    fixture
        .project
        .analysis_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module")
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression")
}

#[test]
fn public_checked_match_query_exposes_only_complete_read_only_facts() {
    let fixture = fixture(
        "fn root(flag: bool) -> i64 {\n    match flag {\n        true when true => 1i64\n        false => 2i64\n    }\n}\n",
    );
    let report = analyze(&fixture);
    let product = report
        .checked_match(
            fixture.project.analysis_view().expect("executable HIR"),
            fixture.registered.symbols(),
            match_owner(&fixture),
            CheckedMatchLimits::PRODUCTION,
        )
        .expect("complete exhaustive Match");
    assert_eq!(product.root(), product.path().root());
    assert!(product.callable_owner().is_some());
    assert_eq!(product.transcript().version(), 1);
    assert!(product.transcript().byte_len() > 0);
    assert_eq!(product.semantic_digest(), product.transcript().digest());
    assert!(product.coverage().exhaustive());
    assert!(
        product
            .coverage()
            .observed_work(CheckedMatchLimitKind::Arms)
            >= 2
    );
    assert_eq!(product.arms().len(), 2);
    assert_eq!(product.arms()[0].coordinate().ordinal(), 0);
    assert!(matches!(
        product.arms()[0].guard(),
        CheckedGuardSemantic::ConstantTrue(_)
    ));
    assert!(matches!(
        product.arms()[1].guard(),
        CheckedGuardSemantic::Absent
    ));
}

#[test]
fn public_checked_match_query_rejects_stale_project_generation() {
    let original = fixture("fn root(flag: bool) -> i64 { match flag { _ => 1i64 } }\n");
    let changed = fixture("fn root(flag: bool) -> i64 { match flag { _ => 2i64 } }\n");
    let report = analyze(&original);
    let result = report.checked_match(
        changed.project.analysis_view().expect("executable HIR"),
        changed.registered.symbols(),
        match_owner(&changed),
        CheckedMatchLimits::PRODUCTION,
    );
    assert!(matches!(
        result,
        Err(CheckedMatchQueryError::Generation(
            FinalSemanticAnalysisError::GenerationMismatch
                | FinalSemanticAnalysisError::SymbolGenerationMismatch
        ))
    ));
}

#[test]
fn public_checked_match_query_returns_typed_nonexhaustive_witness() {
    let fixture = fixture("fn root(flag: bool) -> i64 { match flag { true => 1i64 } }\n");
    let report = analyze(&fixture);
    let result = report.checked_match(
        fixture.project.analysis_view().expect("executable HIR"),
        fixture.registered.symbols(),
        match_owner(&fixture),
        CheckedMatchLimits::PRODUCTION,
    );
    assert!(matches!(
        result,
        Err(CheckedMatchQueryError::NonExhaustive { witness })
            if witness.kind() == CheckedMatchWitnessKind::Bool && witness.boolean() == Some(false)
    ));
}

#[test]
fn public_checked_match_witness_exposes_nested_record_children() {
    let fixture = fixture(
        "struct Pair { first: bool, second: bool }\nfn root(value: Pair) -> i64 { match value {} }\n",
    );
    let report = analyze(&fixture);
    let result = report.checked_match(
        fixture.project.analysis_view().expect("executable HIR"),
        fixture.registered.symbols(),
        match_owner(&fixture),
        CheckedMatchLimits::PRODUCTION,
    );
    let Err(CheckedMatchQueryError::NonExhaustive { witness }) = result else {
        panic!("record domain must have a structured witness: {result:?}");
    };
    let CheckedMatchWitnessView::Record { owner, fields } = witness.view() else {
        panic!("record witness expected: {witness:?}");
    };
    assert_ne!(owner.as_bytes(), &[0; 32]);
    assert_eq!(fields.len(), 2);
    assert!(
        fields
            .iter()
            .all(|field| matches!(field.view(), CheckedMatchWitnessView::Bool(false)))
    );
}

#[test]
fn public_checked_match_coverage_exposes_precise_or_alternative_path() {
    let fixture = fixture(
        "fn root(pair: (bool, bool)) -> i64 {\n    match pair {\n        (true | false, true | true) => 1i64\n        _ => 0i64\n    }\n}\n",
    );
    let report = analyze(&fixture);
    let product = report
        .checked_match(
            fixture.project.analysis_view().expect("executable HIR"),
            fixture.registered.symbols(),
            match_owner(&fixture),
            CheckedMatchLimits::PRODUCTION,
        )
        .expect("exhaustive product");
    let redundant = product
        .coverage()
        .unreachable()
        .iter()
        .find(|row| row.reason() == CheckedUnreachableReason::CoveredByEarlierOrAlternative)
        .expect("redundant Or alternative");
    let steps = redundant
        .alternative_view()
        .expect("precise alternative path")
        .steps()
        .collect::<Vec<_>>();
    assert_eq!(
        steps,
        [
            CheckedPatternCoordinateStep::TupleElement(1),
            CheckedPatternCoordinateStep::OrAlternative(1),
        ]
    );
}
