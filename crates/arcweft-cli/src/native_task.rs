use crate::native_system::{HostSystemInfo, host_system_info, system_info_value};
use arcweft_core::task::{
    HostTaskRequest, LogicalEpoch, SchedulerBudget, TaskEvent, TaskEventKind, TaskSequence,
    TaskSpec,
};
use arcweft_runtime_scheduler::{RuntimeScheduler, RuntimeSchedulerStats};
use rayon::prelude::*;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct NativeTaskBridge {
    io_root: PathBuf,
    host_system: HostSystemInfo,
    sequence: u64,
    scheduler: RuntimeScheduler,
    stats: NativeTaskStats,
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
    pub(crate) parallel_marker_tasks: usize,
    pub(crate) parallel_workers: usize,
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
}

impl NativeTaskBridge {
    pub(crate) fn new(source_path: &Path) -> Self {
        let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
        Self {
            io_root: source_dir.join(".arcweft"),
            host_system: host_system_info(),
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
        let bridge = Self::new(source_path);
        virtual_path(&bridge.io_root, value)
            .and_then(|path| fs::read_to_string(path).map_err(|error| error.to_string()))
    }

    pub(crate) fn complete_tasks(&mut self, tasks: &[TaskSpec]) -> Vec<TaskEvent> {
        self.scheduler
            .submit(tasks.iter().filter(|task| can_complete_task(task)).cloned());
        let dispatch = self.scheduler.dispatch(SchedulerBudget {
            max_events: usize::MAX,
        });
        let completions =
            complete_dispatched_tasks(&self.io_root, self.host_system, &dispatch.tasks);
        if completions.parallel {
            self.stats.parallel_batches += 1;
            self.stats.parallel_tasks += completions.items.len();
            self.stats.parallel_io_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_io_task(task))
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
        let events = completions
            .items
            .into_iter()
            .map(|completion| self.task_event(completion))
            .collect::<Vec<_>>();
        self.scheduler.complete(events)
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
    let parallel = tasks.len() > 1 && tasks.iter().all(can_complete_in_parallel);
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
        }
    }
}
