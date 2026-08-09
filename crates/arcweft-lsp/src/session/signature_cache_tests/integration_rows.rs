use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

use arcweft_character::{
    id::CharacterId, manifest::registration::SourceBackedCharacterManifest,
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::{
    database::HirDatabase,
    lowering::{HirModuleKey, LoweringRequest},
    project::{HirProjectBuilder, HirProjectModule},
    proof_return::HirProofReturnSemanticFactSet,
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    callable::{
        CallableCandidateId, CallableGroupKind, CallableName, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallableQueryLimitError,
        CapacityMethodId, PresentationCallableId,
    },
    env::TypeCheckEnv,
    final_analysis::{FinalSemanticAnalysisControl, FinalSemanticCatalogs, analyze_final_project},
    registration::{CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts},
    signature::{
        SignatureNotApplicable, SignatureQuery, SignatureQueryControl, SignatureQueryError,
        SignatureQueryOutcome, query_signature,
    },
    types::TypeKind,
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase};
use arcweft_launch::ProfileId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};
use lsp_types::{
    ParameterLabel, SignatureHelp, SignatureHelpParams, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkDoneProgressParams,
};

use super::*;
use crate::{
    profiles::{
        accepted_build_work_snapshot_for_test, accepted_project::AcceptedProjectSnapshot,
        state::AcceptedProfileKey,
    },
    requests::{
        RequestControl,
        signature::{SignatureRequestError, SignatureRequestStale},
    },
    session::tests::position_of,
    uri_key::LspUriKey,
};

fn params(uri: lsp_types::Uri, position: Position) -> SignatureHelpParams {
    SignatureHelpParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        context: None,
    }
}

