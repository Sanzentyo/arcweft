use arcweft_core::math::{
    DenseMatrix, DenseMatrixF32, DenseMatrixF64, DenseTensor, DenseTensorF32, DenseTensorF64,
    RuntimeMathError,
};
use std::ops::{Add, AddAssign, Mul};
use thiserror::Error;

/// Runtime math backend selection for built-in matrix and tensor operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeMathBackend {
    Scalar,
    Glam,
    Ndarray,
    Wgpu,
    #[default]
    Auto,
}

/// Reason recorded when `Auto` chooses a concrete math backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMathAutoSelectionReason {
    Matmul4x4Glam,
    MatmulWgpuWorkThreshold,
    MatmulCpuDefault,
    ElementwiseWgpuWorkThreshold,
    ElementwiseCpuDefault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeMathAcceleratorConfig {
    pub backend: RuntimeMathBackend,
    /// Minimum element count before `Auto` considers GPU dispatch.
    pub wgpu_min_elements: usize,
}

impl Default for RuntimeMathAcceleratorConfig {
    fn default() -> Self {
        Self {
            backend: RuntimeMathBackend::Auto,
            wgpu_min_elements: 67_108_864,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeMathStats {
    pub scalar_calls: usize,
    pub glam_calls: usize,
    pub ndarray_calls: usize,
    pub wgpu_calls: usize,
    pub fused_matmul_bias_add_calls: usize,
    pub fallback_calls: usize,
    pub bytes_borrowed: usize,
    pub bytes_copied: usize,
    pub bytes_uploaded: usize,
    pub bytes_downloaded: usize,
    pub gpu_buffer_creations: usize,
    pub gpu_buffer_reuse_hits: usize,
    pub gpu_staging_buffer_creations: usize,
    pub gpu_staging_buffer_reuse_hits: usize,
    pub gpu_reused_dispatches: usize,
    pub last_backend: Option<RuntimeMathBackend>,
    pub last_auto_reason: Option<RuntimeMathAutoSelectionReason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeMathBackendSelection {
    backend: RuntimeMathBackend,
    auto_reason: Option<RuntimeMathAutoSelectionReason>,
}

impl RuntimeMathBackendSelection {
    pub const fn backend(self) -> RuntimeMathBackend {
        self.backend
    }

    pub const fn auto_reason(self) -> Option<RuntimeMathAutoSelectionReason> {
        self.auto_reason
    }
}

/// Adapter-owned accelerator for dense `f32` matrix/tensor kernels.
pub struct RuntimeMathAccelerator {
    config: RuntimeMathAcceleratorConfig,
    stats: RuntimeMathStats,
    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    wgpu: Option<wgpu_backend::WgpuMathContext>,
}

/// Prepared GPU storage for repeatedly adding shape-compatible `f32` matrices.
pub struct RuntimePreparedMatrixAddF32 {
    rows: usize,
    cols: usize,
    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    gpu: wgpu_backend::PreparedAddBuffers,
}

/// Prepared GPU storage for repeatedly multiplying shape-compatible `f32` matrices.
pub struct RuntimePreparedMatrixMatmulF32 {
    rows: usize,
    shared: usize,
    cols: usize,
    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    gpu: wgpu_backend::PreparedMatmulBuffers,
}

/// Prepared GPU storage for repeatedly computing `bias_add(matmul(lhs, rhs), bias)`.
pub struct RuntimePreparedMatrixMatmulBiasAddF32 {
    rows: usize,
    shared: usize,
    cols: usize,
    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    gpu: wgpu_backend::PreparedMatmulBiasAddBuffers,
}

/// Prepared GPU storage for repeatedly adding shape-compatible `f32` tensors.
pub struct RuntimePreparedTensorAddF32 {
    dims: Vec<usize>,
    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    gpu: wgpu_backend::PreparedAddBuffers,
}

impl RuntimePreparedMatrixAddF32 {
    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub const fn cols(&self) -> usize {
        self.cols
    }
}

impl RuntimePreparedMatrixMatmulF32 {
    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub const fn shared(&self) -> usize {
        self.shared
    }

    pub const fn cols(&self) -> usize {
        self.cols
    }
}

impl RuntimePreparedMatrixMatmulBiasAddF32 {
    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub const fn shared(&self) -> usize {
        self.shared
    }

    pub const fn cols(&self) -> usize {
        self.cols
    }
}

impl RuntimePreparedTensorAddF32 {
    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    pub fn element_count(&self) -> usize {
        self.dims.iter().product()
    }
}

impl RuntimeMathAccelerator {
    pub fn new(config: RuntimeMathAcceleratorConfig) -> Self {
        Self {
            config,
            stats: RuntimeMathStats::default(),
            #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
            wgpu: None,
        }
    }

    pub const fn config(&self) -> RuntimeMathAcceleratorConfig {
        self.config
    }

    pub const fn stats(&self) -> RuntimeMathStats {
        self.stats
    }

    pub fn matmul_backend_selection(
        &self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> RuntimeMathBackendSelection {
        self.select_matmul_backend(lhs, rhs)
    }

    pub fn elementwise_backend_selection(&self, elements: usize) -> RuntimeMathBackendSelection {
        self.select_elementwise_backend(elements)
    }

    pub fn record_backend_selection(&mut self, selection: RuntimeMathBackendSelection) {
        self.stats.last_auto_reason = selection.auto_reason();
    }

    pub fn reset_stats(&mut self) {
        self.stats = RuntimeMathStats::default();
    }

    pub fn matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        self.record_matrix_inputs(lhs, rhs);
        let selection = self.select_matmul_backend(lhs, rhs);
        self.stats.last_auto_reason = selection.auto_reason;
        match selection.backend {
            RuntimeMathBackend::Scalar => self.matmul_scalar(lhs, rhs),
            RuntimeMathBackend::Glam => self.matmul_glam(lhs, rhs),
            RuntimeMathBackend::Ndarray => self.matmul_ndarray(lhs, rhs),
            RuntimeMathBackend::Wgpu => self.matmul_wgpu(lhs, rhs),
            RuntimeMathBackend::Auto => unreachable!("auto is resolved before dispatch"),
        }
    }

    pub fn matmul_bias_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        validate_matmul_bias(lhs, rhs, bias)?;
        self.record_matmul_bias_inputs(lhs, rhs, bias);
        self.stats.fused_matmul_bias_add_calls += 1;
        let selection = self.select_matmul_backend(lhs, rhs);
        self.stats.last_auto_reason = selection.auto_reason;
        match selection.backend {
            RuntimeMathBackend::Scalar => self.matmul_bias_add_scalar(lhs, rhs, bias),
            RuntimeMathBackend::Glam => {
                let out = self.matmul_glam(lhs, rhs)?;
                add_bias_to_matrix(out, bias)
            }
            RuntimeMathBackend::Ndarray => {
                let out = self.matmul_ndarray(lhs, rhs)?;
                add_bias_to_matrix(out, bias)
            }
            RuntimeMathBackend::Wgpu => self.matmul_bias_add_wgpu(lhs, rhs, bias),
            RuntimeMathBackend::Auto => unreachable!("auto is resolved before dispatch"),
        }
    }

    pub fn matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        self.record_matrix_inputs(lhs, rhs);
        let selection = self.select_elementwise_backend(lhs.values().len());
        self.stats.last_auto_reason = selection.auto_reason;
        match selection.backend {
            RuntimeMathBackend::Scalar => self.matrix_add_scalar(lhs, rhs),
            RuntimeMathBackend::Glam => self.matrix_add_glam(lhs, rhs),
            RuntimeMathBackend::Ndarray => self.matrix_add_ndarray(lhs, rhs),
            RuntimeMathBackend::Wgpu => self.matrix_add_wgpu(lhs, rhs),
            RuntimeMathBackend::Auto => unreachable!("auto is resolved before dispatch"),
        }
    }

    pub fn tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeMathAcceleratorError> {
        self.record_tensor_inputs(lhs, rhs);
        let selection = self.select_elementwise_backend(lhs.values().len());
        self.stats.last_auto_reason = selection.auto_reason;
        match selection.backend {
            RuntimeMathBackend::Scalar => self.tensor_add_scalar(lhs, rhs),
            RuntimeMathBackend::Glam => self.tensor_add_glam(lhs, rhs),
            RuntimeMathBackend::Ndarray => self.tensor_add_ndarray(lhs, rhs),
            RuntimeMathBackend::Wgpu => self.tensor_add_wgpu(lhs, rhs),
            RuntimeMathBackend::Auto => unreachable!("auto is resolved before dispatch"),
        }
    }

    pub fn matmul_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeMathAcceleratorError> {
        self.record_matrix_inputs(lhs, rhs);
        let selection = self.select_matmul_backend_f64(lhs, rhs);
        self.stats.last_auto_reason = selection.auto_reason;
        match selection.backend {
            RuntimeMathBackend::Scalar => self.matmul_scalar(lhs, rhs),
            RuntimeMathBackend::Glam => self.matmul_glam_f64(lhs, rhs),
            RuntimeMathBackend::Ndarray => self.matmul_ndarray_f64(lhs, rhs),
            RuntimeMathBackend::Wgpu => Err(RuntimeMathAcceleratorError::Backend(
                "wgpu backend does not support portable f64 matrix kernels".to_owned(),
            )),
            RuntimeMathBackend::Auto => unreachable!("auto is resolved before dispatch"),
        }
    }

    pub fn matrix_add_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeMathAcceleratorError> {
        self.record_matrix_inputs(lhs, rhs);
        let selection = self.select_elementwise_backend_f64();
        self.stats.last_auto_reason = selection.auto_reason;
        match selection.backend {
            RuntimeMathBackend::Scalar => self.matrix_add_scalar(lhs, rhs),
            RuntimeMathBackend::Glam => self.matrix_add_glam_f64(lhs, rhs),
            RuntimeMathBackend::Ndarray => self.matrix_add_ndarray_f64(lhs, rhs),
            RuntimeMathBackend::Wgpu => Err(RuntimeMathAcceleratorError::Backend(
                "wgpu backend does not support portable f64 matrix kernels".to_owned(),
            )),
            RuntimeMathBackend::Auto => unreachable!("auto is resolved before dispatch"),
        }
    }

    pub fn tensor_add_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeMathAcceleratorError> {
        self.record_tensor_inputs(lhs, rhs);
        let selection = self.select_elementwise_backend_f64();
        self.stats.last_auto_reason = selection.auto_reason;
        match selection.backend {
            RuntimeMathBackend::Scalar | RuntimeMathBackend::Glam => {
                self.tensor_add_scalar(lhs, rhs)
            }
            RuntimeMathBackend::Ndarray => self.tensor_add_ndarray_f64(lhs, rhs),
            RuntimeMathBackend::Wgpu => Err(RuntimeMathAcceleratorError::Backend(
                "wgpu backend does not support portable f64 tensor kernels".to_owned(),
            )),
            RuntimeMathBackend::Auto => unreachable!("auto is resolved before dispatch"),
        }
    }

    pub fn prepare_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<RuntimePreparedMatrixAddF32, RuntimeMathAcceleratorError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "add",
            }
            .into());
        }
        self.record_matrix_inputs(lhs, rhs);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self
                    .wgpu_context()
                    .and_then(|context| context.prepare_add(lhs.values(), rhs.values()))?;
                self.stats.bytes_uploaded +=
                    (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>();
                self.stats.bytes_copied +=
                    (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>();
                self.stats.gpu_buffer_creations += 4;
                Ok(RuntimePreparedMatrixAddF32 {
                    rows: lhs.rows(),
                    cols: lhs.cols(),
                    gpu,
                })
            }
            _ => {
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn prepare_matrix_add_f32_capacity(
        &mut self,
        rows: usize,
        cols: usize,
    ) -> Result<RuntimePreparedMatrixAddF32, RuntimeMathAcceleratorError> {
        let capacity_len = rows.saturating_mul(cols);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self
                    .wgpu_context()
                    .and_then(|context| context.prepare_add_capacity(capacity_len))?;
                self.stats.gpu_buffer_creations += 4;
                Ok(RuntimePreparedMatrixAddF32 { rows, cols, gpu })
            }
            _ => {
                let _ = (rows, cols, capacity_len);
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_matrix_add_f32(
        &mut self,
        prepared: &RuntimePreparedMatrixAddF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        let mut out = vec![0.0; prepared.rows * prepared.cols];
        self.run_prepared_matrix_add_f32_into(prepared, &mut out)?;
        DenseMatrixF32::new(prepared.rows, prepared.cols, out).map_err(Into::into)
    }

    pub fn run_prepared_matrix_add_f32_into(
        &mut self,
        prepared: &RuntimePreparedMatrixAddF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        let expected = prepared.rows * prepared.cols;
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_add(&prepared.gpu, out))?;
                self.record_prepared_gpu_dispatch(prepared.gpu.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_matrix_add_f32_shape_into(
        &mut self,
        prepared: &RuntimePreparedMatrixAddF32,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix add capacity is {}x{}, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_add(&prepared.gpu, out))?;
                self.record_prepared_gpu_dispatch(out.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn update_prepared_matrix_add_f32(
        &mut self,
        prepared: &RuntimePreparedMatrixAddF32,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "add",
            }
            .into());
        }
        if lhs.rows() > prepared.rows || lhs.cols() > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix add capacity is {}x{}, got {}x{}",
                prepared.rows,
                prepared.cols,
                lhs.rows(),
                lhs.cols()
            )));
        }
        self.record_matrix_inputs(lhs, rhs);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                self.wgpu_context()
                    .and_then(|context| context.update_prepared_add(&prepared.gpu, lhs.values(), rhs.values()))?;
                self.record_prepared_gpu_upload(lhs.values().len() + rhs.values().len());
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn prepare_matrix_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<RuntimePreparedMatrixMatmulF32, RuntimeMathAcceleratorError> {
        if lhs.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "matmul",
            }
            .into());
        }
        self.record_matrix_inputs(lhs, rhs);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self.wgpu_context().and_then(|context| {
                    context.prepare_matmul(
                        lhs.values(),
                        rhs.values(),
                        lhs.rows(),
                        lhs.cols(),
                        rhs.cols(),
                    )
                })?;
                self.stats.bytes_uploaded +=
                    (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>();
                self.stats.bytes_copied +=
                    (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>();
                self.stats.gpu_buffer_creations += 4;
                Ok(RuntimePreparedMatrixMatmulF32 {
                    rows: lhs.rows(),
                    shared: lhs.cols(),
                    cols: rhs.cols(),
                    gpu,
                })
            }
            _ => {
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn prepare_matrix_matmul_f32_capacity(
        &mut self,
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<RuntimePreparedMatrixMatmulF32, RuntimeMathAcceleratorError> {
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self
                    .wgpu_context()
                    .and_then(|context| context.prepare_matmul_capacity(rows, shared, cols))?;
                self.stats.gpu_buffer_creations += 4;
                Ok(RuntimePreparedMatrixMatmulF32 {
                    rows,
                    shared,
                    cols,
                    gpu,
                })
            }
            _ => {
                let _ = (rows, shared, cols);
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_matrix_matmul_f32(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        let mut out = vec![0.0; prepared.rows * prepared.cols];
        self.run_prepared_matrix_matmul_f32_into(prepared, &mut out)?;
        DenseMatrixF32::new(prepared.rows, prepared.cols, out).map_err(Into::into)
    }

    pub fn run_prepared_matrix_matmul_f32_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        let expected = prepared.rows * prepared.cols;
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_matmul(&prepared.gpu, out))?;
                self.record_prepared_gpu_dispatch(prepared.gpu.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_matrix_matmul_f32_shape_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_matmul_shape(&prepared.gpu, rows, cols, out))?;
                self.record_prepared_gpu_dispatch(out.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn submit_prepared_matrix_matmul_f32_without_readback(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.submit_prepared_matrix_matmul_f32_shape_without_readback(
            prepared,
            prepared.rows,
            prepared.cols,
        )
    }

    pub fn submit_prepared_matrix_matmul_f32_shape_without_readback(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                self.wgpu_context()
                    .and_then(|context| {
                        context.submit_prepared_matmul_shape_without_readback(
                            &prepared.gpu,
                            rows,
                            cols,
                        )
                    })?;
                self.record_prepared_gpu_submit_with_reuse(4);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn read_prepared_matrix_matmul_f32_output_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.read_prepared_matrix_matmul_f32_shape_output_into(
            prepared,
            prepared.rows,
            prepared.cols,
            out,
        )
    }

    pub fn read_prepared_matrix_matmul_f32_shape_output_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self.wgpu_context().and_then(|context| {
                    context.read_prepared_matmul_shape_output(&prepared.gpu, rows, cols, out)
                })?;
                self.record_prepared_gpu_readback(out.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn update_prepared_matrix_matmul_f32(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulF32,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if lhs.cols() != rhs.rows() {
            return Err(RuntimeMathError::MatrixShapeMismatch {
                lhs: lhs.shape(),
                rhs: rhs.shape(),
                op: "matmul",
            }
            .into());
        }
        if lhs.rows() > prepared.rows || lhs.cols() > prepared.shared || rhs.cols() > prepared.cols
        {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul capacity is {}x{} and {}x{}, got {}x{} and {}x{}",
                prepared.rows,
                prepared.shared,
                prepared.shared,
                prepared.cols,
                lhs.rows(),
                lhs.cols(),
                rhs.rows(),
                rhs.cols()
            )));
        }
        self.record_matrix_inputs(lhs, rhs);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                self.wgpu_context().and_then(|context| {
                    context.update_prepared_matmul(
                        &prepared.gpu,
                        lhs.values(),
                        rhs.values(),
                        lhs.rows(),
                        lhs.cols(),
                        rhs.cols(),
                    )
                })?;
                self.record_prepared_gpu_upload(lhs.values().len() + rhs.values().len());
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn prepare_matrix_matmul_bias_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<RuntimePreparedMatrixMatmulBiasAddF32, RuntimeMathAcceleratorError> {
        validate_matmul_bias(lhs, rhs, bias)?;
        self.record_matmul_bias_inputs(lhs, rhs, bias);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self.wgpu_context().and_then(|context| {
                    context.prepare_matmul_bias_add(
                        lhs.values(),
                        rhs.values(),
                        bias.values(),
                        lhs.rows(),
                        lhs.cols(),
                        rhs.cols(),
                    )
                })?;
                self.stats.bytes_uploaded +=
                    (lhs.values().len() + rhs.values().len() + bias.values().len())
                        * std::mem::size_of::<f32>();
                self.stats.bytes_copied +=
                    (lhs.values().len() + rhs.values().len() + bias.values().len())
                        * std::mem::size_of::<f32>();
                self.stats.gpu_buffer_creations += 7;
                Ok(RuntimePreparedMatrixMatmulBiasAddF32 {
                    rows: lhs.rows(),
                    shared: lhs.cols(),
                    cols: rhs.cols(),
                    gpu,
                })
            }
            _ => {
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn prepare_matrix_matmul_bias_add_f32_capacity(
        &mut self,
        rows: usize,
        shared: usize,
        cols: usize,
    ) -> Result<RuntimePreparedMatrixMatmulBiasAddF32, RuntimeMathAcceleratorError> {
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self
                    .wgpu_context()
                    .and_then(|context| context.prepare_matmul_bias_add_capacity(rows, shared, cols))?;
                self.stats.gpu_buffer_creations += 7;
                Ok(RuntimePreparedMatrixMatmulBiasAddF32 {
                    rows,
                    shared,
                    cols,
                    gpu,
                })
            }
            _ => {
                let _ = (rows, shared, cols);
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_matrix_matmul_bias_add_f32(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        let mut out = vec![0.0; prepared.rows * prepared.cols];
        self.run_prepared_matrix_matmul_bias_add_f32_into(prepared, &mut out)?;
        DenseMatrixF32::new(prepared.rows, prepared.cols, out).map_err(Into::into)
    }

    pub fn run_prepared_matrix_matmul_bias_add_f32_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        let expected = prepared.rows * prepared.cols;
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_matmul_bias_add(&prepared.gpu, out))?;
                self.stats.fused_matmul_bias_add_calls += 1;
                self.record_prepared_gpu_dispatch_with_reuse(prepared.gpu.len(), readback, 7);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_matrix_matmul_bias_add_f32_shape_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul-bias-add capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_matmul_bias_add_shape(&prepared.gpu, rows, cols, out))?;
                self.stats.fused_matmul_bias_add_calls += 1;
                self.record_prepared_gpu_dispatch_with_reuse(out.len(), readback, 7);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn submit_prepared_matrix_matmul_bias_add_f32_without_readback(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.submit_prepared_matrix_matmul_bias_add_f32_shape_without_readback(
            prepared,
            prepared.rows,
            prepared.cols,
        )
    }

    pub fn submit_prepared_matrix_matmul_bias_add_f32_shape_without_readback(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul-bias-add capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                self.wgpu_context()
                    .and_then(|context| {
                        context.submit_prepared_matmul_bias_add_shape_without_readback(
                            &prepared.gpu,
                            rows,
                            cols,
                        )
                    })?;
                self.stats.fused_matmul_bias_add_calls += 1;
                self.record_prepared_gpu_submit_with_reuse(7);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn read_prepared_matrix_matmul_bias_add_f32_output_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.read_prepared_matrix_matmul_bias_add_f32_shape_output_into(
            prepared,
            prepared.rows,
            prepared.cols,
            out,
        )
    }

    pub fn read_prepared_matrix_matmul_bias_add_f32_shape_output_into(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
        rows: usize,
        cols: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul-bias-add capacity is {}x{} output, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        let expected = rows.saturating_mul(cols);
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self.wgpu_context().and_then(|context| {
                    context.read_prepared_matmul_bias_add_shape_output(
                        &prepared.gpu,
                        rows,
                        cols,
                        out,
                    )
                })?;
                self.record_prepared_gpu_readback(out.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn update_prepared_matrix_matmul_bias_add_f32(
        &mut self,
        prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        validate_matmul_bias(lhs, rhs, bias)?;
        if lhs.rows() > prepared.rows || lhs.cols() > prepared.shared || rhs.cols() > prepared.cols
        {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix matmul-bias-add capacity is {}x{} and {}x{}, got {}x{} and {}x{}",
                prepared.rows,
                prepared.shared,
                prepared.shared,
                prepared.cols,
                lhs.rows(),
                lhs.cols(),
                rhs.rows(),
                rhs.cols()
            )));
        }
        self.record_matmul_bias_inputs(lhs, rhs, bias);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                self.wgpu_context().and_then(|context| {
                    context.update_prepared_matmul_bias_add(
                        &prepared.gpu,
                        lhs.values(),
                        rhs.values(),
                        bias.values(),
                        wgpu_backend::MatmulShape {
                            rows: lhs.rows(),
                            shared: lhs.cols(),
                            cols: rhs.cols(),
                        },
                    )
                })?;
                self.record_prepared_gpu_upload_with_reuse(
                    lhs.values().len() + rhs.values().len() + bias.values().len(),
                    5,
                );
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn prepare_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<RuntimePreparedTensorAddF32, RuntimeMathAcceleratorError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::TensorShapeMismatch {
                lhs: lhs.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "add",
            }
            .into());
        }
        self.record_tensor_inputs(lhs, rhs);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self
                    .wgpu_context()
                    .and_then(|context| context.prepare_add(lhs.values(), rhs.values()))?;
                self.stats.bytes_uploaded +=
                    (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>();
                self.stats.bytes_copied +=
                    (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>();
                self.stats.gpu_buffer_creations += 4;
                Ok(RuntimePreparedTensorAddF32 {
                    dims: lhs.shape().dims().to_vec(),
                    gpu,
                })
            }
            _ => {
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn prepare_tensor_add_f32_capacity(
        &mut self,
        capacity_len: usize,
    ) -> Result<RuntimePreparedTensorAddF32, RuntimeMathAcceleratorError> {
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let gpu = self
                    .wgpu_context()
                    .and_then(|context| context.prepare_add_capacity(capacity_len))?;
                self.stats.gpu_buffer_creations += 4;
                Ok(RuntimePreparedTensorAddF32 {
                    dims: vec![capacity_len],
                    gpu,
                })
            }
            _ => {
                let _ = capacity_len;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_tensor_add_f32(
        &mut self,
        prepared: &RuntimePreparedTensorAddF32,
    ) -> Result<DenseTensorF32, RuntimeMathAcceleratorError> {
        let mut out = vec![0.0; prepared.element_count()];
        self.run_prepared_tensor_add_f32_into(prepared, &mut out)?;
        DenseTensorF32::new(prepared.dims.clone(), out).map_err(Into::into)
    }

    pub fn run_prepared_tensor_add_f32_into(
        &mut self,
        prepared: &RuntimePreparedTensorAddF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        let expected = prepared.element_count();
        if out.len() != expected {
            return Err(RuntimeMathError::InvalidElementCount {
                expected,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_add(&prepared.gpu, out))?;
                self.record_prepared_gpu_dispatch(prepared.gpu.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn run_prepared_tensor_add_f32_len_into(
        &mut self,
        prepared: &RuntimePreparedTensorAddF32,
        len: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if len > prepared.element_count() {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared tensor add capacity is {}, got {}",
                prepared.element_count(),
                len
            )));
        }
        if out.len() != len {
            return Err(RuntimeMathError::InvalidElementCount {
                expected: len,
                found: out.len(),
            }
            .into());
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let readback = self
                    .wgpu_context()
                    .and_then(|context| context.dispatch_prepared_add(&prepared.gpu, out))?;
                self.record_prepared_gpu_dispatch(out.len(), readback);
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    pub fn update_prepared_tensor_add_f32(
        &mut self,
        prepared: &RuntimePreparedTensorAddF32,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if lhs.shape() != rhs.shape() {
            return Err(RuntimeMathError::TensorShapeMismatch {
                lhs: lhs.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "add",
            }
            .into());
        }
        if lhs.values().len() > prepared.element_count() {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared tensor add capacity is {}, got {}",
                prepared.element_count(),
                lhs.values().len()
            )));
        }
        self.record_tensor_inputs(lhs, rhs);
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                self.wgpu_context()
                    .and_then(|context| context.update_prepared_add(&prepared.gpu, lhs.values(), rhs.values()))?;
                self.record_prepared_gpu_upload(lhs.values().len() + rhs.values().len());
                Ok(())
            }
            _ => {
                let _ = prepared;
                Err(wgpu_unavailable_error())
            }
        }
    }

    fn select_matmul_backend<T>(
        &self,
        lhs: &DenseMatrix<T>,
        rhs: &DenseMatrix<T>,
    ) -> RuntimeMathBackendSelection {
        match self.config.backend {
            RuntimeMathBackend::Auto if lhs.rows() == 4 && lhs.cols() == 4 && rhs.cols() == 4 => {
                RuntimeMathBackendSelection {
                    backend: RuntimeMathBackend::Glam,
                    auto_reason: Some(RuntimeMathAutoSelectionReason::Matmul4x4Glam),
                }
            }
            RuntimeMathBackend::Auto
                if wgpu_backend_enabled()
                    && matmul_work_items(lhs, rhs) >= self.config.wgpu_min_elements =>
            {
                RuntimeMathBackendSelection {
                    backend: RuntimeMathBackend::Wgpu,
                    auto_reason: Some(RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold),
                }
            }
            RuntimeMathBackend::Auto => RuntimeMathBackendSelection {
                backend: RuntimeMathBackend::Ndarray,
                auto_reason: Some(RuntimeMathAutoSelectionReason::MatmulCpuDefault),
            },
            backend => RuntimeMathBackendSelection {
                backend,
                auto_reason: None,
            },
        }
    }

    fn select_matmul_backend_f64(
        &self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> RuntimeMathBackendSelection {
        match self.config.backend {
            RuntimeMathBackend::Auto if lhs.rows() == 4 && lhs.cols() == 4 && rhs.cols() == 4 => {
                RuntimeMathBackendSelection {
                    backend: RuntimeMathBackend::Glam,
                    auto_reason: Some(RuntimeMathAutoSelectionReason::Matmul4x4Glam),
                }
            }
            RuntimeMathBackend::Auto => RuntimeMathBackendSelection {
                backend: RuntimeMathBackend::Ndarray,
                auto_reason: Some(RuntimeMathAutoSelectionReason::MatmulCpuDefault),
            },
            backend => RuntimeMathBackendSelection {
                backend,
                auto_reason: None,
            },
        }
    }

    fn select_elementwise_backend(&self, elements: usize) -> RuntimeMathBackendSelection {
        match self.config.backend {
            RuntimeMathBackend::Auto
                if wgpu_backend_enabled() && elements >= self.config.wgpu_min_elements =>
            {
                RuntimeMathBackendSelection {
                    backend: RuntimeMathBackend::Wgpu,
                    auto_reason: Some(RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold),
                }
            }
            RuntimeMathBackend::Auto => RuntimeMathBackendSelection {
                backend: RuntimeMathBackend::Ndarray,
                auto_reason: Some(RuntimeMathAutoSelectionReason::ElementwiseCpuDefault),
            },
            backend => RuntimeMathBackendSelection {
                backend,
                auto_reason: None,
            },
        }
    }

    fn select_elementwise_backend_f64(&self) -> RuntimeMathBackendSelection {
        match self.config.backend {
            RuntimeMathBackend::Auto => RuntimeMathBackendSelection {
                backend: RuntimeMathBackend::Ndarray,
                auto_reason: Some(RuntimeMathAutoSelectionReason::ElementwiseCpuDefault),
            },
            backend => RuntimeMathBackendSelection {
                backend,
                auto_reason: None,
            },
        }
    }

    fn matmul_scalar<T>(
        &mut self,
        lhs: &DenseMatrix<T>,
        rhs: &DenseMatrix<T>,
    ) -> Result<DenseMatrix<T>, RuntimeMathAcceleratorError>
    where
        T: Copy + Default + AddAssign + Mul<Output = T>,
    {
        self.stats.scalar_calls += 1;
        self.stats.last_backend = Some(RuntimeMathBackend::Scalar);
        lhs.matmul_scalar(rhs).map_err(Into::into)
    }

    fn matrix_add_scalar<T>(
        &mut self,
        lhs: &DenseMatrix<T>,
        rhs: &DenseMatrix<T>,
    ) -> Result<DenseMatrix<T>, RuntimeMathAcceleratorError>
    where
        T: Copy + Add<Output = T>,
    {
        self.stats.scalar_calls += 1;
        self.stats.last_backend = Some(RuntimeMathBackend::Scalar);
        lhs.add_scalar(rhs).map_err(Into::into)
    }

    fn tensor_add_scalar<T>(
        &mut self,
        lhs: &DenseTensor<T>,
        rhs: &DenseTensor<T>,
    ) -> Result<DenseTensor<T>, RuntimeMathAcceleratorError>
    where
        T: Copy + Add<Output = T>,
    {
        self.stats.scalar_calls += 1;
        self.stats.last_backend = Some(RuntimeMathBackend::Scalar);
        lhs.add_scalar(rhs).map_err(Into::into)
    }

    fn matmul_bias_add_scalar(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        self.stats.scalar_calls += 1;
        self.stats.last_backend = Some(RuntimeMathBackend::Scalar);
        let rows = lhs.rows();
        let shared = lhs.cols();
        let cols = rhs.cols();
        let mut out = vec![0.0; rows * cols];
        for row in 0..rows {
            let out_row = &mut out[row * cols..(row + 1) * cols];
            out_row.copy_from_slice(bias.values());
            let lhs_row = &lhs.values()[row * shared..(row + 1) * shared];
            for (k, lhs_value) in lhs_row.iter().copied().enumerate() {
                let rhs_row = &rhs.values()[k * cols..(k + 1) * cols];
                for (out_value, rhs_value) in out_row.iter_mut().zip(rhs_row.iter().copied()) {
                    *out_value += lhs_value * rhs_value;
                }
            }
        }
        DenseMatrixF32::new(rows, cols, out).map_err(Into::into)
    }

    fn matmul_glam(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-glam")]
        {
            if lhs.rows() == 4 && lhs.cols() == 4 && rhs.rows() == 4 && rhs.cols() == 4 {
                self.stats.glam_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Glam);
                let lhs_matrix = glam::Mat4::from_cols_array(&row_major_4x4_to_cols(lhs.values()));
                let rhs_matrix = glam::Mat4::from_cols_array(&row_major_4x4_to_cols(rhs.values()));
                let out = cols_4x4_to_row_major((lhs_matrix * rhs_matrix).to_cols_array());
                return DenseMatrixF32::new(4, 4, out).map_err(Into::into);
            }
        }
        self.stats.fallback_calls += 1;
        self.matmul_scalar(lhs, rhs)
    }

    fn matrix_add_glam(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-glam")]
        {
            if lhs.rows() == 4 && lhs.cols() == 4 && rhs.rows() == 4 && rhs.cols() == 4 {
                self.stats.glam_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Glam);
                let lhs_matrix = glam::Mat4::from_cols_array(&row_major_4x4_to_cols(lhs.values()));
                let rhs_matrix = glam::Mat4::from_cols_array(&row_major_4x4_to_cols(rhs.values()));
                let out = cols_4x4_to_row_major((lhs_matrix + rhs_matrix).to_cols_array());
                return DenseMatrixF32::new(4, 4, out).map_err(Into::into);
            }
        }
        self.stats.fallback_calls += 1;
        self.matrix_add_scalar(lhs, rhs)
    }

    fn matmul_glam_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-glam")]
        {
            if lhs.rows() == 4 && lhs.cols() == 4 && rhs.rows() == 4 && rhs.cols() == 4 {
                self.stats.glam_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Glam);
                let lhs_matrix = glam::DMat4::from_cols_array(&row_major_4x4_to_cols(lhs.values()));
                let rhs_matrix = glam::DMat4::from_cols_array(&row_major_4x4_to_cols(rhs.values()));
                let out = cols_4x4_to_row_major((lhs_matrix * rhs_matrix).to_cols_array());
                return DenseMatrixF64::new(4, 4, out).map_err(Into::into);
            }
        }
        self.stats.fallback_calls += 1;
        self.matmul_scalar(lhs, rhs)
    }

    fn matrix_add_glam_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-glam")]
        {
            if lhs.rows() == 4 && lhs.cols() == 4 && rhs.rows() == 4 && rhs.cols() == 4 {
                self.stats.glam_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Glam);
                let lhs_matrix = glam::DMat4::from_cols_array(&row_major_4x4_to_cols(lhs.values()));
                let rhs_matrix = glam::DMat4::from_cols_array(&row_major_4x4_to_cols(rhs.values()));
                let out = cols_4x4_to_row_major((lhs_matrix + rhs_matrix).to_cols_array());
                return DenseMatrixF64::new(4, 4, out).map_err(Into::into);
            }
        }
        self.stats.fallback_calls += 1;
        self.matrix_add_scalar(lhs, rhs)
    }

    fn tensor_add_glam(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeMathAcceleratorError> {
        self.stats.fallback_calls += 1;
        self.tensor_add_scalar(lhs, rhs)
    }

    fn matmul_ndarray(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-ndarray")]
        {
            if lhs.cols() == rhs.rows() {
                self.stats.ndarray_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Ndarray);
                let lhs_view =
                    ndarray::ArrayView2::from_shape((lhs.rows(), lhs.cols()), lhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let rhs_view =
                    ndarray::ArrayView2::from_shape((rhs.rows(), rhs.cols()), rhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let out = lhs_view.dot(&rhs_view);
                DenseMatrixF32::new(lhs.rows(), rhs.cols(), out.into_raw_vec_and_offset().0)
                    .map_err(Into::into)
            } else {
                lhs.matmul_scalar(rhs).map_err(Into::into)
            }
        }
        #[cfg(not(feature = "math-ndarray"))]
        {
            self.stats.fallback_calls += 1;
            self.matmul_scalar(lhs, rhs)
        }
    }

    fn matrix_add_ndarray(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-ndarray")]
        {
            if lhs.shape() == rhs.shape() {
                self.stats.ndarray_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Ndarray);
                let lhs_view =
                    ndarray::ArrayView2::from_shape((lhs.rows(), lhs.cols()), lhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let rhs_view =
                    ndarray::ArrayView2::from_shape((rhs.rows(), rhs.cols()), rhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let out = &lhs_view + &rhs_view;
                DenseMatrixF32::new(lhs.rows(), lhs.cols(), out.into_raw_vec_and_offset().0)
                    .map_err(Into::into)
            } else {
                lhs.add_scalar(rhs).map_err(Into::into)
            }
        }
        #[cfg(not(feature = "math-ndarray"))]
        {
            self.stats.fallback_calls += 1;
            self.matrix_add_scalar(lhs, rhs)
        }
    }

    fn tensor_add_ndarray(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-ndarray")]
        {
            if lhs.shape() == rhs.shape() {
                self.stats.ndarray_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Ndarray);
                let lhs_view = ndarray::ArrayViewD::from_shape(
                    ndarray::IxDyn(lhs.shape().dims()),
                    lhs.values(),
                )
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let rhs_view = ndarray::ArrayViewD::from_shape(
                    ndarray::IxDyn(rhs.shape().dims()),
                    rhs.values(),
                )
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let out = &lhs_view + &rhs_view;
                DenseTensorF32::new(lhs.shape().dims().to_vec(), out.into_raw_vec_and_offset().0)
                    .map_err(Into::into)
            } else {
                lhs.add_scalar(rhs).map_err(Into::into)
            }
        }
        #[cfg(not(feature = "math-ndarray"))]
        {
            self.stats.fallback_calls += 1;
            self.tensor_add_scalar(lhs, rhs)
        }
    }

    fn matmul_ndarray_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-ndarray")]
        {
            if lhs.cols() == rhs.rows() {
                self.stats.ndarray_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Ndarray);
                let lhs_view =
                    ndarray::ArrayView2::from_shape((lhs.rows(), lhs.cols()), lhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let rhs_view =
                    ndarray::ArrayView2::from_shape((rhs.rows(), rhs.cols()), rhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let out = lhs_view.dot(&rhs_view);
                DenseMatrixF64::new(lhs.rows(), rhs.cols(), out.into_raw_vec_and_offset().0)
                    .map_err(Into::into)
            } else {
                lhs.matmul_scalar(rhs).map_err(Into::into)
            }
        }
        #[cfg(not(feature = "math-ndarray"))]
        {
            self.stats.fallback_calls += 1;
            self.matmul_scalar(lhs, rhs)
        }
    }

    fn matrix_add_ndarray_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-ndarray")]
        {
            if lhs.shape() == rhs.shape() {
                self.stats.ndarray_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Ndarray);
                let lhs_view =
                    ndarray::ArrayView2::from_shape((lhs.rows(), lhs.cols()), lhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let rhs_view =
                    ndarray::ArrayView2::from_shape((rhs.rows(), rhs.cols()), rhs.values())
                        .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let out = &lhs_view + &rhs_view;
                DenseMatrixF64::new(lhs.rows(), lhs.cols(), out.into_raw_vec_and_offset().0)
                    .map_err(Into::into)
            } else {
                lhs.add_scalar(rhs).map_err(Into::into)
            }
        }
        #[cfg(not(feature = "math-ndarray"))]
        {
            self.stats.fallback_calls += 1;
            self.matrix_add_scalar(lhs, rhs)
        }
    }

    fn tensor_add_ndarray_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-ndarray")]
        {
            if lhs.shape() == rhs.shape() {
                self.stats.ndarray_calls += 1;
                self.stats.last_backend = Some(RuntimeMathBackend::Ndarray);
                let lhs_view = ndarray::ArrayViewD::from_shape(
                    ndarray::IxDyn(lhs.shape().dims()),
                    lhs.values(),
                )
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let rhs_view = ndarray::ArrayViewD::from_shape(
                    ndarray::IxDyn(rhs.shape().dims()),
                    rhs.values(),
                )
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
                let out = &lhs_view + &rhs_view;
                DenseTensorF64::new(lhs.shape().dims().to_vec(), out.into_raw_vec_and_offset().0)
                    .map_err(Into::into)
            } else {
                lhs.add_scalar(rhs).map_err(Into::into)
            }
        }
        #[cfg(not(feature = "math-ndarray"))]
        {
            self.stats.fallback_calls += 1;
            self.tensor_add_scalar(lhs, rhs)
        }
    }

    fn matmul_wgpu(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let result = self
                    .wgpu_context()
                    .and_then(|context| context.matmul_f32(lhs, rhs));
                match result {
                    Ok((value, readback)) => {
                        self.stats.wgpu_calls += 1;
                        self.stats.last_backend = Some(RuntimeMathBackend::Wgpu);
                        self.record_gpu_transfer(
                            lhs.values().len() + rhs.values().len(),
                            lhs.rows() * rhs.cols(),
                            readback,
                        );
                        return Ok(value);
                    }
                    Err(error) => {
                        self.stats.fallback_calls += 1;
                        if self.config.backend == RuntimeMathBackend::Wgpu {
                            return Err(error);
                        }
                    }
                }
            }
            _ => {
                if self.config.backend == RuntimeMathBackend::Wgpu {
                    return Err(wgpu_unavailable_error());
                }
                self.stats.fallback_calls += 1;
            }
        }
        self.matmul_ndarray(lhs, rhs)
    }

    fn matrix_add_wgpu(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let result = self
                    .wgpu_context()
                    .and_then(|context| context.matrix_add_f32(lhs, rhs));
                match result {
                    Ok((value, readback)) => {
                        self.stats.wgpu_calls += 1;
                        self.stats.last_backend = Some(RuntimeMathBackend::Wgpu);
                        self.record_gpu_transfer(
                            lhs.values().len() + rhs.values().len(),
                            lhs.values().len(),
                            readback,
                        );
                        return Ok(value);
                    }
                    Err(error) => {
                        self.stats.fallback_calls += 1;
                        if self.config.backend == RuntimeMathBackend::Wgpu {
                            return Err(error);
                        }
                    }
                }
            }
            _ => {
                if self.config.backend == RuntimeMathBackend::Wgpu {
                    return Err(wgpu_unavailable_error());
                }
                self.stats.fallback_calls += 1;
            }
        }
        self.matrix_add_ndarray(lhs, rhs)
    }

    fn tensor_add_wgpu(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeMathAcceleratorError> {
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let result = self
                    .wgpu_context()
                    .and_then(|context| context.tensor_add_f32(lhs, rhs));
                match result {
                    Ok((value, readback)) => {
                        self.stats.wgpu_calls += 1;
                        self.stats.last_backend = Some(RuntimeMathBackend::Wgpu);
                        self.record_gpu_transfer(
                            lhs.values().len() + rhs.values().len(),
                            lhs.values().len(),
                            readback,
                        );
                        return Ok(value);
                    }
                    Err(error) => {
                        self.stats.fallback_calls += 1;
                        if self.config.backend == RuntimeMathBackend::Wgpu {
                            return Err(error);
                        }
                    }
                }
            }
            _ => {
                if self.config.backend == RuntimeMathBackend::Wgpu {
                    return Err(wgpu_unavailable_error());
                }
                self.stats.fallback_calls += 1;
            }
        }
        self.tensor_add_ndarray(lhs, rhs)
    }

    fn matmul_bias_add_wgpu(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let result = self
                    .wgpu_context()
                    .and_then(|context| context.matmul_bias_add_f32(lhs, rhs, bias));
                match result {
                    Ok((value, readback)) => {
                        self.stats.wgpu_calls += 1;
                        self.stats.last_backend = Some(RuntimeMathBackend::Wgpu);
                        self.record_gpu_transfer_with_creations(
                            lhs.values().len() + rhs.values().len() + bias.values().len(),
                            lhs.rows() * rhs.cols(),
                            readback,
                            7,
                        );
                        return Ok(value);
                    }
                    Err(error) => {
                        self.stats.fallback_calls += 1;
                        if self.config.backend == RuntimeMathBackend::Wgpu {
                            return Err(error);
                        }
                    }
                }
            }
            _ => {
                if self.config.backend == RuntimeMathBackend::Wgpu {
                    return Err(wgpu_unavailable_error());
                }
                self.stats.fallback_calls += 1;
            }
        }
        let out = self.matmul_ndarray(lhs, rhs)?;
        add_bias_to_matrix(out, bias)
    }

    fn record_matrix_inputs<T>(&mut self, lhs: &DenseMatrix<T>, rhs: &DenseMatrix<T>) {
        self.stats.bytes_borrowed +=
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<T>();
    }

    fn record_matmul_bias_inputs(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) {
        self.stats.bytes_borrowed +=
            (lhs.values().len() + rhs.values().len() + bias.values().len())
                * std::mem::size_of::<f32>();
    }

    fn record_tensor_inputs<T>(&mut self, lhs: &DenseTensor<T>, rhs: &DenseTensor<T>) {
        self.stats.bytes_borrowed +=
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<T>();
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_gpu_transfer(
        &mut self,
        uploaded_elements: usize,
        downloaded_elements: usize,
        readback: wgpu_backend::GpuReadbackUsage,
    ) {
        self.record_gpu_transfer_with_creations(
            uploaded_elements,
            downloaded_elements,
            readback,
            4,
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_gpu_transfer_with_creations(
        &mut self,
        uploaded_elements: usize,
        downloaded_elements: usize,
        readback: wgpu_backend::GpuReadbackUsage,
        buffer_creations: usize,
    ) {
        let uploaded = uploaded_elements * std::mem::size_of::<f32>();
        let downloaded = downloaded_elements * std::mem::size_of::<f32>();
        self.stats.bytes_uploaded += uploaded;
        self.stats.bytes_downloaded += downloaded;
        self.stats.bytes_copied += uploaded + downloaded;
        self.stats.gpu_buffer_creations += buffer_creations;
        self.record_gpu_readback(readback);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_prepared_gpu_dispatch(
        &mut self,
        downloaded_elements: usize,
        readback: wgpu_backend::GpuReadbackUsage,
    ) {
        self.record_prepared_gpu_dispatch_with_reuse(downloaded_elements, readback, 4);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_prepared_gpu_dispatch_with_reuse(
        &mut self,
        downloaded_elements: usize,
        readback: wgpu_backend::GpuReadbackUsage,
        reused_buffers: usize,
    ) {
        let downloaded = downloaded_elements * std::mem::size_of::<f32>();
        self.stats.wgpu_calls += 1;
        self.stats.last_backend = Some(RuntimeMathBackend::Wgpu);
        self.stats.bytes_downloaded += downloaded;
        self.stats.bytes_copied += downloaded;
        self.stats.gpu_buffer_reuse_hits += reused_buffers;
        self.stats.gpu_reused_dispatches += 1;
        self.record_gpu_readback(readback);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_prepared_gpu_submit_with_reuse(&mut self, reused_buffers: usize) {
        self.stats.wgpu_calls += 1;
        self.stats.last_backend = Some(RuntimeMathBackend::Wgpu);
        self.stats.gpu_buffer_reuse_hits += reused_buffers;
        self.stats.gpu_reused_dispatches += 1;
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_prepared_gpu_readback(
        &mut self,
        downloaded_elements: usize,
        readback: wgpu_backend::GpuReadbackUsage,
    ) {
        let downloaded = downloaded_elements * std::mem::size_of::<f32>();
        self.stats.bytes_downloaded += downloaded;
        self.stats.bytes_copied += downloaded;
        self.record_gpu_readback(readback);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_prepared_gpu_upload(&mut self, uploaded_elements: usize) {
        self.record_prepared_gpu_upload_with_reuse(uploaded_elements, 3);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_prepared_gpu_upload_with_reuse(
        &mut self,
        uploaded_elements: usize,
        reused_buffers: usize,
    ) {
        let uploaded = uploaded_elements * std::mem::size_of::<f32>();
        self.stats.bytes_uploaded += uploaded;
        self.stats.bytes_copied += uploaded;
        self.stats.gpu_buffer_reuse_hits += reused_buffers;
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn record_gpu_readback(&mut self, readback: wgpu_backend::GpuReadbackUsage) {
        self.stats.gpu_staging_buffer_creations += usize::from(readback.created);
        self.stats.gpu_staging_buffer_reuse_hits += usize::from(readback.reused);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    fn wgpu_context(
        &mut self,
    ) -> Result<&mut wgpu_backend::WgpuMathContext, RuntimeMathAcceleratorError> {
        if self.wgpu.is_none() {
            self.wgpu = Some(wgpu_backend::WgpuMathContext::new()?);
        }
        Ok(self.wgpu.as_mut().expect("wgpu context was initialized"))
    }
}

#[cfg(feature = "math-glam")]
fn row_major_4x4_to_cols<T: Copy>(values: &[T]) -> [T; 16] {
    [
        values[0], values[4], values[8], values[12], values[1], values[5], values[9], values[13],
        values[2], values[6], values[10], values[14], values[3], values[7], values[11], values[15],
    ]
}

#[cfg(feature = "math-glam")]
fn cols_4x4_to_row_major<T: Copy>(values: [T; 16]) -> Vec<T> {
    vec![
        values[0], values[4], values[8], values[12], values[1], values[5], values[9], values[13],
        values[2], values[6], values[10], values[14], values[3], values[7], values[11], values[15],
    ]
}

fn validate_matmul_bias(
    lhs: &DenseMatrixF32,
    rhs: &DenseMatrixF32,
    bias: &DenseTensorF32,
) -> Result<(), RuntimeMathAcceleratorError> {
    if lhs.cols() != rhs.rows() {
        return Err(RuntimeMathError::MatrixShapeMismatch {
            lhs: lhs.shape(),
            rhs: rhs.shape(),
            op: "matmul_bias_add",
        }
        .into());
    }
    match bias.shape().dims() {
        [cols] if *cols == rhs.cols() => Ok(()),
        dims => Err(RuntimeMathAcceleratorError::Backend(format!(
            "matmul_bias_add expected bias shape [{}], found {:?}",
            rhs.cols(),
            dims
        ))),
    }
}

fn add_bias_to_matrix(
    mut matrix: DenseMatrixF32,
    bias: &DenseTensorF32,
) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
    if bias.shape().dims() != [matrix.cols()] {
        return Err(RuntimeMathAcceleratorError::Backend(format!(
            "matmul_bias_add expected bias shape [{}], found {:?}",
            matrix.cols(),
            bias.shape().dims()
        )));
    }
    for row in matrix.values_mut().chunks_exact_mut(bias.values().len()) {
        for (value, bias) in row.iter_mut().zip(bias.values().iter().copied()) {
            *value += bias;
        }
    }
    Ok(matrix)
}

fn matmul_work_items<T>(lhs: &DenseMatrix<T>, rhs: &DenseMatrix<T>) -> usize {
    lhs.rows()
        .saturating_mul(lhs.cols())
        .saturating_mul(rhs.cols())
}

const fn wgpu_backend_enabled() -> bool {
    cfg_select! {
        all(feature = "math-wgpu", not(target_arch = "wasm32")) => { true }
        _ => { false }
    }
}

#[cfg(any(not(feature = "math-wgpu"), target_arch = "wasm32"))]
const fn wgpu_unavailable_reason() -> &'static str {
    cfg_select! {
        all(feature = "math-wgpu", target_arch = "wasm32") => {
            "browser WebGPU math requires the async browser_webgpu adapter"
        }
        _ => {
            "math-wgpu feature is disabled"
        }
    }
}

#[cfg(any(not(feature = "math-wgpu"), target_arch = "wasm32"))]
fn wgpu_unavailable_error() -> RuntimeMathAcceleratorError {
    RuntimeMathAcceleratorError::Backend(wgpu_unavailable_reason().to_owned())
}

#[derive(Debug, Error)]
pub enum RuntimeMathAcceleratorError {
    #[error(transparent)]
    Math(#[from] RuntimeMathError),
    #[error("runtime math accelerator backend error: {0}")]
    Backend(String),
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
mod wgpu_backend {
    use super::{DenseMatrixF32, DenseTensorF32, RuntimeMathAcceleratorError, RuntimeMathError};
    use bytemuck::{Pod, Zeroable};
    use std::sync::mpsc;
    use wgpu::util::DeviceExt;

    pub struct WgpuMathContext {
        device: wgpu::Device,
        queue: wgpu::Queue,
        matmul_pipeline: wgpu::ComputePipeline,
        add_pipeline: wgpu::ComputePipeline,
        bias_add_pipeline: wgpu::ComputePipeline,
        readback: Option<ReusableReadbackBuffer>,
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct GpuReadbackUsage {
        pub created: bool,
        pub reused: bool,
    }

    struct ReusableReadbackBuffer {
        buffer: wgpu::Buffer,
        byte_len: usize,
    }

    pub struct PreparedAddBuffers {
        len: usize,
        lhs: wgpu::Buffer,
        rhs: wgpu::Buffer,
        out: wgpu::Buffer,
        params: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
    }

    impl PreparedAddBuffers {
        pub const fn len(&self) -> usize {
            self.len
        }
    }

    pub struct PreparedMatmulBuffers {
        rows: usize,
        shared: usize,
        cols: usize,
        lhs: wgpu::Buffer,
        rhs: wgpu::Buffer,
        out: wgpu::Buffer,
        params: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
    }

    impl PreparedMatmulBuffers {
        pub const fn len(&self) -> usize {
            self.rows * self.cols
        }
    }

    pub struct PreparedMatmulBiasAddBuffers {
        matmul: PreparedMatmulBuffers,
        bias: wgpu::Buffer,
        out: wgpu::Buffer,
        params: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
    }

    impl PreparedMatmulBiasAddBuffers {
        pub const fn len(&self) -> usize {
            self.matmul.rows * self.matmul.cols
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct MatmulParams {
        rows: u32,
        shared: u32,
        cols: u32,
        _pad: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct AddParams {
        len: u32,
        x_threads: u32,
        _pad1: u32,
        _pad2: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct BiasAddParams {
        rows: u32,
        cols: u32,
        _pad1: u32,
        _pad2: u32,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct MatmulShape {
        pub(super) rows: usize,
        pub(super) shared: usize,
        pub(super) cols: usize,
    }

    impl WgpuMathContext {
        pub fn new() -> Result<Self, RuntimeMathAcceleratorError> {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                }))
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("arcweft-runtime-math"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                }))
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
            let matmul_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("arcweft-matmul-f32"),
                source: wgpu::ShaderSource::Wgsl(MATMUL_SHADER.into()),
            });
            let add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("arcweft-add-f32"),
                source: wgpu::ShaderSource::Wgsl(ADD_SHADER.into()),
            });
            let bias_add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("arcweft-bias-add-f32"),
                source: wgpu::ShaderSource::Wgsl(BIAS_ADD_SHADER.into()),
            });
            let matmul_pipeline =
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("arcweft-matmul-f32"),
                    layout: None,
                    module: &matmul_shader,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });
            let add_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("arcweft-add-f32"),
                layout: None,
                module: &add_shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            let bias_add_pipeline =
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("arcweft-bias-add-f32"),
                    layout: None,
                    module: &bias_add_shader,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });
            Ok(Self {
                device,
                queue,
                matmul_pipeline,
                add_pipeline,
                bias_add_pipeline,
                readback: None,
            })
        }

        pub fn matmul_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<(DenseMatrixF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
            if lhs.cols() != rhs.rows() {
                let out = lhs.matmul_scalar(rhs)?;
                return Ok((out, GpuReadbackUsage::default()));
            }
            let out_len = lhs.rows() * rhs.cols();
            let mut out = vec![0.0; out_len];
            let readback = self.dispatch_matmul(
                lhs.values(),
                rhs.values(),
                &mut out,
                lhs.rows(),
                lhs.cols(),
                rhs.cols(),
            )?;
            Ok((DenseMatrixF32::new(lhs.rows(), rhs.cols(), out)?, readback))
        }

        pub fn matrix_add_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<(DenseMatrixF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
            if lhs.shape() != rhs.shape() {
                let out = lhs.add_scalar(rhs)?;
                return Ok((out, GpuReadbackUsage::default()));
            }
            let mut out = vec![0.0; lhs.values().len()];
            let readback = self.dispatch_add(lhs.values(), rhs.values(), &mut out)?;
            Ok((DenseMatrixF32::new(lhs.rows(), lhs.cols(), out)?, readback))
        }

        pub fn tensor_add_f32(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
        ) -> Result<(DenseTensorF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
            if lhs.shape() != rhs.shape() {
                let out = lhs.add_scalar(rhs)?;
                return Ok((out, GpuReadbackUsage::default()));
            }
            let mut out = vec![0.0; lhs.values().len()];
            let readback = self.dispatch_add(lhs.values(), rhs.values(), &mut out)?;
            Ok((
                DenseTensorF32::new(lhs.shape().dims().to_vec(), out)?,
                readback,
            ))
        }

        pub fn matmul_bias_add_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
            bias: &DenseTensorF32,
        ) -> Result<(DenseMatrixF32, GpuReadbackUsage), RuntimeMathAcceleratorError> {
            if lhs.cols() != rhs.rows() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "matmul_bias_add",
                }
                .into());
            }
            if bias.shape().dims() != [rhs.cols()] {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "matmul_bias_add expected bias shape [{}], found {:?}",
                    rhs.cols(),
                    bias.shape().dims()
                )));
            }
            let prepared = self.prepare_matmul_bias_add(
                lhs.values(),
                rhs.values(),
                bias.values(),
                lhs.rows(),
                lhs.cols(),
                rhs.cols(),
            )?;
            let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
            let readback = self.dispatch_prepared_matmul_bias_add(&prepared, &mut out)?;
            Ok((DenseMatrixF32::new(lhs.rows(), rhs.cols(), out)?, readback))
        }

        pub fn prepare_add(
            &self,
            lhs: &[f32],
            rhs: &[f32],
        ) -> Result<PreparedAddBuffers, RuntimeMathAcceleratorError> {
            if lhs.len() != rhs.len() {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared add expected matching input lengths, got {} and {}",
                    lhs.len(),
                    rhs.len()
                )));
            }
            let len = lhs.len();
            let byte_len = std::mem::size_of_val(lhs).max(std::mem::size_of::<f32>());
            let out = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-math-prepared-out"),
                size: byte_len as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let params = AddParams {
                len: checked_u32(len)?,
                x_threads: checked_u32(add_groups(len).0 * 256)?,
                _pad1: 0,
                _pad2: 0,
            };
            let lhs = storage_buffer(
                &self.device,
                bytemuck::cast_slice(lhs),
                "arcweft-math-prepared-lhs",
            );
            let rhs = storage_buffer(
                &self.device,
                bytemuck::cast_slice(rhs),
                "arcweft-math-prepared-rhs",
            );
            let params = storage_buffer(
                &self.device,
                bytemuck::bytes_of(&params),
                "arcweft-math-prepared-params",
            );
            let layout = self.add_pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-math-prepared-bind-group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: lhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: rhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: out.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params.as_entire_binding(),
                    },
                ],
            });
            Ok(PreparedAddBuffers {
                len,
                lhs,
                rhs,
                out,
                params,
                bind_group,
            })
        }

        pub fn prepare_add_capacity(
            &self,
            capacity_len: usize,
        ) -> Result<PreparedAddBuffers, RuntimeMathAcceleratorError> {
            let _ = checked_u32(capacity_len)?;
            let byte_len = capacity_len
                .saturating_mul(std::mem::size_of::<f32>())
                .max(std::mem::size_of::<f32>());
            let out = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-math-prepared-out"),
                size: byte_len as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let lhs = storage_buffer_capacity(&self.device, byte_len, "arcweft-math-prepared-lhs");
            let rhs = storage_buffer_capacity(&self.device, byte_len, "arcweft-math-prepared-rhs");
            let params = storage_buffer_capacity(
                &self.device,
                std::mem::size_of::<AddParams>(),
                "arcweft-math-prepared-params",
            );
            let layout = self.add_pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-math-prepared-bind-group"),
                layout: &layout,
                entries: &bind_group_entries(&lhs, &rhs, &out, &params),
            });
            Ok(PreparedAddBuffers {
                len: capacity_len,
                lhs,
                rhs,
                out,
                params,
                bind_group,
            })
        }

        pub fn update_prepared_add(
            &self,
            prepared: &PreparedAddBuffers,
            lhs: &[f32],
            rhs: &[f32],
        ) -> Result<(), RuntimeMathAcceleratorError> {
            if lhs.len() != rhs.len() || lhs.len() > prepared.len {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared add capacity is {} value(s), got {} and {}",
                    prepared.len,
                    lhs.len(),
                    rhs.len()
                )));
            }
            let params = AddParams {
                len: checked_u32(lhs.len())?,
                x_threads: checked_u32(add_groups(lhs.len()).0 * 256)?,
                _pad1: 0,
                _pad2: 0,
            };
            self.queue
                .write_buffer(&prepared.lhs, 0, bytemuck::cast_slice(lhs));
            self.queue
                .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
            self.queue
                .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
            Ok(())
        }

        pub fn prepare_matmul(
            &self,
            lhs: &[f32],
            rhs: &[f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<PreparedMatmulBuffers, RuntimeMathAcceleratorError> {
            if lhs.len() != rows * shared || rhs.len() != shared * cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul expected lhs {}x{} and rhs {}x{}, got {} and {} value(s)",
                    rows,
                    shared,
                    shared,
                    cols,
                    lhs.len(),
                    rhs.len()
                )));
            }
            let byte_len = rows
                .saturating_mul(cols)
                .saturating_mul(std::mem::size_of::<f32>())
                .max(std::mem::size_of::<f32>());
            let out = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-math-prepared-matmul-out"),
                size: byte_len as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let params = MatmulParams {
                rows: checked_u32(rows)?,
                shared: checked_u32(shared)?,
                cols: checked_u32(cols)?,
                _pad: 0,
            };
            let lhs = storage_buffer(
                &self.device,
                bytemuck::cast_slice(lhs),
                "arcweft-math-prepared-matmul-lhs",
            );
            let rhs = storage_buffer(
                &self.device,
                bytemuck::cast_slice(rhs),
                "arcweft-math-prepared-matmul-rhs",
            );
            let params = storage_buffer(
                &self.device,
                bytemuck::bytes_of(&params),
                "arcweft-math-prepared-matmul-params",
            );
            let layout = self.matmul_pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-math-prepared-matmul-bind-group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: lhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: rhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: out.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params.as_entire_binding(),
                    },
                ],
            });
            Ok(PreparedMatmulBuffers {
                rows,
                shared,
                cols,
                lhs,
                rhs,
                out,
                params,
                bind_group,
            })
        }

        pub fn prepare_matmul_capacity(
            &self,
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<PreparedMatmulBuffers, RuntimeMathAcceleratorError> {
            let _ = checked_u32(rows)?;
            let _ = checked_u32(shared)?;
            let _ = checked_u32(cols)?;
            let lhs_len = rows.saturating_mul(shared);
            let rhs_len = shared.saturating_mul(cols);
            let out_len = rows.saturating_mul(cols);
            let lhs = storage_buffer_capacity(
                &self.device,
                lhs_len
                    .saturating_mul(std::mem::size_of::<f32>())
                    .max(std::mem::size_of::<f32>()),
                "arcweft-math-prepared-matmul-lhs",
            );
            let rhs = storage_buffer_capacity(
                &self.device,
                rhs_len
                    .saturating_mul(std::mem::size_of::<f32>())
                    .max(std::mem::size_of::<f32>()),
                "arcweft-math-prepared-matmul-rhs",
            );
            let out = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-math-prepared-matmul-out"),
                size: out_len
                    .saturating_mul(std::mem::size_of::<f32>())
                    .max(std::mem::size_of::<f32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let params = storage_buffer_capacity(
                &self.device,
                std::mem::size_of::<MatmulParams>(),
                "arcweft-math-prepared-matmul-params",
            );
            let layout = self.matmul_pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-math-prepared-matmul-bind-group"),
                layout: &layout,
                entries: &bind_group_entries(&lhs, &rhs, &out, &params),
            });
            Ok(PreparedMatmulBuffers {
                rows,
                shared,
                cols,
                lhs,
                rhs,
                out,
                params,
                bind_group,
            })
        }

        pub fn update_prepared_matmul(
            &self,
            prepared: &PreparedMatmulBuffers,
            lhs: &[f32],
            rhs: &[f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<(), RuntimeMathAcceleratorError> {
            if rows > prepared.rows || shared > prepared.shared || cols > prepared.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul capacity is {}x{} and {}x{}, got {}x{} and {}x{}",
                    prepared.rows,
                    prepared.shared,
                    prepared.shared,
                    prepared.cols,
                    rows,
                    shared,
                    shared,
                    cols
                )));
            }
            if lhs.len() != rows * shared || rhs.len() != shared * cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul expected lhs {}x{} and rhs {}x{}, got {} and {} value(s)",
                    rows,
                    shared,
                    shared,
                    cols,
                    lhs.len(),
                    rhs.len()
                )));
            }
            let params = MatmulParams {
                rows: checked_u32(rows)?,
                shared: checked_u32(shared)?,
                cols: checked_u32(cols)?,
                _pad: 0,
            };
            self.queue
                .write_buffer(&prepared.lhs, 0, bytemuck::cast_slice(lhs));
            self.queue
                .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
            self.queue
                .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
            Ok(())
        }

        pub fn prepare_matmul_bias_add(
            &self,
            lhs: &[f32],
            rhs: &[f32],
            bias: &[f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<PreparedMatmulBiasAddBuffers, RuntimeMathAcceleratorError> {
            if bias.len() != cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul-bias-add expected bias length {cols}, got {}",
                    bias.len()
                )));
            }
            let matmul = self.prepare_matmul(lhs, rhs, rows, shared, cols)?;
            let bias = storage_buffer(
                &self.device,
                bytemuck::cast_slice(bias),
                "arcweft-math-prepared-matmul-bias-add-bias",
            );
            self.prepare_matmul_bias_add_output(matmul, bias, rows, cols)
        }

        pub fn prepare_matmul_bias_add_capacity(
            &self,
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<PreparedMatmulBiasAddBuffers, RuntimeMathAcceleratorError> {
            let matmul = self.prepare_matmul_capacity(rows, shared, cols)?;
            let bias = storage_buffer_capacity(
                &self.device,
                cols.saturating_mul(std::mem::size_of::<f32>())
                    .max(std::mem::size_of::<f32>()),
                "arcweft-math-prepared-matmul-bias-add-bias",
            );
            self.prepare_matmul_bias_add_output(matmul, bias, rows, cols)
        }

        fn prepare_matmul_bias_add_output(
            &self,
            matmul: PreparedMatmulBuffers,
            bias: wgpu::Buffer,
            rows: usize,
            cols: usize,
        ) -> Result<PreparedMatmulBiasAddBuffers, RuntimeMathAcceleratorError> {
            let out_len = rows.saturating_mul(cols);
            let out = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-out"),
                size: out_len
                    .saturating_mul(std::mem::size_of::<f32>())
                    .max(std::mem::size_of::<f32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let params = BiasAddParams {
                rows: checked_u32(rows)?,
                cols: checked_u32(cols)?,
                _pad1: 0,
                _pad2: 0,
            };
            let params = storage_buffer(
                &self.device,
                bytemuck::bytes_of(&params),
                "arcweft-math-prepared-matmul-bias-add-params",
            );
            let layout = self.bias_add_pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-math-prepared-matmul-bias-add-bind-group"),
                layout: &layout,
                entries: &bind_group_entries(&matmul.out, &bias, &out, &params),
            });
            Ok(PreparedMatmulBiasAddBuffers {
                matmul,
                bias,
                out,
                params,
                bind_group,
            })
        }

        pub fn update_prepared_matmul_bias_add(
            &self,
            prepared: &PreparedMatmulBiasAddBuffers,
            lhs: &[f32],
            rhs: &[f32],
            bias: &[f32],
            shape: MatmulShape,
        ) -> Result<(), RuntimeMathAcceleratorError> {
            if bias.len() != shape.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul-bias-add expected bias length {}, got {}",
                    shape.cols,
                    bias.len()
                )));
            }
            self.update_prepared_matmul(
                &prepared.matmul,
                lhs,
                rhs,
                shape.rows,
                shape.shared,
                shape.cols,
            )?;
            let params = BiasAddParams {
                rows: checked_u32(shape.rows)?,
                cols: checked_u32(shape.cols)?,
                _pad1: 0,
                _pad2: 0,
            };
            self.queue
                .write_buffer(&prepared.bias, 0, bytemuck::cast_slice(bias));
            self.queue
                .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
            Ok(())
        }

        pub fn dispatch_prepared_add(
            &mut self,
            prepared: &PreparedAddBuffers,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if out.len() > prepared.len {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared add capacity is {} value(s), got {}",
                    prepared.len,
                    out.len()
                )));
            }
            if out.is_empty() {
                return Ok(GpuReadbackUsage::default());
            }
            let (groups_x, groups_y) = add_groups(out.len());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-math-prepared-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-prepared-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.add_pipeline);
                pass.set_bind_group(0, &prepared.bind_group, &[]);
                pass.dispatch_workgroups(checked_u32(groups_x)?, checked_u32(groups_y)?, 1);
            }
            let readback = self.encode_readback(&mut encoder, &prepared.out, out.len());
            self.queue.submit(Some(encoder.finish()));
            self.read_staging_buffer(out)?;
            Ok(readback)
        }

        pub fn dispatch_prepared_matmul(
            &mut self,
            prepared: &PreparedMatmulBuffers,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            self.dispatch_prepared_matmul_shape(prepared, prepared.rows, prepared.cols, out)
        }

        pub fn dispatch_prepared_matmul_shape(
            &mut self,
            prepared: &PreparedMatmulBuffers,
            rows: usize,
            cols: usize,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if rows > prepared.rows || cols > prepared.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul capacity is {}x{} output, got {}x{}",
                    prepared.rows, prepared.cols, rows, cols
                )));
            }
            let expected = rows.saturating_mul(cols);
            if out.len() != expected {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul output expected {} value(s), got {}",
                    expected,
                    out.len()
                )));
            }
            if out.is_empty() {
                return Ok(GpuReadbackUsage::default());
            }
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-math-prepared-matmul-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-prepared-matmul-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.matmul_pipeline);
                pass.set_bind_group(0, &prepared.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_u32(cols.div_ceil(16))?,
                    checked_u32(rows.div_ceil(16))?,
                    1,
                );
            }
            let readback = self.encode_readback(&mut encoder, &prepared.out, out.len());
            self.queue.submit(Some(encoder.finish()));
            self.read_staging_buffer(out)?;
            Ok(readback)
        }

        pub fn submit_prepared_matmul_shape_without_readback(
            &self,
            prepared: &PreparedMatmulBuffers,
            rows: usize,
            cols: usize,
        ) -> Result<(), RuntimeMathAcceleratorError> {
            if rows > prepared.rows || cols > prepared.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul capacity is {}x{} output, got {}x{}",
                    prepared.rows, prepared.cols, rows, cols
                )));
            }
            if rows == 0 || cols == 0 {
                return Ok(());
            }
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-math-prepared-matmul-submit-only-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-prepared-matmul-submit-only-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.matmul_pipeline);
                pass.set_bind_group(0, &prepared.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_u32(cols.div_ceil(16))?,
                    checked_u32(rows.div_ceil(16))?,
                    1,
                );
            }
            self.queue.submit(Some(encoder.finish()));
            Ok(())
        }

        pub fn read_prepared_matmul_shape_output(
            &mut self,
            prepared: &PreparedMatmulBuffers,
            rows: usize,
            cols: usize,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if rows > prepared.rows || cols > prepared.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul capacity is {}x{} output, got {}x{}",
                    prepared.rows, prepared.cols, rows, cols
                )));
            }
            let expected = rows.saturating_mul(cols);
            if out.len() != expected {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul output expected {} value(s), got {}",
                    expected,
                    out.len()
                )));
            }
            self.read_output_buffer(&prepared.out, out)
        }

        pub fn dispatch_prepared_matmul_bias_add(
            &mut self,
            prepared: &PreparedMatmulBiasAddBuffers,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            self.dispatch_prepared_matmul_bias_add_shape(
                prepared,
                prepared.matmul.rows,
                prepared.matmul.cols,
                out,
            )
        }

        pub fn dispatch_prepared_matmul_bias_add_shape(
            &mut self,
            prepared: &PreparedMatmulBiasAddBuffers,
            rows: usize,
            cols: usize,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if rows > prepared.matmul.rows || cols > prepared.matmul.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul-bias-add capacity is {}x{} output, got {}x{}",
                    prepared.matmul.rows, prepared.matmul.cols, rows, cols
                )));
            }
            let expected = rows.saturating_mul(cols);
            if out.len() != expected {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul-bias-add output expected {} value(s), got {}",
                    expected,
                    out.len()
                )));
            }
            if out.is_empty() {
                return Ok(GpuReadbackUsage::default());
            }
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-math-prepared-matmul-bias-add-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-prepared-matmul-bias-add-matmul-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.matmul_pipeline);
                pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_u32(cols.div_ceil(16))?,
                    checked_u32(rows.div_ceil(16))?,
                    1,
                );
            }
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-prepared-matmul-bias-add-bias-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.bias_add_pipeline);
                pass.set_bind_group(0, &prepared.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_u32(cols.div_ceil(16))?,
                    checked_u32(rows.div_ceil(16))?,
                    1,
                );
            }
            let readback = self.encode_readback(&mut encoder, &prepared.out, out.len());
            self.queue.submit(Some(encoder.finish()));
            self.read_staging_buffer(out)?;
            Ok(readback)
        }

        pub fn submit_prepared_matmul_bias_add_shape_without_readback(
            &self,
            prepared: &PreparedMatmulBiasAddBuffers,
            rows: usize,
            cols: usize,
        ) -> Result<(), RuntimeMathAcceleratorError> {
            if rows > prepared.matmul.rows || cols > prepared.matmul.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul-bias-add capacity is {}x{} output, got {}x{}",
                    prepared.matmul.rows, prepared.matmul.cols, rows, cols
                )));
            }
            if rows == 0 || cols == 0 {
                return Ok(());
            }
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-math-prepared-matmul-bias-add-submit-only-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-prepared-matmul-bias-add-submit-only-matmul-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.matmul_pipeline);
                pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_u32(cols.div_ceil(16))?,
                    checked_u32(rows.div_ceil(16))?,
                    1,
                );
            }
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-prepared-matmul-bias-add-submit-only-bias-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.bias_add_pipeline);
                pass.set_bind_group(0, &prepared.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_u32(cols.div_ceil(16))?,
                    checked_u32(rows.div_ceil(16))?,
                    1,
                );
            }
            self.queue.submit(Some(encoder.finish()));
            Ok(())
        }

        pub fn read_prepared_matmul_bias_add_shape_output(
            &mut self,
            prepared: &PreparedMatmulBiasAddBuffers,
            rows: usize,
            cols: usize,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if rows > prepared.matmul.rows || cols > prepared.matmul.cols {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul-bias-add capacity is {}x{} output, got {}x{}",
                    prepared.matmul.rows, prepared.matmul.cols, rows, cols
                )));
            }
            let expected = rows.saturating_mul(cols);
            if out.len() != expected {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul-bias-add output expected {} value(s), got {}",
                    expected,
                    out.len()
                )));
            }
            self.read_output_buffer(&prepared.out, out)
        }

        fn dispatch_matmul(
            &mut self,
            lhs: &[f32],
            rhs: &[f32],
            out: &mut [f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            let params = MatmulParams {
                rows: checked_u32(rows)?,
                shared: checked_u32(shared)?,
                cols: checked_u32(cols)?,
                _pad: 0,
            };
            let pipeline = self.matmul_pipeline.clone();
            self.dispatch(
                &pipeline,
                &[bytemuck::cast_slice(lhs), bytemuck::cast_slice(rhs)],
                bytemuck::bytes_of(&params),
                out,
                checked_u32(cols.div_ceil(16))?,
                checked_u32(rows.div_ceil(16))?,
            )
        }

        fn dispatch_add(
            &mut self,
            lhs: &[f32],
            rhs: &[f32],
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            let (groups_x, groups_y) = add_groups(out.len());
            let params = AddParams {
                len: checked_u32(out.len())?,
                x_threads: checked_u32(groups_x * 256)?,
                _pad1: 0,
                _pad2: 0,
            };
            let pipeline = self.add_pipeline.clone();
            self.dispatch(
                &pipeline,
                &[bytemuck::cast_slice(lhs), bytemuck::cast_slice(rhs)],
                bytemuck::bytes_of(&params),
                out,
                checked_u32(groups_x)?,
                checked_u32(groups_y)?,
            )
        }

        fn dispatch(
            &mut self,
            pipeline: &wgpu::ComputePipeline,
            input_bytes: &[&[u8]],
            params_bytes: &[u8],
            out: &mut [f32],
            workgroups_x: u32,
            workgroups_y: u32,
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if out.is_empty() {
                return Ok(GpuReadbackUsage::default());
            }
            let lhs = storage_buffer(&self.device, input_bytes[0], "arcweft-math-lhs");
            let rhs = storage_buffer(&self.device, input_bytes[1], "arcweft-math-rhs");
            let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-math-out"),
                size: std::mem::size_of_val(out) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let params = storage_buffer(&self.device, params_bytes, "arcweft-math-params");
            let layout = pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-math-bind-group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: lhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: rhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: out_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params.as_entire_binding(),
                    },
                ],
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-math-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-math-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
            }
            let readback = self.encode_readback(&mut encoder, &out_buffer, out.len());
            self.queue.submit(Some(encoder.finish()));
            self.read_staging_buffer(out)?;
            Ok(readback)
        }

        fn read_output_buffer(
            &mut self,
            source: &wgpu::Buffer,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if out.is_empty() {
                return Ok(GpuReadbackUsage::default());
            }
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-math-prepared-output-readback-encoder"),
                });
            let readback = self.encode_readback(&mut encoder, source, out.len());
            self.queue.submit(Some(encoder.finish()));
            self.read_staging_buffer(out)?;
            Ok(readback)
        }

        fn encode_readback(
            &mut self,
            encoder: &mut wgpu::CommandEncoder,
            source: &wgpu::Buffer,
            elements: usize,
        ) -> GpuReadbackUsage {
            if elements == 0 {
                return GpuReadbackUsage::default();
            }
            let byte_len = elements * std::mem::size_of::<f32>();
            let usage = self.ensure_readback_buffer(byte_len);
            let readback = &self
                .readback
                .as_ref()
                .expect("readback buffer was initialized")
                .buffer;
            encoder.copy_buffer_to_buffer(source, 0, readback, 0, byte_len as u64);
            usage
        }

        fn ensure_readback_buffer(&mut self, byte_len: usize) -> GpuReadbackUsage {
            match self.readback.as_ref() {
                Some(buffer) if buffer.byte_len >= byte_len => GpuReadbackUsage {
                    created: false,
                    reused: true,
                },
                _ => {
                    self.readback = Some(ReusableReadbackBuffer {
                        buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("arcweft-math-readback"),
                            size: byte_len.max(std::mem::size_of::<f32>()) as u64,
                            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                            mapped_at_creation: false,
                        }),
                        byte_len,
                    });
                    GpuReadbackUsage {
                        created: true,
                        reused: false,
                    }
                }
            }
        }

        fn read_staging_buffer(&self, out: &mut [f32]) -> Result<(), RuntimeMathAcceleratorError> {
            if out.is_empty() {
                return Ok(());
            }
            let byte_len = std::mem::size_of_val(out);
            let buffer = &self
                .readback
                .as_ref()
                .expect("readback buffer was initialized")
                .buffer;
            let (sender, receiver) = mpsc::channel();
            buffer
                .slice(0..byte_len as u64)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
            receiver
                .recv()
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?
                .map_err(|error| RuntimeMathAcceleratorError::Backend(error.to_string()))?;
            {
                let mapped = buffer.slice(0..byte_len as u64).get_mapped_range();
                let values: &[f32] = bytemuck::cast_slice(&mapped);
                out.copy_from_slice(values);
            }
            buffer.unmap();
            Ok(())
        }
    }

    fn storage_buffer(device: &wgpu::Device, contents: &[u8], label: &'static str) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        })
    }

    fn storage_buffer_capacity(
        device: &wgpu::Device,
        byte_len: usize,
        label: &'static str,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: byte_len.max(std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_group_entries<'a>(
        lhs: &'a wgpu::Buffer,
        rhs: &'a wgpu::Buffer,
        out: &'a wgpu::Buffer,
        params: &'a wgpu::Buffer,
    ) -> [wgpu::BindGroupEntry<'a>; 4] {
        [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: lhs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: rhs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params.as_entire_binding(),
            },
        ]
    }

    fn checked_u32(value: usize) -> Result<u32, RuntimeMathAcceleratorError> {
        u32::try_from(value).map_err(|_| {
            RuntimeMathAcceleratorError::Backend(format!(
                "GPU dispatch dimension {value} exceeds u32"
            ))
        })
    }

    fn add_groups(len: usize) -> (usize, usize) {
        let groups = len.div_ceil(256).max(1);
        let groups_x = groups.min(65_535);
        let groups_y = groups.div_ceil(groups_x);
        (groups_x, groups_y.max(1))
    }

    const MATMUL_SHADER: &str = r"
