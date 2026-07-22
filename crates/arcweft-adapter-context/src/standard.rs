//! Standard adapter manifests bundled with Arcweft tooling.

use crate::manifest::{
    AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
    AdapterCallableParameterIndex, AdapterEffectCapability, AdapterEnvironmentOwnerId,
    AdapterFunctionParam, AdapterFunctionSignature, AdapterHostCall, AdapterId, AdapterManifest,
    AdapterNominalDeclaration, AdapterNominalOwner, AdapterNominalPath, AdapterNominalPathSegment,
    AdapterNominalTypeRef, AdapterNominalVisibility, AdapterParameterGroup,
    AdapterParameterPassing, AdapterParameterPresence, AdapterRegistry, AdapterSymbol,
    AdapterSymbolPath, AdapterSymbolSegment, AdapterTypeKind,
};

/// Adapter id for the default Sans I/O environment.
pub const SANS_IO_ADAPTER_ID: &str = "sans-io";

/// Adapter id for the native HTTP server environment.
pub const NATIVE_HTTP_ADAPTER_ID: &str = "native-http";

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

/// Native HTTP server manifest.
pub fn native_http_manifest() -> AdapterManifest {
    declare_nominals(
        AdapterManifest::new(NATIVE_HTTP_ADAPTER_ID, "Native HTTP"),
        ["HttpRequestContext"],
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
pub fn system_info_manifest() -> AdapterManifest {
    let effect = AdapterEffectCapability::new("system.read");
    AdapterManifest::new(SYSTEM_INFO_ADAPTER_ID, "System Info")
        .with_effect(effect.clone())
        .with_host_call(AdapterHostCall::new("system.core_count", [effect.clone()]))
        .with_host_call(AdapterHostCall::new(
            "system.thread_count",
            [effect.clone()],
        ))
        .with_host_call(AdapterHostCall::new(
            "system.available_parallelism",
            [effect],
        ))
}

/// Native file manifest.
pub fn native_file_manifest() -> AdapterManifest {
    let read = AdapterEffectCapability::new("fs.read");
    let write = AdapterEffectCapability::new("fs.write");
    AdapterManifest::new(NATIVE_FILE_ADAPTER_ID, "Native File")
        .with_effect(read.clone())
        .with_effect(write.clone())
        .with_host_call(AdapterHostCall::new("fs.read_text", [read.clone()]))
        .with_host_call(AdapterHostCall::new("fs.read_bytes", [read]))
        .with_host_call(AdapterHostCall::new("fs.write_text", [write.clone()]))
        .with_host_call(AdapterHostCall::new("fs.write_bytes", [write]))
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
) -> AdapterManifest {
    for name in names {
        manifest = manifest
            .try_with_nominal_declaration(
                AdapterNominalDeclaration::try_new(
                    nominal_path(name),
                    0,
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
    let adapter = AdapterId::new(adapter_id);
    AdapterTypeKind::Nominal {
        nominal: AdapterNominalTypeRef::try_new(
            AdapterNominalOwner::Environment {
                owner: AdapterEnvironmentOwnerId::for_adapter(&adapter),
            },
            nominal_path(name),
            [],
        )
        .expect("standard nominal references are valid"),
    }
}

fn nominal_path(name: &str) -> AdapterNominalPath {
    AdapterNominalPath::try_new([AdapterNominalPathSegment::try_new(name)
        .expect("standard nominal names are valid Rust identifiers")])
    .expect("one valid segment forms a nominal path")
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

        assert!(
            system
                .host_calls()
                .iter()
                .any(|call| call.id() == "system.core_count")
        );
        assert!(file.host_calls().iter().any(|call| {
            call.id() == "fs.write_text"
                && call
                    .effects()
                    .iter()
                    .any(|effect| effect.as_str() == "fs.write")
        }));
    }
}
