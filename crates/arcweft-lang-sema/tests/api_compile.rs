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
