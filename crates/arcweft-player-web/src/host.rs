use arcweft_bundle::{ArcweftBundle, BundleVirtualFileSpace};
use arcweft_core::task::{
    CancelScopeId, HostTaskRequest, SystemInfoKind, TaskEvent, TaskEventKind,
};
use arcweft_core::value::{RuntimePayload, RuntimeValue, runtime_sequence_dense_bytes};
use arcweft_runtime_driver::task::HostTaskDispatch;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Browser host task broker for the synchronous embedded-VFS MVP slice.
///
/// Fetch, `IndexedDB`, `WebAudio`, and nested Wasm are explicit post-MVP adapters.
/// Unsupported calls produce deterministic task errors instead of being ignored.
#[derive(Clone, Debug)]
pub struct BrowserTaskBroker {
    allowed_calls: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
    asset_files: BTreeMap<String, String>,
    cancelled_scopes: BTreeSet<CancelScopeId>,
    queued_task_events: Vec<TaskEvent>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BrowserHostTaskError {
    #[error("host call `{0}` is not declared by the active bundle manifests")]
    UndeclaredHostCall(String),
    #[error("browser MVP does not implement host call `{0}`")]
    UnsupportedHostCall(String),
    #[error("virtual path must be relative, normalized, and use asset/save/temp/export")]
    InvalidVirtualPath,
    #[error("virtual file `{0}` was not found")]
    MissingVirtualFile(String),
    #[error("virtual file `{0}` is not valid UTF-8")]
    InvalidUtf8(String),
    #[error("asset virtual files are read-only")]
    ReadOnlyAsset,
    #[error("bundle image asset `{0}` was not found")]
    MissingAsset(String),
}

impl BrowserTaskBroker {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Result<Self, BrowserHostTaskError> {
        let allowed_calls = bundle
            .manifest
            .required_host_calls
            .iter()
            .cloned()
            .chain(
                bundle
                    .adapter_manifests
                    .iter()
                    .flat_map(|manifest| manifest.host_calls.iter().map(|call| call.id.clone())),
            )
            .collect();
        let files = bundle
            .virtual_files
            .iter()
            .try_fold(BTreeMap::new(), |mut files, file| {
                let key = virtual_file_key(file.space, &file.path);
                validate_virtual_path(&key)?;
                files.insert(key, file.bytes.clone());
                Ok::<_, BrowserHostTaskError>(files)
            })?;
        let asset_files =
            bundle
                .image_assets
                .iter()
                .try_fold(BTreeMap::new(), |mut assets, asset| {
                    let key = virtual_file_key(asset.file.space, &asset.file.path);
                    if !files.contains_key(&key) {
                        return Err(BrowserHostTaskError::MissingVirtualFile(key));
                    }
                    assets.insert(asset.id.clone(), key);
                    Ok::<_, BrowserHostTaskError>(assets)
                })?;
        Ok(Self {
            allowed_calls,
            files,
            asset_files,
            cancelled_scopes: BTreeSet::new(),
            queued_task_events: Vec::new(),
        })
    }

    /// Resolves currently supported work and queues task events for the next VM
    /// step. The return value is a queue count, not a claim that the VM has
    /// already consumed those completions.
    pub fn queue_dispatches(&mut self, dispatches: Vec<HostTaskDispatch>) -> usize {
        let events = dispatches
            .into_iter()
            .map(|dispatch| {
                if self.cancelled_scopes.contains(&dispatch.task.cancel_scope) {
                    return dispatch.cancelled();
                }
                let kind = self
                    .resolve(&dispatch)
                    .unwrap_or_else(|error| TaskEventKind::Failed(error.to_string()));
                dispatch.into_event(kind)
            })
            .collect::<Vec<_>>();
        self.queued_task_events.extend(events);
        self.queued_task_events.sort_by(|left, right| {
            (left.logical_epoch, left.sequence, &left.task_id).cmp(&(
                right.logical_epoch,
                right.sequence,
                &right.task_id,
            ))
        });
        self.queued_task_events.len()
    }

    pub fn drain_queued_task_events(&mut self) -> Vec<TaskEvent> {
        std::mem::take(&mut self.queued_task_events)
    }

    pub fn queued_task_event_count(&self) -> usize {
        self.queued_task_events.len()
    }

    pub fn cancel_scopes(&mut self, scopes: impl IntoIterator<Item = CancelScopeId>) {
        self.cancelled_scopes.extend(scopes);
    }

