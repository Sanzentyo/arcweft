//! Validated nominal identities carried by adapter manifests.

use std::collections::BTreeMap;
use std::fmt;

use arcweft_manifest_model::AdapterOpaqueTypeProducerId;
use arcweft_rust_abi::{ArcweftRustPackageId, ArcweftRustTypePath, ArcweftRustTypePathSegment};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{AdapterId, AdapterTypeKind};

const MAX_NOMINAL_PATH_SEGMENTS: usize = 256;
const MAX_NOMINAL_ARGUMENTS: usize = 256;
const MAX_NOMINAL_ARITY: u16 = 256;

/// Stable semantic owner assigned to adapter-native nominal declarations.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AdapterEnvironmentOwnerId(String);

/// One validated segment in an accepted adapter nominal path.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AdapterNominalPathSegment(String);

/// A validated non-empty accepted adapter nominal path.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AdapterNominalPath {
    segments: Box<[AdapterNominalPathSegment]>,
}

/// A validated prefix used to mount one Rust package into the accepted world.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AdapterNominalPathPrefix {
    segments: Box<[AdapterNominalPathSegment]>,
}

/// Exact semantic owner of an adapter type reference.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdapterNominalOwner {
    /// Nominal declared by the built-in standard environment.
    Standard,
    /// Nominal declared by one adapter environment.
    Environment { owner: AdapterEnvironmentOwnerId },
    /// Nominal declared by one Rust ABI package.
    RustPackage { package: ArcweftRustPackageId },
}

/// Exact owner, world path, and recursive arguments of one nominal reference.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AdapterNominalTypeRef {
    owner: AdapterNominalOwner,
    path: AdapterNominalPath,
    arguments: Box<[AdapterTypeKind]>,
}

/// Visibility contributed to the accepted nominal inventory.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterNominalVisibility {
    Public,
    Private,
}

/// One adapter-native nominal declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdapterNominalDeclaration {
    path: AdapterNominalPath,
    arity: u16,
    opaque_producer: AdapterOpaqueTypeProducerId,
    visibility: AdapterNominalVisibility,
    source_label: String,
}

/// Exact package-to-world-prefix mounts known by one adapter manifest.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterRustPackageMountTable {
    by_package: BTreeMap<ArcweftRustPackageId, AdapterNominalPathPrefix>,
}

/// Invalid adapter nominal path or prefix.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterNominalPathError {
    #[error("adapter nominal path must contain at least one segment")]
    Empty,
    #[error("adapter nominal path has {observed} segments, exceeding {maximum}")]
    SegmentLimit { observed: usize, maximum: usize },
    #[error("invalid adapter nominal path segment `{segment}`")]
    InvalidSegment { segment: String },
}

/// Invalid recursive adapter type model.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterTypeModelError {
    #[error(transparent)]
    Path(#[from] AdapterNominalPathError),
    #[error("adapter nominal type has {observed} arguments, exceeding {maximum}")]
    ArgumentLimit { observed: usize, maximum: usize },
    #[error("adapter type depth {observed} exceeds {maximum}")]
    DepthLimit { observed: usize, maximum: usize },
    #[error("adapter type node count {observed} exceeds {maximum}")]
    NodeLimit { observed: usize, maximum: usize },
    #[error("adapter nominal declaration arity {observed} exceeds {maximum}")]
    ArityLimit { observed: u16, maximum: u16 },
    #[error("adapter nominal declaration source label must not be empty")]
    EmptySourceLabel,
    #[error("adapter nominal declaration source label contains a control character at byte {byte}")]
    SourceLabelControl { byte: usize },
}

impl AdapterEnvironmentOwnerId {
    /// Derives the sole accepted owner for declarations in this adapter.
    pub fn for_adapter(adapter: &AdapterId) -> Self {
        Self(format!("adapter:{}", adapter.as_str()))
    }

    /// Canonical environment binding spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AdapterNominalPathSegment {
    /// Validates and constructs one nominal path segment.
    pub fn try_new(value: impl Into<String>) -> Result<Self, AdapterNominalPathError> {
        let value = value.into();
        ArcweftRustTypePathSegment::try_new(value.clone()).map_err(|_| {
            AdapterNominalPathError::InvalidSegment {
                segment: value.clone(),
            }
        })?;
        Ok(Self(value))
    }

    /// Exact stored segment spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AdapterNominalPath {
    /// Validates and constructs one non-empty nominal path.
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterNominalPathSegment>,
    ) -> Result<Self, AdapterNominalPathError> {
        let segments = segments.into_iter().collect::<Box<[_]>>();
        validate_segments(&segments, false)?;
        Ok(Self { segments })
    }

    /// Exact path segments in semantic order.
    pub fn segments(&self) -> &[AdapterNominalPathSegment] {
        &self.segments
    }
}

impl fmt::Display for AdapterNominalPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 {
                formatter.write_str("::")?;
            }
            formatter.write_str(segment.as_str())?;
        }
        Ok(())
    }
}

impl AdapterNominalPathPrefix {
    /// Validates and constructs one possibly empty package mount prefix.
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterNominalPathSegment>,
    ) -> Result<Self, AdapterNominalPathError> {
        let segments = segments.into_iter().collect::<Box<[_]>>();
        validate_segments(&segments, true)?;
        Ok(Self { segments })
    }

    /// Exact prefix segments in semantic order.
    pub fn segments(&self) -> &[AdapterNominalPathSegment] {
        &self.segments
    }

    /// Joins this world prefix to one validated package-local Rust path.
    pub fn join(
        &self,
        local: &ArcweftRustTypePath,
    ) -> Result<AdapterNominalPath, AdapterNominalPathError> {
        let observed = self
            .segments
            .len()
            .checked_add(local.segments().len())
            .ok_or(AdapterNominalPathError::SegmentLimit {
                observed: usize::MAX,
                maximum: MAX_NOMINAL_PATH_SEGMENTS,
            })?;
        if observed > MAX_NOMINAL_PATH_SEGMENTS {
            return Err(AdapterNominalPathError::SegmentLimit {
                observed,
                maximum: MAX_NOMINAL_PATH_SEGMENTS,
            });
        }
        let local = local
            .segments()
            .iter()
            .map(|segment| AdapterNominalPathSegment(segment.as_str().to_owned()));
        AdapterNominalPath::try_new(self.segments.iter().cloned().chain(local))
    }
}

