//! Rust declarations, accepted callables and codec defaults share one producer.

use super::*;
use std::{cell::RefCell, collections::BTreeMap};

use arcweft_adapter_context::manifest::AdapterNominalPathPrefix;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, schema::AwbcProgram},
    entry::{RuntimeCallableId, RuntimeSchemaLimits},
    pattern::RuntimeSemanticTypeId,
    plan::RuntimePlan,
    program_types::{RuntimeProgramDataShapes, RuntimeProgramTypes},
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext, VmRuntimePureCallBackend},
    task::RuntimeProgramOwner,
    value::{RuntimeCallTarget, RuntimeEvalError, RuntimeExprKind, RuntimeValue},
};
use arcweft_data::{
    DecodeShapeAccess, FieldDefaultProvider, FieldDefaultRequest, RawValue, Value,
    decode_with_shape_ref,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_rust_abi::{
    ArcweftRustManifest, ArcweftRustPackage, ArcweftRustPackageId, ArcweftRustPurity,
    ArcweftRustTypeRef,
};
use arcweft_rust_abi_macros::{ArcweftType, arcweft_export, arcweft_export_default};

arcweft_export_default!(pure, pub fn default_bool() -> bool);
arcweft_export_default!(pure, pub fn default_string() -> String);

#[derive(ArcweftType)]
pub struct PrimitiveDefaults {
    #[arcweft(default)]
    flag: bool,
    #[arcweft(default)]
    text: String,
}

#[derive(ArcweftType)]
pub struct GenericDefault<T> {
    #[arcweft(default)]
    value: T,
}

#[arcweft_export(pure)]
fn primitive_defaults() -> PrimitiveDefaults {
    PrimitiveDefaults {
        flag: default_bool(),
        text: default_string(),
    }
}

#[arcweft_export(pure)]
fn generic_default() -> GenericDefault<bool> {
    GenericDefault {
        value: default_bool(),
    }
}

#[derive(ArcweftType)]
pub struct Marker(bool);

#[arcweft_export(pure, name = "default_marker")]
impl Default for Marker {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(ArcweftType)]
pub struct Settings {
    #[arcweft(default = "default_flag")]
    flag: bool,
    #[arcweft(default)]
    marker: Marker,
    #[arcweft(skip)]
    cache: Marker,
}

#[arcweft_export(pure)]
fn default_flag() -> bool {
    true
}

#[arcweft_export(pure)]
fn settings() -> Settings {
    Settings {
        flag: default_flag(),
        marker: default_marker(),
        cache: default_marker(),
    }
}

fn metadata() -> ArcweftRustManifest {
    ArcweftRustManifest::builder(ArcweftRustPackage {
        id: ArcweftRustPackageId::try_new(env!("CARGO_PKG_NAME")).unwrap(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        metadata_hash: None,
    })
    .with_type_metadata::<Marker>()
    .with_type_metadata::<Settings>()
    .with_type_metadata::<PrimitiveDefaults>()
    .with_type_metadata::<GenericDefault<bool>>()
    .with_function(__arcweft_export_default_bool_metadata())
    .with_function(__arcweft_export_default_string_metadata())
    .with_function(__arcweft_export_primitive_defaults_metadata())
    .with_function(__arcweft_export_generic_default_metadata())
    .with_function(__arcweft_export_default_marker_metadata())
    .with_function(__arcweft_export_default_flag_metadata())
    .with_function(__arcweft_export_settings_metadata())
    .build()
}

fn compile_defaults(
    rust: &ArcweftRustManifest,
) -> Result<(Arc<RuntimePlan>, Arc<AwbcProgram>), String> {
    compile_rust(
        rust,
        "flow main { let value = settings(); }\nentry cli @entry.cli.main { goto @flow.main }\n",
    )
}

pub(super) fn compile_rust(
    rust: &ArcweftRustManifest,
    source: &str,
) -> Result<(Arc<RuntimePlan>, Arc<AwbcProgram>), String> {
    let manifest = AdapterManifest::new("rust-defaults", "Rust default integration")
        .try_with_rust_package_mount(
            rust.package.id.clone(),
            AdapterNominalPathPrefix::try_new([]).unwrap(),
        )
        .unwrap()
        .try_with_rust_manifest(rust)
        .map_err(|error| error.to_string())?;
    let (project, registration, env) =
        fixture_with_manifest(source, "rust-default-runtime", &manifest);
    let compiled = AttachedCompiler::new(&project)
        .compile(
            &project,
            &context(env, registration),
            &mut RecordingCache::default(),
        )
        .map_err(|error| format!("{error:?}"))?;
    let lowered = compiled.runtime_plan();
    let program = AwbcLowerer::new(
        &lowered.plan,
        &lowered.dialogue_content_catalog,
        "rust-defaults",
    )
    .lower()
    .map_err(|error| format!("{error:?}"))?
    .program;
    let wire = program
        .encode_canonical()
        .map_err(|error| error.to_string())?;
    let program = AwbcProgram::decode_canonical(&wire, AwbcDecodeBudget::default())
        .map_err(|error| error.to_string())?;
    Ok((Arc::new(lowered.plan.clone()), Arc::new(program)))
}

#[derive(Clone, Copy)]
enum Factory {
    Flag,
    Marker,
    BoolDefault,
    StringDefault,
}

struct RustDefaults {
    owner: RuntimeProgramOwner,
    factories: BTreeMap<RuntimeCallableId, Factory>,
    wrong_result: bool,
}

impl RuntimeExternalCallBackend for RustDefaults {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        target: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let RuntimeCallTarget::Callable(target) = target else {
            return None;
        };
        let factory = self.factories.get(target)?;
        assert!(args.is_empty());
        assert_eq!(context.argument_types(), Some([].as_slice()));
        assert!(match (context.program_owner().unwrap(), &self.owner) {
            (RuntimeProgramOwner::Plan(left), RuntimeProgramOwner::Plan(right)) =>
                Arc::ptr_eq(left, right),
            (RuntimeProgramOwner::Awbc(left), RuntimeProgramOwner::Awbc(right)) =>
                Arc::ptr_eq(left, right),
            _ => false,
        });
        if self.wrong_result {
            return Some(Ok(RuntimeValue::String("tampered result".to_owned())));
        }
        let types = selected_types(&self.owner);
        Some(Ok(match factory {
            Factory::Flag => RuntimeValue::Bool(default_flag()),
            Factory::BoolDefault => RuntimeValue::Bool(default_bool()),
            Factory::StringDefault => RuntimeValue::String(default_string()),
            Factory::Marker => types
                .try_record_value(
                    context.result_type().unwrap(),
                    vec![RuntimeValue::Bool(default_marker().0)],
                    RuntimeSchemaLimits::engine_default(),
                )
                .unwrap(),
        }))
    }
}

fn selected_types(owner: &RuntimeProgramOwner) -> RuntimeProgramTypes<'_> {
    match owner {
        RuntimeProgramOwner::Plan(plan) => RuntimeProgramTypes::Plan(plan),
        RuntimeProgramOwner::Awbc(program) => RuntimeProgramTypes::Awbc(program),
    }
}

