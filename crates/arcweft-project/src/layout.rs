//! Non-serialized physical authoring layout selected by the project host.

use arcweft_manifest_model::NormalizedProjectPath;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Canonical physical directories used to resolve authored resources.
///
/// This value is host input rather than manifest semantic data and therefore
/// intentionally has no serialization implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectLayoutSpec {
    asset_dir: NormalizedProjectPath,
    content_dir: NormalizedProjectPath,
}

/// Resolved filesystem roots for authored asset and structured-content inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredResourceRoots {
    asset: PathBuf,
    content: PathBuf,
}

impl Default for ProjectLayoutSpec {
    fn default() -> Self {
        Self {
            asset_dir: NormalizedProjectPath::new("assets")
                .expect("static asset directory is valid"),
            content_dir: NormalizedProjectPath::new("content")
                .expect("static content directory is valid"),
        }
    }
}

impl ProjectLayoutSpec {
    /// Constructs disjoint normalized asset and content roots.
    pub fn try_new(
        asset_dir: NormalizedProjectPath,
        content_dir: NormalizedProjectPath,
    ) -> Result<Self, ProjectLayoutError> {
        if roots_overlap(&asset_dir, &content_dir) {
            return Err(ProjectLayoutError::Overlap {
                asset_dir,
                content_dir,
            });
        }
        Ok(Self {
            asset_dir,
            content_dir,
        })
    }

    pub const fn asset_dir(&self) -> &NormalizedProjectPath {
        &self.asset_dir
    }

    pub const fn content_dir(&self) -> &NormalizedProjectPath {
        &self.content_dir
    }
}

impl AuthoredResourceRoots {
    pub fn new(asset: impl Into<PathBuf>, content: impl Into<PathBuf>) -> Self {
        Self {
            asset: asset.into(),
            content: content.into(),
        }
    }

    pub fn asset(&self) -> &Path {
        &self.asset
    }

    pub fn content(&self) -> &Path {
        &self.content
    }
}

/// Invalid physical authoring layout.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectLayoutError {
    #[error(
        "project asset root `{asset_dir}` and content root `{content_dir}` overlap by portable path segments"
    )]
    Overlap {
        asset_dir: NormalizedProjectPath,
        content_dir: NormalizedProjectPath,
    },
}

fn roots_overlap(asset_dir: &NormalizedProjectPath, content_dir: &NormalizedProjectPath) -> bool {
    path_starts_with(asset_dir, content_dir) || path_starts_with(content_dir, asset_dir)
}

fn path_starts_with(candidate: &NormalizedProjectPath, prefix: &NormalizedProjectPath) -> bool {
    let candidate = candidate.as_str().split('/').collect::<Vec<_>>();
    let prefix = prefix.as_str().split('/').collect::<Vec<_>>();
    candidate.len() >= prefix.len()
        && candidate
            .iter()
            .zip(prefix)
            .all(|(candidate, prefix)| candidate.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::{ProjectLayoutError, ProjectLayoutSpec};
    use arcweft_manifest_model::NormalizedProjectPath;

    fn path(value: &str) -> NormalizedProjectPath {
        NormalizedProjectPath::new(value).expect("normalized test path")
    }

    #[test]
    fn canonical_layout_uses_assets_and_content() {
        let layout = ProjectLayoutSpec::default();
        assert_eq!(layout.asset_dir().as_str(), "assets");
        assert_eq!(layout.content_dir().as_str(), "content");
    }

    #[test]
    fn disjoint_custom_layout_is_accepted() {
        let layout = ProjectLayoutSpec::try_new(path("media/assets"), path("story/content"))
            .expect("disjoint layout");
        assert_eq!(layout.asset_dir().as_str(), "media/assets");
        assert_eq!(layout.content_dir().as_str(), "story/content");
    }

    #[test]
    fn portable_segment_overlap_is_rejected_case_insensitively() {
        for (asset_dir, content_dir) in [
            ("assets", "assets"),
            ("assets", "assets/story"),
            ("assets/story", "assets"),
            ("ASSETS", "assets/story"),
            ("media/Assets", "MEDIA/assets/story"),
        ] {
            assert!(matches!(
                ProjectLayoutSpec::try_new(path(asset_dir), path(content_dir)),
                Err(ProjectLayoutError::Overlap { .. })
            ));
        }
    }

    #[test]
    fn textual_prefix_without_a_segment_boundary_does_not_overlap() {
        assert!(ProjectLayoutSpec::try_new(path("assets"), path("assets-old")).is_ok());
    }
}
