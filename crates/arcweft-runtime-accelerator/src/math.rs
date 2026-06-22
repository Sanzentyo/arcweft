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

impl RuntimeMathBackend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Scalar => "scalar",
            Self::Glam => "glam",
            Self::Ndarray => "ndarray",
            Self::Wgpu => "wgpu",
        }
    }
}

/// Reason recorded when `Auto` chooses a concrete math backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMathAutoSelectionReason {
    Matmul4x4Glam,
    MatmulWgpuWorkThreshold,
    MatmulScalarSmallWork,
    MatmulNdarrayCpuDefault,
    ElementwiseWgpuWorkThreshold,
    ElementwiseScalarCpuDefault,
    ElementwiseNdarrayCpuDefault,
}

impl RuntimeMathAutoSelectionReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Matmul4x4Glam => "matmul_4x4_glam",
            Self::MatmulWgpuWorkThreshold => "matmul_wgpu_work_threshold",
            Self::MatmulScalarSmallWork => "matmul_scalar_small_work",
            Self::MatmulNdarrayCpuDefault => "matmul_ndarray_cpu_default",
            Self::ElementwiseWgpuWorkThreshold => "elementwise_wgpu_work_threshold",
            Self::ElementwiseScalarCpuDefault => "elementwise_scalar_cpu_default",
            Self::ElementwiseNdarrayCpuDefault => "elementwise_ndarray_cpu_default",
        }
    }
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

    pub fn tensor_elementwise_backend_selection(
        &self,
        elements: usize,
    ) -> RuntimeMathBackendSelection {
        self.select_tensor_elementwise_backend(elements)
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
        let selection = self.select_tensor_elementwise_backend(lhs.values().len());
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
            RuntimeMathBackend::Scalar => self.tensor_add_scalar(lhs, rhs),
            RuntimeMathBackend::Glam => self.tensor_add_glam_f64(lhs, rhs),
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

    pub fn submit_prepared_matrix_add_f32_without_readback(
        &mut self,
        prepared: &RuntimePreparedMatrixAddF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.submit_prepared_matrix_add_f32_shape_without_readback(
            prepared,
            prepared.rows,
            prepared.cols,
        )
    }

    pub fn submit_prepared_matrix_add_f32_shape_without_readback(
        &mut self,
        prepared: &RuntimePreparedMatrixAddF32,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if rows > prepared.rows || cols > prepared.cols {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared matrix add capacity is {}x{}, got {}x{}",
                prepared.rows, prepared.cols, rows, cols
            )));
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                let len = rows.saturating_mul(cols);
                self.wgpu_context()
                    .and_then(|context| {
                        context.submit_prepared_add_without_readback(&prepared.gpu, len)
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

    pub fn read_prepared_matrix_add_f32_output_into(
        &mut self,
        prepared: &RuntimePreparedMatrixAddF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.read_prepared_matrix_add_f32_shape_output_into(
            prepared,
            prepared.rows,
            prepared.cols,
            out,
        )
    }

    pub fn read_prepared_matrix_add_f32_shape_output_into(
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
                    .and_then(|context| context.read_prepared_add_output(&prepared.gpu, out))?;
                self.record_prepared_gpu_readback(out.len(), readback);
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

    pub fn submit_prepared_tensor_add_f32_without_readback(
        &mut self,
        prepared: &RuntimePreparedTensorAddF32,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.submit_prepared_tensor_add_f32_len_without_readback(prepared, prepared.element_count())
    }

    pub fn submit_prepared_tensor_add_f32_len_without_readback(
        &mut self,
        prepared: &RuntimePreparedTensorAddF32,
        len: usize,
    ) -> Result<(), RuntimeMathAcceleratorError> {
        if len > prepared.element_count() {
            return Err(RuntimeMathAcceleratorError::Backend(format!(
                "prepared tensor add capacity is {}, got {}",
                prepared.element_count(),
                len
            )));
        }
        cfg_select! {
            all(feature = "math-wgpu", not(target_arch = "wasm32")) => {
                self.wgpu_context()
                    .and_then(|context| {
                        context.submit_prepared_add_without_readback(&prepared.gpu, len)
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

    pub fn read_prepared_tensor_add_f32_output_into(
        &mut self,
        prepared: &RuntimePreparedTensorAddF32,
        out: &mut [f32],
    ) -> Result<(), RuntimeMathAcceleratorError> {
        self.read_prepared_tensor_add_f32_len_output_into(prepared, prepared.element_count(), out)
    }

    pub fn read_prepared_tensor_add_f32_len_output_into(
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
                    .and_then(|context| context.read_prepared_add_output(&prepared.gpu, out))?;
                self.record_prepared_gpu_readback(out.len(), readback);
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
            RuntimeMathBackend::Auto
                if matmul_work_items(lhs, rhs) <= F32_SCALAR_MATMUL_MAX_WORK_ITEMS =>
            {
                RuntimeMathBackendSelection {
                    backend: RuntimeMathBackend::Scalar,
                    auto_reason: Some(RuntimeMathAutoSelectionReason::MatmulScalarSmallWork),
                }
            }
            RuntimeMathBackend::Auto => RuntimeMathBackendSelection {
                backend: RuntimeMathBackend::Ndarray,
                auto_reason: Some(RuntimeMathAutoSelectionReason::MatmulNdarrayCpuDefault),
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
            RuntimeMathBackend::Auto
                if matmul_work_items(lhs, rhs) <= F64_SCALAR_MATMUL_MAX_WORK_ITEMS =>
            {
                RuntimeMathBackendSelection {
                    backend: RuntimeMathBackend::Scalar,
                    auto_reason: Some(RuntimeMathAutoSelectionReason::MatmulScalarSmallWork),
                }
            }
            RuntimeMathBackend::Auto => RuntimeMathBackendSelection {
                backend: RuntimeMathBackend::Ndarray,
                auto_reason: Some(RuntimeMathAutoSelectionReason::MatmulNdarrayCpuDefault),
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
                backend: RuntimeMathBackend::Scalar,
                auto_reason: Some(RuntimeMathAutoSelectionReason::ElementwiseScalarCpuDefault),
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
                auto_reason: Some(RuntimeMathAutoSelectionReason::ElementwiseNdarrayCpuDefault),
            },
            backend => RuntimeMathBackendSelection {
                backend,
                auto_reason: None,
            },
        }
    }

    fn select_tensor_elementwise_backend(&self, elements: usize) -> RuntimeMathBackendSelection {
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
                auto_reason: Some(RuntimeMathAutoSelectionReason::ElementwiseNdarrayCpuDefault),
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

    fn tensor_add_glam_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeMathAcceleratorError> {
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

const F32_SCALAR_MATMUL_MAX_WORK_ITEMS: usize = 64 * 64 * 64;
const F64_SCALAR_MATMUL_MAX_WORK_ITEMS: usize = 64 * 64 * 64;

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
mod wgpu_backend;

/// Target-independent Browser WebGPU math auto-selection policy.
///
/// The policy is separated from the wasm adapter so benchmark and native tests
/// can validate browser Auto thresholds without linking browser-only APIs.
pub mod browser_webgpu_policy;

/// Browser WebGPU math adapter for `wasm32` players.
///
/// Browser WebGPU is asynchronous, so this adapter intentionally does not
/// implement the synchronous runtime math backend. Browser players can await
/// these calls at their adapter boundary and feed the resulting deterministic
/// dense values back into the VM.
#[cfg(all(feature = "math-wgpu", target_arch = "wasm32"))]
pub mod browser_webgpu;

#[cfg(test)]
mod tests;
