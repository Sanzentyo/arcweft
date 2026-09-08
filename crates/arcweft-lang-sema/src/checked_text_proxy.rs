//! Generation-bound semantic authority for explicit rich-text object proxies.
//!
//! Source spellings, formatter discovery, and runtime DTOs are deliberately
//! absent from this module. A checked application is joined to exactly one
//! project nominal declaration and its declaration-ordered semantic fields.

use std::collections::BTreeMap;

use arcweft_id::PublicId;
use arcweft_lang_hir::{identity::ExprId, symbol::nominal::ProjectNominalDeclarationId};
use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};
use thiserror::Error;

use crate::callable::CheckedCallApplicationDigest;
use crate::{
    checked_rich_text::{
        CheckedAngle, CheckedColor, CheckedDuration, CheckedLength, CheckedObjectDepth, Milli,
        RatioMilli,
    },
    record_field::AcceptedRecordFieldSemanticId,
    semantic_coordinate::{SemanticCoordinateEncodingError, StableCheckedValueCoordinate},
    types::{AcceptedVariantCaseSemanticId, ProjectNominalType, SemanticTypeDigest, TypeKind},
};

mod defaults;
mod prepared;
pub(crate) use defaults::{reduce_color_argument, reduce_enum_value, reduce_literal_expression};
pub(crate) use prepared::{
    PreparedCheckedTextProxyApplication, PreparedCheckedTextProxyCatalog,
    PreparedCheckedTextProxyDefinition, PreparedCheckedTextProxyOrigin,
    PreparedCheckedTextProxyValue, TextProxyFinalSealAuthority,
};

/// Canonical metadata coordinate that can be supplied by a proxy attribute.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TextProxyMetadataRole {
    Role,
    Layer,
    Depth,
    HitTest,
}

impl TextProxyMetadataRole {
    /// Source-facing name of the metadata coordinate.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Role => "role",
            Self::Layer => "layer",
            Self::Depth => "depth",
            Self::HitTest => "hit_test",
        }
    }
}

/// Typed expectation retained by one declaration-default diagnostic.
///
/// `Unknown` is used only for malformed or unknown default arguments where no
/// scalar or metadata contract can be named without guessing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextProxyDefaultExpectation {
    Scalar(CheckedCompileTimeScalarKind),
    Metadata(TextProxyMetadataRole),
    Unknown,
}

impl TextProxyDefaultExpectation {
    pub const fn kind(&self) -> Option<&CheckedCompileTimeScalarKind> {
        match self {
            Self::Scalar(kind) => Some(kind),
            Self::Metadata(_) | Self::Unknown => None,
        }
    }

    pub const fn role(&self) -> Option<TextProxyMetadataRole> {
        match self {
            Self::Metadata(role) => Some(*role),
            Self::Scalar(_) | Self::Unknown => None,
        }
    }
}

/// Authored cause retained by one text-proxy declaration diagnostic.
///
/// The expression/type/call cases intentionally carry no final-analysis error:
/// those errors may contain analyzer-owned state and are not safe to retain in
/// a declaration catalog. Reduction failures use the closed scalar-reduction
/// algebra instead.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TextProxyDeclarationDiagnosticCause {
    #[error("text-proxy default argument has an invalid HIR shape")]
    InvalidArgument,
    #[error("text-proxy declaration default is specified more than once")]
    DuplicateDefault,
    #[error("text-proxy declaration default names no metadata coordinate or schema field")]
    UnknownDefault,
    #[error("text-proxy default expression is not an admitted authored expression")]
    Expression,
    #[error("text-proxy default expression has the wrong type")]
    Type,
    #[error("text-proxy default call is not an admitted authored call")]
    Call,
    #[error("text-proxy default reduction failed: {0}")]
    Reduction(#[source] CompileTimeScalarReductionError),
}

impl TextProxyDeclarationDiagnosticCause {
    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::InvalidArgument => "sema.rich_text.proxy.invalid_default_argument",
            Self::DuplicateDefault => "sema.rich_text.proxy.duplicate_default",
            Self::UnknownDefault => "sema.rich_text.proxy.unknown_default",
            Self::Expression => "sema.rich_text.proxy.invalid_default_expression",
            Self::Type => "sema.rich_text.proxy.invalid_default_type",
            Self::Call => "sema.rich_text.proxy.invalid_default_call",
            Self::Reduction(_) => "sema.rich_text.proxy.invalid_default",
        }
    }
}

/// One source-backed, typed failure while reducing a declaration default.
///
/// This is deliberately a closed algebra rather than a unit `Result` error, so
/// every authored scalar failure has one stable semantic cause and analyzer
/// invariants cannot be mistaken for recoverable declaration input.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileTimeScalarReductionError {
    #[error("default expression has the wrong HIR shape")]
    WrongHir,
    #[error("default literal is not admitted by the expected scalar kind")]
    WrongLiteral,
    #[error("default literal uses a sign that is not admitted by the scalar kind")]
    WrongSign,
    #[error("default literal uses a unit that is not admitted by the scalar kind")]
    WrongUnit,
    #[error("default literal loses precision at the admitted fixed-point scale")]
    PrecisionLoss,
    #[error("default literal overflows the admitted scalar representation")]
    Overflow,
    #[error("default literal is outside the admitted scalar range")]
    OutOfRange,
    #[error("default expression is not the exact admitted builtin call")]
    WrongBuiltinCall,
    #[error("default enum expression belongs to the wrong declaration")]
    WrongEnumDeclaration,
    #[error("default enum expression selects an unadmitted case")]
    WrongEnumCase,
    #[error("default enum expression carries a payload")]
    WrongEnumPayload,
}

/// Complete typed diagnostic for one authored text-proxy declaration default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextProxyDeclarationDiagnostic {
    declaration: ProjectNominalDeclarationId,
    attribute: CheckedTextProxyAttributeOrigin,
    default_name: Option<ModuleSegment>,
    expression: ExprId,
    span: SourceSpan,
    expected: TextProxyDefaultExpectation,
    cause: TextProxyDeclarationDiagnosticCause,
}

