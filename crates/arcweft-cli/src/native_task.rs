use crate::native_system::{HostSystemInfo, host_system_info, system_info_value};
use arcweft_adapter_context::{manifest::AdapterManifest, standard};
use arcweft_core::task::{
    HostTaskRequest, LogicalEpoch, SchedulerBudget, TaskEvent, TaskEventKind, TaskSequence,
    TaskSpec,
};
use arcweft_runtime_scheduler::{RuntimeScheduler, RuntimeSchedulerStats, TaskClassCounts};
use rayon::prelude::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug)]
pub(crate) struct NativeTaskBridge {
    io_root: PathBuf,
    host_system: HostSystemInfo,
    host_calls: NativeHostCallSet,
    sequence: u64,
    scheduler: RuntimeScheduler,
    stats: NativeTaskStats,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeHostCallSet {
    ids: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub(crate) struct NativeTaskStats {
    pub(crate) completed_tasks: usize,
    pub(crate) failed_tasks: usize,
    pub(crate) read_ops: usize,
    pub(crate) write_ops: usize,
    pub(crate) system_info_ops: usize,
    pub(crate) bytes_read: usize,
    pub(crate) bytes_written: usize,
    pub(crate) parallel_batches: usize,
    pub(crate) parallel_tasks: usize,
    pub(crate) parallel_io_tasks: usize,
    pub(crate) parallel_system_info_tasks: usize,
    pub(crate) parallel_marker_tasks: usize,
    pub(crate) parallel_workers: usize,
    pub(crate) scheduler_submit_elapsed_ns: u128,
    pub(crate) scheduler_dispatch_elapsed_ns: u128,
    pub(crate) host_complete_elapsed_ns: u128,
    pub(crate) event_build_elapsed_ns: u128,
    pub(crate) scheduler_complete_elapsed_ns: u128,
    pub(crate) scheduler: NativeSchedulerStats,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub(crate) struct NativeSchedulerStats {
    pub(crate) submitted: usize,
    pub(crate) joined: usize,
    pub(crate) dispatched: usize,
    pub(crate) completed: usize,
    pub(crate) failed: usize,
    pub(crate) cancelled: usize,
    pub(crate) cancel_requested: usize,
    pub(crate) joined_completed: usize,
    pub(crate) in_flight: usize,
    pub(crate) max_in_flight: usize,
    pub(crate) dispatch_sorts: usize,
    pub(crate) dispatch_sort_items: usize,
    pub(crate) completion_sorts: usize,
    pub(crate) completion_sort_items: usize,
    pub(crate) completion_normalization_passes: usize,
    pub(crate) completion_normalization_checks: usize,
    pub(crate) completion_events_in: usize,
    pub(crate) completion_events_joined: usize,
    pub(crate) completion_events_out: usize,
    pub(crate) completion_sort_skipped_items: usize,
    pub(crate) completion_sort_performed_items: usize,
    pub(crate) joined_completion_events_emitted: usize,
    pub(crate) submitted_by_class: NativeTaskClassCounts,
    pub(crate) dispatched_by_class: NativeTaskClassCounts,
    pub(crate) completed_by_class: NativeTaskClassCounts,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub(crate) struct NativeTaskClassCounts {
    pub(crate) local_ui: usize,
    pub(crate) io: usize,
    pub(crate) cpu: usize,
    pub(crate) gpu_prepare: usize,
    pub(crate) shader_compile: usize,
    pub(crate) wasm_call: usize,
    pub(crate) asset_decode: usize,
    pub(crate) audio_decode: usize,
    pub(crate) audio_render: usize,
    pub(crate) tts_synthesis: usize,
    pub(crate) bgm_precompose: usize,
    pub(crate) lsp: usize,
    pub(crate) background: usize,
}

impl NativeTaskBridge {
    pub(crate) fn new(source_path: &Path, host_calls: NativeHostCallSet) -> Self {
        let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
        Self {
            io_root: source_dir.join(".arcweft"),
            host_system: host_system_info(),
            host_calls,
            sequence: 0,
            scheduler: RuntimeScheduler::default(),
            stats: NativeTaskStats::default(),
        }
    }

    pub(crate) fn stats(&self) -> NativeTaskStats {
        let mut stats = self.stats;
        stats.scheduler = NativeSchedulerStats::from(self.scheduler.stats());
        stats
    }

    pub(crate) fn read_text_snapshot(source_path: &Path, value: &str) -> Result<String, String> {
        let bridge = Self::new(source_path, NativeHostCallSet::standard_cli());
        virtual_path(&bridge.io_root, value)
            .and_then(|path| fs::read_to_string(path).map_err(|error| error.to_string()))
    }

    pub(crate) fn complete_tasks(&mut self, tasks: Vec<TaskSpec>) -> Vec<TaskEvent> {
        let (unauthorized, tasks): (Vec<_>, Vec<_>) = tasks
            .into_iter()
            .partition(|task| !self.host_calls.allows_task(task));
        let unauthorized_events = unauthorized
            .into_iter()
            .map(|task| self.rejected_task_event(task))
            .collect::<Vec<_>>();

        let started = Instant::now();
        self.scheduler
            .submit(tasks.into_iter().filter(can_complete_task));
        self.stats.scheduler_submit_elapsed_ns = self
            .stats
            .scheduler_submit_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        let started = Instant::now();
        let dispatch = self.scheduler.dispatch(SchedulerBudget {
            max_events: usize::MAX,
        });
        self.stats.scheduler_dispatch_elapsed_ns = self
            .stats
            .scheduler_dispatch_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        let started = Instant::now();
        let completions =
            complete_dispatched_tasks(&self.io_root, self.host_system, &dispatch.tasks);
        self.stats.host_complete_elapsed_ns = self
            .stats
            .host_complete_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        if completions.parallel {
            self.stats.parallel_batches += 1;
            self.stats.parallel_tasks += completions.items.len();
            self.stats.parallel_io_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_io_task(task))
                .count();
            self.stats.parallel_system_info_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_system_info_task(task))
                .count();
            self.stats.parallel_marker_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_scheduler_marker_task(task))
                .count();
            self.stats.parallel_workers = self.stats.parallel_workers.max(
                rayon::current_num_threads()
                    .min(completions.items.len())
                    .max(1),
            );
        }

        let started = Instant::now();
        let mut events = completions
            .items
            .into_iter()
            .map(|completion| self.task_event(completion))
            .collect::<Vec<_>>();
        events.extend(unauthorized_events);
        self.stats.event_build_elapsed_ns = self
            .stats
            .event_build_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        let started = Instant::now();
        let events = self.scheduler.complete(events);
        self.stats.scheduler_complete_elapsed_ns = self
            .stats
            .scheduler_complete_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());
        events
    }

