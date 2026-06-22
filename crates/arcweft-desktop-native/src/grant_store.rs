use arcweft_desktop_contract::{
    DesktopError, DesktopFeature, FileEntryKind, FileGrant, FileGrantId, GrantAccess,
    GrantLifetime, GrantOrigin, GrantPath, PermissionKind, PlatformKind,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone)]
enum GrantRoot {
    Exact { path: PathBuf, parent: PathBuf },
    Directory(PathBuf),
}

#[derive(Clone)]
struct NativeGrant {
    public: FileGrant,
    root: GrantRoot,
}

#[derive(Default)]
struct GrantState {
    grants: BTreeMap<FileGrantId, NativeGrant>,
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
}

/// Persistable native file-grant root owned by an embedding host.
///
/// This type intentionally lives in the native adapter crate: native paths and
/// platform restoration tokens do not cross the portable desktop contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistentGrantRoot {
    Exact { path: PathBuf, parent: PathBuf },
    Directory(PathBuf),
}

/// Host-owned persistent grant metadata restored when a native backend starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentGrantRecord {
    pub id: FileGrantId,
    pub display_name: String,
    pub access: GrantAccess,
    pub entry_kind: FileEntryKind,
    pub root: PersistentGrantRoot,
}

/// Sealed persistent grant storage supplied by an embedding host.
///
/// Implementations may use OS bookmarks, credential stores, app-private files,
/// or test fixtures. The Arcweft runtime receives only `FileGrantId` values.
pub trait PersistentGrantStore: Send + Sync {
    fn load(&self) -> Result<Vec<PersistentGrantRecord>, DesktopError>;

    fn persist(&self, record: PersistentGrantRecord) -> Result<(), DesktopError>;

    fn revoke(&self, id: &FileGrantId) -> Result<(), DesktopError>;
}

/// Process-local path authority store.
///
/// The runtime sees only an unguessable `FileGrantId`. Existing targets and
/// their parent directories are canonicalized immediately before I/O to reject
/// symlink escapes. Destructive recursive operations are omitted from this
/// reference backend to keep the residual TOCTOU surface small.
pub(crate) struct GrantStore {
    platform: PlatformKind,
    persistent: Option<Arc<dyn PersistentGrantStore>>,
    state: Mutex<GrantState>,
}

