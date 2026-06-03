use arcweft_core::task::SystemInfoKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct HostSystemInfo {
    pub physical_cores: usize,
    pub logical_threads: usize,
    pub available_parallelism: usize,
}

pub fn host_system_info() -> HostSystemInfo {
    HostSystemInfo {
        physical_cores: num_cpus::get_physical().max(1),
        logical_threads: num_cpus::get().max(1),
        available_parallelism: std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get),
    }
}

pub fn system_info_value(info: HostSystemInfo, kind: SystemInfoKind) -> usize {
    match kind {
        SystemInfoKind::CoreCount => info.physical_cores,
        SystemInfoKind::ThreadCount => info.logical_threads,
        SystemInfoKind::AvailableParallelism => info.available_parallelism,
    }
}
