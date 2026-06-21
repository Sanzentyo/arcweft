use arcweft_runtime_accelerator::math::browser_webgpu::{
    BrowserMatmulCapacity, BrowserWebGpuCapacityGrowth,
};

use crate::model::{BrowserBenchMode, BrowserBenchModeOrder, BrowserMathBenchConfig, MatmulShape};

pub(crate) fn ordered_modes(
    config: &BrowserMathBenchConfig,
    round_index: usize,
) -> Vec<BrowserBenchMode> {
    let mut modes = config.modes.clone();
    if modes.is_empty() {
        return modes;
    }
    match config.mode_order {
        BrowserBenchModeOrder::AsListed => modes,
        BrowserBenchModeOrder::RotateByRound => {
            let offset = round_index % modes.len();
            modes.rotate_left(offset);
            modes
        }
    }
}

pub(crate) fn async_batch_depth(mode: BrowserBenchMode, config: &BrowserMathBenchConfig) -> usize {
    if matches!(
        mode,
        BrowserBenchMode::AutoPipelined
            | BrowserBenchMode::AutoResidentPipelined
            | BrowserBenchMode::AutoResidentDirectPipelined
            | BrowserBenchMode::WebGpuPreparedResidentPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined
            | BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined
    ) {
        config.async_batch_depth.max(1)
    } else {
        1
    }
}

pub(crate) fn elementwise_capacity_len(mode: BrowserBenchMode, len: usize) -> usize {
    if uses_overcapacity(mode) {
        overcapacity_len(len)
    } else {
        len
    }
}

pub(crate) fn matmul_capacity(mode: BrowserBenchMode, shape: MatmulShape) -> BrowserMatmulCapacity {
    if uses_overcapacity(mode) {
        BrowserMatmulCapacity {
            rows: overcapacity_len(shape.rows),
            shared: overcapacity_len(shape.shared),
            cols: overcapacity_len(shape.cols),
        }
    } else {
        BrowserMatmulCapacity {
            rows: shape.rows,
            shared: shape.shared,
            cols: shape.cols,
        }
    }
}

const fn uses_overcapacity(mode: BrowserBenchMode) -> bool {
    matches!(
        mode,
        BrowserBenchMode::WebGpuPreparedCapacityResident
            | BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined
    )
}

fn overcapacity_len(len: usize) -> usize {
    BrowserWebGpuCapacityGrowth::Double.grow(len)
}
