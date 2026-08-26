//! Standard adapter manifests bundled with Arcweft tooling.

use crate::manifest::{
    AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
    AdapterCallableParameterIndex, AdapterEffectCapability, AdapterEnvironmentOwnerId,
    AdapterFunctionParam, AdapterFunctionSignature, AdapterHostCall, AdapterId, AdapterManifest,
    AdapterNominalDeclaration, AdapterNominalOwner, AdapterNominalPath, AdapterNominalPathSegment,
    AdapterNominalTypeRef, AdapterNominalVisibility, AdapterOpaqueTypeProducerId,
    AdapterParameterGroup, AdapterParameterPassing, AdapterParameterPresence, AdapterRegistry,
    AdapterSymbol, AdapterSymbolPath, AdapterSymbolSegment, AdapterTypeKind,
};

/// Adapter id for the default Sans I/O environment.
pub const SANS_IO_ADAPTER_ID: &str = "sans-io";

/// Adapter id for the native HTTP server environment.
pub const NATIVE_HTTP_ADAPTER_ID: &str = "native-http";

/// Adapter id for the native command-line process environment.
pub const NATIVE_CLI_ADAPTER_ID: &str = "native-cli";

/// Adapter id for optional tensor inference helpers.
pub const INFERENCE_TENSOR_ADAPTER_ID: &str = "inference-tensor";

/// Adapter id for host system information.
pub const SYSTEM_INFO_ADAPTER_ID: &str = "system-info";

/// Adapter id for native filesystem access.
pub const NATIVE_FILE_ADAPTER_ID: &str = "native-file";

/// Adapter id for host math accelerators.
pub const MATH_ADAPTER_ID: &str = "math";

/// Registry containing all standard adapter manifests.
pub fn standard_registry() -> AdapterRegistry {
    AdapterRegistry::from_manifests([
        sans_io_manifest(),
        native_cli_manifest(),
        native_http_manifest(),
        inference_tensor_manifest(),
        system_info_manifest(),
        native_file_manifest(),
        math_manifest(),
    ])
}

/// Default Sans I/O manifest.
pub fn sans_io_manifest() -> AdapterManifest {
    AdapterManifest::new(SANS_IO_ADAPTER_ID, "Sans I/O")
}

/// Native command-line process manifest.
pub fn native_cli_manifest() -> AdapterManifest {
    AdapterManifest::new(NATIVE_CLI_ADAPTER_ID, "Native CLI").with_host_call(
        AdapterHostCall::with_signature(
            "cli.args",
            signature(
                [],
                AdapterTypeKind::Vec {
                    item: Box::new(AdapterTypeKind::String),
                },
            ),
            [],
        ),
    )
}

/// Native HTTP server manifest.
pub fn native_http_manifest() -> AdapterManifest {
    declare_nominals(
        AdapterManifest::new(NATIVE_HTTP_ADAPTER_ID, "Native HTTP"),
        ["HttpRequestContext"],
        "arcweft.adapter.native-http",
    )
    .with_symbol(adapter_symbol(
        ["request"],
        environment_nominal(NATIVE_HTTP_ADAPTER_ID, "HttpRequestContext"),
    ))
    .with_effect(AdapterEffectCapability::new("http.respond"))
    .with_host_call(AdapterHostCall::new(
        "http.respond",
        [AdapterEffectCapability::new("http.respond")],
    ))
}

/// Optional forward-inference tensor manifest.
pub fn inference_tensor_manifest() -> AdapterManifest {
    let tensor = inference_nominal("TensorF32");
    let manifest = declare_nominals(
        AdapterManifest::new(INFERENCE_TENSOR_ADAPTER_ID, "Inference Tensor"),
        ["Conv2dApi", "InferApi", "TensorF32"],
        "arcweft.adapter.inference-tensor",
    )
    .with_symbol(adapter_symbol(["conv2d"], inference_nominal("Conv2dApi")))
    .with_symbol(adapter_symbol(["infer"], inference_nominal("InferApi")));
    let manifest = with_conv2d_callable(manifest, &tensor);
    let manifest = with_inference_callables(manifest, &tensor);
    with_inference_host_calls(manifest)
}

fn with_conv2d_callable(manifest: AdapterManifest, tensor: &AdapterTypeKind) -> AdapterManifest {
    manifest.with_method_signature(
        inference_nominal("Conv2dApi"),
        callable_name("valid_f32"),
        overload_zero(),
        signature(
            [
                ("input", tensor.clone()),
                ("kernel", tensor.clone()),
                ("stride_y", AdapterTypeKind::USize),
                ("stride_x", AdapterTypeKind::USize),
            ],
            tensor.clone(),
        ),
        [],
    )
}

