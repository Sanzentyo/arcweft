use super::*;
use std::collections::BTreeMap;

const STRING: usize = 0;
const BYTES: usize = 1;
const USIZE: usize = 2;
const TUPLE_STRING: usize = 3;
const TUPLE_USIZE: usize = 4;
const PATH_SEGMENT: usize = 5;
const PATH_SEGMENT_SEQUENCE: usize = 6;
const DATA_PATH: usize = 7;
const DATA_ERROR_KIND: usize = 8;
const DATA_ERROR: usize = 9;
const TUPLE_BYTES: usize = 10;
const TUPLE_DATA_ERROR: usize = 11;
const RESULT_BYTES: usize = 12;
const RESULT_STRING: usize = 13;
const DATA_SHAPE_STRING: usize = 14;
const DATA_FORMAT: usize = 15;
const UNIT: usize = 16;
const I8: usize = 17;
const TUPLE_UNIT: usize = 18;
const TUPLE_ZERO: usize = 19;
const TUPLE_ONE: usize = 20;
const MAP: usize = 21;
const OPTION: usize = 22;
const DATA_SHAPE_OPTION: usize = 23;
const DATA_SHAPE_TUPLE_ZERO: usize = 24;
const DATA_SHAPE_TUPLE_ONE: usize = 25;
const DATA_SHAPE_MAP: usize = 26;
const TUPLE_PAYLOAD_OPTION: usize = 27;
const TUPLE_PAYLOAD_TUPLE_ZERO: usize = 28;
const TUPLE_PAYLOAD_TUPLE_ONE: usize = 29;
const TUPLE_PAYLOAD_MAP: usize = 30;
const RESULT_OPTION: usize = 31;
const RESULT_TUPLE_ZERO: usize = 32;
const RESULT_TUPLE_ONE: usize = 33;
const RESULT_MAP: usize = 34;
const TUPLE_ONE_I8: usize = 35;
const TRANSPARENT_TUPLE_VARIANT: usize = 36;
const DATA_SHAPE_TRANSPARENT_TUPLE_VARIANT: usize = 37;
const TUPLE_PAYLOAD_TRANSPARENT_TUPLE_VARIANT: usize = 38;
const RESULT_TRANSPARENT_TUPLE_VARIANT: usize = 39;

fn semantic(index: usize) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([u8::try_from(index + 32).expect("fixture row fits u8"); 32])
}

fn type_id(index: usize) -> AwbcTypeId {
    AwbcTypeId(u32::try_from(index).expect("fixture has fewer than u32 rows"))
}

fn intern(
    strings: &mut Vec<String>,
    ids: &mut BTreeMap<String, AwbcStringId>,
    text: &str,
) -> AwbcStringId {
    if let Some(id) = ids.get(text) {
        return *id;
    }
    let id = AwbcStringId(u32::try_from(strings.len()).expect("fixture string table fits u32"));
    strings.push(text.to_owned());
    ids.insert(text.to_owned(), id);
    id
}

fn runtime_type(
    row: usize,
    shape: AwbcRuntimeTypeShape,
    data_codec: Option<RuntimeCodecUse>,
) -> AwbcRuntimeType {
    let row = AwbcRuntimeType::new(semantic(row), shape);
    data_codec.map_or(row.clone(), |codec| row.with_data_codec(codec))
}

fn record_field(
    ordinal: usize,
    name: &str,
    ty: usize,
    strings: &mut Vec<String>,
    ids: &mut BTreeMap<String, AwbcStringId>,
) -> AwbcRecordField {
    AwbcRecordField {
        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
            .expect("fixture record field ordinal fits"),
        name: Some(intern(strings, ids, name)),
        ty: type_id(ty),
    }
}

fn codec_field(name: &str, value: RuntimeCodecUse) -> RuntimeFieldCodecUse {
    RuntimeFieldCodecUse {
        wire_name: name.to_owned(),
        has_default: false,
        default_program: None,
        skip: false,
        bytes_format: None,
        value,
    }
}

