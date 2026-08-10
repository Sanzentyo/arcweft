use arcweft_source::SourceSpan;

/// Stable failure category for the resource extension-manifest boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceManifestDiagnosticCode {
    InvalidUtf8,
    BomNotAllowed,
    ByteLimit,
    DepthLimit,
    NodeLimit,
    StringLimit,
    CollectionLimit,
    RecordLimit,
    WorkLimit,
    InvalidJson,
    DuplicateKey,
    RootWrongShape,
    MissingFormat,
    MalformedFormat,
    UnsupportedFormat,
    MissingSchemaVersion,
    MalformedSchemaVersion,
    UnsupportedSchemaVersion,
    UnknownField,
    MissingField,
    NullNotAllowed,
    WrongShape,
    UnknownTag,
    WrongTagContent,
    InvalidInteger,
    IntegerOverflow,
    NonFiniteFloat,
    NonCanonicalFloat,
    InvalidString,
    InvalidId,
    InvalidDigest,
    DuplicateRecord,
    PackageMismatch,
    VersionConflict,
    UnresolvedPackage,
    DescriptorDigestMismatch,
    RegistryValidation,
    ArtifactMalformed,
    ArtifactNonCanonicalManifest,
    ArtifactDigestMismatch,
    RegistryDigestMismatch,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceManifestRelatedSpan {
    label: String,
    span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceManifestDiagnostic {
    code: ResourceManifestDiagnosticCode,
    message: String,
    primary: SourceSpan,
    related: Box<[ResourceManifestRelatedSpan]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceManifestReport {
    diagnostics: Box<[ResourceManifestDiagnostic]>,
}

impl ResourceManifestDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "resource_manifest.invalid_utf8",
            Self::BomNotAllowed => "resource_manifest.bom_not_allowed",
            Self::ByteLimit => "resource_manifest.byte_limit",
            Self::DepthLimit => "resource_manifest.depth_limit",
            Self::NodeLimit => "resource_manifest.node_limit",
            Self::StringLimit => "resource_manifest.string_limit",
            Self::CollectionLimit => "resource_manifest.collection_limit",
            Self::RecordLimit => "resource_manifest.record_limit",
            Self::WorkLimit => "resource_manifest.work_limit",
            Self::InvalidJson => "resource_manifest.invalid_json",
            Self::DuplicateKey => "resource_manifest.duplicate_key",
            Self::RootWrongShape => "resource_manifest.root_wrong_shape",
            Self::MissingFormat => "resource_manifest.missing_format",
            Self::MalformedFormat => "resource_manifest.malformed_format",
            Self::UnsupportedFormat => "resource_manifest.unsupported_format",
            Self::MissingSchemaVersion => "resource_manifest.missing_schema_version",
            Self::MalformedSchemaVersion => "resource_manifest.malformed_schema_version",
            Self::UnsupportedSchemaVersion => "resource_manifest.unsupported_schema_version",
            Self::UnknownField => "resource_manifest.unknown_field",
            Self::MissingField => "resource_manifest.missing_field",
            Self::NullNotAllowed => "resource_manifest.null_not_allowed",
            Self::WrongShape => "resource_manifest.wrong_shape",
            Self::UnknownTag => "resource_manifest.unknown_tag",
            Self::WrongTagContent => "resource_manifest.wrong_tag_content",
            Self::InvalidInteger => "resource_manifest.invalid_integer",
            Self::IntegerOverflow => "resource_manifest.integer_overflow",
            Self::NonFiniteFloat => "resource_manifest.non_finite_float",
            Self::NonCanonicalFloat => "resource_manifest.non_canonical_float",
            Self::InvalidString => "resource_manifest.invalid_string",
            Self::InvalidId => "resource_manifest.invalid_id",
            Self::InvalidDigest => "resource_manifest.invalid_digest",
            Self::DuplicateRecord => "resource_manifest.duplicate_record",
            Self::PackageMismatch => "resource_manifest.package_mismatch",
            Self::VersionConflict => "resource_manifest.version_conflict",
            Self::UnresolvedPackage => "resource_manifest.unresolved_package",
            Self::DescriptorDigestMismatch => "resource_manifest.descriptor_digest_mismatch",
            Self::RegistryValidation => "resource_manifest.registry_validation",
            Self::ArtifactMalformed => "resource_manifest.artifact_malformed",
            Self::ArtifactNonCanonicalManifest => {
                "resource_manifest.artifact_non_canonical_manifest"
            }
            Self::ArtifactDigestMismatch => "resource_manifest.artifact_digest_mismatch",
            Self::RegistryDigestMismatch => "resource_manifest.registry_digest_mismatch",
        }
    }
}

impl std::fmt::Display for ResourceManifestReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.diagnostics.first() {
            Some(diagnostic) => write!(
                formatter,
                "{}: {}",
                diagnostic.code.as_str(),
                diagnostic.message
            ),
            None => formatter.write_str("resource manifest report contains no diagnostics"),
        }
    }
}

impl std::error::Error for ResourceManifestReport {}

impl ResourceManifestRelatedSpan {
    pub fn new(label: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            label: label.into(),
            span,
        }
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

impl ResourceManifestDiagnostic {
    pub fn new(
        code: ResourceManifestDiagnosticCode,
        message: impl Into<String>,
        primary: SourceSpan,
        related: impl IntoIterator<Item = ResourceManifestRelatedSpan>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            primary,
            related: related.into_iter().collect(),
        }
    }
    pub const fn code(&self) -> ResourceManifestDiagnosticCode {
        self.code
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub const fn primary(&self) -> &SourceSpan {
        &self.primary
    }
    pub fn related(&self) -> &[ResourceManifestRelatedSpan] {
        &self.related
    }
}

impl ResourceManifestReport {
    pub fn one(diagnostic: ResourceManifestDiagnostic) -> Self {
        Self {
            diagnostics: Box::new([diagnostic]),
        }
    }
    pub fn new(diagnostics: impl IntoIterator<Item = ResourceManifestDiagnostic>) -> Self {
        let mut diagnostics = diagnostics.into_iter().collect::<Vec<_>>();
        diagnostics.sort_by(|left, right| {
            left.primary
                .source()
                .id()
                .cmp(right.primary.source().id())
                .then_with(|| {
                    left.primary
                        .source()
                        .revision()
                        .cmp(&right.primary.source().revision())
                })
                .then_with(|| {
                    left.primary
                        .range()
                        .start()
                        .cmp(&right.primary.range().start())
                })
                .then_with(|| left.primary.range().end().cmp(&right.primary.range().end()))
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.related.cmp(&right.related))
        });
        Self {
            diagnostics: diagnostics.into_boxed_slice(),
        }
    }
    pub fn diagnostics(&self) -> &[ResourceManifestDiagnostic] {
        &self.diagnostics
    }
}
