//! Real Rust publications reach both executable type tables without constructors.

use super::*;
use arcweft_adapter_context::manifest::AdapterNominalPathPrefix;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        schema::{AwbcProgram, AwbcTypeId},
    },
    entry::{RuntimeNominalRecordShape as Shape, RuntimeSchemaLimits},
    plan::RuntimePlanTypeProjection,
    value::RuntimeValue,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_rust_abi::{
    ArcweftRustField, ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage,
    ArcweftRustPackageId, ArcweftRustPurity, ArcweftRustStructShape, ArcweftRustTypeDecl,
    ArcweftRustTypeKind, ArcweftRustTypeParameter, ArcweftRustTypeParameterIndex,
    ArcweftRustTypePath, ArcweftRustTypePathSegment, ArcweftRustTypeRef as Ty, ArcweftRustVariant,
    ArcweftRustVariantPayload as Payload,
};

fn package() -> ArcweftRustPackageId {
    ArcweftRustPackageId::try_new("compiler_rust_nominals").unwrap()
}

fn path(name: &str) -> ArcweftRustTypePath {
    ArcweftRustTypePath::try_new([ArcweftRustTypePathSegment::try_new(name).unwrap()]).unwrap()
}

fn nominal(name: &str, arguments: Vec<Ty>) -> Ty {
    Ty::Nominal {
        package: package(),
        path: path(name),
        arguments,
    }
}

