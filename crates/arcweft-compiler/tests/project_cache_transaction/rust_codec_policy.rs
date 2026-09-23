//! The same Rust attributes drive reflection and accepted executable metadata.

use arcweft_codec_json::JsonCodec;
use arcweft_core::{
    entry::{RuntimeCodecUse, RuntimeSchemaLimits, TypeLayoutHash},
    pattern::RuntimeSemanticTypeId,
    plan::{RuntimePlan, RuntimePlanTypeProjection},
    program_types::{RuntimeProgramDataShapes, RuntimeProgramTypes},
};
use arcweft_data::{
    Bytes, Codec, DecodeOptions, Encode, EncodeOptions, Reflect, ShapeAccess, ShapeRef, TypeShape,
};
use arcweft_data_derive::{ArcweftEncode, ArcweftReflect};
use arcweft_rust_abi::{
    ArcweftRustBytesFormat, ArcweftRustEnumRepr, ArcweftRustEnumTagStyle, ArcweftRustManifest,
    ArcweftRustPackage, ArcweftRustPackageId, ArcweftRustStructShape, ArcweftRustTypeKind,
};
use arcweft_rust_abi_macros::{ArcweftType, arcweft_export};

#[derive(ArcweftType, ArcweftReflect, ArcweftEncode)]
#[arcweft(rename_all = "camelCase", deny_unknown_fields)]
pub struct WireRecord {
    plain_bytes: Bytes,
    #[arcweft(bytes = "hex", rename = "hexValue")]
    hex_bytes: Bytes,
    #[arcweft(bytes = "base64")]
    base64_bytes: Bytes,
    #[arcweft(bytes = "array")]
    array_bytes: Bytes,
    #[arcweft(bytes)]
    binary_bytes: Bytes,
    nested_bytes: Vec<Bytes>,
}

#[derive(ArcweftType, ArcweftReflect, ArcweftEncode)]
#[arcweft(rename_all = "kebab-case", tag = "kind", content = "body")]
pub enum Adjacent {
    Nothing,
    #[arcweft(rename = "one-value")]
    Payload(Bytes),
    NamedValue {
        #[arcweft(bytes = "hex")]
        byte_value: Bytes,
    },
}

#[derive(ArcweftType, ArcweftReflect, ArcweftEncode)]
#[arcweft(rename_all = "snake_case", tag = "which")]
pub enum Internal {
    NoValue,
    SomeValue { enabled_flag: bool },
}

#[derive(ArcweftType, ArcweftReflect, ArcweftEncode)]
#[arcweft(rename_all = "snake_case", repr = "i16")]
pub enum Numeric {
    Below = -7,
    #[arcweft(rename = "answer")]
    Above = 42,
}

#[derive(ArcweftType, ArcweftReflect, ArcweftEncode)]
#[arcweft(rename_all = "camelCase")]
pub struct Packet<T> {
    payload_value: T,
    optional_value: Option<T>,
}

#[arcweft_export(pure)]
fn wire_record() -> WireRecord {
    let bytes = Bytes::from(vec![0, 17, 254]);
    WireRecord {
        plain_bytes: bytes.clone(),
        hex_bytes: bytes.clone(),
        base64_bytes: bytes.clone(),
        array_bytes: bytes.clone(),
        binary_bytes: bytes.clone(),
        nested_bytes: vec![bytes],
    }
}

#[arcweft_export(pure)]
fn adjacent() -> Adjacent {
    Adjacent::Nothing
}

#[arcweft_export(pure)]
fn internal() -> Internal {
    Internal::NoValue
}

#[arcweft_export(pure)]
fn numeric() -> Numeric {
    Numeric::Below
}

#[arcweft_export(pure)]
fn packet() -> Packet<Bytes> {
    Packet {
        payload_value: Bytes::from(vec![7]),
        optional_value: None,
    }
}

