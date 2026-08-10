//! Filesystem containment for the host-selected project layout.

use arcweft_manifest_model::{BuildSpec, NormalizedProjectPath};
use arcweft_project::layout::ProjectLayoutSpec;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

/// One project-relative path admitted beneath the canonical project root.
///
/// Construction is private to this module so downstream code cannot bypass
/// canonical ancestor and symlink containment checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainedProjectPath(PathBuf);

impl ContainedProjectPath {
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Canonical filesystem roots derived from accepted build and host layout facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainedProjectLayout {
    project: PathBuf,
    source: ContainedProjectPath,
    target: ContainedProjectPath,
    asset: ContainedProjectPath,
    content: ContainedProjectPath,
}

impl ContainedProjectLayout {
    /// Resolves every configured root beneath one canonical project directory.
    ///
    /// Paths that do not exist yet are resolved through their nearest existing
    /// ancestor. Existing symlink ancestors are canonicalized before the
    /// missing suffix is appended, so a symlink cannot redirect a future path
    /// outside the project.
    pub fn try_new(
        project_root: &Path,
        build: &BuildSpec,
        layout: &ProjectLayoutSpec,
    ) -> Result<Self, ProjectLayoutContainmentError> {
        let project_root = canonical_project_root(project_root)?;
        let source_root = contain(&project_root, &build.source_dir, ProjectPathRole::Source)?;
        let target_root = contain(&project_root, &build.target_dir, ProjectPathRole::Target)?;
        let asset_root = contain(&project_root, layout.asset_dir(), ProjectPathRole::Asset)?;
        let content_root = contain(
            &project_root,
            layout.content_dir(),
            ProjectPathRole::Content,
        )?;

        Ok(Self {
            project: project_root,
            source: source_root,
            target: target_root,
            asset: asset_root,
            content: content_root,
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project
    }

    pub const fn source_root(&self) -> &ContainedProjectPath {
        &self.source
    }

    pub const fn target_root(&self) -> &ContainedProjectPath {
        &self.target
    }

    pub const fn asset_root(&self) -> &ContainedProjectPath {
        &self.asset
    }

    pub const fn content_root(&self) -> &ContainedProjectPath {
        &self.content
    }

    /// Contains one additional manifest-selected path beneath this layout's
    /// already canonical project root.
    pub fn contain_project_path(
        &self,
        relative: &NormalizedProjectPath,
        role: ProjectPathRole,
    ) -> Result<ContainedProjectPath, ProjectLayoutContainmentError> {
        contain(&self.project, relative, role)
    }
}

/// Resolves the physical workspace root used as the key space for overlays and
/// contained resources.
pub(crate) fn canonical_project_root(
    project_root: &Path,
) -> Result<PathBuf, ProjectLayoutContainmentError> {
    validate_absolute_normalized_root(project_root)?;
    fs::canonicalize(project_root).map_err(|source| {
        ProjectLayoutContainmentError::CanonicalizeProjectRoot {
            path: project_root.to_path_buf(),
            source,
        }
    })
}

/// Filesystem role reported by containment diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectPathRole {
    Source,
    Target,
    Asset,
    Content,
    ExternalMetadata,
    ResourceTypeManifest,
}

impl ProjectPathRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Target => "target",
            Self::Asset => "asset",
            Self::Content => "content",
            Self::ExternalMetadata => "external metadata",
            Self::ResourceTypeManifest => "resource type manifest",
        }
    }
}

impl std::fmt::Display for ProjectPathRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Failure to construct a filesystem-contained project layout.
#[derive(Debug, Error)]
pub enum ProjectLayoutContainmentError {
    #[error("project root `{path}` must be absolute and lexically normalized")]
    ProjectRoot { path: PathBuf },
    #[error("failed to canonicalize project root `{path}`: {source}")]
    CanonicalizeProjectRoot {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect {role} path ancestor `{path}`: {source}")]
    InspectAncestor {
        role: ProjectPathRole,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to canonicalize {role} path ancestor `{path}`: {source}")]
    CanonicalizeAncestor {
        role: ProjectPathRole,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "project.layout.uncontained: {role} path `{path}` resolves outside project root `{project_root}`"
    )]
    Uncontained {
        role: ProjectPathRole,
        path: PathBuf,
        project_root: PathBuf,
    },
}

fn validate_absolute_normalized_root(
    project_root: &Path,
) -> Result<(), ProjectLayoutContainmentError> {
    if !project_root.is_absolute()
        || project_root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(ProjectLayoutContainmentError::ProjectRoot {
            path: project_root.to_path_buf(),
        });
    }
    Ok(())
}