fn codec_enum(
    name: &str,
    cases: impl IntoIterator<Item = RuntimeVariantCodecUse>,
) -> RuntimeCodecUse {
    RuntimeCodecUse::Enum {
        name: name.to_owned(),
        tag: RuntimeEnumTagStyle::External,
        repr: None,
        cases: cases.into_iter().collect::<Vec<_>>().into_boxed_slice(),
    }
}

fn codec_case(name: &str, payload: Option<RuntimeCodecUse>) -> RuntimeVariantCodecUse {
    RuntimeVariantCodecUse {
        wire_name: name.to_owned(),
        discriminant: None,
        payload,
    }
}

fn tuple_codec(item: RuntimeCodecUse) -> RuntimeCodecUse {
    RuntimeCodecUse::Tuple {
        items: vec![item].into_boxed_slice(),
    }
}

struct DataProgramFixture {
    owner: RuntimeProgramOwner,
}

impl DataProgramFixture {
    fn new() -> Self {
        let mut strings = Vec::new();
        let mut string_ids = BTreeMap::new();
        let data_format_cases = DataFormat::ALL
            .iter()
            .map(|format| format.variant_name().to_owned())
            .collect::<Vec<_>>();
        let error_kind_cases = [
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
        ];
        let record_layout = |tag: u8| [tag; 32];
        let data_format_owner = AwbcVariantIdentity::Nominal {
            public_id: intern(&mut strings, &mut string_ids, "DataFormat"),
            layout: record_layout(0xF0),
        };
        let data_error_kind_owner = AwbcVariantIdentity::Nominal {
            public_id: intern(&mut strings, &mut string_ids, "DataErrorKind"),
            layout: record_layout(0xF1),
        };
        let path_segment_owner = AwbcVariantIdentity::Nominal {
            public_id: intern(&mut strings, &mut string_ids, "DataPathSegment"),
            layout: record_layout(0xF2),
        };
        let transparent_tuple_variant_owner = AwbcVariantIdentity::Nominal {
            public_id: intern(&mut strings, &mut string_ids, "TransparentTuplePayload"),
            layout: record_layout(0xF5),
        };
        let data_path_public_id = intern(&mut strings, &mut string_ids, "DataPath");
        let data_error_public_id = intern(&mut strings, &mut string_ids, "DataError");

        let format_use = codec_enum(
            "DataFormat",
            data_format_cases.iter().map(|name| codec_case(name, None)),
        );
        let error_kind_use = codec_enum(
            "DataErrorKind",
            error_kind_cases.iter().map(|name| codec_case(name, None)),
        );
        let path_segment_use = codec_enum(
            "DataPathSegment",
            [
                codec_case("Field", Some(tuple_codec(RuntimeCodecUse::Plain))),
                codec_case("Index", Some(tuple_codec(RuntimeCodecUse::Plain))),
                codec_case("Variant", Some(tuple_codec(RuntimeCodecUse::Plain))),
            ],
        );
        let path_use = RuntimeCodecUse::Record {
            name: "DataPath".to_owned(),
            deny_unknown_fields: true,
            fields: vec![codec_field(
                "segments",
                RuntimeCodecUse::Unary {
                    item: Box::new(path_segment_use.clone()),
                },
            )]
            .into_boxed_slice(),
        };
        let error_use = RuntimeCodecUse::Record {
            name: "DataError".to_owned(),
            deny_unknown_fields: true,
            fields: vec![
                codec_field("kind", error_kind_use.clone()),
                codec_field("path", path_use.clone()),
                codec_field("message", RuntimeCodecUse::Plain),
            ]
            .into_boxed_slice(),
        };
        let transparent_tuple_variant_use = codec_enum(
            "TransparentTuplePayload",
            [codec_case(
                "Value",
                Some(RuntimeCodecUse::Newtype {
                    inner: Box::new(RuntimeCodecUse::Plain),
                }),
            )],
        );

        let mut format_cases = Vec::new();
        let mut error_kind_runtime_cases = Vec::new();
        for name in &data_format_cases {
            format_cases.push(AwbcVariantCase {
                name: intern(&mut strings, &mut string_ids, name),
                payload: None,
            });
        }
        for name in error_kind_cases {
            error_kind_runtime_cases.push(AwbcVariantCase {
                name: intern(&mut strings, &mut string_ids, name),
                payload: None,
            });
        }
        let path_segment_cases = [
            ("Field", TUPLE_STRING),
            ("Index", TUPLE_USIZE),
            ("Variant", TUPLE_STRING),
        ]
        .into_iter()
        .map(|(name, payload)| AwbcVariantCase {
            name: intern(&mut strings, &mut string_ids, name),
            payload: Some(type_id(payload)),
        })
        .collect::<Vec<_>>();

        let path_fields = vec![record_field(
            0,
            "segments",
            PATH_SEGMENT_SEQUENCE,
            &mut strings,
            &mut string_ids,
        )];
        let error_fields = vec![
            record_field(0, "kind", DATA_ERROR_KIND, &mut strings, &mut string_ids),
            record_field(1, "path", DATA_PATH, &mut strings, &mut string_ids),
            record_field(2, "message", STRING, &mut strings, &mut string_ids),
        ];

        let format_owner_id = match &data_format_owner {
            AwbcVariantIdentity::Nominal { public_id, .. } => *public_id,
            AwbcVariantIdentity::Builtin(_) => unreachable!(),
        };
        let format_layout = record_layout(0xF0);
        let error_kind_owner_id = match &data_error_kind_owner {
            AwbcVariantIdentity::Nominal { public_id, .. } => *public_id,
            AwbcVariantIdentity::Builtin(_) => unreachable!(),
        };
        let error_kind_layout = record_layout(0xF1);
        let path_segment_owner_id = match &path_segment_owner {
            AwbcVariantIdentity::Nominal { public_id, .. } => *public_id,
            AwbcVariantIdentity::Builtin(_) => unreachable!(),
        };
        let path_segment_layout = record_layout(0xF2);

        let mut runtime_types = vec![
            runtime_type(STRING, AwbcRuntimeTypeShape::String, None),
            runtime_type(
                BYTES,
                AwbcRuntimeTypeShape::Bytes,
                Some(RuntimeCodecUse::Bytes {
                    format: RuntimeBytesFormat::Binary,
                }),
            ),
            runtime_type(
                USIZE,
                AwbcRuntimeTypeShape::UInt(arcweft_core::awbc::schema::AwbcUnsignedIntKind::USize),
                None,
            ),
            runtime_type(
                TUPLE_STRING,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(STRING)]),
                None,
            ),
            runtime_type(
                TUPLE_USIZE,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(USIZE)]),
                None,
            ),
            runtime_type(
                PATH_SEGMENT,
                AwbcRuntimeTypeShape::Variant {
                    owner: path_segment_owner,
                    arguments: Vec::new(),
                    cases: path_segment_cases,
                },
                Some(path_segment_use),
            ),
            runtime_type(
                PATH_SEGMENT_SEQUENCE,
                AwbcRuntimeTypeShape::Sequence(type_id(PATH_SEGMENT)),
                None,
            ),
            runtime_type(
                DATA_PATH,
                AwbcRuntimeTypeShape::NominalRecord {
                    public_id: data_path_public_id,
                    layout: record_layout(0xF3),
                    arguments: Vec::new(),
                    shape: RuntimeNominalRecordShape::Record,
                    fields: path_fields,
                },
                Some(path_use),
            ),
            runtime_type(
                DATA_ERROR_KIND,
                AwbcRuntimeTypeShape::Variant {
                    owner: data_error_kind_owner,
                    arguments: Vec::new(),
                    cases: error_kind_runtime_cases,
                },
                Some(error_kind_use),
            ),
            runtime_type(
                DATA_ERROR,
                AwbcRuntimeTypeShape::NominalRecord {
                    public_id: data_error_public_id,
                    layout: record_layout(0xF4),
                    arguments: Vec::new(),
                    shape: RuntimeNominalRecordShape::Record,
                    fields: error_fields,
                },
                Some(error_use),
            ),
            runtime_type(
                TUPLE_BYTES,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(BYTES)]),
                None,
            ),
            runtime_type(
                TUPLE_DATA_ERROR,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(DATA_ERROR)]),
                None,
            ),
            runtime_type(
                RESULT_BYTES,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Ok"),
                            payload: Some(type_id(TUPLE_BYTES)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Err"),
                            payload: Some(type_id(TUPLE_DATA_ERROR)),
                        },
                    ],
                },
                None,
            ),
            runtime_type(
                RESULT_STRING,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Ok"),
                            payload: Some(type_id(TUPLE_STRING)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Err"),
                            payload: Some(type_id(TUPLE_DATA_ERROR)),
                        },
                    ],
                },
                None,
            ),
            runtime_type(
                DATA_SHAPE_STRING,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(type_id(STRING))),
                None,
            ),
            runtime_type(
                DATA_FORMAT,
                AwbcRuntimeTypeShape::Variant {
                    owner: data_format_owner,
                    arguments: Vec::new(),
                    cases: format_cases,
                },
                Some(format_use),
            ),
            runtime_type(UNIT, AwbcRuntimeTypeShape::Unit, None),
            runtime_type(
                I8,
                AwbcRuntimeTypeShape::Int(arcweft_core::awbc::schema::AwbcSignedIntKind::I8),
                None,
            ),
            runtime_type(
                TUPLE_UNIT,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(UNIT)]),
                None,
            ),
            runtime_type(TUPLE_ZERO, AwbcRuntimeTypeShape::Tuple(Vec::new()), None),
            runtime_type(
                TUPLE_ONE,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(STRING)]),
                None,
            ),
            runtime_type(
                MAP,
                AwbcRuntimeTypeShape::Map {
                    kind: arcweft_core::entry::RuntimeMapKind::Ordered,
                    key: type_id(I8),
                    value: type_id(STRING),
                },
                None,
            ),
            runtime_type(
                OPTION,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Some"),
                            payload: Some(type_id(TUPLE_UNIT)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "None"),
                            payload: None,
                        },
                    ],
                },
                None,
            ),
            runtime_type(
                DATA_SHAPE_OPTION,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(type_id(OPTION))),
                None,
            ),
            runtime_type(
                DATA_SHAPE_TUPLE_ZERO,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(type_id(TUPLE_ZERO))),
                None,
            ),
            runtime_type(
                DATA_SHAPE_TUPLE_ONE,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(type_id(TUPLE_ONE))),
                None,
            ),
            runtime_type(
                DATA_SHAPE_MAP,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(type_id(MAP))),
                None,
            ),
            runtime_type(
                TUPLE_PAYLOAD_OPTION,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(OPTION)]),
                None,
            ),
            runtime_type(
                TUPLE_PAYLOAD_TUPLE_ZERO,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(TUPLE_ZERO)]),
                None,
            ),
            runtime_type(
                TUPLE_PAYLOAD_TUPLE_ONE,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(TUPLE_ONE)]),
                None,
            ),
            runtime_type(
                TUPLE_PAYLOAD_MAP,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(MAP)]),
                None,
            ),
            runtime_type(
                RESULT_OPTION,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Ok"),
                            payload: Some(type_id(TUPLE_PAYLOAD_OPTION)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Err"),
                            payload: Some(type_id(TUPLE_DATA_ERROR)),
                        },
                    ],
                },
                None,
            ),
            runtime_type(
                RESULT_TUPLE_ZERO,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Ok"),
                            payload: Some(type_id(TUPLE_PAYLOAD_TUPLE_ZERO)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Err"),
                            payload: Some(type_id(TUPLE_DATA_ERROR)),
                        },
                    ],
                },
                None,
            ),
            runtime_type(
                RESULT_TUPLE_ONE,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Ok"),
                            payload: Some(type_id(TUPLE_PAYLOAD_TUPLE_ONE)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Err"),
                            payload: Some(type_id(TUPLE_DATA_ERROR)),
                        },
                    ],
                },
                None,
            ),
            runtime_type(
                RESULT_MAP,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Ok"),
                            payload: Some(type_id(TUPLE_PAYLOAD_MAP)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Err"),
                            payload: Some(type_id(TUPLE_DATA_ERROR)),
                        },
                    ],
                },
                None,
            ),
        ];
        runtime_types.extend([
            runtime_type(
                TUPLE_ONE_I8,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(I8)]),
                None,
            ),
            runtime_type(
                TRANSPARENT_TUPLE_VARIANT,
                AwbcRuntimeTypeShape::Variant {
                    owner: transparent_tuple_variant_owner,
                    arguments: Vec::new(),
                    cases: vec![AwbcVariantCase {
                        name: intern(&mut strings, &mut string_ids, "Value"),
                        payload: Some(type_id(TUPLE_ONE_I8)),
                    }],
                },
                Some(transparent_tuple_variant_use),
            ),
            runtime_type(
                DATA_SHAPE_TRANSPARENT_TUPLE_VARIANT,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(type_id(
                    TRANSPARENT_TUPLE_VARIANT,
                ))),
                None,
            ),
            runtime_type(
                TUPLE_PAYLOAD_TRANSPARENT_TUPLE_VARIANT,
                AwbcRuntimeTypeShape::Tuple(vec![type_id(TRANSPARENT_TUPLE_VARIANT)]),
                None,
            ),
            runtime_type(
                RESULT_TRANSPARENT_TUPLE_VARIANT,
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: Vec::new(),
                    cases: vec![
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Ok"),
                            payload: Some(type_id(TUPLE_PAYLOAD_TRANSPARENT_TUPLE_VARIANT)),
                        },
                        AwbcVariantCase {
                            name: intern(&mut strings, &mut string_ids, "Err"),
                            payload: Some(type_id(TUPLE_DATA_ERROR)),
                        },
                    ],
                },
                None,
            ),
        ]);
        // Keep the fixture table contiguous and the ids above tied to row positions.
        runtime_types.shrink_to_fit();
        let mut program = AwbcProgram::default();
        program.strings = strings;
        program.runtime_types = std::mem::take(&mut runtime_types);
        let _ = (
            format_owner_id,
            format_layout,
            error_kind_owner_id,
            error_kind_layout,
            path_segment_owner_id,
            path_segment_layout,
        );
        Self {
            owner: RuntimeProgramOwner::Awbc(Arc::new(program)),
        }
    }

    fn context(
        &self,
        argument_types: impl IntoIterator<Item = RuntimeSemanticTypeId>,
        result: usize,
    ) -> RuntimeExternalCallContext {
        RuntimeExternalCallContext::for_program(
            self.owner.clone(),
            argument_types,
            semantic(result),
            RuntimeSchemaLimits::engine_default(),
        )
        .expect("fixture call context resolves exact selected rows")
    }

    fn format_value(&self, format: DataFormat) -> RuntimeValue {
        let ordinal = DataFormat::ALL
            .iter()
            .position(|candidate| candidate == &format)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .expect("DataFormat inventory fits the runtime ordinal");
        RuntimeProgramTypes::Awbc(match &self.owner {
            RuntimeProgramOwner::Awbc(program) => program,
            RuntimeProgramOwner::Plan(_) => unreachable!(),
        })
        .try_variant_value(
            semantic(DATA_FORMAT),
            ordinal,
            None,
            RuntimeSchemaLimits::engine_default(),
        )
        .expect("DataFormat is constructed by its selected nominal enum")
    }

    fn string_shape(&self) -> RuntimeValue {
        self.shape_value(DATA_SHAPE_STRING)
    }

    fn shape_value(&self, shape_type: usize) -> RuntimeValue {
        RuntimeValue::Agent(arcweft_core::value::RuntimeAgentValue::DataShape(
            arcweft_core::value::RuntimeDataShape::bind(self.owner.clone(), semantic(shape_type))
                .expect("selected DataShape child binds to its program"),
        ))
    }
}

