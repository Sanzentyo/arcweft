use arcweft_adapter_context::{
    codec::{AdapterManifestCodecError, AdapterManifestFile},
    manifest::{AdapterManifest, AdapterRegistry},
    standard::{self, SANS_IO_ADAPTER_ID},
};
use arcweft_launch::{LaunchProfileError, LaunchProfileManifest, ResolvedLaunchProfile};
use arcweft_runtime_host::RuntimeHostRunnerKind;
use arcweft_rust_abi::{ArcweftRustAbiError, ArcweftRustManifest};
use arcweft_verify_lsp::{ArcweftLspContext, ArcweftLspProfileContextBuilder};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

const DEFAULT_MANIFEST_NAME: &str = "arcw.toml";

/// LSP-visible profile facts resolved outside the Sans I/O helper crate.
#[derive(Clone, Debug)]
pub struct LspProfile {
    adapter: AdapterManifest,
    declared_manifests: Vec<AdapterManifest>,
    runner: RuntimeHostRunnerKind,
    diagnostics: Vec<LspProfileDiagnostic>,
}

impl LspProfile {
    /// Creates a profile from adapter metadata and a runner preset.
    pub fn new(adapter: AdapterManifest, runner: RuntimeHostRunnerKind) -> Self {
        Self {
            adapter,
            declared_manifests: Vec::new(),
            runner,
            diagnostics: Vec::new(),
        }
    }

    /// Minimal built-in profile used before project metadata is loaded.
    pub fn default_for_runner(runner: RuntimeHostRunnerKind) -> Self {
        Self {
            adapter: standard::sans_io_manifest(),
            declared_manifests: Vec::new(),
            runner,
            diagnostics: Vec::new(),
        }
    }

    /// Adapter manifest selected for this profile.
    pub const fn adapter(&self) -> &AdapterManifest {
        &self.adapter
    }

    /// Runtime runner selected for this profile.
    pub const fn runner(&self) -> RuntimeHostRunnerKind {
        self.runner
    }

    /// Adapter manifests declared by the selected profile.
    pub fn declared_manifests(&self) -> &[AdapterManifest] {
        &self.declared_manifests
    }

    /// Profile-loading diagnostics that should be surfaced in the editor.
    pub fn diagnostics(&self) -> &[LspProfileDiagnostic] {
        &self.diagnostics
    }

    /// Builds a Sans I/O LSP context for helper calls.
    pub fn context(&self) -> ArcweftLspContext<'_> {
        ArcweftLspProfileContextBuilder::new(&self.adapter)
            .with_runner_kind(self.runner)
            .build()
    }
}

/// Resolves LSP profile metadata from project manifests near opened documents.
#[derive(Clone, Debug)]
pub struct LspProfileResolver {
    runner: RuntimeHostRunnerKind,
    manifest_name: String,
    profile_id: Option<String>,
}

/// One profile metadata diagnostic independent of source parsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LspProfileDiagnostic {
    kind: LspProfileDiagnosticKind,
    message: String,
    profile_id: Option<String>,
    resource: Option<String>,
}

/// Stable profile diagnostic categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LspProfileDiagnosticKind {
    /// The document URI was not a local file URI.
    NonFileDocumentUri,
    /// No project manifest was found for an opened document.
    WorkspaceManifestNotFound,
    /// The project manifest could not be read.
    ManifestRead,
    /// The project manifest could not be parsed.
    ManifestParse,
    /// The selected profile could not be resolved.
    ProfileResolve,
    /// A project-local adapter manifest could not be read.
    AdapterManifestRead,
    /// A project-local adapter manifest could not be parsed.
    AdapterManifestParse,
    /// Rust ABI metadata could not be read.
    RustMetadataRead,
    /// Rust ABI metadata could not be parsed.
    RustMetadataParse,
}

#[derive(Debug, Error)]
enum LspProfileLoadError {
    #[error("document URI is not a local file URI")]
    NonFileDocumentUri,
    #[error("no arcw.toml manifest was found for this document")]
    WorkspaceManifestNotFound,
    #[error("failed to read arcw.toml: {0}")]
    ManifestRead(std::io::Error),
    #[error("{0}")]
    ManifestParse(LaunchProfileError),
    #[error("{0}")]
    ProfileResolve(LaunchProfileError),
}

impl LspProfileResolver {
    /// Creates a resolver for one runner preset and optional explicit profile id.
    pub fn new(runner: RuntimeHostRunnerKind, profile_id: Option<String>) -> Self {
        Self {
            runner,
            manifest_name: DEFAULT_MANIFEST_NAME.to_owned(),
            profile_id,
        }
    }

