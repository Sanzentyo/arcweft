use arcweft_compiler::source::compile_source;
use arcweft_core::{
    entry::RuntimeSchemaLimits, plan::RuntimePlanTypeProjection,
    program_types::RuntimeProgramTypes, value::RuntimeValue,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[test]
fn data_format_uses_its_source_owned_nominal_variant_domain() {
    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> DataFormat {
    return DataFormat::Json
}
"#,
    )
    .expect("DataFormat flow compiles through its accepted closed-enum owner");

    let (type_id, declaration) = compiled
        .plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, declaration)| {
            matches!(
                declaration.projection(),
                RuntimePlanTypeProjection::Nominal { nominal, .. }
                    if nominal.as_str() == "DataFormat"
            )
        })
        .expect("DataFormat has a nominal runtime type row");
    let RuntimePlanTypeProjection::Nominal { nominal, .. } = declaration.projection() else {
        unreachable!("the query selected a nominal row")
    };
    assert_eq!(nominal.as_str(), "DataFormat");

    let domain = compiled
        .plan
        .variant_domains()
        .get(type_id)
        .expect("DataFormat has a source-owned nominal variant domain");
    assert_eq!(
        domain
            .cases()
            .iter()
            .map(|case| case.name())
            .collect::<Vec<_>>(),
        arcweft_data::DataFormat::ALL
            .map(arcweft_data::DataFormat::variant_name)
            .into_iter()
            .collect::<Vec<_>>()
    );

    let limits = RuntimeSchemaLimits::engine_default();
    let semantic_type = declaration.semantic_identity();
    let native = RuntimeProgramTypes::Plan(&compiled.plan)
        .try_variant_value(semantic_type, 0, None, limits)
        .expect("selected plan issues the exact first DataFormat case");
    assert!(matches!(
        &native,
        RuntimeValue::Variant {
            ordinal: 0,
            name,
            ..
        } if name == "Json"
    ));

    let awbc = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "data_format_nominal.arcw",
    )
    .lower()
    .expect("the nominal DataFormat row and domain lower to verified AWBC");
    let projected = RuntimeProgramTypes::Awbc(&awbc.program)
        .try_variant_value(semantic_type, 0, None, limits)
        .expect("selected AWBC issues the exact first DataFormat case");
    assert_eq!(projected, native);
}

#[test]
fn dynamic_decode_uses_complete_standard_data_nominal_domains() {
    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> Result<DataValue, DataError> {
    let bytes = try data.encode("", .Json)
    return data.decode(bytes, .Json)
}
"#,
    )
    .expect("dynamic decode carries the complete standard Data ADT into the plan");

    let nominal = |name: &str| {
        compiled
            .plan
            .type_table()
            .declarations_with_ids()
            .find(|(_, declaration)| {
                matches!(
                    declaration.projection(),
                    RuntimePlanTypeProjection::Nominal { nominal, .. }
                        if nominal.as_str() == name
                )
            })
            .unwrap_or_else(|| panic!("standard Data type `{name}` has a nominal plan row"))
    };

    for (name, expected_cases) in [
        (
            "DataValue",
            vec![
                "Unit", "Bool", "I128", "U128", "F32", "F64", "String", "Char", "Bytes", "Option",
                "Seq", "Tuple", "Map", "Record", "Enum",
            ],
        ),
        (
            "DataErrorKind",
            vec![
                "MissingField",
                "UnknownField",
                "DuplicateField",
                "InvalidType",
                "InvalidEnumTag",
                "NumberOutOfRange",
                "InvalidEncoding",
                "TrailingData",
                "LimitExceeded",
                "UnsupportedFormat",
                "Io",
                "Custom",
            ],
        ),
        ("DataPathSegment", vec!["Field", "Index", "Variant"]),
        ("DataMapKind", vec!["Ordered", "Sorted", "BTree"]),
    ] {
        let (type_id, _) = nominal(name);
        let domain = compiled
            .plan
            .variant_domains()
            .get(type_id)
            .unwrap_or_else(|| panic!("standard Data enum `{name}` has a variant domain"));
        assert_eq!(
            domain
                .cases()
                .iter()
                .map(|case| case.name())
                .collect::<Vec<_>>(),
            expected_cases
        );
    }

    let (data_value_id, _) = nominal("DataValue");
    let data_value_domain = compiled
        .plan
        .variant_domains()
        .get(data_value_id)
        .expect("DataValue has its recursive closed variant domain");
    for case_name in ["Option", "Seq", "Tuple", "Map", "Record", "Enum"] {
        let payload = data_value_domain
            .cases()
            .iter()
            .find(|case| case.name() == case_name)
            .and_then(|case| case.payload())
            .unwrap_or_else(|| panic!("DataValue::{case_name} retains its typed payload"));
        let mut pending = vec![payload];
        let mut seen = std::collections::BTreeSet::new();
        let mut reaches_data_value = false;
        while let Some(current) = pending.pop() {
            if current == data_value_id {
                reaches_data_value = true;
                break;
            }
            if seen.insert(current)
                && let Some(declaration) = compiled.plan.type_table().get(current)
            {
                pending.extend(declaration.projection().children().into_iter().copied());
            }
        }
        assert!(
            reaches_data_value,
            "DataValue::{case_name} points back to the exact DataValue row"
        );
    }

    for (name, expected_fields) in [
        ("standard::DataError", vec!["kind", "path", "message"]),
        ("standard::DataPath", vec!["segments"]),
    ] {
        let (type_id, _) = nominal(name);
        let domain = compiled
            .plan
            .nominal_record_domains()
            .get(type_id)
            .unwrap_or_else(|| panic!("standard Data record `{name}` has a record domain"));
        assert_eq!(
            domain
                .fields()
                .iter()
                .map(|field| field.name().expect("standard fields are named"))
                .collect::<Vec<_>>(),
            expected_fields
        );
    }
}
