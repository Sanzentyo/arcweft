use arcweft_adapter_context::{
    manifest::{AdapterManifest, AdapterRegistry},
    standard::{self, SANS_IO_ADAPTER_ID},
};
use arcweft_character::catalog::CharacterCatalog;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_launch::{LaunchProfileError, LaunchProfileManifest, ResolvedLaunchProfile};
use arcweft_runtime_host::RuntimeHostRunnerKind;
use arcweft_rust_abi::ArcweftRustManifest;
use arcweft_verify_lsp::{ArcweftLspContext, ArcweftLspProfileContextBuilder};
use lsp_types::Uri;
use std::{
    fmt, fs,
    ops::Range,
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
    dialogue_defaults: Option<String>,
    dialogue_defaults_selection: Option<ProfileSourceSelection>,
    characters: CharacterCatalog,
    diagnostics: Vec<LspProfileDiagnostic>,
    arbitrary_expression_type_inlays: bool,
}

/// Source location of a launch-profile-selected setting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileSourceSelection {
    path: PathBuf,
    source: String,
    value_range: Range<usize>,
}

impl LspProfile {
    /// Creates a profile from adapter metadata and a runner preset.
    pub fn new(adapter: AdapterManifest, runner: RuntimeHostRunnerKind) -> Self {
        Self {
            adapter,
            declared_manifests: Vec::new(),
            runner,
            dialogue_defaults: None,
            dialogue_defaults_selection: None,
            characters: CharacterCatalog::new(),
            diagnostics: Vec::new(),
            arbitrary_expression_type_inlays: false,
        }
    }

    /// Minimal built-in profile used before project metadata is loaded.
    pub fn default_for_runner(runner: RuntimeHostRunnerKind) -> Self {
        Self {
            adapter: standard::sans_io_manifest(),
            declared_manifests: Vec::new(),
            runner,
            dialogue_defaults: None,
            dialogue_defaults_selection: None,
            characters: CharacterCatalog::new(),
            diagnostics: Vec::new(),
            arbitrary_expression_type_inlays: false,
        }
    }

    /// Enables or disables expression-level type inlays for this profile.
    #[must_use]
    pub const fn with_arbitrary_expression_type_inlays(mut self, enabled: bool) -> Self {
        self.arbitrary_expression_type_inlays = enabled;
        self
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

    /// Dialogue defaults profile selected by the launch profile, if any.
    pub fn dialogue_defaults(&self) -> Option<&str> {
        self.dialogue_defaults.as_deref()
    }

    /// Character manifests selected by the active launch profile.
    pub const fn characters(&self) -> &CharacterCatalog {
        &self.characters
    }

    /// Source location of the launch profile's `dialogue_defaults` selection.
    pub fn dialogue_defaults_selection(&self) -> Option<&ProfileSourceSelection> {
        self.dialogue_defaults_selection.as_ref()
    }

    /// Whether expression-level type inlays are enabled for this profile.
    pub const fn arbitrary_expression_type_inlays(&self) -> bool {
        self.arbitrary_expression_type_inlays
    }

    /// Builds a Sans I/O LSP context for helper calls.
    pub fn context(&self) -> ArcweftLspContext<'_> {
        ArcweftLspProfileContextBuilder::new(&self.adapter)
            .with_runner_kind(self.runner)
            .build()
    }

    /// Builds the semantic environment selected by this profile.
    pub fn typecheck_env(&self) -> TypeCheckEnv {
        let env = self.adapter.apply_to_env(TypeCheckEnv::standard());
        self.characters.manifests().fold(
            env,
            arcweft_lang_sema::env::TypeCheckEnv::with_character_manifest,
        )
    }
}