struct MatrixParams {
    rows: u32,
    k_len: u32,
    cols: u32,
    pad: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> rhs: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: MatrixParams;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let col = id.x;
    let row = id.y;
    if (row >= params.rows || col >= params.cols) {
        return;
    }
    var acc = 0.0;
    for (var k = 0u; k < params.k_len; k = k + 1u) {
        acc = acc + lhs[row * params.k_len + k] * rhs[k * params.cols + col];
    }
    out[row * params.cols + col] = acc;
}
";

    const ADD_SHADER: &str = r"
struct AddParams {
    len: u32,
    x_threads: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> rhs: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: AddParams;

@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.y * params.x_threads + id.x;
    if (index >= params.len) {
        return;
    }
    out[index] = lhs[index] + rhs[index];
}
";

    const BIAS_ADD_SHADER: &str = r"
struct BiasAddParams {
    rows: u32,
    cols: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> bias: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: BiasAddParams;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let col = id.x;
    let row = id.y;
    if (row >= params.rows || col >= params.cols) {
        return;
    }
    let index = row * params.cols + col;
    out[index] = lhs[index] + bias[col];
}
";
}

/// Target-independent Browser WebGPU math auto-selection policy.
///
/// The policy is separated from the wasm adapter so benchmark and native tests
/// can validate browser Auto thresholds without linking browser-only APIs.
pub mod browser_webgpu_policy {
    /// Browser WebGPU limits captured without host paths.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct BrowserWebGpuLimits {
        pub max_storage_buffer_binding_size: u64,
        pub max_buffer_size: u64,
        pub max_compute_invocations_per_workgroup: u32,
        pub max_compute_workgroups_per_dimension: u32,
    }

    /// Capacity for prepared browser `f32` matrix multiplication.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct BrowserMatmulCapacity {
        pub rows: usize,
        pub shared: usize,
        pub cols: usize,
    }

    /// Browser-side math operation family used by the async WebGPU policy.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum BrowserWebGpuMathOp {
        MatmulF32,
        ElementwiseF32,
    }

    /// Runtime execution mode selected for a browser math operation.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum BrowserWebGpuMathMode {
        CpuWasm,
        WebGpuPreparedResidentPipelined,
        WebGpuPreparedCapacityResidentPipelined,
    }

    /// Reason recorded when browser-side Auto chooses a math execution mode.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum BrowserWebGpuMathAutoReason {
        MatmulPreparedResidentPipelined,
        MatmulPreparedCapacityResidentPipelined,
        MatmulCpuDefault,
        ElementwisePreparedResidentPipelined,
        ElementwiseCpuReadbackDominated,
        StorageLimit,
    }

    /// Selection returned by [`BrowserWebGpuMathAutoPolicy`].
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct BrowserWebGpuMathSelection {
        mode: BrowserWebGpuMathMode,
        reason: BrowserWebGpuMathAutoReason,
        capacity: Option<BrowserMatmulCapacity>,
    }

    impl BrowserWebGpuMathSelection {
        pub const fn mode(self) -> BrowserWebGpuMathMode {
            self.mode
        }

        pub const fn reason(self) -> BrowserWebGpuMathAutoReason {
            self.reason
        }

        pub const fn capacity(self) -> Option<BrowserMatmulCapacity> {
            self.capacity
        }
    }

    /// Browser WebGPU Auto policy derived from path-free benchmark evidence.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct BrowserWebGpuMathAutoPolicy {
        pub matmul_exact_min_elements: usize,
        pub matmul_capacity_min_elements: usize,
        pub elementwise_gpu_min_elements: usize,
        pub capacity_growth: BrowserWebGpuCapacityGrowth,
    }

    /// Capacity growth policy for prepared browser buffers.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum BrowserWebGpuCapacityGrowth {
        Exact,
        PowerOfTwo,
        Double,
    }

    impl Default for BrowserWebGpuMathAutoPolicy {
        fn default() -> Self {
            Self {
                matmul_exact_min_elements: 128 * 128 * 128,
                matmul_capacity_min_elements: usize::MAX,
                elementwise_gpu_min_elements: usize::MAX,
                capacity_growth: BrowserWebGpuCapacityGrowth::Double,
            }
        }
    }

    impl BrowserWebGpuMathAutoPolicy {
        pub fn select_matmul_f32(
            self,
            rows: usize,
            shared: usize,
            cols: usize,
            limits: BrowserWebGpuLimits,
        ) -> BrowserWebGpuMathSelection {
            let work = rows.saturating_mul(shared).saturating_mul(cols);
            if work >= self.matmul_capacity_min_elements {
                let capacity = BrowserMatmulCapacity {
                    rows: self.capacity_growth.grow(rows),
                    shared: self.capacity_growth.grow(shared),
                    cols: self.capacity_growth.grow(cols),
                };
                if matmul_capacity_fits(capacity, limits) {
                    return BrowserWebGpuMathSelection {
                        mode: BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined,
                        reason:
                            BrowserWebGpuMathAutoReason::MatmulPreparedCapacityResidentPipelined,
                        capacity: Some(capacity),
                    };
                }
                return BrowserWebGpuMathSelection {
                    mode: BrowserWebGpuMathMode::CpuWasm,
                    reason: BrowserWebGpuMathAutoReason::StorageLimit,
                    capacity: None,
                };
            }
            if work >= self.matmul_exact_min_elements {
                let capacity = BrowserMatmulCapacity { rows, shared, cols };
                if matmul_capacity_fits(capacity, limits) {
                    return BrowserWebGpuMathSelection {
                        mode: BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined,
                        reason: BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined,
                        capacity: Some(capacity),
                    };
                }
                return BrowserWebGpuMathSelection {
                    mode: BrowserWebGpuMathMode::CpuWasm,
                    reason: BrowserWebGpuMathAutoReason::StorageLimit,
                    capacity: None,
                };
            }
            BrowserWebGpuMathSelection {
                mode: BrowserWebGpuMathMode::CpuWasm,
                reason: BrowserWebGpuMathAutoReason::MatmulCpuDefault,
                capacity: None,
            }
        }

        pub fn select_elementwise_f32(
            self,
            len: usize,
            limits: BrowserWebGpuLimits,
        ) -> BrowserWebGpuMathSelection {
            if len >= self.elementwise_gpu_min_elements {
                if f32_storage_fits(len, limits) {
                    return BrowserWebGpuMathSelection {
                        mode: BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined,
                        reason: BrowserWebGpuMathAutoReason::ElementwisePreparedResidentPipelined,
                        capacity: None,
                    };
                }
                return BrowserWebGpuMathSelection {
                    mode: BrowserWebGpuMathMode::CpuWasm,
                    reason: BrowserWebGpuMathAutoReason::StorageLimit,
                    capacity: None,
                };
            }
            BrowserWebGpuMathSelection {
                mode: BrowserWebGpuMathMode::CpuWasm,
                reason: BrowserWebGpuMathAutoReason::ElementwiseCpuReadbackDominated,
                capacity: None,
            }
        }
    }

    impl BrowserWebGpuCapacityGrowth {
        pub fn grow(self, value: usize) -> usize {
            match self {
                Self::Exact => value.max(1),
                Self::PowerOfTwo => value.checked_next_power_of_two().unwrap_or(value).max(1),
                Self::Double => value.saturating_mul(2).max(1),
            }
        }
    }

    fn f32_storage_fits(len: usize, limits: BrowserWebGpuLimits) -> bool {
        checked_f32_bytes(len).is_some_and(|byte_len| {
            byte_len <= limits.max_buffer_size && byte_len <= limits.max_storage_buffer_binding_size
        })
    }

    fn matmul_capacity_fits(capacity: BrowserMatmulCapacity, limits: BrowserWebGpuLimits) -> bool {
        let Some(lhs_len) = capacity.rows.checked_mul(capacity.shared) else {
            return false;
        };
        let Some(rhs_len) = capacity.shared.checked_mul(capacity.cols) else {
            return false;
        };
        let Some(out_len) = capacity.rows.checked_mul(capacity.cols) else {
            return false;
        };
        f32_storage_fits(lhs_len, limits)
            && f32_storage_fits(rhs_len, limits)
            && f32_storage_fits(out_len, limits)
    }

    fn checked_f32_bytes(len: usize) -> Option<u64> {
        len.checked_mul(std::mem::size_of::<f32>())
            .and_then(|bytes| u64::try_from(bytes).ok())
    }
}