#[test]
fn cache_miss_uses_the_exact_accepted_query_tuple_without_compiler_work() {
    let fixture = SignatureCacheFixture::new("lsp-signature-exact-query-tuple");
    let accepted = fixture.accepted();
    let before = accepted_build_work_snapshot_for_test();
    let prepared = fixture.prepare(80, position_after(SOURCE, "sum(value,"));
    let work = fixture
        .session
        .read()
        .expect("session read")
        .signature_work(&prepared)
        .expect("exact accepted cache lookup");
    let crate::requests::signature::SignatureRequestWork::Miss(key) = work else {
        panic!("fresh accepted generation must miss")
    };
    let stamp = prepared.stamp();
    let lease = prepared.lease();
    let exact_hir = stamp
        .project()
        .hir(stamp.module())
        .expect("stamped accepted HIR");

    assert!(Arc::ptr_eq(stamp.accepted(), &accepted));
    assert!(Arc::ptr_eq(stamp.project(), accepted.project()));
    assert!(Arc::ptr_eq(
        stamp.hir_project(),
        accepted.project().hir_project()
    ));
    let accepted_world = accepted
        .executable()
        .expect("accepted executable")
        .registered_world();
    assert!(std::ptr::eq(stamp.world().as_ref(), accepted_world));
    assert!(std::ptr::eq(lease.world(), stamp.world().as_ref()));
    assert!(std::ptr::eq(
        lease.document(),
        stamp.accepted_document().as_ref()
    ));
    assert!(std::ptr::eq(
        lease.hir().expect("lease HIR"),
        exact_hir.as_ref()
    ));
    assert_eq!(
        lease.document().identity(),
        stamp.accepted_document_identity()
    );
    assert_eq!(
        exact_hir.provenance().source_identity(),
        stamp.accepted_document_identity()
    );
    assert_eq!(
        key.byte_offset(),
        prepared
            .snapshot()
            .line_index()
            .try_byte_offset_from_position(prepared.position())
            .expect("exact protocol position")
    );

    let result = ArcweftLspSession::compute_signature(&prepared, key)
        .expect("native semantic signature query");
    assert!(matches!(
        result.outcome().as_ref(),
        SignatureQueryOutcome::Help(_)
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);

    let cancelled = fixture.prepare(81, position_after(SOURCE, "sum(value,"));
    let work = fixture
        .session
        .read()
        .expect("session read")
        .signature_work(&cancelled)
        .expect("second cache miss");
    let crate::requests::signature::SignatureRequestWork::Miss(key) = work else {
        panic!("unpublished first result leaves a miss")
    };
    fixture
        .runtime
        .as_ref()
        .expect("request runtime")
        .registry()
        .cancel(
            cancelled.request_id(),
            SignatureCancellationReason::ClientCancelled,
        );
    assert!(matches!(
        ArcweftLspSession::compute_signature(&cancelled, key),
        Err(SignatureRequestError::Query(SignatureQueryError::Cancelled))
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
}

#[test]
fn parser_owned_argument_range_is_the_only_successful_carrier() {
    let fixture = SignatureCacheFixture::new("lsp-signature-parser-owned-carrier");
    let before = accepted_build_work_snapshot_for_test();

    let inside = fixture.prepare(82, position_after(SOURCE, "sum(value,"));
    let result = fixture.execute(&inside).expect("inside argument list");
    assert!(matches!(
        result.outcome().as_ref(),
        SignatureQueryOutcome::Help(_)
    ));

    let outside = fixture.prepare(83, position_of(SOURCE, "sum(value"));
    let result = fixture.execute(&outside).expect("outside argument list");
    assert_eq!(
        result.outcome().as_ref(),
        &SignatureQueryOutcome::NotApplicable(SignatureNotApplicable::CursorOutsideArgumentList,)
    );
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
}

#[test]
fn cache_hit_preserves_the_exact_native_result_and_wire_projection() {
    let fixture = SignatureCacheFixture::new("lsp-signature-exact-cache-semantics");
    let accepted = fixture.accepted();
    let before = accepted_build_work_snapshot_for_test();
    let first = fixture.prepare(86, position_after(SOURCE, "sum(value,"));
    let first_result = fixture.execute(&first).expect("native cache miss result");
    let expected = Arc::clone(first_result.outcome());
    let first_response = fixture.publish(&first, Ok(first_result));
    assert!(first_response.error.is_none(), "{:?}", first_response.error);
    drop(first);

    let second = fixture.prepare(87, position_after(SOURCE, "sum(value,"));
    let hit = fixture.execute(&second).expect("exact native cache hit");
    assert!(Arc::ptr_eq(hit.outcome(), &expected));
    let second_response = fixture.publish(&second, Ok(hit));
    assert!(
        second_response.error.is_none(),
        "{:?}",
        second_response.error
    );
    assert_eq!(second_response.result, first_response.result);
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
    let cache = accepted.signature_cache_snapshot_for_test();
    assert_eq!(cache.entries, 1);
    assert_eq!(cache.hits, 1);
}

#[test]
fn exhausted_native_query_work_publishes_no_help_or_cache_state() {
    let fixture = SignatureCacheFixture::new("lsp-signature-exhausted-native-query-work");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(88, position_after(SOURCE, "sum(value,"));
    let before = accepted.signature_cache_snapshot_for_test();

    let response = fixture.publish(
        &prepared,
        Err(SignatureRequestError::Query(
            SignatureQueryError::CallableLimitExceeded(CallableQueryLimitError::Work {
                requested: 1,
                consumed: 3,
                limit: 3,
            }),
        )),
    );

    assert!(response.result.is_none());
    let error = response.error.expect("bounded-work failure response");
    assert_eq!(error.code, lsp_server::ErrorCode::ServerCancelled as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.query.limit_exceeded"
        }))
    );
    assert_eq!(accepted.signature_cache_snapshot_for_test(), before);
}