    fn rejected_task_event(&mut self, task: TaskSpec) -> TaskEvent {
        self.stats.failed_tasks += 1;
        let event = TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: task.id,
            sequence: TaskSequence(self.sequence),
            kind: TaskEventKind::Err(format!(
                "host call `{}` is not provided by the active adapter manifest",
                task.request.host_call_id()
            )),
        };
        self.sequence = self.sequence.saturating_add(1);
        event
    }

    fn task_event(&mut self, completion: TaskCompletion) -> TaskEvent {
        self.stats.read_ops += completion.stats.read_ops;
        self.stats.write_ops += completion.stats.write_ops;
        self.stats.system_info_ops += completion.stats.system_info_ops;
        self.stats.bytes_read += completion.stats.bytes_read;
        self.stats.bytes_written += completion.stats.bytes_written;
        let kind = completion.result.map_or_else(
            |error| {
                self.stats.failed_tasks += 1;
                TaskEventKind::Err(error)
            },
            |value| {
                self.stats.completed_tasks += 1;
                TaskEventKind::Ready(value)
            },
        );
        let event = TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: completion.task_id,
            sequence: TaskSequence(self.sequence),
            kind,
        };
        self.sequence = self.sequence.saturating_add(1);
        event
    }
}

impl NativeHostCallSet {
    pub(crate) fn standard_cli() -> Self {
        Self::from_manifests([
            standard::native_file_manifest(),
            standard::system_info_manifest(),
        ])
        .with_call("line_task.run_child")
        .with_call("flow_thread.run_child")
    }

    pub(crate) fn from_manifest(manifest: &AdapterManifest) -> Self {
        Self::from_manifests([manifest.clone()])
    }