impl TextProxyDeclarationDiagnostic {
    pub(crate) fn new(
        declaration: ProjectNominalDeclarationId,
        attribute: CheckedTextProxyAttributeOrigin,
        default_name: Option<ModuleSegment>,
        expression: ExprId,
        span: SourceSpan,
        expected: TextProxyDefaultExpectation,
        cause: TextProxyDeclarationDiagnosticCause,
    ) -> Self {
        Self {
            declaration,
            attribute,
            default_name,
            expression,
            span,
            expected,
            cause,
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub const fn attribute(&self) -> &CheckedTextProxyAttributeOrigin {
        &self.attribute
    }

    pub const fn default_name(&self) -> Option<&ModuleSegment> {
        self.default_name.as_ref()
    }

    /// Returns the default name without exposing the module-path wrapper.
    pub fn name(&self) -> Option<&str> {
        self.default_name().map(ModuleSegment::as_str)
    }

    pub const fn expression(&self) -> ExprId {
        self.expression
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub const fn expected(&self) -> &TextProxyDefaultExpectation {
        &self.expected
    }

    pub const fn cause(&self) -> &TextProxyDeclarationDiagnosticCause {
        &self.cause
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        self.cause.diagnostic_code()
    }

    /// Projects this typed declaration failure into the general source
    /// diagnostic carrier without losing the typed cause retained above.
    #[must_use]
    pub fn source_diagnostic(&self) -> Diagnostic {
        let name = self.name().unwrap_or("<unknown>");
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("invalid text-proxy default `{name}`: {}", self.cause),
        )
        .with_code(self.diagnostic_code())
        .with_label(DiagnosticLabel::primary(
            self.span.clone(),
            Some("invalid declaration default".to_owned()),
        ))
    }
}

/// Exact declaration attribute that admitted one proxy schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTextProxyAttributeFamily {
    TextProxy,
    RichTextProxy,
}

impl CheckedTextProxyAttributeFamily {
    /// Stable zero-based tag for the declaration-attribute family.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::TextProxy => 0,
            Self::RichTextProxy => 1,
        }
    }

    /// Canonical diagnostic/runtime provenance label.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::TextProxy => "text_proxy",
            Self::RichTextProxy => "rich_text_proxy",
        }
    }
}

/// Exact project-enum schema admitted for one closed-enum proxy field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCompileTimeScalarEnum {
    declaration: ProjectNominalDeclarationId,
    semantic_type: SemanticTypeDigest,
    cases: Box<[CheckedCompileTimeScalarEnumCase]>,
}

/// Declaration-ordered payload-free project-enum case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCompileTimeScalarEnumCase {
    ordinal: u32,
    semantic_id: AcceptedVariantCaseSemanticId,
    diagnostic_name: ModuleSegment,
}

/// Shared closed scalar-kind algebra admitted by compile-time consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCompileTimeScalarKind {
    Bool,
    Int,
    Milli,
    Ratio,
    Length,
    Angle,
    Duration,
    ClosedEnum(CheckedCompileTimeScalarEnum),
    PublicId,
    Text,
    Color,
}

impl CheckedCompileTimeScalarKind {
    /// Projects this checked scalar into the callable-owned admission
    /// algebra without reinterpreting a source spelling.
    pub(crate) const fn callable_kind(&self) -> crate::callable::CallableCompileTimeScalarKind {
        match self {
            Self::Bool => crate::callable::CallableCompileTimeScalarKind::Bool,
            Self::Int => crate::callable::CallableCompileTimeScalarKind::Int,
            Self::Milli => crate::callable::CallableCompileTimeScalarKind::Milli,
            Self::Ratio => crate::callable::CallableCompileTimeScalarKind::Ratio,
            Self::Length => crate::callable::CallableCompileTimeScalarKind::Length,
            Self::Angle => crate::callable::CallableCompileTimeScalarKind::Angle,
            Self::Duration => crate::callable::CallableCompileTimeScalarKind::Duration,
            Self::ClosedEnum(schema) => {
                crate::callable::CallableCompileTimeScalarKind::ClosedEnum(schema.semantic_type())
            }
            Self::PublicId => crate::callable::CallableCompileTimeScalarKind::PublicId,
            Self::Text => crate::callable::CallableCompileTimeScalarKind::Text,
            Self::Color => crate::callable::CallableCompileTimeScalarKind::Color,
        }
    }
}

/// One typed scalar accepted by the shared compile-time value boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCompileTimeScalar {
    Bool(bool),
    Int(i64),
    Milli(Milli),
    Ratio(RatioMilli),
    Length(CheckedLength),
    Angle(CheckedAngle),
    Duration(CheckedDuration),
    Enum(CheckedCompileTimeScalarEnumValue),
    PublicId(PublicId),
    Text(String),
    Color(CheckedColor),
}

/// Exact checked value of one payload-free project enum.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCompileTimeScalarEnumValue {
    declaration: ProjectNominalDeclarationId,
    case: AcceptedVariantCaseSemanticId,
    ordinal: u32,
}

/// Typed declaration-owned default for one schema field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyFieldDefault {
    expression: ExprId,
    value: CheckedCompileTimeScalar,
}

/// Canonical metadata defaults carried by the proxy declaration attribute.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedTextProxyMetadataDefaults {
    role: Option<CheckedTextProxyMetadataDefault<PublicId>>,
    layer: Option<CheckedTextProxyMetadataDefault<PublicId>>,
    depth: Option<CheckedTextProxyMetadataDefault<CheckedObjectDepth>>,
    hit_test: Option<CheckedTextProxyMetadataDefault<bool>>,
}

/// Typed declaration expression supplying one canonical metadata default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyMetadataDefault<T> {
    expression: ExprId,
    value: T,
}

/// Declaration-ordered semantic definition of one proxy field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyFieldDefinition {
    declaration_ordinal: u32,
    semantic_id: AcceptedRecordFieldSemanticId,
    field_type: SemanticTypeDigest,
    diagnostic_name: ModuleSegment,
    kind: CheckedCompileTimeScalarKind,
    registered_type: Option<crate::registration::RegisteredCompileTimeScalarType>,
    optional: bool,
    default: Option<CheckedTextProxyFieldDefault>,
}

/// Exact semantic identity of one accepted text-proxy declaration.
///
/// This wraps the project nominal declaration key so a proxy owner cannot be
/// confused with another declaration that happens to expose the same fields.
/// The declaration key is semantic identity; its display/diagnostic spelling
/// is never reconstructed by proxy consumers.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTextProxyDefinitionId {
    declaration: ProjectNominalDeclarationId,
}

/// Stable semantic digest of one complete checked text-proxy definition.
///
/// The digest is issued by the definition owner. It contains the exact
/// declaration identity, attribute origin/family, nominal type, ordered field
/// semantic rows, reduced defaults, and metadata defaults. Diagnostic field
/// names and other display-only spellings are deliberately excluded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTextProxyDefinitionDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTextProxyDefinitionDigestError {
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
    #[error("text-proxy definition digest sequence length exceeds u64")]
    LengthOverflow,
}

