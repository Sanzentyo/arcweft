use std::path::{Component, Path};
use thiserror::Error;

/// Stable owner of one profile-topology resource.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyOwnerId {
    /// Resource owned by one isolated workspace and primary manifest.
    Workspace {
        root_uri: String,
        manifest_uri: String,
    },
    /// Resource owned by one exact dependency package identity.
    Dependency { package_id: String },
}

/// Normalized slash-separated path relative to its topology owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProfileTopologyLogicalPath(String);

/// Owner-qualified identity of one exact topology resource.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProfileTopologyResourceId {
    owner: ProfileTopologyOwnerId,
    path: ProfileTopologyLogicalPath,
}

/// Invalid topology-owner identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProfileTopologyIdError {
    #[error("topology owner field `{field}` must not be empty")]
    Empty { field: &'static str },
    #[error("topology owner field `{field}` contains a control character at byte {byte}")]
    Control { field: &'static str, byte: usize },
}

/// Invalid topology logical path.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProfileTopologyPathError {
    #[error("topology logical path must not be empty")]
    Empty,
    #[error("topology logical path must use normalized `/` separators")]
    Separator,
    #[error("topology logical path must be relative")]
    Absolute,
    #[error("topology logical path contains invalid component `{component}`")]
    Component { component: String },
    #[error("topology logical path contains a control character at byte {byte}")]
    Control { byte: usize },
}

impl ProfileTopologyOwnerId {
    pub fn workspace(
        root_uri: impl Into<String>,
        manifest_uri: impl Into<String>,
    ) -> Result<Self, ProfileTopologyIdError> {
        let root_uri = checked_owner_field("root_uri", root_uri.into())?;
        let manifest_uri = checked_owner_field("manifest_uri", manifest_uri.into())?;
        Ok(Self::Workspace {
            root_uri,
            manifest_uri,
        })
    }

    pub fn dependency(package_id: impl Into<String>) -> Result<Self, ProfileTopologyIdError> {
        Ok(Self::Dependency {
            package_id: checked_owner_field("package_id", package_id.into())?,
        })
    }

    pub fn root_uri(&self) -> Option<&str> {
        match self {
            Self::Workspace { root_uri, .. } => Some(root_uri),
            Self::Dependency { .. } => None,
        }
    }

    pub fn manifest_uri(&self) -> Option<&str> {
        match self {
            Self::Workspace { manifest_uri, .. } => Some(manifest_uri),
            Self::Dependency { .. } => None,
        }
    }

    pub fn package_id(&self) -> Option<&str> {
        match self {
            Self::Dependency { package_id } => Some(package_id),
            Self::Workspace { .. } => None,
        }
    }
}

impl ProfileTopologyLogicalPath {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProfileTopologyPathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProfileTopologyPathError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(ProfileTopologyPathError::Control { byte });
        }
        if value.contains('\\') {
            return Err(ProfileTopologyPathError::Separator);
        }
        let path = Path::new(&value);
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::RootDir | Component::Prefix(_)))
        {
            return Err(ProfileTopologyPathError::Absolute);
        }
        for component in value.split('/') {
            if component.is_empty() || matches!(component, "." | "..") {
                return Err(ProfileTopologyPathError::Component {
                    component: component.to_owned(),
                });
            }
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ProfileTopologyResourceId {
    pub fn new(owner: ProfileTopologyOwnerId, path: ProfileTopologyLogicalPath) -> Self {
        Self { owner, path }
    }

    pub const fn owner(&self) -> &ProfileTopologyOwnerId {
        &self.owner
    }

    pub const fn path(&self) -> &ProfileTopologyLogicalPath {
        &self.path
    }
}

fn checked_owner_field(
    field: &'static str,
    value: String,
) -> Result<String, ProfileTopologyIdError> {
    if value.is_empty() {
        return Err(ProfileTopologyIdError::Empty { field });
    }
    if let Some((byte, _)) = value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(ProfileTopologyIdError::Control { field, byte });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_paths_require_normalized_relative_components() {
        assert_eq!(
            ProfileTopologyLogicalPath::try_new("characters/akane.awchar.json")
                .expect("normalized path")
                .as_str(),
            "characters/akane.awchar.json"
        );
        for invalid in ["", "/root", "a\\b", "a//b", "a/./b", "a/../b"] {
            assert!(
                ProfileTopologyLogicalPath::try_new(invalid).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn owner_fields_are_nonempty_and_control_free() {
        assert!(
            ProfileTopologyOwnerId::workspace("file:///root", "file:///root/arcw.toml").is_ok()
        );
        assert!(ProfileTopologyOwnerId::workspace("", "file:///manifest").is_err());
        assert!(ProfileTopologyOwnerId::dependency("registry:game@1").is_ok());
        assert!(ProfileTopologyOwnerId::dependency("bad\npackage").is_err());
    }
}