fn typed_round_trip(
    fixture: &DataProgramFixture,
    accelerator: &mut RuntimePureAccelerator,
    value_type: usize,
    shape_type: usize,
    result_type: usize,
    value: RuntimeValue,
) -> Vec<u8> {
    let format = fixture.format_value(DataFormat::Json);
    let encode = fixture.context(
        [
            semantic(value_type),
            semantic(DATA_FORMAT),
            semantic(shape_type),
        ],
        RESULT_BYTES,
    );
    let encoded = accelerator
        .call_external(
            &encode,
            &callable_target("data.encode"),
            &[
                value.clone(),
                format.clone(),
                fixture.shape_value(shape_type),
            ],
        )
        .expect("data.encode is handled")
        .expect("selected graph encodes the typed value");
    let (case, payload) = encoded.builtin_variant_case().expect("Result");
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultOk);
    let Some(RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(bytes)))) = payload else {
        panic!("data.encode returns Result<Bytes, DataError>");
    };
    let bytes = bytes.as_slice().to_vec();
    let decode = fixture.context(
        [semantic(BYTES), semantic(DATA_FORMAT), semantic(shape_type)],
        result_type,
    );
    let decoded = accelerator
        .call_external(
            &decode,
            &callable_target("data.decode"),
            &[
                runtime_sequence_dense_bytes(bytes.clone()),
                format,
                fixture.shape_value(shape_type),
            ],
        )
        .expect("data.decode is handled")
        .expect("selected witness and occurrence graph decode the typed value");
    let (case, payload) = decoded.builtin_variant_case().expect("Result");
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultOk);
    assert_eq!(payload, Some(&value));
    bytes
}

