use crate::model::{
    BrowserBenchMode, BrowserMathBenchCase, BrowserMathBenchShape, BrowserMathBenchStability,
};
use crate::stats::median_sample;

pub fn browser_math_bench_stability(
    cases: &[BrowserMathBenchCase],
) -> Vec<BrowserMathBenchStability> {
    let mut groups = Vec::<(&'static str, BrowserMathBenchShape, BrowserBenchMode)>::new();
    for case in cases {
        if case.median_ms.is_none() || !case.correctness.passed {
            continue;
        }
        let key = (case.op, case.shape, case.mode);
        if !groups.contains(&key) {
            groups.push(key);
        }
    }
    groups
        .into_iter()
        .map(|(op, shape, mode)| {
            let samples = cases
                .iter()
                .filter(|case| {
                    case.op == op
                        && case.shape == shape
                        && case.mode == mode
                        && case.correctness.passed
                })
                .filter_map(|case| case.median_ms)
                .collect::<Vec<_>>();
            let median = median_sample(samples.clone());
            let min = samples.iter().copied().reduce(f64::min);
            let max = samples.iter().copied().reduce(f64::max);
            BrowserMathBenchStability {
                op,
                shape,
                mode,
                measured_rounds: samples.len(),
                median_of_medians_ms: median,
                min_median_ms: min,
                max_median_ms: max,
                median_mad_ms: median.and_then(|center| {
                    median_sample(
                        samples
                            .iter()
                            .map(|sample| (sample - center).abs())
                            .collect(),
                    )
                }),
                spread_ratio: match (min, max) {
                    (Some(min), Some(max)) if min > 0.0 => Some(max / min),
                    _ => None,
                },
            }
        })
        .collect()
}