/// Resolves LSP profile metadata from project manifests near opened documents.
#[derive(Clone, Debug)]
pub struct LspProfileResolver {
    runner: RuntimeHostRunnerKind,
    manifest_name: String,
    profile_id: Option<String>,
    arbitrary_expression_type_inlays: bool,
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
    /// A character manifest could not be read.
    CharacterManifestRead,
    /// A character manifest could not be parsed or validated.
    CharacterManifestParse,
    /// Character manifests declared duplicate public character ids.
    CharacterCatalog,
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
            arbitrary_expression_type_inlays: false,
        }
    }

    /// Carries editor-selected inlay policy into every resolved profile.
    #[must_use]
    pub const fn with_arbitrary_expression_type_inlays(mut self, enabled: bool) -> Self {
        self.arbitrary_expression_type_inlays = enabled;
        self
    }

    /// Minimal built-in profile used when no document-specific metadata is cached.
    pub fn default_profile(&self) -> LspProfile {
        LspProfile::default_for_runner(self.runner)
            .with_arbitrary_expression_type_inlays(self.arbitrary_expression_type_inlays)
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
        Ok(self.profile_from_resolved(
            &profile,
            manifest_dir,
            &manifest_path,
            &source,
            profile_id,
            standard_registry,
        ))
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
        manifest_path: &Path,
        manifest_source: &str,
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
        let characters = read_character_manifests(
            profile.character_manifests(),
            manifest_dir,
            profile_id,
            &mut diagnostics,
        );
        let declared_manifests = vec![adapter.clone()];
        LspProfile {
            adapter,
            declared_manifests,
            runner: self.runner,
            dialogue_defaults: profile.dialogue_defaults().map(str::to_owned),
            dialogue_defaults_selection: profile.dialogue_defaults().and_then(|selected| {
                dialogue_defaults_selection(manifest_path, manifest_source, profile_id, selected)
            }),
            characters,
            diagnostics,
            arbitrary_expression_type_inlays: self.arbitrary_expression_type_inlays,
        }
    }

    fn default_with_diagnostic(&self, diagnostic: LspProfileDiagnostic) -> LspProfile {
        let mut profile = LspProfile::default_for_runner(self.runner);
        profile.diagnostics.push(diagnostic);
        profile.with_arbitrary_expression_type_inlays(self.arbitrary_expression_type_inlays)
    }
}

impl ProfileSourceSelection {
    /// Manifest path containing the selected setting.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Manifest source text used to compute `value_range`.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Byte range of the selected value inside `source`.
    pub fn value_range(&self) -> Range<usize> {
        self.value_range.clone()
    }

    /// File URI for the manifest source.
    pub fn uri(&self) -> Option<Uri> {
        file_uri_from_path(&self.path)
    }
}

fn dialogue_defaults_selection(
    manifest_path: &Path,
    source: &str,
    profile_id: &str,
    selected: &str,
) -> Option<ProfileSourceSelection> {
    let table = profile_table_range(source, profile_id)?;
    let value_range = key_value_string_range(&source[table.clone()], "dialogue_defaults", selected)
        .map(|range| table.start + range.start..table.start + range.end)?;
    Some(ProfileSourceSelection {
        path: manifest_path.to_path_buf(),
        source: source.to_owned(),
        value_range,
    })
}

fn profile_table_range(source: &str, profile_id: &str) -> Option<Range<usize>> {
    let wanted = format!("[profiles.{profile_id}]");
    let quoted = format!("[profiles.\"{profile_id}\"]");
    let mut body_start = None;
    let mut cursor = 0usize;
    for line in source.split_inclusive('\n') {
        let line_start = cursor;
        let line_end = cursor + line.len();
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(start) = body_start {
                return Some(start..line_start);
            }
            if trimmed == wanted || trimmed == quoted {
                body_start = Some(line_end);
            }
        }
        cursor = line_end;
    }
    body_start.map(|start| start..source.len())
}

fn key_value_string_range(source: &str, key: &str, selected: &str) -> Option<Range<usize>> {
    let mut cursor = 0usize;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let leading = line.len() - trimmed.len();
        if let Some(rest) = trimmed.strip_prefix(key) {
            let rest_leading = rest.len() - rest.trim_start().len();
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                let value_leading = value.len() - value.trim_start().len();
                let value = value.trim_start();
                if let Some(quoted) = value.strip_prefix('"')
                    && let Some(close) = quoted.find('"')
                    && &quoted[..close] == selected
                {
                    let start = cursor
                        + leading
                        + key.len()
                        + rest_leading
                        + '='.len_utf8()
                        + value_leading
                        + '"'.len_utf8();
                    return Some(start..start + selected.len());
                }
            }
        }
        cursor += line.len();
    }
    None
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
            Self::CharacterManifestRead => "profile.character_manifest.read",
            Self::CharacterManifestParse => "profile.character_manifest.parse",
            Self::CharacterCatalog => "profile.character_manifest.catalog",
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
        match arcweft_project_loader::adapter_manifest::load(path) {
            Ok(manifest) => registry.with_manifest(manifest),
            Err(error) => {
                diagnostics.push(adapter_manifest_diagnostic(
                    &error,
                    path_label(path, manifest_dir),
                    profile_id,
                ));
                registry
            }
        }
    })
}

