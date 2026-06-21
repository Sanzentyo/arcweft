use arcweft_core::math::DenseMatrixF32;
use arcweft_runtime_accelerator::math::browser_webgpu::{
    BrowserMatmulAddF32Shape, BrowserMatmulCapacity, BrowserResidentF32GraphInputs,
    BrowserResidentF32GraphSpec, BrowserResidentMatmulAddF32Inputs,
    BrowserResidentMatmulBiasAddF32Inputs, BrowserWebGpuAutoMathAdapter, BrowserWebGpuMathRequest,
};

use crate::correctness::{checksum, compare, matmul_abs_tol};
use crate::model::{
    BrowserBenchMode, BrowserMathBenchCase, BrowserMathBenchConfig, BrowserMathBenchShape,
    MatmulShape,
};

use super::browser::{fallback_reason, now_ms, yield_to_browser};
use super::capture::capture_matrix_response;
use super::case::{empty_case, fill_breakdown, finish_gpu_case, measure_case, skipped_case};
use super::cpu::{deterministic_values, matmul_cpu};
use super::mode::{async_batch_depth, matmul_capacity};
use super::pipelined::{
    run_auto_pipelined_case, run_auto_resident_direct_pipelined_case,
    run_auto_resident_pipelined_case,
};

pub(crate) async fn run_matmul_case(
    config: &BrowserMathBenchConfig,
    adapter: Option<&mut BrowserWebGpuAutoMathAdapter>,
    mode: BrowserBenchMode,
    shape: MatmulShape,
    round_index: usize,
    mode_order_index: usize,
) -> BrowserMathBenchCase {
    let lhs_len = shape.rows * shape.shared;
    let rhs_len = shape.shared * shape.cols;
    let lhs = deterministic_values(lhs_len, config.seed ^ lhs_len as u32);
    let rhs = deterministic_values(rhs_len, config.seed.rotate_left(11) ^ rhs_len as u32);
    let expected = matmul_cpu(&lhs, &rhs, shape);
    let mut case = empty_case(
        format!(
            "matmul_f32_m{}_k{}_n{}_{mode:?}",
            shape.rows, shape.shared, shape.cols
        ),
        "matmul_f32",
        BrowserMathBenchShape::Matmul {
            rows: shape.rows,
            shared: shape.shared,
            cols: shape.cols,
        },
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
            let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                .expect("valid lhs matrix");
            let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                .expect("valid rhs matrix");
            adapter.reset_stats();
            let mut out = Vec::new();
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                match adapter
                    .dispatch(BrowserWebGpuMathRequest::MatmulF32 {
                        lhs: &lhs_matrix,
                        rhs: &rhs_matrix,
                    })
                    .await
                {
                    Ok(response) => {
                        if let Some(matrix) = response.matrix_f32() {
                            out = matrix.values().to_vec();
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
                        .dispatch(BrowserWebGpuMathRequest::MatmulF32 {
                            lhs: &lhs_matrix,
                            rhs: &rhs_matrix,
                        })
                        .await
                    {
                        Ok(response) => {
                            if let Some(matrix) = response.matrix_f32() {
                                out = matrix.values().to_vec();
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
            let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                .expect("valid lhs matrix");
            let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                .expect("valid rhs matrix");
            case = run_auto_pipelined_case(
                config,
                mode,
                case,
                adapter,
                &expected,
                |adapter| {
                    adapter.submit(BrowserWebGpuMathRequest::MatmulF32 {
                        lhs: &lhs_matrix,
                        rhs: &rhs_matrix,
                    })
                },
                capture_matrix_response,
            )
            .await;
        }
        BrowserBenchMode::AutoResidentPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                .expect("valid lhs matrix");
            let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                .expect("valid rhs matrix");
            case = run_auto_resident_pipelined_case(
                config,
                mode,
                case,
                adapter,
                &expected,
                BrowserWebGpuMathRequest::MatmulF32 {
                    lhs: &lhs_matrix,
                    rhs: &rhs_matrix,
                },
                capture_matrix_response,
            )
            .await;
        }
        BrowserBenchMode::AutoResidentDirectPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                .expect("valid lhs matrix");
            let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                .expect("valid rhs matrix");
            case = run_auto_resident_direct_pipelined_case(
                config,
                mode,
                case,
                adapter,
                &expected,
                BrowserWebGpuMathRequest::MatmulF32 {
                    lhs: &lhs_matrix,
                    rhs: &rhs_matrix,
                },
            )
            .await;
        }
        BrowserBenchMode::CpuWasm => {
            let mut out = Vec::new();
            case = measure_case(case, config, || {
                out = matmul_cpu(&lhs, &rhs, shape);
            });
            case.correctness = compare(&expected, &out, matmul_abs_tol(shape.shared), 1.0e-4);
            case.checksum = checksum(&out);
        }
        BrowserBenchMode::WebGpuOneShot => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                .expect("valid lhs matrix");
            let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                .expect("valid rhs matrix");
            context.reset_stats();
            let mut out = Vec::new();
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                match context.matmul_f32(&lhs_matrix, &rhs_matrix).await {
                    Ok(matrix) => out = matrix.values().to_vec(),
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let start = now_ms();
                    match context.matmul_f32(&lhs_matrix, &rhs_matrix).await {
                        Ok(matrix) => out = matrix.values().to_vec(),
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
            let capacity = BrowserMatmulCapacity {
                rows: shape.rows,
                shared: shape.shared,
                cols: shape.cols,
            };
            case.capacity = Some(capacity.into());
            let prepared = match context.prepare_matmul_f32(capacity) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            let mut out = vec![0.0; shape.rows * shape.cols];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                if let Err(current) = context
                    .dispatch_prepared_matmul_f32(
                        &prepared,
                        &lhs,
                        &rhs,
                        &mut out,
                        shape.rows,
                        shape.shared,
                        shape.cols,
                    )
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
                        .dispatch_prepared_matmul_f32(
                            &prepared,
                            &lhs,
                            &rhs,
                            &mut out,
                            shape.rows,
                            shape.shared,
                            shape.cols,
                        )
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
            let capacity = matmul_capacity(mode, shape);
            case.capacity = Some(capacity.into());
            let prepared = match context.prepare_matmul_f32(capacity) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_matmul_f32(
                &prepared,
                &lhs,
                &rhs,
                shape.rows,
                shape.shared,
                shape.cols,
            ) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; shape.rows * shape.cols];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                if let Err(current) = context
                    .dispatch_resident_matmul_f32(&prepared, &mut out, shape.rows, shape.cols)
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
                        .dispatch_resident_matmul_f32(&prepared, &mut out, shape.rows, shape.cols)
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
            let capacity = matmul_capacity(mode, shape);
            case.capacity = Some(capacity.into());
            let prepared = match context.prepare_matmul_f32(capacity) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_matmul_f32(
                &prepared,
                &lhs,
                &rhs,
                shape.rows,
                shape.shared,
                shape.cols,
            ) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; shape.rows * shape.cols];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters);
            let mut readback_samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = Vec::with_capacity(batch_depth);
                for _ in 0..batch_depth {
                    match context.submit_resident_matmul_f32(&prepared, shape.rows, shape.cols) {
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
                        match context.submit_resident_matmul_f32(&prepared, shape.rows, shape.cols)
                        {
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
            let capacity = matmul_capacity(mode, shape);
            case.capacity = Some(capacity.into());
            let prepared = match context.prepare_matmul_f32(capacity) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_matmul_f32(
                &prepared,
                &lhs,
                &rhs,
                shape.rows,
                shape.shared,
                shape.cols,
            ) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; shape.rows * shape.cols];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut drain = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = Vec::with_capacity(batch_depth);
                for _ in 0..batch_depth {
                    match context.submit_resident_matmul_f32(&prepared, shape.rows, shape.cols) {
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
                        match context.submit_resident_matmul_f32(&prepared, shape.rows, shape.cols)
                        {
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
            let capacity = matmul_capacity(mode, shape);
            case.capacity = Some(capacity.into());
            let prepared = match context.prepare_matmul_f32(capacity) {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            if let Err(error) = context.upload_prepared_matmul_f32(
                &prepared,
                &lhs,
                &rhs,
                shape.rows,
                shape.shared,
                shape.cols,
            ) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; shape.rows * shape.cols];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut latest = None;
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = None;
                for _ in 0..batch_depth {
                    match context.submit_resident_matmul_f32_without_readback(
                        &prepared, shape.rows, shape.cols,
                    ) {
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
                        .read_resident_matmul_f32(
                            &prepared, current, shape.rows, shape.cols, &mut out,
                        )
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
                        match context.submit_resident_matmul_f32_without_readback(
                            &prepared, shape.rows, shape.cols,
                        ) {
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
                    .read_resident_matmul_f32(&prepared, current, shape.rows, shape.cols, &mut out)
                    .await
            {
                error = Some(current);
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            fill_breakdown(&mut case, submit_samples, Vec::new());
        }
        BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            let batch_depth = async_batch_depth(mode, config);
            context.reset_stats();
            let capacity = BrowserMatmulCapacity {
                rows: shape.rows,
                shared: shape.shared,
                cols: shape.cols,
            };
            let graph_shape = BrowserMatmulAddF32Shape::new(shape.rows, shape.shared, shape.cols);
            case.capacity = Some(capacity.into());
            let prepared = match context
                .prepare_resident_f32_graph(BrowserResidentF32GraphSpec::matmul_add(capacity))
            {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            let add_rhs = vec![0.0; shape.rows * shape.cols];
            let inputs = BrowserResidentF32GraphInputs::MatmulAdd(
                BrowserResidentMatmulAddF32Inputs::new(&lhs, &rhs, &add_rhs, graph_shape),
            );
            if let Err(error) = context.upload_prepared_resident_f32_graph(&prepared, inputs) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; shape.rows * shape.cols];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut latest = None;
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = None;
                for _ in 0..batch_depth {
                    match context
                        .submit_prepared_resident_f32_graph_without_readback(&prepared, graph_shape)
                    {
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
                        .read_prepared_resident_f32_graph(&prepared, current, graph_shape, &mut out)
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
                        match context.submit_prepared_resident_f32_graph_without_readback(
                            &prepared,
                            graph_shape,
                        ) {
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
                    .read_prepared_resident_f32_graph(&prepared, current, graph_shape, &mut out)
                    .await
            {
                error = Some(current);
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            fill_breakdown(&mut case, submit_samples, Vec::new());
        }
        BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined => {
            let Some(adapter) = adapter else {
                return skipped_case(case, "webgpu_unavailable");
            };
            let context = adapter.context_mut();
            let batch_depth = async_batch_depth(mode, config);
            context.reset_stats();
            let capacity = BrowserMatmulCapacity {
                rows: shape.rows,
                shared: shape.shared,
                cols: shape.cols,
            };
            let graph_shape = BrowserMatmulAddF32Shape::new(shape.rows, shape.shared, shape.cols);
            case.capacity = Some(capacity.into());
            let prepared = match context
                .prepare_resident_f32_graph(BrowserResidentF32GraphSpec::matmul_bias_add(capacity))
            {
                Ok(prepared) => prepared,
                Err(error) => return skipped_case(case, &fallback_reason(&error)),
            };
            let bias = vec![0.0; shape.cols];
            let inputs = BrowserResidentF32GraphInputs::MatmulBiasAdd(
                BrowserResidentMatmulBiasAddF32Inputs::new(&lhs, &rhs, &bias, graph_shape),
            );
            if let Err(error) = context.upload_prepared_resident_f32_graph(&prepared, inputs) {
                return skipped_case(case, &fallback_reason(&error));
            }
            let mut out = vec![0.0; shape.rows * shape.cols];
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
            let mut latest = None;
            let mut error = None;
            for _ in 0..config.warmup_iters {
                let mut submitted = None;
                for _ in 0..batch_depth {
                    match context
                        .submit_prepared_resident_f32_graph_without_readback(&prepared, graph_shape)
                    {
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
                        .read_prepared_resident_f32_graph(&prepared, current, graph_shape, &mut out)
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
                        match context.submit_prepared_resident_f32_graph_without_readback(
                            &prepared,
                            graph_shape,
                        ) {
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
                    .read_prepared_resident_f32_graph(&prepared, current, graph_shape, &mut out)
                    .await
            {
                error = Some(current);
            }
            case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            fill_breakdown(&mut case, submit_samples, Vec::new());
        }
    }
    case
}