fn contain(
    project_root: &Path,
    relative: &NormalizedProjectPath,
    role: ProjectPathRole,
) -> Result<ContainedProjectPath, ProjectLayoutContainmentError> {
    let requested = project_root.join(relative.as_str());
    let mut ancestor = requested.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                let name = ancestor.file_name().ok_or_else(|| {
                    ProjectLayoutContainmentError::Uncontained {
                        role,
                        path: requested.clone(),
                        project_root: project_root.to_path_buf(),
                    }
                })?;
                missing.push(name.to_os_string());
                ancestor = ancestor.parent().ok_or_else(|| {
                    ProjectLayoutContainmentError::Uncontained {
                        role,
                        path: requested.clone(),
                        project_root: project_root.to_path_buf(),
                    }
                })?;
            }
            Err(source) => {
                return Err(ProjectLayoutContainmentError::InspectAncestor {
                    role,
                    path: ancestor.to_path_buf(),
                    source,
                });
            }
        }
    }

    let mut resolved = fs::canonicalize(ancestor).map_err(|source| {
        ProjectLayoutContainmentError::CanonicalizeAncestor {
            role,
            path: ancestor.to_path_buf(),
            source,
        }
    })?;
    if !resolved.starts_with(project_root) {
        return Err(ProjectLayoutContainmentError::Uncontained {
            role,
            path: requested,
            project_root: project_root.to_path_buf(),
        });
    }
    for segment in missing.iter().rev() {
        resolved.push(segment);
    }
    if !resolved.starts_with(project_root) {
        return Err(ProjectLayoutContainmentError::Uncontained {
            role,
            path: requested,
            project_root: project_root.to_path_buf(),
        });
    }
    Ok(ContainedProjectPath(resolved))
}

#[cfg(test)]
mod tests {
    use super::{ContainedProjectLayout, ProjectLayoutContainmentError, ProjectPathRole};
    use arcweft_manifest_model::{BuildSpec, NormalizedProjectPath};
    use arcweft_project::layout::ProjectLayoutSpec;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "arcweft-contained-layout-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock follows epoch")
                    .as_nanos()
            ));
            fs::create_dir_all(root.join("src")).expect("fixture root creates");
            Self { root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn canonical_layout_contains_existing_and_future_roots() {
        let fixture = Fixture::new();
        let layout = ContainedProjectLayout::try_new(
            &fixture.root,
            &BuildSpec::default(),
            &ProjectLayoutSpec::default(),
        )
        .expect("canonical layout");
        let canonical_root = fs::canonicalize(&fixture.root).expect("canonical fixture root");

        assert_eq!(layout.project_root(), canonical_root);
        assert_eq!(layout.source_root().as_path(), canonical_root.join("src"));
        assert_eq!(
            layout.target_root().as_path(),
            canonical_root.join("target/arcweft")
        );
        assert_eq!(layout.asset_root().as_path(), canonical_root.join("assets"));
        assert_eq!(
            layout.content_root().as_path(),
            canonical_root.join("content")
        );
    }

    #[test]
    fn project_root_must_be_absolute_and_lexically_normalized() {
        let error = ContainedProjectLayout::try_new(
            Path::new("relative/project"),
            &BuildSpec::default(),
            &ProjectLayoutSpec::default(),
        )
        .expect_err("relative root rejects");
        assert!(matches!(
            error,
            ProjectLayoutContainmentError::ProjectRoot { .. }
        ));
    }

    #[test]
    fn existing_ancestor_symlink_escape_is_rejected() {
        let fixture = Fixture::new();
        let outside = fixture
            .root
            .parent()
            .expect("temporary root has parent")
            .join(format!(
                "{}-outside",
                fixture
                    .root
                    .file_name()
                    .expect("fixture root has file name")
                    .to_string_lossy()
            ));
        fs::create_dir_all(&outside).expect("outside directory creates");
        let link = fixture.root.join("escaped");
        if let Err(source) = symlink_dir(&outside, &link) {
            if source.kind() == std::io::ErrorKind::PermissionDenied
                || source.raw_os_error() == Some(1314)
            {
                let _ = fs::remove_dir_all(outside);
                return;
            }
            panic!("symlink creates: {source}");
        }
        let layout = ProjectLayoutSpec::try_new(
            NormalizedProjectPath::new("escaped/assets").expect("asset path"),
            NormalizedProjectPath::new("content").expect("content path"),
        )
        .expect("disjoint host layout");

        let error = ContainedProjectLayout::try_new(&fixture.root, &BuildSpec::default(), &layout)
            .expect_err("symlink escape rejects");
        assert!(matches!(
            error,
            ProjectLayoutContainmentError::Uncontained {
                role: ProjectPathRole::Asset,
                ..
            }
        ));
        fs::remove_dir_all(outside).expect("outside directory removes");
    }

    #[cfg(unix)]
    fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(original, link)
    }

    #[cfg(windows)]
    fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(original, link)
    }
}