#[test]
fn data_shape_uses_the_selected_program_type_row() {
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let value_type = RuntimeSemanticTypeId::from_bytes([0xE1; 32]);

    let empty_context = data_shape_context(std::iter::empty());
    let shape = accelerator
        .call_external(&empty_context, &callable_target("data.shape"), &[])
        .expect("data.shape is handled")
        .expect("zero-argument shape binds its selected generic result");
    let RuntimeValue::Agent(arcweft_core::value::RuntimeAgentValue::DataShape(shape)) = shape
    else {
        panic!("data.shape returns a program-bound DataShape value");
    };
    assert_eq!(shape.value_type(), value_type);

    let value_context = data_shape_context([value_type]);
    let shape = accelerator
        .call_external(
            &value_context,
            &callable_target("data.shape"),
            &[RuntimeValue::i64(42)],
        )
        .expect("data.shape is handled")
        .expect("value argument matches the selected DataShape child");
    assert!(matches!(
        shape,
        RuntimeValue::Agent(arcweft_core::value::RuntimeAgentValue::DataShape(_))
    ));

    let error = accelerator
        .call_external(
            &value_context,
            &callable_target("data.shape"),
            &[RuntimeValue::String("wrong type".to_owned())],
        )
        .expect("data.shape is handled")
        .expect_err("source value must satisfy the selected child row");
    assert!(matches!(error, RuntimeEvalError::UnsupportedPure { .. }));
}