fn read_rust_metadata(
    paths: &[PathBuf],
    manifest_dir: &Path,
    profile_id: &str,
    diagnostics: &mut Vec<LspProfileDiagnostic>,
) -> Vec<ArcweftRustManifest> {
    paths
        .iter()
        .filter_map(
            |path| match arcweft_project_loader::rust_metadata::load(path) {
                Ok(manifest) => Some(manifest),
                Err(error) => {
                    diagnostics.push(rust_metadata_diagnostic(
                        &error,
                        path_label(path, manifest_dir),
                        profile_id,
                    ));
                    None
                }
            },
        )
        .collect()
}

fn read_character_manifests(
    paths: &[PathBuf],
    manifest_dir: &Path,
    profile_id: &str,
    diagnostics: &mut Vec<LspProfileDiagnostic>,
) -> CharacterCatalog {
    let mut catalog = CharacterCatalog::new();
    for path in paths {
        let resource = path_label(path, manifest_dir);
        match arcweft_project_loader::character_manifest::load(path) {
            Ok(manifest) => {
                if let Err(error) = catalog.insert(manifest) {
                    diagnostics.push(
                        LspProfileDiagnostic::new(
                            LspProfileDiagnosticKind::CharacterCatalog,
                            format!("{error} `{resource}`"),
                        )
                        .with_profile_id(profile_id)
                        .with_resource(resource),
                    );
                }
            }
            Err(error) => {
                diagnostics.push(character_manifest_diagnostic(&error, resource, profile_id));
            }
        }
    }
    catalog
}

fn character_manifest_diagnostic(
    error: &arcweft_project_loader::character_manifest::LoadError,
    resource: String,
    profile_id: &str,
) -> LspProfileDiagnostic {
    let kind = match error {
        arcweft_project_loader::character_manifest::LoadError::Read(_) => {
            LspProfileDiagnosticKind::CharacterManifestRead
        }
        arcweft_project_loader::character_manifest::LoadError::Parse(_) => {
            LspProfileDiagnosticKind::CharacterManifestParse
        }
    };
    LspProfileDiagnostic::new(kind, format!("{error} `{resource}`"))
        .with_profile_id(profile_id)
        .with_resource(resource)
}

fn adapter_manifest_diagnostic(
    error: &arcweft_project_loader::adapter_manifest::LoadError,
    resource: String,
    profile_id: &str,
) -> LspProfileDiagnostic {
    let kind = match error {
        arcweft_project_loader::adapter_manifest::LoadError::Read(_) => {
            LspProfileDiagnosticKind::AdapterManifestRead
        }
        arcweft_project_loader::adapter_manifest::LoadError::Parse(_) => {
            LspProfileDiagnosticKind::AdapterManifestParse
        }
    };
    LspProfileDiagnostic::new(kind, format!("{error} `{resource}`"))
        .with_profile_id(profile_id)
        .with_resource(resource)
}

