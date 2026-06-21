use arcweft_core::math::DenseMatrixF32;

use crate::model::MatmulShape;

pub(crate) fn deterministic_values(len: usize, seed: u32) -> Vec<f32> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let value = ((state >> 8) & 0xffff) as f32 / 65_535.0;
            value * 2.0 - 1.0
        })
        .collect()
}

pub(crate) fn add_cpu(lhs: &[f32], rhs: &[f32]) -> Vec<f32> {
    lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs + rhs).collect()
}

pub(crate) fn matmul_cpu(lhs: &[f32], rhs: &[f32], shape: MatmulShape) -> Vec<f32> {
    let lhs = DenseMatrixF32::new(shape.rows, shape.shared, lhs.to_vec()).expect("valid lhs");
    let rhs = DenseMatrixF32::new(shape.shared, shape.cols, rhs.to_vec()).expect("valid rhs");
    lhs.matmul_scalar(&rhs)
        .expect("valid matmul")
        .values()
        .to_vec()
}
