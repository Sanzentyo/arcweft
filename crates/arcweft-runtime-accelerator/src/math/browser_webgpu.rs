mod auto_adapter;
mod context;

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
