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
        Self::conservative()
    }
}

impl BrowserWebGpuMathAutoPolicy {
    /// Conservative browser policy used by default Auto selection.
    ///
    /// This keeps elementwise work on CPU Wasm and selects exact prepared
    /// resident WebGPU only for matmul work large enough to have benchmark
    /// evidence independent of one local machine.
    pub const fn conservative() -> Self {
        Self {
            matmul_exact_min_elements: 128 * 128 * 128,
            matmul_capacity_min_elements: usize::MAX,
            elementwise_gpu_min_elements: usize::MAX,
            capacity_growth: BrowserWebGpuCapacityGrowth::Double,
        }
    }

    /// Explicit CPU policy for replay, diagnostics, and product profiles
    /// that must avoid browser GPU dispatch.
    pub const fn cpu_only() -> Self {
        Self {
            matmul_exact_min_elements: usize::MAX,
            matmul_capacity_min_elements: usize::MAX,
            elementwise_gpu_min_elements: usize::MAX,
            capacity_growth: BrowserWebGpuCapacityGrowth::Exact,
        }
    }

    /// Explicit exact-resident WebGPU policy.
    ///
    /// This is intended for profile-controlled runs and harnesses. It does
    /// not bypass shape/limit validation; oversized work still selects CPU
    /// with a structured storage-limit reason.
    pub const fn explicit_webgpu_resident() -> Self {
        Self {
            matmul_exact_min_elements: 0,
            matmul_capacity_min_elements: usize::MAX,
            elementwise_gpu_min_elements: 0,
            capacity_growth: BrowserWebGpuCapacityGrowth::Exact,
        }
    }

    /// Bench harness policy for probing overprovisioned resident matmul.
    ///
    /// This is not the default Auto policy. Use it to collect path-free
    /// evidence before changing a product or runtime profile.
    pub const fn harness_capacity_matmul(
        matmul_min_elements: usize,
        capacity_growth: BrowserWebGpuCapacityGrowth,
    ) -> Self {
        Self {
            matmul_exact_min_elements: usize::MAX,
            matmul_capacity_min_elements: matmul_min_elements,
            elementwise_gpu_min_elements: usize::MAX,
            capacity_growth,
        }
    }

    #[must_use]
    pub const fn with_matmul_exact_min_elements(mut self, elements: usize) -> Self {
        self.matmul_exact_min_elements = elements;
        self
    }

    #[must_use]
    pub const fn with_elementwise_gpu_min_elements(mut self, elements: usize) -> Self {
        self.elementwise_gpu_min_elements = elements;
        self
    }

    #[must_use]
    pub const fn with_capacity_growth(mut self, growth: BrowserWebGpuCapacityGrowth) -> Self {
        self.capacity_growth = growth;
        self
    }

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
                    reason: BrowserWebGpuMathAutoReason::MatmulPreparedCapacityResidentPipelined,
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
