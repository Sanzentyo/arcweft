use crate::correctness::{checksum, compare};
use crate::model::{
    BrowserBenchMode, BrowserMathBenchCase, BrowserMathBenchConfig, BrowserMathBenchCorrectness,
    BrowserMathBenchShape,
};
use crate::stats::median_sample;
use arcweft_runtime_accelerator::math::browser_webgpu::{
    BrowserWebGpuError, BrowserWebGpuMathStats,
};

use super::browser::{fallback_reason, now_ms};
use super::estimate::{estimated_flops, estimated_work_items, estimated_workgroups};

pub(crate) fn empty_case(
    case_id: String,
    op: &'static str,
    shape: BrowserMathBenchShape,
    mode: BrowserBenchMode,
    round_index: usize,
    mode_order_index: usize,
    config: &BrowserMathBenchConfig,
) -> BrowserMathBenchCase {
    let workgroups = estimated_workgroups(op, &shape);
    let work_items = estimated_work_items(op, &shape);
    let estimated_flops = estimated_flops(op, &shape);
    let case_id = if config.repeat_rounds.max(1) > 1 {
        format!("{case_id}_round{round_index}_order{mode_order_index}")
    } else {
        case_id
    };
    BrowserMathBenchCase {
        case_id,
        op,
        shape,
        capacity: None,
        mode,
        round_index,
        mode_order_index,
        warmup_iters: config.warmup_iters,
        sample_iters: config.sample_iters,
        median_ms: None,
        mad_ms: None,
        min_ms: None,
        p95_ms: None,
        effective_gflops: None,
        submit_median_ms: None,
        readback_median_ms: None,
        submit_median_share: None,
        readback_median_share: None,
        bytes_uploaded: 0,
        bytes_readback: 0,
        dispatches: 0,
        async_submissions: 0,
        async_readbacks: 0,
        max_in_flight: 0,
        buffer_alloc_count: 0,
        buffer_reuse_count: 0,
        workgroups,
        work_items,
        estimated_flops,
        correctness: BrowserMathBenchCorrectness {
            passed: false,
            max_abs: 0.0,
            max_rel: 0.0,
        },
        fallback_reason: None,
        checksum: 0.0,
    }
}

pub(crate) fn measure_case(
    mut case: BrowserMathBenchCase,
    config: &BrowserMathBenchConfig,
    mut run: impl FnMut(),
) -> BrowserMathBenchCase {
    for _ in 0..config.warmup_iters {
        run();
    }
    let mut samples = Vec::with_capacity(config.sample_iters);
    for _ in 0..config.sample_iters {
        let start = now_ms();
        run();
        samples.push(now_ms() - start);
    }
    fill_timing(&mut case, samples);
    case
}

pub(crate) fn finish_gpu_case(
    mut case: BrowserMathBenchCase,
    error: Option<BrowserWebGpuError>,
    samples: Vec<f64>,
    stats: BrowserWebGpuMathStats,
    expected: &[f32],
    out: &[f32],
) -> BrowserMathBenchCase {
    if let Some(error) = error {
        return skipped_case(case, &fallback_reason(&error));
    }
    fill_timing(&mut case, samples);
    let tolerance = if case.op == "matmul_f32" {
        (1.0e-4, 1.0e-4)
    } else {
        (1.0e-6, 1.0e-6)
    };
    case.correctness = compare(expected, out, tolerance.0, tolerance.1);
    case.checksum = checksum(out);
    case.dispatches = stats.dispatches;
    case.async_submissions = stats.async_submissions;
    case.async_readbacks = stats.async_readbacks;
    case.max_in_flight = stats.max_in_flight;
    case.bytes_uploaded = stats.bytes_uploaded;
    case.bytes_readback = stats.bytes_downloaded;
    case.buffer_alloc_count = stats.buffer_creations + stats.readback_buffer_creations;
    case.buffer_reuse_count = stats.buffer_reuse_hits + stats.readback_buffer_reuse_hits;
    case
}

pub(crate) fn fill_breakdown(
    case: &mut BrowserMathBenchCase,
    submit_samples: Vec<f64>,
    readback_samples: Vec<f64>,
) {
    case.submit_median_ms = median_sample(submit_samples);
    case.readback_median_ms = median_sample(readback_samples);
    case.submit_median_share = median_share(case.submit_median_ms, case.median_ms);
    case.readback_median_share = median_share(case.readback_median_ms, case.median_ms);
}

pub(crate) fn skipped_case(mut case: BrowserMathBenchCase, reason: &str) -> BrowserMathBenchCase {
    case.fallback_reason = Some(reason.to_owned());
    case
}

pub(crate) fn fill_timing(case: &mut BrowserMathBenchCase, mut samples: Vec<f64>) {
    if samples.is_empty() {
        return;
    }
    samples.sort_by(f64::total_cmp);
    let median = samples[samples.len() / 2];
    let min = samples[0];
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let mut deviations = samples
        .iter()
        .map(|sample| (sample - median).abs())
        .collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    case.median_ms = Some(median);
    case.mad_ms = Some(deviations[deviations.len() / 2]);
    case.min_ms = Some(min);
    case.p95_ms = Some(p95);
    case.effective_gflops = effective_gflops(case.estimated_flops, median);
    case.submit_median_share = median_share(case.submit_median_ms, case.median_ms);
    case.readback_median_share = median_share(case.readback_median_ms, case.median_ms);
}

fn effective_gflops(estimated_flops: u64, median_ms: f64) -> Option<f64> {
    if estimated_flops == 0 || median_ms <= 0.0 {
        return None;
    }
    Some(estimated_flops as f64 / (median_ms * 1_000_000.0))
}

fn median_share(part: Option<f64>, whole: Option<f64>) -> Option<f64> {
    let part = part?;
    let whole = whole?;
    if whole <= 0.0 {
        return None;
    }
    Some(part / whole)
}