    /// Minimal built-in profile used when no document-specific metadata is cached.
    pub fn default_profile(&self) -> LspProfile {
        LspProfile::default_for_runner(self.runner)
    }

    /// Resolves a profile for one LSP document URI.
    pub fn resolve_for_uri(&self, uri: &lsp_types::Uri) -> LspProfile {
        match file_path_from_uri(uri) {
            Some(path) => self.resolve_for_document_path(&path),
            None => self.default_with_diagnostic(LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::NonFileDocumentUri,
                LspProfileLoadError::NonFileDocumentUri.to_string(),
            )),
        }
    }

    /// Resolves a profile for one local document path.
    pub fn resolve_for_document_path(&self, document_path: &Path) -> LspProfile {
        self.try_resolve_for_document_path(document_path)
            .unwrap_or_else(|error| self.default_with_diagnostic(error.into_diagnostic()))
    }

    fn try_resolve_for_document_path(
        &self,
        document_path: &Path,
    ) -> Result<LspProfile, LspProfileLoadError> {
        let manifest_path = self
            .find_manifest(document_path)
            .ok_or(LspProfileLoadError::WorkspaceManifestNotFound)?;
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let source =
            fs::read_to_string(&manifest_path).map_err(LspProfileLoadError::ManifestRead)?;
        let manifest = LaunchProfileManifest::parse_toml(&source)
            .map_err(LspProfileLoadError::ManifestParse)?;
        let profile_id = self
            .profile_id
            .as_deref()
            .or_else(|| manifest.profiles().keys().next().map(String::as_str))
            .ok_or_else(|| {
                LspProfileLoadError::ProfileResolve(LaunchProfileError::MissingProfile(
                    "<default>".to_owned(),
                ))
            })?;
        let standard_registry = standard::standard_registry();
        let profile = manifest
            .resolve_profile_with_adapters(
                profile_id,
                manifest_dir,
                &standard_registry.adapter_ids(),
            )
            .map_err(LspProfileLoadError::ProfileResolve)?;
        Ok(self.profile_from_resolved(&profile, manifest_dir, profile_id, standard_registry))
    }

    fn find_manifest(&self, document_path: &Path) -> Option<PathBuf> {
        let start = document_path.parent().unwrap_or(document_path);
        start
            .ancestors()
            .map(|ancestor| ancestor.join(&self.manifest_name))
            .find(|candidate| candidate.is_file())
    }

    fn profile_from_resolved(
        &self,
        profile: &ResolvedLaunchProfile,
        manifest_dir: &Path,
        profile_id: &str,
        standard_registry: AdapterRegistry,
    ) -> LspProfile {
        let mut diagnostics = Vec::new();
        let registry = read_adapter_manifests(
            profile.adapter_manifests(),
            manifest_dir,
            profile_id,
            standard_registry,
            &mut diagnostics,
        );
        let mut adapter = registry
            .get(profile.adapter().unwrap_or(SANS_IO_ADAPTER_ID))
            .cloned()
            .unwrap_or_else(standard::sans_io_manifest);
        for rust_manifest in read_rust_metadata(
            profile.rust_metadata(),
            manifest_dir,
            profile_id,
            &mut diagnostics,
        ) {
            adapter = adapter.with_rust_manifest(&rust_manifest);
        }
        let declared_manifests = vec![adapter.clone()];
        LspProfile {
            adapter,
            declared_manifests,
            runner: self.runner,
            diagnostics,
        }
    }

    fn default_with_diagnostic(&self, diagnostic: LspProfileDiagnostic) -> LspProfile {
        let mut profile = LspProfile::default_for_runner(self.runner);
        profile.diagnostics.push(diagnostic);
        profile
    }
}

impl LspProfileDiagnostic {
    /// Creates a typed profile diagnostic.
    pub fn new(kind: LspProfileDiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            profile_id: None,
            resource: None,
        }
    }

    /// Attaches the selected launch profile id without embedding host paths.
    #[must_use]
    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = Some(profile_id.into());
        self
    }

    /// Attaches a profile-relative resource label.
    #[must_use]
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// Diagnostic category.
    pub const fn kind(&self) -> LspProfileDiagnosticKind {
        self.kind
    }

    /// Human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Optional launch profile id associated with this diagnostic.
    pub fn profile_id(&self) -> Option<&str> {
        self.profile_id.as_deref()
    }

    /// Optional profile-relative resource associated with this diagnostic.
    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }
}

