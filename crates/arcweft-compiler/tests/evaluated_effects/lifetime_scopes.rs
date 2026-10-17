use super::{assert_simple_flow_logs_native_and_awbc, compile_source};
use arcweft_lang_sema::final_analysis::CheckedItemRole;

#[test]
fn lifetime_scoped_function_value_survives_partial_application_in_native_and_awbc() {
    let compiled = compile_source(
        r"
fn scoped(value: i64) -> i64 effects { state.write('flow) } { value + 1i64 }
fn forward(
    run: (i64) -> i64 effects { state.write('flow) }
)(value: i64) -> i64 effects { state.write('flow) } { run(value) }
flow main() effects { state.write('flow), log.write } {
    let transform = scoped
    let prefix = forward(transform)
    log.info(prefix(41i64))
}
entry cli @entry.main { goto @flow.main }
",
    )
    .expect("the typed scoped permission reaches final signatures and executable calls");
    let (_, flow) = compiled
        .analysis
        .final_analysis()
        .items()
        .find(|(_, item)| matches!(item.role(), CheckedItemRole::Flow { .. }))
        .expect("one checked Flow");
    assert_eq!(
        flow.effects().to_labels(),
        ["log.write", "state.write('flow)"]
    );
    assert_simple_flow_logs_native_and_awbc(&compiled, &["42"]);
}
