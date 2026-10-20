use std::collections::BTreeMap;

use arcweft_data::{Bytes, DataErrorKind, DecodeLimits, MapKind, Number, Value};

#[test]
fn every_logical_value_including_empty_containers_consumes_a_node() {
    let values = [
        Value::Unit,
        Value::Bool(true),
        Value::Number(Number::I(1)),
        Value::String(String::new()),
        Value::Char('a'),
        Value::Bytes(Bytes::default()),
        Value::Option(None),
        Value::Seq(Vec::new()),
        Value::Tuple(Vec::new()),
        Value::map(MapKind::Ordered, []),
        Value::Record(BTreeMap::new()),
        Value::Enum {
            variant: "Empty".into(),
            payload: None,
        },
    ];
    for value in values {
        DecodeLimits {
            max_nodes: 1,
            ..DecodeLimits::default()
        }
        .validate(&value)
        .expect("one logical node fits");
        let error = DecodeLimits {
            max_nodes: 0,
            ..DecodeLimits::default()
        }
        .validate(&value)
        .expect_err("the root consumes one node");
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        assert!(error.path().is_root());
    }
}

#[test]
fn siblings_share_the_same_node_and_collection_banks() {
    let value = Value::Seq(vec![
        Value::Seq(vec![Value::Unit]),
        Value::Seq(vec![Value::Unit]),
    ]);
    let limits = DecodeLimits {
        max_nodes: 5,
        max_collection_items: 4,
        max_sequence_len: 2,
        ..DecodeLimits::default()
    };
    limits.validate(&value).expect("exact tree allowance");
    limits
        .validate(&value)
        .expect("a new admission receives its own bank");
    let error = DecodeLimits {
        max_nodes: 4,
        ..limits
    }
    .validate(&value)
    .expect_err("the second branch cannot reset nodes");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
    assert_eq!(error.path().to_string(), "$[1][0]");
    let error = DecodeLimits {
        max_collection_items: 3,
        ..limits
    }
    .validate(&value)
    .expect_err("the second branch cannot reset items");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
    assert_eq!(error.path().to_string(), "$[1]");
}

#[test]
fn map_keys_and_values_consume_nodes_but_pairs_consume_one_item() {
    let value = Value::map(
        MapKind::Ordered,
        [(Value::String("key".into()), Value::Unit)],
    );
    let limits = DecodeLimits {
        max_nodes: 3,
        max_collection_items: 1,
        max_map_len: 1,
        ..DecodeLimits::default()
    };
    limits
        .validate(&value)
        .expect("one pair and three logical values");
    for limits in [
        DecodeLimits {
            max_nodes: 2,
            ..limits
        },
        DecodeLimits {
            max_collection_items: 0,
            ..limits
        },
        DecodeLimits {
            max_map_len: 0,
            ..limits
        },
    ] {
        assert_eq!(
            limits
                .validate(&value)
                .expect_err("map allowance exceeded")
                .kind(),
            &DataErrorKind::LimitExceeded
        );
    }
}

#[test]
fn record_names_and_variant_tags_are_bounded_metadata() {
    let record = Value::Record(BTreeMap::from([("猫".into(), Value::Unit)]));
    let variant = Value::Enum {
        variant: "猫".into(),
        payload: Some(Box::new(Value::Unit)),
    };
    let limits = DecodeLimits {
        max_nodes: 2,
        max_string_len: 3,
        max_collection_items: 1,
        ..DecodeLimits::default()
    };
    for value in [&record, &variant] {
        limits
            .validate(value)
            .expect("metadata does not invent logical nodes");
        let error = DecodeLimits {
            max_string_len: 2,
            ..limits
        }
        .validate(value)
        .expect_err("UTF-8 metadata exceeds its byte allowance");
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        assert!(
            error.path().is_root(),
            "unadmitted names must not enter error paths"
        );
    }
    assert_eq!(
        DecodeLimits {
            max_collection_items: 0,
            ..limits
        }
        .validate(&record)
        .expect_err("a field consumes an item")
        .kind(),
        &DataErrorKind::LimitExceeded
    );
    DecodeLimits {
        max_collection_items: 0,
        ..limits
    }
    .validate(&variant)
    .expect("a variant payload is not a collection entry");
}

#[test]
fn nested_record_and_variant_errors_retain_the_admitted_path() {
    let value = Value::Record(BTreeMap::from([(
        "item".into(),
        Value::Enum {
            variant: "Some".into(),
            payload: Some(Box::new(Value::Seq(vec![Value::Unit]))),
        },
    )]));
    let error = DecodeLimits {
        max_nodes: 3,
        ..DecodeLimits::default()
    }
    .validate(&value)
    .expect_err("last leaf exhausts the bank");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
    assert_eq!(error.path().to_string(), "$.item::Some[0]");
}

#[test]
fn depth_and_scalar_bounds_apply_to_every_borrowed_payload() {
    let value = Value::Option(Some(Box::new(Value::Enum {
        variant: "Some".into(),
        payload: Some(Box::new(Value::Tuple(vec![Value::Unit]))),
    })));
    let limits = DecodeLimits {
        max_depth: 3,
        ..DecodeLimits::default()
    };
    limits.validate(&value).expect("root depth starts at zero");
    assert_eq!(
        DecodeLimits {
            max_depth: 2,
            ..limits
        }
        .validate(&value)
        .expect_err("payload depth exceeds the limit")
        .kind(),
        &DataErrorKind::LimitExceeded
    );
    for value in [Value::Char('猫'), Value::String("猫".into())] {
        assert_eq!(
            DecodeLimits {
                max_string_len: 2,
                ..limits
            }
            .validate(&value)
            .expect_err("three UTF-8 bytes exceed two")
            .kind(),
            &DataErrorKind::LimitExceeded
        );
    }
    assert_eq!(
        DecodeLimits {
            max_bytes_len: 1,
            ..limits
        }
        .validate(&Value::Bytes(Bytes::new(vec![0, 1])))
        .expect_err("byte payload exceeds its allowance")
        .kind(),
        &DataErrorKind::LimitExceeded
    );
}

#[test]
fn source_byte_and_trailing_data_policy_do_not_invent_a_value_encoding() {
    DecodeLimits {
        max_input_len: 0,
        allow_trailing_data: true,
        ..DecodeLimits::default()
    }
    .validate(&Value::String("already decoded".into()))
    .expect("borrowed values have no encoded input bytes");
}
