#[test]
fn removed_and_session_identity_public_api_contract() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/capability_policy_absent.rs");
    cases.compile_fail("tests/ui/internal_lowering_modules_private.rs");
    cases.compile_fail("tests/ui/lower_to_hir_removed.rs");
    cases.compile_fail("tests/ui/standalone_hir_assertion_removed.rs");
    cases.compile_fail("tests/ui/removed_unused_hir_accessors.rs");
    cases.compile_fail("tests/ui/removed_hir_borrow.rs");
    cases.compile_fail("tests/ui/removed_reference_hir.rs");
    cases.compile_fail("tests/ui/removed_role_hir.rs");
    cases.compile_fail("tests/ui/removed_syntax_forwarding_facade.rs");
    cases.compile_fail("tests/ui/hir_module_fields_private.rs");
    cases.compile_fail("tests/ui/raw_hir_id_view_slot_private.rs");
    cases.compile_fail("tests/ui/session_identity_raw_constructor.rs");
    cases.compile_fail("tests/ui/session_identity_serde.rs");
}
