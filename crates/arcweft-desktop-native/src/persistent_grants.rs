//! Host-only persistent file-grant coordination for the native desktop backend.
//!
//! Public construction is intentionally narrow. Stored records, platform
//! authority, and restore leases remain private to this crate.

#[cfg(test)]
mod record;

use arcweft_desktop_contract::{
    DesktopError, DesktopFeature, FileEntryKind, FileGrant, FileGrantId, GrantLifetime,
    PermissionKind, PlatformKind,
};
#[cfg(test)]
use record::PersistentGrantRecord;
#[cfg(test)]
use std::collections::BTreeMap;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentGrantConfig {
    platform: PlatformKind,
    namespace: String,
    storage_root: PathBuf,
}

impl PersistentGrantConfig {
    pub fn try_new(
        platform: PlatformKind,
        namespace: impl Into<String>,
        storage_root: impl Into<PathBuf>,
    ) -> Result<Self, DesktopError> {
        let namespace = namespace.into();
        validate_namespace(&namespace)?;
        Ok(Self {
            platform,
            namespace,
            storage_root: storage_root.into(),
        })
    }

    pub const fn platform(&self) -> PlatformKind {
        self.platform
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn storage_root(&self) -> &std::path::Path {
        &self.storage_root
    }
}

pub struct PersistentGrantServices {
    platform: PlatformKind,
    #[cfg(test)]
    repository: Arc<MemoryPersistentGrantRepository>,
}

impl PersistentGrantServices {
    pub fn open(config: PersistentGrantConfig) -> Result<Self, DesktopError> {
        let PersistentGrantConfig {
            platform,
            namespace,
            storage_root,
        } = config;
        drop((namespace, storage_root));
        Err(DesktopError::Unsupported {
            feature: DesktopFeature::PersistentFileGrant,
            platform,
            detail: "no live platform persistent grant authority has been validated for this host"
                .to_owned(),
        })
    }

    pub const fn platform(&self) -> PlatformKind {
        self.platform
    }

    pub(crate) fn persist(
        &self,
        public: &FileGrant,
        target: &CaptureTarget,
    ) -> Result<Option<PersistentGrantLease>, DesktopError> {
        validate_persistent_issue(public, target)?;
        #[cfg(test)]
        {
            self.repository
                .persist(PersistentGrantRecord::issue(public, target)?);
            Ok(Some(PersistentGrantLease::Memory))
        }
        #[cfg(not(test))]
        {
            let _ = (public, target);
            Err(unavailable(self.platform))
        }
    }

    pub(crate) fn restore(
        &self,
        id: &FileGrantId,
    ) -> Result<RestoredPersistentGrant, DesktopError> {
        if id.generated_lifetime() != Some(GrantLifetime::Persistent) {
            return Err(stale(id));
        }
        #[cfg(test)]
        {
            let record = self.repository.load(id)?;
            Ok(RestoredPersistentGrant {
                public: record.public_restored(),
                issued_permission: record.issued_permission()?,
                root: record.root.into_restored_root(),
                lease: Some(PersistentGrantLease::Memory),
            })
        }
        #[cfg(not(test))]
        {
            Err(unavailable(self.platform))
        }
    }

    pub(crate) fn revoke(&self, id: &FileGrantId) -> Result<(), DesktopError> {
        if id.generated_lifetime() != Some(GrantLifetime::Persistent) {
            return Err(stale(id));
        }
        #[cfg(test)]
        {
            self.repository.revoke(id)
        }
        #[cfg(not(test))]
        {
            Err(unavailable(self.platform))
        }
    }

    #[cfg(test)]
    pub(crate) fn memory_for_tests(platform: PlatformKind) -> Self {
        Self {
            platform,
            repository: Arc::new(MemoryPersistentGrantRepository {
                records: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn clone_for_tests(&self) -> Self {
        Self {
            platform: self.platform,
            repository: Arc::clone(&self.repository),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CaptureTarget {
    Exact { path: PathBuf, parent: PathBuf },
    Directory(PathBuf),
}

impl CaptureTarget {
    pub(crate) const fn expected_entry_kind(&self) -> FileEntryKind {
        match self {
            Self::Exact { .. } => FileEntryKind::File,
            Self::Directory(_) => FileEntryKind::Directory,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RestoredGrantRoot {
    Exact { path: PathBuf, parent: PathBuf },
    Directory(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PersistentGrantLease {
    #[cfg(test)]
    Memory,
}

pub(crate) struct RestoredPersistentGrant {
    pub(crate) public: FileGrant,
    pub(crate) issued_permission: PermissionKind,
    pub(crate) root: RestoredGrantRoot,
    pub(crate) lease: Option<PersistentGrantLease>,
}

fn validate_namespace(namespace: &str) -> Result<(), DesktopError> {
    if namespace.is_empty()
        || namespace.len() > 512
        || !namespace.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-' | b'@')
        })
    {
        return Err(DesktopError::InvalidArgument {
            field: "namespace".to_owned(),
            detail: "persistent grant namespace is empty, too long, or contains unsupported bytes"
                .to_owned(),
        });
    }
    Ok(())
}

fn validate_persistent_issue(
    public: &FileGrant,
    target: &CaptureTarget,
) -> Result<(), DesktopError> {
    if public.lifetime != GrantLifetime::Persistent
        || public.id.generated_lifetime() != Some(GrantLifetime::Persistent)
        || !public.origin.is_valid_issuance_for(public.entry_kind)
        || target.expected_entry_kind().is_directory() != public.entry_kind.is_directory()
    {
        return Err(DesktopError::InvalidArgument {
            field: "persistent_grant".to_owned(),
            detail: "persistent grant metadata does not match the inspected target".to_owned(),
        });
    }
    Ok(())
}

fn stale(id: &FileGrantId) -> DesktopError {
    DesktopError::StaleHandle {
        handle: id.to_string(),
    }
}

#[cfg(not(test))]
fn unavailable(platform: PlatformKind) -> DesktopError {
    let _ = RestoredGrantRoot::Directory(PathBuf::new());
    let _ = RestoredGrantRoot::Exact {
        path: PathBuf::new(),
        parent: PathBuf::new(),
    };
    DesktopError::Unsupported {
        feature: DesktopFeature::PersistentFileGrant,
        platform,
        detail: "no live platform persistent grant authority has been validated for this host"
            .to_owned(),
    }
}

#[cfg(test)]
struct MemoryPersistentGrantRepository {
    records: Mutex<BTreeMap<FileGrantId, PersistentGrantRecord>>,
}

#[cfg(test)]
impl MemoryPersistentGrantRepository {
    fn persist(&self, record: PersistentGrantRecord) {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(record.id.clone(), record);
    }

    fn load(&self, id: &FileGrantId) -> Result<PersistentGrantRecord, DesktopError> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .cloned()
            .ok_or_else(|| stale(id))
    }

    fn revoke(&self, id: &FileGrantId) -> Result<(), DesktopError> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| stale(id))
    }
}