fn fixture_manifest() -> AdapterManifest {
    let parameter = ArcweftRustTypeParameterIndex::try_from_usize(0).unwrap();
    let node = nominal("Node", vec![Ty::TypeParameter { index: parameter }]);
    let declarations = [
        (
            "UnitStruct",
            vec![],
            ArcweftRustTypeKind::Struct {
                shape: ArcweftRustStructShape::Unit,
            },
        ),
        (
            "Tuple0",
            vec![],
            ArcweftRustTypeKind::Struct {
                shape: ArcweftRustStructShape::Tuple { fields: vec![] },
            },
        ),
        (
            "Record0",
            vec![],
            ArcweftRustTypeKind::Struct {
                shape: ArcweftRustStructShape::Record { fields: vec![] },
            },
        ),
        (
            "Newtype",
            vec![],
            ArcweftRustTypeKind::Newtype { inner: Ty::Bool },
        ),
        (
            "Node",
            vec![ArcweftRustTypeParameter {
                index: parameter,
                name: arcweft_rust_abi::ArcweftRustTypeParameterName::try_new("T").unwrap(),
            }],
            ArcweftRustTypeKind::Struct {
                shape: ArcweftRustStructShape::Record {
                    fields: vec![
                        ArcweftRustField {
                            wire_name: None,
                            bytes_format: None,
                            default: None,
                            skip: false,
                            name: "value".to_owned(),
                            ty: Ty::TypeParameter { index: parameter },
                        },
                        ArcweftRustField {
                            wire_name: None,
                            bytes_format: None,
                            default: None,
                            skip: false,
                            name: "next".to_owned(),
                            ty: Ty::Option {
                                item: Box::new(node),
                            },
                        },
                    ],
                },
            },
        ),
        (
            "Cases",
            vec![],
            ArcweftRustTypeKind::Enum {
                variants: vec![
                    ArcweftRustVariant {
                        wire_name: None,
                        discriminant: None,
                        name: "Unit".to_owned(),
                        payload: Payload::Unit,
                    },
                    ArcweftRustVariant {
                        wire_name: None,
                        discriminant: None,
                        name: "Tuple0".to_owned(),
                        payload: Payload::Tuple { fields: vec![] },
                    },
                    ArcweftRustVariant {
                        wire_name: None,
                        discriminant: None,
                        name: "Record0".to_owned(),
                        payload: Payload::Record { fields: vec![] },
                    },
                    ArcweftRustVariant {
                        wire_name: None,
                        discriminant: None,
                        name: "Node".to_owned(),
                        payload: Payload::Tuple {
                            fields: vec![nominal("Node", vec![Ty::Bool])],
                        },
                    },
                ],
            },
        ),
    ];
    let mut rust = ArcweftRustManifest::new(ArcweftRustPackage {
        id: package(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    });
    for (name, parameters, kind) in declarations {
        rust = rust.with_type(ArcweftRustTypeDecl {
            data_policy: None,
            path: path(name),
            rust_path: format!("compiler_rust_nominals::{name}"),
            parameters,
            kind,
        });
    }
    for (name, result) in [
        ("unit_value", nominal("UnitStruct", vec![])),
        ("tuple_value", nominal("Tuple0", vec![])),
        ("record_value", nominal("Record0", vec![])),
        ("newtype_value", nominal("Newtype", vec![])),
        ("bool_node", nominal("Node", vec![Ty::Bool])),
        ("integer_node", nominal("Node", vec![Ty::I32])),
        ("cases_value", nominal("Cases", vec![])),
    ] {
        rust = rust.with_function(ArcweftRustFunction {
            role: Default::default(),
            name: name.to_owned(),
            rust_path: format!("compiler_rust_nominals::{name}"),
            params: vec![],
            return_type: result,
            purity: ArcweftRustPurity::Pure,
            effects: vec![],
        });
    }
    AdapterManifest::new("rust-nominals", "Rust nominal integration")
        .try_with_rust_package_mount(package(), AdapterNominalPathPrefix::try_new([]).unwrap())
        .unwrap()
        .try_with_rust_manifest(&rust)
        .unwrap()
}

#[test]
fn source_rust_nominals_preserve_shapes_recursion_and_exact_arguments_in_native_and_awbc() {
    let source = "flow main {\nlet u = unit_value();\nlet t = tuple_value();\nlet r = record_value();\nlet n = newtype_value();\nlet b = bool_node();\nlet i = integer_node();\nlet c = cases_value();\n}\nentry cli @entry.cli.main { goto @flow.main }\n";
    let (project, registration, env) =
        fixture_with_manifest(source, "rust-nominal-runtime", &fixture_manifest());
    let mut session = AttachedCompiler::new(&project);
    let compiled = session
        .compile(
            &project,
            &context(env, registration),
            &mut RecordingCache::default(),
        )
        .unwrap();
    let lowered = compiled.runtime_plan();
    let plan = &lowered.plan;
    let program = AwbcLowerer::new(plan, &lowered.dialogue_content_catalog, "rust-nominals")
        .lower()
        .unwrap()
        .program;
    let program = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    for (shape, count) in [
        (Shape::Unit, 1),
        (Shape::Tuple, 1),
        (Shape::Newtype, 1),
        (Shape::Record, 3),
    ] {
        assert_eq!(
            plan.nominal_record_domains()
                .domains()
                .filter(|domain| domain.shape() == shape)
                .count(),
            count
        );
    }

    let limits = RuntimeSchemaLimits::engine_default();
    let mut nodes = Vec::new();
    for domain in plan
        .nominal_record_domains()
        .domains()
        .filter(|domain| domain.fields().len() == 2)
    {
        let row = plan.type_table().get(domain.owner()).unwrap();
        let RuntimePlanTypeProjection::Nominal { .. } = row.projection() else {
            panic!("source nominal row");
        };
        let (valid, invalid) = match plan
            .type_table()
            .get(domain.fields()[0].ty())
            .unwrap()
            .projection()
        {
            RuntimePlanTypeProjection::Bool => (RuntimeValue::Bool(true), RuntimeValue::i32(1)),
            RuntimePlanTypeProjection::Signed(arcweft_core::value::RuntimeSignedIntWidth::I32) => {
                (RuntimeValue::i32(1), RuntimeValue::Bool(true))
            }
            other => panic!("unexpected instantiated field: {other:?}"),
        };
        let node = |value, next| {
            arcweft_core::program_types::RuntimeProgramTypes::Plan(plan)
                .try_record_value(row.semantic_identity(), vec![value, next], limits)
                .unwrap()
        };
        let leaf = node(valid.clone(), RuntimeValue::option_none());
        let value = node(valid.clone(), RuntimeValue::option_some(leaf));
        let RuntimeValue::NominalRecord(leaf) = node(valid, RuntimeValue::option_none()) else {
            unreachable!();
        };
        let mut fields = leaf.fields().iter().cloned();
        let _ = fields.next();
        let mut replacement = vec![invalid, fields.next().unwrap()].into_iter();
        let wrong_leaf = leaf.map_fields(|_| replacement.next().unwrap());
        let wrong = value
            .clone()
            .as_nominal_record()
            .unwrap()
            .clone()
            .map_fields({
                let mut fields = vec![
                    value.as_nominal_record().unwrap().fields()[0].clone(),
                    RuntimeValue::option_some(RuntimeValue::NominalRecord(wrong_leaf)),
                ]
                .into_iter();
                move |_| fields.next().unwrap()
            });
        let wrong = RuntimeValue::NominalRecord(wrong);
        let awbc_id = program
            .runtime_types
            .iter()
            .position(|ty| ty.semantic_identity() == row.semantic_identity())
            .unwrap();
        let awbc_id = AwbcTypeId(u32::try_from(awbc_id).unwrap());
        let native = plan.accepts_value(domain.owner(), &value, limits).unwrap();
        assert_eq!(
            program.accepts_value(awbc_id, &value, limits).unwrap(),
            native
        );
        assert!(plan.accepts_value(domain.owner(), &wrong, limits).is_err());
        assert!(program.accepts_value(awbc_id, &wrong, limits).is_err());
        nodes.push(row.semantic_identity());
    }
    assert_eq!(nodes.len(), 2);
    assert_ne!(nodes[0], nodes[1]);
    let cases = plan
        .variant_domains()
        .domains()
        .find(|domain| domain.cases().len() == 4)
        .unwrap();
    assert_eq!(
        cases
            .cases()
            .iter()
            .map(arcweft_core::plan::RuntimeVariantCase::name)
            .collect::<Vec<_>>(),
        ["Unit", "Tuple0", "Record0", "Node"]
    );
    assert!(cases.cases()[0].payload().is_none());
    assert!(
        cases.cases()[1..]
            .iter()
            .all(|case| case.payload().is_some())
    );
    let identity = plan
        .type_table()
        .get(cases.owner())
        .unwrap()
        .semantic_identity();
    let awbc = AwbcTypeId(
        u32::try_from(
            program
                .runtime_types
                .iter()
                .position(|ty| ty.semantic_identity() == identity)
                .unwrap(),
        )
        .unwrap(),
    );
    let tuple = RuntimeValue::Tuple(vec![]);
    let record =
        RuntimeValue::Record(arcweft_core::value::RuntimeRecordValue::try_new(vec![]).unwrap());
    for (ordinal, correct, incorrect) in [
        (0, None, Some(RuntimeValue::Unit)),
        (1, Some(tuple.clone()), Some(record.clone())),
        (2, Some(record), Some(tuple)),
    ] {
        let correct = arcweft_core::program_types::RuntimeProgramTypes::Plan(&plan)
            .try_variant_value(identity, ordinal, correct, limits)
            .unwrap();
        assert_eq!(
            plan.accepts_value(cases.owner(), &correct, limits).unwrap(),
            program.accepts_value(awbc, &correct, limits).unwrap()
        );
        for authority in [
            arcweft_core::program_types::RuntimeProgramTypes::Plan(&plan),
            arcweft_core::program_types::RuntimeProgramTypes::Awbc(&program),
        ] {
            assert!(
                authority
                    .try_variant_value(identity, ordinal, incorrect.clone(), limits)
                    .is_err()
            );
        }
    }
}

#[test]
fn recursive_rust_nominal_handler_capture_executes_with_its_original_program() {
    use arcweft_bundle::resource_codec::{
        ViewProductValidationLimits, view::ViewProgramInstruction,
    };
    use arcweft_core::{
        awbc::{product_step::AwbcProductProgram, verify::AwbcVerifyBudget},
        plan::RuntimeTaskSourceArtifactLimits,
        program_types::RuntimeProgramTypes,
        pure::{RuntimeProgramExecutionContext, VmRuntimePureCallBackend},
        task::RuntimeProgramOwner,
        value::AwbcRuntimeValueSnapshot,
    };

    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main(node: Node<bool>) {
    Button().on_click { let observed = node; let again = node; () }
}
"#;
    let (project, registration, env) =
        fixture_with_manifest(source, "rust-nominal-handler-capture", &fixture_manifest());
    let compiled = AttachedCompiler::new(&project)
        .compile(
            &project,
            &context(env, registration),
            &mut RecordingCache::default(),
        )
        .expect("a recursive Rust data capture is snapshot-retainable");
    let view = compiled.view_product().product().program().unwrap();
    let handler = view
        .resource()
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::BindHandler { handler, .. } => Some(*handler),
            _ => None,
        })
        .unwrap();
    let specification = view.resource().handler_ref(handler).unwrap();
    assert_eq!(specification.captures.len(), 1);
    assert_eq!(specification.captures[0].parameter().unwrap().value(), 0);
    let lowered = compiled.runtime_plan();
    let plan = Arc::new(lowered.plan.clone());
    let binding = plan
        .pure_programs()
        .iter()
        .find(|binding| binding.program() == handler)
        .unwrap();
    let [identity] = binding.input_types() else {
        panic!("the retained handler has exactly its Rust capture input");
    };
    let limits = RuntimeSchemaLimits::engine_default();
    let node = |next| {
        RuntimeProgramTypes::Plan(&plan)
            .try_record_value(*identity, vec![RuntimeValue::Bool(true), next], limits)
            .unwrap()
    };
    let input = node(RuntimeValue::option_some(node(RuntimeValue::option_none())));
    let native_owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let image =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&input, &native_owner).unwrap();
    let awbc = AwbcLowerer::new(&plan, &lowered.dialogue_content_catalog, "rust-capture")
        .lower()
        .unwrap()
        .program;
    let awbc = Arc::new(
        AwbcProgram::decode_canonical(
            &awbc.encode_canonical().unwrap(),
            AwbcDecodeBudget::default(),
        )
        .unwrap(),
    );
    let authority = view
        .task_source_authority(ViewProductValidationLimits::default())
        .unwrap();
    let admitted = AwbcProductProgram::admit(
        Arc::clone(&awbc),
        AwbcVerifyBudget::default(),
        arcweft_core::awbc::verify::AwbcVerifyContext {
            require_entrypoint: false,
            ..Default::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        Some(&authority),
    )
    .unwrap();
    let awbc_owner = RuntimeProgramOwner::Awbc(Arc::clone(&awbc));
    let restored = image.into_runtime_value_for_program(&awbc_owner).unwrap();
    let native = RuntimeProgramExecutionContext::native(plan, Default::default())
        .evaluate_pure_program(handler, &[input], &mut VmRuntimePureCallBackend::default())
        .unwrap();
    let decoded = RuntimeProgramExecutionContext::awbc(admitted, Default::default())
        .evaluate_pure_program(
            handler,
            &[restored],
            &mut VmRuntimePureCallBackend::default(),
        )
        .unwrap();
    assert_eq!(native, RuntimeValue::Tuple(vec![RuntimeValue::Unit]));
    assert_eq!(decoded, native);
}
