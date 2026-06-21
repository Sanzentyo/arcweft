use crate::model::{
    BrowserBenchMode, BrowserMathBenchCapacity, BrowserMathBenchCase, BrowserMathBenchLimits,
    BrowserMathBenchRecommendation, BrowserMathBenchRecommendationReason, BrowserMathBenchShape,
};
use crate::policy::browser_math_policy_selection;
use crate::stats::median_sample;

pub fn browser_math_bench_recommendations(
    cases: &[BrowserMathBenchCase],
    limits: Option<BrowserMathBenchLimits>,
) -> Vec<BrowserMathBenchRecommendation> {
    let mut groups = Vec::<(&'static str, BrowserMathBenchShape)>::new();
    for case in cases {
        if !groups
            .iter()
            .any(|(op, shape)| *op == case.op && *shape == case.shape)
        {
            groups.push((case.op, case.shape));
        }
    }
    groups
        .into_iter()
        .map(|(op, shape)| recommend_browser_math_case(op, shape, cases, limits))
        .collect()
}

fn recommend_browser_math_case(
    op: &'static str,
    shape: BrowserMathBenchShape,
    cases: &[BrowserMathBenchCase],
    limits: Option<BrowserMathBenchLimits>,
) -> BrowserMathBenchRecommendation {
    let measurements = browser_math_bench_mode_measurements(op, shape, cases);
    let cpu = measurements
        .iter()
        .copied()
        .find(|measurement| measurement.mode == BrowserBenchMode::CpuWasm);
    let fastest_gpu = measurements
        .iter()
        .copied()
        .filter(|measurement| {
            !matches!(
                measurement.mode,
                BrowserBenchMode::Auto
                    | BrowserBenchMode::AutoPipelined
                    | BrowserBenchMode::AutoResidentPipelined
                    | BrowserBenchMode::AutoResidentDirectPipelined
                    | BrowserBenchMode::CpuWasm
                    | BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined
            )
        })
        .min_by(|lhs, rhs| lhs.median_ms.total_cmp(&rhs.median_ms));

    let mut recommendation = build_browser_math_recommendation(op, shape, cpu, fastest_gpu);
    attach_policy_recommendation(&mut recommendation, limits);
    recommendation
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BrowserMathBenchModeMeasurement {
    mode: BrowserBenchMode,
    capacity: Option<BrowserMathBenchCapacity>,
    median_ms: f64,
    mad_ms: Option<f64>,
    p95_ms: Option<f64>,
}

fn browser_math_bench_mode_measurements(
    op: &'static str,
    shape: BrowserMathBenchShape,
    cases: &[BrowserMathBenchCase],
) -> Vec<BrowserMathBenchModeMeasurement> {
    let mut groups = Vec::<(BrowserBenchMode, Option<BrowserMathBenchCapacity>)>::new();
    for case in cases
        .iter()
        .filter(|case| case.op == op && case.shape == shape && measured_case(case))
    {
        let key = (case.mode, case.capacity);
        if !groups.contains(&key) {
            groups.push(key);
        }
    }
    groups
        .into_iter()
        .filter_map(|(mode, capacity)| {
            let matching_cases = cases
                .iter()
                .filter(|case| {
                    case.op == op
                        && case.shape == shape
                        && case.mode == mode
                        && case.capacity == capacity
                        && measured_case(case)
                })
                .collect::<Vec<_>>();
            let medians = matching_cases
                .iter()
                .filter_map(|case| case.median_ms)
                .collect::<Vec<_>>();
            let mad_ms = median_sample(
                matching_cases
                    .iter()
                    .filter_map(|case| case.mad_ms)
                    .collect(),
            );
            let p95_ms = median_sample(
                matching_cases
                    .iter()
                    .filter_map(|case| case.p95_ms)
                    .collect(),
            );
            median_sample(medians).map(|median_ms| BrowserMathBenchModeMeasurement {
                mode,
                capacity,
                median_ms,
                mad_ms,
                p95_ms,
            })
        })
        .collect()
}

fn build_browser_math_recommendation(
    op: &'static str,
    shape: BrowserMathBenchShape,
    cpu: Option<BrowserMathBenchModeMeasurement>,
    fastest_gpu: Option<BrowserMathBenchModeMeasurement>,
) -> BrowserMathBenchRecommendation {
    match (cpu, fastest_gpu) {
        (Some(cpu), Some(gpu)) => {
            let cpu_ms = cpu.median_ms;
            let gpu_ms = gpu.median_ms;
            if gpu_ms > 0.0 && gpu_ms < cpu_ms {
                BrowserMathBenchRecommendation {
                    op,
                    shape,
                    selected_mode: Some(gpu.mode),
                    selected_capacity: gpu.capacity,
                    policy_mode: None,
                    policy_capacity: None,
                    policy_reason: None,
                    policy_matches_selected: None,
                    selected_median_ms: Some(gpu_ms),
                    selected_mad_ms: gpu.mad_ms,
                    selected_p95_ms: gpu.p95_ms,
                    cpu_median_ms: Some(cpu_ms),
                    cpu_mad_ms: cpu.mad_ms,
                    cpu_p95_ms: cpu.p95_ms,
                    speedup: Some(cpu_ms / gpu_ms),
                    reason: BrowserMathBenchRecommendationReason::WebGpuFaster,
                }
            } else {
                BrowserMathBenchRecommendation {
                    op,
                    shape,
                    selected_mode: Some(BrowserBenchMode::CpuWasm),
                    selected_capacity: None,
                    policy_mode: None,
                    policy_capacity: None,
                    policy_reason: None,
                    policy_matches_selected: None,
                    selected_median_ms: Some(cpu_ms),
                    selected_mad_ms: cpu.mad_ms,
                    selected_p95_ms: cpu.p95_ms,
                    cpu_median_ms: Some(cpu_ms),
                    cpu_mad_ms: cpu.mad_ms,
                    cpu_p95_ms: cpu.p95_ms,
                    speedup: Some(1.0),
                    reason: BrowserMathBenchRecommendationReason::CpuFasterOrEqual,
                }
            }
        }
        (Some(cpu), None) => BrowserMathBenchRecommendation {
            op,
            shape,
            selected_mode: Some(BrowserBenchMode::CpuWasm),
            selected_capacity: None,
            policy_mode: None,
            policy_capacity: None,
            policy_reason: None,
            policy_matches_selected: None,
            selected_median_ms: Some(cpu.median_ms),
            selected_mad_ms: cpu.mad_ms,
            selected_p95_ms: cpu.p95_ms,
            cpu_median_ms: Some(cpu.median_ms),
            cpu_mad_ms: cpu.mad_ms,
            cpu_p95_ms: cpu.p95_ms,
            speedup: Some(1.0),
            reason: BrowserMathBenchRecommendationReason::NoMeasuredWebGpuCase,
        },
        (None, Some(gpu)) => BrowserMathBenchRecommendation {
            op,
            shape,
            selected_mode: Some(gpu.mode),
            selected_capacity: gpu.capacity,
            policy_mode: None,
            policy_capacity: None,
            policy_reason: None,
            policy_matches_selected: None,
            selected_median_ms: Some(gpu.median_ms),
            selected_mad_ms: gpu.mad_ms,
            selected_p95_ms: gpu.p95_ms,
            cpu_median_ms: None,
            cpu_mad_ms: None,
            cpu_p95_ms: None,
            speedup: None,
            reason: BrowserMathBenchRecommendationReason::MissingCpuBaseline,
        },
        (None, None) => BrowserMathBenchRecommendation {
            op,
            shape,
            selected_mode: None,
            selected_capacity: None,
            policy_mode: None,
            policy_capacity: None,
            policy_reason: None,
            policy_matches_selected: None,
            selected_median_ms: None,
            selected_mad_ms: None,
            selected_p95_ms: None,
            cpu_median_ms: None,
            cpu_mad_ms: None,
            cpu_p95_ms: None,
            speedup: None,
            reason: BrowserMathBenchRecommendationReason::NoMeasuredWebGpuCase,
        },
    }
}

fn measured_case(case: &BrowserMathBenchCase) -> bool {
    case.fallback_reason.is_none()
        && case.correctness.passed
        && case.median_ms.is_some_and(f64::is_finite)
}

fn attach_policy_recommendation(
    recommendation: &mut BrowserMathBenchRecommendation,
    limits: Option<BrowserMathBenchLimits>,
) {
    let Some(limits) = limits else {
        return;
    };
    let Some(policy) =
        browser_math_policy_selection(recommendation.op, recommendation.shape, limits)
    else {
        return;
    };
    recommendation.policy_mode = Some(policy.mode);
    recommendation.policy_capacity = policy.capacity;
    recommendation.policy_reason = Some(policy.reason);
    recommendation.policy_matches_selected = recommendation
        .selected_mode
        .map(|selected_mode| selected_mode == policy.mode);
}
