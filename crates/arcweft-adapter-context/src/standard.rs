//! Standard adapter manifests bundled with Arcweft tooling.

use crate::manifest::{
    AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
    AdapterEffectCapability, AdapterFunctionSignature, AdapterHostCall, AdapterManifest,
    AdapterParameterGroup, AdapterRegistry, AdapterTypeKind,
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

/// Publishes every accepted standard adapter through its fixed typed owner.
#[cfg(feature = "sema")]
pub fn callable_publications(
    limits: &arcweft_lang_sema::callable::CallableLimits,
) -> Result<
    Vec<arcweft_lang_sema::callable::EnvironmentCallablePublication>,
    crate::publication::AdapterCallablePublicationError,
> {
    use arcweft_lang_sema::callable::StandardEnvironmentId;

    [
        (sans_io_manifest(), StandardEnvironmentId::SansIo),
        (native_http_manifest(), StandardEnvironmentId::NativeHttp),
        (
            inference_tensor_manifest(),
            StandardEnvironmentId::InferenceTensor,
        ),
        (system_info_manifest(), StandardEnvironmentId::SystemInfo),
        (native_file_manifest(), StandardEnvironmentId::NativeFile),
        (math_manifest(), StandardEnvironmentId::Math),
    ]
    .into_iter()
    .map(|(manifest, id)| {
        manifest.try_callable_publication(
            crate::publication::AdapterManifestSource::Standard(id),
            limits,
        )
    })
    .collect()
}

/// Returns the fixed standard owner for one reserved adapter manifest ID.
#[cfg(feature = "sema")]
pub fn manifest_source(id: &str) -> Option<crate::publication::AdapterManifestSource> {
    use arcweft_lang_sema::callable::StandardEnvironmentId;

    Some(crate::publication::AdapterManifestSource::Standard(
        match id {
            SANS_IO_ADAPTER_ID => StandardEnvironmentId::SansIo,
            NATIVE_HTTP_ADAPTER_ID => StandardEnvironmentId::NativeHttp,
            INFERENCE_TENSOR_ADAPTER_ID => StandardEnvironmentId::InferenceTensor,
            SYSTEM_INFO_ADAPTER_ID => StandardEnvironmentId::SystemInfo,
            NATIVE_FILE_ADAPTER_ID => StandardEnvironmentId::NativeFile,
            MATH_ADAPTER_ID => StandardEnvironmentId::Math,
            _ => return None,
        },
    ))
}

/// Default Sans I/O manifest.
pub fn sans_io_manifest() -> AdapterManifest {
    AdapterManifest::new(SANS_IO_ADAPTER_ID, "Sans I/O")
}

/// Native HTTP server manifest.
pub fn native_http_manifest() -> AdapterManifest {
    AdapterManifest::new(NATIVE_HTTP_ADAPTER_ID, "Native HTTP")
        .with_symbol(
            "request",
            AdapterTypeKind::Named("HttpRequestContext".to_owned()),
        )
        .with_effect(AdapterEffectCapability::new("http.respond"))
        .with_host_call(AdapterHostCall::new(
            "http.respond",
            [AdapterEffectCapability::new("http.respond")],
        ))
}

/// Optional forward-inference tensor manifest.
pub fn inference_tensor_manifest() -> AdapterManifest {
    let tensor = AdapterTypeKind::Named("TensorF32".to_owned());
    AdapterManifest::new(INFERENCE_TENSOR_ADAPTER_ID, "Inference Tensor")
        .with_symbol("conv2d", AdapterTypeKind::Named("Conv2dApi".to_owned()))
        .with_symbol("infer", AdapterTypeKind::Named("InferApi".to_owned()))
        .with_method_signature(
            AdapterTypeKind::Named("Conv2dApi".to_owned()),
            callable_name("valid_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("matmul_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("add_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("bias_add_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("matmul_bias_add_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("relu_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("max_pool2d_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("softmax_last_dim_f32"),
            overload_zero(),
            return_only(tensor.clone()),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("argmax_last_dim_f32"),
            overload_zero(),
            return_only(AdapterTypeKind::Seq(Box::new(AdapterTypeKind::USize))),
            [],
        )
        .with_method_signature(
            AdapterTypeKind::Named("InferApi".to_owned()),
            callable_name("flatten_outer_f32"),
            overload_zero(),
            return_only(tensor),
            [],
        )
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

fn callable_name(value: &str) -> AdapterCallableName {
    AdapterCallableName::try_new(value).expect("standard callable names are valid typed segments")
}

fn overload_zero() -> AdapterCallableOverloadIndex {
    AdapterCallableOverloadIndex::try_from_usize(0)
        .expect("zero is a valid adapter callable overload")
}

fn return_only(return_type: AdapterTypeKind) -> AdapterFunctionSignature {
    AdapterFunctionSignature::try_new(
        vec![
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(0)
                    .expect("zero is a valid adapter callable group"),
                Vec::new(),
            )
            .expect("an empty initial adapter group is valid"),
        ],
        return_type,
    )
    .expect("a standard return-only adapter signature is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AdapterSymbol;

    #[test]
    fn inference_tensor_manifest_injects_namespaced_methods_without_core_prelude() {
        let manifest = inference_tensor_manifest();
        let tensor = AdapterTypeKind::Named("TensorF32".to_owned());

        assert_eq!(
            manifest.symbols(),
            &[
                AdapterSymbol::new("conv2d", AdapterTypeKind::Named("Conv2dApi".to_owned())),
                AdapterSymbol::new("infer", AdapterTypeKind::Named("InferApi".to_owned()))
            ]
        );
        assert!(manifest.methods().iter().any(|method| {
            method.receiver() == &AdapterTypeKind::Named("Conv2dApi".to_owned())
                && method.name() == "valid_f32"
                && method.signature().return_type() == &tensor
        }));
        assert!(manifest.methods().iter().any(|method| {
            method.receiver() == &AdapterTypeKind::Named("InferApi".to_owned())
                && method.name() == "argmax_last_dim_f32"
                && method.signature().return_type()
                    == &AdapterTypeKind::Seq(Box::new(AdapterTypeKind::USize))
        }));
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