fn assert_capacity_native_signature(
    outcome: &SignatureQueryOutcome,
    expected_candidate: &CallableCandidateId,
    slot: usize,
) -> SignatureHelp {
    let SignatureQueryOutcome::Help(native_help) = outcome else {
        panic!("argument slot {slot} must produce native semantic help")
    };
    assert_eq!(native_help.active_signature().get(), 0);
    let active = native_help
        .active_parameter()
        .expect("each authored argument has an active parameter");
    assert_eq!(active.group().get(), 0);
    assert_eq!(active.parameter().get(), 0);

    let [native_signature] = native_help.signatures() else {
        panic!("Capacity must project exactly one semantic signature")
    };
    assert_eq!(native_signature.candidate(), expected_candidate);
    assert_eq!(native_signature.authored_callee(), "String.with_capacity");
    assert_eq!(native_signature.result(), &TypeKind::String);
    let CallableCandidateId::CapacityMethod(capacity) = native_signature.candidate() else {
        panic!("semantic signature must retain CapacityMethod identity")
    };
    assert_eq!(capacity.receiver(), &TypeKind::String);
    assert_eq!(capacity.arity(), 3);

    let [group] = native_signature.groups() else {
        panic!("Capacity must retain one semantic parameter group")
    };
    assert_eq!(group.index().get(), 0);
    assert_eq!(group.kind(), CallableGroupKind::Initial);
    let [parameter] = group.parameters() else {
        panic!("Capacity must retain one unchecked rest parameter")
    };
    assert_eq!(parameter.coordinate(), active);
    assert_eq!(parameter.label(), "...args: _?");
    assert_eq!(parameter.name().map(CallableName::as_str), Some("args"));
    assert_eq!(parameter.ty(), &CallableParameterType::Unchecked);
    assert_eq!(
        parameter.passing(),
        CallableParameterPassing::RestPositional
    );
    assert_eq!(parameter.presence(), CallableParameterPresence::Optional);

    crate::features::signature::signature_help(outcome)
        .expect("native semantic help projects without lookup")
        .expect("Capacity signature is applicable")
}

fn assert_capacity_lsp_projection(wire: &SignatureHelp, native_projection: &SignatureHelp) {
    assert_eq!(wire, native_projection);
    assert_eq!(wire.active_signature, Some(0));
    assert_eq!(wire.active_parameter, Some(0));
    let [wire_signature] = wire.signatures.as_slice() else {
        panic!("LSP must expose exactly one Capacity signature")
    };
    assert_eq!(
        wire_signature.label,
        "String.with_capacity(...args: _?) -> String"
    );
    let Some([wire_parameter]) = wire_signature.parameters.as_deref() else {
        panic!("LSP must expose exactly one Capacity parameter")
    };
    assert_eq!(wire_parameter.label, ParameterLabel::LabelOffsets([21, 32]));
}

