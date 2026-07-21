//! Binary project resources and the canonical accepted-topology identity.
//!
//! This module is Sans I/O. Loaders supply exact bytes and typed logical
//! identities; this owner canonicalizes them without retaining host paths or
//! acquisition origin.

use std::{collections::BTreeSet, sync::Arc};

use arcweft_character::{id::CharacterId, manifest::CharacterAssetPath};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_manifest_model::{
    ContentRootRef, ContentUnitId, ExternalModuleImportId, NormalizedProjectPath, PackageId,
    PackageVersion, ProfileId,
};
use arcweft_resource_model::registry::ResourceTypeRegistryDigest;
use thiserror::Error;

use crate::fingerprint::BuildDigest;

const TOPOLOGY_TRANSCRIPT_HEADER: &[u8] = b"arcweft.project-topology.v1\0";
const TOPOLOGY_TRANSCRIPT_VERSION: u32 = 1;

/// Immutable binary resource admitted into one project topology candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBinaryResource {
    bytes: Arc<[u8]>,
    digest: BuildDigest,
}

/// Nominal identity of one complete accepted project topology.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectTopologyRevision(BuildDigest);

/// Exact semantic kind of one present topology resource.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectTopologyResourceKind {
    ProjectManifest,
    ArcweftModule {
        module: CanonicalModulePath,
    },
    ExternalModuleMetadata {
        import: ExternalModuleImportId,
    },
    CharacterManifest {
        character: CharacterId,
    },
    CharacterLayer {
        character: CharacterId,
        asset: CharacterAssetPath,
    },
}

/// One present resource included in the canonical topology transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyResourceRecord {
    owner_package_id: PackageId,
    owner_package_version: PackageVersion,
    kind: ProjectTopologyResourceKind,
    logical_path: NormalizedProjectPath,
    resource: ProjectBinaryResource,
}

/// One accepted typed product represented by its existing semantic digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectTopologySemanticRecord {
    ResourceTypeRegistry(ResourceTypeRegistryDigest),
}

/// Explicit absence of one optional file-backed Character root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyAbsenceRecord {
    content_unit: ContentUnitId,
    root_ordinal: u32,
    authored_root: ContentRootRef,
    character: CharacterId,
    expected_package_root: NormalizedProjectPath,
    expected_manifest_path: NormalizedProjectPath,
}