impl GrantStore {
    pub fn new(
        platform: PlatformKind,
        persistent: Option<Arc<dyn PersistentGrantStore>>,
    ) -> Result<Self, DesktopError> {
        let grants = persistent.as_ref().map_or_else(
            || Ok(BTreeMap::new()),
            |store| restored_grants(store.load()?),
        )?;
        Ok(Self {
            platform,
            persistent,
            state: Mutex::new(GrantState { grants }),
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
        let (root, entry_kind) = match std::fs::metadata(path) {
            Ok(metadata) => {
                let canonical = std::fs::canonicalize(path)
                    .map_err(|error| DesktopError::sanitized_io("canonicalize_grant", &error))?;
                if metadata.is_dir() {
                    (GrantRoot::Directory(canonical), FileEntryKind::Directory)
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
        let mut state = self.state();
        let id = next_native_grant_id(&state.grants)?;
        let public = FileGrant {
            id: id.clone(),
            display_name,
            access,
            lifetime,
            origin,
            entry_kind,
        };
        if lifetime == GrantLifetime::Persistent {
            self.persist_grant(&public, &root)?;
        }
        state.grants.insert(
            id,
            NativeGrant {
                public: public.clone(),
                root,
            },
        );
        Ok(public)
    }

    pub fn resolve(
        &self,
        path: &GrantPath,
        intent: ResolveIntent,
    ) -> Result<ResolvedGrant, DesktopError> {
        let grant = self
            .state()
            .grants
            .get(&path.grant)
            .cloned()
            .ok_or_else(|| DesktopError::StaleHandle {
                handle: path.grant.to_string(),
            })?;
        check_access(&grant.public, intent)?;

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
                resolve_beneath(
                    root,
                    &candidate,
                    intent,
                    grant_permission(grant.public.origin),
                )?
            }
        };

        Ok(ResolvedGrant { path: resolved })
    }

    pub fn revoke(&self, id: &FileGrantId) -> Result<(), DesktopError> {
        let mut state = self.state();
        let grant = state
            .grants
            .get(id)
            .ok_or_else(|| DesktopError::StaleHandle {
                handle: id.to_string(),
            })?;
        if grant.public.lifetime == GrantLifetime::Persistent {
            self.persistent
                .as_ref()
                .ok_or_else(|| self.persistent_unsupported())?
                .revoke(id)?;
        }
        state
            .grants
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| DesktopError::StaleHandle {
                handle: id.to_string(),
            })
    }

    fn ensure_persistent_supported(&self, lifetime: GrantLifetime) -> Result<(), DesktopError> {
        if lifetime == GrantLifetime::Persistent && self.persistent.is_none() {
            Err(self.persistent_unsupported())
        } else {
            Ok(())
        }
    }

    fn persistent_unsupported(&self) -> DesktopError {
        DesktopError::Unsupported {
            feature: DesktopFeature::PersistentFileGrant,
            platform: self.platform,
            detail: "persistent native grants require a host-provided sealed-token store"
                .to_owned(),
        }
    }

    fn persist_grant(&self, public: &FileGrant, root: &GrantRoot) -> Result<(), DesktopError> {
        let record = PersistentGrantRecord {
            id: public.id.clone(),
            display_name: public.display_name.clone(),
            access: public.access,
            entry_kind: public.entry_kind,
            root: persistent_root(root),
        };
        self.persistent
            .as_ref()
            .ok_or_else(|| self.persistent_unsupported())?
            .persist(record)
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

fn restored_grants(
    records: Vec<PersistentGrantRecord>,
) -> Result<BTreeMap<FileGrantId, NativeGrant>, DesktopError> {
    records
        .into_iter()
        .try_fold(BTreeMap::new(), |mut grants, record| {
            if grants.contains_key(&record.id) {
                return Err(DesktopError::InvalidArgument {
                    field: "persistent_grant_id".to_owned(),
                    detail: format!("duplicate restored grant `{}`", record.id),
                });
            }
            let public = FileGrant {
                id: record.id.clone(),
                display_name: record.display_name,
                access: record.access,
                lifetime: GrantLifetime::Persistent,
                origin: GrantOrigin::Restored,
                entry_kind: record.entry_kind,
            };
            grants.insert(
                record.id,
                NativeGrant {
                    public,
                    root: native_root(record.root),
                },
            );
            Ok(grants)
        })
}

fn persistent_root(root: &GrantRoot) -> PersistentGrantRoot {
    match root {
        GrantRoot::Exact { path, parent } => PersistentGrantRoot::Exact {
            path: path.clone(),
            parent: parent.clone(),
        },
        GrantRoot::Directory(path) => PersistentGrantRoot::Directory(path.clone()),
    }
}

fn native_root(root: PersistentGrantRoot) -> GrantRoot {
    match root {
        PersistentGrantRoot::Exact { path, parent } => GrantRoot::Exact { path, parent },
        PersistentGrantRoot::Directory(path) => GrantRoot::Directory(path),
    }
}

fn next_native_grant_id(
    existing: &BTreeMap<FileGrantId, NativeGrant>,
) -> Result<FileGrantId, DesktopError> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|error| DesktopError::BackendUnavailable {
            backend: "system_random".to_owned(),
            detail: error.to_string(),
        })?;
        let mut token = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut token, "{byte:02x}").expect("writing to a String cannot fail");
        }
        let id = FileGrantId::new(format!("native-grant-{token}"))
            .expect("generated grant identifier is valid");
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

const fn grant_permission(origin: GrantOrigin) -> PermissionKind {
    match origin {
        GrantOrigin::KnownDirectory(_) => PermissionKind::KnownDirectoryAccess,
        GrantOrigin::UserSelection | GrantOrigin::Restored => PermissionKind::UserFileSelection,
    }
}

fn check_access(public: &FileGrant, intent: ResolveIntent) -> Result<(), DesktopError> {
    let permitted = match intent {
        ResolveIntent::Read | ResolveIntent::Metadata | ResolveIntent::ListDirectory => {
            public.access.permits_read()
        }
        ResolveIntent::Write => public.access.permits_write(),
    };
    if permitted {
        return Ok(());
    }
    Err(DesktopError::PermissionDenied {
        permission: grant_permission(public.origin),
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
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryPersistentGrantStore {
        records: Mutex<BTreeMap<FileGrantId, PersistentGrantRecord>>,
    }

    impl PersistentGrantStore for MemoryPersistentGrantStore {
        fn load(&self) -> Result<Vec<PersistentGrantRecord>, DesktopError> {
            Ok(self
                .records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values()
                .cloned()
                .collect())
        }

        fn persist(&self, record: PersistentGrantRecord) -> Result<(), DesktopError> {
            self.records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(record.id.clone(), record);
            Ok(())
        }

        fn revoke(&self, id: &FileGrantId) -> Result<(), DesktopError> {
            self.records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(id)
                .map(|_| ())
                .ok_or_else(|| DesktopError::StaleHandle {
                    handle: id.to_string(),
                })
        }
    }

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
        let provider = Arc::new(MemoryPersistentGrantStore::default());
        let dir = temp_test_dir("restore");
        let file = dir.join("save.txt");
        std::fs::write(&file, "kept").expect("test file written");

        let grant = GrantStore::new(PlatformKind::Windows, Some(provider.clone()))
            .expect("store builds")
            .insert_path(
                &file,
                GrantAccess::Read,
                GrantLifetime::Persistent,
                GrantOrigin::UserSelection,
            )
            .expect("persistent grant is stored");

        let restored = GrantStore::new(PlatformKind::Windows, Some(provider.clone()))
            .expect("persistent grants restore");
        let resolved = restored
            .resolve(&GrantPath::root(grant.id.clone()), ResolveIntent::Read)
            .expect("restored grant resolves");
        assert_eq!(resolved.path, std::fs::canonicalize(&file).unwrap());

        restored
            .revoke(&grant.id)
            .expect("persistent revoke succeeds");
        let loaded = provider.load().expect("provider still works");
        assert!(loaded.is_empty());
    }

    #[test]
    fn duplicate_restored_grant_ids_are_rejected() {
        let id = FileGrantId::try_new("persistent-duplicate").expect("valid id");
        let record = PersistentGrantRecord {
            id,
            display_name: "docs".to_owned(),
            access: GrantAccess::Read,
            entry_kind: FileEntryKind::Directory,
            root: PersistentGrantRoot::Directory(PathBuf::from(".")),
        };
        let Err(error) = restored_grants(vec![record.clone(), record]) else {
            panic!("duplicate persistent ids are invalid");
        };
        assert!(matches!(error, DesktopError::InvalidArgument { .. }));
    }
}
