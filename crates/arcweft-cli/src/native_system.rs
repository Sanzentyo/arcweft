use arcweft_core::task::SystemInfoKind;

pub(crate) fn system_info_value(kind: SystemInfoKind) -> usize {
    match kind {
        SystemInfoKind::CoreCount => num_cpus::get_physical().max(1),
        SystemInfoKind::ThreadCount => num_cpus::get().max(1),
        SystemInfoKind::AvailableParallelism => {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        }
    }
}