/// Stable semantic identity of one complete checked Object proxy application.
///
/// This digest is issued only after the application has passed the final
/// definition/application relation. Its input is the closed typed carrier:
/// definition and selected-call digests, canonical object identity bytes,
/// effective metadata with provenance, and declaration-ordered scalar fields.
/// The attached rich-text body is intentionally not included; it is joined by
/// the checked content-insertion owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTextProxyApplicationSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum CheckedTextProxyApplicationSemanticDigestError {
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
    #[error("text-proxy application digest sequence length exceeds u64")]
    LengthOverflow,
    #[error(transparent)]
    Coordinate(#[from] SemanticCoordinateEncodingError),
}

/// Generation-bound coordinate of the declaration attribute that admitted a
/// proxy definition.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTextProxyAttributeOrigin {
    declaration: ProjectNominalDeclarationId,
    ordinal: u32,
}

/// Complete checked definition of one marked project struct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyDefinition {
    id: CheckedTextProxyDefinitionId,
    attribute_origin: CheckedTextProxyAttributeOrigin,
    semantic_type: SemanticTypeDigest,
    diagnostic_name: ModuleSegment,
    attribute: CheckedTextProxyAttributeFamily,
    fields: Box<[CheckedTextProxyFieldDefinition]>,
    metadata_defaults: CheckedTextProxyMetadataDefaults,
}

const TEXT_PROXY_DEFINITION_DIGEST_DOMAIN: &[u8] =
    b"arcweft.lang.checked-text-proxy-definition.v1\0";
const TEXT_PROXY_APPLICATION_DIGEST_DOMAIN: &[u8] =
    b"arcweft.lang.checked-text-proxy-application.v1\0";

impl CheckedTextProxyDefinitionId {
    pub(crate) const fn new(declaration: ProjectNominalDeclarationId) -> Self {
        Self { declaration }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    /// Returns the canonical nominal identity used when this declaration is
    /// embedded in another semantic owner.
    #[must_use]
    pub fn semantic_type(&self) -> SemanticTypeDigest {
        TypeKind::ProjectNominal(ProjectNominalType::new(
            self.declaration.clone(),
            Box::<[TypeKind]>::default(),
        ))
        .semantic_identity_digest()
        .expect("text-proxy declaration identity contains no type arguments")
    }
}

impl CheckedTextProxyDefinitionDigest {
    pub(crate) const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl CheckedTextProxyApplicationSemanticDigest {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Default)]
struct CheckedTextProxyDefinitionDigestEncoder {
    bytes: Vec<u8>,
}

impl CheckedTextProxyDefinitionDigestEncoder {
    fn finish(self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(TEXT_PROXY_DEFINITION_DIGEST_DOMAIN);
        hasher.update(&self.bytes);
        *hasher.finalize().as_bytes()
    }

    fn tag(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.u64(
            u64::try_from(value)
                .map_err(|_| CheckedTextProxyDefinitionDigestError::LengthOverflow)?,
        );
        Ok(())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.len(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.bytes(value.as_bytes())
    }

    fn digest(
        &mut self,
        value: SemanticTypeDigest,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.bytes(value.as_bytes())
    }

    fn definition(
        &mut self,
        definition: &CheckedTextProxyDefinition,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.definition_id(definition.id())?;
        self.attribute_origin(definition.attribute_origin())?;
        self.tag(definition.attribute().semantic_tag());
        self.digest(definition.semantic_type())?;
        self.len(definition.fields().len())?;
        for field in definition.fields() {
            self.u32(field.declaration_ordinal());
            self.bytes(field.semantic_id().as_bytes())?;
            self.digest(field.field_type())?;
            self.tag(if field.default().is_some() {
                2
            } else if field.optional() {
                1
            } else {
                0
            });
            self.scalar_kind(field.kind())?;
            self.option(field.default().map(CheckedTextProxyFieldDefault::value))?;
        }
        self.metadata_defaults(definition.metadata_defaults())
    }

    fn definition_id(
        &mut self,
        id: &CheckedTextProxyDefinitionId,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.digest(id.semantic_type())
    }

    fn attribute_origin(
        &mut self,
        origin: &CheckedTextProxyAttributeOrigin,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.digest(
            TypeKind::ProjectNominal(ProjectNominalType::new(
                origin.declaration.clone(),
                Box::<[TypeKind]>::default(),
            ))
            .semantic_identity_digest()?,
        )?;
        self.u32(origin.ordinal());
        Ok(())
    }

    fn scalar_kind(
        &mut self,
        kind: &CheckedCompileTimeScalarKind,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        match kind {
            CheckedCompileTimeScalarKind::Bool => self.tag(0),
            CheckedCompileTimeScalarKind::Int => self.tag(1),
            CheckedCompileTimeScalarKind::Milli => self.tag(2),
            CheckedCompileTimeScalarKind::Ratio => self.tag(3),
            CheckedCompileTimeScalarKind::Length => self.tag(4),
            CheckedCompileTimeScalarKind::Angle => self.tag(5),
            CheckedCompileTimeScalarKind::Duration => self.tag(6),
            CheckedCompileTimeScalarKind::ClosedEnum(schema) => {
                self.tag(7);
                self.digest(schema.semantic_type())?;
                self.len(schema.cases().len())?;
                for case in schema.cases() {
                    self.u32(case.ordinal());
                    self.bytes(case.semantic_id.as_bytes())?;
                }
            }
            CheckedCompileTimeScalarKind::PublicId => self.tag(8),
            CheckedCompileTimeScalarKind::Text => self.tag(9),
            CheckedCompileTimeScalarKind::Color => self.tag(10),
        }
        Ok(())
    }

    fn option(
        &mut self,
        value: Option<&CheckedCompileTimeScalar>,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        match value {
            Some(value) => {
                self.bool(true);
                self.scalar(value)?;
            }
            None => self.bool(false),
        }
        Ok(())
    }

    fn scalar(
        &mut self,
        value: &CheckedCompileTimeScalar,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        match value {
            CheckedCompileTimeScalar::Bool(value) => {
                self.tag(0);
                self.bool(*value);
            }
            CheckedCompileTimeScalar::Int(value) => {
                self.tag(1);
                self.i64(*value);
            }
            CheckedCompileTimeScalar::Milli(value) => {
                self.tag(2);
                self.i32(value.0);
            }
            CheckedCompileTimeScalar::Ratio(value) => {
                self.tag(3);
                self.u16(value.0);
            }
            CheckedCompileTimeScalar::Length(value) => {
                self.tag(4);
                self.i32(value.milli);
                self.tag(match value.unit {
                    crate::checked_rich_text::LengthUnit::Px => 0,
                    crate::checked_rich_text::LengthUnit::Pt => 1,
                    crate::checked_rich_text::LengthUnit::Ch => 2,
                    crate::checked_rich_text::LengthUnit::Em => 3,
                });
            }
            CheckedCompileTimeScalar::Angle(value) => {
                self.tag(5);
                self.i32(value.milli_degrees);
            }
            CheckedCompileTimeScalar::Duration(value) => {
                self.tag(6);
                self.u64(value.millis);
            }
            CheckedCompileTimeScalar::Enum(value) => {
                self.tag(7);
                self.digest(
                    TypeKind::ProjectNominal(ProjectNominalType::new(
                        value.declaration.clone(),
                        Box::<[TypeKind]>::default(),
                    ))
                    .semantic_identity_digest()?,
                )?;
                self.bytes(value.case.as_bytes())?;
                self.u32(value.ordinal);
            }
            CheckedCompileTimeScalar::PublicId(value) => {
                self.tag(8);
                self.string(value.as_str())?;
            }
            CheckedCompileTimeScalar::Text(value) => {
                self.tag(9);
                self.string(value)?;
            }
            CheckedCompileTimeScalar::Color(value) => {
                self.tag(10);
                match value {
                    CheckedColor::Rgba8(rgba) => {
                        self.tag(0);
                        self.bytes(rgba)?;
                    }
                    CheckedColor::Resource(id) => {
                        self.tag(1);
                        self.string(id.as_str())?;
                    }
                }
            }
        }
        Ok(())
    }

    fn metadata_defaults(
        &mut self,
        defaults: &CheckedTextProxyMetadataDefaults,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        self.metadata_public_id(defaults.role_default())?;
        self.metadata_public_id(defaults.layer_default())?;
        match defaults.depth_default() {
            Some(default) => {
                self.bool(true);
                self.i32(default.value().milli());
            }
            None => self.bool(false),
        }
        match defaults.hit_test_default() {
            Some(default) => {
                self.bool(true);
                self.bool(*default.value());
            }
            None => self.bool(false),
        }
        Ok(())
    }

    fn metadata_public_id(
        &mut self,
        value: Option<&CheckedTextProxyMetadataDefault<PublicId>>,
    ) -> Result<(), CheckedTextProxyDefinitionDigestError> {
        match value {
            Some(default) => {
                self.bool(true);
                self.string(default.value().as_str())?;
            }
            None => self.bool(false),
        }
        Ok(())
    }
}

#[derive(Default)]
struct CheckedTextProxyApplicationSemanticDigestEncoder {
    bytes: Vec<u8>,
}

impl CheckedTextProxyApplicationSemanticDigestEncoder {
    fn finish(self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(TEXT_PROXY_APPLICATION_DIGEST_DOMAIN);
        hasher.update(&self.bytes);
        *hasher.finalize().as_bytes()
    }

