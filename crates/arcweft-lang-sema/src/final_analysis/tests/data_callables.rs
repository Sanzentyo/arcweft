use super::*;

#[test]
fn data_callables_infer_typed_and_dynamic_contracts_from_source_calls() {
    let fixture = fixture(
        r#"
struct Config { value: i64 }

fn declared_shape() -> DataShape<Config> {
    data.shape<Config>()
}

fn inferred_shape(value: Config) -> DataShape<Config> {
    data.shape(value)
}

fn encode_with_shape(value: Config) -> Result<Bytes, DataError> {
    data.encode(value, .Json, data.shape<Config>())
}

fn encode_inferred(value: Config) -> Result<Bytes, DataError> {
    data.encode(value, .Json)
}

fn decode_typed(bytes: Bytes) -> Result<Config, DataError> {
    data.decode(bytes, .Json, data.shape<Config>())
}

fn decode_dynamic(bytes: Bytes) -> Result<DataValue, DataError> {
    data.decode(bytes, .Json)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("data callable signatures type-check from source");
    assert_eq!(
        analysis
            .calls()
            .filter(|(_, call)| call.selected_application().is_some())
            .count(),
        8
    );
    let config = project_nominal_expression_type(&analysis, "Config");

    let results = analysis
        .calls()
        .filter_map(|(_, call)| {
            call.selected_application()
                .and_then(|application| application.result().value_type())
        })
        .collect::<Vec<_>>();
    let shape = TypeKind::DataShape(Box::new(config.clone()));
    let encoded = TypeKind::Result {
        ok: Box::new(TypeKind::Bytes),
        error: Box::new(TypeKind::DataError),
    };
    let typed = TypeKind::Result {
        ok: Box::new(config),
        error: Box::new(TypeKind::DataError),
    };
    let dynamic = TypeKind::Result {
        ok: Box::new(TypeKind::DataValue),
        error: Box::new(TypeKind::DataError),
    };

    assert_eq!(results.len(), 8);
    assert_eq!(
        results.iter().filter(|result| **result == &shape).count(),
        4
    );
    assert_eq!(
        results.iter().filter(|result| **result == &encoded).count(),
        2
    );
    assert_eq!(
        results.iter().filter(|result| **result == &typed).count(),
        1
    );
    assert_eq!(
        results.iter().filter(|result| **result == &dynamic).count(),
        1
    );
}

#[test]
fn standard_data_error_record_pattern_uses_accepted_fields() {
    let fixture = fixture(
        r#"
fn read_error(error: DataError) -> String {
    let DataError { message, .. } = error
    return message
}
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("standard DataError destructures from its checked record schema");
    let records = analysis
        .patterns()
        .filter_map(|(_, pattern)| match pattern.resolution() {
            CheckedPatternResolution::Record(record) => Some((pattern, record)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(pattern, record)] = records.as_slice() else {
        panic!("one accepted DataError record pattern: {records:?}");
    };
    assert_eq!(pattern.ty(), &TypeKind::DataError);
    let crate::final_analysis::CheckedRecordPatternOwner::Environment { record: owner } =
        record.owner()
    else {
        panic!("DataError pattern retains its accepted environment owner");
    };
    assert_eq!(owner.nominal().source_label(), "standard::DataError");
    assert_eq!(record.fields().len(), 1);
    assert_eq!(record.fields()[0].field_type(), &TypeKind::String);
}

#[test]
fn explicit_generic_arguments_bind_before_optional_value_omission() {
    let omitted_value = fixture(
        r#"
struct Config {}

fn config_identity(value: Config) -> Config {
    value
}

fn bind_without_value() {
    data.shape<Config>();
}
"#,
        None,
    );
    let analysis = analyze(&omitted_value)
        .expect("explicit type argument binds the omitted optional value type");
    let shape = TypeKind::DataShape(Box::new(project_nominal_expression_type(
        &analysis, "Config",
    )));
    assert!(analysis.calls().any(|(_, call)| {
        call.selected_application()
            .and_then(|application| application.result().value_type())
            == Some(&shape)
    }));

    let conflicting_value = fixture(
        r#"
struct Config {}
struct Other {}

fn reject_conflicting_value(value: Other) {
    data.shape<Config>(value);
}
"#,
        None,
    );
    let analysis =
        analyze(&conflicting_value).expect("call rejection remains available as tooling evidence");
    let calls = analysis.calls().collect::<Vec<_>>();
    let [(owner, call)] = calls.as_slice() else {
        panic!("one conflicting data.shape call: {calls:?}");
    };
    assert!(matches!(
        call.outcome(),
        crate::callable::CallAnalysisOutcome::Rejected(_)
    ));
    super::callable_values::assert_unselected_call_has_no_execution(&analysis, *owner);
}

#[test]
fn local_value_named_data_keeps_value_receiver_precedence() {
    let fixture = fixture(
        r#"
struct Config {}

fn shape(self: String, value: Config) -> DataShape<Config> {
    data.shape<Config>()
}

fn namespace_call(value: Config) -> DataShape<Config> {
    data.shape(value)
}

fn shadowed_namespace(data: String, value: Config) -> DataShape<Config> {
    data.shape(value)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("registered namespace and shadowing value resolve");
    let (project_calls, environment_calls) =
        analysis
            .calls()
            .fold(
                (0, 0),
                |(project_calls, environment_calls), (_, call)| match selected_candidate(call).id()
                {
                    crate::callable::CallableCandidateId::Project(_) => {
                        (project_calls + 1, environment_calls)
                    }
                    crate::callable::CallableCandidateId::Environment(_) => {
                        (project_calls, environment_calls + 1)
                    }
                    _ => (project_calls, environment_calls),
                },
            );
    assert_eq!(
        project_calls, 1,
        "local `data` selects the project extension"
    );
    assert_eq!(environment_calls, 2, "unshadowed `data` selects data.shape");
}