#[expect(
    clippy::too_many_lines,
    reason = "the signature matrix fixture builds one exact native semantic authority transaction"
)]
fn final_native_outcomes(
    source: &str,
    query_needles: &[&str],
    supporting_documents: Vec<Arc<SourceDocument>>,
    character_catalogs: Vec<SourceBackedCharacterCatalog>,
) -> Vec<SignatureQueryOutcome> {
    let package = CallablePackageId::try_new("lsp-signature-matrix-tests").expect("package");
    let path = CanonicalModulePath::crate_root();
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://lsp/signature-matrix").expect("source ID"),
            SourceName::path("signature-matrix.arcw"),
            source,
        )
        .expect("source document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(&document),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("parsed signature source");
    let world_id =
        ProjectSymbolWorldId::try_new(package.clone(), document.identity().id().clone(), "test")
            .expect("symbol world");
    let mut documents = vec![Arc::clone(&document)];
    documents.extend(supporting_documents);
    let facts = ProjectRegistrationFacts::try_new(
        world_id.clone(),
        documents,
        Vec::new(),
        character_catalogs,
        Vec::new(),
    )
    .expect("registration facts");
    let mut hir_database = HirDatabase::try_new().expect("HIR database");
    let transaction = hir_database
        .stage_proof_return_project(
            [LoweringRequest::try_new(
                HirModuleKey::new(package.clone(), path.clone(), document.identity().clone()),
                &parsed,
            )
            .expect("lowering request")],
            world_id,
            *facts.symbol_revision(),
            facts.documents().map(|document| document.identity()),
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .expect("staged HIR project");
    let semantic_facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("no Proof returns");
    let module = transaction
        .publish_with_semantic_facts(&mut hir_database, semantic_facts)
        .expect("published HIR project")
        .into_iter()
        .next()
        .expect("root HIR module")
        .into_module();
    let project_module = HirProjectModule::try_new(
        &hir_database,
        &package,
        &path,
        module.provenance().source_identity(),
        Arc::clone(&module),
    )
    .expect("project module");
    let mut builder = HirProjectBuilder::new(&hir_database, package);
    builder
        .insert_module(project_module)
        .expect("module insertion");
    let project = builder.finish().expect("module-preserving HIR project");
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        project.view(),
        &facts,
        None,
    ))
    .expect("registered semantic world");
    let cancellation = AtomicBool::new(false);
    let analysis = analyze_final_project(
        project.executable_view().expect("executable HIR"),
        registered.symbols(),
        FinalSemanticCatalogs::production(&registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("final semantic signature analysis");

    query_needles
        .iter()
        .map(|argument| {
            let byte_offset = source.find(*argument).expect("authored query argument") + 2;
            query_signature(
                SignatureQuery::production(
                    &registered,
                    &document,
                    &module,
                    &analysis,
                    byte_offset,
                    SignatureQueryControl::new(&cancellation, None),
                )
                .expect("exact final-sema signature query"),
            )
            .expect("native signature outcome")
        })
        .collect()
}

#[test]
fn associated_capacity_native_lsp_projection_parity() {
    const CAPACITY_SOURCE: &str = "fn allocate() -> String {\n\
    String.with_capacity(1usize, 2usize, 3usize)\n\
}\n";
    let expected_candidate = CallableCandidateId::CapacityMethod(
        CapacityMethodId::try_new(
            TypeKind::String,
            CallableName::try_new("with_capacity").expect("capacity method name"),
            3,
        )
        .expect("capacity candidate identity"),
    );
    for (slot, outcome) in final_native_outcomes(
        CAPACITY_SOURCE,
        &["1usize", "2usize", "3usize"],
        Vec::new(),
        Vec::new(),
    )
    .into_iter()
    .enumerate()
    {
        let native_projection =
            assert_capacity_native_signature(&outcome, &expected_candidate, slot);
        let wire = crate::features::signature::signature_help(&outcome)
            .expect("typed LSP projection")
            .expect("Capacity signature is applicable");
        assert_capacity_lsp_projection(&wire, &native_projection);
    }
}

#[test]
fn character_nominal_show_native_lsp_projection_parity() {
    const SOURCE: &str = concat!(
        "pub character @character.akane Akane as akane {}\n",
        "fn caller() { show(@character.akane, look = .normal); }\n",
    );
    const MANIFEST: &str = r#"{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.akane",
  "canvas": { "width": 64, "height": 128 },
  "anchor": { "x": 32, "y": 128 },
  "default_look": "normal",
  "parts": [{
    "id": "body",
    "z": 0,
    "variants": [{
      "id": "default",
      "asset": "layers/body.png",
      "rect": { "x": 0, "y": 0, "width": 64, "height": 128 },
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }]
  }],
  "looks": [{
    "id": "normal",
    "select": [{ "part": "body", "variant": "default" }]
  }]
}"#;
    let manifest_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://lsp/signature-character")
                .expect("manifest source ID"),
            SourceName::path("character-akane.json"),
            MANIFEST,
        )
        .expect("manifest source"),
    );
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&manifest_document)
        .expect("source-backed Character manifest");
    let catalog =
        SourceBackedCharacterCatalog::try_new(manifest_document.identity().clone(), vec![manifest])
            .expect("source-backed Character catalog");
    let mut outcomes =
        final_native_outcomes(SOURCE, &[".normal"], vec![manifest_document], vec![catalog]);
    let outcome = outcomes.pop().expect("one Character signature outcome");
    let SignatureQueryOutcome::Help(help) = &outcome else {
        panic!("Character look cursor must produce native help")
    };
    assert_eq!(help.active_signature().get(), 0);
    let active = help.active_parameter().expect("active look parameter");
    assert_eq!(active.group().get(), 0);
    assert_eq!(active.parameter().get(), 1);
    let [native_signature] = help.signatures() else {
        panic!("one Character presentation signature")
    };
    assert_eq!(
        native_signature.candidate(),
        &CallableCandidateId::Presentation(PresentationCallableId::Show)
    );
    let [group] = native_signature.groups() else {
        panic!("one Character presentation parameter group")
    };
    assert_eq!(
        group.parameters()[1].ty(),
        &CallableParameterType::Exact(TypeKind::character_look(
            CharacterId::try_new("character.akane").expect("Character ID"),
        ))
    );

    let wire = crate::features::signature::signature_help(&outcome)
        .expect("typed LSP projection")
        .expect("Character signature is applicable");
    assert_eq!(wire.active_signature, Some(0));
    assert_eq!(wire.active_parameter, Some(1));
    let [wire_signature] = wire.signatures.as_slice() else {
        panic!("one LSP Character signature")
    };
    assert!(
        wire_signature
            .label
            .contains("look: CharacterLook<character.akane>?"),
        "typed Character nominal was lost from LSP label: {}",
        wire_signature.label,
    );
    let parameters = wire_signature.parameters.as_ref().expect("LSP parameters");
    assert_eq!(parameters.len(), group.parameters().len());
    assert!(matches!(
        parameters[1].label,
        ParameterLabel::LabelOffsets(_)
    ));
}

