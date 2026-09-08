use super::{HirDatabase, key, parsed_revisions_with_source, publish_attached_project};

macro_rules! closure_call_case {
    ($name:ident, $source:literal) => {
        #[test]
        fn $name() {
            let (parsed, _) = parsed_revisions_with_source(
                concat!("arcweft-test://closure-call/", stringify!($name)),
                $source,
            );
            let mut database = HirDatabase::try_new().expect("HIR database");
            let accepted = publish_attached_project(&mut database, &parsed, &key(&parsed));
            assert_eq!(
                accepted.module().status(),
                crate::module::HirModuleStatus::Clean
            );
        }
    };
}

closure_call_case!(
    direct_closure_call,
    "flow main() -> i64 { return (|value: i64| value)(42i64) }"
);

closure_call_case!(
    direct_closure_call_with_capture,
    r#"
flow main() -> i64 {
    let mut offset = 1i64
    return (|value: i64| value + offset)(41i64)
}
"#
);

closure_call_case!(
    direct_closure_call_with_block_argument,
    r#"
flow main() -> i64 {
    return (|value: i64| value)({ 42i64 })
}
"#
);

closure_call_case!(
    direct_closure_call_with_mutating_block_argument,
    r#"
fn identity(value: i64) -> i64 { value }
flow main() -> i64 {
    let mut offset = 1i64
    return (|value: i64| value + offset)({
        let input = 41i64
        offset = 2i64
        identity(input)
    })
}
"#
);

#[test]
fn invalid_shorthand_record_name_retains_recovery_instead_of_failing_the_transaction() {
    let (parsed, _) = parsed_revisions_with_source(
        "arcweft-test://closure-call/invalid-record-shorthand",
        "flow main() { return { offset = 2i64, identity(41i64) } }",
    );
    let mut database = HirDatabase::try_new().expect("HIR database");
    let accepted = publish_attached_project(&mut database, &parsed, &key(&parsed));
    assert_eq!(
        accepted.module().status(),
        crate::module::HirModuleStatus::Recovered
    );
}