    fn resolve(
        &mut self,
        dispatch: &HostTaskDispatch,
    ) -> Result<TaskEventKind, BrowserHostTaskError> {
        let request = &dispatch.task.request;
        let call = request.host_call_id();
        if !self.allowed_calls.contains(&call) && !is_internal_scheduler_marker(request) {
            return Err(BrowserHostTaskError::UndeclaredHostCall(call));
        }
        match request {
            HostTaskRequest::FileReadText(request) => self.read_text(&request.path),
            HostTaskRequest::FileReadBytes(request) => self.read_bytes(&request.path),
            HostTaskRequest::FileWriteText(request) => {
                self.write_bytes(&request.path, request.text.as_bytes())
            }
            HostTaskRequest::FileWriteBytes(request) => {
                self.write_bytes(&request.path, &request.bytes)
            }
            HostTaskRequest::AssetLoad(request) => self.read_asset(&request.id),
            HostTaskRequest::SystemInfo(request) => {
                let value = match request.kind {
                    SystemInfoKind::CoreCount
                    | SystemInfoKind::ThreadCount
                    | SystemInfoKind::AvailableParallelism => 1_u64,
                };
                Ok(TaskEventKind::Ready(RuntimePayload::new(
                    RuntimeValue::usize(value),
                )))
            }
            request if is_internal_scheduler_marker(request) => Ok(TaskEventKind::Ready(
                RuntimePayload::new(RuntimeValue::Unit),
            )),
            request => Err(BrowserHostTaskError::UnsupportedHostCall(
                request.host_call_id(),
            )),
        }
    }

    fn read_text(&self, path: &str) -> Result<TaskEventKind, BrowserHostTaskError> {
        validate_virtual_path(path)?;
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| BrowserHostTaskError::MissingVirtualFile(path.to_owned()))?;
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| BrowserHostTaskError::InvalidUtf8(path.to_owned()))?;
        Ok(TaskEventKind::Ready(RuntimePayload::from(text)))
    }

    fn read_bytes(&self, path: &str) -> Result<TaskEventKind, BrowserHostTaskError> {
        validate_virtual_path(path)?;
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| BrowserHostTaskError::MissingVirtualFile(path.to_owned()))?;
        Ok(TaskEventKind::Ready(RuntimePayload::new(
            runtime_sequence_dense_bytes(bytes.clone()),
        )))
    }

    fn read_asset(&self, id: &str) -> Result<TaskEventKind, BrowserHostTaskError> {
        let path = self
            .asset_files
            .get(id)
            .ok_or_else(|| BrowserHostTaskError::MissingAsset(id.to_owned()))?;
        self.read_bytes(path)
    }

    fn write_bytes(
        &mut self,
        path: &str,
        bytes: &[u8],
    ) -> Result<TaskEventKind, BrowserHostTaskError> {
        match validate_virtual_path(path)? {
            BundleVirtualFileSpace::Asset => Err(BrowserHostTaskError::ReadOnlyAsset),
            BundleVirtualFileSpace::Save
            | BundleVirtualFileSpace::Temp
            | BundleVirtualFileSpace::Export => {
                self.files.insert(path.to_owned(), bytes.to_vec());
                Ok(TaskEventKind::Ready(RuntimePayload::new(
                    RuntimeValue::Unit,
                )))
            }
        }
    }
}

fn is_internal_scheduler_marker(request: &HostTaskRequest) -> bool {
    matches!(
        request,
        HostTaskRequest::Custom { capability, operation, .. }
            if matches!(capability.0.as_str(), "line_task" | "flow_thread")
                && operation == "run_child"
    )
}

fn virtual_file_key(space: BundleVirtualFileSpace, path: &str) -> String {
    format!("{}:{path}", space.as_str())
}

fn validate_virtual_path(path: &str) -> Result<BundleVirtualFileSpace, BrowserHostTaskError> {
    let (space, relative) = path
        .split_once(':')
        .ok_or(BrowserHostTaskError::InvalidVirtualPath)?;
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.contains('\\')
        || relative.contains('\0')
        || relative
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(BrowserHostTaskError::InvalidVirtualPath);
    }
    match space {
        "asset" => Ok(BundleVirtualFileSpace::Asset),
        "save" => Ok(BundleVirtualFileSpace::Save),
        "temp" => Ok(BundleVirtualFileSpace::Temp),
        "export" => Ok(BundleVirtualFileSpace::Export),
        _ => Err(BrowserHostTaskError::InvalidVirtualPath),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_paths_reject_traversal_and_host_paths() {
        for path in [
            "save:../slot.json",
            "save:/slot.json",
            "save:slot//data.json",
            "save:slot\\data.json",
            "native:slot.json",
        ] {
            assert!(
                validate_virtual_path(path).is_err(),
                "path should fail: {path}"
            );
        }
    }
}