/// Browser WebGPU math adapter for `wasm32` players.
///
/// Browser WebGPU is asynchronous, so this adapter intentionally does not
/// implement the synchronous runtime math backend. Browser players can await
/// these calls at their adapter boundary and feed the resulting deterministic
/// dense values back into the VM.
#[cfg(all(feature = "math-wgpu", target_arch = "wasm32"))]
pub mod browser_webgpu {
    pub use super::browser_webgpu_policy::{
        BrowserMatmulCapacity, BrowserWebGpuCapacityGrowth, BrowserWebGpuLimits,
        BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
        BrowserWebGpuMathOp, BrowserWebGpuMathSelection,
    };
    use super::{DenseMatrixF32, DenseTensorF32, RuntimeMathError};
    use bytemuck::{Pod, Zeroable};
    use futures_channel::oneshot;
    use std::sync::{Arc, Mutex};
    use thiserror::Error;
    use wgpu::util::DeviceExt;

    /// Structured reason for browser WebGPU unavailability or dispatch failure.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum BrowserWebGpuFallbackReason {
        InsecureContext,
        NavigatorGpuMissing,
        AdapterUnavailable,
        DeviceRequestFailed,
        RequiredLimitsUnsupported,
        StorageBufferTooLarge,
        BufferSizeTooLarge,
        WorkgroupCountTooLarge,
        ValidationError,
        OutOfMemory,
        InternalError,
        DeviceLost,
        MapFailed,
        CorrectnessMismatch,
        AutoCpuThreshold,
    }

    /// Browser WebGPU feature-detection result.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct BrowserWebGpuAvailability {
        pub secure_context: bool,
        pub navigator_gpu_present: bool,
        pub cross_origin_isolated: bool,
    }

    /// Typed browser WebGPU error.
    #[derive(Debug, Error)]
    pub enum BrowserWebGpuError {
        #[error("browser WebGPU fallback {reason:?}: {detail}")]
        Fallback {
            reason: BrowserWebGpuFallbackReason,
            detail: String,
        },
        #[error(transparent)]
        Math(#[from] RuntimeMathError),
    }

    impl BrowserWebGpuError {
        pub fn fallback(reason: BrowserWebGpuFallbackReason, detail: impl Into<String>) -> Self {
            Self::Fallback {
                reason,
                detail: detail.into(),
            }
        }

        pub const fn reason(&self) -> Option<BrowserWebGpuFallbackReason> {
            match self {
                Self::Fallback { reason, .. } => Some(*reason),
                Self::Math(_) => None,
            }
        }
    }

    /// Browser-side WebGPU transfer counters.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct BrowserWebGpuMathStats {
        pub dispatches: usize,
        pub async_submissions: usize,
        pub async_readbacks: usize,
        pub max_in_flight: usize,
        pub bytes_uploaded: usize,
        pub bytes_downloaded: usize,
        pub bytes_copied: usize,
        pub buffer_creations: usize,
        pub buffer_reuse_hits: usize,
        pub bind_group_rebuilds: usize,
        pub readback_buffer_creations: usize,
        pub readback_buffer_reuse_hits: usize,
        pub map_count: usize,
        pub map_wait_ms: f64,
        pub pipeline_cache_hits: usize,
    }

    /// Async browser WebGPU context for dense `f32` math kernels.
    pub struct BrowserWebGpuMathContext {
        device: wgpu::Device,
        queue: wgpu::Queue,
        matmul_pipeline: wgpu::ComputePipeline,
        add_pipeline: wgpu::ComputePipeline,
        bias_add_pipeline: wgpu::ComputePipeline,
        readback: Option<ReusableReadbackBuffer>,
        async_readback: Option<ReusableReadbackBuffer>,
        limits: BrowserWebGpuLimits,
        device_lost: Arc<Mutex<Option<String>>>,
        uncaptured_error: Arc<Mutex<Option<String>>>,
        stats: BrowserWebGpuMathStats,
        in_flight: usize,
    }

    struct ReusableReadbackBuffer {
        buffer: wgpu::Buffer,
        byte_len: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct MatmulParams {
        rows: u32,
        shared: u32,
        cols: u32,
        _pad: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct AddParams {
        len: u32,
        x_threads: u32,
        _pad1: u32,
        _pad2: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct BiasAddParams {
        rows: u32,
        cols: u32,
        _pad1: u32,
        _pad2: u32,
    }

    /// Prepared browser buffers for repeated `f32` elementwise add.
    pub struct BrowserPreparedElementwiseF32 {
        capacity_len: usize,
        lhs: wgpu::Buffer,
        rhs: wgpu::Buffer,
        out: wgpu::Buffer,
        params: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
    }

    /// Prepared browser buffers for repeated `f32` matrix multiplication.
    pub struct BrowserPreparedMatmulF32 {
        capacity: BrowserMatmulCapacity,
        lhs: wgpu::Buffer,
        rhs: wgpu::Buffer,
        out: wgpu::Buffer,
        params: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
    }

    /// Prepared resident browser buffers for `matmul(lhs, rhs) + add_rhs`.
    ///
    /// This is a small forward-graph fragment: the matmul output remains on the
    /// GPU and is bound directly as the add lhs input. Host-visible readback is
    /// explicit and delayed until the caller crosses the value boundary.
    pub struct BrowserPreparedMatmulAddF32 {
        matmul: BrowserPreparedMatmulF32,
        add: BrowserPreparedElementwiseF32,
        add_bind_group: wgpu::BindGroup,
    }

    /// Prepared resident browser buffers for `bias_add(matmul(lhs, rhs), bias)`.
    pub struct BrowserPreparedMatmulBiasAddF32 {
        matmul: BrowserPreparedMatmulF32,
        bias: wgpu::Buffer,
        out: wgpu::Buffer,
        params: wgpu::Buffer,
        bias_bind_group: wgpu::BindGroup,
        output_capacity_len: usize,
    }

    /// Shape for a resident browser `matmul(lhs, rhs) + add_rhs` graph fragment.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct BrowserMatmulAddF32Shape {
        pub rows: usize,
        pub shared: usize,
        pub cols: usize,
    }

    /// Browser resident `f32` graph fragment preparation request.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum BrowserResidentF32GraphSpec {
        MatmulAdd { capacity: BrowserMatmulCapacity },
        MatmulBiasAdd { capacity: BrowserMatmulCapacity },
    }

    /// Borrowed inputs for a resident browser `matmul(lhs, rhs) + add_rhs` graph fragment.
    #[derive(Clone, Copy, Debug)]
    pub struct BrowserResidentMatmulAddF32Inputs<'a> {
        pub lhs: &'a [f32],
        pub rhs: &'a [f32],
        pub add_rhs: &'a [f32],
        pub shape: BrowserMatmulAddF32Shape,
    }

    /// Borrowed inputs for a resident browser `bias_add(matmul(lhs, rhs), bias)` fragment.
    #[derive(Clone, Copy, Debug)]
    pub struct BrowserResidentMatmulBiasAddF32Inputs<'a> {
        pub lhs: &'a [f32],
        pub rhs: &'a [f32],
        pub bias: &'a [f32],
        pub shape: BrowserMatmulAddF32Shape,
    }

    /// Borrowed inputs for a prepared resident browser `f32` graph fragment.
    #[derive(Clone, Copy, Debug)]
    pub enum BrowserResidentF32GraphInputs<'a> {
        MatmulAdd(BrowserResidentMatmulAddF32Inputs<'a>),
        MatmulBiasAdd(BrowserResidentMatmulBiasAddF32Inputs<'a>),
    }

    /// Prepared resident browser `f32` graph fragment.
    pub enum BrowserPreparedResidentF32Graph {
        MatmulAdd(BrowserPreparedMatmulAddF32),
        MatmulBiasAdd(BrowserPreparedMatmulBiasAddF32),
    }

    /// Submitted browser GPU work whose readback can be awaited later.
    pub struct BrowserSubmittedF32 {
        readback: Option<ReusableReadbackBuffer>,
        len: usize,
        submitted_at_ms: f64,
    }

    /// Submitted browser GPU work that intentionally keeps its output resident.
    ///
    /// This ticket records command submission without allocating or copying into
    /// a readback buffer. Callers must explicitly read from the prepared output
    /// buffer when they cross back to a host-visible value boundary.
    pub struct BrowserResidentSubmission {
        len: usize,
        submitted_at_ms: f64,
    }

    /// Result of a browser Auto math operation.
    #[derive(Clone, Debug)]
    pub struct BrowserWebGpuAutoMathResult<T> {
        value: T,
        selection: BrowserWebGpuMathSelection,
        stats: BrowserWebGpuMathStats,
    }

    /// Borrowed math request accepted by the browser async WebGPU adapter.
    ///
    /// Inputs are borrowed so player/host code can dispatch through this
    /// boundary without copying dense matrix or tensor buffers before the
    /// selected CPU/WebGPU path decides what transfer work is required.
    #[derive(Clone, Copy, Debug)]
    pub enum BrowserWebGpuMathRequest<'a> {
        MatmulF32 {
            lhs: &'a DenseMatrixF32,
            rhs: &'a DenseMatrixF32,
        },
        MatrixAddF32 {
            lhs: &'a DenseMatrixF32,
            rhs: &'a DenseMatrixF32,
        },
        TensorAddF32 {
            lhs: &'a DenseTensorF32,
            rhs: &'a DenseTensorF32,
        },
    }

    /// Typed response returned by the browser async WebGPU adapter.
    #[derive(Clone, Debug)]
    pub enum BrowserWebGpuMathResponse {
        MatrixF32(BrowserWebGpuAutoMathResult<DenseMatrixF32>),
        TensorF32(BrowserWebGpuAutoMathResult<DenseTensorF32>),
    }

    /// Result of selecting a browser math request before readback.
    ///
    /// CPU-selected work completes immediately. WebGPU-selected work returns a
    /// submitted handle so flow/batch runtimes can overlap future submissions
    /// with delayed readback.
    pub enum BrowserWebGpuMathDispatch {
        Immediate(BrowserWebGpuMathResponse),
        Submitted(BrowserWebGpuSubmittedMath),
    }

    /// Result of preparing a browser math request for resident repeated
    /// submission.
    pub enum BrowserWebGpuPreparedMathDispatch {
        Cpu(BrowserWebGpuMathSelection),
        Prepared(BrowserWebGpuPreparedMath),
    }

    /// Browser GPU resident math work with inputs already uploaded.
    pub enum BrowserWebGpuPreparedMath {
        MatmulF32 {
            prepared: BrowserPreparedMatmulF32,
            rows: usize,
            cols: usize,
            selection: BrowserWebGpuMathSelection,
        },
        MatrixAddF32 {
            prepared: BrowserPreparedElementwiseF32,
            rows: usize,
            cols: usize,
            len: usize,
            selection: BrowserWebGpuMathSelection,
        },
        TensorAddF32 {
            prepared: BrowserPreparedElementwiseF32,
            dims: Vec<usize>,
            len: usize,
            selection: BrowserWebGpuMathSelection,
        },
    }

    /// Submitted browser math work with typed output reconstruction metadata.
    pub enum BrowserWebGpuSubmittedMath {
        MatrixF32 {
            submitted: BrowserSubmittedF32,
            rows: usize,
            cols: usize,
            selection: BrowserWebGpuMathSelection,
        },
        TensorF32 {
            submitted: BrowserSubmittedF32,
            dims: Vec<usize>,
            selection: BrowserWebGpuMathSelection,
        },
    }

    /// Typed metadata returned after reading submitted browser GPU values into
    /// a caller-owned buffer.
    pub enum BrowserWebGpuSubmittedOutput {
        MatrixF32 {
            rows: usize,
            cols: usize,
            selection: BrowserWebGpuMathSelection,
        },
        TensorF32 {
            dims: Vec<usize>,
            selection: BrowserWebGpuMathSelection,
        },
    }

    /// Adapter-owned browser WebGPU Auto dispatcher.
    ///
    /// This is the async browser counterpart to the synchronous native math
    /// backend. It stays outside `arcweft-core` and keeps WebGPU device state
    /// in an adapter object that host/player code can await.
    pub struct BrowserWebGpuAutoMathAdapter {
        context: BrowserWebGpuMathContext,
        policy: BrowserWebGpuMathAutoPolicy,
        elementwise_f32: Option<BrowserPreparedElementwiseF32>,
        matmul_f32: Option<BrowserPreparedMatmulF32>,
    }

    impl BrowserPreparedElementwiseF32 {
        pub const fn capacity_len(&self) -> usize {
            self.capacity_len
        }
    }

    impl BrowserPreparedMatmulF32 {
        pub const fn capacity(&self) -> BrowserMatmulCapacity {
            self.capacity
        }
    }

    impl BrowserPreparedMatmulAddF32 {
        pub const fn capacity(&self) -> BrowserMatmulCapacity {
            self.matmul.capacity()
        }

        pub const fn output_capacity_len(&self) -> usize {
            self.add.capacity_len()
        }
    }

    impl BrowserPreparedMatmulBiasAddF32 {
        pub const fn capacity(&self) -> BrowserMatmulCapacity {
            self.matmul.capacity()
        }

        pub const fn output_capacity_len(&self) -> usize {
            self.output_capacity_len
        }
    }

    impl BrowserMatmulAddF32Shape {
        pub const fn new(rows: usize, shared: usize, cols: usize) -> Self {
            Self { rows, shared, cols }
        }

        pub fn output_len(self) -> Result<usize, BrowserWebGpuError> {
            checked_len_mul(self.rows, self.cols)
        }

        pub const fn capacity(self) -> BrowserMatmulCapacity {
            BrowserMatmulCapacity {
                rows: self.rows,
                shared: self.shared,
                cols: self.cols,
            }
        }
    }

    impl BrowserResidentF32GraphSpec {
        pub const fn matmul_add(capacity: BrowserMatmulCapacity) -> Self {
            Self::MatmulAdd { capacity }
        }

        pub const fn matmul_bias_add(capacity: BrowserMatmulCapacity) -> Self {
            Self::MatmulBiasAdd { capacity }
        }
    }

    impl<'a> BrowserResidentMatmulAddF32Inputs<'a> {
        pub const fn new(
            lhs: &'a [f32],
            rhs: &'a [f32],
            add_rhs: &'a [f32],
            shape: BrowserMatmulAddF32Shape,
        ) -> Self {
            Self {
                lhs,
                rhs,
                add_rhs,
                shape,
            }
        }
    }

    impl<'a> BrowserResidentMatmulBiasAddF32Inputs<'a> {
        pub const fn new(
            lhs: &'a [f32],
            rhs: &'a [f32],
            bias: &'a [f32],
            shape: BrowserMatmulAddF32Shape,
        ) -> Self {
            Self {
                lhs,
                rhs,
                bias,
                shape,
            }
        }
    }

    impl BrowserPreparedResidentF32Graph {
        pub const fn output_capacity_len(&self) -> usize {
            match self {
                Self::MatmulAdd(prepared) => prepared.output_capacity_len(),
                Self::MatmulBiasAdd(prepared) => prepared.output_capacity_len(),
            }
        }
    }

    impl BrowserSubmittedF32 {
        pub const fn len(&self) -> usize {
            self.len
        }

        pub const fn is_empty(&self) -> bool {
            self.len == 0
        }
    }

    impl BrowserResidentSubmission {
        pub const fn len(&self) -> usize {
            self.len
        }

        pub const fn is_empty(&self) -> bool {
            self.len == 0
        }

        pub const fn submitted_at_ms(&self) -> f64 {
            self.submitted_at_ms
        }
    }

    impl<T> BrowserWebGpuAutoMathResult<T> {
        pub const fn value(&self) -> &T {
            &self.value
        }

        pub fn into_value(self) -> T {
            self.value
        }

        pub const fn selection(&self) -> BrowserWebGpuMathSelection {
            self.selection
        }

        pub const fn stats(&self) -> BrowserWebGpuMathStats {
            self.stats
        }
    }

    impl BrowserWebGpuMathRequest<'_> {
        pub const fn op(&self) -> BrowserWebGpuMathOp {
            match self {
                Self::MatmulF32 { .. } => BrowserWebGpuMathOp::MatmulF32,
                Self::MatrixAddF32 { .. } | Self::TensorAddF32 { .. } => {
                    BrowserWebGpuMathOp::ElementwiseF32
                }
            }
        }
    }

    impl BrowserWebGpuMathResponse {
        pub const fn selection(&self) -> BrowserWebGpuMathSelection {
            match self {
                Self::MatrixF32(result) => result.selection(),
                Self::TensorF32(result) => result.selection(),
            }
        }

        pub const fn stats(&self) -> BrowserWebGpuMathStats {
            match self {
                Self::MatrixF32(result) => result.stats(),
                Self::TensorF32(result) => result.stats(),
            }
        }

        pub const fn matrix_f32(&self) -> Option<&DenseMatrixF32> {
            match self {
                Self::MatrixF32(result) => Some(result.value()),
                Self::TensorF32(_) => None,
            }
        }

        pub const fn tensor_f32(&self) -> Option<&DenseTensorF32> {
            match self {
                Self::MatrixF32(_) => None,
                Self::TensorF32(result) => Some(result.value()),
            }
        }
    }

    impl BrowserWebGpuMathDispatch {
        pub const fn selection(&self) -> BrowserWebGpuMathSelection {
            match self {
                Self::Immediate(response) => response.selection(),
                Self::Submitted(submitted) => submitted.selection(),
            }
        }

        pub const fn is_submitted(&self) -> bool {
            matches!(self, Self::Submitted(_))
        }
    }

    impl BrowserWebGpuPreparedMathDispatch {
        pub const fn selection(&self) -> BrowserWebGpuMathSelection {
            match self {
                Self::Cpu(selection) => *selection,
                Self::Prepared(prepared) => prepared.selection(),
            }
        }

        pub const fn is_prepared(&self) -> bool {
            matches!(self, Self::Prepared(_))
        }
    }

    impl BrowserWebGpuPreparedMath {
        pub const fn selection(&self) -> BrowserWebGpuMathSelection {
            match self {
                Self::MatmulF32 { selection, .. }
                | Self::MatrixAddF32 { selection, .. }
                | Self::TensorAddF32 { selection, .. } => *selection,
            }
        }

        pub fn len(&self) -> usize {
            match self {
                Self::MatmulF32 { rows, cols, .. } => rows * cols,
                Self::MatrixAddF32 { len, .. } | Self::TensorAddF32 { len, .. } => *len,
            }
        }

        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }
    }

    impl BrowserWebGpuSubmittedMath {
        pub const fn selection(&self) -> BrowserWebGpuMathSelection {
            match self {
                Self::MatrixF32 { selection, .. } | Self::TensorF32 { selection, .. } => *selection,
            }
        }

        pub const fn len(&self) -> usize {
            match self {
                Self::MatrixF32 {
                    submitted,
                    rows: _,
                    cols: _,
                    selection: _,
                }
                | Self::TensorF32 {
                    submitted,
                    dims: _,
                    selection: _,
                } => submitted.len(),
            }
        }

        pub const fn is_empty(&self) -> bool {
            self.len() == 0
        }
    }

    impl BrowserWebGpuSubmittedOutput {
        pub const fn selection(&self) -> BrowserWebGpuMathSelection {
            match self {
                Self::MatrixF32 { selection, .. } | Self::TensorF32 { selection, .. } => *selection,
            }
        }
    }

    impl BrowserWebGpuAutoMathAdapter {
        pub async fn new(policy: BrowserWebGpuMathAutoPolicy) -> Result<Self, BrowserWebGpuError> {
            let context = BrowserWebGpuMathContext::new().await?;
            Ok(Self::from_context(context, policy))
        }

        pub const fn from_context(
            context: BrowserWebGpuMathContext,
            policy: BrowserWebGpuMathAutoPolicy,
        ) -> Self {
            Self {
                context,
                policy,
                elementwise_f32: None,
                matmul_f32: None,
            }
        }

        pub const fn policy(&self) -> BrowserWebGpuMathAutoPolicy {
            self.policy
        }

        pub fn set_policy(&mut self, policy: BrowserWebGpuMathAutoPolicy) {
            self.policy = policy;
        }

        pub const fn limits(&self) -> BrowserWebGpuLimits {
            self.context.limits()
        }

        pub const fn stats(&self) -> BrowserWebGpuMathStats {
            self.context.stats()
        }

        pub fn reset_stats(&mut self) {
            self.context.reset_stats();
        }

        pub fn clear_prepared_buffers(&mut self) {
            self.elementwise_f32 = None;
            self.matmul_f32 = None;
        }

        pub const fn context(&self) -> &BrowserWebGpuMathContext {
            &self.context
        }

        pub fn context_mut(&mut self) -> &mut BrowserWebGpuMathContext {
            &mut self.context
        }

        pub async fn dispatch(
            &mut self,
            request: BrowserWebGpuMathRequest<'_>,
        ) -> Result<BrowserWebGpuMathResponse, BrowserWebGpuError> {
            let dispatch = self.submit(request)?;
            self.read_dispatch(dispatch).await
        }

        pub fn submit(
            &mut self,
            request: BrowserWebGpuMathRequest<'_>,
        ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
            match request {
                BrowserWebGpuMathRequest::MatmulF32 { lhs, rhs } => {
                    self.submit_matmul_f32(lhs, rhs)
                }
                BrowserWebGpuMathRequest::MatrixAddF32 { lhs, rhs } => {
                    self.submit_matrix_add_f32(lhs, rhs)
                }
                BrowserWebGpuMathRequest::TensorAddF32 { lhs, rhs } => {
                    self.submit_tensor_add_f32(lhs, rhs)
                }
            }
        }

        pub fn prepare_resident(
            &mut self,
            request: BrowserWebGpuMathRequest<'_>,
        ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
            match request {
                BrowserWebGpuMathRequest::MatmulF32 { lhs, rhs } => {
                    self.prepare_resident_matmul_f32(lhs, rhs)
                }
                BrowserWebGpuMathRequest::MatrixAddF32 { lhs, rhs } => {
                    self.prepare_resident_matrix_add_f32(lhs, rhs)
                }
                BrowserWebGpuMathRequest::TensorAddF32 { lhs, rhs } => {
                    self.prepare_resident_tensor_add_f32(lhs, rhs)
                }
            }
        }

        pub fn submit_prepared(
            &mut self,
            prepared: &BrowserWebGpuPreparedMath,
        ) -> Result<BrowserWebGpuSubmittedMath, BrowserWebGpuError> {
            match prepared {
                BrowserWebGpuPreparedMath::MatmulF32 {
                    prepared,
                    rows,
                    cols,
                    selection,
                } => {
                    let submitted = self
                        .context
                        .submit_resident_matmul_f32(prepared, *rows, *cols)?;
                    Ok(BrowserWebGpuSubmittedMath::MatrixF32 {
                        submitted,
                        rows: *rows,
                        cols: *cols,
                        selection: *selection,
                    })
                }
                BrowserWebGpuPreparedMath::MatrixAddF32 {
                    prepared,
                    rows,
                    cols,
                    len,
                    selection,
                } => {
                    let submitted = self
                        .context
                        .submit_resident_elementwise_f32(prepared, *len)?;
                    Ok(BrowserWebGpuSubmittedMath::MatrixF32 {
                        submitted,
                        rows: *rows,
                        cols: *cols,
                        selection: *selection,
                    })
                }
                BrowserWebGpuPreparedMath::TensorAddF32 {
                    prepared,
                    dims,
                    len,
                    selection,
                } => {
                    let submitted = self
                        .context
                        .submit_resident_elementwise_f32(prepared, *len)?;
                    Ok(BrowserWebGpuSubmittedMath::TensorF32 {
                        submitted,
                        dims: dims.clone(),
                        selection: *selection,
                    })
                }
            }
        }

        pub async fn read_dispatch(
            &mut self,
            dispatch: BrowserWebGpuMathDispatch,
        ) -> Result<BrowserWebGpuMathResponse, BrowserWebGpuError> {
            match dispatch {
                BrowserWebGpuMathDispatch::Immediate(response) => Ok(response),
                BrowserWebGpuMathDispatch::Submitted(submitted) => {
                    self.read_submitted(submitted).await
                }
            }
        }

        pub async fn read_submitted(
            &mut self,
            submitted: BrowserWebGpuSubmittedMath,
        ) -> Result<BrowserWebGpuMathResponse, BrowserWebGpuError> {
            let mut out = vec![0.0; submitted.len()];
            match self.read_submitted_values_into(submitted, &mut out).await? {
                BrowserWebGpuSubmittedOutput::MatrixF32 {
                    rows,
                    cols,
                    selection,
                } => {
                    let value = DenseMatrixF32::new(rows, cols, out)?;
                    Ok(BrowserWebGpuMathResponse::MatrixF32(
                        BrowserWebGpuAutoMathResult {
                            value,
                            selection,
                            stats: self.context.stats,
                        },
                    ))
                }
                BrowserWebGpuSubmittedOutput::TensorF32 { dims, selection } => {
                    let value = DenseTensorF32::new(dims, out)?;
                    Ok(BrowserWebGpuMathResponse::TensorF32(
                        BrowserWebGpuAutoMathResult {
                            value,
                            selection,
                            stats: self.context.stats,
                        },
                    ))
                }
            }
        }

        pub async fn read_submitted_values_into(
            &mut self,
            submitted: BrowserWebGpuSubmittedMath,
            out: &mut [f32],
        ) -> Result<BrowserWebGpuSubmittedOutput, BrowserWebGpuError> {
            match submitted {
                BrowserWebGpuSubmittedMath::MatrixF32 {
                    submitted,
                    rows,
                    cols,
                    selection,
                } => {
                    self.context.read_submitted_f32(submitted, out).await?;
                    Ok(BrowserWebGpuSubmittedOutput::MatrixF32 {
                        rows,
                        cols,
                        selection,
                    })
                }
                BrowserWebGpuSubmittedMath::TensorF32 {
                    submitted,
                    dims,
                    selection,
                } => {
                    self.context.read_submitted_f32(submitted, out).await?;
                    Ok(BrowserWebGpuSubmittedOutput::TensorF32 { dims, selection })
                }
            }
        }

        fn submit_matmul_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
            if lhs.cols() != rhs.rows() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "matmul",
                }
                .into());
            }
            let selection = self.policy.select_matmul_f32(
                lhs.rows(),
                lhs.cols(),
                rhs.cols(),
                self.context.limits,
            );
            match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuMathDispatch::Immediate(
                    BrowserWebGpuMathResponse::MatrixF32(BrowserWebGpuAutoMathResult {
                        value: lhs.matmul_scalar(rhs)?,
                        selection,
                        stats: self.context.stats,
                    }),
                )),
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let capacity = selection.capacity().unwrap_or(BrowserMatmulCapacity {
                        rows: lhs.rows(),
                        shared: lhs.cols(),
                        cols: rhs.cols(),
                    });
                    let prepared = self.take_matmul_f32(capacity)?;
                    let submitted = match self.context.upload_prepared_matmul_f32(
                        &prepared,
                        lhs.values(),
                        rhs.values(),
                        lhs.rows(),
                        lhs.cols(),
                        rhs.cols(),
                    ) {
                        Ok(()) => self.context.submit_resident_matmul_f32(
                            &prepared,
                            lhs.rows(),
                            rhs.cols(),
                        ),
                        Err(error) => Err(error),
                    };
                    self.matmul_f32 = Some(prepared);
                    let submitted = submitted?;
                    Ok(BrowserWebGpuMathDispatch::Submitted(
                        BrowserWebGpuSubmittedMath::MatrixF32 {
                            submitted,
                            rows: lhs.rows(),
                            cols: rhs.cols(),
                            selection,
                        },
                    ))
                }
            }
        }

        fn prepare_resident_matmul_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
            if lhs.cols() != rhs.rows() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "matmul",
                }
                .into());
            }
            let selection = self.policy.select_matmul_f32(
                lhs.rows(),
                lhs.cols(),
                rhs.cols(),
                self.context.limits,
            );
            match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => {
                    Ok(BrowserWebGpuPreparedMathDispatch::Cpu(selection))
                }
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let capacity = selection.capacity().unwrap_or(BrowserMatmulCapacity {
                        rows: lhs.rows(),
                        shared: lhs.cols(),
                        cols: rhs.cols(),
                    });
                    let prepared = self.context.prepare_matmul_f32(capacity)?;
                    self.context.upload_prepared_matmul_f32(
                        &prepared,
                        lhs.values(),
                        rhs.values(),
                        lhs.rows(),
                        lhs.cols(),
                        rhs.cols(),
                    )?;
                    Ok(BrowserWebGpuPreparedMathDispatch::Prepared(
                        BrowserWebGpuPreparedMath::MatmulF32 {
                            prepared,
                            rows: lhs.rows(),
                            cols: rhs.cols(),
                            selection,
                        },
                    ))
                }
            }
        }

        fn submit_matrix_add_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "add",
                }
                .into());
            }
            let selection = self
                .policy
                .select_elementwise_f32(lhs.values().len(), self.context.limits);
            match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuMathDispatch::Immediate(
                    BrowserWebGpuMathResponse::MatrixF32(BrowserWebGpuAutoMathResult {
                        value: lhs.add_scalar(rhs)?,
                        selection,
                        stats: self.context.stats,
                    }),
                )),
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let submitted = self.submit_elementwise_f32(lhs.values(), rhs.values())?;
                    Ok(BrowserWebGpuMathDispatch::Submitted(
                        BrowserWebGpuSubmittedMath::MatrixF32 {
                            submitted,
                            rows: lhs.rows(),
                            cols: lhs.cols(),
                            selection,
                        },
                    ))
                }
            }
        }

        fn prepare_resident_matrix_add_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "add",
                }
                .into());
            }
            let selection = self
                .policy
                .select_elementwise_f32(lhs.values().len(), self.context.limits);
            match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => {
                    Ok(BrowserWebGpuPreparedMathDispatch::Cpu(selection))
                }
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let prepared = self.context.prepare_elementwise_f32(lhs.values().len())?;
                    self.context.upload_prepared_elementwise_f32(
                        &prepared,
                        lhs.values(),
                        rhs.values(),
                    )?;
                    Ok(BrowserWebGpuPreparedMathDispatch::Prepared(
                        BrowserWebGpuPreparedMath::MatrixAddF32 {
                            prepared,
                            rows: lhs.rows(),
                            cols: lhs.cols(),
                            len: lhs.values().len(),
                            selection,
                        },
                    ))
                }
            }
        }

        fn submit_tensor_add_f32(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
        ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::TensorShapeMismatch {
                    lhs: lhs.shape().clone(),
                    rhs: rhs.shape().clone(),
                    op: "add",
                }
                .into());
            }
            let selection = self
                .policy
                .select_elementwise_f32(lhs.values().len(), self.context.limits);
            match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => Ok(BrowserWebGpuMathDispatch::Immediate(
                    BrowserWebGpuMathResponse::TensorF32(BrowserWebGpuAutoMathResult {
                        value: lhs.add_scalar(rhs)?,
                        selection,
                        stats: self.context.stats,
                    }),
                )),
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let submitted = self.submit_elementwise_f32(lhs.values(), rhs.values())?;
                    Ok(BrowserWebGpuMathDispatch::Submitted(
                        BrowserWebGpuSubmittedMath::TensorF32 {
                            submitted,
                            dims: lhs.shape().dims().to_vec(),
                            selection,
                        },
                    ))
                }
            }
        }

        fn prepare_resident_tensor_add_f32(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
        ) -> Result<BrowserWebGpuPreparedMathDispatch, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::TensorShapeMismatch {
                    lhs: lhs.shape().clone(),
                    rhs: rhs.shape().clone(),
                    op: "add",
                }
                .into());
            }
            let selection = self
                .policy
                .select_elementwise_f32(lhs.values().len(), self.context.limits);
            match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => {
                    Ok(BrowserWebGpuPreparedMathDispatch::Cpu(selection))
                }
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let prepared = self.context.prepare_elementwise_f32(lhs.values().len())?;
                    self.context.upload_prepared_elementwise_f32(
                        &prepared,
                        lhs.values(),
                        rhs.values(),
                    )?;
                    Ok(BrowserWebGpuPreparedMathDispatch::Prepared(
                        BrowserWebGpuPreparedMath::TensorAddF32 {
                            prepared,
                            dims: lhs.shape().dims().to_vec(),
                            len: lhs.values().len(),
                            selection,
                        },
                    ))
                }
            }
        }

        fn submit_elementwise_f32(
            &mut self,
            lhs: &[f32],
            rhs: &[f32],
        ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
            let prepared = self.take_elementwise_f32(lhs.len())?;
            let submitted = match self
                .context
                .upload_prepared_elementwise_f32(&prepared, lhs, rhs)
            {
                Ok(()) => self
                    .context
                    .submit_resident_elementwise_f32(&prepared, lhs.len()),
                Err(error) => Err(error),
            };
            self.elementwise_f32 = Some(prepared);
            submitted
        }

        fn take_elementwise_f32(
            &mut self,
            len: usize,
        ) -> Result<BrowserPreparedElementwiseF32, BrowserWebGpuError> {
            if let Some(prepared) = self.elementwise_f32.take()
                && prepared.capacity_len() >= len
            {
                return Ok(prepared);
            }
            self.context.prepare_elementwise_f32(len)
        }

        fn take_matmul_f32(
            &mut self,
            capacity: BrowserMatmulCapacity,
        ) -> Result<BrowserPreparedMatmulF32, BrowserWebGpuError> {
            if let Some(prepared) = self.matmul_f32.take()
                && matmul_capacity_covers(prepared.capacity(), capacity)
            {
                return Ok(prepared);
            }
            self.context.prepare_matmul_f32(capacity)
        }
    }

    impl BrowserWebGpuMathContext {
        pub fn availability() -> BrowserWebGpuAvailability {
            let global = js_sys::global();
            let secure_context = bool_property(&global, "isSecureContext");
            let cross_origin_isolated = bool_property(&global, "crossOriginIsolated");
            let navigator_gpu_present = js_sys::Reflect::get(&global, &"navigator".into())
                .ok()
                .and_then(|navigator| js_sys::Reflect::get(&navigator, &"gpu".into()).ok())
                .is_some_and(|gpu| !gpu.is_undefined() && !gpu.is_null());
            BrowserWebGpuAvailability {
                secure_context,
                navigator_gpu_present,
                cross_origin_isolated,
            }
        }

        /// Create a browser WebGPU context. This must be awaited by the browser
        /// adapter before dispatching kernels.
        pub async fn new() -> Result<Self, BrowserWebGpuError> {
            let availability = Self::availability();
            if !availability.secure_context {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::InsecureContext,
                    "browser WebGPU requires a secure context",
                ));
            }
            if !availability.navigator_gpu_present {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::NavigatorGpuMissing,
                    "navigator.gpu is unavailable",
                ));
            }
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|error| {
                    BrowserWebGpuError::fallback(
                        BrowserWebGpuFallbackReason::AdapterUnavailable,
                        error.to_string(),
                    )
                })?;
            let limits = BrowserWebGpuLimits::from(adapter.limits());
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("arcweft-browser-runtime-math"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                })
                .await
                .map_err(|error| {
                    BrowserWebGpuError::fallback(
                        BrowserWebGpuFallbackReason::DeviceRequestFailed,
                        error.to_string(),
                    )
                })?;
            let device_lost = Arc::new(Mutex::new(None));
            let uncaptured_error = Arc::new(Mutex::new(None));
            let device_lost_callback = Arc::clone(&device_lost);
            device.set_device_lost_callback(move |reason, message| {
                if let Ok(mut slot) = device_lost_callback.lock() {
                    *slot = Some(format!("{reason:?}: {message}"));
                }
            });
            let uncaptured_error_callback = Arc::clone(&uncaptured_error);
            device.on_uncaptured_error(Arc::new(move |error| {
                if let Ok(mut slot) = uncaptured_error_callback.lock() {
                    *slot = Some(error.to_string());
                }
            }));
            let matmul_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("arcweft-browser-matmul-f32"),
                source: wgpu::ShaderSource::Wgsl(MATMUL_SHADER.into()),
            });
            let add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("arcweft-browser-add-f32"),
                source: wgpu::ShaderSource::Wgsl(ADD_SHADER.into()),
            });
            let bias_add_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("arcweft-browser-bias-add-f32"),
                source: wgpu::ShaderSource::Wgsl(BIAS_ADD_SHADER.into()),
            });
            let matmul_pipeline =
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("arcweft-browser-matmul-f32"),
                    layout: None,
                    module: &matmul_shader,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });
            let add_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("arcweft-browser-add-f32"),
                layout: None,
                module: &add_shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            let bias_add_pipeline =
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("arcweft-browser-bias-add-f32"),
                    layout: None,
                    module: &bias_add_shader,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });
            Ok(Self {
                device,
                queue,
                matmul_pipeline,
                add_pipeline,
                bias_add_pipeline,
                readback: None,
                async_readback: None,
                limits,
                device_lost,
                uncaptured_error,
                stats: BrowserWebGpuMathStats::default(),
                in_flight: 0,
            })
        }

        pub const fn limits(&self) -> BrowserWebGpuLimits {
            self.limits
        }

        pub const fn stats(&self) -> BrowserWebGpuMathStats {
            self.stats
        }

        pub fn reset_stats(&mut self) {
            self.stats = BrowserWebGpuMathStats::default();
            self.in_flight = 0;
        }

        pub async fn auto_matmul_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
            policy: BrowserWebGpuMathAutoPolicy,
        ) -> Result<BrowserWebGpuAutoMathResult<DenseMatrixF32>, BrowserWebGpuError> {
            if lhs.cols() != rhs.rows() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "matmul",
                }
                .into());
            }
            let selection =
                policy.select_matmul_f32(lhs.rows(), lhs.cols(), rhs.cols(), self.limits);
            let value = match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => lhs.matmul_scalar(rhs)?,
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let capacity = selection.capacity().unwrap_or(BrowserMatmulCapacity {
                        rows: lhs.rows(),
                        shared: lhs.cols(),
                        cols: rhs.cols(),
                    });
                    let prepared = self.prepare_matmul_f32(capacity)?;
                    self.upload_prepared_matmul_f32(
                        &prepared,
                        lhs.values(),
                        rhs.values(),
                        lhs.rows(),
                        lhs.cols(),
                        rhs.cols(),
                    )?;
                    let submitted =
                        self.submit_resident_matmul_f32(&prepared, lhs.rows(), rhs.cols())?;
                    let mut out = vec![0.0; lhs.rows() * rhs.cols()];
                    self.read_submitted_f32(submitted, &mut out).await?;
                    DenseMatrixF32::new(lhs.rows(), rhs.cols(), out)?
                }
            };
            Ok(BrowserWebGpuAutoMathResult {
                value,
                selection,
                stats: self.stats,
            })
        }

        pub async fn auto_matrix_add_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
            policy: BrowserWebGpuMathAutoPolicy,
        ) -> Result<BrowserWebGpuAutoMathResult<DenseMatrixF32>, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "add",
                }
                .into());
            }
            let selection = policy.select_elementwise_f32(lhs.values().len(), self.limits);
            let value = match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => lhs.add_scalar(rhs)?,
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let prepared = self.prepare_elementwise_f32(lhs.values().len())?;
                    self.upload_prepared_elementwise_f32(&prepared, lhs.values(), rhs.values())?;
                    let submitted =
                        self.submit_resident_elementwise_f32(&prepared, lhs.values().len())?;
                    let mut out = vec![0.0; lhs.values().len()];
                    self.read_submitted_f32(submitted, &mut out).await?;
                    DenseMatrixF32::new(lhs.rows(), lhs.cols(), out)?
                }
            };
            Ok(BrowserWebGpuAutoMathResult {
                value,
                selection,
                stats: self.stats,
            })
        }

        pub async fn auto_tensor_add_f32(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
            policy: BrowserWebGpuMathAutoPolicy,
        ) -> Result<BrowserWebGpuAutoMathResult<DenseTensorF32>, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::TensorShapeMismatch {
                    lhs: lhs.shape().clone(),
                    rhs: rhs.shape().clone(),
                    op: "add",
                }
                .into());
            }
            let selection = policy.select_elementwise_f32(lhs.values().len(), self.limits);
            let value = match selection.mode() {
                BrowserWebGpuMathMode::CpuWasm => lhs.add_scalar(rhs)?,
                BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
                | BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
                    let prepared = self.prepare_elementwise_f32(lhs.values().len())?;
                    self.upload_prepared_elementwise_f32(&prepared, lhs.values(), rhs.values())?;
                    let submitted =
                        self.submit_resident_elementwise_f32(&prepared, lhs.values().len())?;
                    let mut out = vec![0.0; lhs.values().len()];
                    self.read_submitted_f32(submitted, &mut out).await?;
                    DenseTensorF32::new(lhs.shape().dims().to_vec(), out)?
                }
            };
            Ok(BrowserWebGpuAutoMathResult {
                value,
                selection,
                stats: self.stats,
            })
        }

        pub async fn matmul_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<DenseMatrixF32, BrowserWebGpuError> {
            if lhs.cols() != rhs.rows() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "matmul",
                }
                .into());
            }
            let out_len = checked_len_mul(lhs.rows(), rhs.cols())?;
            validate_f32_storage(self.limits, lhs.values().len())?;
            validate_f32_storage(self.limits, rhs.values().len())?;
            validate_f32_storage(self.limits, out_len)?;
            let mut out = vec![0.0; out_len];
            let params = MatmulParams {
                rows: checked_u32(lhs.rows())?,
                shared: checked_u32(lhs.cols())?,
                cols: checked_u32(rhs.cols())?,
                _pad: 0,
            };
            let pipeline = self.matmul_pipeline.clone();
            self.dispatch(
                &pipeline,
                &[
                    bytemuck::cast_slice(lhs.values()),
                    bytemuck::cast_slice(rhs.values()),
                ],
                bytemuck::bytes_of(&params),
                &mut out,
                checked_workgroups(self.limits, rhs.cols().div_ceil(16))?,
                checked_workgroups(self.limits, lhs.rows().div_ceil(16))?,
            )
            .await?;
            DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into)
        }

        pub async fn matrix_add_f32(
            &mut self,
            lhs: &DenseMatrixF32,
            rhs: &DenseMatrixF32,
        ) -> Result<DenseMatrixF32, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::MatrixShapeMismatch {
                    lhs: lhs.shape(),
                    rhs: rhs.shape(),
                    op: "add",
                }
                .into());
            }
            let mut out = vec![0.0; lhs.values().len()];
            self.dispatch_add(lhs.values(), rhs.values(), &mut out)
                .await?;
            DenseMatrixF32::new(lhs.rows(), lhs.cols(), out).map_err(Into::into)
        }

        pub async fn tensor_add_f32(
            &mut self,
            lhs: &DenseTensorF32,
            rhs: &DenseTensorF32,
        ) -> Result<DenseTensorF32, BrowserWebGpuError> {
            if lhs.shape() != rhs.shape() {
                return Err(RuntimeMathError::TensorShapeMismatch {
                    lhs: lhs.shape().clone(),
                    rhs: rhs.shape().clone(),
                    op: "add",
                }
                .into());
            }
            let mut out = vec![0.0; lhs.values().len()];
            self.dispatch_add(lhs.values(), rhs.values(), &mut out)
                .await?;
            DenseTensorF32::new(lhs.shape().dims().to_vec(), out).map_err(Into::into)
        }

        pub fn prepare_elementwise_f32(
            &mut self,
            capacity_len: usize,
        ) -> Result<BrowserPreparedElementwiseF32, BrowserWebGpuError> {
            validate_f32_storage(self.limits, capacity_len)?;
            let byte_len = checked_f32_bytes(capacity_len)?.max(std::mem::size_of::<f32>() as u64);
            let lhs = self.storage_buffer_capacity(byte_len, "arcweft-browser-prepared-add-lhs");
            let rhs = self.storage_buffer_capacity(byte_len, "arcweft-browser-prepared-add-rhs");
            let out = self.output_buffer_capacity(byte_len, "arcweft-browser-prepared-add-out");
            let params = self.storage_buffer_capacity(
                std::mem::size_of::<AddParams>() as u64,
                "arcweft-browser-prepared-add-params",
            );
            let layout = self.add_pipeline.get_bind_group_layout(0);
            let entries = bind_group_entries(&lhs, &rhs, &out, &params);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-browser-prepared-add-bind-group"),
                layout: &layout,
                entries: &entries,
            });
            self.stats.buffer_creations += 4;
            self.stats.bind_group_rebuilds += 1;
            Ok(BrowserPreparedElementwiseF32 {
                capacity_len,
                lhs,
                rhs,
                out,
                params,
                bind_group,
            })
        }

        pub fn prepare_matmul_f32(
            &mut self,
            capacity: BrowserMatmulCapacity,
        ) -> Result<BrowserPreparedMatmulF32, BrowserWebGpuError> {
            let lhs_len = checked_len_mul(capacity.rows, capacity.shared)?;
            let rhs_len = checked_len_mul(capacity.shared, capacity.cols)?;
            let out_len = checked_len_mul(capacity.rows, capacity.cols)?;
            validate_f32_storage(self.limits, lhs_len)?;
            validate_f32_storage(self.limits, rhs_len)?;
            validate_f32_storage(self.limits, out_len)?;
            let lhs = self.storage_buffer_capacity(
                checked_f32_bytes(lhs_len)?.max(std::mem::size_of::<f32>() as u64),
                "arcweft-browser-prepared-matmul-lhs",
            );
            let rhs = self.storage_buffer_capacity(
                checked_f32_bytes(rhs_len)?.max(std::mem::size_of::<f32>() as u64),
                "arcweft-browser-prepared-matmul-rhs",
            );
            let out = self.output_buffer_capacity(
                checked_f32_bytes(out_len)?.max(std::mem::size_of::<f32>() as u64),
                "arcweft-browser-prepared-matmul-out",
            );
            let params = self.storage_buffer_capacity(
                std::mem::size_of::<MatmulParams>() as u64,
                "arcweft-browser-prepared-matmul-params",
            );
            let layout = self.matmul_pipeline.get_bind_group_layout(0);
            let entries = bind_group_entries(&lhs, &rhs, &out, &params);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-browser-prepared-matmul-bind-group"),
                layout: &layout,
                entries: &entries,
            });
            self.stats.buffer_creations += 4;
            self.stats.bind_group_rebuilds += 1;
            Ok(BrowserPreparedMatmulF32 {
                capacity,
                lhs,
                rhs,
                out,
                params,
                bind_group,
            })
        }

        pub fn prepare_matmul_add_f32(
            &mut self,
            capacity: BrowserMatmulCapacity,
        ) -> Result<BrowserPreparedMatmulAddF32, BrowserWebGpuError> {
            let output_len = checked_len_mul(capacity.rows, capacity.cols)?;
            let matmul = self.prepare_matmul_f32(capacity)?;
            let add = self.prepare_elementwise_f32(output_len)?;
            let add_layout = self.add_pipeline.get_bind_group_layout(0);
            let add_entries = bind_group_entries(&matmul.out, &add.rhs, &add.out, &add.params);
            let add_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-browser-prepared-matmul-add-bind-group"),
                layout: &add_layout,
                entries: &add_entries,
            });
            self.stats.bind_group_rebuilds += 1;
            Ok(BrowserPreparedMatmulAddF32 {
                matmul,
                add,
                add_bind_group,
            })
        }

        pub fn prepare_matmul_bias_add_f32(
            &mut self,
            capacity: BrowserMatmulCapacity,
        ) -> Result<BrowserPreparedMatmulBiasAddF32, BrowserWebGpuError> {
            let output_len = checked_len_mul(capacity.rows, capacity.cols)?;
            validate_f32_storage(self.limits, capacity.cols)?;
            validate_f32_storage(self.limits, output_len)?;
            let matmul = self.prepare_matmul_f32(capacity)?;
            let bias = self.storage_buffer_capacity(
                checked_f32_bytes(capacity.cols)?.max(std::mem::size_of::<f32>() as u64),
                "arcweft-browser-prepared-bias-add-bias",
            );
            let out = self.output_buffer_capacity(
                checked_f32_bytes(output_len)?.max(std::mem::size_of::<f32>() as u64),
                "arcweft-browser-prepared-bias-add-out",
            );
            let params = self.storage_buffer_capacity(
                std::mem::size_of::<BiasAddParams>() as u64,
                "arcweft-browser-prepared-bias-add-params",
            );
            let layout = self.bias_add_pipeline.get_bind_group_layout(0);
            let entries = bind_group_entries(&matmul.out, &bias, &out, &params);
            let bias_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-browser-prepared-matmul-bias-add-bind-group"),
                layout: &layout,
                entries: &entries,
            });
            self.stats.buffer_creations += 3;
            self.stats.bind_group_rebuilds += 1;
            Ok(BrowserPreparedMatmulBiasAddF32 {
                matmul,
                bias,
                out,
                params,
                bias_bind_group,
                output_capacity_len: output_len,
            })
        }

        pub fn prepare_resident_f32_graph(
            &mut self,
            spec: BrowserResidentF32GraphSpec,
        ) -> Result<BrowserPreparedResidentF32Graph, BrowserWebGpuError> {
            match spec {
                BrowserResidentF32GraphSpec::MatmulAdd { capacity } => self
                    .prepare_matmul_add_f32(capacity)
                    .map(BrowserPreparedResidentF32Graph::MatmulAdd),
                BrowserResidentF32GraphSpec::MatmulBiasAdd { capacity } => self
                    .prepare_matmul_bias_add_f32(capacity)
                    .map(BrowserPreparedResidentF32Graph::MatmulBiasAdd),
            }
        }

        pub async fn dispatch_prepared_elementwise_f32(
            &mut self,
            prepared: &BrowserPreparedElementwiseF32,
            lhs: &[f32],
            rhs: &[f32],
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            self.upload_prepared_elementwise_f32(prepared, lhs, rhs)?;
            self.dispatch_resident_elementwise_f32(prepared, out).await
        }

        pub fn upload_prepared_elementwise_f32(
            &mut self,
            prepared: &BrowserPreparedElementwiseF32,
            lhs: &[f32],
            rhs: &[f32],
        ) -> Result<(), BrowserWebGpuError> {
            if lhs.len() != rhs.len() {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "prepared elementwise add requires matching input lengths",
                ));
            }
            if lhs.len() > prepared.capacity_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "prepared elementwise add exceeded prepared capacity",
                ));
            }
            let params = AddParams {
                len: checked_u32(lhs.len())?,
                x_threads: checked_u32(add_groups(self.limits, lhs.len())?.0 as usize * 256)?,
                _pad1: 0,
                _pad2: 0,
            };
            self.queue
                .write_buffer(&prepared.lhs, 0, bytemuck::cast_slice(lhs));
            self.queue
                .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
            self.queue
                .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
            self.stats.bytes_uploaded += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
            self.stats.bytes_copied += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
            self.stats.buffer_reuse_hits += 3;
            Ok(())
        }

        pub fn upload_prepared_elementwise_rhs_f32(
            &mut self,
            prepared: &BrowserPreparedElementwiseF32,
            rhs: &[f32],
        ) -> Result<(), BrowserWebGpuError> {
            if rhs.len() > prepared.capacity_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "prepared elementwise add rhs exceeded prepared capacity",
                ));
            }
            let params = AddParams {
                len: checked_u32(rhs.len())?,
                x_threads: checked_u32(add_groups(self.limits, rhs.len())?.0 as usize * 256)?,
                _pad1: 0,
                _pad2: 0,
            };
            self.queue
                .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
            self.queue
                .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
            self.stats.bytes_uploaded += std::mem::size_of_val(rhs);
            self.stats.bytes_copied += std::mem::size_of_val(rhs);
            self.stats.buffer_reuse_hits += 2;
            Ok(())
        }

        pub async fn dispatch_resident_elementwise_f32(
            &mut self,
            prepared: &BrowserPreparedElementwiseF32,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            if out.len() > prepared.capacity_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident elementwise add exceeded prepared capacity",
                ));
            }
            if out.is_empty() {
                return Ok(());
            }
            let (groups_x, groups_y) = add_groups(self.limits, out.len())?;
            let pipeline = self.add_pipeline.clone();
            self.dispatch_prepared(
                &pipeline,
                &prepared.bind_group,
                &prepared.out,
                out,
                groups_x,
                groups_y,
            )
            .await?;
            self.record_prepared_readback(out);
            Ok(())
        }

        pub fn submit_resident_elementwise_f32(
            &mut self,
            prepared: &BrowserPreparedElementwiseF32,
            len: usize,
        ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
            if len > prepared.capacity_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident elementwise add exceeded prepared capacity",
                ));
            }
            if len == 0 {
                return Ok(BrowserSubmittedF32 {
                    readback: None,
                    len,
                    submitted_at_ms: browser_now_ms(),
                });
            }
            let (groups_x, groups_y) = add_groups(self.limits, len)?;
            let pipeline = self.add_pipeline.clone();
            self.submit_prepared(
                &pipeline,
                &prepared.bind_group,
                &prepared.out,
                len,
                groups_x,
                groups_y,
            )
        }

        pub fn submit_resident_elementwise_f32_without_readback(
            &mut self,
            prepared: &BrowserPreparedElementwiseF32,
            len: usize,
        ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
            if len > prepared.capacity_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident elementwise add exceeded prepared capacity",
                ));
            }
            if len == 0 {
                return Ok(BrowserResidentSubmission {
                    len,
                    submitted_at_ms: browser_now_ms(),
                });
            }
            let (groups_x, groups_y) = add_groups(self.limits, len)?;
            let pipeline = self.add_pipeline.clone();
            self.submit_prepared_without_readback(
                &pipeline,
                &prepared.bind_group,
                len,
                groups_x,
                groups_y,
            )
        }

        pub async fn read_resident_elementwise_f32(
            &mut self,
            prepared: &BrowserPreparedElementwiseF32,
            submission: BrowserResidentSubmission,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            if out.len() != submission.len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "resident elementwise output length differs from readback target",
                ));
            }
            self.read_resident_f32(&prepared.out, submission, out).await
        }

        pub async fn dispatch_prepared_matmul_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulF32,
            lhs: &[f32],
            rhs: &[f32],
            out: &mut [f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<(), BrowserWebGpuError> {
            self.upload_prepared_matmul_f32(prepared, lhs, rhs, rows, shared, cols)?;
            self.dispatch_resident_matmul_f32(prepared, out, rows, cols)
                .await
        }

        pub fn upload_prepared_matmul_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulF32,
            lhs: &[f32],
            rhs: &[f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<(), BrowserWebGpuError> {
            if rows > prepared.capacity.rows
                || shared > prepared.capacity.shared
                || cols > prepared.capacity.cols
            {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "prepared matmul exceeded prepared capacity",
                ));
            }
            if lhs.len() != checked_len_mul(rows, shared)?
                || rhs.len() != checked_len_mul(shared, cols)?
            {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "prepared matmul shape and buffer lengths differ",
                ));
            }
            let params = MatmulParams {
                rows: checked_u32(rows)?,
                shared: checked_u32(shared)?,
                cols: checked_u32(cols)?,
                _pad: 0,
            };
            self.queue
                .write_buffer(&prepared.lhs, 0, bytemuck::cast_slice(lhs));
            self.queue
                .write_buffer(&prepared.rhs, 0, bytemuck::cast_slice(rhs));
            self.queue
                .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
            self.stats.bytes_uploaded += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
            self.stats.bytes_copied += std::mem::size_of_val(lhs) + std::mem::size_of_val(rhs);
            self.stats.buffer_reuse_hits += 3;
            Ok(())
        }

        pub fn upload_prepared_matmul_add_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulAddF32,
            lhs: &[f32],
            rhs: &[f32],
            add_rhs: &[f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<(), BrowserWebGpuError> {
            let output_len = checked_len_mul(rows, cols)?;
            if add_rhs.len() != output_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "prepared matmul-add shape and add rhs length differ",
                ));
            }
            self.upload_prepared_matmul_f32(&prepared.matmul, lhs, rhs, rows, shared, cols)?;
            self.upload_prepared_elementwise_rhs_f32(&prepared.add, add_rhs)
        }

        pub fn upload_prepared_matmul_bias_add_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulBiasAddF32,
            lhs: &[f32],
            rhs: &[f32],
            bias: &[f32],
            rows: usize,
            shared: usize,
            cols: usize,
        ) -> Result<(), BrowserWebGpuError> {
            if bias.len() != cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "prepared matmul-bias-add shape and bias length differ",
                ));
            }
            self.upload_prepared_matmul_f32(&prepared.matmul, lhs, rhs, rows, shared, cols)?;
            let params = BiasAddParams {
                rows: checked_u32(rows)?,
                cols: checked_u32(cols)?,
                _pad1: 0,
                _pad2: 0,
            };
            self.queue
                .write_buffer(&prepared.bias, 0, bytemuck::cast_slice(bias));
            self.queue
                .write_buffer(&prepared.params, 0, bytemuck::bytes_of(&params));
            self.stats.bytes_uploaded += std::mem::size_of_val(bias);
            self.stats.bytes_copied += std::mem::size_of_val(bias);
            self.stats.buffer_reuse_hits += 2;
            Ok(())
        }

        pub fn upload_prepared_resident_f32_graph(
            &mut self,
            prepared: &BrowserPreparedResidentF32Graph,
            inputs: BrowserResidentF32GraphInputs<'_>,
        ) -> Result<(), BrowserWebGpuError> {
            match (prepared, inputs) {
                (
                    BrowserPreparedResidentF32Graph::MatmulAdd(prepared),
                    BrowserResidentF32GraphInputs::MatmulAdd(inputs),
                ) => self.upload_prepared_matmul_add_f32(
                    prepared,
                    inputs.lhs,
                    inputs.rhs,
                    inputs.add_rhs,
                    inputs.shape.rows,
                    inputs.shape.shared,
                    inputs.shape.cols,
                ),
                (
                    BrowserPreparedResidentF32Graph::MatmulBiasAdd(prepared),
                    BrowserResidentF32GraphInputs::MatmulBiasAdd(inputs),
                ) => self.upload_prepared_matmul_bias_add_f32(
                    prepared,
                    inputs.lhs,
                    inputs.rhs,
                    inputs.bias,
                    inputs.shape.rows,
                    inputs.shape.shared,
                    inputs.shape.cols,
                ),
                (
                    BrowserPreparedResidentF32Graph::MatmulAdd(_),
                    BrowserResidentF32GraphInputs::MatmulBiasAdd(_),
                )
                | (
                    BrowserPreparedResidentF32Graph::MatmulBiasAdd(_),
                    BrowserResidentF32GraphInputs::MatmulAdd(_),
                ) => Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "prepared resident graph inputs do not match the prepared graph fragment",
                )),
            }
        }

        pub async fn dispatch_resident_matmul_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulF32,
            out: &mut [f32],
            rows: usize,
            cols: usize,
        ) -> Result<(), BrowserWebGpuError> {
            if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul exceeded prepared capacity",
                ));
            }
            if out.len() != checked_len_mul(rows, cols)? {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "resident matmul output length differs from shape",
                ));
            }
            if out.is_empty() {
                return Ok(());
            }
            let pipeline = self.matmul_pipeline.clone();
            self.dispatch_prepared(
                &pipeline,
                &prepared.bind_group,
                &prepared.out,
                out,
                checked_workgroups(self.limits, cols.div_ceil(16))?,
                checked_workgroups(self.limits, rows.div_ceil(16))?,
            )
            .await?;
            self.record_prepared_readback(out);
            Ok(())
        }

        pub fn submit_resident_matmul_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulF32,
            rows: usize,
            cols: usize,
        ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
            if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul exceeded prepared capacity",
                ));
            }
            let len = checked_len_mul(rows, cols)?;
            if len == 0 {
                return Ok(BrowserSubmittedF32 {
                    readback: None,
                    len,
                    submitted_at_ms: browser_now_ms(),
                });
            }
            let pipeline = self.matmul_pipeline.clone();
            self.submit_prepared(
                &pipeline,
                &prepared.bind_group,
                &prepared.out,
                len,
                checked_workgroups(self.limits, cols.div_ceil(16))?,
                checked_workgroups(self.limits, rows.div_ceil(16))?,
            )
        }

        pub fn submit_resident_matmul_f32_without_readback(
            &mut self,
            prepared: &BrowserPreparedMatmulF32,
            rows: usize,
            cols: usize,
        ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
            if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul exceeded prepared capacity",
                ));
            }
            let len = checked_len_mul(rows, cols)?;
            if len == 0 {
                return Ok(BrowserResidentSubmission {
                    len,
                    submitted_at_ms: browser_now_ms(),
                });
            }
            let pipeline = self.matmul_pipeline.clone();
            self.submit_prepared_without_readback(
                &pipeline,
                &prepared.bind_group,
                len,
                checked_workgroups(self.limits, cols.div_ceil(16))?,
                checked_workgroups(self.limits, rows.div_ceil(16))?,
            )
        }

        pub fn submit_resident_matmul_add_f32_without_readback(
            &mut self,
            prepared: &BrowserPreparedMatmulAddF32,
            rows: usize,
            cols: usize,
        ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
            if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul-add exceeded prepared matmul capacity",
                ));
            }
            let len = checked_len_mul(rows, cols)?;
            if len > prepared.add.capacity_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul-add exceeded prepared add capacity",
                ));
            }
            if len == 0 {
                return Ok(BrowserResidentSubmission {
                    len,
                    submitted_at_ms: browser_now_ms(),
                });
            }
            self.ensure_healthy()?;
            validate_f32_storage(self.limits, len)?;
            let matmul_pipeline = self.matmul_pipeline.clone();
            let add_pipeline = self.add_pipeline.clone();
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-browser-resident-matmul-add-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-resident-matmul-add-matmul-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&matmul_pipeline);
                pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_workgroups(self.limits, cols.div_ceil(16))?,
                    checked_workgroups(self.limits, rows.div_ceil(16))?,
                    1,
                );
            }
            {
                let (groups_x, groups_y) = add_groups(self.limits, len)?;
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-resident-matmul-add-add-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&add_pipeline);
                pass.set_bind_group(0, &prepared.add_bind_group, &[]);
                pass.dispatch_workgroups(groups_x.max(1), groups_y.max(1), 1);
            }
            self.queue.submit(Some(encoder.finish()));
            self.stats.dispatches += 2;
            self.stats.async_submissions += 1;
            self.stats.pipeline_cache_hits += 2;
            Ok(BrowserResidentSubmission {
                len,
                submitted_at_ms: browser_now_ms(),
            })
        }

        pub fn submit_resident_matmul_bias_add_f32_without_readback(
            &mut self,
            prepared: &BrowserPreparedMatmulBiasAddF32,
            rows: usize,
            cols: usize,
        ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
            if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul-bias-add exceeded prepared matmul capacity",
                ));
            }
            let len = checked_len_mul(rows, cols)?;
            if len > prepared.output_capacity_len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul-bias-add exceeded prepared output capacity",
                ));
            }
            if len == 0 {
                return Ok(BrowserResidentSubmission {
                    len,
                    submitted_at_ms: browser_now_ms(),
                });
            }
            self.ensure_healthy()?;
            validate_f32_storage(self.limits, len)?;
            let matmul_pipeline = self.matmul_pipeline.clone();
            let bias_add_pipeline = self.bias_add_pipeline.clone();
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-browser-resident-matmul-bias-add-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-resident-matmul-bias-add-matmul-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&matmul_pipeline);
                pass.set_bind_group(0, &prepared.matmul.bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_workgroups(self.limits, cols.div_ceil(16))?,
                    checked_workgroups(self.limits, rows.div_ceil(16))?,
                    1,
                );
            }
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-resident-matmul-bias-add-bias-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&bias_add_pipeline);
                pass.set_bind_group(0, &prepared.bias_bind_group, &[]);
                pass.dispatch_workgroups(
                    checked_workgroups(self.limits, cols.div_ceil(16))?,
                    checked_workgroups(self.limits, rows.div_ceil(16))?,
                    1,
                );
            }
            self.queue.submit(Some(encoder.finish()));
            self.stats.dispatches += 2;
            self.stats.async_submissions += 1;
            self.stats.pipeline_cache_hits += 2;
            Ok(BrowserResidentSubmission {
                len,
                submitted_at_ms: browser_now_ms(),
            })
        }

        pub fn submit_prepared_resident_f32_graph_without_readback(
            &mut self,
            prepared: &BrowserPreparedResidentF32Graph,
            shape: BrowserMatmulAddF32Shape,
        ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
            match prepared {
                BrowserPreparedResidentF32Graph::MatmulAdd(prepared) => self
                    .submit_resident_matmul_add_f32_without_readback(
                        prepared, shape.rows, shape.cols,
                    ),
                BrowserPreparedResidentF32Graph::MatmulBiasAdd(prepared) => self
                    .submit_resident_matmul_bias_add_f32_without_readback(
                        prepared, shape.rows, shape.cols,
                    ),
            }
        }

        pub async fn read_resident_matmul_add_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulAddF32,
            submission: BrowserResidentSubmission,
            rows: usize,
            cols: usize,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul-add exceeded prepared capacity",
                ));
            }
            if out.len() != submission.len || out.len() != checked_len_mul(rows, cols)? {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "resident matmul-add output length differs from readback target",
                ));
            }
            self.read_resident_f32(&prepared.add.out, submission, out)
                .await
        }

        pub async fn read_resident_matmul_bias_add_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulBiasAddF32,
            submission: BrowserResidentSubmission,
            rows: usize,
            cols: usize,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            if rows > prepared.matmul.capacity.rows || cols > prepared.matmul.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul-bias-add exceeded prepared capacity",
                ));
            }
            if out.len() != submission.len || out.len() != checked_len_mul(rows, cols)? {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "resident matmul-bias-add output length differs from readback target",
                ));
            }
            self.read_resident_f32(&prepared.out, submission, out).await
        }

        pub async fn read_prepared_resident_f32_graph(
            &mut self,
            prepared: &BrowserPreparedResidentF32Graph,
            submission: BrowserResidentSubmission,
            shape: BrowserMatmulAddF32Shape,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            match prepared {
                BrowserPreparedResidentF32Graph::MatmulAdd(prepared) => {
                    self.read_resident_matmul_add_f32(
                        prepared, submission, shape.rows, shape.cols, out,
                    )
                    .await
                }
                BrowserPreparedResidentF32Graph::MatmulBiasAdd(prepared) => {
                    self.read_resident_matmul_bias_add_f32(
                        prepared, submission, shape.rows, shape.cols, out,
                    )
                    .await
                }
            }
        }

        pub async fn read_resident_matmul_f32(
            &mut self,
            prepared: &BrowserPreparedMatmulF32,
            submission: BrowserResidentSubmission,
            rows: usize,
            cols: usize,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            if rows > prepared.capacity.rows || cols > prepared.capacity.cols {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                    "resident matmul exceeded prepared capacity",
                ));
            }
            if out.len() != submission.len || out.len() != checked_len_mul(rows, cols)? {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "resident matmul output length differs from readback target",
                ));
            }
            self.read_resident_f32(&prepared.out, submission, out).await
        }

        pub async fn read_submitted_f32(
            &mut self,
            submitted: BrowserSubmittedF32,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            if out.len() != submitted.len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "submitted browser GPU output length differs from readback target",
                ));
            }
            let Some(buffer) = submitted.readback else {
                return Ok(());
            };
            self.read_owned_staging_buffer(buffer, submitted.submitted_at_ms, out)
                .await?;
            self.in_flight = self.in_flight.saturating_sub(1);
            self.stats.async_readbacks += 1;
            Ok(())
        }

        async fn dispatch_add(
            &mut self,
            lhs: &[f32],
            rhs: &[f32],
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            validate_f32_storage(self.limits, lhs.len())?;
            validate_f32_storage(self.limits, rhs.len())?;
            validate_f32_storage(self.limits, out.len())?;
            let (groups_x, groups_y) = add_groups(self.limits, out.len())?;
            let params = AddParams {
                len: checked_u32(out.len())?,
                x_threads: checked_u32(groups_x as usize * 256)?,
                _pad1: 0,
                _pad2: 0,
            };
            let pipeline = self.add_pipeline.clone();
            self.dispatch(
                &pipeline,
                &[bytemuck::cast_slice(lhs), bytemuck::cast_slice(rhs)],
                bytemuck::bytes_of(&params),
                out,
                groups_x,
                groups_y,
            )
            .await
        }

        async fn dispatch(
            &mut self,
            pipeline: &wgpu::ComputePipeline,
            input_bytes: &[&[u8]],
            params_bytes: &[u8],
            out: &mut [f32],
            workgroups_x: u32,
            workgroups_y: u32,
        ) -> Result<(), BrowserWebGpuError> {
            if out.is_empty() {
                return Ok(());
            }
            self.ensure_healthy()?;
            let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let lhs = storage_buffer(&self.device, input_bytes[0], "arcweft-browser-math-lhs");
            let rhs = storage_buffer(&self.device, input_bytes[1], "arcweft-browser-math-rhs");
            let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("arcweft-browser-math-out"),
                size: checked_f32_bytes(out.len())?,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let params = storage_buffer(&self.device, params_bytes, "arcweft-browser-math-params");
            let layout = pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arcweft-browser-math-bind-group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: lhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: rhs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: out_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params.as_entire_binding(),
                    },
                ],
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-browser-math-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-math-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
            }
            self.encode_readback(&mut encoder, &out_buffer, out.len())?;
            self.queue.submit(Some(encoder.finish()));
            self.stats.dispatches += 1;
            self.stats.bytes_uploaded += input_bytes.iter().map(|bytes| bytes.len()).sum::<usize>();
            self.stats.bytes_downloaded += std::mem::size_of_val(out);
            self.stats.bytes_copied += input_bytes.iter().map(|bytes| bytes.len()).sum::<usize>()
                + std::mem::size_of_val(out);
            self.stats.buffer_creations += 4;
            if let Some(error) = scope.pop().await {
                return Err(map_wgpu_error(error));
            }
            self.read_staging_buffer(out).await
        }

        async fn dispatch_prepared(
            &mut self,
            pipeline: &wgpu::ComputePipeline,
            bind_group: &wgpu::BindGroup,
            source: &wgpu::Buffer,
            out: &mut [f32],
            workgroups_x: u32,
            workgroups_y: u32,
        ) -> Result<(), BrowserWebGpuError> {
            self.ensure_healthy()?;
            let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-browser-prepared-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-prepared-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
            }
            self.encode_readback(&mut encoder, source, out.len())?;
            self.queue.submit(Some(encoder.finish()));
            self.stats.dispatches += 1;
            if let Some(error) = scope.pop().await {
                return Err(map_wgpu_error(error));
            }
            self.read_staging_buffer(out).await
        }

        fn submit_prepared(
            &mut self,
            pipeline: &wgpu::ComputePipeline,
            bind_group: &wgpu::BindGroup,
            source: &wgpu::Buffer,
            elements: usize,
            workgroups_x: u32,
            workgroups_y: u32,
        ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
            self.ensure_healthy()?;
            let byte_len = checked_f32_bytes(elements)?;
            let readback = self.take_async_readback_buffer(byte_len)?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-browser-async-prepared-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-async-prepared-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
            }
            encoder.copy_buffer_to_buffer(source, 0, &readback.buffer, 0, byte_len);
            self.queue.submit(Some(encoder.finish()));
            self.stats.dispatches += 1;
            self.stats.async_submissions += 1;
            self.stats.bytes_downloaded += std::mem::size_of::<f32>() * elements;
            self.stats.bytes_copied += std::mem::size_of::<f32>() * elements;
            self.stats.buffer_reuse_hits += 1;
            self.stats.pipeline_cache_hits += 1;
            self.in_flight += 1;
            self.stats.max_in_flight = self.stats.max_in_flight.max(self.in_flight);
            Ok(BrowserSubmittedF32 {
                readback: Some(readback),
                len: elements,
                submitted_at_ms: browser_now_ms(),
            })
        }

        fn submit_prepared_without_readback(
            &mut self,
            pipeline: &wgpu::ComputePipeline,
            bind_group: &wgpu::BindGroup,
            elements: usize,
            workgroups_x: u32,
            workgroups_y: u32,
        ) -> Result<BrowserResidentSubmission, BrowserWebGpuError> {
            self.ensure_healthy()?;
            validate_f32_storage(self.limits, elements)?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-browser-resident-prepared-encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("arcweft-browser-resident-prepared-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(workgroups_x.max(1), workgroups_y.max(1), 1);
            }
            self.queue.submit(Some(encoder.finish()));
            self.stats.dispatches += 1;
            self.stats.async_submissions += 1;
            self.stats.pipeline_cache_hits += 1;
            Ok(BrowserResidentSubmission {
                len: elements,
                submitted_at_ms: browser_now_ms(),
            })
        }

        async fn read_resident_f32(
            &mut self,
            source: &wgpu::Buffer,
            submission: BrowserResidentSubmission,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            if out.len() != submission.len {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::RequiredLimitsUnsupported,
                    "resident output length differs from readback target",
                ));
            }
            if out.is_empty() {
                return Ok(());
            }
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("arcweft-browser-resident-readback-encoder"),
                });
            self.encode_readback(&mut encoder, source, out.len())?;
            self.queue.submit(Some(encoder.finish()));
            self.stats.bytes_downloaded += std::mem::size_of_val(out);
            self.stats.bytes_copied += std::mem::size_of_val(out);
            self.stats.buffer_reuse_hits += 1;
            self.read_staging_buffer_from(submission.submitted_at_ms, out)
                .await
        }

        fn encode_readback(
            &mut self,
            encoder: &mut wgpu::CommandEncoder,
            source: &wgpu::Buffer,
            elements: usize,
        ) -> Result<(), BrowserWebGpuError> {
            let byte_len = checked_f32_bytes(elements)?;
            let readback = self.ensure_readback_buffer(byte_len)?;
            encoder.copy_buffer_to_buffer(source, 0, readback, 0, byte_len);
            Ok(())
        }

        fn ensure_readback_buffer(
            &mut self,
            byte_len: u64,
        ) -> Result<&wgpu::Buffer, BrowserWebGpuError> {
            validate_byte_len(self.limits, byte_len)?;
            let byte_len_usize = checked_usize_bytes(byte_len)?;
            let needs_new = self
                .readback
                .as_ref()
                .is_none_or(|buffer| buffer.byte_len < byte_len_usize);
            if needs_new {
                self.readback = Some(ReusableReadbackBuffer {
                    buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("arcweft-browser-math-readback"),
                        size: byte_len.max(std::mem::size_of::<f32>() as u64),
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    byte_len: byte_len_usize,
                });
                self.stats.readback_buffer_creations += 1;
            } else {
                self.stats.readback_buffer_reuse_hits += 1;
            }
            Ok(&self
                .readback
                .as_ref()
                .expect("browser readback buffer was initialized")
                .buffer)
        }

        fn take_async_readback_buffer(
            &mut self,
            byte_len: u64,
        ) -> Result<ReusableReadbackBuffer, BrowserWebGpuError> {
            validate_byte_len(self.limits, byte_len)?;
            let byte_len_usize = checked_usize_bytes(byte_len)?;
            if let Some(buffer) = self.async_readback.take()
                && buffer.byte_len >= byte_len_usize
            {
                self.stats.readback_buffer_reuse_hits += 1;
                return Ok(buffer);
            }
            self.stats.readback_buffer_creations += 1;
            Ok(ReusableReadbackBuffer {
                buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("arcweft-browser-async-readback"),
                    size: byte_len.max(std::mem::size_of::<f32>() as u64),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                byte_len: byte_len_usize,
            })
        }

        async fn read_staging_buffer(&mut self, out: &mut [f32]) -> Result<(), BrowserWebGpuError> {
            self.read_staging_buffer_from(browser_now_ms(), out).await
        }

        async fn read_staging_buffer_from(
            &mut self,
            submitted_at_ms: f64,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            let byte_len = std::mem::size_of_val(out);
            let buffer = &self
                .readback
                .as_ref()
                .expect("browser readback buffer was initialized")
                .buffer;
            let (sender, receiver) = oneshot::channel();
            buffer
                .slice(0..byte_len as u64)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            receiver
                .await
                .map_err(|error| {
                    BrowserWebGpuError::fallback(
                        BrowserWebGpuFallbackReason::MapFailed,
                        error.to_string(),
                    )
                })?
                .map_err(|error| {
                    BrowserWebGpuError::fallback(
                        BrowserWebGpuFallbackReason::MapFailed,
                        error.to_string(),
                    )
                })?;
            self.stats.map_count += 1;
            self.stats.map_wait_ms += browser_now_ms() - submitted_at_ms;
            {
                let mapped = buffer.slice(0..byte_len as u64).get_mapped_range();
                let values: &[f32] = bytemuck::cast_slice(&mapped);
                out.copy_from_slice(values);
            }
            buffer.unmap();
            Ok(())
        }

        async fn read_owned_staging_buffer(
            &mut self,
            buffer: ReusableReadbackBuffer,
            submitted_at_ms: f64,
            out: &mut [f32],
        ) -> Result<(), BrowserWebGpuError> {
            let byte_len = std::mem::size_of_val(out);
            let (sender, receiver) = oneshot::channel();
            buffer
                .buffer
                .slice(0..byte_len as u64)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            receiver
                .await
                .map_err(|error| {
                    BrowserWebGpuError::fallback(
                        BrowserWebGpuFallbackReason::MapFailed,
                        error.to_string(),
                    )
                })?
                .map_err(|error| {
                    BrowserWebGpuError::fallback(
                        BrowserWebGpuFallbackReason::MapFailed,
                        error.to_string(),
                    )
                })?;
            self.stats.map_count += 1;
            self.stats.map_wait_ms += browser_now_ms() - submitted_at_ms;
            {
                let mapped = buffer.buffer.slice(0..byte_len as u64).get_mapped_range();
                let values: &[f32] = bytemuck::cast_slice(&mapped);
                out.copy_from_slice(values);
            }
            buffer.buffer.unmap();
            self.async_readback = Some(buffer);
            Ok(())
        }

        fn storage_buffer_capacity(&self, byte_len: u64, label: &'static str) -> wgpu::Buffer {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: byte_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        }

        fn output_buffer_capacity(&self, byte_len: u64, label: &'static str) -> wgpu::Buffer {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: byte_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        }

        fn record_prepared_readback(&mut self, out: &[f32]) {
            let downloaded = std::mem::size_of_val(out);
            self.stats.bytes_downloaded += downloaded;
            self.stats.bytes_copied += downloaded;
            self.stats.buffer_reuse_hits += 1;
            self.stats.pipeline_cache_hits += 1;
        }

        fn ensure_healthy(&self) -> Result<(), BrowserWebGpuError> {
            if let Ok(slot) = self.device_lost.lock()
                && let Some(message) = slot.as_ref()
            {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::DeviceLost,
                    message.clone(),
                ));
            }
            if let Ok(slot) = self.uncaptured_error.lock()
                && let Some(message) = slot.as_ref()
            {
                return Err(BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::ValidationError,
                    message.clone(),
                ));
            }
            Ok(())
        }
    }

    fn storage_buffer(device: &wgpu::Device, contents: &[u8], label: &'static str) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        })
    }

    fn bind_group_entries<'a>(
        lhs: &'a wgpu::Buffer,
        rhs: &'a wgpu::Buffer,
        out: &'a wgpu::Buffer,
        params: &'a wgpu::Buffer,
    ) -> [wgpu::BindGroupEntry<'a>; 4] {
        [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: lhs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: rhs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params.as_entire_binding(),
            },
        ]
    }

    const fn matmul_capacity_covers(
        available: BrowserMatmulCapacity,
        required: BrowserMatmulCapacity,
    ) -> bool {
        available.rows >= required.rows
            && available.shared >= required.shared
            && available.cols >= required.cols
    }

    impl From<wgpu::Limits> for BrowserWebGpuLimits {
        fn from(limits: wgpu::Limits) -> Self {
            Self {
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
                max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
                max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
            }
        }
    }

    fn checked_u32(value: usize) -> Result<u32, BrowserWebGpuError> {
        u32::try_from(value).map_err(|_| {
            BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::WorkgroupCountTooLarge,
                format!("browser GPU dispatch dimension {value} exceeds u32"),
            )
        })
    }

    fn checked_len_mul(lhs: usize, rhs: usize) -> Result<usize, BrowserWebGpuError> {
        lhs.checked_mul(rhs).ok_or_else(|| {
            BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::BufferSizeTooLarge,
                "matrix/tensor shape multiplication overflowed",
            )
        })
    }

    fn checked_f32_bytes(len: usize) -> Result<u64, BrowserWebGpuError> {
        len.checked_mul(std::mem::size_of::<f32>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| {
                BrowserWebGpuError::fallback(
                    BrowserWebGpuFallbackReason::BufferSizeTooLarge,
                    "f32 buffer byte size overflowed",
                )
            })
    }

    fn checked_usize_bytes(byte_len: u64) -> Result<usize, BrowserWebGpuError> {
        usize::try_from(byte_len).map_err(|_| {
            BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::BufferSizeTooLarge,
                "browser WebGPU buffer byte size exceeds usize",
            )
        })
    }

    fn validate_f32_storage(
        limits: BrowserWebGpuLimits,
        len: usize,
    ) -> Result<(), BrowserWebGpuError> {
        validate_byte_len(limits, checked_f32_bytes(len)?)
    }

    fn validate_byte_len(
        limits: BrowserWebGpuLimits,
        byte_len: u64,
    ) -> Result<(), BrowserWebGpuError> {
        if byte_len > limits.max_buffer_size {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::BufferSizeTooLarge,
                "browser WebGPU buffer exceeds max_buffer_size",
            ));
        }
        if byte_len > limits.max_storage_buffer_binding_size {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::StorageBufferTooLarge,
                "browser WebGPU storage buffer exceeds max_storage_buffer_binding_size",
            ));
        }
        Ok(())
    }

    fn checked_workgroups(
        limits: BrowserWebGpuLimits,
        groups: usize,
    ) -> Result<u32, BrowserWebGpuError> {
        let groups = checked_u32(groups)?;
        if groups > limits.max_compute_workgroups_per_dimension {
            return Err(BrowserWebGpuError::fallback(
                BrowserWebGpuFallbackReason::WorkgroupCountTooLarge,
                "browser WebGPU dispatch exceeds max_compute_workgroups_per_dimension",
            ));
        }
        Ok(groups)
    }

    fn add_groups(
        limits: BrowserWebGpuLimits,
        len: usize,
    ) -> Result<(u32, u32), BrowserWebGpuError> {
        let groups = len.div_ceil(256).max(1);
        let max_per_dimension = usize::try_from(limits.max_compute_workgroups_per_dimension)
            .unwrap_or(usize::MAX)
            .max(1);
        let groups_x = groups.min(max_per_dimension);
        let groups_y = groups.div_ceil(groups_x).max(1);
        Ok((
            checked_workgroups(limits, groups_x)?,
            checked_workgroups(limits, groups_y)?,
        ))
    }

    fn map_wgpu_error(error: wgpu::Error) -> BrowserWebGpuError {
        let reason = match error {
            wgpu::Error::Validation { .. } => BrowserWebGpuFallbackReason::ValidationError,
            wgpu::Error::OutOfMemory { .. } => BrowserWebGpuFallbackReason::OutOfMemory,
            wgpu::Error::Internal { .. } => BrowserWebGpuFallbackReason::InternalError,
        };
        BrowserWebGpuError::fallback(reason, error.to_string())
    }

    fn bool_property(value: &wasm_bindgen::JsValue, name: &str) -> bool {
        js_sys::Reflect::get(value, &name.into())
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    fn browser_now_ms() -> f64 {
        web_sys::window()
            .and_then(|window| window.performance())
            .map_or(0.0, |performance| performance.now())
    }

    const MATMUL_SHADER: &str = r"
struct MatrixParams {
    rows: u32,
    k_len: u32,
    cols: u32,
    pad: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> rhs: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: MatrixParams;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let col = id.x;
    let row = id.y;
    if (row >= params.rows || col >= params.cols) {
        return;
    }
    var acc = 0.0;
    for (var k = 0u; k < params.k_len; k = k + 1u) {
        acc = acc + lhs[row * params.k_len + k] * rhs[k * params.cols + col];
    }
    out[row * params.cols + col] = acc;
}
";

    const ADD_SHADER: &str = r"
struct AddParams {
    len: u32,
    x_threads: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> rhs: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: AddParams;

@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.y * params.x_threads + id.x;
    if (index >= params.len) {
        return;
    }
    out[index] = lhs[index] + rhs[index];
}
";

    const BIAS_ADD_SHADER: &str = r"
struct BiasAddParams {
    rows: u32,
    cols: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<storage, read> lhs: array<f32>;
@group(0) @binding(1) var<storage, read> bias: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<storage, read> params: BiasAddParams;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let col = id.x;
    let row = id.y;
    if (row >= params.rows || col >= params.cols) {
        return;
    }
    let index = row * params.cols + col;
    out[index] = lhs[index] + bias[col];
}
";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "math-glam", feature = "math-ndarray"))]
    #[test]
    fn scalar_ndarray_and_glam_matmul_match() {
        let lhs = DenseMatrixF32::new(4, 4, (0_u8..16).map(f32::from).collect()).unwrap();
        let rhs = DenseMatrixF32::new(
            4,
            4,
            (0_u8..16).map(|value| f32::from(value) * 0.5).collect(),
        )
        .unwrap();
        let expected = lhs.matmul_scalar(&rhs).unwrap();

        let mut scalar = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Scalar,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let mut glam = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Glam,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let mut ndarray = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Ndarray,
            ..RuntimeMathAcceleratorConfig::default()
        });

        assert_eq!(scalar.matmul_f32(&lhs, &rhs).unwrap(), expected);
        assert_eq!(glam.matmul_f32(&lhs, &rhs).unwrap(), expected);
        assert_eq!(ndarray.matmul_f32(&lhs, &rhs).unwrap(), expected);
        assert_eq!(glam.stats().glam_calls, 1);
        assert_eq!(ndarray.stats().ndarray_calls, 1);
    }

    #[test]
    fn matmul_bias_add_fuses_scalar_work_and_records_backend_stats() {
        let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
        let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Scalar,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let out = accelerator.matmul_bias_add_f32(&lhs, &rhs, &bias).unwrap();

        assert_eq!(out.values(), &[58.5, 63.75, 139.5, 153.75]);
        assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
        assert_eq!(accelerator.stats().scalar_calls, 1);
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Scalar)
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn matmul_bias_add_uses_fused_wgpu_one_shot_kernel_when_available() {
        let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
        let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
        let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let Ok(out) = accelerator.matmul_bias_add_f32(&lhs, &rhs, &bias) else {
            return;
        };

        assert_eq!(out, expected);
        assert_eq!(accelerator.stats().wgpu_calls, 1);
        assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
        assert_eq!(accelerator.stats().gpu_buffer_creations, 7);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            (lhs.values().len() + rhs.values().len() + bias.values().len())
                * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(expected.values())
        );
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Wgpu)
        );
    }

    #[cfg(all(feature = "math-glam", feature = "math-ndarray"))]
    #[test]
    fn scalar_ndarray_and_glam_f64_matmul_match_without_widening() {
        let lhs = DenseMatrixF64::new(4, 4, (0_u8..16).map(f64::from).collect()).unwrap();
        let rhs = DenseMatrixF64::new(
            4,
            4,
            (0_u8..16).map(|value| f64::from(value) * 0.5).collect(),
        )
        .unwrap();
        let expected = lhs.matmul_scalar(&rhs).unwrap();

        let mut scalar = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Scalar,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let mut glam = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Glam,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let mut ndarray = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Ndarray,
            ..RuntimeMathAcceleratorConfig::default()
        });

        assert_eq!(scalar.matmul_f64(&lhs, &rhs).unwrap(), expected);
        assert_eq!(glam.matmul_f64(&lhs, &rhs).unwrap(), expected);
        assert_eq!(ndarray.matmul_f64(&lhs, &rhs).unwrap(), expected);
        assert_eq!(glam.stats().glam_calls, 1);
        assert_eq!(ndarray.stats().ndarray_calls, 1);
    }

    #[cfg(feature = "math-ndarray")]
    #[test]
    fn tensor_add_keeps_shape_and_backend_stats() {
        let lhs = DenseTensorF32::new(vec![2, 2, 2], vec![1.0; 8]).unwrap();
        let rhs = DenseTensorF32::new(vec![2, 2, 2], vec![2.0; 8]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Ndarray,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let out = accelerator.tensor_add_f32(&lhs, &rhs).unwrap();

        assert_eq!(out.shape().dims(), &[2, 2, 2]);
        assert_eq!(out.values(), &[3.0; 8]);
        assert_eq!(accelerator.stats().ndarray_calls, 1);
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Ndarray)
        );
    }

    #[cfg(feature = "math-ndarray")]
    #[test]
    fn f64_tensor_add_keeps_shape_and_backend_stats_without_widening() {
        let lhs = DenseTensorF64::new(vec![2, 2, 2], vec![1.5; 8]).unwrap();
        let rhs = DenseTensorF64::new(vec![2, 2, 2], vec![2.25; 8]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Ndarray,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let out = accelerator.tensor_add_f64(&lhs, &rhs).unwrap();

        assert_eq!(out.shape().dims(), &[2, 2, 2]);
        assert_eq!(out.values(), &[3.75; 8]);
        assert_eq!(accelerator.stats().ndarray_calls, 1);
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Ndarray)
        );
        assert_eq!(
            accelerator.stats().bytes_borrowed,
            16 * std::mem::size_of::<f64>()
        );
    }

    #[cfg(feature = "math-ndarray")]
    #[test]
    fn auto_f64_math_stays_on_cpu_even_when_wgpu_threshold_matches() {
        let lhs = DenseMatrixF64::new(8, 8, vec![1.0; 64]).unwrap();
        let rhs = DenseMatrixF64::new(8, 8, vec![2.0; 64]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            wgpu_min_elements: 1,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let out = accelerator.matmul_f64(&lhs, &rhs).unwrap();

        assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
        assert_eq!(accelerator.stats().wgpu_calls, 0);
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Ndarray)
        );
        assert_eq!(
            accelerator.stats().last_auto_reason,
            Some(RuntimeMathAutoSelectionReason::MatmulCpuDefault)
        );
    }

    #[test]
    fn explicit_wgpu_f64_math_reports_portability_error() {
        let lhs = DenseTensorF64::new(vec![2, 2], vec![1.0; 4]).unwrap();
        let rhs = DenseTensorF64::new(vec![2, 2], vec![2.0; 4]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let error = accelerator
            .tensor_add_f64(&lhs, &rhs)
            .expect_err("portable f64 wgpu kernels are not available");

        assert!(error.to_string().contains("portable f64 tensor kernels"));
        assert_eq!(accelerator.stats().wgpu_calls, 0);
    }

    #[cfg(feature = "math-ndarray")]
    #[test]
    fn auto_small_general_matmul_prefers_cpu_backend() {
        let lhs = DenseMatrixF32::new(8, 8, (0_u8..64).map(f32::from).collect()).unwrap();
        let rhs = DenseMatrixF32::new(
            8,
            8,
            (0_u8..64).map(|value| f32::from(value) * 0.5).collect(),
        )
        .unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

        let out = accelerator.matmul_f32(&lhs, &rhs).unwrap();

        assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Ndarray)
        );
        assert_eq!(
            accelerator.stats().last_auto_reason,
            Some(RuntimeMathAutoSelectionReason::MatmulCpuDefault)
        );
        assert_eq!(accelerator.stats().wgpu_calls, 0);
    }

    #[cfg(feature = "math-glam")]
    #[test]
    fn auto_4x4_matmul_records_glam_policy_reason() {
        let lhs = DenseMatrixF32::new(4, 4, (0_u8..16).map(f32::from).collect()).unwrap();
        let rhs = DenseMatrixF32::new(
            4,
            4,
            (0_u8..16).map(|value| f32::from(value) * 0.5).collect(),
        )
        .unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

        let out = accelerator.matmul_f32(&lhs, &rhs).unwrap();

        assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Glam)
        );
        assert_eq!(
            accelerator.stats().last_auto_reason,
            Some(RuntimeMathAutoSelectionReason::Matmul4x4Glam)
        );
    }

    #[cfg(feature = "math-ndarray")]
    #[test]
    fn auto_elementwise_records_cpu_policy_reason() {
        let lhs = DenseMatrixF32::new(2, 2, vec![1.0; 4]).unwrap();
        let rhs = DenseMatrixF32::new(2, 2, vec![2.0; 4]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

        let out = accelerator.matrix_add_f32(&lhs, &rhs).unwrap();

        assert_eq!(out.values(), &[3.0; 4]);
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Ndarray)
        );
        assert_eq!(
            accelerator.stats().last_auto_reason,
            Some(RuntimeMathAutoSelectionReason::ElementwiseCpuDefault)
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn auto_large_elementwise_records_wgpu_threshold_policy_reason() {
        let lhs = DenseMatrixF32::new(4, 4, vec![1.0; 16]).unwrap();
        let rhs = DenseMatrixF32::new(4, 4, vec![2.0; 16]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            wgpu_min_elements: 1,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let Ok(out) = accelerator.matrix_add_f32(&lhs, &rhs) else {
            return;
        };

        assert_eq!(out.values(), &[3.0; 16]);
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Wgpu)
        );
        assert_eq!(
            accelerator.stats().last_auto_reason,
            Some(RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn auto_large_matmul_records_wgpu_threshold_policy_reason() {
        let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).unwrap();
        let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            wgpu_min_elements: 1,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let Ok(out) = accelerator.matmul_f32(&lhs, &rhs) else {
            return;
        };

        assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
        assert_eq!(
            accelerator.stats().last_backend,
            Some(RuntimeMathBackend::Wgpu)
        );
        assert_eq!(
            accelerator.stats().last_auto_reason,
            Some(RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_matmul_reuses_gpu_buffers_when_adapter_is_available() {
        let lhs = DenseMatrixF32::new(
            8,
            16,
            (0_u16..128)
                .map(|value| f32::from(value % 11) - 5.0)
                .collect(),
        )
        .unwrap();
        let rhs = DenseMatrixF32::new(
            16,
            4,
            (0_u16..64)
                .map(|value| (f32::from(value % 7) - 3.0) * 0.25)
                .collect(),
        )
        .unwrap();
        let expected = lhs.matmul_scalar(&rhs).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_matmul_f32(&lhs, &rhs) else {
            return;
        };

        let mut out = vec![0.0; expected.values().len()];
        accelerator
            .run_prepared_matrix_matmul_f32_into(&prepared, &mut out)
            .expect("prepared GPU matrix matmul writes into caller buffer");
        accelerator
            .run_prepared_matrix_matmul_f32_into(&prepared, &mut out)
            .expect("prepared GPU matrix matmul reuses staging buffer");

        assert_eq!(out, expected.values());
        assert_eq!(accelerator.stats().wgpu_calls, 2);
        assert_eq!(accelerator.stats().gpu_buffer_creations, 4);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 2);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 8);
        assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
        assert_eq!(accelerator.stats().gpu_staging_buffer_reuse_hits, 1);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(expected.values()) * 2
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_matmul_capacity_reuses_buffers_for_smaller_shapes() {
        let lhs = DenseMatrixF32::new(8, 8, vec![3.0; 64]).unwrap();
        let rhs = DenseMatrixF32::new(8, 8, vec![0.5; 64]).unwrap();
        let expected = lhs.matmul_scalar(&rhs).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_matmul_f32_capacity(16, 16, 16) else {
            return;
        };

        accelerator.reset_stats();
        accelerator
            .update_prepared_matrix_matmul_f32(&prepared, &lhs, &rhs)
            .expect("capacity-prepared matmul accepts smaller compatible input");
        let mut out = vec![0.0; expected.values().len()];
        accelerator
            .run_prepared_matrix_matmul_f32_shape_into(&prepared, 8, 8, &mut out)
            .expect("capacity-prepared matmul dispatches smaller output");

        assert_eq!(out, expected.values());
        assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(expected.values())
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_matmul_bias_add_reuses_gpu_buffers_when_adapter_is_available() {
        let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
        let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
        let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_matmul_bias_add_f32(&lhs, &rhs, &bias) else {
            return;
        };

        let mut out = vec![0.0; expected.values().len()];
        accelerator
            .run_prepared_matrix_matmul_bias_add_f32_into(&prepared, &mut out)
            .expect("prepared GPU matrix matmul-bias-add writes into caller buffer");
        accelerator
            .run_prepared_matrix_matmul_bias_add_f32_into(&prepared, &mut out)
            .expect("prepared GPU matrix matmul-bias-add reuses staging buffer");

        assert_eq!(out, expected.values());
        assert_eq!(accelerator.stats().wgpu_calls, 2);
        assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 2);
        assert_eq!(accelerator.stats().gpu_buffer_creations, 7);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 2);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 14);
        assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
        assert_eq!(accelerator.stats().gpu_staging_buffer_reuse_hits, 1);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            (lhs.values().len() + rhs.values().len() + bias.values().len())
                * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(expected.values()) * 2
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_matmul_submit_only_defers_readback_when_adapter_is_available() {
        let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
        let expected = lhs.matmul_scalar(&rhs).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_matmul_f32(&lhs, &rhs) else {
            return;
        };

        accelerator.reset_stats();
        accelerator
            .submit_prepared_matrix_matmul_f32_without_readback(&prepared)
            .expect("prepared GPU matmul submit can defer readback");

        assert_eq!(accelerator.stats().wgpu_calls, 1);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 4);
        assert_eq!(accelerator.stats().bytes_downloaded, 0);
        assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 0);

        let mut out = vec![0.0; expected.values().len()];
        accelerator
            .read_prepared_matrix_matmul_f32_output_into(&prepared, &mut out)
            .expect("prepared GPU matmul output can be read after submit");

        assert_eq!(out, expected.values());
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(expected.values())
        );
        assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_matmul_bias_add_submit_only_defers_readback_when_adapter_is_available() {
        let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
        let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
        let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_matmul_bias_add_f32(&lhs, &rhs, &bias) else {
            return;
        };

        accelerator.reset_stats();
        accelerator
            .submit_prepared_matrix_matmul_bias_add_f32_without_readback(&prepared)
            .expect("prepared GPU matmul-bias-add submit can defer readback");

        assert_eq!(accelerator.stats().wgpu_calls, 1);
        assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
        assert_eq!(accelerator.stats().bytes_downloaded, 0);
        assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 0);

        let mut out = vec![0.0; expected.values().len()];
        accelerator
            .read_prepared_matrix_matmul_bias_add_f32_output_into(&prepared, &mut out)
            .expect("prepared GPU matmul-bias-add output can be read after submit");

        assert_eq!(out, expected.values());
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(expected.values())
        );
        assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_matmul_bias_add_capacity_reuses_buffers_for_smaller_shapes() {
        let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
        let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
        let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_matmul_bias_add_f32_capacity(8, 8, 8) else {
            return;
        };

        accelerator.reset_stats();
        accelerator
            .update_prepared_matrix_matmul_bias_add_f32(&prepared, &lhs, &rhs, &bias)
            .expect("capacity-prepared matmul-bias-add accepts smaller compatible input");
        let mut out = vec![0.0; expected.values().len()];
        accelerator
            .run_prepared_matrix_matmul_bias_add_f32_shape_into(&prepared, 2, 2, &mut out)
            .expect("capacity-prepared matmul-bias-add dispatches smaller output");

        assert_eq!(out, expected.values());
        assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
        assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 12);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            (lhs.values().len() + rhs.values().len() + bias.values().len())
                * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(expected.values())
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn one_shot_wgpu_readback_reuses_staging_buffer_when_adapter_is_available() {
        let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).unwrap();
        let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });

        let Ok(first) = accelerator.matrix_add_f32(&lhs, &rhs) else {
            return;
        };
        let second = accelerator
            .matrix_add_f32(&lhs, &rhs)
            .expect("second wgpu matrix add uses initialized adapter");

        assert_eq!(first.values(), &[3.0; 256]);
        assert_eq!(second.values(), &[3.0; 256]);
        assert_eq!(accelerator.stats().wgpu_calls, 2);
        assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
        assert_eq!(accelerator.stats().gpu_staging_buffer_reuse_hits, 1);
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_tensor_add_reuses_gpu_buffers_when_adapter_is_available() {
        let lhs = DenseTensorF32::new(vec![1024], vec![1.0; 1024]).unwrap();
        let rhs = DenseTensorF32::new(vec![1024], vec![2.0; 1024]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_tensor_add_f32(&lhs, &rhs) else {
            return;
        };

        let out = accelerator
            .run_prepared_tensor_add_f32(&prepared)
            .expect("prepared GPU tensor add dispatches");

        assert_eq!(out.values(), vec![3.0; 1024].as_slice());
        assert_eq!(accelerator.stats().wgpu_calls, 1);
        assert_eq!(accelerator.stats().gpu_buffer_creations, 4);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 4);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            2048 * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            1024 * std::mem::size_of::<f32>()
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_tensor_add_capacity_reuses_buffers_for_smaller_lengths() {
        let lhs = DenseTensorF32::new(vec![64], vec![6.0; 64]).unwrap();
        let rhs = DenseTensorF32::new(vec![64], vec![7.0; 64]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_tensor_add_f32_capacity(128) else {
            return;
        };

        accelerator.reset_stats();
        accelerator
            .update_prepared_tensor_add_f32(&prepared, &lhs, &rhs)
            .expect("capacity-prepared tensor add accepts smaller compatible input");
        let mut out = vec![0.0; lhs.values().len()];
        accelerator
            .run_prepared_tensor_add_f32_len_into(&prepared, lhs.values().len(), &mut out)
            .expect("capacity-prepared tensor add dispatches smaller output");

        assert_eq!(out, vec![13.0; 64]);
        assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(lhs.values())
        );
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_add_can_write_into_reused_output_buffer() {
        let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).unwrap();
        let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_add_f32(&lhs, &rhs) else {
            return;
        };

        let mut out = vec![0.0; lhs.values().len()];
        accelerator
            .run_prepared_matrix_add_f32_into(&prepared, &mut out)
            .expect("prepared GPU matrix add writes into caller buffer");

        assert_eq!(out, vec![3.0; 256]);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        match accelerator
            .run_prepared_matrix_add_f32_into(&prepared, &mut out[..255])
            .unwrap_err()
        {
            RuntimeMathAcceleratorError::Math(RuntimeMathError::InvalidElementCount {
                expected,
                found,
            }) => {
                assert_eq!(expected, 256);
                assert_eq!(found, 255);
            }
            error => panic!("unexpected prepared output error: {error}"),
        }
    }

    #[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
    #[test]
    fn prepared_matrix_add_capacity_reuses_buffers_for_smaller_shapes() {
        let lhs = DenseMatrixF32::new(8, 8, vec![4.0; 64]).unwrap();
        let rhs = DenseMatrixF32::new(8, 8, vec![5.0; 64]).unwrap();
        let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
            backend: RuntimeMathBackend::Wgpu,
            ..RuntimeMathAcceleratorConfig::default()
        });
        let Ok(prepared) = accelerator.prepare_matrix_add_f32_capacity(16, 16) else {
            return;
        };

        accelerator.reset_stats();
        accelerator
            .update_prepared_matrix_add_f32(&prepared, &lhs, &rhs)
            .expect("capacity-prepared matrix add accepts smaller compatible input");
        let mut out = vec![0.0; lhs.values().len()];
        accelerator
            .run_prepared_matrix_add_f32_shape_into(&prepared, 8, 8, &mut out)
            .expect("capacity-prepared matrix add dispatches smaller output");

        assert_eq!(out, vec![9.0; 64]);
        assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
        assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
        assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
        assert_eq!(
            accelerator.stats().bytes_uploaded,
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().bytes_downloaded,
            std::mem::size_of_val(lhs.values())
        );
    }

    #[test]
    fn browser_webgpu_auto_policy_selects_matmul_modes_by_work_size() {
        use browser_webgpu_policy::{
            BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
        };

        let policy = BrowserWebGpuMathAutoPolicy::default();
        let limits = large_browser_webgpu_limits();

        let small = policy.select_matmul_f32(64, 64, 64, limits);
        assert_eq!(small.mode(), BrowserWebGpuMathMode::CpuWasm);
        assert_eq!(
            small.reason(),
            BrowserWebGpuMathAutoReason::MatmulCpuDefault
        );
        assert_eq!(small.capacity(), None);

        let exact = policy.select_matmul_f32(128, 128, 128, limits);
        assert_eq!(
            exact.mode(),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
        );
        assert_eq!(
            exact.reason(),
            BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined
        );
        let exact_capacity = exact
            .capacity()
            .expect("exact prepared matmul records exact capacity");
        assert_eq!(exact_capacity.rows, 128);
        assert_eq!(exact_capacity.shared, 128);
        assert_eq!(exact_capacity.cols, 128);

        let exact_256 = policy.select_matmul_f32(256, 256, 256, limits);
        assert_eq!(
            exact_256.mode(),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
        );
        assert_eq!(
            exact_256.reason(),
            BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined
        );
        let exact_256_capacity = exact_256
            .capacity()
            .expect("256 prepared matmul records exact capacity");
        assert_eq!(exact_256_capacity.rows, 256);
        assert_eq!(exact_256_capacity.shared, 256);
        assert_eq!(exact_256_capacity.cols, 256);

        let exact_512 = policy.select_matmul_f32(512, 512, 512, limits);
        assert_eq!(
            exact_512.mode(),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
        );
        assert_eq!(
            exact_512.reason(),
            BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined
        );
        let exact_512_capacity = exact_512
            .capacity()
            .expect("512 prepared matmul records exact capacity");
        assert_eq!(exact_512_capacity.rows, 512);
        assert_eq!(exact_512_capacity.shared, 512);
        assert_eq!(exact_512_capacity.cols, 512);

        let capacity_policy = BrowserWebGpuMathAutoPolicy {
            matmul_capacity_min_elements: 512 * 512 * 512,
            ..BrowserWebGpuMathAutoPolicy::default()
        };
        let grown = capacity_policy.select_matmul_f32(512, 512, 512, limits);
        assert_eq!(
            grown.mode(),
            BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined
        );
        assert_eq!(
            grown.reason(),
            BrowserWebGpuMathAutoReason::MatmulPreparedCapacityResidentPipelined
        );
        let grown_capacity = grown
            .capacity()
            .expect("capacity-prepared matmul records grown capacity");
        assert_eq!(grown_capacity.rows, 1024);
        assert_eq!(grown_capacity.shared, 1024);
        assert_eq!(grown_capacity.cols, 1024);
    }

    #[test]
    fn browser_webgpu_auto_policy_keeps_elementwise_cpu_by_default() {
        use browser_webgpu_policy::{
            BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
        };

        let policy = BrowserWebGpuMathAutoPolicy::default();
        let limits = large_browser_webgpu_limits();

        let selection = policy.select_elementwise_f32(4 * 1024 * 1024, limits);
        assert_eq!(selection.mode(), BrowserWebGpuMathMode::CpuWasm);
        assert_eq!(
            selection.reason(),
            BrowserWebGpuMathAutoReason::ElementwiseCpuReadbackDominated
        );

        let gpu_policy = BrowserWebGpuMathAutoPolicy {
            elementwise_gpu_min_elements: 1024,
            ..BrowserWebGpuMathAutoPolicy::default()
        };
        let gpu_selection = gpu_policy.select_elementwise_f32(1024, limits);
        assert_eq!(
            gpu_selection.mode(),
            BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
        );
        assert_eq!(
            gpu_selection.reason(),
            BrowserWebGpuMathAutoReason::ElementwisePreparedResidentPipelined
        );
    }

    #[test]
    fn browser_webgpu_auto_policy_falls_back_on_storage_limits() {
        use browser_webgpu_policy::{
            BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
        };

        let limits = browser_webgpu_policy::BrowserWebGpuLimits {
            max_storage_buffer_binding_size: 1024,
            max_buffer_size: 1024,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroups_per_dimension: 65_535,
        };
        let policy = BrowserWebGpuMathAutoPolicy {
            elementwise_gpu_min_elements: 1024,
            ..BrowserWebGpuMathAutoPolicy::default()
        };

        let matmul = policy.select_matmul_f32(256, 256, 256, limits);
        assert_eq!(matmul.mode(), BrowserWebGpuMathMode::CpuWasm);
        assert_eq!(matmul.reason(), BrowserWebGpuMathAutoReason::StorageLimit);

        let elementwise = policy.select_elementwise_f32(1024, limits);
        assert_eq!(elementwise.mode(), BrowserWebGpuMathMode::CpuWasm);
        assert_eq!(
            elementwise.reason(),
            BrowserWebGpuMathAutoReason::StorageLimit
        );
    }

    const fn large_browser_webgpu_limits() -> browser_webgpu_policy::BrowserWebGpuLimits {
        browser_webgpu_policy::BrowserWebGpuLimits {
            max_storage_buffer_binding_size: 1_u64 << 34,
            max_buffer_size: 1_u64 << 34,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroups_per_dimension: 65_535,
        }
    }
}