fn rust_metadata_diagnostic(
    error: &arcweft_project_loader::rust_metadata::LoadError,
    resource: String,
    profile_id: &str,
) -> LspProfileDiagnostic {
    let kind = match error {
        arcweft_project_loader::rust_metadata::LoadError::Read(_) => {
            LspProfileDiagnosticKind::RustMetadataRead
        }
        arcweft_project_loader::rust_metadata::LoadError::Parse(_) => {
            LspProfileDiagnosticKind::RustMetadataParse
        }
    };
    LspProfileDiagnostic::new(kind, format!("{error} `{resource}`"))
        .with_profile_id(profile_id)
        .with_resource(resource)
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

pub(crate) fn file_path_from_uri(uri: &lsp_types::Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = percent_decode(path)?;
    Some(normalize_file_uri_path(&path))
}

fn file_uri_from_path(path: &Path) -> Option<Uri> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let body = if normalized
        .as_bytes()
        .get(1)
        .is_some_and(|byte| *byte == b':')
    {
        format!("/{normalized}")
    } else {
        normalized
    };
    format!("file://{}", percent_encode_file_path(&body))
        .parse()
        .ok()
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

fn percent_encode_file_path(input: &str) -> String {
    input
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                vec![char::from(byte)]
            }
            _ => {
                let mut encoded = ['%'; 3];
                encoded[1] = hex_digit(byte >> 4);
                encoded[2] = hex_digit(byte & 0x0f);
                encoded.to_vec()
            }
        })
        .collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'A' + value - 10),
        _ => '?',
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
return_type = "Unit"
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

        assert!(
            profile.diagnostics().is_empty(),
            "unexpected diagnostics: {:?}",
            profile.diagnostics()
        );
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

    #[test]
    fn invalid_adapter_manifest_diagnostic_keeps_profile_relative_resource() {
        let project = TestProject::new("lsp-profile-adapter-invalid");
        project.write(
            "arcw.toml",
            r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "missing"
adapter_manifests = ["adapters/bad.toml"]
"#,
        );
        project.write("src/main.arcw", "flow @.main main {}\n");
        project.write("adapters/bad.toml", "schema_version = ");
        let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

        let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
        let diagnostic = profile
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::AdapterManifestParse)
            .expect("adapter manifest parse diagnostic");

        assert_eq!(diagnostic.profile_id(), Some("dev"));
        assert_eq!(diagnostic.resource(), Some("adapters/bad.toml"));
        assert!(!diagnostic.message().contains(":/"));
        assert!(!diagnostic.message().contains(":\\"));
    }

    #[test]
    fn missing_rust_metadata_diagnostic_keeps_profile_relative_resource() {
        let project = TestProject::new("lsp-profile-rust-missing");
        project.write(
            "arcw.toml",
            r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "sans-io"
rust_metadata = ["target/arcweft/missing.json"]
"#,
        );
        project.write("src/main.arcw", "flow @.main main {}\n");
        let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

        let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
        let diagnostic = profile
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::RustMetadataRead)
            .expect("rust metadata read diagnostic");

        assert_eq!(diagnostic.profile_id(), Some("dev"));
        assert_eq!(diagnostic.resource(), Some("target/arcweft/missing.json"));
        assert!(!diagnostic.message().contains(":/"));
        assert!(!diagnostic.message().contains(":\\"));
    }

    #[test]
    fn invalid_rust_metadata_diagnostic_keeps_profile_relative_resource() {
        let project = TestProject::new("lsp-profile-rust-invalid");
        project.write(
            "arcw.toml",
            r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "sans-io"
rust_metadata = ["target/arcweft/bad.json"]
"#,
        );
        project.write("src/main.arcw", "flow @.main main {}\n");
        project.write("target/arcweft/bad.json", "{ not json");
        let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

        let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
        let diagnostic = profile
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::RustMetadataParse)
            .expect("rust metadata parse diagnostic");

        assert_eq!(diagnostic.profile_id(), Some("dev"));
        assert_eq!(diagnostic.resource(), Some("target/arcweft/bad.json"));
        assert!(!diagnostic.message().contains(":/"));
        assert!(!diagnostic.message().contains(":\\"));
    }

    #[test]
    fn resolves_dialogue_defaults_selection_source_range() {
        let project = TestProject::new("lsp-profile-dialogue-defaults-selection");
        let manifest = r#"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.mobile"

[profiles.other]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.debug"
"#;
        project.write("arcw.toml", manifest);
        project.write("src/main.arcw", "flow @.main main {}\n");
        let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

        let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
        let selection = profile
            .dialogue_defaults_selection()
            .expect("dialogue defaults source selection");
        let range = selection.value_range();

        assert_eq!(&selection.source()[range.clone()], "dialogue.mobile");
        assert_eq!(selection.path(), project.path("arcw.toml").as_path());
        assert!(selection.uri().is_some());
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
