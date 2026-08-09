#[test]
fn removed_public_api_contract() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_borrow_block.rs");
    cases.compile_fail("tests/ui/removed_line_plan_assert.rs");
    cases.compile_fail("tests/ui/capability_policy_absent.rs");
    cases.compile_fail("tests/ui/removed_role_items.rs");
    cases.compile_fail("tests/ui/parse_error_construction.rs");
    cases.compile_fail("tests/ui/removed_parse_expr.rs");
    cases.compile_fail("tests/ui/removed_parse_type_ref.rs");
    cases.compile_fail("tests/ui/try_expr_construction.rs");
    cases.compile_fail("tests/ui/removed_asset_declaration_kind.rs");
    cases.compile_fail("tests/ui/removed_extern_mod_item.rs");
    cases.compile_fail("tests/ui/removed_parse_source.rs");
    cases.compile_fail("tests/ui/removed_into_typed_tree.rs");
    cases.compile_fail("tests/ui/removed_parsed_source_tree.rs");
    cases.compile_fail("tests/ui/removed_typed_syntax_tree.rs");
    cases.compile_fail("tests/ui/unbound_fragment_is_not_parsed_source.rs");
    cases.compile_fail("tests/ui/attached_fragment_is_not_source_file.rs");
    cases.compile_fail("tests/ui/syntax_node_id_has_no_raw_constructor.rs");
    cases.compile_fail("tests/ui/syntax_session_ids_are_not_serde.rs");
    cases.compile_fail("tests/ui/typed_node_constructor_is_private.rs");
    cases.compile_fail("tests/ui/removed_unused_syntax_facades.rs");
    cases.compile_fail("tests/ui/removed_zero_consumer_syntax_accessors.rs");
    cases.compile_fail("tests/ui/removed_detached_call_surface.rs");
    cases.compile_fail("tests/ui/attached_flow_fields_private.rs");
}