    fn tag(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.u64(
            u64::try_from(value)
                .map_err(|_| CheckedTextProxyApplicationSemanticDigestError::LengthOverflow)?,
        );
        Ok(())
    }

    fn bytes(
        &mut self,
        value: &[u8],
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.len(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn digest(
        &mut self,
        value: &[u8; 32],
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.bytes(value)
    }

    fn coordinate(
        &mut self,
        coordinate: &StableCheckedValueCoordinate,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.bytes(&coordinate.canonical_bytes()?)
    }

    fn origin(
        &mut self,
        origin: &CheckedTextProxyValueOrigin,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        match origin {
            CheckedTextProxyValueOrigin::AttributeDefault(coordinate) => {
                self.tag(0);
                self.coordinate(coordinate)?;
            }
            CheckedTextProxyValueOrigin::Inline {
                application,
                coordinate,
            } => {
                self.tag(1);
                self.digest(application.as_bytes())?;
                self.coordinate(coordinate)?;
            }
            CheckedTextProxyValueOrigin::CanonicalDefault => self.tag(2),
            CheckedTextProxyValueOrigin::Absent => self.tag(3),
        }
        Ok(())
    }

    fn public_id(
        &mut self,
        value: &PublicId,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.bytes(value.canonical_identity_bytes())
    }

    fn scalar(
        &mut self,
        value: &CheckedCompileTimeScalar,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        match value {
            CheckedCompileTimeScalar::Bool(value) => {
                self.tag(0);
                self.bool(*value);
            }
            CheckedCompileTimeScalar::Int(value) => {
                self.tag(1);
                self.i64(*value);
            }
            CheckedCompileTimeScalar::Milli(value) => {
                self.tag(2);
                self.i32(value.0);
            }
            CheckedCompileTimeScalar::Ratio(value) => {
                self.tag(3);
                self.u16(value.0);
            }
            CheckedCompileTimeScalar::Length(value) => {
                self.tag(4);
                self.i32(value.milli);
                self.tag(match value.unit {
                    crate::checked_rich_text::LengthUnit::Px => 0,
                    crate::checked_rich_text::LengthUnit::Pt => 1,
                    crate::checked_rich_text::LengthUnit::Ch => 2,
                    crate::checked_rich_text::LengthUnit::Em => 3,
                });
            }
            CheckedCompileTimeScalar::Angle(value) => {
                self.tag(5);
                self.i32(value.milli_degrees);
            }
            CheckedCompileTimeScalar::Duration(value) => {
                self.tag(6);
                self.u64(value.millis);
            }
            CheckedCompileTimeScalar::Enum(value) => {
                self.tag(7);
                self.digest(
                    TypeKind::ProjectNominal(ProjectNominalType::new(
                        value.declaration.clone(),
                        Box::<[TypeKind]>::default(),
                    ))
                    .semantic_identity_digest()?
                    .as_bytes(),
                )?;
                self.bytes(value.case.as_bytes())?;
                self.u32(value.ordinal);
            }
            CheckedCompileTimeScalar::PublicId(value) => {
                self.tag(8);
                self.public_id(value)?;
            }
            CheckedCompileTimeScalar::Text(value) => {
                self.tag(9);
                self.bytes(value.as_bytes())?;
            }
            CheckedCompileTimeScalar::Color(value) => {
                self.tag(10);
                match value {
                    CheckedColor::Rgba8(rgba) => {
                        self.tag(0);
                        self.bytes(rgba)?;
                    }
                    CheckedColor::Resource(id) => {
                        self.tag(1);
                        self.public_id(id)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn optional_scalar(
        &mut self,
        value: Option<&CheckedCompileTimeScalar>,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        match value {
            Some(value) => {
                self.bool(true);
                self.scalar(value)?;
            }
            None => self.bool(false),
        }
        Ok(())
    }

    fn public_id_value(
        &mut self,
        value: &CheckedTextProxyApplicationValue<PublicId>,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.public_id(value.value())?;
        self.origin(value.origin())
    }

    fn depth_value(
        &mut self,
        value: &CheckedTextProxyApplicationValue<CheckedObjectDepth>,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.i32(value.value().milli());
        self.origin(value.origin())
    }

    fn bool_value(
        &mut self,
        value: &CheckedTextProxyApplicationValue<bool>,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.bool(*value.value());
        self.origin(value.origin())
    }

    fn optional_public_id_value(
        &mut self,
        value: Option<&CheckedTextProxyApplicationValue<PublicId>>,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        match value {
            Some(value) => {
                self.bool(true);
                self.public_id_value(value)?;
            }
            None => self.bool(false),
        }
        Ok(())
    }

    fn optional_depth_value(
        &mut self,
        value: Option<&CheckedTextProxyApplicationValue<CheckedObjectDepth>>,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        match value {
            Some(value) => {
                self.bool(true);
                self.depth_value(value)?;
            }
            None => self.bool(false),
        }
        Ok(())
    }

    fn metadata(
        &mut self,
        metadata: &CheckedTextProxyApplicationMetadata,
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.optional_public_id_value(metadata.role())?;
        self.optional_public_id_value(metadata.layer())?;
        self.optional_depth_value(metadata.depth())?;
        self.bool_value(metadata.hit_test())
    }

    fn application(
        &mut self,
        definition_digest: CheckedTextProxyDefinitionDigest,
        call_application: CheckedCallApplicationDigest,
        id: &CheckedTextProxyApplicationValue<PublicId>,
        metadata: &CheckedTextProxyApplicationMetadata,
        fields: &[CheckedTextProxyApplicationField],
    ) -> Result<(), CheckedTextProxyApplicationSemanticDigestError> {
        self.digest(&definition_digest.into_bytes())?;
        self.digest(call_application.as_bytes())?;
        self.public_id(id.value())?;
        self.metadata(metadata)?;
        self.len(fields.len())?;
        for field in fields {
            self.u32(field.declaration_ordinal);
            self.bytes(field.semantic_id.as_bytes())?;
            self.optional_scalar(field.value.as_ref())?;
            self.origin(&field.origin)?;
        }
        Ok(())
    }
}

fn checked_text_proxy_application_semantic_digest(
    definition_digest: CheckedTextProxyDefinitionDigest,
    call_application: CheckedCallApplicationDigest,
    id: &CheckedTextProxyApplicationValue<PublicId>,
    metadata: &CheckedTextProxyApplicationMetadata,
    fields: &[CheckedTextProxyApplicationField],
) -> Result<CheckedTextProxyApplicationSemanticDigest, CheckedTextProxyApplicationSemanticDigestError>
{
    let mut encoder = CheckedTextProxyApplicationSemanticDigestEncoder::default();
    encoder.application(definition_digest, call_application, id, metadata, fields)?;
    Ok(CheckedTextProxyApplicationSemanticDigest::from_bytes(
        encoder.finish(),
    ))
}

/// Stable source provenance of one effective final proxy value.
///
/// Inline values retain the checked call application digest together with a
/// stable semantic coordinate. Attribute defaults retain only their checked
/// coordinate, while absence carries no source coordinate. HIR tag IDs are
/// deliberately not representable here.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTextProxyValueOrigin {
    AttributeDefault(StableCheckedValueCoordinate),
    Inline {
        application: CheckedCallApplicationDigest,
        coordinate: StableCheckedValueCoordinate,
    },
    CanonicalDefault,
    Absent,
}

impl CheckedTextProxyValueOrigin {
    pub const fn application(&self) -> Option<CheckedCallApplicationDigest> {
        match self {
            Self::Inline { application, .. } => Some(*application),
            Self::AttributeDefault(_) | Self::CanonicalDefault | Self::Absent => None,
        }
    }

    pub const fn coordinate(&self) -> Option<&StableCheckedValueCoordinate> {
        match self {
            Self::AttributeDefault(coordinate) | Self::Inline { coordinate, .. } => {
                Some(coordinate)
            }
            Self::CanonicalDefault | Self::Absent => None,
        }
    }
}

/// One effective final proxy value paired with its stable source provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyApplicationValue<T> {
    value: T,
    origin: CheckedTextProxyValueOrigin,
}

impl<T> CheckedTextProxyApplicationValue<T> {
    pub(crate) const fn new(value: T, origin: CheckedTextProxyValueOrigin) -> Self {
        Self { value, origin }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn origin(&self) -> &CheckedTextProxyValueOrigin {
        &self.origin
    }
}

/// Effective metadata values for one final Object proxy application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyApplicationMetadata {
    role: Option<CheckedTextProxyApplicationValue<PublicId>>,
    layer: Option<CheckedTextProxyApplicationValue<PublicId>>,
    depth: Option<CheckedTextProxyApplicationValue<CheckedObjectDepth>>,
    hit_test: CheckedTextProxyApplicationValue<bool>,
}

impl CheckedTextProxyApplicationMetadata {
    pub(crate) fn new(
        role: Option<CheckedTextProxyApplicationValue<PublicId>>,
        layer: Option<CheckedTextProxyApplicationValue<PublicId>>,
        depth: Option<CheckedTextProxyApplicationValue<CheckedObjectDepth>>,
        hit_test: CheckedTextProxyApplicationValue<bool>,
    ) -> Self {
        Self {
            role,
            layer,
            depth,
            hit_test,
        }
    }

    pub const fn role(&self) -> Option<&CheckedTextProxyApplicationValue<PublicId>> {
        self.role.as_ref()
    }

    pub const fn layer(&self) -> Option<&CheckedTextProxyApplicationValue<PublicId>> {
        self.layer.as_ref()
    }

    pub const fn depth(&self) -> Option<&CheckedTextProxyApplicationValue<CheckedObjectDepth>> {
        self.depth.as_ref()
    }

    pub const fn hit_test(&self) -> &CheckedTextProxyApplicationValue<bool> {
        &self.hit_test
    }
}

/// One declaration-ordered effective final application field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyApplicationField {
    declaration_ordinal: u32,
    semantic_id: AcceptedRecordFieldSemanticId,
    value: Option<CheckedCompileTimeScalar>,
    origin: CheckedTextProxyValueOrigin,
}

/// Exact checked proxy application attached to one final checked call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTextProxyApplication {
    call_application: CheckedCallApplicationDigest,
    definition_id: CheckedTextProxyDefinitionId,
    definition_digest: CheckedTextProxyDefinitionDigest,
    semantic_digest: CheckedTextProxyApplicationSemanticDigest,
    id: CheckedTextProxyApplicationValue<PublicId>,
    metadata: CheckedTextProxyApplicationMetadata,
    fields: Box<[CheckedTextProxyApplicationField]>,
}

/// Immutable proxy definitions accepted with one final semantic generation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedTextProxyCatalog {
    definitions: BTreeMap<CheckedTextProxyDefinitionId, CheckedTextProxyDefinition>,
}

/// Read-only exact join between an application and its catalog definition.
#[derive(Clone, Copy, Debug)]
pub struct CheckedTextProxyApplicationView<'a> {
    definition: &'a CheckedTextProxyDefinition,
    application: &'a CheckedTextProxyApplication,
}

impl CheckedTextProxyCatalog {
    pub(crate) fn new(
        definitions: BTreeMap<CheckedTextProxyDefinitionId, CheckedTextProxyDefinition>,
    ) -> Self {
        Self { definitions }
    }