struct Defaults<'a> {
    owner: &'a RuntimeProgramOwner,
    shapes: RuntimeProgramDataShapes<'a>,
    backend: RefCell<VmRuntimePureCallBackend<RustDefaults>>,
    calls: RefCell<Vec<usize>>,
}

impl FieldDefaultProvider for Defaults<'_> {
    fn default_value(&self, field: FieldDefaultRequest<'_>) -> arcweft_data::Result<Value> {
        self.calls.borrow_mut().push(field.field_ordinal());
        let request = self
            .shapes
            .field_default_request(field.record_id().unwrap(), field.field_ordinal())
            .unwrap()
            .unwrap();
        let mut backend = self.backend.borrow_mut();
        let value = match self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                arcweft_core::pure::evaluate_pure_program_with_backend(
                    plan,
                    request.program(),
                    &[],
                    &mut *backend,
                )
                .map_err(|error| error.to_string())
            }
            RuntimeProgramOwner::Awbc(program) => {
                arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                    program,
                    request.program(),
                    &[],
                    &mut *backend,
                )
                .map_err(|error| error.to_string())
            }
        }
        .map_err(|message| {
            arcweft_data::DataError::new(arcweft_data::DataErrorKind::InvalidType, message)
        })?;
        request
            .validate_result(&value, RuntimeSchemaLimits::engine_default())
            .unwrap();
        // These two source factories have exact Bool and Marker(Bool) result
        // contracts. The generic accelerator owns the exhaustive conversion.
        Ok(match value {
            RuntimeValue::Bool(value) => Value::Bool(value),
            RuntimeValue::String(value) => Value::String(value),
            RuntimeValue::NominalRecord(record) => {
                let RuntimeValue::Bool(value) = record.fields()[0] else {
                    panic!("Marker's admitted Bool field")
                };
                Value::Bool(value)
            }
            _ => panic!("the runtime result was admitted against the exact field type"),
        })
    }
}

