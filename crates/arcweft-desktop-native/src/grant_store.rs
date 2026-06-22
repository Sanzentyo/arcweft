use crate::persistent_grants::{
    CaptureTarget, PersistentGrantLease, PersistentGrantServices, RestoredGrantRoot,
    RestoredPersistentGrant,
};
use arcweft_desktop_contract::{
    DesktopError, DesktopFeature, FileEntryKind, FileGrant, FileGrantId, GrantAccess,
    GrantLifetime, GrantOrigin, GrantPath, PermissionKind, PlatformKind,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

#[derive(Clone)]
enum GrantRoot {
    Exact { path: PathBuf, parent: PathBuf },
    Directory(PathBuf),
}

#[derive(Clone)]
struct NativeGrant {
    public: FileGrant,
    root: GrantRoot,
    issued_permission: PermissionKind,
    lease: Option<PersistentGrantLease>,
}

struct InspectedGrantTarget {
    root: GrantRoot,
    capture_target: CaptureTarget,
    display_name: String,
    entry_kind: FileEntryKind,
}

#[derive(Clone, Copy, Default)]
struct GrantLifecycle {
    epoch: u64,
    in_flight_restores: u32,
    revoking: bool,
    revoked: bool,
}

#[derive(Default)]
struct GrantState {
    grants: BTreeMap<FileGrantId, NativeGrant>,
    lifecycle: BTreeMap<FileGrantId, GrantLifecycle>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolveIntent {
    Read,
    Write,
    Metadata,
    ListDirectory,
}

pub(crate) struct ResolvedGrant {
    pub path: PathBuf,
    _lease: Option<PersistentGrantLease>,
}

/// Process-local path authority store.
///
/// The runtime sees only an unguessable `FileGrantId`. Existing targets and
/// their parent directories are canonicalized immediately before I/O to reject
/// symlink escapes. Destructive recursive operations are omitted from this
/// reference backend to keep the residual TOCTOU surface small.
pub(crate) struct GrantStore {
    platform: PlatformKind,
    persistent: Option<PersistentGrantServices>,
    state: Mutex<GrantState>,
}

impl GrantStore {
    pub fn new(
        platform: PlatformKind,
        persistent: Option<PersistentGrantServices>,
    ) -> Result<Self, DesktopError> {
        if persistent
            .as_ref()
            .is_some_and(|services| services.platform() != platform)
        {
            return Err(DesktopError::BackendUnavailable {
                backend: "persistent_file_grants".to_owned(),
                detail: "persistent_grant_platform_mismatch".to_owned(),
            });
        }
        Ok(Self {
            platform,
            persistent,
            state: Mutex::new(GrantState::default()),
        })
    }

    pub const fn has_persistent_store(&self) -> bool {
        self.persistent.is_some()
    }

    pub fn insert_path(
        &self,
        path: impl AsRef<Path>,
        access: GrantAccess,
        lifetime: GrantLifetime,
        origin: GrantOrigin,
    ) -> Result<FileGrant, DesktopError> {
        self.ensure_persistent_supported(lifetime)?;

        let path = path.as_ref();
        let (root, capture_target, entry_kind) = match std::fs::metadata(path) {
            Ok(metadata) => {
                let canonical = std::fs::canonicalize(path)
                    .map_err(|error| DesktopError::sanitized_io("canonicalize_grant", &error))?;
                if metadata.is_dir() {
                    (
                        GrantRoot::Directory(canonical.clone()),
                        CaptureTarget::Directory(canonical),
                        FileEntryKind::Directory,
                    )
                } else {
                    let parent = canonical.parent().map(Path::to_path_buf).ok_or_else(|| {
                        DesktopError::InvalidArgument {
                            field: "selected_path".to_owned(),
                            detail: "selected target has no parent directory".to_owned(),
                        }
                    })?;
                    let kind = if metadata.is_file() {
                        FileEntryKind::File
                    } else {
                        FileEntryKind::Other
                    };
                    (
                        GrantRoot::Exact {
                            path: canonical.clone(),
                            parent: parent.clone(),
                        },
                        CaptureTarget::Exact {
                            path: canonical,
                            parent,
                        },
                        kind,
                    )
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && access.permits_write() =>
            {
                let parent = path.parent().ok_or_else(|| DesktopError::InvalidArgument {
                    field: "selected_path".to_owned(),
                    detail: "save target has no parent directory".to_owned(),
                })?;
                let file_name = path
                    .file_name()
                    .ok_or_else(|| DesktopError::InvalidArgument {
                        field: "selected_path".to_owned(),
                        detail: "save target has no file name".to_owned(),
                    })?;
                let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
                    DesktopError::sanitized_io("canonicalize_save_parent", &error)
                })?;
                (
                    GrantRoot::Exact {
                        path: canonical_parent.join(file_name),
                        parent: canonical_parent.clone(),
                    },
                    CaptureTarget::Exact {
                        path: canonical_parent.join(file_name),
                        parent: canonical_parent,
                    },
                    FileEntryKind::File,
                )
            }
            Err(error) => {
                return Err(DesktopError::sanitized_io("inspect_grant", &error));
            }
        };

        let display_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .map_or_else(|| "selected".to_owned(), ToOwned::to_owned);
        self.insert_inspected(
            InspectedGrantTarget {
                root,
                capture_target,
                display_name,
                entry_kind,
            },
            access,
            lifetime,
            origin,
        )
    }

    fn insert_inspected(
        &self,
        target: InspectedGrantTarget,
        access: GrantAccess,
        lifetime: GrantLifetime,
        origin: GrantOrigin,
    ) -> Result<FileGrant, DesktopError> {
        let InspectedGrantTarget {
            root,
            capture_target,
            display_name,
            entry_kind,
        } = target;
        if !origin.is_valid_issuance_for(entry_kind) {
            return Err(DesktopError::InvalidArgument {
                field: "origin".to_owned(),
                detail: "grant origin is not valid for the inspected entry kind".to_owned(),
            });
        }
        let issued_permission =
            origin
                .issued_permission()
                .ok_or_else(|| DesktopError::InvalidArgument {
                    field: "origin".to_owned(),
                    detail: "restored origin cannot issue a new grant".to_owned(),
                })?;
        let id = self.allocate_id(lifetime)?;
        let public = FileGrant {
            id: id.clone(),
            display_name,
            access,
            lifetime,
            origin,
            entry_kind,
        };

        let lease = if lifetime.is_persistent() {
            let services = self
                .persistent
                .as_ref()
                .ok_or_else(|| self.persistent_unsupported())?;
            services.persist(&public, &capture_target)?
        } else {
            None
        };

        self.state().grants.insert(
            id,
            NativeGrant {
                public: public.clone(),
                root,
                issued_permission,
                lease,
            },
        );
        Ok(public)
    }

    pub fn resolve(
        &self,
        path: &GrantPath,
        intent: ResolveIntent,
    ) -> Result<ResolvedGrant, DesktopError> {
        let grant = self.grant_for(&path.grant)?;
        check_access(&grant.public, grant.issued_permission, intent)?;

        let resolved = match &grant.root {
            GrantRoot::Exact {
                path: exact,
                parent,
            } => {
                if path.relative.is_some() {
                    return Err(DesktopError::InvalidArgument {
                        field: "relative".to_owned(),
                        detail: "a single-file grant has no descendants".to_owned(),
                    });
                }
                resolve_exact(exact, parent, intent)?
            }
            GrantRoot::Directory(root) => {
                let candidate = path.relative.as_ref().map_or_else(
                    || root.clone(),
                    |relative| {
                        relative
                            .components()
                            .fold(root.clone(), |candidate, component| {
                                candidate.join(component)
                            })
                    },
                );
                resolve_beneath(root, &candidate, intent, grant.issued_permission)?
            }
        };

        Ok(ResolvedGrant {
            path: resolved,
            _lease: grant.lease.clone(),
        })
    }

    pub fn revoke(&self, id: &FileGrantId) -> Result<(), DesktopError> {
        if id.generated_lifetime() != Some(GrantLifetime::Persistent) {
            return self
                .state()
                .grants
                .remove(id)
                .map(|_| ())
                .ok_or_else(|| Self::stale(id));
        }
        let services = self.persistent.as_ref().ok_or_else(|| Self::stale(id))?;

        let (previous, evicted) =
            {
                let mut state = self.state();
                let previous = state.lifecycle.get(id).copied().unwrap_or_default();
                if previous.revoking || previous.revoked {
                    return Err(Self::stale(id));
                }
                let epoch = previous.epoch.checked_add(1).ok_or_else(|| {
                    DesktopError::BackendUnavailable {
                        backend: "persistent_file_grants".to_owned(),
                        detail: "persistent_grant_lifecycle_epoch_exhausted".to_owned(),
                    }
                })?;
                state.lifecycle.insert(
                    id.clone(),
                    GrantLifecycle {
                        epoch,
                        in_flight_restores: previous.in_flight_restores,
                        revoking: true,
                        revoked: false,
                    },
                );
                (previous, state.grants.remove(id))
            };
        drop(evicted);

        let result = services.revoke(id);
        let mut state = self.state();
        let mut prune = false;
        if let Some(lifecycle) = state.lifecycle.get_mut(id) {
            if result.is_ok() {
                lifecycle.revoking = false;
                lifecycle.revoked = true;
            } else {
                *lifecycle = previous;
            }
            prune = lifecycle.in_flight_restores == 0;
        }
        if prune {
            state.lifecycle.remove(id);
        }
        result
    }

    fn allocate_id(&self, lifetime: GrantLifetime) -> Result<FileGrantId, DesktopError> {
        for _ in 0..8 {
            let mut entropy = vec![0_u8; lifetime.entropy_bytes()];
            getrandom::fill(&mut entropy).map_err(|_| DesktopError::BackendUnavailable {
                backend: "system_random".to_owned(),
                detail: "system_random_unavailable".to_owned(),
            })?;
            let id = FileGrantId::from_entropy(lifetime, &entropy).map_err(|_| {
                DesktopError::BackendUnavailable {
                    backend: "persistent_file_grants".to_owned(),
                    detail: "generated_file_grant_id_invalid".to_owned(),
                }
            })?;
            if !self.state().grants.contains_key(&id) {
                return Ok(id);
            }
        }
        Err(DesktopError::BackendUnavailable {
            backend: "system_random".to_owned(),
            detail: "repeated opaque grant identifier collision".to_owned(),
        })
    }

    fn grant_for(&self, id: &FileGrantId) -> Result<NativeGrant, DesktopError> {
        {
            let state = self.state();
            if let Some(grant) = state.grants.get(id).cloned() {
                return Ok(grant);
            }
        }
        if id.generated_lifetime() != Some(GrantLifetime::Persistent) {
            return Err(Self::stale(id));
        }
        let services = self.persistent.as_ref().ok_or_else(|| Self::stale(id))?;

        let restore_epoch = {
            let mut state = self.state();
            if let Some(grant) = state.grants.get(id).cloned() {
                return Ok(grant);
            }
            let lifecycle = state.lifecycle.entry(id.clone()).or_default();
            if lifecycle.revoking || lifecycle.revoked {
                return Err(Self::stale(id));
            }
            lifecycle.in_flight_restores =
                lifecycle.in_flight_restores.checked_add(1).ok_or_else(|| {
                    DesktopError::BackendUnavailable {
                        backend: "persistent_file_grants".to_owned(),
                        detail: "persistent_grant_restore_count_exhausted".to_owned(),
                    }
                })?;
            lifecycle.epoch
        };

        let restored = services.restore(id).map(Self::from_restored);
        let mut state = self.state();
        let (publish, prune) =
            {
                let lifecycle = state.lifecycle.get_mut(id).ok_or_else(|| {
                    DesktopError::BackendUnavailable {
                        backend: "persistent_file_grants".to_owned(),
                        detail: "persistent_grant_restore_state_missing".to_owned(),
                    }
                })?;
                lifecycle.in_flight_restores = lifecycle
                    .in_flight_restores
                    .checked_sub(1)
                    .ok_or_else(|| DesktopError::BackendUnavailable {
                        backend: "persistent_file_grants".to_owned(),
                        detail: "persistent_grant_restore_count_underflow".to_owned(),
                    })?;
                (
                    !lifecycle.revoking && !lifecycle.revoked && lifecycle.epoch == restore_epoch,
                    lifecycle.in_flight_restores == 0 && !lifecycle.revoking,
                )
            };

        let (result, release_after_unlock) = match restored {
            Ok(native) if publish => {
                let cached = state
                    .grants
                    .entry(id.clone())
                    .or_insert_with(|| native.clone())
                    .clone();
                (Ok(cached), Some(native))
            }
            Ok(native) => (Err(Self::stale(id)), Some(native)),
            Err(error) => (Err(error), None),
        };
        if prune {
            state.lifecycle.remove(id);
        }
        drop(state);
        drop(release_after_unlock);
        result
    }

    fn from_restored(restored: RestoredPersistentGrant) -> NativeGrant {
        let root = match restored.root {
            RestoredGrantRoot::Exact { path, parent } => GrantRoot::Exact { path, parent },
            RestoredGrantRoot::Directory(path) => GrantRoot::Directory(path),
        };
        NativeGrant {
            public: restored.public,
            root,
            issued_permission: restored.issued_permission,
            lease: restored.lease,
        }
    }

    fn stale(id: &FileGrantId) -> DesktopError {
        DesktopError::StaleHandle {
            handle: id.to_string(),
        }
    }

    fn ensure_persistent_supported(&self, lifetime: GrantLifetime) -> Result<(), DesktopError> {
        if lifetime.is_persistent() && self.persistent.is_none() {
            Err(self.persistent_unsupported())
        } else {
            Ok(())
        }
    }

    fn persistent_unsupported(&self) -> DesktopError {
        DesktopError::Unsupported {
            feature: DesktopFeature::PersistentFileGrant,
            platform: self.platform,
            detail: "persistent native grants require host-opened persistent grant services"
                .to_owned(),
        }
    }

    fn state(&self) -> MutexGuard<'_, GrantState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for GrantStore {
    fn default() -> Self {
        Self {
            platform: PlatformKind::Other,
            persistent: None,
            state: Mutex::new(GrantState::default()),
        }
    }
}

#[cfg(test)]
fn next_native_grant_id(
    existing: &BTreeMap<FileGrantId, NativeGrant>,
) -> Result<FileGrantId, DesktopError> {
    for _ in 0..8 {
        let mut entropy = [0_u8; 16];
        getrandom::fill(&mut entropy).map_err(|_| DesktopError::BackendUnavailable {
            backend: "system_random".to_owned(),
            detail: "system_random_unavailable".to_owned(),
        })?;
        let id = FileGrantId::from_entropy(GrantLifetime::Session, &entropy).map_err(|_| {
            DesktopError::BackendUnavailable {
                backend: "system_random".to_owned(),
                detail: "generated_file_grant_id_invalid".to_owned(),
            }
        })?;
        if !existing.contains_key(&id) {
            return Ok(id);
        }
    }
    Err(DesktopError::BackendUnavailable {
        backend: "system_random".to_owned(),
        detail: "repeated opaque grant identifier collision".to_owned(),
    })
}

fn resolve_exact(
    exact: &Path,
    anchored_parent: &Path,
    intent: ResolveIntent,
) -> Result<PathBuf, DesktopError> {
    validate_exact_parent(exact, anchored_parent)?;
    match std::fs::symlink_metadata(exact) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(DesktopError::PermissionDenied {
            permission: PermissionKind::UserFileSelection,
            detail: "the granted file was replaced by a symbolic link".to_owned(),
        }),
        Ok(_) => {
            let canonical = std::fs::canonicalize(exact)
                .map_err(|error| DesktopError::sanitized_io("canonicalize_exact_grant", &error))?;
            if canonical == exact {
                Ok(exact.to_path_buf())
            } else {
                Err(DesktopError::PermissionDenied {
                    permission: PermissionKind::UserFileSelection,
                    detail: "the granted file now resolves through a replaced ancestor".to_owned(),
                })
            }
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound && intent == ResolveIntent::Write =>
        {
            Ok(exact.to_path_buf())
        }
        Err(error) => Err(DesktopError::sanitized_io("inspect_exact_grant", &error)),
    }
}

fn validate_exact_parent(exact: &Path, anchored_parent: &Path) -> Result<(), DesktopError> {
    if exact.parent() != Some(anchored_parent) {
        return Err(DesktopError::PermissionDenied {
            permission: PermissionKind::UserFileSelection,
            detail: "the granted file parent does not match its authority anchor".to_owned(),
        });
    }
    let current_parent = std::fs::canonicalize(anchored_parent)
        .map_err(|error| DesktopError::sanitized_io("canonicalize_exact_parent", &error))?;
    if current_parent == anchored_parent {
        Ok(())
    } else {
        Err(DesktopError::PermissionDenied {
            permission: PermissionKind::UserFileSelection,
            detail: "the granted file parent was replaced by a symbolic link or mount alias"
                .to_owned(),
        })
    }
}

fn resolve_beneath(
    root: &Path,
    candidate: &Path,
    intent: ResolveIntent,
    permission: PermissionKind,
) -> Result<PathBuf, DesktopError> {
    if intent == ResolveIntent::Write && !candidate.exists() {
        let parent = candidate
            .parent()
            .ok_or_else(|| DesktopError::InvalidArgument {
                field: "relative".to_owned(),
                detail: "write target has no parent".to_owned(),
            })?;
        let file_name = candidate
            .file_name()
            .ok_or_else(|| DesktopError::InvalidArgument {
                field: "relative".to_owned(),
                detail: "write target has no file name".to_owned(),
            })?;
        let canonical_parent = std::fs::canonicalize(parent)
            .map_err(|error| DesktopError::sanitized_io("canonicalize_write_parent", &error))?;
        ensure_beneath(root, &canonical_parent, permission)?;
        return Ok(canonical_parent.join(file_name));
    }

    let canonical = std::fs::canonicalize(candidate)
        .map_err(|error| DesktopError::sanitized_io("canonicalize_granted_path", &error))?;
    ensure_beneath(root, &canonical, permission)?;
    Ok(canonical)
}

fn ensure_beneath(
    root: &Path,
    candidate: &Path,
    permission: PermissionKind,
) -> Result<(), DesktopError> {
    if candidate.starts_with(root) {
        Ok(())
    } else {
        Err(DesktopError::PermissionDenied {
            permission,
            detail: "resolved path escaped the granted directory".to_owned(),
        })
    }
}

fn check_access(
    public: &FileGrant,
    issued_permission: PermissionKind,
    intent: ResolveIntent,
) -> Result<(), DesktopError> {
    let permitted = match intent {
        ResolveIntent::Read | ResolveIntent::Metadata | ResolveIntent::ListDirectory => {
            public.access.permits(GrantAccess::Read)
        }
        ResolveIntent::Write => public.access.permits(GrantAccess::Write),
    };
    if permitted {
        return Ok(());
    }
    Err(DesktopError::PermissionDenied {
        permission: issued_permission,
        detail: match intent {
            ResolveIntent::Write => "grant does not permit writing",
            ResolveIntent::Read | ResolveIntent::Metadata | ResolveIntent::ListDirectory => {
                "grant does not permit reading or metadata access"
            }
        }
        .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "arcweft-persistent-grant-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("test temp dir created");
        path
    }

    #[test]
    fn native_grant_ids_are_random_capability_tokens() {
        let existing = BTreeMap::new();
        let first = next_native_grant_id(&existing).expect("system randomness is available");
        let second = next_native_grant_id(&existing).expect("system randomness is available");
        assert_ne!(first, second);
        assert!(first.as_str().starts_with("native-grant-"));
        assert_eq!(first.as_str().len(), "native-grant-".len() + 32);
    }

    #[test]
    fn persistent_grants_require_a_host_store() {
        let store = GrantStore::new(PlatformKind::Windows, None).expect("store builds");
        let dir = temp_test_dir("requires-store");
        let error = store
            .insert_path(
                &dir,
                GrantAccess::Read,
                GrantLifetime::Persistent,
                GrantOrigin::UserSelection,
            )
            .expect_err("persistent grant needs a provider");
        assert!(matches!(
            error,
            DesktopError::Unsupported {
                feature: DesktopFeature::PersistentFileGrant,
                ..
            }
        ));
    }

    #[test]
    fn persistent_grants_are_saved_restored_and_revoked() {
        let dir = temp_test_dir("restore");
        let file = dir.join("save.txt");
        std::fs::write(&file, "kept").expect("test file written");

        let services = PersistentGrantServices::memory_for_tests(PlatformKind::Windows);
        let grant = GrantStore::new(PlatformKind::Windows, Some(services.clone_for_tests()))
            .expect("store builds")
            .insert_path(
                &file,
                GrantAccess::Read,
                GrantLifetime::Persistent,
                GrantOrigin::UserSelection,
            )
            .expect("persistent grant is stored");

        let restored = GrantStore::new(PlatformKind::Windows, Some(services))
            .expect("persistent grants restore");
        let resolved = restored
            .resolve(&GrantPath::root(grant.id.clone()), ResolveIntent::Read)
            .expect("restored grant resolves");
        assert_eq!(resolved.path, std::fs::canonicalize(&file).unwrap());

        restored
            .revoke(&grant.id)
            .expect("persistent revoke succeeds");
        assert!(matches!(
            restored.resolve(&GrantPath::root(grant.id), ResolveIntent::Read),
            Err(DesktopError::StaleHandle { .. })
        ));
    }
}