    pub(crate) fn from_manifests(manifests: impl IntoIterator<Item = AdapterManifest>) -> Self {
        Self {
            ids: manifests
                .into_iter()
                .flat_map(|manifest| {
                    manifest
                        .host_calls()
                        .iter()
                        .map(|host_call| host_call.id().to_owned())
                        .collect::<Vec<_>>()
                })
                .collect(),
        }
    }

    #[must_use]
    pub(crate) fn union(mut self, other: Self) -> Self {
        self.ids.extend(other.ids);
        self
    }

    #[must_use]
    pub(crate) fn with_call(mut self, id: impl Into<String>) -> Self {
        self.ids.insert(id.into());
        self
    }

    pub(crate) fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    fn allows_task(&self, task: &TaskSpec) -> bool {
        self.contains(&task.request.host_call_id())
    }
}

#[derive(Clone, Debug)]
struct TaskCompletions {
    parallel: bool,
    items: Vec<TaskCompletion>,
}

#[derive(Clone, Debug)]
struct TaskCompletion {
    task_id: arcweft_core::task::TaskId,
    result: Result<String, String>,
    stats: TaskCompletionStats,
}

#[derive(Clone, Copy, Debug, Default)]
struct TaskCompletionStats {
    read_ops: usize,
    write_ops: usize,
    system_info_ops: usize,
    bytes_read: usize,
    bytes_written: usize,
}

fn complete_dispatched_tasks(
    io_root: &Path,
    host_system: HostSystemInfo,
    tasks: &[TaskSpec],
) -> TaskCompletions {
    let parallel = should_complete_in_parallel(tasks);
    let items = if parallel {
        tasks
            .par_iter()
            .filter_map(|task| complete_task(io_root, host_system, task))
            .collect()
    } else {
        tasks
            .iter()
            .filter_map(|task| complete_task(io_root, host_system, task))
            .collect()
    };
    TaskCompletions { parallel, items }
}

fn should_complete_in_parallel(tasks: &[TaskSpec]) -> bool {
    tasks.len() > 1
        && tasks.iter().all(can_complete_in_parallel)
        && tasks.iter().any(is_parallel_host_work)
}

fn complete_task(
    io_root: &Path,
    host_system: HostSystemInfo,
    task: &TaskSpec,
) -> Option<TaskCompletion> {
    let (result, stats) = match &task.request {
        HostTaskRequest::FileReadText(request) => complete_read_text(io_root, &request.path),
        HostTaskRequest::FileWriteText(request) => {
            complete_write_text(io_root, &request.path, &request.text)
        }
        HostTaskRequest::FileReadBytes(request) => complete_read_bytes(io_root, &request.path),
        HostTaskRequest::FileWriteBytes(request) => {
            complete_write_bytes(io_root, &request.path, &request.bytes)
        }
        HostTaskRequest::Custom {
            capability,
            operation,
            ..
        } if is_scheduler_marker(capability.0.as_str(), operation) => {
            (Ok(String::new()), TaskCompletionStats::default())
        }
        HostTaskRequest::SystemInfo(request) => (
            Ok(system_info_value(host_system, request.kind).to_string()),
            TaskCompletionStats {
                system_info_ops: 1,
                ..TaskCompletionStats::default()
            },
        ),
        _ => return None,
    };
    Some(TaskCompletion {
        task_id: task.id.clone(),
        result,
        stats,
    })
}

fn complete_read_text(io_root: &Path, path: &str) -> (Result<String, String>, TaskCompletionStats) {
    match virtual_path(io_root, path)
        .and_then(|path| fs::read_to_string(path).map_err(|error| error.to_string()))
    {
        Ok(text) => {
            let bytes_read = text.len();
            (
                Ok(text),
                TaskCompletionStats {
                    read_ops: 1,
                    bytes_read,
                    ..TaskCompletionStats::default()
                },
            )
        }
        Err(error) => (Err(error), TaskCompletionStats::default()),
    }
}

fn complete_read_bytes(
    io_root: &Path,
    path: &str,
) -> (Result<String, String>, TaskCompletionStats) {
    match virtual_path(io_root, path)
        .and_then(|path| fs::read(path).map_err(|error| error.to_string()))
    {
        Ok(bytes) => {
            let bytes_read = bytes.len();
            (
                Ok(bytes
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(",")),
                TaskCompletionStats {
                    read_ops: 1,
                    bytes_read,
                    ..TaskCompletionStats::default()
                },
            )
        }
        Err(error) => (Err(error), TaskCompletionStats::default()),
    }
}