fn with_inference_callables(
    manifest: AdapterManifest,
    tensor: &AdapterTypeKind,
) -> AdapterManifest {
    manifest
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("matmul_f32"),
            overload_zero(),
            signature(
                [("lhs", tensor.clone()), ("rhs", tensor.clone())],
                tensor.clone(),
            ),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("add_f32"),
            overload_zero(),
            signature(
                [("lhs", tensor.clone()), ("rhs", tensor.clone())],
                tensor.clone(),
            ),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("bias_add_f32"),
            overload_zero(),
            signature(
                [("tensor", tensor.clone()), ("bias", tensor.clone())],
                tensor.clone(),
            ),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("matmul_bias_add_f32"),
            overload_zero(),
            signature(
                [
                    ("lhs", tensor.clone()),
                    ("rhs", tensor.clone()),
                    ("bias", tensor.clone()),
                ],
                tensor.clone(),
            ),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("relu_f32"),
            overload_zero(),
            signature([("input", tensor.clone())], tensor.clone()),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("max_pool2d_f32"),
            overload_zero(),
            signature(
                [
                    ("input", tensor.clone()),
                    ("kernel_y", AdapterTypeKind::USize),
                    ("kernel_x", AdapterTypeKind::USize),
                    ("stride_y", AdapterTypeKind::USize),
                    ("stride_x", AdapterTypeKind::USize),
                ],
                tensor.clone(),
            ),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("softmax_last_dim_f32"),
            overload_zero(),
            signature([("input", tensor.clone())], tensor.clone()),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("argmax_last_dim_f32"),
            overload_zero(),
            signature(
                [("input", tensor.clone())],
                AdapterTypeKind::Seq {
                    item: Box::new(AdapterTypeKind::USize),
                },
            ),
            [],
        )
        .with_method_signature(
            inference_nominal("InferApi"),
            callable_name("flatten_outer_f32"),
            overload_zero(),
            signature([("input", tensor.clone())], tensor.clone()),
            [],
        )
}

fn with_inference_host_calls(manifest: AdapterManifest) -> AdapterManifest {
    manifest
        .with_host_call(AdapterHostCall::new("conv2d.valid_f32", []))
        .with_host_call(AdapterHostCall::new("infer.matmul_f32", []))
        .with_host_call(AdapterHostCall::new("infer.add_f32", []))
        .with_host_call(AdapterHostCall::new("infer.bias_add_f32", []))
        .with_host_call(AdapterHostCall::new("infer.matmul_bias_add_f32", []))
        .with_host_call(AdapterHostCall::new("infer.relu_f32", []))
        .with_host_call(AdapterHostCall::new("infer.max_pool2d_f32", []))
        .with_host_call(AdapterHostCall::new("infer.softmax_last_dim_f32", []))
        .with_host_call(AdapterHostCall::new("infer.argmax_last_dim_f32", []))
        .with_host_call(AdapterHostCall::new("infer.flatten_outer_f32", []))
}

/// Host system information manifest.
///
/// # Panics
///
/// Panics only if Arcweft's fixed standard IDs or declaration inventory violate
/// their own checked constructors.
pub fn system_info_manifest() -> AdapterManifest {
    let effect = AdapterEffectCapability::new("system.read");
    let system_error = environment_nominal_path(SYSTEM_INFO_ADAPTER_ID, ["system", "SystemError"]);
    AdapterManifest::new(SYSTEM_INFO_ADAPTER_ID, "System Info")
        .try_with_nominal_declaration(
            AdapterNominalDeclaration::try_new(
                nominal_path_segments(["system", "SystemError"]),
                0,
                AdapterOpaqueTypeProducerId::try_new("arcweft.adapter.system-info")
                    .expect("standard opaque producer IDs are valid"),
                AdapterNominalVisibility::Public,
                "system.SystemError",
            )
            .expect("standard SystemError declaration is valid"),
        )
        .expect("standard SystemError declaration is unique")
        .with_effect(effect.clone())
        .with_host_call(
            AdapterHostCall::with_signature(
                "system.core_count",
                signature(
                    [],
                    fallible_need(AdapterTypeKind::String, system_error.clone()),
                ),
                [effect.clone()],
            )
            .with_domain_error(system_error.clone()),
        )
        .with_host_call(
            AdapterHostCall::with_signature(
                "system.thread_count",
                signature(
                    [],
                    fallible_need(AdapterTypeKind::String, system_error.clone()),
                ),
                [effect.clone()],
            )
            .with_domain_error(system_error.clone()),
        )
        .with_host_call(
            AdapterHostCall::with_signature(
                "system.available_parallelism",
                signature(
                    [],
                    fallible_need(AdapterTypeKind::String, system_error.clone()),
                ),
                [effect],
            )
            .with_domain_error(system_error),
        )
}