fn default_contract(
    plan: &RuntimePlan,
) -> (RuntimeSemanticTypeId, BTreeMap<RuntimeCallableId, Factory>) {
    let domain = plan
        .nominal_record_domains()
        .domains()
        .find(|domain| domain.fields().len() == 3)
        .unwrap();
    let semantic = plan
        .type_table()
        .get(domain.owner())
        .unwrap()
        .semantic_identity();
    let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Plan(plan));
    let root = shapes
        .root(semantic, RuntimeSchemaLimits::engine_default())
        .unwrap()
        .referenced_id()
        .unwrap();
    let mut factories = BTreeMap::new();
    for (ordinal, factory) in [
        (0, Factory::Flag),
        (1, Factory::Marker),
        (2, Factory::Marker),
    ] {
        let request = shapes
            .field_default_request(root, ordinal)
            .unwrap()
            .unwrap();
        let binding = plan
            .pure_programs()
            .iter()
            .find(|binding| binding.program() == request.program())
            .unwrap();
        let RuntimeExprKind::Call {
            callee: RuntimeCallTarget::Callable(target),
            args,
        } = plan.pure_helpers()[binding.helper().0].expr.kind()
        else {
            panic!("accepted Rust callable wrapper")
        };
        assert!(args.is_empty());
        factories.insert(target.clone(), factory);
    }
    (semantic, factories)
}