fn complete_write_text(
    io_root: &Path,
    path: &str,
    text: &str,
) -> (Result<String, String>, TaskCompletionStats) {
    let result = virtual_path(io_root, path).and_then(|path| {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, text).map_err(|error| error.to_string())?;
        Ok(String::new())
    });
    let stats = result.as_ref().map_or_else(
        |_| TaskCompletionStats::default(),
        |_| TaskCompletionStats {
            write_ops: 1,
            bytes_written: text.len(),
            ..TaskCompletionStats::default()
        },
    );
    (result, stats)
}

fn complete_write_bytes(
    io_root: &Path,
    path: &str,
    bytes: &[u8],
) -> (Result<String, String>, TaskCompletionStats) {
    let result = virtual_path(io_root, path).and_then(|path| {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, bytes).map_err(|error| error.to_string())?;
        Ok(String::new())
    });
    let stats = result.as_ref().map_or_else(
        |_| TaskCompletionStats::default(),
        |_| TaskCompletionStats {
            write_ops: 1,
            bytes_written: bytes.len(),
            ..TaskCompletionStats::default()
        },
    );
    (result, stats)
}

fn can_complete_task(task: &TaskSpec) -> bool {
    match &task.request {
        HostTaskRequest::FileReadText(_)
        | HostTaskRequest::FileWriteText(_)
        | HostTaskRequest::FileReadBytes(_)
        | HostTaskRequest::FileWriteBytes(_)
        | HostTaskRequest::SystemInfo(_) => true,
        HostTaskRequest::Custom {
            capability,
            operation,
            ..
        } => is_scheduler_marker(capability.0.as_str(), operation),
        _ => false,
    }
}

fn can_complete_in_parallel(task: &TaskSpec) -> bool {
    match &task.request {
        HostTaskRequest::FileReadText(_)
        | HostTaskRequest::FileReadBytes(_)
        | HostTaskRequest::SystemInfo(_) => true,
        HostTaskRequest::Custom {
            capability,
            operation,
            ..
        } => is_scheduler_marker(capability.0.as_str(), operation),
        _ => false,
    }
}

fn is_io_task(task: &TaskSpec) -> bool {
    matches!(
        &task.request,
        HostTaskRequest::FileReadText(_) | HostTaskRequest::FileReadBytes(_)
    )
}

fn is_system_info_task(task: &TaskSpec) -> bool {
    matches!(&task.request, HostTaskRequest::SystemInfo(_))
}

fn is_parallel_host_work(task: &TaskSpec) -> bool {
    is_io_task(task) || is_system_info_task(task)
}

fn is_scheduler_marker_task(task: &TaskSpec) -> bool {
    match &task.request {
        HostTaskRequest::Custom {
            capability,
            operation,
            ..
        } => is_scheduler_marker(capability.0.as_str(), operation),
        _ => false,
    }
}

fn is_scheduler_marker(capability: &str, operation: &str) -> bool {
    matches!(capability, "line_task" | "flow_thread") && operation == "run_child"
}

fn virtual_path(io_root: &Path, value: &str) -> Result<PathBuf, String> {
    let (space, relative) = value
        .split_once(':')
        .ok_or_else(|| "file task path must be a virtual path".to_owned())?;
    if !matches!(space, "save" | "asset" | "temp" | "export") {
        return Err(format!("unsupported virtual path space `{space}`"));
    }
    let relative_path = Path::new(relative);
    if relative_path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
        )
    }) {
        return Err("virtual path must be relative and normalized".to_owned());
    }
    Ok(io_root.join(space).join(relative_path))
}