impl AdapterNominalTypeRef {
    /// Validates and constructs one exact recursive nominal reference.
    pub fn try_new(
        owner: AdapterNominalOwner,
        path: AdapterNominalPath,
        arguments: impl IntoIterator<Item = AdapterTypeKind>,
    ) -> Result<Self, AdapterTypeModelError> {
        let arguments = arguments.into_iter().collect::<Box<[_]>>();
        if arguments.len() > MAX_NOMINAL_ARGUMENTS {
            return Err(AdapterTypeModelError::ArgumentLimit {
                observed: arguments.len(),
                maximum: MAX_NOMINAL_ARGUMENTS,
            });
        }
        Ok(Self {
            owner,
            path,
            arguments,
        })
    }

    /// Exact accepted owner.
    pub const fn owner(&self) -> &AdapterNominalOwner {
        &self.owner
    }

    /// Exact accepted world path.
    pub const fn path(&self) -> &AdapterNominalPath {
        &self.path
    }

    /// Recursive type arguments in declaration order.
    pub fn arguments(&self) -> &[AdapterTypeKind] {
        &self.arguments
    }
}

impl AdapterNominalDeclaration {
    /// Validates one adapter-native nominal declaration.
    pub fn try_new(
        path: AdapterNominalPath,
        arity: u16,
        opaque_producer: AdapterOpaqueTypeProducerId,
        visibility: AdapterNominalVisibility,
        source_label: impl Into<String>,
    ) -> Result<Self, AdapterTypeModelError> {
        if arity > MAX_NOMINAL_ARITY {
            return Err(AdapterTypeModelError::ArityLimit {
                observed: arity,
                maximum: MAX_NOMINAL_ARITY,
            });
        }
        let source_label = source_label.into();
        if source_label.is_empty() {
            return Err(AdapterTypeModelError::EmptySourceLabel);
        }
        if let Some((byte, _)) = source_label
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(AdapterTypeModelError::SourceLabelControl { byte });
        }
        Ok(Self {
            path,
            arity,
            opaque_producer,
            visibility,
            source_label,
        })
    }

    pub const fn path(&self) -> &AdapterNominalPath {
        &self.path
    }

    pub const fn arity(&self) -> u16 {
        self.arity
    }

    pub const fn opaque_producer(&self) -> &AdapterOpaqueTypeProducerId {
        &self.opaque_producer
    }

    pub const fn visibility(&self) -> AdapterNominalVisibility {
        self.visibility
    }

    pub fn source_label(&self) -> &str {
        &self.source_label
    }
}

impl AdapterRustPackageMountTable {
    /// Returns the exact mount prefix registered for one Rust package.
    pub fn get(&self, package: &ArcweftRustPackageId) -> Option<&AdapterNominalPathPrefix> {
        self.by_package.get(package)
    }

    pub(crate) fn insert(
        &mut self,
        package: ArcweftRustPackageId,
        prefix: AdapterNominalPathPrefix,
    ) -> Option<AdapterNominalPathPrefix> {
        self.by_package.insert(package, prefix)
    }

    /// Deterministic package-order iteration.
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ArcweftRustPackageId, &AdapterNominalPathPrefix)> {
        self.by_package.iter()
    }
}

fn validate_segments(
    segments: &[AdapterNominalPathSegment],
    empty_allowed: bool,
) -> Result<(), AdapterNominalPathError> {
    if !empty_allowed && segments.is_empty() {
        return Err(AdapterNominalPathError::Empty);
    }
    if segments.len() > MAX_NOMINAL_PATH_SEGMENTS {
        return Err(AdapterNominalPathError::SegmentLimit {
            observed: segments.len(),
            maximum: MAX_NOMINAL_PATH_SEGMENTS,
        });
    }
    for segment in segments {
        ArcweftRustTypePathSegment::try_new(segment.0.clone()).map_err(|_| {
            AdapterNominalPathError::InvalidSegment {
                segment: segment.0.clone(),
            }
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(value: &str) -> AdapterNominalPathSegment {
        AdapterNominalPathSegment::try_new(value).expect("valid segment")
    }

    #[test]
    fn prefix_join_preserves_exact_segment_identity() {
        let prefix = AdapterNominalPathPrefix::try_new([segment("vendor"), segment("tensor")])
            .expect("valid prefix");
        let local = ArcweftRustTypePath::try_new([
            ArcweftRustTypePathSegment::try_new("module").unwrap(),
            ArcweftRustTypePathSegment::try_new("Tensor").unwrap(),
        ])
        .unwrap();
        let joined = prefix.join(&local).expect("joined path");

        assert_eq!(
            joined
                .segments()
                .iter()
                .map(AdapterNominalPathSegment::as_str)
                .collect::<Vec<_>>(),
            ["vendor", "tensor", "module", "Tensor"]
        );
    }

    #[test]
    fn path_segment_rejects_raw_or_keyword_spellings() {
        assert!(AdapterNominalPathSegment::try_new("r#type").is_err());
        assert!(AdapterNominalPathSegment::try_new("type").is_err());
    }
}
