use crate::model::BrowserMathBenchShape;

pub(crate) fn estimated_workgroups(op: &str, shape: &BrowserMathBenchShape) -> usize {
    match (op, shape) {
        ("tensor_add_f32", BrowserMathBenchShape::Len { len }) => len.div_ceil(256),
        ("matmul_f32", BrowserMathBenchShape::Matmul { rows, cols, .. }) => {
            rows.div_ceil(16) * cols.div_ceil(16)
        }
        _ => 0,
    }
}

pub(crate) fn estimated_work_items(op: &str, shape: &BrowserMathBenchShape) -> usize {
    match (op, shape) {
        ("tensor_add_f32", BrowserMathBenchShape::Len { len }) => {
            estimated_workgroups(op, shape) * 256.min((*len).max(1))
        }
        ("matmul_f32", BrowserMathBenchShape::Matmul { .. }) => {
            estimated_workgroups(op, shape) * 16 * 16
        }
        _ => 0,
    }
}

pub(crate) fn estimated_flops(op: &str, shape: &BrowserMathBenchShape) -> u64 {
    match (op, shape) {
        ("tensor_add_f32", BrowserMathBenchShape::Len { len }) => *len as u64,
        ("matmul_f32", BrowserMathBenchShape::Matmul { rows, shared, cols }) => {
            2 * *rows as u64 * *shared as u64 * *cols as u64
        }
        _ => 0,
    }
}