#[test]
fn data_encode_and_typed_decode_use_the_selected_graph_and_data_format_owner() {
    let fixture = DataProgramFixture::new();
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let value = RuntimeValue::String("hello".to_owned());
    let format = fixture.format_value(DataFormat::Json);
    let encode = fixture.context([semantic(STRING), semantic(DATA_FORMAT)], RESULT_BYTES);
    let encoded = accelerator
        .call_external(
            &encode,
            &callable_target("data.encode"),
            &[value.clone(), format.clone()],
        )
        .expect("data.encode is handled")
        .expect("selected DataFormat and String shape encode successfully");
    let (case, bytes) = encoded
        .builtin_variant_case()
        .expect("Result::Ok has the core builtin identity");
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultOk);
    let Some(RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(bytes)))) = bytes else {
        panic!("data.encode returns Result<Bytes, DataError>");
    };
    assert_eq!(bytes.as_slice(), br#""hello""#);

    let decode = fixture.context(
        [
            semantic(BYTES),
            semantic(DATA_FORMAT),
            semantic(DATA_SHAPE_STRING),
        ],
        RESULT_STRING,
    );
    let decoded = accelerator
        .call_external(
            &decode,
            &callable_target("data.decode"),
            &[
                RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(bytes.clone()))),
                format,
                fixture.string_shape(),
            ],
        )
        .expect("data.decode is handled")
        .expect("explicit witness matches the selected program and String child");
    let (case, payload) = decoded
        .builtin_variant_case()
        .expect("typed decode returns the selected builtin Result");
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultOk);
    assert_eq!(payload, Some(&RuntimeValue::String("hello".to_owned())));
}