/// Native file manifest.
///
/// # Panics
///
/// Panics only if Arcweft's fixed standard IDs or declaration inventory violate
/// their own checked constructors.
pub fn native_file_manifest() -> AdapterManifest {
    let read = AdapterEffectCapability::new("fs.read");
    let write = AdapterEffectCapability::new("fs.write");
    let fs_error = environment_nominal_path(NATIVE_FILE_ADAPTER_ID, ["fs", "FsError"]);
    let virtual_path = standard_nominal(["VirtualPath"]);
    AdapterManifest::new(NATIVE_FILE_ADAPTER_ID, "Native File")
        .try_with_nominal_declaration(
            AdapterNominalDeclaration::try_new(
                nominal_path_segments(["fs", "FsError"]),
                0,
                AdapterOpaqueTypeProducerId::try_new("arcweft.adapter.native-file")
                    .expect("standard opaque producer IDs are valid"),
                AdapterNominalVisibility::Public,
                "fs.FsError",
            )
            .expect("standard FsError declaration is valid"),
        )
        .expect("standard FsError declaration is unique")
        .with_effect(read.clone())
        .with_effect(write.clone())
        .with_host_call(
            AdapterHostCall::with_signature(
                "fs.read_text",
                signature(
                    [("path", virtual_path.clone())],
                    fallible_need(AdapterTypeKind::String, fs_error.clone()),
                ),
                [read.clone()],
            )
            .with_domain_error(fs_error.clone()),
        )
        .with_host_call(
            AdapterHostCall::with_signature(
                "fs.read_bytes",
                signature(
                    [("path", virtual_path.clone())],
                    fallible_need(
                        AdapterTypeKind::Vec {
                            item: Box::new(AdapterTypeKind::U8),
                        },
                        fs_error.clone(),
                    ),
                ),
                [read],
            )
            .with_domain_error(fs_error.clone()),
        )
        .with_host_call(
            AdapterHostCall::with_signature(
                "fs.write_text",
                signature(
                    [
                        ("path", virtual_path.clone()),
                        ("body", AdapterTypeKind::String),
                    ],
                    fallible_need(AdapterTypeKind::Unit, fs_error.clone()),
                ),
                [write.clone()],
            )
            .with_domain_error(fs_error.clone()),
        )
        .with_host_call(
            AdapterHostCall::with_signature(
                "fs.write_bytes",
                signature(
                    [
                        ("path", virtual_path),
                        (
                            "body",
                            AdapterTypeKind::Vec {
                                item: Box::new(AdapterTypeKind::U8),
                            },
                        ),
                    ],
                    fallible_need(AdapterTypeKind::Unit, fs_error.clone()),
                ),
                [write],
            )
            .with_domain_error(fs_error),
        )
}

/// Host math accelerator manifest.
pub fn math_manifest() -> AdapterManifest {
    AdapterManifest::new(MATH_ADAPTER_ID, "Math")
        .with_host_call(AdapterHostCall::new("math.matmul_f32", []))
        .with_host_call(AdapterHostCall::new("math.matmul_f64", []))
        .with_host_call(AdapterHostCall::new("math.tensor.add_f32", []))
        .with_host_call(AdapterHostCall::new("math.tensor.relu_f32", []))
}

fn declare_nominals<const N: usize>(
    mut manifest: AdapterManifest,
    names: [&str; N],
    opaque_producer: &str,
) -> AdapterManifest {
    for name in names {
        manifest = manifest
            .try_with_nominal_declaration(
                AdapterNominalDeclaration::try_new(
                    nominal_path(name),
                    0,
                    AdapterOpaqueTypeProducerId::try_new(opaque_producer)
                        .expect("standard opaque producer IDs are valid"),
                    AdapterNominalVisibility::Public,
                    name,
                )
                .expect("standard nominal declarations are valid"),
            )
            .expect("standard nominal declarations have distinct paths");
    }
    manifest
}

fn inference_nominal(name: &str) -> AdapterTypeKind {
    environment_nominal(INFERENCE_TENSOR_ADAPTER_ID, name)
}

fn environment_nominal(adapter_id: &str, name: &str) -> AdapterTypeKind {
    environment_nominal_path(adapter_id, [name])
}

