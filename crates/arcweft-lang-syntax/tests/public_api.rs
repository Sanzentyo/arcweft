#[test]
fn removed_public_api_contract() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_borrow_block.rs");
    cases.compile_fail("tests/ui/removed_line_plan_assert.rs");
    cases.compile_fail("tests/ui/capability_policy_absent.rs");
    cases.compile_fail("tests/ui/removed_role_items.rs");
    cases.compile_fail("tests/ui/parse_error_construction.rs");
    cases.compile_fail("tests/ui/try_expr_construction.rs");
    cases.compile_fail("tests/ui/removed_asset_declaration_kind.rs");
}
