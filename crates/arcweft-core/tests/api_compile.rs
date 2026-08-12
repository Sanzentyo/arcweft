#[test]
fn core_identity_boundaries_are_compile_time_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/core_cannot_name_hir_ids.rs");
    cases.compile_fail("tests/ui/prove_is_not_runtime_assertion_mode.rs");
    cases.compile_fail("tests/ui/runtime_ownership_raw_identity_constructors_are_private.rs");
    cases.compile_fail("tests/ui/runtime_nominal_record_field_expr_is_private.rs");
    cases.compile_fail("tests/ui/runtime_record_carriers_are_private.rs");
    cases.compile_fail("tests/ui/record_seq_new_is_absent.rs");
    cases.compile_fail("tests/ui/record_columns_rejects_raw_carriers.rs");
}
