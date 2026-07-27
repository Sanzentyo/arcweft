use super::support::*;

#[test]
fn multiline_struct_fields_are_structured() {
    let tree = parse_ok(
        r"
pub struct GameState {
    pub config: Config
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("multiline struct field lowers");
    validate_typecheck_ready(&hir).expect("multiline struct field is structured");
}

#[test]
fn pure_function_call_is_runtime_value_expression() {
    let tree = parse_ok(
        r#"
#[pure]
fn add(a: i32, b: i32) -> i32 { a + b }

flow @flow.main main {
    let n = add(1, 2)
    return "done"
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("pure function call lowers to HIR");
    assert!(hir.functions()[0].has_attribute("pure"));
    validate_typecheck_ready(&hir).expect("pure function call is typecheck ready");
}

#[test]
fn entry_selects_runtime_goto_flow() {
    let tree = parse_ok(
        r#"
entry cli @entry.main { goto @flow.second }
flow @flow.first first { return "wrong" }
flow @flow.second second { return "right" }
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree()).expect("entry lowers");
    validate_typecheck_ready(&hir).expect("explicit entry is typecheck ready");
}

#[test]
fn capability_file_read_is_need_request_not_string_call() {
    let tree = parse_ok(
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
}
extern capability path { fn save(path: String) -> VirtualPath }
entry cli @entry.main { goto @flow.main }
flow @flow.main main effects { fs.read(save) } {
    let text = try await fs.read_text(path.save("profile.json")) with {
        error e => return "missing"
    }
    return text
}
"#,
    );
    let hir =
        lower_document_to_hir(tree.document(), tree.typed_tree()).expect("capability HIR lowers");
    validate_typecheck_ready(&hir).expect("capability calls structured");
}
