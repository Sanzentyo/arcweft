use arcweft_runtime_accelerator::math::browser_webgpu::BrowserWebGpuMathResponse;

pub(crate) fn resize_out_for_submitted(out: &mut Vec<f32>, len: usize) {
    if out.len() != len {
        out.resize(len, 0.0);
    }
}

pub(crate) fn capture_tensor_response(response: &BrowserWebGpuMathResponse, out: &mut Vec<f32>) {
    if let Some(tensor) = response.tensor_f32() {
        out.clear();
        out.extend_from_slice(tensor.values());
    }
}

pub(crate) fn capture_matrix_response(response: &BrowserWebGpuMathResponse, out: &mut Vec<f32>) {
    if let Some(matrix) = response.matrix_f32() {
        out.clear();
        out.extend_from_slice(matrix.values());
    }
}

pub(crate) fn capture_response(response: &BrowserWebGpuMathResponse, out: &mut Vec<f32>) {
    capture_tensor_response(response, out);
    capture_matrix_response(response, out);
}
