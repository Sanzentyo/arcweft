use std::collections::BTreeMap;

use arcweft_config::{
    ConfigLayer, ConfigLayerKind, ConfigMergePolicy, ListMergeStrategy, merge_config_layers,
};
use arcweft_data::{DataErrorKind, FieldShape, MapKind, Number, RecordPolicy, TypeShape, Value};

fn server_shape() -> TypeShape {
    TypeShape::Record {
        name: "ServerConfig".to_owned(),
        fields: vec![
            FieldShape::new("host", "host", TypeShape::String),
            FieldShape::new("port", "port", TypeShape::U16),
            FieldShape::new("features", "features", TypeShape::seq(TypeShape::String)),
            FieldShape::new("token", "token", TypeShape::option(TypeShape::String)),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    }
}

fn record(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Record(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

#[test]
fn shape_merge_tracks_precedence_and_provenance() {
    let defaults = ConfigLayer::new(
        ConfigLayerKind::Defaults,
        record([
            ("host", Value::String("127.0.0.1".to_owned())),
            ("port", Value::Number(Number::U(80))),
            (
                "features",
                Value::Seq(vec![Value::String("base".to_owned())]),
            ),
        ]),
    )
    .with_source("defaults");
    let env = ConfigLayer::new(
        ConfigLayerKind::Environment,
        record([
            ("port", Value::Number(Number::U(8080))),
            (
                "token",
                Value::Option(Some(Box::new(Value::String("secret".to_owned())))),
            ),
        ]),
    )
    .with_source("env");

    let report = merge_config_layers(
        [defaults, env],
        &server_shape(),
        &ConfigMergePolicy::default(),
    )
    .expect("merge");

    let fields = report.value.as_record().expect("record");
    assert_eq!(
        fields.get("host"),
        Some(&Value::String("127.0.0.1".to_owned()))
    );
    assert_eq!(fields.get("port"), Some(&Value::Number(Number::U(8080))));
    assert_eq!(
        report.provenance["$.host"].source.as_deref(),
        Some("defaults")
    );
    assert_eq!(report.provenance["$.port"].source.as_deref(), Some("env"));
    assert_eq!(
        report.provenance["$.port"].layer_kind,
        ConfigLayerKind::Environment
    );
}

#[test]
fn shape_merge_rejects_unknown_fields() {
    let error = merge_config_layers(
        [ConfigLayer::new(
            ConfigLayerKind::File,
            record([
                ("host", Value::String("localhost".to_owned())),
                ("port", Value::Number(Number::U(80))),
                ("features", Value::Seq(Vec::new())),
                ("extra", Value::Bool(true)),
            ]),
        )],
        &server_shape(),
        &ConfigMergePolicy::default(),
    )
    .expect_err("unknown field");

    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn shape_merge_reports_missing_required_fields_after_all_layers() {
    let error = merge_config_layers(
        [ConfigLayer::new(
            ConfigLayerKind::File,
            record([("host", Value::String("localhost".to_owned()))]),
        )],
        &server_shape(),
        &ConfigMergePolicy::default(),
    )
    .expect_err("missing port/features");

    assert_eq!(error.kind(), &DataErrorKind::MissingField);
}

#[test]
fn shape_merge_appends_lists_when_policy_requests_it() {
    let policy = ConfigMergePolicy {
        list_strategy: ListMergeStrategy::Append,
        ..ConfigMergePolicy::default()
    };
    let report = merge_config_layers(
        [
            ConfigLayer::new(
                ConfigLayerKind::Defaults,
                record([
                    ("host", Value::String("localhost".to_owned())),
                    ("port", Value::Number(Number::U(80))),
                    (
                        "features",
                        Value::Seq(vec![Value::String("base".to_owned())]),
                    ),
                ]),
            ),
            ConfigLayer::new(
                ConfigLayerKind::CommandLine,
                record([(
                    "features",
                    Value::Seq(vec![Value::String("cli".to_owned())]),
                )]),
            )
            .with_source("argv"),
        ],
        &server_shape(),
        &policy,
    )
    .expect("merge");

    let features = report
        .value
        .as_record()
        .expect("record")
        .get("features")
        .expect("features");
    assert_eq!(
        features,
        &Value::Seq(vec![
            Value::String("base".to_owned()),
            Value::String("cli".to_owned())
        ])
    );
    assert_eq!(
        report.provenance["$.features[1]"].layer_kind,
        ConfigLayerKind::CommandLine
    );
}

#[test]
fn redact_uses_policy_keys_recursively() {
    let value = Value::Record(BTreeMap::from([(
        "auth".to_owned(),
        Value::Record(BTreeMap::from([(
            "api_token".to_owned(),
            Value::String("secret".to_owned()),
        )])),
    )]));

    let redacted = arcweft_config::redact(&value, &ConfigMergePolicy::default());
    let Value::Record(root) = redacted else {
        panic!("redacted root stays record");
    };
    let Value::Record(auth) = root.get("auth").expect("auth") else {
        panic!("auth stays record");
    };
    assert_eq!(
        auth.get("api_token"),
        Some(&Value::String("<redacted>".to_owned()))
    );
}

#[test]
fn shape_merge_rejects_non_finite_float_values() {
    let shape = TypeShape::record(
        "NumericConfig",
        [FieldShape::new(
            "threshold",
            "threshold",
            TypeShape::option(TypeShape::F64),
        )],
    );

    let error = merge_config_layers(
        [ConfigLayer::new(
            ConfigLayerKind::File,
            record([(
                "threshold",
                Value::Option(Some(Box::new(Value::Number(Number::F64(f64::INFINITY))))),
            )]),
        )],
        &shape,
        &ConfigMergePolicy::default(),
    )
    .expect_err("non-finite float");

    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn map_merge_preserves_kind_and_ordering_contract() {
    let entries = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(key, value)| {
                (
                    Value::String((*key).to_owned()),
                    Value::String((*value).to_owned()),
                )
            })
            .collect::<Vec<_>>()
    };
    for (kind, expected) in [
        (MapKind::Ordered, vec!["z", "a", "m"]),
        (MapKind::BTree, vec!["a", "m", "z"]),
    ] {
        let shape = TypeShape::map(TypeShape::String, TypeShape::String, kind);
        let report = merge_config_layers(
            [
                ConfigLayer::new(
                    ConfigLayerKind::Defaults,
                    Value::map(kind, entries(&[("z", "old"), ("a", "first")])),
                ),
                ConfigLayer::new(
                    ConfigLayerKind::File,
                    Value::map(kind, entries(&[("z", "new"), ("m", "last")])),
                ),
            ],
            &shape,
            &ConfigMergePolicy::default(),
        )
        .expect("matching maps merge");
        let Value::Map {
            kind: merged_kind,
            entries: merged,
        } = report.value
        else {
            panic!("merge returns a map");
        };
        assert_eq!(merged_kind, kind);
        assert_eq!(
            merged
                .iter()
                .map(|(key, _)| match key {
                    Value::String(key) => key.as_str(),
                    _ => panic!("string-key map"),
                })
                .collect::<Vec<_>>(),
            expected
        );
        assert!(merged.contains(&(
            Value::String("z".to_owned()),
            Value::String("new".to_owned())
        )));
    }
}

#[test]
fn missing_optional_is_explicit_and_missing_default_requires_a_producer() {
    let optional = TypeShape::record(
        "OptionalConfig",
        [FieldShape::new(
            "token",
            "token",
            TypeShape::option(TypeShape::String),
        )],
    );
    let report = merge_config_layers(
        [ConfigLayer::new(ConfigLayerKind::File, record([]))],
        &optional,
        &ConfigMergePolicy::default(),
    )
    .expect("missing option is empty");
    assert_eq!(
        report.value.as_record().unwrap().get("token"),
        Some(&Value::Option(None))
    );

    let defaulted = TypeShape::record(
        "DefaultedConfig",
        [FieldShape::new("token", "token", TypeShape::option(TypeShape::String)).with_default()],
    );
    let error = merge_config_layers(
        [ConfigLayer::new(ConfigLayerKind::File, record([]))],
        &defaulted,
        &ConfigMergePolicy::default(),
    )
    .expect_err("a default marker cannot synthesize a value");
    assert_eq!(error.kind(), &DataErrorKind::MissingField);
}