    /// Borrows one exact definition by its owner-issued identity.
    #[must_use]
    pub fn get(&self, id: &CheckedTextProxyDefinitionId) -> Option<&CheckedTextProxyDefinition> {
        self.definitions.get(id)
    }

    /// Iterates accepted definitions in semantic declaration order.
    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &CheckedTextProxyDefinition> {
        self.definitions.values()
    }

    /// Iterates the owner-issued definition identities in semantic order.
    pub fn definition_ids(&self) -> impl ExactSizeIterator<Item = &CheckedTextProxyDefinitionId> {
        self.definitions.keys()
    }

    /// Joins an application to its sole definition and validates its complete
    /// declaration-ordered field relation. Consumers must not reproduce this
    /// check from runtime names.
    pub fn application<'a>(
        &'a self,
        application: &'a CheckedTextProxyApplication,
    ) -> Option<CheckedTextProxyApplicationView<'a>> {
        let definition = self.get(application.definition_id())?;
        if !application_matches_definition(definition, application) {
            return None;
        }
        Some(CheckedTextProxyApplicationView {
            definition,
            application,
        })
    }
}

fn application_matches_definition(
    definition: &CheckedTextProxyDefinition,
    application: &CheckedTextProxyApplication,
) -> bool {
    definition.id() == application.definition_id()
        && definition.digest().ok() == Some(application.definition_digest())
        && application_id_matches(application)
        && metadata_matches_definition(definition, application)
        && definition.fields.len() == application.fields.len()
        && definition
            .fields
            .iter()
            .zip(application.fields.iter())
            .all(|(field, applied)| {
                field.declaration_ordinal == applied.declaration_ordinal
                    && field.semantic_id == applied.semantic_id
                    && (applied.value.is_some() || (field.optional && field.default.is_none()))
                    && scalar_matches_kind(applied.value.as_ref(), &field.kind)
                    && application_origin_matches(field, application, applied)
            })
}

fn application_id_matches(application: &CheckedTextProxyApplication) -> bool {
    matches!(
        application.id().origin(),
        CheckedTextProxyValueOrigin::Inline { application: digest, .. }
            if *digest == application.call_application
    )
}

fn metadata_matches_definition(
    definition: &CheckedTextProxyDefinition,
    application: &CheckedTextProxyApplication,
) -> bool {
    let metadata = application.metadata();
    let role = match (
        definition.metadata_defaults().role_default(),
        metadata.role(),
    ) {
        (Some(default), Some(value)) => {
            metadata_value_matches(value, Some(default), application.call_application, None)
        }
        (Some(_), None) => false,
        (None, Some(value)) => {
            metadata_value_matches(value, None, application.call_application, None)
        }
        (None, None) => true,
    };
    let layer = match (
        definition.metadata_defaults().layer_default(),
        metadata.layer(),
    ) {
        (Some(default), Some(value)) => {
            metadata_value_matches(value, Some(default), application.call_application, None)
        }
        (Some(_), None) => false,
        (None, Some(value)) => {
            metadata_value_matches(value, None, application.call_application, None)
        }
        (None, None) => true,
    };
    let depth = match (
        definition.metadata_defaults().depth_default(),
        metadata.depth(),
    ) {
        (Some(default), Some(value)) => {
            metadata_value_matches(value, Some(default), application.call_application, None)
        }
        (Some(_), None) => false,
        (None, Some(value)) => {
            metadata_value_matches(value, None, application.call_application, None)
        }
        (None, None) => true,
    };
    let hit_test = metadata_value_matches(
        metadata.hit_test(),
        definition.metadata_defaults().hit_test_default(),
        application.call_application,
        Some(&false),
    );
    role && layer && depth && hit_test
}

fn metadata_value_matches<T: Eq>(
    value: &CheckedTextProxyApplicationValue<T>,
    default: Option<&CheckedTextProxyMetadataDefault<T>>,
    call_application: CheckedCallApplicationDigest,
    canonical: Option<&T>,
) -> bool {
    match value.origin() {
        CheckedTextProxyValueOrigin::AttributeDefault(_) => {
            default.is_some_and(|default| value.value() == default.value())
        }
        CheckedTextProxyValueOrigin::Inline { application, .. } => *application == call_application,
        CheckedTextProxyValueOrigin::CanonicalDefault => {
            default.is_none() && canonical.is_some_and(|canonical| value.value() == canonical)
        }
        CheckedTextProxyValueOrigin::Absent => false,
    }
}

fn application_origin_matches(
    field: &CheckedTextProxyFieldDefinition,
    application: &CheckedTextProxyApplication,
    applied: &CheckedTextProxyApplicationField,
) -> bool {
    match &applied.origin {
        CheckedTextProxyValueOrigin::AttributeDefault(_) => field
            .default
            .as_ref()
            .is_some_and(|default| applied.value.as_ref() == Some(&default.value)),
        CheckedTextProxyValueOrigin::Inline {
            application: digest,
            ..
        } => *digest == application.call_application && applied.value.is_some(),
        CheckedTextProxyValueOrigin::CanonicalDefault => false,
        CheckedTextProxyValueOrigin::Absent => {
            field.optional && field.default.is_none() && applied.value.is_none()
        }
    }
}

impl CheckedTextProxyDefinition {
    pub(crate) fn new(
        id: CheckedTextProxyDefinitionId,
        attribute_origin: CheckedTextProxyAttributeOrigin,
        semantic_type: SemanticTypeDigest,
        diagnostic_name: ModuleSegment,
        attribute: CheckedTextProxyAttributeFamily,
        fields: Box<[CheckedTextProxyFieldDefinition]>,
        metadata_defaults: CheckedTextProxyMetadataDefaults,
    ) -> Self {
        Self {
            id,
            attribute_origin,
            semantic_type,
            diagnostic_name,
            attribute,
            fields,
            metadata_defaults,
        }
    }

