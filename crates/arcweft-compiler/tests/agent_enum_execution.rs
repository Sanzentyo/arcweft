#[path = "support/execution.rs"]
mod execution;

use arcweft_core::value::RuntimeValue;

const CASES: &[(&str, &[&str])] = &[
    ("CaptureFormat", &["png", "raw_rgba"]),
    ("CaptureKind", &["color", "mask"]),
    ("PointerButton", &["primary", "secondary", "middle"]),
    ("AgentBinaryEncoding", &["Base64"]),
];

fn program(ty: &str, selected: &str, cases: &[&str]) -> String {
    let arms = cases
        .iter()
        .enumerate()
        .map(|(ordinal, name)| format!(".{name} => {ordinal}i64,"))
        .collect::<String>();
    format!(
        "entry cli @entry.main {{ goto @flow.main }}\n\
         fn case_index(value: {ty}) -> i64 {{ match value {{ {arms} }} }}\n\
         flow main() -> i64 {{ return case_index(.{selected}) }}"
    )
}

#[test]
fn agent_enum_cases_match_after_native_function_arguments() {
    for &(ty, cases) in CASES {
        for (ordinal, case) in cases.iter().enumerate() {
            execution::assert_native_return(&program(ty, case, cases), &ordinal.to_string());
        }
    }
}

#[test]
fn agent_enum_cases_match_after_awbc_function_arguments() {
    for &(ty, cases) in CASES {
        for (ordinal, case) in cases.iter().enumerate() {
            execution::assert_awbc_return(
                &program(ty, case, cases),
                RuntimeValue::i64(i64::try_from(ordinal).unwrap()),
            );
        }
    }
}

#[test]
fn agent_resource_body_text_payload_matches_after_native_and_awbc_arguments() {
    let source = r#"
entry cli @entry.main { goto @flow.main }
fn inspect_body(value: AgentResourceBody) -> i64 {
    match value {
        .Json(_) => 0i64,
        .Text(text) => if text == "payload" { 17i64 } else { -1i64 },
        .BytesBase64(_) => 2i64,
    }
}
flow main() -> i64 { return inspect_body(.Text("payload")) }
"#;
    execution::assert_native_return(source, "17");
    execution::assert_awbc_return(source, RuntimeValue::i64(17));
}