#[test]
fn data_codec_errors_are_returned_as_the_selected_data_error_variant() {
    let fixture = DataProgramFixture::new();
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let decode = fixture.context(
        [
            semantic(BYTES),
            semantic(DATA_FORMAT),
            semantic(DATA_SHAPE_STRING),
        ],
        RESULT_STRING,
    );
    let result = accelerator
        .call_external(
            &decode,
            &callable_target("data.decode"),
            &[
                runtime_sequence_dense_bytes(b"{".to_vec()),
                fixture.format_value(DataFormat::Json),
                fixture.string_shape(),
            ],
        )
        .expect("data.decode is handled")
        .expect("codec failures use Result::Err rather than escaping the data boundary");
    let (case, error) = result
        .builtin_variant_case()
        .expect("decode result has the selected builtin Result identity");
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultErr);
    let Some(RuntimeValue::NominalRecord(error)) = error else {
        panic!("Result::Err payload is the selected nominal DataError record");
    };
    assert_eq!(error.type_id().as_str(), "DataError");
    let [
        RuntimeValue::Variant { name: kind, .. },
        RuntimeValue::NominalRecord(path),
        RuntimeValue::String(message),
    ] = error.fields()
    else {
        panic!("DataError retains its kind, path, and message fields");
    };
    assert_eq!(kind, "InvalidEncoding");
    assert!(!message.is_empty());
    let [RuntimeValue::Seq(segments)] = path.fields() else {
        panic!("DataPath retains its ordered segments field");
    };
    assert!(segments.is_empty());
}

