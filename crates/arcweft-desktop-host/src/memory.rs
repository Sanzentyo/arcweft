use crate::{BackendSubmission, DesktopBackend, DesktopTaskId, ExecutionLane};
use arcweft_desktop_contract::{
    DesktopCapabilities, DesktopError, DesktopFeature, DesktopRequest, DesktopResponse,
    DirectoryEntry, ExternalWindowRequest, ExternalWindowResponse, FeatureSupport, FileDialogMode,
    FileEntryKind, FileGrant, FileGrantId, FileMetadata, GlobalPointerRequest,
    GlobalPointerResponse, GrantAccess, GrantLifetime, GrantOrigin, GrantPath, OwnedCursorRequest,
    OwnedWindowRequest, OwnedWindowResponse, PermissionKind, PhysicalPosition, PlatformKind,
    PointerCoordinateSpace, PointerPosition, PortableRelativePath, SupportLevel, UserFileRequest,
    UserFileResponse, WindowId, WindowScope, WindowSnapshot, WindowTarget,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

#[derive(Clone)]
struct MemoryEntry {
    kind: FileEntryKind,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct MemoryGrant {
    public: FileGrant,
    entries: BTreeMap<Option<PortableRelativePath>, MemoryEntry>,
}

#[derive(Default)]
struct MemoryState {
    owned_windows: BTreeMap<WindowId, WindowSnapshot>,
    external_windows: BTreeMap<WindowId, WindowSnapshot>,
    primary_owned: Option<WindowId>,
    global_pointer: PhysicalPosition,
    grants: BTreeMap<FileGrantId, MemoryGrant>,
    dialog_results: VecDeque<Vec<FileGrantId>>,
    next_grant: u64,
}

/// Deterministic backend used by tests, replay fixtures, and headless tooling.
pub struct MemoryDesktopBackend {
    capabilities: DesktopCapabilities,
    state: Mutex<MemoryState>,
}

impl Default for MemoryDesktopBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryDesktopBackend {
    pub fn new() -> Self {
        let features = all_features().map(|feature| FeatureSupport {
            feature,
            level: SupportLevel::Supported,
            permissions: Vec::new(),
            detail: "deterministic in-memory backend".to_owned(),
        });
        Self {
            capabilities: DesktopCapabilities::new(PlatformKind::Other, features),
            state: Mutex::new(MemoryState::default()),
        }
    }

    pub fn insert_window(&self, mut window: WindowSnapshot) {
        let mut state = self.state();
        match window.scope {
            WindowScope::Owned => {
                if state.primary_owned.is_none() {
                    state.primary_owned = Some(window.id.clone());
                }
                window.scope = WindowScope::Owned;
                state.owned_windows.insert(window.id.clone(), window);
            }
            WindowScope::External => {
                state.external_windows.insert(window.id.clone(), window);
            }
        }
    }

    pub fn insert_file(
        &self,
        display_name: impl Into<String>,
        access: GrantAccess,
        bytes: Vec<u8>,
    ) -> FileGrant {
        let mut state = self.state();
        let grant = next_memory_grant(
            &mut state,
            display_name.into(),
            access,
            GrantOrigin::UserSelection,
            FileEntryKind::File,
        );
        state.grants.insert(
            grant.id.clone(),
            MemoryGrant {
                public: grant.clone(),
                entries: BTreeMap::from([(
                    None,
                    MemoryEntry {
                        kind: FileEntryKind::File,
                        bytes,
                    },
                )]),
            },
        );
        grant
    }

    pub fn insert_directory(
        &self,
        display_name: impl Into<String>,
        access: GrantAccess,
    ) -> FileGrant {
        let mut state = self.state();
        let grant = next_memory_grant(
            &mut state,
            display_name.into(),
            access,
            GrantOrigin::UserSelection,
            FileEntryKind::Directory,
        );
        state.grants.insert(
            grant.id.clone(),
            MemoryGrant {
                public: grant.clone(),
                entries: BTreeMap::from([(
                    None,
                    MemoryEntry {
                        kind: FileEntryKind::Directory,
                        bytes: Vec::new(),
                    },
                )]),
            },
        );
        grant
    }

    pub fn insert_directory_entry(
        &self,
        grant: &FileGrantId,
        relative: PortableRelativePath,
        kind: FileEntryKind,
        bytes: Vec<u8>,
    ) -> Result<(), DesktopError> {
        let mut state = self.state();
        let memory_grant = state
            .grants
            .get_mut(grant)
            .ok_or_else(|| stale_grant(grant))?;
        if memory_grant.public.entry_kind != FileEntryKind::Directory {
            return Err(DesktopError::InvalidArgument {
                field: "grant".to_owned(),
                detail: "child entries require a directory grant".to_owned(),
            });
        }
        memory_grant
            .entries
            .insert(Some(relative), MemoryEntry { kind, bytes });
        Ok(())
    }

    pub fn queue_dialog_result(&self, grants: impl IntoIterator<Item = FileGrantId>) {
        self.state()
            .dialog_results
            .push_back(grants.into_iter().collect());
    }

    fn state(&self) -> MutexGuard<'_, MemoryState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn resolve_owned_id(
        state: &MemoryState,
        target: &WindowTarget,
    ) -> Result<WindowId, DesktopError> {
        match target {
            WindowTarget::PrimaryOwned => {
                state
                    .primary_owned
                    .clone()
                    .ok_or_else(|| DesktopError::NotFound {
                        resource: "primary owned window".to_owned(),
                    })
            }
            WindowTarget::Owned(id) => Ok(id.clone()),
            WindowTarget::External(id) => Err(DesktopError::InvalidArgument {
                field: "target".to_owned(),
                detail: format!("external window `{id}` cannot be used by the owned adapter"),
            }),
        }
    }

    fn execute(&self, request: DesktopRequest) -> Result<DesktopResponse, DesktopError> {
        match request {
            DesktopRequest::Capabilities => {
                Ok(DesktopResponse::Capabilities(self.capabilities.clone()))
            }
            DesktopRequest::OwnedWindow(request) => self
                .execute_owned_window(request)
                .map(DesktopResponse::OwnedWindow),
            DesktopRequest::ExternalWindow(request) => self
                .execute_external_window(request)
                .map(DesktopResponse::ExternalWindow),
            DesktopRequest::OwnedCursor(request) => {
                validate_owned_cursor_target(&self.state(), &request)?;
                Ok(DesktopResponse::OwnedCursorApplied)
            }
            DesktopRequest::GlobalPointer(request) => Ok(DesktopResponse::GlobalPointer(
                self.execute_global_pointer(&request),
            )),
            DesktopRequest::UserFile(request) => self
                .execute_user_file(request)
                .map(DesktopResponse::UserFile),
        }
    }

    fn execute_owned_window(
        &self,
        request: OwnedWindowRequest,
    ) -> Result<OwnedWindowResponse, DesktopError> {
        let mut state = self.state();
        match request {
            OwnedWindowRequest::List => Ok(OwnedWindowResponse::Windows(
                state.owned_windows.values().cloned().collect(),
            )),
            OwnedWindowRequest::Get { target } => {
                let id = Self::resolve_owned_id(&state, &target)?;
                state
                    .owned_windows
                    .get(&id)
                    .cloned()
                    .map(OwnedWindowResponse::Window)
                    .ok_or_else(|| stale_window(&id))
            }
            OwnedWindowRequest::SetTitle { target, title } => {
                let id = Self::resolve_owned_id(&state, &target)?;
                state
                    .owned_windows
                    .get_mut(&id)
                    .ok_or_else(|| stale_window(&id))?
                    .title = Some(title);
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::SetVisible { target, visible } => {
                let id = Self::resolve_owned_id(&state, &target)?;
                state
                    .owned_windows
                    .get_mut(&id)
                    .ok_or_else(|| stale_window(&id))?
                    .visible = Some(visible);
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::SetMode { target, mode } => {
                let id = Self::resolve_owned_id(&state, &target)?;
                state
                    .owned_windows
                    .get_mut(&id)
                    .ok_or_else(|| stale_window(&id))?
                    .mode = mode;
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::SetBounds { target, bounds } => {
                let id = Self::resolve_owned_id(&state, &target)?;
                state
                    .owned_windows
                    .get_mut(&id)
                    .ok_or_else(|| stale_window(&id))?
                    .bounds = Some(bounds);
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::RequestFocus { target } => {
                let id = Self::resolve_owned_id(&state, &target)?;
                if !state.owned_windows.contains_key(&id) {
                    return Err(stale_window(&id));
                }
                state
                    .owned_windows
                    .values_mut()
                    .for_each(|window| window.focused = Some(window.id == id));
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::RequestClose { target } => {
                let id = Self::resolve_owned_id(&state, &target)?;
                state
                    .owned_windows
                    .remove(&id)
                    .ok_or_else(|| stale_window(&id))?;
                if state.primary_owned.as_ref() == Some(&id) {
                    state.primary_owned = state.owned_windows.keys().next().cloned();
                }
                Ok(OwnedWindowResponse::Applied)
            }
        }
    }

    fn execute_external_window(
        &self,
        request: ExternalWindowRequest,
    ) -> Result<ExternalWindowResponse, DesktopError> {
        let mut state = self.state();
        match request {
            ExternalWindowRequest::List => Ok(ExternalWindowResponse::Windows(
                state.external_windows.values().cloned().collect(),
            )),
            ExternalWindowRequest::Get { id } => state
                .external_windows
                .get(&id)
                .cloned()
                .map(ExternalWindowResponse::Window)
                .ok_or_else(|| stale_window(&id)),
            ExternalWindowRequest::Activate { id } => {
                if !state.external_windows.contains_key(&id) {
                    return Err(stale_window(&id));
                }
                state
                    .external_windows
                    .values_mut()
                    .for_each(|window| window.focused = Some(window.id == id));
                Ok(ExternalWindowResponse::Applied)
            }
            ExternalWindowRequest::SetBounds { id, bounds } => {
                state
                    .external_windows
                    .get_mut(&id)
                    .ok_or_else(|| stale_window(&id))?
                    .bounds = Some(bounds);
                Ok(ExternalWindowResponse::Applied)
            }
            ExternalWindowRequest::RequestClose { id } => {
                state
                    .external_windows
                    .remove(&id)
                    .ok_or_else(|| stale_window(&id))?;
                Ok(ExternalWindowResponse::Applied)
            }
        }
    }

    fn execute_global_pointer(&self, request: &GlobalPointerRequest) -> GlobalPointerResponse {
        let mut state = self.state();
        match request {
            GlobalPointerRequest::Position => GlobalPointerResponse::Position(PointerPosition {
                position: state.global_pointer,
                space: PointerCoordinateSpace::GlobalPhysical,
            }),
            GlobalPointerRequest::Move { position } => {
                state.global_pointer = *position;
                GlobalPointerResponse::Applied
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn execute_user_file(
        &self,
        request: UserFileRequest,
    ) -> Result<UserFileResponse, DesktopError> {
        let mut state = self.state();
        match request {
            UserFileRequest::ShowDialog(dialog) => {
                let ids = state
                    .dialog_results
                    .pop_front()
                    .ok_or(DesktopError::UserCancelled)?;
                let grants = ids
                    .into_iter()
                    .map(|id| {
                        state
                            .grants
                            .get(&id)
                            .map(|grant| grant.public.clone())
                            .ok_or_else(|| stale_grant(&id))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                match dialog.mode {
                    FileDialogMode::OpenFile
                    | FileDialogMode::SaveFile
                    | FileDialogMode::PickDirectory
                        if grants.len() != 1 =>
                    {
                        Err(DesktopError::ResponseMismatch {
                            request: "single-selection file dialog".to_owned(),
                        })
                    }
                    _ => Ok(UserFileResponse::Grants(grants)),
                }
            }
            UserFileRequest::GrantKnownDirectory {
                directory,
                access,
                lifetime,
            } => {
                let mut grant = next_memory_grant(
                    &mut state,
                    format!("{directory:?}"),
                    access,
                    GrantOrigin::KnownDirectory(directory),
                    FileEntryKind::Directory,
                );
                grant.lifetime = lifetime;
                state.grants.insert(
                    grant.id.clone(),
                    MemoryGrant {
                        public: grant.clone(),
                        entries: BTreeMap::from([(
                            None,
                            MemoryEntry {
                                kind: FileEntryKind::Directory,
                                bytes: Vec::new(),
                            },
                        )]),
                    },
                );
                Ok(UserFileResponse::Grants(vec![grant]))
            }
            UserFileRequest::ReadText { path } => {
                let entry = readable_entry(&state, &path)?;
                if entry.kind != FileEntryKind::File {
                    return Err(expected_file());
                }
                String::from_utf8(entry.bytes.clone())
                    .map(UserFileResponse::Text)
                    .map_err(|_| DesktopError::Io {
                        operation: "read_text".to_owned(),
                        detail: "invalid_utf8".to_owned(),
                    })
            }
            UserFileRequest::ReadBytes { path } => {
                let entry = readable_entry(&state, &path)?;
                if entry.kind != FileEntryKind::File {
                    return Err(expected_file());
                }
                Ok(UserFileResponse::Bytes(entry.bytes.clone()))
            }
            UserFileRequest::WriteText { path, text } => {
                let entry = writable_entry(&mut state, &path)?;
                if entry.kind != FileEntryKind::File {
                    return Err(expected_file());
                }
                entry.bytes = text.into_bytes();
                Ok(UserFileResponse::Applied)
            }
            UserFileRequest::WriteBytes { path, bytes } => {
                let entry = writable_entry(&mut state, &path)?;
                if entry.kind != FileEntryKind::File {
                    return Err(expected_file());
                }
                entry.bytes = bytes;
                Ok(UserFileResponse::Applied)
            }
            UserFileRequest::Metadata { path } => {
                let memory_grant = grant(&state, &path.grant)?;
                ensure_read_access(memory_grant)?;
                let entry = memory_grant.entries.get(&path.relative).ok_or_else(|| {
                    DesktopError::NotFound {
                        resource: "granted path".to_owned(),
                    }
                })?;
                Ok(UserFileResponse::Metadata(memory_metadata(
                    entry,
                    memory_grant.public.access,
                )))
            }
            UserFileRequest::ListDirectory { path } => {
                ensure_read_access(grant(&state, &path.grant)?)?;
                let base = entry(&state, &path)?;
                if base.kind != FileEntryKind::Directory {
                    return Err(DesktopError::InvalidArgument {
                        field: "path".to_owned(),
                        detail: "directory listing requires a directory".to_owned(),
                    });
                }
                let memory_grant = grant(&state, &path.grant)?;
                let entries = list_directory_entries(memory_grant, path.relative.as_ref());
                Ok(UserFileResponse::DirectoryEntries(entries))
            }
            UserFileRequest::Revoke { grant } => {
                state
                    .grants
                    .remove(&grant)
                    .ok_or_else(|| stale_grant(&grant))?;
                Ok(UserFileResponse::Applied)
            }
        }
    }
}

impl DesktopBackend for MemoryDesktopBackend {
    fn execution_lane(&self, request: &DesktopRequest) -> ExecutionLane {
        match request {
            DesktopRequest::OwnedWindow(_)
            | DesktopRequest::OwnedCursor(_)
            | DesktopRequest::UserFile(UserFileRequest::ShowDialog(_)) => {
                ExecutionLane::HostMainThread
            }
            DesktopRequest::Capabilities
            | DesktopRequest::ExternalWindow(_)
            | DesktopRequest::GlobalPointer(_)
            | DesktopRequest::UserFile(_) => ExecutionLane::AnyThread,
        }
    }

    fn submit(&self, _task: DesktopTaskId, request: DesktopRequest) -> BackendSubmission {
        BackendSubmission::Completed(self.execute(request))
    }
}

fn all_features() -> impl Iterator<Item = DesktopFeature> {
    [
        DesktopFeature::OwnedWindowObserve,
        DesktopFeature::OwnedWindowControl,
        DesktopFeature::OwnedWindowAbsolutePosition,
        DesktopFeature::OwnedCursorControl,
        DesktopFeature::UserFileDialog,
        DesktopFeature::KnownDirectoryGrant,
        DesktopFeature::GrantedFileIo,
        DesktopFeature::PersistentFileGrant,
        DesktopFeature::ExternalWindowObserve,
        DesktopFeature::ExternalWindowControl,
        DesktopFeature::GlobalPointerObserve,
        DesktopFeature::GlobalPointerControl,
    ]
    .into_iter()
}

fn next_memory_grant(
    state: &mut MemoryState,
    display_name: String,
    access: GrantAccess,
    origin: GrantOrigin,
    entry_kind: FileEntryKind,
) -> FileGrant {
    state.next_grant = state.next_grant.saturating_add(1).max(1);
    let id = FileGrantId::new(format!("memory-grant-{}", state.next_grant))
        .expect("generated identifier is valid");
    FileGrant {
        id,
        display_name,
        access,
        lifetime: GrantLifetime::Session,
        origin,
        entry_kind,
    }
}

fn grant<'a>(state: &'a MemoryState, id: &FileGrantId) -> Result<&'a MemoryGrant, DesktopError> {
    state.grants.get(id).ok_or_else(|| stale_grant(id))
}

fn grant_mut<'a>(
    state: &'a mut MemoryState,
    id: &FileGrantId,
) -> Result<&'a mut MemoryGrant, DesktopError> {
    state.grants.get_mut(id).ok_or_else(|| stale_grant(id))
}

fn entry<'a>(state: &'a MemoryState, path: &GrantPath) -> Result<&'a MemoryEntry, DesktopError> {
    grant(state, &path.grant)?
        .entries
        .get(&path.relative)
        .ok_or_else(|| DesktopError::NotFound {
            resource: "granted path".to_owned(),
        })
}

fn readable_entry<'a>(
    state: &'a MemoryState,
    path: &GrantPath,
) -> Result<&'a MemoryEntry, DesktopError> {
    let memory_grant = grant(state, &path.grant)?;
    ensure_read_access(memory_grant)?;
    memory_grant
        .entries
        .get(&path.relative)
        .ok_or_else(|| DesktopError::NotFound {
            resource: "granted path".to_owned(),
        })
}

fn writable_entry<'a>(
    state: &'a mut MemoryState,
    path: &GrantPath,
) -> Result<&'a mut MemoryEntry, DesktopError> {
    let memory_grant = grant_mut(state, &path.grant)?;
    ensure_write_access(memory_grant)?;
    memory_grant
        .entries
        .get_mut(&path.relative)
        .ok_or_else(|| DesktopError::NotFound {
            resource: "granted path".to_owned(),
        })
}

fn ensure_read_access(grant: &MemoryGrant) -> Result<(), DesktopError> {
    if grant.public.access.permits_read() {
        Ok(())
    } else {
        Err(DesktopError::PermissionDenied {
            permission: grant_permission(grant.public.origin),
            detail: "grant does not permit reading".to_owned(),
        })
    }
}

fn ensure_write_access(grant: &MemoryGrant) -> Result<(), DesktopError> {
    if grant.public.access.permits_write() {
        Ok(())
    } else {
        Err(DesktopError::PermissionDenied {
            permission: grant_permission(grant.public.origin),
            detail: "grant does not permit writing".to_owned(),
        })
    }
}

const fn grant_permission(origin: GrantOrigin) -> PermissionKind {
    match origin {
        GrantOrigin::KnownDirectory(_) => PermissionKind::KnownDirectoryAccess,
        GrantOrigin::UserSelection | GrantOrigin::Restored => PermissionKind::UserFileSelection,
    }
}

fn memory_metadata(entry: &MemoryEntry, access: GrantAccess) -> FileMetadata {
    FileMetadata {
        entry_kind: entry.kind,
        byte_len: (entry.kind == FileEntryKind::File)
            .then_some(u64::try_from(entry.bytes.len()).unwrap_or(u64::MAX)),
        modified_unix_millis: None,
        readonly: Some(!access.permits_write()),
    }
}

fn list_directory_entries(
    grant: &MemoryGrant,
    base: Option<&PortableRelativePath>,
) -> Vec<DirectoryEntry> {
    let prefix = base.map_or_else(String::new, |base| format!("{base}/"));
    grant
        .entries
        .iter()
        .filter_map(|(relative, entry)| {
            let relative = relative.as_ref()?;
            let remainder = relative.as_str().strip_prefix(&prefix)?;
            (!remainder.contains('/')).then(|| DirectoryEntry {
                relative: relative.clone(),
                display_name: remainder.to_owned(),
                metadata: memory_metadata(entry, grant.public.access),
            })
        })
        .collect()
}

fn validate_owned_cursor_target(
    state: &MemoryState,
    request: &OwnedCursorRequest,
) -> Result<(), DesktopError> {
    let target = match request {
        OwnedCursorRequest::SetIcon { target, .. }
        | OwnedCursorRequest::SetVisible { target, .. }
        | OwnedCursorRequest::SetGrab { target, .. }
        | OwnedCursorRequest::SetPosition { target, .. } => target,
    };
    let id = MemoryDesktopBackend::resolve_owned_id(state, target)?;
    state
        .owned_windows
        .contains_key(&id)
        .then_some(())
        .ok_or_else(|| stale_window(&id))
}

fn expected_file() -> DesktopError {
    DesktopError::InvalidArgument {
        field: "path".to_owned(),
        detail: "operation requires a file".to_owned(),
    }
}

fn stale_window(id: &WindowId) -> DesktopError {
    DesktopError::StaleHandle {
        handle: id.to_string(),
    }
}

fn stale_grant(id: &FileGrantId) -> DesktopError {
    DesktopError::StaleHandle {
        handle: id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_desktop_contract::{PhysicalRect, PhysicalSize, ScaleFactor, WindowMode};

    fn window(id: &str) -> WindowSnapshot {
        WindowSnapshot {
            id: WindowId::new(id).expect("valid id"),
            scope: WindowScope::Owned,
            title: Some("Arcweft".to_owned()),
            application_name: Some("arcweft-player-native".to_owned()),
            process_id: Some(1),
            bounds: Some(PhysicalRect {
                position: PhysicalPosition::default(),
                size: PhysicalSize {
                    width: 1280,
                    height: 720,
                },
            }),
            scale_factor: Some(ScaleFactor::ONE),
            mode: WindowMode::Normal,
            visible: Some(true),
            focused: Some(true),
        }
    }

    #[test]
    fn owned_window_mutation_is_deterministic() {
        let backend = MemoryDesktopBackend::new();
        backend.insert_window(window("owned-1"));
        let response = backend.execute(DesktopRequest::OwnedWindow(OwnedWindowRequest::SetTitle {
            target: WindowTarget::PrimaryOwned,
            title: "Changed".to_owned(),
        }));
        assert_eq!(
            response,
            Ok(DesktopResponse::OwnedWindow(OwnedWindowResponse::Applied))
        );
    }

    #[test]
    fn grants_enforce_access() {
        let backend = MemoryDesktopBackend::new();
        let grant = backend.insert_file("readonly.txt", GrantAccess::Read, b"hello".to_vec());
        let error = backend
            .execute(DesktopRequest::UserFile(UserFileRequest::WriteText {
                path: GrantPath::root(grant.id),
                text: "no".to_owned(),
            }))
            .expect_err("write must fail");
        assert!(matches!(error, DesktopError::PermissionDenied { .. }));
    }

    #[test]
    fn directory_listing_is_relative_to_the_grant() {
        let backend = MemoryDesktopBackend::new();
        let grant = backend.insert_directory("docs", GrantAccess::Read);
        backend
            .insert_directory_entry(
                &grant.id,
                PortableRelativePath::new("chapter.txt").expect("valid path"),
                FileEntryKind::File,
                b"text".to_vec(),
            )
            .expect("entry inserted");
        let response = backend
            .execute(DesktopRequest::UserFile(UserFileRequest::ListDirectory {
                path: GrantPath::root(grant.id),
            }))
            .expect("listing succeeds");
        assert!(matches!(
            response,
            DesktopResponse::UserFile(UserFileResponse::DirectoryEntries(entries))
                if entries.len() == 1 && entries[0].display_name == "chapter.txt"
        ));
    }
}
