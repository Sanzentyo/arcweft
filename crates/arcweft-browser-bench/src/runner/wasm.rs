use crate::model::{
    BrowserMathBenchCase, BrowserMathBenchConfig, BrowserMathBenchLimits, BrowserMathBenchReport,
    BrowserMathBenchRun, BrowserMathBenchSkip, BrowserMathBenchWebGpu,
};
use crate::recommend::browser_math_bench_recommendations;
use crate::stability::browser_math_bench_stability;
use arcweft_runtime_accelerator::math::browser_webgpu::{
    BrowserWebGpuAutoMathAdapter, BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathContext,
};
use wasm_bindgen::prelude::*;

use super::add_case::run_add_case;
use super::browser::fallback_reason;
use super::config::parse_config;
use super::matmul_case::run_matmul_case;
use super::mode::ordered_modes;

#[wasm_bindgen]
pub async fn run_arcweft_browser_math_bench(config_json: String) -> Result<JsValue, JsValue> {
    let config = parse_config(&config_json).map_err(|error| JsValue::from_str(&error))?;
    let report = run(config).await;
    let json =
        serde_json::to_string(&report).map_err(|error| JsValue::from_str(&error.to_string()))?;
    js_sys::JSON::parse(&json)
}

async fn run(config: BrowserMathBenchConfig) -> BrowserMathBenchReport {
    let availability = BrowserWebGpuMathContext::availability();
    let mut skips = Vec::new();
    let mut adapter = match BrowserWebGpuMathContext::new().await {
        Ok(context) => Some(BrowserWebGpuAutoMathAdapter::from_context(
            context,
            BrowserWebGpuMathAutoPolicy::default(),
        )),
        Err(error) => {
            skips.push(BrowserMathBenchSkip {
                scope: "webgpu",
                reason: fallback_reason(&error),
            });
            None
        }
    };
    let limits = adapter.as_ref().map(|adapter| {
        let limits = adapter.limits();
        BrowserMathBenchLimits {
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
            max_buffer_size: limits.max_buffer_size,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
        }
    });
    let mut cases = Vec::new();
    for len in &config.add_lengths {
        for round_index in 0..config.repeat_rounds.max(1) {
            let ordered_modes = ordered_modes(&config, round_index);
            for (mode_order_index, mode) in ordered_modes.into_iter().enumerate() {
                cases.push(
                    run_add_case(
                        &config,
                        adapter.as_mut(),
                        mode,
                        *len,
                        round_index,
                        mode_order_index,
                    )
                    .await,
                );
            }
        }
    }
    for shape in &config.matmul_shapes {
        for round_index in 0..config.repeat_rounds.max(1) {
            let ordered_modes = ordered_modes(&config, round_index);
            for (mode_order_index, mode) in ordered_modes.into_iter().enumerate() {
                cases.push(
                    run_matmul_case(
                        &config,
                        adapter.as_mut(),
                        mode,
                        *shape,
                        round_index,
                        mode_order_index,
                    )
                    .await,
                );
            }
        }
    }
    let stability = browser_math_bench_stability(&cases);
    let recommendations = browser_math_bench_recommendations(&cases, limits);
    BrowserMathBenchReport {
        schema_version: "arcweft.browser_webgpu_bench.v1",
        run: BrowserMathBenchRun {
            secure_context: availability.secure_context,
            cross_origin_isolated: availability.cross_origin_isolated,
            webgpu: BrowserMathBenchWebGpu {
                available: adapter.is_some(),
                fallback_reason: skips.first().map(|skip| skip.reason.clone()),
                limits,
            },
        },
        cases,
        stability,
        recommendations,
        skips,
    }
}
