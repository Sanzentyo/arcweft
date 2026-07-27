#[test]
fn removed_character_classifier_and_mutation_bypasses_do_not_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/character_nominal_*.rs");
    cases.compile_fail("tests/ui/typecheck_env_character_builder_removed.rs");
}

#[test]
fn runtime_manifest_cannot_enter_registration_without_source_provenance() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/bare_manifest_not_registerable.rs");
    cases.compile_fail("tests/ui/character_manifest_from_json_removed.rs");
}

#[test]
fn descriptor_has_no_persisted_codec() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/character_descriptor_not_serializable.rs");
}

#[test]
fn capability_policy_has_no_mutable_semantic_record() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/capability_policy_absent.rs");
}

#[test]
fn signature_query_identity_does_not_accept_incremental_snapshot_ids() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/signature_query_source_snapshot_id.rs");
    cases.compile_fail("tests/ui/source_snapshot_document_identity_conversion.rs");
}

#[test]
fn associated_capacity_no_runtime_receiver_injection() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/associated_type_receiver_requires_nominal_proof.rs");
    cases.compile_fail("tests/ui/resolved_callable_constructor_is_internal.rs");
}

#[test]
fn removed_zero_consumer_type_helpers_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_zero_consumer_type_kind_helpers.rs");
}

#[test]
fn removed_zero_consumer_nominal_index_helper_is_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_zero_consumer_nominal_index_helper.rs");
}