#[test]
fn data_calls_reject_unbound_context_and_wrong_format_identity() {
    let fixture = DataProgramFixture::new();
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let value = RuntimeValue::String("hello".to_owned());
    let valid_format = fixture.format_value(DataFormat::Json);
    let encode = fixture.context([semantic(STRING), semantic(DATA_FORMAT)], RESULT_BYTES);
    assert!(
        accelerator
            .call_external(
                &RuntimeExternalCallContext::unbound(),
                &callable_target("data.encode"),
                &[value.clone(), valid_format.clone()],
            )
            .expect("data.encode is handled")
            .is_err()
    );

    let wrong_format = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new("OtherFormat")
                .expect("test nominal identity is valid"),
            semantic_identity: semantic(DATA_FORMAT),
            layout: arcweft_core::entry::TypeLayoutHash::from_bytes([0xF0; 32]),
        },
        ordinal: 0,
        name: "Json".to_owned(),
        payload: None,
    };
    assert!(
        accelerator
            .call_external(
                &encode,
                &callable_target("data.encode"),
                &[value, wrong_format],
            )
            .expect("data.encode is handled")
            .is_err()
    );
}

#[test]
fn data_avro_envelope_round_trips_with_the_selected_shape() {
    let fixture = DataProgramFixture::new();
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let value = RuntimeValue::String("alice".to_owned());
    let format = fixture.format_value(DataFormat::Avro);
    let encode = fixture.context([semantic(STRING), semantic(DATA_FORMAT)], RESULT_BYTES);
    let encoded = accelerator
        .call_external(
            &encode,
            &callable_target("data.encode"),
            &[value.clone(), format.clone()],
        )
        .expect("Avro encode is handled")
        .expect("selected shape is encoded inside the Avro envelope");
    let (_, bytes) = encoded.builtin_variant_case().expect("Result");
    let Some(RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(bytes)))) = bytes else {
        panic!("Avro encode returns bytes");
    };
    let decode = fixture.context(
        [
            semantic(BYTES),
            semantic(DATA_FORMAT),
            semantic(DATA_SHAPE_STRING),
        ],
        RESULT_STRING,
    );
    let decoded = accelerator
        .call_external(
            &decode,
            &callable_target("data.decode"),
            &[
                runtime_sequence_dense_bytes(bytes.as_slice().to_vec()),
                format,
                fixture.string_shape(),
            ],
        )
        .expect("Avro decode is handled")
        .expect("Avro envelope payload decodes through the selected shape graph");
    let (case, value) = decoded.builtin_variant_case().expect("Result");
    assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultOk);
    assert_eq!(value, Some(&RuntimeValue::String("alice".to_owned())));
}