impl From<RuntimeSchedulerStats> for NativeSchedulerStats {
    fn from(stats: RuntimeSchedulerStats) -> Self {
        Self {
            submitted: stats.submitted,
            joined: stats.joined,
            dispatched: stats.dispatched,
            completed: stats.completed,
            failed: stats.failed,
            cancelled: stats.cancelled,
            cancel_requested: stats.cancel_requested,
            joined_completed: stats.joined_completed,
            in_flight: stats.in_flight,
            max_in_flight: stats.max_in_flight,
            dispatch_sorts: stats.dispatch_sorts,
            dispatch_sort_items: stats.dispatch_sort_items,
            completion_sorts: stats.completion_sorts,
            completion_sort_items: stats.completion_sort_items,
            completion_normalization_passes: stats.completion_normalization_passes,
            completion_normalization_checks: stats.completion_normalization_checks,
            completion_events_in: stats.completion_events_in,
            completion_events_joined: stats.completion_events_joined,
            completion_events_out: stats.completion_events_out,
            completion_sort_skipped_items: stats.completion_sort_skipped_items,
            completion_sort_performed_items: stats.completion_sort_performed_items,
            joined_completion_events_emitted: stats.joined_completion_events_emitted,
            submitted_by_class: NativeTaskClassCounts::from(stats.submitted_by_class),
            dispatched_by_class: NativeTaskClassCounts::from(stats.dispatched_by_class),
            completed_by_class: NativeTaskClassCounts::from(stats.completed_by_class),
        }
    }
}

impl From<TaskClassCounts> for NativeTaskClassCounts {
    fn from(counts: TaskClassCounts) -> Self {
        Self {
            local_ui: counts.local_ui,
            io: counts.io,
            cpu: counts.cpu,
            gpu_prepare: counts.gpu_prepare,
            shader_compile: counts.shader_compile,
            wasm_call: counts.wasm_call,
            asset_decode: counts.asset_decode,
            audio_decode: counts.audio_decode,
            audio_render: counts.audio_render,
            tts_synthesis: counts.tts_synthesis,
            bgm_precompose: counts.bgm_precompose,
            lsp: counts.lsp,
            background: counts.background,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::task::{
        CancelScopeId, HostTaskRequest, SystemInfoKind, SystemInfoRequest, TaskClass, TaskId,
        TaskKey, TaskPolicy, TaskPriority,
    };

    #[test]
    fn native_bridge_rejects_host_call_missing_from_manifest() {
        let source_path = std::env::temp_dir().join("arcweft-native-bridge-reject.arcw");
        let mut bridge = NativeTaskBridge::new(&source_path, NativeHostCallSet::default());
        let events = bridge.complete_tasks(vec![task(
            "missing",
            HostTaskRequest::SystemInfo(SystemInfoRequest {
                kind: SystemInfoKind::CoreCount,
            }),
        )]);

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::Err(message)
                if message.contains("host call `system.core_count` is not provided")
        ));
        assert_eq!(bridge.stats().failed_tasks, 1);
        assert_eq!(bridge.stats().scheduler.submitted, 0);
    }

    #[test]
    fn native_bridge_completes_system_info_allowed_by_manifest() {
        let source_path = std::env::temp_dir().join("arcweft-native-bridge-system.arcw");
        let calls = NativeHostCallSet::from_manifest(&standard::system_info_manifest());
        let mut bridge = NativeTaskBridge::new(&source_path, calls);
        let events = bridge.complete_tasks(vec![task(
            "system",
            HostTaskRequest::SystemInfo(SystemInfoRequest {
                kind: SystemInfoKind::AvailableParallelism,
            }),
        )]);

        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0].kind, TaskEventKind::Ready(value) if !value.is_empty()));
        assert_eq!(bridge.stats().completed_tasks, 1);
        assert_eq!(bridge.stats().system_info_ops, 1);
        assert_eq!(bridge.stats().scheduler.submitted, 1);
    }

    #[test]
    fn standard_cli_host_call_set_is_manifest_derived() {
        let calls = NativeHostCallSet::standard_cli();

        for id in [
            "fs.read_text",
            "fs.read_bytes",
            "fs.write_text",
            "fs.write_bytes",
            "system.core_count",
            "system.thread_count",
            "system.available_parallelism",
            "line_task.run_child",
            "flow_thread.run_child",
        ] {
            assert!(calls.contains(id), "missing host call {id}");
        }
    }

    fn task(id: &str, request: HostTaskRequest) -> TaskSpec {
        TaskSpec::new(
            TaskId(id.to_owned()),
            TaskKey(id.to_owned()),
            TaskClass::Cpu,
            TaskPriority(0),
            CancelScopeId("test".to_owned()),
            TaskPolicy::JoinSameKey,
            request,
        )
    }
}
