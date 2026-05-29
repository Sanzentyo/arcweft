use arcweft_core::task::{
    HostTaskRequest, LogicalEpoch, SchedulerBudget, TaskEvent, TaskEventKind, TaskSequence,
    TaskSpec,
};
use arcweft_runtime_scheduler::{RuntimeScheduler, RuntimeSchedulerStats};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct NativeTaskBridge {
    io_root: PathBuf,
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
    pub(crate) bytes_read: usize,
    pub(crate) bytes_written: usize,
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
    pub(crate) in_flight: usize,
    pub(crate) max_in_flight: usize,
}

impl NativeTaskBridge {
    pub(crate) fn new(source_path: &Path) -> Self {
        let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
        Self {
            io_root: source_dir.join(".arcweft"),
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
        Self::new(source_path)
            .virtual_path(value)
            .and_then(|path| fs::read_to_string(path).map_err(|error| error.to_string()))
    }

    pub(crate) fn complete_tasks(&mut self, tasks: &[TaskSpec]) -> Vec<TaskEvent> {
        self.scheduler
            .submit(tasks.iter().filter(|task| can_complete_task(task)).cloned());
        let dispatch = self.scheduler.dispatch(SchedulerBudget {
            max_events: usize::MAX,
        });
        let events = dispatch
            .tasks
            .iter()
            .filter_map(|task| {
                let result = match &task.request {
                    HostTaskRequest::FileReadText(request) => {
                        self.virtual_path(&request.path).and_then(|path| {
                            fs::read_to_string(path)
                                .inspect(|text| {
                                    self.stats.read_ops += 1;
                                    self.stats.bytes_read += text.len();
                                })
                                .map_err(|error| error.to_string())
                        })
                    }
                    HostTaskRequest::FileWriteText(request) => {
                        self.virtual_path(&request.path).and_then(|path| {
                            if let Some(parent) = path.parent() {
                                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                            }
                            fs::write(path, &request.text).map_err(|error| error.to_string())?;
                            self.stats.write_ops += 1;
                            self.stats.bytes_written += request.text.len();
                            Ok(String::new())
                        })
                    }
                    HostTaskRequest::FileReadBytes(request) => {
                        self.virtual_path(&request.path).and_then(|path| {
                            fs::read(path)
                                .map(|bytes| {
                                    self.stats.read_ops += 1;
                                    self.stats.bytes_read += bytes.len();
                                    bytes
                                        .iter()
                                        .map(u8::to_string)
                                        .collect::<Vec<_>>()
                                        .join(",")
                                })
                                .map_err(|error| error.to_string())
                        })
                    }
                    HostTaskRequest::FileWriteBytes(request) => {
                        self.virtual_path(&request.path).and_then(|path| {
                            if let Some(parent) = path.parent() {
                                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                            }
                            fs::write(path, &request.bytes).map_err(|error| error.to_string())?;
                            self.stats.write_ops += 1;
                            self.stats.bytes_written += request.bytes.len();
                            Ok(String::new())
                        })
                    }
                    HostTaskRequest::Custom {
                        capability,
                        operation,
                        ..
                    } if is_scheduler_marker(capability.0.as_str(), operation) => Ok(String::new()),
                    _ => return None,
                };
                let kind = result.map_or_else(
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
                    task_id: task.id.clone(),
                    sequence: TaskSequence(self.sequence),
                    kind,
                };
                self.sequence = self.sequence.saturating_add(1);
                Some(event)
            })
            .collect::<Vec<_>>();
        self.scheduler.complete(events)
    }

    fn virtual_path(&self, value: &str) -> Result<PathBuf, String> {
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
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::ParentDir
                    | Component::CurDir
            )
        }) {
            return Err("virtual path must be relative and normalized".to_owned());
        }
        Ok(self.io_root.join(space).join(relative_path))
    }
}

fn can_complete_task(task: &TaskSpec) -> bool {
    match &task.request {
        HostTaskRequest::FileReadText(_)
        | HostTaskRequest::FileWriteText(_)
        | HostTaskRequest::FileReadBytes(_)
        | HostTaskRequest::FileWriteBytes(_) => true,
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
            in_flight: stats.in_flight,
            max_in_flight: stats.max_in_flight,
        }
    }
}