#[test]
fn data_codec_keeps_option_tuple_map_kind_and_entry_order_distinct() {
    let fixture = DataProgramFixture::new();
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let none = typed_round_trip(
        &fixture,
        &mut accelerator,
        OPTION,
        DATA_SHAPE_OPTION,
        RESULT_OPTION,
        RuntimeValue::option_none(),
    );
    let some_unit = typed_round_trip(
        &fixture,
        &mut accelerator,
        OPTION,
        DATA_SHAPE_OPTION,
        RESULT_OPTION,
        RuntimeValue::option_some(RuntimeValue::Unit),
    );
    assert_ne!(
        none, some_unit,
        "Option(None) differs from Some(Unit) on the wire"
    );

    let tuple_zero = typed_round_trip(
        &fixture,
        &mut accelerator,
        TUPLE_ZERO,
        DATA_SHAPE_TUPLE_ZERO,
        RESULT_TUPLE_ZERO,
        RuntimeValue::Tuple(Vec::new()),
    );
    let tuple_one = typed_round_trip(
        &fixture,
        &mut accelerator,
        TUPLE_ONE,
        DATA_SHAPE_TUPLE_ONE,
        RESULT_TUPLE_ONE,
        RuntimeValue::Tuple(vec![RuntimeValue::String("one".to_owned())]),
    );
    assert_ne!(
        tuple_zero, tuple_one,
        "zero- and one-item tuples retain their arity"
    );

    let map = RuntimeValue::Seq(RuntimeSeq::Values(vec![
        RuntimeValue::Tuple(vec![
            RuntimeValue::i8(2),
            RuntimeValue::String("two".to_owned()),
        ]),
        RuntimeValue::Tuple(vec![
            RuntimeValue::i8(1),
            RuntimeValue::String("one".to_owned()),
        ]),
    ]));
    let map_wire = typed_round_trip(
        &fixture,
        &mut accelerator,
        MAP,
        DATA_SHAPE_MAP,
        RESULT_MAP,
        map,
    );
    let wire_text = std::str::from_utf8(&map_wire).expect("JSON map output is UTF-8");
    assert!(wire_text.find('2').unwrap() < wire_text.find('1').unwrap());
}

#[test]
fn data_codec_applies_source_transparent_policy_to_enum_tuple_one_payloads() {
    let fixture = DataProgramFixture::new();
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let variant = RuntimeProgramTypes::Awbc(match &fixture.owner {
        RuntimeProgramOwner::Awbc(program) => program,
        RuntimeProgramOwner::Plan(_) => unreachable!(),
    })
    .try_variant_value(
        semantic(TRANSPARENT_TUPLE_VARIANT),
        0,
        Some(RuntimeValue::Tuple(vec![RuntimeValue::i8(7)])),
        RuntimeSchemaLimits::engine_default(),
    )
    .expect("selected nominal enum retains its one-item tuple payload ABI");
    let wire = typed_round_trip(
        &fixture,
        &mut accelerator,
        TRANSPARENT_TUPLE_VARIANT,
        DATA_SHAPE_TRANSPARENT_TUPLE_VARIANT,
        RESULT_TRANSPARENT_TUPLE_VARIANT,
        variant,
    );
    let wire_text = std::str::from_utf8(&wire).expect("JSON enum output is UTF-8");
    assert!(wire_text.contains(r#""payload":7"#));
    assert!(!wire_text.contains(r#""payload":[7]"#));
}
