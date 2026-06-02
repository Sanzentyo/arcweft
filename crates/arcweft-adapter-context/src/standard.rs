//! Standard adapter manifests bundled with Arcweft tooling.

use crate::manifest::{AdapterEffectCapability, AdapterHostCall, AdapterManifest, AdapterRegistry};
use arcweft_lang_sema::types::TypeKind;

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
    AdapterManifest::new(NATIVE_HTTP_ADAPTER_ID, "Native HTTP")
        .with_symbol("request", TypeKind::Named("HttpRequestContext".to_owned()))
        .with_effect(AdapterEffectCapability::new("http.respond"))
        .with_host_call(AdapterHostCall::new(
            "http.respond",
            [AdapterEffectCapability::new("http.respond")],
        ))
}

/// Optional forward-inference tensor manifest.
pub fn inference_tensor_manifest() -> AdapterManifest {
    let tensor = TypeKind::Named("TensorF32".to_owned());
    AdapterManifest::new(INFERENCE_TENSOR_ADAPTER_ID, "Inference Tensor")
        .with_symbol("conv2d", TypeKind::Named("Conv2dApi".to_owned()))
        .with_symbol("infer", TypeKind::Named("InferApi".to_owned()))
        .with_method(
            TypeKind::Named("Conv2dApi".to_owned()),
            "valid_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "matmul_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "add_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "bias_add_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "relu_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "max_pool2d_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "softmax_last_dim_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "argmax_last_dim_f32",
            TypeKind::Seq(Box::new(TypeKind::USize)),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "flatten_outer_f32",
            tensor,
        )
        .with_host_call(AdapterHostCall::new("infer.matmul_f32", []))
        .with_host_call(AdapterHostCall::new("conv.valid_f32", []))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AdapterSymbol;

    #[test]
    fn inference_tensor_manifest_injects_namespaced_methods_without_core_prelude() {
        let manifest = inference_tensor_manifest();
        let tensor = TypeKind::Named("TensorF32".to_owned());
        let env = manifest.apply_to_env(arcweft_lang_sema::env::TypeCheckEnv::new());

        assert_eq!(
            manifest.symbols(),
            &[
                AdapterSymbol::new("conv2d", TypeKind::Named("Conv2dApi".to_owned())),
                AdapterSymbol::new("infer", TypeKind::Named("InferApi".to_owned()))
            ]
        );
        assert!(manifest.methods().iter().any(|method| {
            method.receiver() == &TypeKind::Named("Conv2dApi".to_owned())
                && method.name() == "valid_f32"
                && method.signature().return_type() == &tensor
        }));
        assert!(manifest.methods().iter().any(|method| {
            method.receiver() == &TypeKind::Named("InferApi".to_owned())
                && method.name() == "argmax_last_dim_f32"
                && method.signature().return_type() == &TypeKind::Seq(Box::new(TypeKind::USize))
        }));
        assert_eq!(
            env,
            manifest
                .clone()
                .apply_to_env(arcweft_lang_sema::env::TypeCheckEnv::new())
        );
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
