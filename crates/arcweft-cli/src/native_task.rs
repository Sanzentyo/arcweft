use arcweft_core::task::{
    HostTaskRequest, LogicalEpoch, TaskEvent, TaskEventKind, TaskSequence, TaskSpec,
};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct NativeTaskBridge {
    io_root: PathBuf,
    sequence: u64,
}

impl NativeTaskBridge {
    pub(crate) fn new(source_path: &Path) -> Self {
        let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
        Self {
            io_root: source_dir.join(".arcweft"),
            sequence: 0,
        }
    }

    pub(crate) fn complete_tasks(&mut self, tasks: &[TaskSpec]) -> Vec<TaskEvent> {
        tasks
            .iter()
            .filter_map(|task| {
                let result = match &task.request {
                    HostTaskRequest::FileReadText(request) => {
                        self.virtual_path(&request.path).and_then(|path| {
                            fs::read_to_string(path).map_err(|error| error.to_string())
                        })
                    }
                    HostTaskRequest::FileWriteText(request) => {
                        self.virtual_path(&request.path).and_then(|path| {
                            if let Some(parent) = path.parent() {
                                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                            }
                            fs::write(path, &request.text).map_err(|error| error.to_string())?;
                            Ok(String::new())
                        })
                    }
                    HostTaskRequest::FileReadBytes(request) => {
                        self.virtual_path(&request.path).and_then(|path| {
                            fs::read(path)
                                .map(|bytes| {
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
                            Ok(String::new())
                        })
                    }
                    _ => return None,
                };
                let kind = result.map_or_else(TaskEventKind::Err, TaskEventKind::Ready);
                let event = TaskEvent {
                    logical_epoch: LogicalEpoch(0),
                    task_id: task.id.clone(),
                    sequence: TaskSequence(self.sequence),
                    kind,
                };
                self.sequence = self.sequence.saturating_add(1);
                Some(event)
            })
            .collect()
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