impl LspProfileDiagnosticKind {
    /// Stable code used in LSP diagnostics.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NonFileDocumentUri => "profile.uri.non_file",
            Self::WorkspaceManifestNotFound => "profile.manifest.missing",
            Self::ManifestRead => "profile.manifest.read",
            Self::ManifestParse => "profile.manifest.parse",
            Self::ProfileResolve => "profile.resolve",
            Self::AdapterManifestRead => "profile.adapter_manifest.read",
            Self::AdapterManifestParse => "profile.adapter_manifest.parse",
            Self::RustMetadataRead => "profile.rust_metadata.read",
            Self::RustMetadataParse => "profile.rust_metadata.parse",
        }
    }
}

impl LspProfileLoadError {
    fn into_diagnostic(self) -> LspProfileDiagnostic {
        let kind = match self {
            Self::NonFileDocumentUri => LspProfileDiagnosticKind::NonFileDocumentUri,
            Self::WorkspaceManifestNotFound => LspProfileDiagnosticKind::WorkspaceManifestNotFound,
            Self::ManifestRead(_) => LspProfileDiagnosticKind::ManifestRead,
            Self::ManifestParse(_) => LspProfileDiagnosticKind::ManifestParse,
            Self::ProfileResolve(_) => LspProfileDiagnosticKind::ProfileResolve,
        };
        LspProfileDiagnostic::new(kind, self.to_string())
    }
}

fn read_adapter_manifests(
    paths: &[PathBuf],
    manifest_dir: &Path,
    profile_id: &str,
    registry: AdapterRegistry,
    diagnostics: &mut Vec<LspProfileDiagnostic>,
) -> AdapterRegistry {
    paths.iter().fold(registry, |registry, path| {
        match read_adapter_manifest(path) {
            Ok(manifest) => registry.with_manifest(manifest),
            Err(error) => {
                diagnostics.push(error.into_diagnostic(path_label(path, manifest_dir), profile_id));
                registry
            }
        }
    })
}

fn read_adapter_manifest(path: &Path) -> Result<AdapterManifest, AdapterManifestReadError> {
    let source = fs::read_to_string(path).map_err(AdapterManifestReadError::Read)?;
    let file = match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => AdapterManifestFile::from_json(&source),
        _ => AdapterManifestFile::from_toml(&source),
    }
    .map_err(AdapterManifestReadError::Parse)?;
    Ok(file.into_manifest())
}

fn read_rust_metadata(
    paths: &[PathBuf],
    manifest_dir: &Path,
    profile_id: &str,
    diagnostics: &mut Vec<LspProfileDiagnostic>,
) -> Vec<ArcweftRustManifest> {
    paths
        .iter()
        .filter_map(|path| match read_rust_manifest(path) {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                diagnostics.push(error.into_diagnostic(path_label(path, manifest_dir), profile_id));
                None
            }
        })
        .collect()
}

fn read_rust_manifest(path: &Path) -> Result<ArcweftRustManifest, RustMetadataReadError> {
    let source = fs::read_to_string(path).map_err(RustMetadataReadError::Read)?;
    ArcweftRustManifest::from_json(&source).map_err(RustMetadataReadError::Parse)
}

#[derive(Debug, Error)]
enum AdapterManifestReadError {
    #[error("failed to read adapter manifest: {0}")]
    Read(std::io::Error),
    #[error("failed to parse adapter manifest: {0}")]
    Parse(AdapterManifestCodecError),
}

#[derive(Debug, Error)]
enum RustMetadataReadError {
    #[error("failed to read Rust ABI metadata: {0}")]
    Read(std::io::Error),
    #[error("failed to parse Rust ABI metadata: {0}")]
    Parse(ArcweftRustAbiError),
}

impl AdapterManifestReadError {
    fn into_diagnostic(self, resource: String, profile_id: &str) -> LspProfileDiagnostic {
        let kind = match self {
            Self::Read(_) => LspProfileDiagnosticKind::AdapterManifestRead,
            Self::Parse(_) => LspProfileDiagnosticKind::AdapterManifestParse,
        };
        LspProfileDiagnostic::new(kind, format!("{self} `{resource}`"))
            .with_profile_id(profile_id)
            .with_resource(resource)
    }
}

impl RustMetadataReadError {
    fn into_diagnostic(self, resource: String, profile_id: &str) -> LspProfileDiagnostic {
        let kind = match self {
            Self::Read(_) => LspProfileDiagnosticKind::RustMetadataRead,
            Self::Parse(_) => LspProfileDiagnosticKind::RustMetadataParse,
        };
        LspProfileDiagnostic::new(kind, format!("{self} `{resource}`"))
            .with_profile_id(profile_id)
            .with_resource(resource)
    }
}