    /// Returns the exact declaration identity owned by this definition.
    pub const fn id(&self) -> &CheckedTextProxyDefinitionId {
        &self.id
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        self.id.declaration()
    }

    pub const fn attribute_origin(&self) -> &CheckedTextProxyAttributeOrigin {
        &self.attribute_origin
    }

    pub const fn semantic_type(&self) -> SemanticTypeDigest {
        self.semantic_type
    }

    pub fn diagnostic_name(&self) -> &str {
        self.diagnostic_name.as_str()
    }

    pub const fn attribute(&self) -> CheckedTextProxyAttributeFamily {
        self.attribute
    }

    pub const fn fields(&self) -> &[CheckedTextProxyFieldDefinition] {
        &self.fields
    }

    pub(crate) fn fields_mut(&mut self) -> &mut [CheckedTextProxyFieldDefinition] {
        &mut self.fields
    }

    pub const fn metadata_defaults(&self) -> &CheckedTextProxyMetadataDefaults {
        &self.metadata_defaults
    }

    /// Returns the owner-defined semantic digest of this complete
    /// definition. The digest is computed from typed semantic rows and never
    /// from diagnostic/display names.
    #[must_use]
    pub fn digest(
        &self,
    ) -> Result<CheckedTextProxyDefinitionDigest, CheckedTextProxyDefinitionDigestError> {
        let mut encoder = CheckedTextProxyDefinitionDigestEncoder::default();
        encoder.definition(self)?;
        Ok(CheckedTextProxyDefinitionDigest(encoder.finish()))
    }

