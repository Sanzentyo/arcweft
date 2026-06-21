#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::model::BrowserMathBenchCorrectness;

pub(crate) fn compare(
    expected: &[f32],
    actual: &[f32],
    abs_tol: f32,
    rel_tol: f32,
) -> BrowserMathBenchCorrectness {
    let mut max_abs = 0.0_f32;
    let mut max_rel = 0.0_f32;
    for (expected, actual) in expected.iter().zip(actual) {
        let abs = (expected - actual).abs();
        let rel = abs / expected.abs().max(1.0);
        max_abs = max_abs.max(abs);
        max_rel = max_rel.max(rel);
    }
    BrowserMathBenchCorrectness {
        passed: expected.len() == actual.len() && max_abs <= abs_tol && max_rel <= rel_tol,
        max_abs,
        max_rel,
    }
}

pub(crate) fn checksum(values: &[f32]) -> f64 {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| f64::from(*value) * ((index + 1) as f64))
        .sum()
}

pub(crate) fn matmul_abs_tol(shared: usize) -> f32 {
    (4.0 * f32::EPSILON * shared as f32).max(1.0e-4)
}
