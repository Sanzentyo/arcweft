use crate::NativeDesktopOptions;
use crate::grant_store::{GrantStore, ResolveIntent};
use arcweft_desktop_contract::{
    DesktopError, DirectoryEntry, FileDialogMode, FileDialogRequest, FileEntryKind, FileMetadata,
    GrantOrigin, GrantPath, KnownDirectory, PlatformKind, PortableRelativePath, UserFileRequest,
    UserFileResponse,
};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

pub(crate) fn execute_user_file(
    platform: PlatformKind,
    options: &NativeDesktopOptions,
    grants: &GrantStore,
    request: UserFileRequest,
) -> Result<UserFileResponse, DesktopError> {
    match request {
        UserFileRequest::ShowDialog(request) => show_dialog(platform, grants, &request),
        UserFileRequest::GrantKnownDirectory {
            directory,
            access,
            lifetime,
        } => {
            if !options.allowed_known_directories.contains(&directory) {
                return Err(DesktopError::PermissionDenied {
                    permission: arcweft_desktop_contract::PermissionKind::KnownDirectoryAccess,
                    detail: format!("{directory:?} is not present in the host allowlist"),
                });
            }
            let path = known_directory_path(platform, directory)?;
            grants
                .insert_path(
                    platform,
                    path,
                    access,
                    lifetime,
                    GrantOrigin::KnownDirectory(directory),
                )
                .map(|grant| UserFileResponse::Grants(vec![grant]))
        }
        UserFileRequest::ReadText { path } => {
            let resolved = grants.resolve(&path, ResolveIntent::Read)?;
            ensure_file(&resolved.path)?;
            std::fs::read_to_string(&resolved.path)
                .map(UserFileResponse::Text)
                .map_err(|error| DesktopError::sanitized_io("read_text", &error))
        }
        UserFileRequest::ReadBytes { path } => {
            let resolved = grants.resolve(&path, ResolveIntent::Read)?;
            ensure_file(&resolved.path)?;
            std::fs::read(&resolved.path)
                .map(UserFileResponse::Bytes)
                .map_err(|error| DesktopError::sanitized_io("read_bytes", &error))
        }
        UserFileRequest::WriteText { path, text } => {
            let resolved = grants.resolve(&path, ResolveIntent::Write)?;
            ensure_not_directory(&resolved.path)?;
            std::fs::write(&resolved.path, text)
                .map(|()| UserFileResponse::Applied)
                .map_err(|error| DesktopError::sanitized_io("write_text", &error))
        }
        UserFileRequest::WriteBytes { path, bytes } => {
            let resolved = grants.resolve(&path, ResolveIntent::Write)?;
            ensure_not_directory(&resolved.path)?;
            std::fs::write(&resolved.path, bytes)
                .map(|()| UserFileResponse::Applied)
                .map_err(|error| DesktopError::sanitized_io("write_bytes", &error))
        }
        UserFileRequest::Metadata { path } => {
            let resolved = grants.resolve(&path, ResolveIntent::Metadata)?;
            file_metadata(&resolved.path).map(UserFileResponse::Metadata)
        }
        UserFileRequest::ListDirectory { path } => {
            let resolved = grants.resolve(&path, ResolveIntent::ListDirectory)?;
            list_directory(&path, &resolved.path).map(UserFileResponse::DirectoryEntries)
        }
        UserFileRequest::Revoke { grant } => {
            grants.revoke(&grant).map(|()| UserFileResponse::Applied)
        }
    }
}

