use super::support::*;

#[test]
fn multiline_state_defaults_are_structured() {
    let tree = parse_ok(
        r"
pub state GameState {
    pub config: Config = Config {
        text_speed = 1.0f32,
        volume = 0.8f32,
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("multiline state default lowers");
    validate_typecheck_ready(&hir).expect("multiline state default is structured");
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
    let hir = lower_to_hir(&tree).expect("pure function call lowers to HIR");
    validate_typecheck_ready(&hir).expect("pure function call is typecheck ready");
    lower_runtime_plan(&hir).expect("pure function call lowers to runtime plan");
}

#[test]
fn entry_selects_runtime_start_flow() {
    let tree = parse_ok(
        r#"
entry game @entry.main { start @flow.second }
flow @flow.first first { return "wrong" }
flow @flow.second second { return "right" }
"#,
    );
    let hir = lower_to_hir(&tree).expect("entry lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers with explicit entry");
    assert!(
        plan.entry_flow
            .as_ref()
            .is_some_and(|id| id.0 == "flow.second")
    );
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
entry cli @entry.main { run @flow.main }
flow @flow.main main effects { fs.read(save) } {
    let text = try await fs.read_text(path.save("profile.json")) with {
        error e => return "missing"
    }
    return text
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("capability HIR lowers");
    validate_typecheck_ready(&hir).expect("capability calls structured");
}
