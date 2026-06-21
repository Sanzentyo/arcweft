use arcweft_runtime_accelerator::math::browser_webgpu::{
    BrowserSubmittedF32, BrowserWebGpuAutoMathAdapter, BrowserWebGpuError,
    BrowserWebGpuMathContext, BrowserWebGpuMathDispatch, BrowserWebGpuMathRequest,
    BrowserWebGpuMathResponse, BrowserWebGpuPreparedMath, BrowserWebGpuPreparedMathDispatch,
};

use crate::model::{BrowserBenchMode, BrowserMathBenchCase, BrowserMathBenchConfig};

use super::browser::{now_ms, yield_to_browser};
use super::capture::{capture_response, resize_out_for_submitted};
use super::case::{fill_breakdown, finish_gpu_case};
use super::mode::async_batch_depth;

pub(crate) async fn run_auto_pipelined_case(
    config: &BrowserMathBenchConfig,
    mode: BrowserBenchMode,
    mut case: BrowserMathBenchCase,
    adapter: &mut BrowserWebGpuAutoMathAdapter,
    expected: &[f32],
    mut submit: impl FnMut(
        &mut BrowserWebGpuAutoMathAdapter,
    ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError>,
    capture_immediate: fn(&BrowserWebGpuMathResponse, &mut Vec<f32>),
) -> BrowserMathBenchCase {
    let batch_depth = async_batch_depth(mode, config);
    adapter.reset_stats();
    let mut out = Vec::new();
    let mut samples = Vec::with_capacity(config.sample_iters);
    let mut submit_samples = Vec::with_capacity(config.sample_iters);
    let mut readback_samples = Vec::with_capacity(config.sample_iters);
    let mut error = None;
    for _ in 0..config.warmup_iters {
        let mut submitted = Vec::with_capacity(batch_depth);
        for _ in 0..batch_depth {
            match submit(adapter) {
                Ok(BrowserWebGpuMathDispatch::Immediate(response)) => {
                    capture_immediate(&response, &mut out);
                    case.capacity = response.selection().capacity().map(Into::into);
                }
                Ok(BrowserWebGpuMathDispatch::Submitted(current)) => {
                    case.capacity = current.selection().capacity().map(Into::into);
                    submitted.push(current);
                }
                Err(current) => {
                    error = Some(current);
                    break;
                }
            }
        }
        yield_to_browser().await;
        for current in submitted {
            resize_out_for_submitted(&mut out, current.len());
            match adapter.read_submitted_values_into(current, &mut out).await {
                Ok(_) => {}
                Err(current) => {
                    error = Some(current);
                    break;
                }
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
                match submit(adapter) {
                    Ok(BrowserWebGpuMathDispatch::Immediate(response)) => {
                        capture_immediate(&response, &mut out);
                        case.capacity = response.selection().capacity().map(Into::into);
                    }
                    Ok(BrowserWebGpuMathDispatch::Submitted(current)) => {
                        case.capacity = current.selection().capacity().map(Into::into);
                        submitted.push(current);
                    }
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
                resize_out_for_submitted(&mut out, current.len());
                match adapter.read_submitted_values_into(current, &mut out).await {
                    Ok(_) => {}
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
                readback_samples.push(now_ms() - readback_start);
            }
            if error.is_some() {
                break;
            }
            samples.push((now_ms() - total_start) / batch_depth as f64);
        }
    }
    case = finish_gpu_case(case, error, samples, adapter.stats(), expected, &out);
    fill_breakdown(&mut case, submit_samples, readback_samples);
    case
}

pub(crate) async fn run_auto_resident_pipelined_case(
    config: &BrowserMathBenchConfig,
    mode: BrowserBenchMode,
    mut case: BrowserMathBenchCase,
    adapter: &mut BrowserWebGpuAutoMathAdapter,
    expected: &[f32],
    request: BrowserWebGpuMathRequest<'_>,
    capture_immediate: fn(&BrowserWebGpuMathResponse, &mut Vec<f32>),
) -> BrowserMathBenchCase {
    adapter.reset_stats();
    let dispatch = match adapter.prepare_resident(request) {
        Ok(dispatch) => dispatch,
        Err(error) => {
            return finish_gpu_case(
                case,
                Some(error),
                Vec::new(),
                adapter.stats(),
                expected,
                &[],
            );
        }
    };
    case.capacity = dispatch.selection().capacity().map(Into::into);
    match dispatch {
        BrowserWebGpuPreparedMathDispatch::Cpu(_) => {
            let mut out = Vec::new();
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                match adapter.dispatch(request).await {
                    Ok(response) => capture_immediate(&response, &mut out),
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let start = now_ms();
                    match adapter.dispatch(request).await {
                        Ok(response) => capture_immediate(&response, &mut out),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    samples.push(now_ms() - start);
                }
            }
            finish_gpu_case(case, error, samples, adapter.stats(), expected, &out)
        }
        BrowserWebGpuPreparedMathDispatch::Prepared(prepared) => {
            run_prepared_auto_resident_pipelined_case(
                config, mode, case, adapter, expected, &prepared,
            )
            .await
        }
    }
}

pub(crate) async fn run_auto_resident_direct_pipelined_case(
    config: &BrowserMathBenchConfig,
    mode: BrowserBenchMode,
    mut case: BrowserMathBenchCase,
    adapter: &mut BrowserWebGpuAutoMathAdapter,
    expected: &[f32],
    request: BrowserWebGpuMathRequest<'_>,
) -> BrowserMathBenchCase {
    adapter.reset_stats();
    let dispatch = match adapter.prepare_resident(request) {
        Ok(dispatch) => dispatch,
        Err(error) => {
            return finish_gpu_case(
                case,
                Some(error),
                Vec::new(),
                adapter.stats(),
                expected,
                &[],
            );
        }
    };
    case.capacity = dispatch.selection().capacity().map(Into::into);
    match dispatch {
        BrowserWebGpuPreparedMathDispatch::Cpu(_) => {
            let mut out = Vec::new();
            let mut samples = Vec::with_capacity(config.sample_iters);
            let mut error = None;
            for _ in 0..config.warmup_iters {
                match adapter.dispatch(request).await {
                    Ok(response) => capture_response(&response, &mut out),
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            if error.is_none() {
                for _ in 0..config.sample_iters {
                    let start = now_ms();
                    match adapter.dispatch(request).await {
                        Ok(response) => capture_response(&response, &mut out),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    samples.push(now_ms() - start);
                }
            }
            finish_gpu_case(case, error, samples, adapter.stats(), expected, &out)
        }
        BrowserWebGpuPreparedMathDispatch::Prepared(prepared) => {
            run_prepared_auto_resident_direct_pipelined_case(
                config, mode, case, adapter, expected, &prepared,
            )
            .await
        }
    }
}

async fn run_prepared_auto_resident_direct_pipelined_case(
    config: &BrowserMathBenchConfig,
    mode: BrowserBenchMode,
    mut case: BrowserMathBenchCase,
    adapter: &mut BrowserWebGpuAutoMathAdapter,
    expected: &[f32],
    prepared: &BrowserWebGpuPreparedMath,
) -> BrowserMathBenchCase {
    let batch_depth = async_batch_depth(mode, config);
    let mut out = vec![0.0; prepared.len()];
    let mut samples = Vec::with_capacity(config.sample_iters);
    let mut submit_samples = Vec::with_capacity(config.sample_iters);
    let mut readback_samples = Vec::with_capacity(config.sample_iters);
    let mut error = None;
    for _ in 0..config.warmup_iters {
        let mut submitted = Vec::with_capacity(batch_depth);
        for _ in 0..batch_depth {
            match submit_prepared_direct(adapter.context_mut(), prepared) {
                Ok(current) => submitted.push(current),
                Err(current) => {
                    error = Some(current);
                    break;
                }
            }
        }
        yield_to_browser().await;
        for current in submitted {
            if let Err(current) = adapter
                .context_mut()
                .read_submitted_f32(current, &mut out)
                .await
            {
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
                match submit_prepared_direct(adapter.context_mut(), prepared) {
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
                if let Err(current) = adapter
                    .context_mut()
                    .read_submitted_f32(current, &mut out)
                    .await
                {
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
    case = finish_gpu_case(case, error, samples, adapter.stats(), expected, &out);
    fill_breakdown(&mut case, submit_samples, readback_samples);
    case
}

fn submit_prepared_direct(
    context: &mut BrowserWebGpuMathContext,
    prepared: &BrowserWebGpuPreparedMath,
) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
    match prepared {
        BrowserWebGpuPreparedMath::MatmulF32 {
            prepared,
            rows,
            cols,
            selection: _,
        } => context.submit_resident_matmul_f32(prepared, *rows, *cols),
        BrowserWebGpuPreparedMath::MatrixAddF32 {
            prepared,
            rows: _,
            cols: _,
            len,
            selection: _,
        }
        | BrowserWebGpuPreparedMath::TensorAddF32 {
            prepared,
            dims: _,
            len,
            selection: _,
        } => context.submit_resident_elementwise_f32(prepared, *len),
    }
}

async fn run_prepared_auto_resident_pipelined_case(
    config: &BrowserMathBenchConfig,
    mode: BrowserBenchMode,
    mut case: BrowserMathBenchCase,
    adapter: &mut BrowserWebGpuAutoMathAdapter,
    expected: &[f32],
    prepared: &BrowserWebGpuPreparedMath,
) -> BrowserMathBenchCase {
    let batch_depth = async_batch_depth(mode, config);
    let mut out = vec![0.0; prepared.len()];
    let mut samples = Vec::with_capacity(config.sample_iters);
    let mut submit_samples = Vec::with_capacity(config.sample_iters);
    let mut readback_samples = Vec::with_capacity(config.sample_iters);
    let mut error = None;
    for _ in 0..config.warmup_iters {
        let mut submitted = Vec::with_capacity(batch_depth);
        for _ in 0..batch_depth {
            match adapter.submit_prepared(prepared) {
                Ok(current) => submitted.push(current),
                Err(current) => {
                    error = Some(current);
                    break;
                }
            }
        }
        yield_to_browser().await;
        for current in submitted {
            resize_out_for_submitted(&mut out, current.len());
            if let Err(current) = adapter.read_submitted_values_into(current, &mut out).await {
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
                match adapter.submit_prepared(prepared) {
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
                resize_out_for_submitted(&mut out, current.len());
                if let Err(current) = adapter.read_submitted_values_into(current, &mut out).await {
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
    case = finish_gpu_case(case, error, samples, adapter.stats(), expected, &out);
    fill_breakdown(&mut case, submit_samples, readback_samples);
    case
}