#[cfg(feature = "file-dialog")]
fn show_dialog(
    platform: PlatformKind,
    grants: &GrantStore,
    request: &FileDialogRequest,
) -> Result<UserFileResponse, DesktopError> {
    if request.lifetime == arcweft_desktop_contract::GrantLifetime::Persistent {
        return Err(DesktopError::Unsupported {
            feature: arcweft_desktop_contract::DesktopFeature::PersistentFileGrant,
            platform,
            detail: "persistent native grants require a host-provided sealed-token store"
                .to_owned(),
        });
    }
    validate_dialog_request(request)?;
    let mut dialog = rfd::FileDialog::new().set_title(request.title.clone());
    if let Some(suggested_name) = &request.suggested_name {
        dialog = dialog.set_file_name(suggested_name.clone());
    }
    for filter in &request.filters {
        dialog = dialog.add_filter(filter.name.clone(), &filter.extensions);
    }

    let paths = match request.mode {
        FileDialogMode::OpenFile => dialog.pick_file().map(|path| vec![path]),
        FileDialogMode::OpenFiles => dialog.pick_files(),
        FileDialogMode::SaveFile => dialog.save_file().map(|path| vec![path]),
        FileDialogMode::PickDirectory => dialog.pick_folder().map(|path| vec![path]),
    }
    .ok_or(DesktopError::UserCancelled)?;

    let mut inserted = Vec::with_capacity(paths.len());
    for path in paths {
        match grants.insert_path(
            platform,
            path,
            request.access,
            request.lifetime,
            GrantOrigin::UserSelection,
        ) {
            Ok(grant) => inserted.push(grant),
            Err(error) => {
                for grant in &inserted {
                    let _ = grants.revoke(&grant.id);
                }
                return Err(error);
            }
        }
    }
    Ok(UserFileResponse::Grants(inserted))
}

#[cfg(not(feature = "file-dialog"))]
fn show_dialog(
    platform: PlatformKind,
    _grants: &GrantStore,
    _request: &FileDialogRequest,
) -> Result<UserFileResponse, DesktopError> {
    Err(DesktopError::Unsupported {
        feature: arcweft_desktop_contract::DesktopFeature::UserFileDialog,
        platform,
        detail: "crate feature `file-dialog` is disabled".to_owned(),
    })
}

fn validate_dialog_request(request: &FileDialogRequest) -> Result<(), DesktopError> {
    match request.mode {
        FileDialogMode::OpenFile | FileDialogMode::OpenFiles if !request.access.permits_read() => {
            return Err(DesktopError::InvalidArgument {
                field: "access".to_owned(),
                detail: "open dialogs require read access".to_owned(),
            });
        }
        FileDialogMode::SaveFile if !request.access.permits_write() => {
            return Err(DesktopError::InvalidArgument {
                field: "access".to_owned(),
                detail: "save dialogs require write access".to_owned(),
            });
        }
        FileDialogMode::OpenFile
        | FileDialogMode::OpenFiles
        | FileDialogMode::SaveFile
        | FileDialogMode::PickDirectory => {}
    }
    if request.suggested_name.as_ref().is_some_and(|name| {
        name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0')
    }) {
        return Err(DesktopError::InvalidArgument {
            field: "suggested_name".to_owned(),
            detail: "suggested file name must be a single portable component".to_owned(),
        });
    }
    if request.filters.iter().any(|filter| {
        filter.name.is_empty()
            || filter.extensions.iter().any(|extension| {
                extension.is_empty()
                    || extension.starts_with('.')
                    || extension.contains('/')
                    || extension.contains('\\')
            })
    }) {
        return Err(DesktopError::InvalidArgument {
            field: "filters".to_owned(),
            detail:
                "filter names and extensions must be non-empty; extensions omit the leading dot"
                    .to_owned(),
        });
    }
    Ok(())
}

#[cfg(feature = "known-directories")]
fn known_directory_path(
    platform: PlatformKind,
    directory: KnownDirectory,
) -> Result<PathBuf, DesktopError> {
    use directories::{BaseDirs, UserDirs};

    let base = BaseDirs::new();
    let user = UserDirs::new();
    let path = match directory {
        KnownDirectory::Home => base.as_ref().map(directories::BaseDirs::home_dir),
        KnownDirectory::Config => base.as_ref().map(directories::BaseDirs::config_dir),
        KnownDirectory::Cache => base.as_ref().map(directories::BaseDirs::cache_dir),
        KnownDirectory::Data => base.as_ref().map(directories::BaseDirs::data_dir),
        KnownDirectory::Desktop => user.as_ref().and_then(|dirs| dirs.desktop_dir()),
        KnownDirectory::Documents => user.as_ref().and_then(|dirs| dirs.document_dir()),
        KnownDirectory::Downloads => user.as_ref().and_then(|dirs| dirs.download_dir()),
        KnownDirectory::Music => user.as_ref().and_then(|dirs| dirs.audio_dir()),
        KnownDirectory::Pictures => user.as_ref().and_then(|dirs| dirs.picture_dir()),
        KnownDirectory::Videos => user.as_ref().and_then(|dirs| dirs.video_dir()),
    };
    path.map(PathBuf::from)
        .ok_or_else(|| DesktopError::NotFound {
            resource: format!("known directory {directory:?} on {platform:?}"),
        })
}