#[test]
fn acquisition_failure_returns_directly_without_build_or_fallback() {
    let fixture = SignatureCacheFixture::new("lsp-signature-no-acquisition-fallback");
    let accepted = fixture.accepted();
    let accepted_cache = accepted.signature_cache_snapshot_for_test();
    let before = accepted_build_work_snapshot_for_test();
    let missing_uri = "file:///not-open-and-not-mapped.arcw"
        .parse()
        .expect("missing URI");

    let result = fixture
        .session
        .read()
        .expect("session read")
        .prepare_signature_request(
            lsp_server::RequestId::from(84),
            params(missing_uri, Position::new(0, 0)),
            fixture
                .runtime
                .as_ref()
                .expect("request runtime")
                .registry(),
        );

    assert!(matches!(
        result,
        Err(SignatureAcquireError::DocumentNotOpen { .. })
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
    let retained = fixture.accepted();
    assert!(Arc::ptr_eq(&retained, &accepted));
    assert_eq!(retained.signature_cache_snapshot_for_test(), accepted_cache);
}

#[test]
fn worker_transfer_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Arc<RwLock<ArcweftLspSession>>>();
    assert_send_sync::<AcceptedProjectSnapshot>();
    assert_send_sync::<RequestControl>();
    assert_send_sync::<PreparedSignatureRequest>();
}

#[test]
fn typed_uri_and_profile_keys_preserve_exact_lookup_and_remap_rejection() {
    let fixture = SignatureCacheFixture::new("lsp-signature-typed-uri-profile");
    let prepared = fixture.prepare(85, position_after(SOURCE, "sum(value,"));
    let before = accepted_build_work_snapshot_for_test();
    let exact_uri = LspUriKey::from_uri(&fixture.uri);
    let distinct_uri = "file:///definitely-distinct.arcw"
        .parse()
        .expect("distinct URI");
    let distinct_uri = LspUriKey::from_uri(&distinct_uri);
    let stamped = prepared.stamp().profile().clone();
    let alternate = AcceptedProfileKey::new(
        &stamped.workspace_key().to_uri(),
        &stamped.manifest_key().to_uri(),
        ProfileId::new("typed-other").expect("alternate profile ID"),
    );
    let mut typed = BTreeMap::new();
    typed.insert(exact_uri.clone(), stamped.clone());

    assert_eq!(typed.get(&exact_uri), Some(&stamped));
    assert_eq!(typed.get(&distinct_uri), None);
    assert_ne!(stamped, alternate);

    fixture
        .session
        .write()
        .expect("session write")
        .profile_keys_by_uri
        .insert(exact_uri, alternate.clone());
    assert!(matches!(
        fixture
            .session
            .read()
            .expect("session read")
            .signature_work(&prepared),
        Err(SignatureRequestError::Stale(
            SignatureRequestStale::ProfileRemapped {
                expected,
                actual: Some(actual),
            }
        )) if expected == stamped && actual == alternate
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
}
