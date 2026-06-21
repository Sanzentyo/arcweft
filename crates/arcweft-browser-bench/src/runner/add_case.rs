use arcweft_core::math::DenseTensorF32;
use arcweft_runtime_accelerator::math::browser_webgpu::{
    BrowserWebGpuAutoMathAdapter, BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathRequest,
};

use crate::correctness::{checksum, compare};
use crate::model::{
    BrowserBenchMode, BrowserMathBenchCase, BrowserMathBenchConfig, BrowserMathBenchShape,
};

use super::browser::{fallback_reason, now_ms, yield_to_browser};
use super::capture::capture_tensor_response;
use super::case::{empty_case, fill_breakdown, finish_gpu_case, measure_case, skipped_case};
use super::cpu::{add_cpu, deterministic_values};
use super::mode::{async_batch_depth, elementwise_capacity_len};
use super::pipelined::{
    run_auto_pipelined_case, run_auto_resident_direct_pipelined_case,
    run_auto_resident_pipelined_case,
};

pub(crate) async fn run_add_case(
    config: &BrowserMathBenchConfig,
    adapter: Option<&mut BrowserWebGpuAutoMathAdapter>,
    mode: BrowserBenchMode,
    len: usize,
    round_index: usize,
    mode_order_index: usize,
) -> BrowserMathBenchCase {
    let lhs = deterministic_values(len, config.seed ^ len as u32);
    let rhs = deterministic_values(len, config.seed.rotate_left(7) ^ len as u32);
    let expected = add_cpu(&lhs, &rhs);
    let shape = BrowserMathBenchShape::Len { len };
    let mut case = empty_case(
        format!("tensor_add_f32_len{len}_{mode:?}"),
        "tensor_add_f32",
        shape,
        mode,
        round_index,
        mode_order_index,
        config,
    );
    match mode {
        BrowserBenchMode::Auto => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let lhs_tensor = DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
            let rhs_tensor = DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
            adapter.reset_stats();
            let mut out = Vec::new();
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                match adapter
                    .dispatch(BrowserWebGpuMathRequest::TensorAddF32 {
                        lhs: &lhs_tensor,
                        rhs: &rhs_tensor,
                    })
                    .await
                {
                    Ok(response) => {
                        if let Some(tensor) = response.tensor_f32() {
                            out = tensor.values().to_vec();
                        }
                    }
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let start = now_ms();
                    match adapter
                        .dispatch(BrowserWebGpuMathRequest::TensorAddF32 {
                            lhs: &lhs_tensor,
                            rhs: &rhs_tensor,
                        })
                        .await
                    {
                        Ok(response) => {
                            if let Some(tensor) = response.tensor_f32() {
                                out = tensor.values().to_vec();
                                case.capacity = response.selection().capacity().map(Into::into);
                            }
                        }
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    samples.push(now_ms() - start);
                }
            }
            case = finish_gpu_case(case, error, samples, adapter.stats(), &expected, &out);
        }
        BrowserBenchMode::AutoPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let lhs_tensor = DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
            let rhs_tensor = DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
            case = run_auto_pipelined_case(
                config,
                mode,
                case,
                adapter,
                &expected,
                |adapter| {
                    adapter.submit(BrowserWebGpuMathRequest::TensorAddF32 {
                        lhs: &lhs_tensor,
                        rhs: &rhs_tensor,
                    })
                },
                capture_tensor_response,
            )
            .await;
        }
        BrowserBenchMode::AutoResidentPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let lhs_tensor = DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
            let rhs_tensor = DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
            case = run_auto_resident_pipelined_case(
                config,
                mode,
                case,
                adapter,
                &expected,
                BrowserWebGpuMathRequest::TensorAddF32 {
                    lhs: &lhs_tensor,
                    rhs: &rhs_tensor,
                },
                capture_tensor_response,
            )
            .await;
        }
        BrowserBenchMode::AutoResidentDirectPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let lhs_tensor = DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
            let rhs_tensor = DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
            case = run_auto_resident_direct_pipelined_case(
                config,
                mode,
                case,
                adapter,
                &expected,
                BrowserWebGpuMathRequest::TensorAddF32 {
                    lhs: &lhs_tensor,
                    rhs: &rhs_tensor,
                },
            )
            .await;
        }
        BrowserBenchMode::CpuWasm => {
            let mut out = Vec::new();
            case = measure_case(case, config, || {
                out = add_cpu(&lhs, &rhs);
            });
            case.correctness = compare(&expected, &out, 1.0e-6, 1.0e-6);
            case.checksum = checksum(&out);
        }
        BrowserBenchMode::WebGpuOneShot => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            let lhs_tensor = DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
            let rhs_tensor = DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
            context.reset_stats();
            let mut out = Vec::new();
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                match context.tensor_add_f32(&lhs_tensor, &rhs_tensor).await {
                    Ok(tensor) => out = tensor.values().to_vec(),
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let start = now_ms();
                    match context.tensor_add_f32(&lhs_tensor, &rhs_tensor).await {
                        Ok(tensor) => out = tensor.values().to_vec(),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    samples.push(now_ms() - start);
                }
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
        }
        BrowserBenchMode::WebGpuPreparedUpload => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            context.reset_stats();
            case.capacity = Some(crate::model::BrowserMathBenchCapacity::Len { len });
            let prepared = match context.prepare_elementwise_f32(len) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            let mut out = vec![0.0; len];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                if let Err(current) = context
                    .dispatch_prepared_elementwise_f32(&prepared, &lhs, &rhs, &mut out)
                    .await
                {
                    error = Some(current);
                    break;
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let start = now_ms();
                    if let Err(current) = context
                        .dispatch_prepared_elementwise_f32(&prepared, &lhs, &rhs, &mut out)
                        .await
                    {
                        error = Some(current);
                        break;
                    }
                    samples.push(now_ms() - start);
                }
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
        }
        BrowserBenchMode::WebGpuPreparedResident
        | BrowserBenchMode::WebGpuPreparedCapacityResident => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            context.reset_stats();
            let capacity_len = elementwise_capacity_len(mode, len);
            case.capacity = Some(crate::model::BrowserMathBenchCapacity::Len { len: capacity_len });
            let prepared = match context.prepare_elementwise_f32(capacity_len) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; len];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                if let Err(current) = context
                    .dispatch_resident_elementwise_f32(&prepared, &mut out)
                    .await
                {
                    error = Some(current);
                    break;
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let start = now_ms();
                    if let Err(current) = context
                        .dispatch_resident_elementwise_f32(&prepared, &mut out)
                        .await
                    {
                        error = Some(current);
                        break;
                    }
                    samples.push(now_ms() - start);
                }
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
        }
        BrowserBenchMode::WebGpuPreparedResidentAsync
        | BrowserBenchMode::WebGpuPreparedResidentPipelined
        | BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            let batch_depth = async_batch_depth(mode, config);
            context.reset_stats();
            let capacity_len = elementwise_capacity_len(mode, len);
            case.capacity = Some(crate::model::BrowserMathBenchCapacity::Len { len: capacity_len });
            let prepared = match context.prepare_elementwise_f32(capacity_len) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; len];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters);
            let mut readback_samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = Vec::with_capacity(batch_depth);
                for _ in 0..batch_depth {
                    match context.submit_resident_elementwise_f32(&prepared, len) {
                        Ok(current) => submitted.push(current),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                yield_to_browser().await;
                for current in submitted {
                    if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                        error = Some(current);
                        break;
                    }
                }
                if error.is_some() {
                    break;
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let total_start = now_ms();
                    let mut submitted = Vec::with_capacity(batch_depth);
                    for _ in 0..batch_depth {
                        let submit_start = now_ms();
                        match context.submit_resident_elementwise_f32(&prepared, len) {
                            Ok(current) => submitted.push(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        submit_samples.push(now_ms() - submit_start);
                    }
                    if error.is_some() {
                        break;
                    }
                    yield_to_browser().await;
                    for current in submitted {
                        let readback_start = now_ms();
                        if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                            error = Some(current);
                            break;
                        }
                        readback_samples.push(now_ms() - readback_start);
                    }
                    if error.is_some() {
                        break;
                    }
                    samples.push((now_ms() - total_start) / batch_depth as f64);
                }
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            fill_breakdown(&mut case, submit_samples, readback_samples);
        }
        BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined
        | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            let batch_depth = async_batch_depth(mode, config);
            context.reset_stats();
            let capacity_len = elementwise_capacity_len(mode, len);
            case.capacity = Some(crate::model::BrowserMathBenchCapacity::Len { len: capacity_len });
            let prepared = match context.prepare_elementwise_f32(capacity_len) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; len];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut drain = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = Vec::with_capacity(batch_depth);
                for _ in 0..batch_depth {
                    match context.submit_resident_elementwise_f32(&prepared, len) {
                        Ok(current) => submitted.push(current),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                yield_to_browser().await;
                for current in submitted {
                    if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                        error = Some(current);
                        break;
                    }
                }
                if error.is_some() {
                    break;
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let total_start = now_ms();
                    for _ in 0..batch_depth {
                        let submit_start = now_ms();
                        match context.submit_resident_elementwise_f32(&prepared, len) {
                            Ok(current) => drain.push(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        submit_samples.push(now_ms() - submit_start);
                    }
                    if error.is_some() {
                        break;
                    }
                    yield_to_browser().await;
                    samples.push((now_ms() - total_start) / batch_depth as f64);
                }
            }
            if error.is_none() {
                for current in drain {
                    if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                        error = Some(current);
                        break;
                    }
                }
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            fill_breakdown(&mut case, submit_samples, Vec::new());
        }
        BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined
        | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            let batch_depth = async_batch_depth(mode, config);
            context.reset_stats();
            let capacity_len = elementwise_capacity_len(mode, len);
            case.capacity = Some(crate::model::BrowserMathBenchCapacity::Len { len: capacity_len });
            let prepared = match context.prepare_elementwise_f32(capacity_len) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; len];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut latest = None;
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = None;
                for _ in 0..batch_depth {
                    match context.submit_resident_elementwise_f32_without_readback(&prepared, len) {
                        Ok(current) => submitted = Some(current),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                yield_to_browser().await;
                if let Some(current) = submitted
                    && let Err(current) = context
                        .read_resident_elementwise_f32(&prepared, current, &mut out)
                        .await
                {
                    error = Some(current);
                }
                if error.is_some() {
                    break;
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let total_start = now_ms();
                    for _ in 0..batch_depth {
                        let submit_start = now_ms();
                        match context
                            .submit_resident_elementwise_f32_without_readback(&prepared, len)
                        {
                            Ok(current) => latest = Some(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        submit_samples.push(now_ms() - submit_start);
                    }
                    if error.is_some() {
                        break;
                    }
                    yield_to_browser().await;
                    samples.push((now_ms() - total_start) / batch_depth as f64);
                }
            }
            if error.is_none()
                && let Some(current) = latest
                && let Err(current) = context
                    .read_resident_elementwise_f32(&prepared, current, &mut out)
                    .await
            {
                error = Some(current);
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            fill_breakdown(&mut case, submit_samples, Vec::new());
        }
        BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined
        | BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined => {
            return skipped_case(case, "mode_only_supports_matmul");
        }
    }
    case
}