fn environment_nominal_path<const N: usize>(
    adapter_id: &str,
    segments: [&str; N],
) -> AdapterTypeKind {
    let adapter = AdapterId::new(adapter_id);
    AdapterTypeKind::Nominal {
        nominal: AdapterNominalTypeRef::try_new(
            AdapterNominalOwner::Environment {
                owner: AdapterEnvironmentOwnerId::for_adapter(&adapter),
            },
            nominal_path_segments(segments),
            [],
        )
        .expect("standard nominal references are valid"),
    }
}

fn nominal_path(name: &str) -> AdapterNominalPath {
    nominal_path_segments([name])
}

fn nominal_path_segments<const N: usize>(segments: [&str; N]) -> AdapterNominalPath {
    AdapterNominalPath::try_new(
        segments
            .into_iter()
            .map(|segment| {
                AdapterNominalPathSegment::try_new(segment)
                    .expect("standard nominal names are valid Rust identifiers")
            })
            .collect::<Vec<_>>(),
    )
    .expect("standard nominal paths are valid")
}

fn standard_nominal<const N: usize>(segments: [&str; N]) -> AdapterTypeKind {
    AdapterTypeKind::Nominal {
        nominal: AdapterNominalTypeRef::try_new(
            AdapterNominalOwner::Standard,
            nominal_path_segments(segments),
            [],
        )
        .expect("standard nominal references are valid"),
    }
}

fn fallible_need(ok: AdapterTypeKind, error: AdapterTypeKind) -> AdapterTypeKind {
    AdapterTypeKind::Need {
        item: Box::new(AdapterTypeKind::Result {
            ok: Box::new(ok),
            error: Box::new(error),
        }),
    }
}

fn callable_name(value: &str) -> AdapterCallableName {
    AdapterCallableName::try_new(value).expect("standard callable names are valid typed segments")
}

fn adapter_symbol<const N: usize>(segments: [&str; N], ty: AdapterTypeKind) -> AdapterSymbol {
    AdapterSymbol::new(
        AdapterSymbolPath::try_new(segments.map(|segment| {
            AdapterSymbolSegment::try_new(segment)
                .expect("standard adapter symbol segments are valid")
        }))
        .expect("standard adapter symbol paths are non-empty"),
        ty,
    )
}

fn overload_zero() -> AdapterCallableOverloadIndex {
    AdapterCallableOverloadIndex::try_from_usize(0)
        .expect("zero is a valid adapter callable overload")
}