/// Failure to construct the bounded canonical topology transcript.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectTopologyRevisionError {
    #[error("topology {field} count does not fit the canonical u32 encoding")]
    CountOverflow { field: &'static str },
    #[error("topology {field} does not fit the canonical u32 string encoding")]
    StringLengthOverflow { field: &'static str },
    #[error("topology resource byte length does not fit the canonical u64 encoding")]
    ResourceLengthOverflow,
    #[error("duplicate present topology resource `{logical_path}`")]
    DuplicatePresent { logical_path: String },
    #[error("duplicate topology semantic record `{semantic_key}`")]
    DuplicateSemantic { semantic_key: &'static str },
    #[error("duplicate topology absence record `{content_unit}` root {root_ordinal}")]
    DuplicateAbsence {
        content_unit: ContentUnitId,
        root_ordinal: u32,
    },
}

impl ProjectBinaryResource {
    /// Retains exact bytes and computes their existing project build digest.
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        let bytes = bytes.into();
        let digest = BuildDigest::of(&bytes);
        Self { bytes, digest }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Shares the exact immutable byte allocation with another typed owner.
    pub fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }

    pub const fn digest(&self) -> BuildDigest {
        self.digest
    }
}

impl ProjectTopologyResourceKind {
    const fn tag(&self) -> u8 {
        match self {
            Self::ProjectManifest => 0x01,
            Self::ArcweftModule { .. } => 0x02,
            Self::ExternalModuleMetadata { .. } => 0x03,
            Self::CharacterManifest { .. } => 0x04,
            Self::CharacterLayer { .. } => 0x05,
        }
    }

    fn semantic_key(&self) -> String {
        match self {
            Self::ProjectManifest => "manifest".to_owned(),
            Self::ArcweftModule { module } => module.to_string(),
            Self::ExternalModuleMetadata { import } => import.as_str().to_owned(),
            Self::CharacterManifest { character } => character.as_str().to_owned(),
            Self::CharacterLayer { character, asset } => {
                let mut key =
                    String::with_capacity(character.as_str().len() + asset.as_str().len() + 1);
                key.push_str(character.as_str());
                key.push('\0');
                key.push_str(asset.as_str());
                key
            }
        }
    }
}

impl ProjectTopologyResourceRecord {
    pub fn new(
        owner_package_id: PackageId,
        owner_package_version: PackageVersion,
        kind: ProjectTopologyResourceKind,
        logical_path: NormalizedProjectPath,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Self {
        Self {
            owner_package_id,
            owner_package_version,
            kind,
            logical_path,
            resource: ProjectBinaryResource::new(bytes),
        }
    }

    pub const fn owner_package_id(&self) -> &PackageId {
        &self.owner_package_id
    }

    pub const fn owner_package_version(&self) -> &PackageVersion {
        &self.owner_package_version
    }

    pub const fn kind(&self) -> &ProjectTopologyResourceKind {
        &self.kind
    }

    pub const fn logical_path(&self) -> &NormalizedProjectPath {
        &self.logical_path
    }

    pub const fn resource(&self) -> &ProjectBinaryResource {
        &self.resource
    }

    fn canonical_key(&self) -> (String, String, u8, String, String) {
        (
            self.owner_package_id.as_str().to_owned(),
            self.owner_package_version.to_string(),
            self.kind.tag(),
            self.kind.semantic_key(),
            self.logical_path.as_str().to_owned(),
        )
    }
}

impl ProjectTopologySemanticRecord {
    const fn tag(self) -> u8 {
        match self {
            Self::ResourceTypeRegistry(_) => 0x20,
        }
    }

    const fn semantic_key(self) -> &'static str {
        match self {
            Self::ResourceTypeRegistry(_) => "resource-type-registry",
        }
    }

    fn digest_bytes(self) -> [u8; 32] {
        match self {
            Self::ResourceTypeRegistry(digest) => *digest.semantic_digest().as_bytes(),
        }
    }
}

impl ProjectTopologyAbsenceRecord {
    pub fn new(
        content_unit: ContentUnitId,
        root_ordinal: u32,
        authored_root: ContentRootRef,
        character: CharacterId,
        expected_package_root: NormalizedProjectPath,
        expected_manifest_path: NormalizedProjectPath,
    ) -> Self {
        Self {
            content_unit,
            root_ordinal,
            authored_root,
            character,
            expected_package_root,
            expected_manifest_path,
        }
    }

    pub const fn content_unit(&self) -> &ContentUnitId {
        &self.content_unit
    }

    pub const fn root_ordinal(&self) -> u32 {
        self.root_ordinal
    }

    pub const fn authored_root(&self) -> &ContentRootRef {
        &self.authored_root
    }

    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    pub const fn expected_package_root(&self) -> &NormalizedProjectPath {
        &self.expected_package_root
    }

    pub const fn expected_manifest_path(&self) -> &NormalizedProjectPath {
        &self.expected_manifest_path
    }

    fn canonical_key(&self) -> (String, u32, String) {
        (
            self.content_unit.as_str().to_owned(),
            self.root_ordinal,
            self.authored_root.0.as_str().to_owned(),
        )
    }
}

impl ProjectTopologyRevision {
    /// Computes the sole accepted topology identity from canonical typed records.
    pub fn try_for_inventory(
        package: (&PackageId, &PackageVersion),
        profile: &ProfileId,
        records: impl IntoIterator<Item = ProjectTopologyResourceRecord>,
        semantic_records: impl IntoIterator<Item = ProjectTopologySemanticRecord>,
        absences: impl IntoIterator<Item = ProjectTopologyAbsenceRecord>,
    ) -> Result<Self, ProjectTopologyRevisionError> {
        let mut records = records.into_iter().collect::<Vec<_>>();
        records.sort_by_key(ProjectTopologyResourceRecord::canonical_key);
        reject_duplicate_present(&records)?;

        let mut semantic_records = semantic_records.into_iter().collect::<Vec<_>>();
        semantic_records.sort_by_key(|record| (record.tag(), record.semantic_key()));
        reject_duplicate_semantic(&semantic_records)?;

        let mut absences = absences.into_iter().collect::<Vec<_>>();
        absences.sort_by_key(ProjectTopologyAbsenceRecord::canonical_key);
        reject_duplicate_absence(&absences)?;

        let mut transcript = Vec::new();
        transcript.extend_from_slice(TOPOLOGY_TRANSCRIPT_HEADER);
        transcript.extend_from_slice(&TOPOLOGY_TRANSCRIPT_VERSION.to_le_bytes());
        put_string(&mut transcript, "package ID", package.0.as_str())?;
        put_string(&mut transcript, "package version", &package.1.to_string())?;
        put_string(&mut transcript, "profile ID", profile.as_str())?;
        put_count(&mut transcript, "present record", records.len())?;
        for record in &records {
            transcript.push(record.kind.tag());
            put_string(
                &mut transcript,
                "resource owner package ID",
                record.owner_package_id.as_str(),
            )?;
            put_string(
                &mut transcript,
                "resource owner package version",
                &record.owner_package_version.to_string(),
            )?;
            put_string(
                &mut transcript,
                "resource semantic key",
                &record.kind.semantic_key(),
            )?;
            put_string(
                &mut transcript,
                "resource logical path",
                record.logical_path.as_str(),
            )?;
            let byte_length = u64::try_from(record.resource.bytes.len())
                .map_err(|_| ProjectTopologyRevisionError::ResourceLengthOverflow)?;
            transcript.extend_from_slice(&byte_length.to_le_bytes());
            transcript.extend_from_slice(&record.resource.digest.as_bytes());
        }
        put_count(&mut transcript, "semantic record", semantic_records.len())?;
        for record in semantic_records {
            transcript.push(record.tag());
            put_string(
                &mut transcript,
                "semantic record key",
                record.semantic_key(),
            )?;
            transcript.extend_from_slice(&record.digest_bytes());
        }
        put_count(&mut transcript, "absence record", absences.len())?;
        for absence in absences {
            transcript.push(0x80);
            put_string(
                &mut transcript,
                "content unit ID",
                absence.content_unit.as_str(),
            )?;
            transcript.extend_from_slice(&absence.root_ordinal.to_le_bytes());
            put_string(
                &mut transcript,
                "authored content root",
                absence.authored_root.0.as_str(),
            )?;
            put_string(&mut transcript, "character ID", absence.character.as_str())?;
            put_string(
                &mut transcript,
                "expected package root",
                absence.expected_package_root.as_str(),
            )?;
            put_string(
                &mut transcript,
                "expected manifest path",
                absence.expected_manifest_path.as_str(),
            )?;
        }
        Ok(Self(BuildDigest::of(&transcript)))
    }

    pub const fn digest(self) -> BuildDigest {
        self.0
    }
}

fn reject_duplicate_present(
    records: &[ProjectTopologyResourceRecord],
) -> Result<(), ProjectTopologyRevisionError> {
    let mut keys = BTreeSet::new();
    for record in records {
        if !keys.insert(record.canonical_key()) {
            return Err(ProjectTopologyRevisionError::DuplicatePresent {
                logical_path: record.logical_path.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn reject_duplicate_semantic(
    records: &[ProjectTopologySemanticRecord],
) -> Result<(), ProjectTopologyRevisionError> {
    let mut keys = BTreeSet::new();
    for record in records {
        let key = (record.tag(), record.semantic_key());
        if !keys.insert(key) {
            return Err(ProjectTopologyRevisionError::DuplicateSemantic {
                semantic_key: record.semantic_key(),
            });
        }
    }
    Ok(())
}

fn reject_duplicate_absence(
    records: &[ProjectTopologyAbsenceRecord],
) -> Result<(), ProjectTopologyRevisionError> {
    let mut keys = BTreeSet::new();
    for record in records {
        if !keys.insert(record.canonical_key()) {
            return Err(ProjectTopologyRevisionError::DuplicateAbsence {
                content_unit: record.content_unit.clone(),
                root_ordinal: record.root_ordinal,
            });
        }
    }
    Ok(())
}

fn put_count(
    transcript: &mut Vec<u8>,
    field: &'static str,
    count: usize,
) -> Result<(), ProjectTopologyRevisionError> {
    let count =
        u32::try_from(count).map_err(|_| ProjectTopologyRevisionError::CountOverflow { field })?;
    transcript.extend_from_slice(&count.to_le_bytes());
    Ok(())
}

fn put_string(
    transcript: &mut Vec<u8>,
    field: &'static str,
    value: &str,
) -> Result<(), ProjectTopologyRevisionError> {
    let length = u32::try_from(value.len())
        .map_err(|_| ProjectTopologyRevisionError::StringLengthOverflow { field })?;
    transcript.extend_from_slice(&length.to_le_bytes());
    transcript.extend_from_slice(value.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_syntax::ast::module_path::ModuleSegment;
    use arcweft_resource_model::registry::ResourceTypeRegistry;

    fn package() -> (PackageId, PackageVersion, ProfileId) {
        (
            PackageId::new("org.arcweft.test").unwrap(),
            PackageVersion::new("1.2.3").unwrap(),
            ProfileId::new("release").unwrap(),
        )
    }

    fn records() -> Vec<ProjectTopologyResourceRecord> {
        let (package, version, _) = package();
        vec![
            ProjectTopologyResourceRecord::new(
                package.clone(),
                version.clone(),
                ProjectTopologyResourceKind::ArcweftModule {
                    module: CanonicalModulePath::from_segments([
                        ModuleSegment::new("game").unwrap()
                    ]),
                },
                NormalizedProjectPath::new("src/game.arcw").unwrap(),
                Arc::<[u8]>::from(&b"fn game() {}"[..]),
            ),
            ProjectTopologyResourceRecord::new(
                package,
                version,
                ProjectTopologyResourceKind::ProjectManifest,
                NormalizedProjectPath::new("arcw.toml").unwrap(),
                Arc::<[u8]>::from(&b"schema = 1"[..]),
            ),
        ]
    }

    #[test]
    fn binary_resource_retains_exact_bytes_and_digest() {
        let resource = ProjectBinaryResource::new(Arc::<[u8]>::from(&b"binary\0bytes"[..]));
        assert_eq!(resource.bytes(), b"binary\0bytes");
        assert_eq!(resource.digest(), BuildDigest::of(b"binary\0bytes"));
    }

    #[test]
    fn topology_revision_is_insertion_order_independent() {
        let (package, version, profile) = package();
        let mut reversed = records();
        reversed.reverse();
        let first = ProjectTopologyRevision::try_for_inventory(
            (&package, &version),
            &profile,
            records(),
            [],
            [],
        )
        .unwrap();
        let second = ProjectTopologyRevision::try_for_inventory(
            (&package, &version),
            &profile,
            reversed,
            [],
            [],
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.digest().to_hex(),
            "1f9a6a22ad356cfe85c4455de874877b3ab7cce3700b079a458e64a64bd7bc4e"
        );
    }

    #[test]
    fn topology_revision_rejects_duplicate_canonical_keys() {
        let (package, version, profile) = package();
        let record = records().pop().unwrap();
        let error = ProjectTopologyRevision::try_for_inventory(
            (&package, &version),
            &profile,
            [record.clone(), record],
            [],
            [],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProjectTopologyRevisionError::DuplicatePresent { .. }
        ));
    }

    #[test]
    fn topology_revision_changes_for_binary_semantic_and_absence_inputs() {
        let (package, version, profile) = package();
        let base = ProjectTopologyRevision::try_for_inventory(
            (&package, &version),
            &profile,
            records(),
            [],
            [],
        )
        .unwrap();
        let with_semantic = ProjectTopologyRevision::try_for_inventory(
            (&package, &version),
            &profile,
            records(),
            [ProjectTopologySemanticRecord::ResourceTypeRegistry(
                ResourceTypeRegistry::empty().digest(),
            )],
            [],
        )
        .unwrap();
        assert_ne!(base, with_semantic);

        let absence = ProjectTopologyAbsenceRecord::new(
            ContentUnitId::new("optional-cast").unwrap(),
            0,
            ContentRootRef(arcweft_manifest_model::EntityIdRef::new("@character.alice").unwrap()),
            CharacterId::try_new("character.alice").unwrap(),
            NormalizedProjectPath::new("assets/alice.awchar").unwrap(),
            NormalizedProjectPath::new("assets/alice.awchar/character.awchar.json").unwrap(),
        );
        let with_absence = ProjectTopologyRevision::try_for_inventory(
            (&package, &version),
            &profile,
            records(),
            [],
            [absence],
        )
        .unwrap();
        assert_ne!(base, with_absence);
    }
}
