use crate::codec::{decode_request, outcome};
use crate::manifest::{
    DESKTOP_CAPABILITIES_CALL, DESKTOP_EXTERNAL_CONTROL_CALL, DESKTOP_EXTERNAL_OBSERVE_CALL,
    DESKTOP_FILES_READ_CALL, DESKTOP_FILES_WRITE_CALL, DESKTOP_GLOBAL_POINTER_CONTROL_CALL,
    DESKTOP_GLOBAL_POINTER_OBSERVE_CALL, DESKTOP_KNOWN_READ_CALL, DESKTOP_KNOWN_WRITE_CALL,
    desktop_external_control_manifest, desktop_external_observe_manifest,
    desktop_files_read_manifest, desktop_files_write_manifest,
    desktop_known_directory_read_manifest, desktop_known_directory_write_manifest,
    desktop_owned_window_manifest, desktop_platform_manifest,
    desktop_pointer_global_control_manifest, desktop_pointer_global_observe_manifest,
};
use arcweft_adapter_context::manifest::AdapterManifest;
use arcweft_core::task::{HostTaskRequest, TaskId, TaskSpec};
use arcweft_desktop_contract::{
    DesktopRequest, ExternalWindowRequest, FileDialogMode, GlobalPointerRequest, GrantAccess,
    UserFileRequest,
};
use arcweft_desktop_host::{
    DesktopBackend, DesktopHost, DesktopSubmission, DesktopTaskId, PumpReport,
};
use arcweft_host_adapter::{
    HostAdapter, HostAdapterCompletion, HostAdapterError, HostAdapterRegistryBuilder,
    HostTaskSubmission,
};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestDomain {
    Capabilities,
    OwnedWindow,
    OwnedCursor,
    UserFileRead,
    UserFileWrite,
    KnownDirectoryRead,
    KnownDirectoryWrite,
    GlobalPointerObserve,
    GlobalPointerControl,
    ExternalObserve,
    ExternalControl,
}

#[derive(Clone, Debug)]
struct PendingTask {
    arcweft_task: TaskId,
    request: DesktopRequest,
}

/// Shared bridge retained by the native player so it can pump window-thread work.
pub struct DesktopCoordinator<B: DesktopBackend> {
    host: DesktopHost<B>,
}

impl<B: DesktopBackend> DesktopCoordinator<B> {
    pub fn bind_current_thread(backend: B) -> Self {
        Self {
            host: DesktopHost::bind_current_thread(backend),
        }
    }

    /// Must be called from the event-loop thread once per host turn.
    pub fn pump_main_thread(&self) -> Result<PumpReport, arcweft_desktop_contract::DesktopError> {
        self.host.pump_main_thread()
    }

    pub fn pending_count(&self) -> usize {
        self.host.pending_count()
    }
}

impl<B: DesktopBackend> fmt::Debug for DesktopCoordinator<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopCoordinator")
            .field("pending_count", &self.pending_count())
            .finish_non_exhaustive()
    }
}

struct DesktopArcweftAdapter<B: DesktopBackend> {
    manifest: AdapterManifest,
    domains: BTreeMap<String, RequestDomain>,
    coordinator: Arc<DesktopCoordinator<B>>,
    pending: Mutex<BTreeMap<DesktopTaskId, PendingTask>>,
    pumps_main_thread: bool,
}

impl<B: DesktopBackend> DesktopArcweftAdapter<B> {
    fn new<I, S>(
        manifest: AdapterManifest,
        domains: I,
        coordinator: Arc<DesktopCoordinator<B>>,
        pumps_main_thread: bool,
    ) -> Self
    where
        I: IntoIterator<Item = (S, RequestDomain)>,
        S: Into<String>,
    {
        Self {
            manifest,
            domains: domains
                .into_iter()
                .map(|(call, domain)| (call.into(), domain))
                .collect(),
            coordinator,
            pending: Mutex::new(BTreeMap::new()),
            pumps_main_thread,
        }
    }

    fn validate_domain(&self, task: &TaskSpec, request: &DesktopRequest) -> Result<(), String> {
        let call = task.request.host_call_id();
        let expected = self
            .domains
            .get(&call)
            .ok_or_else(|| format!("adapter does not own host call `{call}`"))?;
        if request_matches(*expected, request) {
            Ok(())
        } else {
            Err(format!(
                "desktop request domain does not match host call `{call}`"
            ))
        }
    }

    fn pending(&self) -> MutexGuard<'_, BTreeMap<DesktopTaskId, PendingTask>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<B: DesktopBackend> fmt::Debug for DesktopArcweftAdapter<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopArcweftAdapter")
            .field("manifest", &self.manifest.id().as_str())
            .field("pending", &self.pending().len())
            .finish_non_exhaustive()
    }
}

