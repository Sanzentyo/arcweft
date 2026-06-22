use crate::persistent_grants::{CaptureTarget, RestoredGrantRoot};
use arcweft_desktop_contract::{
    DesktopError, FileEntryKind, FileGrant, FileGrantId, GrantAccess, GrantLifetime, GrantOrigin,
    PermissionKind,
};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PersistentGrantRecord {
    pub(crate) id: FileGrantId,
    display_name: String,
    access: GrantAccess,
    issued_origin: GrantOrigin,
    entry_kind: FileEntryKind,
    pub(crate) root: PersistentGrantRoot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PersistentGrantRoot {
    Exact { path: PathBuf, parent: PathBuf },
    Directory(PathBuf),
}

impl PersistentGrantRecord {
    pub(crate) fn issue(public: &FileGrant, target: &CaptureTarget) -> Result<Self, DesktopError> {
        let record = Self {
            id: public.id.clone(),
            display_name: public.display_name.clone(),
            access: public.access,
            issued_origin: public.origin,
            entry_kind: public.entry_kind,
            root: match target {
                CaptureTarget::Exact { path, parent } => PersistentGrantRoot::Exact {
                    path: path.clone(),
                    parent: parent.clone(),
                },
                CaptureTarget::Directory(path) => PersistentGrantRoot::Directory(path.clone()),
            },
        };
        record.validate()?;
        Ok(record)
    }

    pub(crate) fn issued_permission(&self) -> Result<PermissionKind, DesktopError> {
        self.issued_origin
            .issued_permission()
            .filter(|_| self.issued_origin.is_valid_issuance_for(self.entry_kind))
            .ok_or_else(|| DesktopError::BackendUnavailable {
                backend: "persistent_file_grants".to_owned(),
                detail: "persistent_grant_record_corrupt".to_owned(),
            })
    }

    pub(crate) fn public_restored(&self) -> FileGrant {
        FileGrant {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            access: self.access,
            lifetime: GrantLifetime::Persistent,
            origin: GrantOrigin::Restored,
            entry_kind: self.entry_kind,
        }
    }

    fn validate(&self) -> Result<(), DesktopError> {
        if self.id.generated_lifetime() != Some(GrantLifetime::Persistent)
            || self.display_name.is_empty()
            || !self.issued_origin.is_valid_issuance_for(self.entry_kind)
            || self.issued_origin.issued_permission().is_none()
        {
            return Err(DesktopError::BackendUnavailable {
                backend: "persistent_file_grants".to_owned(),
                detail: "persistent_grant_record_corrupt".to_owned(),
            });
        }
        Ok(())
    }
}

impl PersistentGrantRoot {
    pub(crate) fn into_restored_root(self) -> RestoredGrantRoot {
        match self {
            Self::Exact { path, parent } => RestoredGrantRoot::Exact { path, parent },
            Self::Directory(path) => RestoredGrantRoot::Directory(path),
        }
    }
}