    /// Alias used by semantic consumers that name one-way identity methods
    /// `semantic_digest`.
    #[must_use]
    pub fn semantic_digest(
        &self,
    ) -> Result<CheckedTextProxyDefinitionDigest, CheckedTextProxyDefinitionDigestError> {
        self.digest()
    }

    pub(crate) const fn metadata_defaults_mut(&mut self) -> &mut CheckedTextProxyMetadataDefaults {
        &mut self.metadata_defaults
    }

    pub(crate) fn visit_project_nominal_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&TypeKind::ProjectNominal(
            crate::types::ProjectNominalType::new(
                self.declaration().clone(),
                Box::<[TypeKind]>::default(),
            ),
        ))?;
        for field in &self.fields {
            if let CheckedCompileTimeScalarKind::ClosedEnum(schema) = &field.kind {
                visitor(&TypeKind::ProjectNominal(
                    crate::types::ProjectNominalType::new(
                        schema.declaration.clone(),
                        Box::<[TypeKind]>::default(),
                    ),
                ))?;
            }
        }
        Ok(())
    }
}

impl CheckedTextProxyAttributeOrigin {
    pub(crate) const fn new(declaration: ProjectNominalDeclarationId, ordinal: u32) -> Self {
        Self {
            declaration,
            ordinal,
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl CheckedTextProxyFieldDefinition {
    pub(crate) fn new(
        declaration_ordinal: u32,
        semantic_id: AcceptedRecordFieldSemanticId,
        field_type: SemanticTypeDigest,
        diagnostic_name: ModuleSegment,
        kind: CheckedCompileTimeScalarKind,
        registered_type: Option<crate::registration::RegisteredCompileTimeScalarType>,
        optional: bool,
        default: Option<CheckedTextProxyFieldDefault>,
    ) -> Self {
        Self {
            declaration_ordinal,
            semantic_id,
            field_type,
            diagnostic_name,
            kind,
            registered_type,
            optional,
            default,
        }
    }

    pub const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub const fn semantic_id(&self) -> AcceptedRecordFieldSemanticId {
        self.semantic_id
    }

    pub const fn field_type(&self) -> SemanticTypeDigest {
        self.field_type
    }

    pub fn diagnostic_name(&self) -> &str {
        self.diagnostic_name.as_str()
    }

    pub const fn kind(&self) -> &CheckedCompileTimeScalarKind {
        &self.kind
    }

    pub const fn registered_type(
        &self,
    ) -> Option<&crate::registration::RegisteredCompileTimeScalarType> {
        self.registered_type.as_ref()
    }

    pub const fn optional(&self) -> bool {
        self.optional
    }

    pub const fn default(&self) -> Option<&CheckedTextProxyFieldDefault> {
        self.default.as_ref()
    }

    pub(crate) fn set_default(&mut self, value: CheckedTextProxyFieldDefault) -> bool {
        if self.default.is_some() {
            return false;
        }
        self.default = Some(value);
        true
    }
}

impl CheckedTextProxyFieldDefault {
    pub(crate) const fn new(expression: ExprId, value: CheckedCompileTimeScalar) -> Self {
        Self { expression, value }
    }

    pub const fn expression(&self) -> ExprId {
        self.expression
    }

    pub const fn value(&self) -> &CheckedCompileTimeScalar {
        &self.value
    }
}

impl CheckedTextProxyMetadataDefaults {
    pub const fn role(&self) -> Option<&PublicId> {
        match self.role.as_ref() {
            Some(value) => Some(&value.value),
            None => None,
        }
    }

    pub const fn role_default(&self) -> Option<&CheckedTextProxyMetadataDefault<PublicId>> {
        self.role.as_ref()
    }

    pub const fn layer(&self) -> Option<&PublicId> {
        match self.layer.as_ref() {
            Some(value) => Some(&value.value),
            None => None,
        }
    }

    pub const fn layer_default(&self) -> Option<&CheckedTextProxyMetadataDefault<PublicId>> {
        self.layer.as_ref()
    }

    pub const fn depth(&self) -> Option<CheckedObjectDepth> {
        match &self.depth {
            Some(value) => Some(value.value),
            None => None,
        }
    }

    pub const fn depth_default(
        &self,
    ) -> Option<&CheckedTextProxyMetadataDefault<CheckedObjectDepth>> {
        self.depth.as_ref()
    }

    pub const fn hit_test(&self) -> bool {
        match &self.hit_test {
            Some(value) => value.value,
            None => false,
        }
    }

    pub const fn hit_test_default(&self) -> Option<&CheckedTextProxyMetadataDefault<bool>> {
        self.hit_test.as_ref()
    }

    pub(crate) fn set_role(&mut self, expression: ExprId, value: PublicId) {
        self.role = Some(CheckedTextProxyMetadataDefault { expression, value });
    }

    pub(crate) fn set_layer(&mut self, expression: ExprId, value: PublicId) {
        self.layer = Some(CheckedTextProxyMetadataDefault { expression, value });
    }

    pub(crate) fn set_depth(&mut self, expression: ExprId, value: CheckedObjectDepth) {
        self.depth = Some(CheckedTextProxyMetadataDefault { expression, value });
    }

    pub(crate) fn set_hit_test(&mut self, expression: ExprId, value: bool) {
        self.hit_test = Some(CheckedTextProxyMetadataDefault { expression, value });
    }
}

impl<T> CheckedTextProxyMetadataDefault<T> {
    pub const fn expression(&self) -> ExprId {
        self.expression
    }

    pub const fn value(&self) -> &T {
        &self.value
    }
}

impl CheckedCompileTimeScalarEnum {
    pub(crate) fn new(
        declaration: ProjectNominalDeclarationId,
        semantic_type: SemanticTypeDigest,
        cases: Box<[CheckedCompileTimeScalarEnumCase]>,
    ) -> Self {
        Self {
            declaration,
            semantic_type,
            cases,
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub const fn semantic_type(&self) -> SemanticTypeDigest {
        self.semantic_type
    }

    pub const fn cases(&self) -> &[CheckedCompileTimeScalarEnumCase] {
        &self.cases
    }
}

impl CheckedCompileTimeScalarEnumCase {
    pub(crate) fn new(
        ordinal: u32,
        semantic_id: AcceptedVariantCaseSemanticId,
        diagnostic_name: ModuleSegment,
    ) -> Self {
        Self {
            ordinal,
            semantic_id,
            diagnostic_name,
        }
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn diagnostic_name(&self) -> &str {
        self.diagnostic_name.as_str()
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedVariantCaseSemanticId {
        self.semantic_id
    }
}

impl CheckedCompileTimeScalarEnumValue {
    pub(crate) const fn new(
        declaration: ProjectNominalDeclarationId,
        case: AcceptedVariantCaseSemanticId,
        ordinal: u32,
    ) -> Self {
        Self {
            declaration,
            case,
            ordinal,
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedVariantCaseSemanticId {
        self.case
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl CheckedTextProxyApplication {
    /// Seals one prepared Object proxy application against its exact checked
    /// definition and checked call application. No source spelling or HIR tag
    /// identity can enter the final carrier.
    pub(crate) fn seal(
        definition: &CheckedTextProxyDefinition,
        call_application: CheckedCallApplicationDigest,
        id: CheckedTextProxyApplicationValue<PublicId>,
        metadata: CheckedTextProxyApplicationMetadata,
        fields: impl Into<Box<[CheckedTextProxyApplicationField]>>,
    ) -> Option<Self> {
        let fields = fields.into();
        let definition_digest = definition.digest().ok()?;
        let semantic_digest = checked_text_proxy_application_semantic_digest(
            definition_digest,
            call_application,
            &id,
            &metadata,
            &fields,
        )
        .ok()?;
        let application = Self {
            call_application,
            definition_id: definition.id().clone(),
            definition_digest,
            semantic_digest,
            id,
            metadata,
            fields,
        };
        application_matches_definition(definition, &application).then_some(application)
    }

    pub const fn call_application(&self) -> CheckedCallApplicationDigest {
        self.call_application
    }

    pub const fn call_application_digest(&self) -> CheckedCallApplicationDigest {
        self.call_application
    }

    pub const fn definition_id(&self) -> &CheckedTextProxyDefinitionId {
        &self.definition_id
    }

    pub const fn definition_digest(&self) -> CheckedTextProxyDefinitionDigest {
        self.definition_digest
    }

    /// Returns the owner-issued semantic identity of this complete checked
    /// Object application. The attached RichText body is joined separately by
    /// the checked content-insertion authority.
    pub const fn semantic_digest(&self) -> CheckedTextProxyApplicationSemanticDigest {
        self.semantic_digest
    }

    pub const fn id(&self) -> &CheckedTextProxyApplicationValue<PublicId> {
        &self.id
    }

    pub const fn metadata(&self) -> &CheckedTextProxyApplicationMetadata {
        &self.metadata
    }

    pub const fn fields(&self) -> &[CheckedTextProxyApplicationField] {
        &self.fields
    }
}

impl CheckedTextProxyApplicationField {
    pub(crate) const fn new(
        declaration_ordinal: u32,
        semantic_id: AcceptedRecordFieldSemanticId,
        value: Option<CheckedCompileTimeScalar>,
        origin: CheckedTextProxyValueOrigin,
    ) -> Self {
        Self {
            declaration_ordinal,
            semantic_id,
            value,
            origin,
        }
    }

    pub const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub const fn semantic_id(&self) -> AcceptedRecordFieldSemanticId {
        self.semantic_id
    }

    pub const fn value(&self) -> Option<&CheckedCompileTimeScalar> {
        self.value.as_ref()
    }

    pub const fn origin(&self) -> &CheckedTextProxyValueOrigin {
        &self.origin
    }
}

impl<'a> CheckedTextProxyApplicationView<'a> {
    pub const fn definition(self) -> &'a CheckedTextProxyDefinition {
        self.definition
    }

    pub const fn application(self) -> &'a CheckedTextProxyApplication {
        self.application
    }
}

fn scalar_matches_kind(
    value: Option<&CheckedCompileTimeScalar>,
    kind: &CheckedCompileTimeScalarKind,
) -> bool {
    let Some(value) = value else {
        return true;
    };
    match (value, kind) {
        (CheckedCompileTimeScalar::Bool(_), CheckedCompileTimeScalarKind::Bool)
        | (CheckedCompileTimeScalar::Int(_), CheckedCompileTimeScalarKind::Int)
        | (CheckedCompileTimeScalar::Milli(_), CheckedCompileTimeScalarKind::Milli)
        | (CheckedCompileTimeScalar::Ratio(_), CheckedCompileTimeScalarKind::Ratio)
        | (CheckedCompileTimeScalar::Length(_), CheckedCompileTimeScalarKind::Length)
        | (CheckedCompileTimeScalar::Angle(_), CheckedCompileTimeScalarKind::Angle)
        | (CheckedCompileTimeScalar::Duration(_), CheckedCompileTimeScalarKind::Duration)
        | (CheckedCompileTimeScalar::PublicId(_), CheckedCompileTimeScalarKind::PublicId)
        | (CheckedCompileTimeScalar::Text(_), CheckedCompileTimeScalarKind::Text)
        | (CheckedCompileTimeScalar::Color(_), CheckedCompileTimeScalarKind::Color) => true,
        (
            CheckedCompileTimeScalar::Enum(value),
            CheckedCompileTimeScalarKind::ClosedEnum(schema),
        ) => {
            &value.declaration == schema.declaration()
                && schema.cases.iter().any(|candidate| {
                    candidate.ordinal == value.ordinal && candidate.semantic_id == value.case
                })
        }
        _ => false,
    }
}