fn path_label(path: &Path, manifest_dir: &Path) -> String {
    let display_path = path.strip_prefix(manifest_dir).unwrap_or(path);
    let components = display_path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.is_empty() {
        path.file_name().map_or_else(
            || "metadata".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    } else {
        components.join("/")
    }
}

fn file_path_from_uri(uri: &lsp_types::Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = percent_decode(path)?;
    Some(normalize_file_uri_path(&path))
}

fn normalize_file_uri_path(path: &str) -> PathBuf {
    let without_leading_windows_slash = path
        .strip_prefix('/')
        .filter(|rest| rest.as_bytes().get(1).is_some_and(|byte| *byte == b':'))
        .unwrap_or(path);
    let normalized = without_leading_windows_slash.replace('/', std::path::MAIN_SEPARATOR_STR);
    PathBuf::from(normalized)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

impl fmt::Display for LspProfileDiagnosticKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_rust_abi::{
        ArcweftRustFunction, ArcweftRustPackage, ArcweftRustParam, ArcweftRustPurity,
        ArcweftRustTypeRef,
    };
    use std::{
        fs::{create_dir_all, write},
        path::Component,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn resolves_project_profile_adapter_and_rust_metadata() {
        let project = TestProject::new("lsp-profile-resolve");
        project.write(
            "arcw.toml",
            r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "custom-echo"
adapter_manifests = ["adapters/custom-echo.toml"]
rust_metadata = ["target/arcweft/custom.json"]
"#,
        );
        project.write("src/main.arcw", "flow @.main main {}\n");
        project.write(
            "adapters/custom-echo.toml",
            r#"
schema_version = 1
id = "custom-echo"
display_name = "Custom Echo"

[[functions]]
name = "custom.echo"
return_type = "String"
params = [{ name = "value", ty = "String" }]

[[host_calls]]
id = "custom.echo"
"#,
        );
        let rust_manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            name: "custom_adapter".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_function(ArcweftRustFunction {
            name: "custom.score".to_owned(),
            rust_path: "custom_adapter::score".to_owned(),
            params: vec![ArcweftRustParam {
                name: "value".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::I64,
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        });
        project.write(
            "target/arcweft/custom.json",
            &rust_manifest.to_json_pretty().expect("metadata json"),
        );

        let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
        let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));

        assert!(profile.diagnostics().is_empty());
        assert_eq!(profile.adapter().id().as_str(), "custom-echo");
        assert!(
            profile
                .adapter()
                .functions()
                .iter()
                .any(|function| function.name() == "custom.echo")
        );
        assert!(
            profile
                .adapter()
                .rust_functions()
                .iter()
                .any(|function| function.name() == "custom.score")
        );
    }

    #[test]
    fn missing_manifest_is_reported_without_absolute_path() {
        let project = TestProject::new("lsp-profile-missing");
        project.write("src/main.arcw", "flow @.main main {}\n");
        let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, None);

        let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));

        assert_eq!(
            profile.diagnostics()[0].kind(),
            LspProfileDiagnosticKind::WorkspaceManifestNotFound
        );
        assert!(!profile.diagnostics()[0].message().contains(":/"));
        assert!(!profile.diagnostics()[0].message().contains(":\\"));
    }

    #[test]
    fn adapter_manifest_diagnostic_keeps_profile_relative_resource() {
        let project = TestProject::new("lsp-profile-adapter-diagnostic");
        project.write(
            "arcw.toml",
            r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "missing"
adapter_manifests = ["adapters/missing.toml"]
"#,
        );
        project.write("src/main.arcw", "flow @.main main {}\n");
        let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

        let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
        let diagnostic = profile
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::AdapterManifestRead)
            .expect("adapter manifest diagnostic");

        assert_eq!(diagnostic.profile_id(), Some("dev"));
        assert_eq!(diagnostic.resource(), Some("adapters/missing.toml"));
        assert!(!diagnostic.message().contains(":/"));
        assert!(!diagnostic.message().contains(":\\"));
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("{name}-{unique}"));
            create_dir_all(&root).expect("create test project root");
            Self { root }
        }

        fn path(&self, path: &str) -> PathBuf {
            self.root.join(path)
        }

        fn write(&self, path: &str, contents: &str) {
            let path = self.path(path);
            if let Some(parent) = path.parent() {
                create_dir_all(parent).expect("create parent");
            }
            write(path, contents).expect("write fixture");
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            if self
                .root
                .components()
                .any(|component| matches!(component, Component::Normal(_)))
            {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }
}