impl<B: DesktopBackend> HostAdapter for DesktopArcweftAdapter<B> {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn submit(&self, task: &TaskSpec) -> Option<HostTaskSubmission> {
        if !self.domains.contains_key(&task.request.host_call_id()) {
            return None;
        }
        let request = match decode_request(task).and_then(|request| {
            self.validate_domain(task, &request)?;
            Ok(request)
        }) {
            Ok(request) => request,
            Err(error) => {
                return Some(HostTaskSubmission::Completed(
                    arcweft_host_adapter::HostTaskOutcome {
                        result: Err(error),
                        metrics: arcweft_host_adapter::HostTaskMetrics::default(),
                    },
                ));
            }
        };
        match self.coordinator.host.submit(request.clone()) {
            DesktopSubmission::Completed(result) => {
                Some(HostTaskSubmission::Completed(outcome(&request, result)))
            }
            DesktopSubmission::Pending(desktop_task) => {
                self.pending().insert(
                    desktop_task,
                    PendingTask {
                        arcweft_task: task.id.clone(),
                        request,
                    },
                );
                Some(HostTaskSubmission::Pending)
            }
        }
    }

    fn drain_completions(&self) -> Vec<HostAdapterCompletion> {
        let ids = self.pending().keys().copied().collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|desktop_task| {
                let result = self.coordinator.host.poll(desktop_task)?;
                let pending = self.pending().remove(&desktop_task)?;
                Some(HostAdapterCompletion {
                    task_id: pending.arcweft_task,
                    outcome: outcome(&pending.request, result),
                })
            })
            .collect()
    }

    fn cancel(&self, task_id: &TaskId) -> bool {
        let desktop_task = self.pending().iter().find_map(|(desktop_task, pending)| {
            (&pending.arcweft_task == task_id).then_some(*desktop_task)
        });
        let Some(desktop_task) = desktop_task else {
            return false;
        };
        let removed = self.pending().remove(&desktop_task).is_some();
        self.coordinator.host.cancel(desktop_task) || removed
    }

    fn pump_main_thread(&self) -> Result<(), String> {
        if self.pumps_main_thread {
            self.coordinator
                .pump_main_thread()
                .map(|_| ())
                .map_err(|error| error.to_string())
        } else {
            Ok(())
        }
    }

    fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
        false
    }
}

/// Ten logical adapters sharing one native backend and one main-thread queue.
pub struct DesktopAdapterSet<B: DesktopBackend> {
    coordinator: Arc<DesktopCoordinator<B>>,
    adapters: Vec<DesktopArcweftAdapter<B>>,
}

impl<B: DesktopBackend> DesktopAdapterSet<B> {
    pub fn bind_current_thread(backend: B) -> Self {
        let coordinator = Arc::new(DesktopCoordinator::bind_current_thread(backend));
        let owned_window_manifest = desktop_owned_window_manifest();
        let owned_window_domains = owned_window_manifest
            .host_calls()
            .iter()
            .map(|call| {
                let domain = if call.id().starts_with("desktop.cursor.") {
                    RequestDomain::OwnedCursor
                } else {
                    RequestDomain::OwnedWindow
                };
                (call.id().to_owned(), domain)
            })
            .collect::<Vec<_>>();
        let adapters = vec![
            DesktopArcweftAdapter::new(
                desktop_platform_manifest(),
                [(DESKTOP_CAPABILITIES_CALL, RequestDomain::Capabilities)],
                coordinator.clone(),
                true,
            ),
            DesktopArcweftAdapter::new(
                owned_window_manifest,
                owned_window_domains,
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_files_read_manifest(),
                [(DESKTOP_FILES_READ_CALL, RequestDomain::UserFileRead)],
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_files_write_manifest(),
                [(DESKTOP_FILES_WRITE_CALL, RequestDomain::UserFileWrite)],
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_known_directory_read_manifest(),
                [(DESKTOP_KNOWN_READ_CALL, RequestDomain::KnownDirectoryRead)],
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_known_directory_write_manifest(),
                [(DESKTOP_KNOWN_WRITE_CALL, RequestDomain::KnownDirectoryWrite)],
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_pointer_global_observe_manifest(),
                [(
                    DESKTOP_GLOBAL_POINTER_OBSERVE_CALL,
                    RequestDomain::GlobalPointerObserve,
                )],
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_pointer_global_control_manifest(),
                [(
                    DESKTOP_GLOBAL_POINTER_CONTROL_CALL,
                    RequestDomain::GlobalPointerControl,
                )],
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_external_observe_manifest(),
                [(
                    DESKTOP_EXTERNAL_OBSERVE_CALL,
                    RequestDomain::ExternalObserve,
                )],
                coordinator.clone(),
                false,
            ),
            DesktopArcweftAdapter::new(
                desktop_external_control_manifest(),
                [(
                    DESKTOP_EXTERNAL_CONTROL_CALL,
                    RequestDomain::ExternalControl,
                )],
                coordinator.clone(),
                false,
            ),
        ];
        Self {
            coordinator,
            adapters,
        }
    }

