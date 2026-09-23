use super::*;

#[test]
fn accepted_environment_record_pattern_projects_its_exact_nominal_fields() {
    let compiled = crate::source::compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
pub fn read_error(error: DataError) -> String {
    let DataError { message, .. } = error
    return message
}
flow main() -> String { return "ok" }
"#,
    )
    .expect("the typed DataError parameter pattern reaches final semantic analysis");
    let analysis_lease = &compiled.analysis;
    let analysis = analysis_lease.final_analysis().as_ref();
    let (owner, pattern) = analysis
        .patterns()
        .find_map(|(owner, pattern)| {
            let CheckedPatternResolution::Record(record) = pattern.resolution() else {
                return None;
            };
            matches!(
                record.owner(),
                CheckedRecordPatternOwner::Environment { .. }
            )
            .then_some((owner, record))
        })
        .expect("the checked DataError destructure has an accepted environment record owner");

    let runtime = runtime_record_pattern(
        owner,
        pattern,
        analysis_lease.project_symbols(),
        analysis_lease.registered_world(),
        analysis,
    )
    .expect("the compiler lowers the checked environment pattern through its nominal owner");
    let nominal = runtime
        .nominal()
        .expect("accepted DataError pattern retains a nominal record definition");
    assert_eq!(
        nominal
            .layout()
            .fields()
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>(),
        [Some("kind"), Some("path"), Some("message")]
    );
    assert_eq!(runtime.fields().len(), 1);
    assert_eq!(runtime.fields()[0].field().zero_based(), 2);
}