#[cfg(not(feature = "known-directories"))]
fn known_directory_path(
    platform: PlatformKind,
    _directory: KnownDirectory,
) -> Result<PathBuf, DesktopError> {
    Err(DesktopError::Unsupported {
        feature: arcweft_desktop_contract::DesktopFeature::KnownDirectoryGrant,
        platform,
        detail: "crate feature `known-directories` is disabled".to_owned(),
    })
}

fn ensure_file(path: &std::path::Path) -> Result<(), DesktopError> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| DesktopError::sanitized_io("inspect_file", &error))?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(DesktopError::InvalidArgument {
            field: "path".to_owned(),
            detail: "operation requires a regular file".to_owned(),
        })
    }
}

fn ensure_not_directory(path: &std::path::Path) -> Result<(), DesktopError> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Err(DesktopError::InvalidArgument {
            field: "path".to_owned(),
            detail: "write target is a directory".to_owned(),
        }),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DesktopError::sanitized_io("inspect_write_target", &error)),
    }
}

fn file_metadata(path: &std::path::Path) -> Result<FileMetadata, DesktopError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| DesktopError::sanitized_io("metadata", &error))?;
    Ok(metadata_from_std(&metadata))
}

fn metadata_from_std(metadata: &std::fs::Metadata) -> FileMetadata {
    let entry_kind = if metadata.is_file() {
        FileEntryKind::File
    } else if metadata.is_dir() {
        FileEntryKind::Directory
    } else {
        FileEntryKind::Other
    };
    let modified_unix_millis = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_millis()).ok());
    FileMetadata {
        entry_kind,
        byte_len: metadata.is_file().then_some(metadata.len()),
        modified_unix_millis,
        readonly: Some(metadata.permissions().readonly()),
    }
}

fn list_directory(
    grant_path: &GrantPath,
    native_path: &std::path::Path,
) -> Result<Vec<DirectoryEntry>, DesktopError> {
    let metadata = std::fs::metadata(native_path)
        .map_err(|error| DesktopError::sanitized_io("inspect_directory", &error))?;
    if !metadata.is_dir() {
        return Err(DesktopError::InvalidArgument {
            field: "path".to_owned(),
            detail: "directory listing requires a directory".to_owned(),
        });
    }

    let mut entries = std::fs::read_dir(native_path)
        .map_err(|error| DesktopError::sanitized_io("list_directory", &error))?
        .map(|entry| {
            let entry = entry
                .map_err(|error| DesktopError::sanitized_io("read_directory_entry", &error))?;
            let display_name = entry
                .file_name()
                .into_string()
                .map_err(|_| DesktopError::Io {
                    operation: "list_directory".to_owned(),
                    detail: "non_utf8_file_name".to_owned(),
                })?;
            let relative_text = grant_path.relative.as_ref().map_or_else(
                || display_name.clone(),
                |base| format!("{base}/{display_name}"),
            );
            let relative =
                PortableRelativePath::new(relative_text).map_err(|_| DesktopError::Io {
                    operation: "list_directory".to_owned(),
                    detail: "non_portable_file_name".to_owned(),
                })?;
            let file_type = entry
                .file_type()
                .map_err(|error| DesktopError::sanitized_io("inspect_directory_entry", &error))?;
            let metadata = if file_type.is_symlink() {
                FileMetadata {
                    entry_kind: FileEntryKind::Other,
                    byte_len: None,
                    modified_unix_millis: None,
                    readonly: None,
                }
            } else {
                let metadata = entry.metadata().map_err(|error| {
                    DesktopError::sanitized_io("metadata_directory_entry", &error)
                })?;
                metadata_from_std(&metadata)
            };
            Ok(DirectoryEntry {
                relative,
                display_name,
                metadata,
            })
        })
        .collect::<Result<Vec<_>, DesktopError>>()?;
    entries.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(entries)
}
