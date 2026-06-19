use arcweft_desktop_contract::{
    DesktopError, DesktopFeature, FileEntryKind, FileGrant, FileGrantId, GrantAccess,
    GrantLifetime, GrantOrigin, GrantPath, PermissionKind, PlatformKind,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
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

/// Process-local path authority store.
///
/// The runtime sees only an unguessable `FileGrantId`. Existing targets and
/// their parent directories are canonicalized immediately before I/O to reject
/// symlink escapes. Destructive recursive operations are omitted from this
/// reference backend to keep the residual TOCTOU surface small.
#[derive(Default)]
pub(crate) struct GrantStore {
    state: Mutex<GrantState>,
}

impl GrantStore {
    pub fn insert_path(
        &self,
        platform: PlatformKind,
        path: impl AsRef<Path>,
        access: GrantAccess,
        lifetime: GrantLifetime,
        origin: GrantOrigin,
    ) -> Result<FileGrant, DesktopError> {
        if lifetime == GrantLifetime::Persistent {
            return Err(DesktopError::Unsupported {
                feature: DesktopFeature::PersistentFileGrant,
                platform,
                detail: "persistent native grants require a host-provided sealed-token store"
                    .to_owned(),
            });
        }

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
        self.state()
            .grants
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| DesktopError::StaleHandle {
                handle: id.to_string(),
            })
    }

    fn state(&self) -> MutexGuard<'_, GrantState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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

    #[test]
    fn native_grant_ids_are_random_capability_tokens() {
        let existing = BTreeMap::new();
        let first = next_native_grant_id(&existing).expect("system randomness is available");
        let second = next_native_grant_id(&existing).expect("system randomness is available");
        assert_ne!(first, second);
        assert!(first.as_str().starts_with("native-grant-"));
        assert_eq!(first.as_str().len(), "native-grant-".len() + 32);
    }
}