fn metadata() -> ArcweftRustManifest {
    ArcweftRustManifest::builder(ArcweftRustPackage {
        id: ArcweftRustPackageId::try_new(env!("CARGO_PKG_NAME")).unwrap(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        metadata_hash: None,
    })
    .with_type_metadata::<WireRecord>()
    .with_type_metadata::<Adjacent>()
    .with_type_metadata::<Internal>()
    .with_type_metadata::<Numeric>()
    .with_type_metadata::<Packet<Bytes>>()
    .with_function(__arcweft_export_wire_record_metadata())
    .with_function(__arcweft_export_adjacent_metadata())
    .with_function(__arcweft_export_internal_metadata())
    .with_function(__arcweft_export_numeric_metadata())
    .with_function(__arcweft_export_packet_metadata())
    .build()
}

const SOURCE: &str = "flow main { let a = wire_record(); let b = adjacent(); let c = internal(); let d = numeric(); let e = packet(); }\nentry cli @entry.cli.main { goto @flow.main }\n";

fn semantic(plan: &RuntimePlan, name: &str) -> RuntimeSemanticTypeId {
    let row = plan
        .nominal_record_domains()
        .domains()
        .map(|domain| (domain.owner(), domain.data_codec()))
        .chain(
            plan.variant_domains()
                .domains()
                .map(|domain| (domain.owner(), domain.data_codec())),
        )
        .find_map(|(row, codec)| match &codec?.body {
            RuntimeCodecUse::Record { name: actual, .. }
            | RuntimeCodecUse::Enum { name: actual, .. }
                if actual == name =>
            {
                Some(row)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("selected codec domain {name}"));
    plan.type_table().get(row).unwrap().semantic_identity()
}

fn layout(plan: &RuntimePlan, name: &str) -> TypeLayoutHash {
    let row = plan
        .type_table()
        .id_for_semantic(semantic(plan, name))
        .unwrap();
    let RuntimePlanTypeProjection::Nominal { layout, .. } =
        plan.type_table().get(row).unwrap().projection()
    else {
        panic!("Rust nominal row")
    };
    *layout
}

// The fixtures are finite; compare their semantic shapes independently of
// the two graph owners' unrelated local coordinate assignments.
fn expanded(shape: ShapeRef<'_>, access: &dyn ShapeAccess) -> TypeShape {
    let mut shape = shape.resolve(access).unwrap().into_owned();
    match &mut shape {
        TypeShape::Option(item) | TypeShape::Seq(item) => {
            **item = expanded(ShapeRef::Inline(item), access);
        }
        TypeShape::Tuple(items) => {
            for item in items {
                *item = expanded(ShapeRef::Inline(item), access);
            }
        }
        TypeShape::Map { key, value, .. } => {
            **key = expanded(ShapeRef::Inline(key), access);
            **value = expanded(ShapeRef::Inline(value), access);
        }
        TypeShape::Record { fields, .. } => {
            for field in fields {
                field.shape = expanded(ShapeRef::Inline(&field.shape), access);
            }
        }
        TypeShape::Enum { variants, .. } => {
            for variant in variants {
                if let Some(payload) = &mut variant.payload {
                    *payload = expanded(ShapeRef::Inline(payload), access);
                }
            }
        }
        _ => {}
    }
    shape
}

fn same_policy<T: Reflect + Encode + 'static>(
    value: &T,
    semantic: RuntimeSemanticTypeId,
    types: RuntimeProgramTypes<'_>,
) {
    let shapes = RuntimeProgramDataShapes::new(types);
    let selected = shapes
        .root(semantic, RuntimeSchemaLimits::engine_default())
        .unwrap();
    let (reflected, root) = T::shape_graph().unwrap();
    let root = reflected.reference(root);
    assert_eq!(expanded(selected, &shapes), expanded(root, &reflected));
    let value = value.encode().unwrap();
    let expected = JsonCodec
        .encode_value(&value, root, &reflected, &EncodeOptions::default())
        .unwrap();
    let actual = JsonCodec
        .encode_value(&value, selected, &shapes, &EncodeOptions::default())
        .unwrap();
    assert_eq!(actual, expected);
    let expected_value = JsonCodec
        .decode_value(&expected, root, &reflected, &DecodeOptions::default())
        .unwrap();
    let actual_value = JsonCodec
        .decode_value(&actual, selected, &shapes, &DecodeOptions::default())
        .unwrap();
    assert_eq!(actual_value, expected_value);
    assert_eq!(
        JsonCodec
            .encode_value(&actual_value, selected, &shapes, &EncodeOptions::default())
            .unwrap(),
        actual
    );
}

#[test]
fn rust_codec_policy_matches_reflect_after_plan_and_awbc_publication() {
    let rust = metadata();
    let rust = ArcweftRustManifest::from_json(&rust.to_json_pretty().unwrap()).unwrap();
    let (plan, awbc) = super::rust_defaults::compile_rust(&rust, SOURCE).unwrap();
    for types in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        same_policy(&wire_record(), semantic(&plan, "WireRecord"), types);
        for value in [
            adjacent(),
            Adjacent::Payload(Bytes::from(vec![8, 9])),
            Adjacent::NamedValue {
                byte_value: Bytes::from(vec![8, 9]),
            },
        ] {
            same_policy(&value, semantic(&plan, "Adjacent"), types);
        }
        same_policy(&internal(), semantic(&plan, "Internal"), types);
        same_policy(
            &Internal::SomeValue { enabled_flag: true },
            semantic(&plan, "Internal"),
            types,
        );
        same_policy(&numeric(), semantic(&plan, "Numeric"), types);
        same_policy(&Numeric::Above, semantic(&plan, "Numeric"), types);
        same_policy(&packet(), semantic(&plan, "Packet"), types);
    }
}

#[test]
fn every_rust_wire_policy_change_is_committed_to_the_source_layout() {
    let (base, _) = super::rust_defaults::compile_rust(&metadata(), SOURCE).unwrap();
    for mutation in 0..7 {
        let mut rust = metadata();
        let name = match mutation {
            0..=2 => "WireRecord",
            3..=4 => "Adjacent",
            _ => "Numeric",
        };
        let declaration = rust
            .types
            .iter_mut()
            .find(|declaration| declaration.data_policy.as_ref().unwrap().name == name)
            .unwrap();
        match (&mut declaration.kind, mutation) {
            (
                ArcweftRustTypeKind::Struct {
                    shape: ArcweftRustStructShape::Record { fields },
                },
                0,
            ) => fields[1].wire_name = Some("differentHexKey".to_owned()),
            (
                ArcweftRustTypeKind::Struct {
                    shape: ArcweftRustStructShape::Record { fields },
                },
                1,
            ) => fields[1].bytes_format = Some(ArcweftRustBytesFormat::Base64),
            (_, 2) => {
                declaration
                    .data_policy
                    .as_mut()
                    .unwrap()
                    .deny_unknown_fields = false
            }
            (ArcweftRustTypeKind::Enum { variants }, 3) => {
                variants[1].wire_name = Some("different-value".to_owned())
            }
            (_, 4) => {
                declaration.data_policy.as_mut().unwrap().tag = ArcweftRustEnumTagStyle::Adjacent {
                    tag: "tag".to_owned(),
                    content: "payload".to_owned(),
                }
            }
            (_, 5) => {
                declaration.data_policy.as_mut().unwrap().repr = Some(ArcweftRustEnumRepr::I32)
            }
            (ArcweftRustTypeKind::Enum { variants }, 6) => variants[0].discriminant = Some(-8),
            _ => panic!("fixture shape"),
        }
        let (changed, _) = super::rust_defaults::compile_rust(&rust, SOURCE).unwrap();
        assert_ne!(
            layout(&base, name),
            layout(&changed, name),
            "policy mutation {mutation}"
        );
    }
}

#[test]
fn malformed_rust_wire_policies_cannot_publish_an_abi_manifest() {
    for mutation in 0..7 {
        let mut rust = metadata();
        let name = match mutation {
            0 => "WireRecord",
            1 | 2 => "Adjacent",
            3 => "Internal",
            _ => "Numeric",
        };
        let declaration = rust
            .types
            .iter_mut()
            .find(|declaration| declaration.data_policy.as_ref().unwrap().name == name)
            .unwrap();
        match (&mut declaration.kind, mutation) {
            (
                ArcweftRustTypeKind::Struct {
                    shape: ArcweftRustStructShape::Record { fields },
                },
                0,
            ) => fields[1].wire_name = fields[0].wire_name.clone(),
            (ArcweftRustTypeKind::Enum { variants }, 1) => {
                variants[1].wire_name = variants[0].wire_name.clone()
            }
            (_, 2) => {
                declaration.data_policy.as_mut().unwrap().tag = ArcweftRustEnumTagStyle::Internal {
                    tag: "kind".to_owned(),
                }
            }
            (ArcweftRustTypeKind::Enum { variants }, 3) => {
                let arcweft_rust_abi::ArcweftRustVariantPayload::Record { fields } =
                    &mut variants[1].payload
                else {
                    panic!()
                };
                fields[0].wire_name = Some("which".to_owned());
            }
            (ArcweftRustTypeKind::Enum { variants }, 4) => variants[0].discriminant = None,
            (ArcweftRustTypeKind::Enum { variants }, 5) => {
                variants[0].discriminant = Some(i128::MAX)
            }
            (ArcweftRustTypeKind::Enum { variants }, 6) => {
                variants[1].discriminant = variants[0].discriminant
            }
            _ => panic!("fixture shape"),
        }
        assert!(
            matches!(
                rust.validate(arcweft_rust_abi::ArcweftRustAbiLimits::PRODUCTION),
                Err(arcweft_rust_abi::ArcweftRustManifestError::CodecPolicy { .. })
            ),
            "policy mutation {mutation}"
        );
    }
}