fn signature<const N: usize>(
    parameters: [(&str, AdapterTypeKind); N],
    return_type: AdapterTypeKind,
) -> AdapterFunctionSignature {
    let parameters = parameters
        .into_iter()
        .enumerate()
        .map(|(index, (name, ty))| {
            AdapterFunctionParam::try_new(
                AdapterCallableParameterIndex::try_from_usize(index)
                    .expect("standard adapter parameter indices fit"),
                Some(callable_name(name)),
                ty,
                AdapterParameterPassing::PositionalOrNamed,
                AdapterParameterPresence::Required,
            )
            .expect("standard adapter parameters are valid")
        })
        .collect();
    AdapterFunctionSignature::try_new(
        vec![
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(0)
                    .expect("zero is a valid adapter callable group"),
                parameters,
            )
            .expect("the standard adapter initial group is valid"),
        ],
        return_type,
    )
    .expect("a standard adapter signature is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inference_tensor_manifest_injects_namespaced_methods_without_core_prelude() {
        let manifest = inference_tensor_manifest();
        let tensor = inference_nominal("TensorF32");

        assert_eq!(
            manifest.symbols(),
            &[
                adapter_symbol(["conv2d"], inference_nominal("Conv2dApi")),
                adapter_symbol(["infer"], inference_nominal("InferApi"))
            ]
        );
        assert!(manifest.methods().iter().any(|method| {
            method.receiver() == &inference_nominal("Conv2dApi")
                && method.name() == "valid_f32"
                && method.signature().return_type() == &tensor
        }));
        assert!(manifest.methods().iter().any(|method| {
            method.receiver() == &inference_nominal("InferApi")
                && method.name() == "argmax_last_dim_f32"
                && method.signature().return_type()
                    == &AdapterTypeKind::Seq {
                        item: Box::new(AdapterTypeKind::USize),
                    }
        }));
        let fused = manifest
            .methods()
            .iter()
            .find(|method| method.name() == "matmul_bias_add_f32")
            .expect("fused inference method");
        assert_eq!(fused.signature().groups().len(), 1);
        let parameters = fused.signature().groups()[0].parameters();
        assert_eq!(parameters.len(), 3);
        assert_eq!(
            parameters
                .iter()
                .map(|parameter| (
                    parameter.name().expect("named parameter").as_str(),
                    parameter.ty(),
                ))
                .collect::<Vec<_>>(),
            vec![("lhs", &tensor), ("rhs", &tensor), ("bias", &tensor),]
        );
        for call in [
            "conv2d.valid_f32",
            "infer.matmul_f32",
            "infer.add_f32",
            "infer.bias_add_f32",
            "infer.matmul_bias_add_f32",
            "infer.relu_f32",
            "infer.max_pool2d_f32",
            "infer.softmax_last_dim_f32",
            "infer.argmax_last_dim_f32",
            "infer.flatten_outer_f32",
        ] {
            assert!(
                manifest
                    .host_calls()
                    .iter()
                    .any(|host_call| host_call.id() == call)
            );
        }
    }

    #[test]
    fn standard_registry_contains_builtin_adapter_ids() {
        let registry = standard_registry();
        let ids = registry.adapter_ids();

        for id in [
            SANS_IO_ADAPTER_ID,
            NATIVE_CLI_ADAPTER_ID,
            NATIVE_HTTP_ADAPTER_ID,
            INFERENCE_TENSOR_ADAPTER_ID,
            SYSTEM_INFO_ADAPTER_ID,
            NATIVE_FILE_ADAPTER_ID,
            MATH_ADAPTER_ID,
        ] {
            assert!(ids.contains(&id));
        }
    }

    #[test]
    fn standard_system_and_file_manifests_expose_host_calls() {
        let system = system_info_manifest();
        let file = native_file_manifest();

        let core_count = system
            .host_calls()
            .iter()
            .find(|call| call.id() == "system.core_count")
            .expect("system core-count host call");
        let AdapterTypeKind::Need { item } = core_count.signature().return_type() else {
            panic!("system core-count must return its typed Need contract");
        };
        let AdapterTypeKind::Result { ok, error } = item.as_ref() else {
            panic!("fallible system call must publish Result inside Need");
        };
        assert_eq!(ok.as_ref(), &AdapterTypeKind::String);
        assert_eq!(core_count.domain_error(), Some(error.as_ref()));
        assert!(file.host_calls().iter().any(|call| {
            call.id() == "fs.write_text"
                && call
                    .effects()
                    .iter()
                    .any(|effect| effect.as_str() == "fs.write")
        }));
    }

    #[test]
    fn host_call_contract_digest_is_structural_and_effect_order_independent() {
        let first = AdapterHostCall::with_signature(
            "contract.call",
            signature(
                [
                    ("left", AdapterTypeKind::String),
                    ("right", AdapterTypeKind::U32),
                ],
                AdapterTypeKind::Bool,
            ),
            [
                AdapterEffectCapability::new("io.write"),
                AdapterEffectCapability::new("io.read"),
            ],
        );
        let reordered_effects = AdapterHostCall::with_signature(
            "contract.call",
            signature(
                [
                    ("left", AdapterTypeKind::String),
                    ("right", AdapterTypeKind::U32),
                ],
                AdapterTypeKind::Bool,
            ),
            [
                AdapterEffectCapability::new("io.read"),
                AdapterEffectCapability::new("io.write"),
            ],
        );
        let reordered_parameters = AdapterHostCall::with_signature(
            "contract.call",
            signature(
                [
                    ("right", AdapterTypeKind::U32),
                    ("left", AdapterTypeKind::String),
                ],
                AdapterTypeKind::Bool,
            ),
            [
                AdapterEffectCapability::new("io.read"),
                AdapterEffectCapability::new("io.write"),
            ],
        );
        let changed_result = AdapterHostCall::with_signature(
            "contract.call",
            signature(
                [
                    ("left", AdapterTypeKind::String),
                    ("right", AdapterTypeKind::U32),
                ],
                AdapterTypeKind::String,
            ),
            [
                AdapterEffectCapability::new("io.read"),
                AdapterEffectCapability::new("io.write"),
            ],
        );

        assert_eq!(first.contract_digest(), reordered_effects.contract_digest());
        assert_ne!(
            first.contract_digest(),
            reordered_parameters.contract_digest()
        );
        assert_ne!(first.contract_digest(), changed_result.contract_digest());
        assert_ne!(
            AdapterHostCall::new("a.bc", []).contract_digest(),
            AdapterHostCall::new("ab.c", []).contract_digest()
        );
    }
}
