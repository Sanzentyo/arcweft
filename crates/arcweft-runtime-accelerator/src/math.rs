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
    #[cfg(feature = "math-wgpu")]
    wgpu: Option<wgpu_backend::WgpuMathContext>,
}

/// Prepared GPU storage for repeatedly adding the same `f32` matrices.
pub struct RuntimePreparedMatrixAddF32 {
    rows: usize,
    cols: usize,
    #[cfg(feature = "math-wgpu")]
    gpu: wgpu_backend::PreparedAddBuffers,
}

/// Prepared GPU storage for repeatedly multiplying the same `f32` matrices.
pub struct RuntimePreparedMatrixMatmulF32 {
    rows: usize,
    shared: usize,
    cols: usize,
    #[cfg(feature = "math-wgpu")]
    gpu: wgpu_backend::PreparedMatmulBuffers,
}

/// Prepared GPU storage for repeatedly adding the same `f32` tensors.
pub struct RuntimePreparedTensorAddF32 {
    dims: Vec<usize>,
    #[cfg(feature = "math-wgpu")]
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
            #[cfg(feature = "math-wgpu")]
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
        let selection = self.select_matmul_backend(lhs, rhs);
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
        let selection = self.select_elementwise_backend(lhs.values().len());
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
        let selection = self.select_elementwise_backend(lhs.values().len());
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
        #[cfg(feature = "math-wgpu")]
        {
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
        #[cfg(not(feature = "math-wgpu"))]
        {
            let _ = (lhs, rhs);
            Err(RuntimeMathAcceleratorError::Backend(
                "math-wgpu feature is disabled".to_owned(),
            ))
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
        #[cfg(feature = "math-wgpu")]
        {
            let readback = self
                .wgpu_context()
                .and_then(|context| context.dispatch_prepared_add(&prepared.gpu, out))?;
            self.record_prepared_gpu_dispatch(prepared.gpu.len(), readback);
            Ok(())
        }
        #[cfg(not(feature = "math-wgpu"))]
        {
            let _ = prepared;
            Err(RuntimeMathAcceleratorError::Backend(
                "math-wgpu feature is disabled".to_owned(),
            ))
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
        #[cfg(feature = "math-wgpu")]
        {
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
        #[cfg(not(feature = "math-wgpu"))]
        {
            let _ = (lhs, rhs);
            Err(RuntimeMathAcceleratorError::Backend(
                "math-wgpu feature is disabled".to_owned(),
            ))
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
        #[cfg(feature = "math-wgpu")]
        {
            let readback = self
                .wgpu_context()
                .and_then(|context| context.dispatch_prepared_matmul(&prepared.gpu, out))?;
            self.record_prepared_gpu_dispatch(prepared.gpu.len(), readback);
            Ok(())
        }
        #[cfg(not(feature = "math-wgpu"))]
        {
            let _ = prepared;
            Err(RuntimeMathAcceleratorError::Backend(
                "math-wgpu feature is disabled".to_owned(),
            ))
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
        #[cfg(feature = "math-wgpu")]
        {
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
        #[cfg(not(feature = "math-wgpu"))]
        {
            let _ = (lhs, rhs);
            Err(RuntimeMathAcceleratorError::Backend(
                "math-wgpu feature is disabled".to_owned(),
            ))
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
        #[cfg(feature = "math-wgpu")]
        {
            let readback = self
                .wgpu_context()
                .and_then(|context| context.dispatch_prepared_add(&prepared.gpu, out))?;
            self.record_prepared_gpu_dispatch(prepared.gpu.len(), readback);
            Ok(())
        }
        #[cfg(not(feature = "math-wgpu"))]
        {
            let _ = prepared;
            Err(RuntimeMathAcceleratorError::Backend(
                "math-wgpu feature is disabled".to_owned(),
            ))
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
        #[cfg(feature = "math-wgpu")]
        {
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
        #[cfg(not(feature = "math-wgpu"))]
        {
            if self.config.backend == RuntimeMathBackend::Wgpu {
                return Err(RuntimeMathAcceleratorError::Backend(
                    "math-wgpu feature is disabled".to_owned(),
                ));
            }
            self.stats.fallback_calls += 1;
        }
        self.matmul_ndarray(lhs, rhs)
    }

    fn matrix_add_wgpu(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-wgpu")]
        {
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
        #[cfg(not(feature = "math-wgpu"))]
        {
            if self.config.backend == RuntimeMathBackend::Wgpu {
                return Err(RuntimeMathAcceleratorError::Backend(
                    "math-wgpu feature is disabled".to_owned(),
                ));
            }
            self.stats.fallback_calls += 1;
        }
        self.matrix_add_ndarray(lhs, rhs)
    }

    fn tensor_add_wgpu(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeMathAcceleratorError> {
        #[cfg(feature = "math-wgpu")]
        {
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
        #[cfg(not(feature = "math-wgpu"))]
        {
            if self.config.backend == RuntimeMathBackend::Wgpu {
                return Err(RuntimeMathAcceleratorError::Backend(
                    "math-wgpu feature is disabled".to_owned(),
                ));
            }
            self.stats.fallback_calls += 1;
        }
        self.tensor_add_ndarray(lhs, rhs)
    }

    fn record_matrix_inputs<T>(&mut self, lhs: &DenseMatrix<T>, rhs: &DenseMatrix<T>) {
        self.stats.bytes_borrowed +=
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<T>();
    }

    fn record_tensor_inputs<T>(&mut self, lhs: &DenseTensor<T>, rhs: &DenseTensor<T>) {
        self.stats.bytes_borrowed +=
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<T>();
    }

    #[cfg(feature = "math-wgpu")]
    fn record_gpu_transfer(
        &mut self,
        uploaded_elements: usize,
        downloaded_elements: usize,
        readback: wgpu_backend::GpuReadbackUsage,
    ) {
        let uploaded = uploaded_elements * std::mem::size_of::<f32>();
        let downloaded = downloaded_elements * std::mem::size_of::<f32>();
        self.stats.bytes_uploaded += uploaded;
        self.stats.bytes_downloaded += downloaded;
        self.stats.bytes_copied += uploaded + downloaded;
        self.stats.gpu_buffer_creations += 4;
        self.record_gpu_readback(readback);
    }

    #[cfg(feature = "math-wgpu")]
    fn record_prepared_gpu_dispatch(
        &mut self,
        downloaded_elements: usize,
        readback: wgpu_backend::GpuReadbackUsage,
    ) {
        let downloaded = downloaded_elements * std::mem::size_of::<f32>();
        self.stats.wgpu_calls += 1;
        self.stats.last_backend = Some(RuntimeMathBackend::Wgpu);
        self.stats.bytes_downloaded += downloaded;
        self.stats.bytes_copied += downloaded;
        self.stats.gpu_buffer_reuse_hits += 4;
        self.stats.gpu_reused_dispatches += 1;
        self.record_gpu_readback(readback);
    }

    #[cfg(feature = "math-wgpu")]
    fn record_gpu_readback(&mut self, readback: wgpu_backend::GpuReadbackUsage) {
        self.stats.gpu_staging_buffer_creations += usize::from(readback.created);
        self.stats.gpu_staging_buffer_reuse_hits += usize::from(readback.reused);
    }

    #[cfg(feature = "math-wgpu")]
    fn wgpu_context(
        &mut self,
    ) -> Result<&mut wgpu_backend::WgpuMathContext, RuntimeMathAcceleratorError> {
        if self.wgpu.is_none() {
            self.wgpu = Some(wgpu_backend::WgpuMathContext::new()?);
        }
        Ok(self.wgpu.as_mut().expect("wgpu context was initialized"))
    }
}

fn row_major_4x4_to_cols<T: Copy>(values: &[T]) -> [T; 16] {
    [
        values[0], values[4], values[8], values[12], values[1], values[5], values[9], values[13],
        values[2], values[6], values[10], values[14], values[3], values[7], values[11], values[15],
    ]
}

fn cols_4x4_to_row_major<T: Copy>(values: [T; 16]) -> Vec<T> {
    vec![
        values[0], values[4], values[8], values[12], values[1], values[5], values[9], values[13],
        values[2], values[6], values[10], values[14], values[3], values[7], values[11], values[15],
    ]
}

fn matmul_work_items<T>(lhs: &DenseMatrix<T>, rhs: &DenseMatrix<T>) -> usize {
    lhs.rows()
        .saturating_mul(lhs.cols())
        .saturating_mul(rhs.cols())
}

const fn wgpu_backend_enabled() -> bool {
    cfg!(feature = "math-wgpu")
}

#[derive(Debug, Error)]
pub enum RuntimeMathAcceleratorError {
    #[error(transparent)]
    Math(#[from] RuntimeMathError),
    #[error("runtime math accelerator backend error: {0}")]
    Backend(String),
}

#[cfg(feature = "math-wgpu")]
mod wgpu_backend {
    use super::{DenseMatrixF32, DenseTensorF32, RuntimeMathAcceleratorError};
    use bytemuck::{Pod, Zeroable};
    use std::sync::mpsc;
    use wgpu::util::DeviceExt;

    pub struct WgpuMathContext {
        device: wgpu::Device,
        queue: wgpu::Queue,
        matmul_pipeline: wgpu::ComputePipeline,
        add_pipeline: wgpu::ComputePipeline,
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
        _lhs: wgpu::Buffer,
        _rhs: wgpu::Buffer,
        out: wgpu::Buffer,
        _params: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
    }

    impl PreparedAddBuffers {
        pub const fn len(&self) -> usize {
            self.len
        }
    }

    pub struct PreparedMatmulBuffers {
        rows: usize,
        cols: usize,
        _lhs: wgpu::Buffer,
        _rhs: wgpu::Buffer,
        out: wgpu::Buffer,
        _params: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
    }

    impl PreparedMatmulBuffers {
        pub const fn len(&self) -> usize {
            self.rows * self.cols
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
            Ok(Self {
                device,
                queue,
                matmul_pipeline,
                add_pipeline,
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
                _lhs: lhs,
                _rhs: rhs,
                out,
                _params: params,
                bind_group,
            })
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
                cols,
                _lhs: lhs,
                _rhs: rhs,
                out,
                _params: params,
                bind_group,
            })
        }

        pub fn dispatch_prepared_add(
            &mut self,
            prepared: &PreparedAddBuffers,
            out: &mut [f32],
        ) -> Result<GpuReadbackUsage, RuntimeMathAcceleratorError> {
            if out.len() != prepared.len {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared add output expected {} value(s), got {}",
                    prepared.len,
                    out.len()
                )));
            }
            if out.is_empty() {
                return Ok(GpuReadbackUsage::default());
            }
            let (groups_x, groups_y) = add_groups(prepared.len);
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
            if out.len() != prepared.len() {
                return Err(RuntimeMathAcceleratorError::Backend(format!(
                    "prepared matmul output expected {} value(s), got {}",
                    prepared.len(),
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
                    checked_u32(prepared.cols.div_ceil(16))?,
                    checked_u32(prepared.rows.div_ceil(16))?,
                    1,
                );
            }
            let readback = self.encode_readback(&mut encoder, &prepared.out, out.len());
            self.queue.submit(Some(encoder.finish()));
            self.read_staging_buffer(out)?;
            Ok(readback)
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
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[cfg(feature = "math-wgpu")]
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

    #[cfg(feature = "math-wgpu")]
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

    #[cfg(feature = "math-wgpu")]
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

    #[cfg(feature = "math-wgpu")]
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

    #[cfg(feature = "math-wgpu")]
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

    #[cfg(feature = "math-wgpu")]
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
}