    pub fn coordinator(&self) -> Arc<DesktopCoordinator<B>> {
        self.coordinator.clone()
    }

    pub fn register(
        self,
        builder: HostAdapterRegistryBuilder,
    ) -> Result<(HostAdapterRegistryBuilder, Arc<DesktopCoordinator<B>>), HostAdapterError> {
        let coordinator = self.coordinator;
        let builder = self
            .adapters
            .into_iter()
            .try_fold(builder, HostAdapterRegistryBuilder::register)?;
        Ok((builder, coordinator))
    }
}

fn request_matches(domain: RequestDomain, request: &DesktopRequest) -> bool {
    match domain {
        RequestDomain::Capabilities => matches!(request, DesktopRequest::Capabilities),
        RequestDomain::OwnedWindow => matches!(request, DesktopRequest::OwnedWindow(_)),
        RequestDomain::OwnedCursor => matches!(request, DesktopRequest::OwnedCursor(_)),
        RequestDomain::UserFileRead => matches!(
            request,
            DesktopRequest::UserFile(request) if user_file_read_request(request)
        ),
        RequestDomain::UserFileWrite => matches!(
            request,
            DesktopRequest::UserFile(request) if user_file_write_request(request)
        ),
        RequestDomain::KnownDirectoryRead => matches!(
            request,
            DesktopRequest::UserFile(UserFileRequest::GrantKnownDirectory { access, .. })
                if *access == GrantAccess::Read
        ),
        RequestDomain::KnownDirectoryWrite => matches!(
            request,
            DesktopRequest::UserFile(UserFileRequest::GrantKnownDirectory { access, .. })
                if access.permits_write()
        ),
        RequestDomain::GlobalPointerObserve => matches!(
            request,
            DesktopRequest::GlobalPointer(GlobalPointerRequest::Position)
        ),
        RequestDomain::GlobalPointerControl => matches!(
            request,
            DesktopRequest::GlobalPointer(GlobalPointerRequest::Move { .. })
        ),
        RequestDomain::ExternalObserve => matches!(
            request,
            DesktopRequest::ExternalWindow(
                ExternalWindowRequest::List | ExternalWindowRequest::Get { .. }
            )
        ),
        RequestDomain::ExternalControl => matches!(
            request,
            DesktopRequest::ExternalWindow(
                ExternalWindowRequest::Activate { .. }
                    | ExternalWindowRequest::SetBounds { .. }
                    | ExternalWindowRequest::RequestClose { .. }
            )
        ),
    }
}

fn user_file_read_request(request: &UserFileRequest) -> bool {
    match request {
        UserFileRequest::ShowDialog(dialog) => {
            dialog.access == GrantAccess::Read
                && matches!(
                    dialog.mode,
                    FileDialogMode::OpenFile
                        | FileDialogMode::OpenFiles
                        | FileDialogMode::PickDirectory
                )
        }
        UserFileRequest::ReadText { .. }
        | UserFileRequest::ReadBytes { .. }
        | UserFileRequest::Metadata { .. }
        | UserFileRequest::ListDirectory { .. }
        | UserFileRequest::Revoke { .. } => true,
        UserFileRequest::GrantKnownDirectory { .. }
        | UserFileRequest::WriteText { .. }
        | UserFileRequest::WriteBytes { .. } => false,
    }
}

fn user_file_write_request(request: &UserFileRequest) -> bool {
    match request {
        UserFileRequest::ShowDialog(dialog) => {
            dialog.access.permits_write()
                && matches!(
                    dialog.mode,
                    FileDialogMode::SaveFile | FileDialogMode::PickDirectory
                )
        }
        UserFileRequest::WriteText { .. }
        | UserFileRequest::WriteBytes { .. }
        | UserFileRequest::Revoke { .. } => true,
        UserFileRequest::GrantKnownDirectory { .. }
        | UserFileRequest::ReadText { .. }
        | UserFileRequest::ReadBytes { .. }
        | UserFileRequest::Metadata { .. }
        | UserFileRequest::ListDirectory { .. } => false,
    }
}