#[test]
fn rust_declared_defaults_reach_plan_awbc_and_execute_only_on_missing_fields() {
    let (plan, awbc) = compile_defaults(&metadata()).unwrap();
    let (semantic, factories) = default_contract(&plan);
    assert_eq!(
        plan.pure_programs().len(),
        2,
        "one binding per admitted producer"
    );
    let source_value = settings();
    assert!(source_value.flag && source_value.marker.0 && source_value.cache.0);
    for owner in [
        RuntimeProgramOwner::Plan(plan),
        RuntimeProgramOwner::Awbc(awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(selected_types(&owner));
        let root = shapes
            .root(semantic, RuntimeSchemaLimits::engine_default())
            .unwrap();
        let provider = Defaults {
            owner: &owner,
            shapes: RuntimeProgramDataShapes::new(selected_types(&owner)),
            backend: RefCell::new(VmRuntimePureCallBackend::default().with_external_calls(
                RustDefaults {
                    owner: owner.clone(),
                    factories: factories.clone(),
                    wrong_result: false,
                },
            )),
            calls: RefCell::new(vec![]),
        };
        let absent = RawValue::Map(vec![]);
        assert!(decode_with_shape_ref(&absent, root, &shapes).is_err());
        let access = DecodeShapeAccess::new(&shapes, &provider);
        let value = decode_with_shape_ref(&absent, root, &access).unwrap();
        assert_eq!(*provider.calls.borrow(), [0, 1, 2]);
        assert!(
            value
                .as_record()
                .unwrap()
                .values()
                .all(|value| value == &Value::Bool(true))
        );
        provider.calls.borrow_mut().clear();
        let present = RawValue::Map(vec![
            (RawValue::String("flag".to_owned()), RawValue::Bool(false)),
            (RawValue::String("marker".to_owned()), RawValue::Bool(false)),
            (RawValue::String("cache".to_owned()), RawValue::Bool(false)),
        ]);
        let value = decode_with_shape_ref(&present, root, &access).unwrap();
        assert_eq!(
            *provider.calls.borrow(),
            [2],
            "skip ignores input and executes its declared producer"
        );
        assert_eq!(value.as_record().unwrap()["flag"], Value::Bool(false));
        let invalid = Defaults {
            owner: &owner,
            shapes: RuntimeProgramDataShapes::new(selected_types(&owner)),
            backend: RefCell::new(VmRuntimePureCallBackend::default().with_external_calls(
                RustDefaults {
                    owner: owner.clone(),
                    factories: factories.clone(),
                    wrong_result: true,
                },
            )),
            calls: RefCell::new(vec![]),
        };
        assert!(
            decode_with_shape_ref(&absent, root, &DecodeShapeAccess::new(&shapes, &invalid))
                .is_err(),
            "tampered factory results cannot publish a typed field"
        );
    }
}

#[test]
fn rust_default_producer_tampering_fails_before_runtime_publication() {
    for mutation in 0..6 {
        let mut rust = metadata();
        let factory = rust
            .functions
            .iter_mut()
            .find(|function| function.name == "default_flag")
            .unwrap();
        match mutation {
            0 => {
                rust.functions
                    .retain(|function| function.name != "default_flag");
            }
            1 => factory.purity = ArcweftRustPurity::External,
            2 => factory.return_type = ArcweftRustTypeRef::String,
            3 => factory.params.push(arcweft_rust_abi::ArcweftRustParam {
                name: "unexpected".to_owned(),
                ty: ArcweftRustTypeRef::Bool,
            }),
            4 => {
                let mut duplicate = factory.clone();
                duplicate.name = "other_flag".to_owned();
                rust.functions.push(duplicate);
            }
            5 => factory.effects.push("io.read".to_owned()),
            _ => unreachable!(),
        }
        assert!(
            compile_defaults(&rust).is_err(),
            "mutation {mutation} cannot issue a default program"
        );
    }
}

const PRIMITIVE_SOURCE: &str = "flow main { let a = primitive_defaults(); let b = generic_default(); }\nentry cli @entry.cli.main { goto @flow.main }\n";

#[test]
fn rust_defaults_for_primitive_foreign_and_generic_fields_share_checked_bindings() {
    let (plan, awbc) = compile_rust(&metadata(), PRIMITIVE_SOURCE).unwrap();
    assert_eq!(plan.pure_programs().len(), 2);
    assert!(!primitive_defaults().flag && primitive_defaults().text.is_empty());
    assert!(!generic_default().value);
    let mut records = Vec::new();
    let mut factories = BTreeMap::new();
    let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Plan(&plan));
    for domain in plan.nominal_record_domains().domains() {
        let Some(codec) = domain.data_codec() else {
            continue;
        };
        let arcweft_core::entry::RuntimeCodecUse::Record { name, .. } = &codec.body else {
            continue;
        };
        if !matches!(name.as_str(), "PrimitiveDefaults" | "GenericDefault") {
            continue;
        }
        let semantic = plan
            .type_table()
            .get(domain.owner())
            .unwrap()
            .semantic_identity();
        let root = shapes
            .root(semantic, RuntimeSchemaLimits::engine_default())
            .unwrap()
            .referenced_id()
            .unwrap();
        for ordinal in 0..domain.fields().len() {
            let request = shapes
                .field_default_request(root, ordinal)
                .unwrap()
                .unwrap();
            let binding = plan
                .pure_programs()
                .iter()
                .find(|binding| binding.program() == request.program())
                .unwrap();
            let RuntimeExprKind::Call {
                callee: RuntimeCallTarget::Callable(target),
                ..
            } = plan.pure_helpers()[binding.helper().0].expr.kind()
            else {
                panic!("declared Rust wrapper")
            };
            factories.insert(
                target.clone(),
                if name == "PrimitiveDefaults" && ordinal == 1 {
                    Factory::StringDefault
                } else {
                    Factory::BoolDefault
                },
            );
        }
        records.push((name.clone(), semantic));
    }
    assert_eq!(records.len(), 2);
    for owner in [
        RuntimeProgramOwner::Plan(plan),
        RuntimeProgramOwner::Awbc(awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(selected_types(&owner));
        let provider = Defaults {
            owner: &owner,
            shapes: RuntimeProgramDataShapes::new(selected_types(&owner)),
            backend: RefCell::new(VmRuntimePureCallBackend::default().with_external_calls(
                RustDefaults {
                    owner: owner.clone(),
                    factories: factories.clone(),
                    wrong_result: false,
                },
            )),
            calls: RefCell::new(vec![]),
        };
        for (name, semantic) in &records {
            let value = decode_with_shape_ref(
                &RawValue::Map(vec![]),
                shapes
                    .root(*semantic, RuntimeSchemaLimits::engine_default())
                    .unwrap(),
                &DecodeShapeAccess::new(&shapes, &provider),
            )
            .unwrap();
            let expected = if name == "PrimitiveDefaults" {
                BTreeMap::from([
                    ("flag".to_owned(), Value::Bool(false)),
                    ("text".to_owned(), Value::String(String::new())),
                ])
            } else {
                BTreeMap::from([("value".to_owned(), Value::Bool(false))])
            };
            assert_eq!(value, Value::Record(expected));
        }
    }
}

#[test]
fn rust_trait_default_bindings_reject_missing_wrong_and_duplicate_producers() {
    for mutation in 0..3 {
        let mut rust = metadata();
        let factory = rust
            .functions
            .iter_mut()
            .find(|function| function.name == "default_bool")
            .unwrap();
        match mutation {
            0 => rust
                .functions
                .retain(|function| function.name != "default_bool"),
            1 => factory.return_type = ArcweftRustTypeRef::String,
            2 => {
                let mut duplicate = factory.clone();
                duplicate.name = "other_default_bool".to_owned();
                rust.functions.push(duplicate);
            }
            _ => unreachable!(),
        }
        assert!(
            compile_rust(&rust, PRIMITIVE_SOURCE).is_err(),
            "trait witness mutation {mutation}"
        );
    }
}
