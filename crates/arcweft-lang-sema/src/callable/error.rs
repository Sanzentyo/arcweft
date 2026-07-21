//! Typed callable construction and query failures.

use std::sync::Arc;

use arcweft_lang_hir::symbol::{CallableDeclarationId, ProjectSymbolTargetId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use thiserror::Error;

use super::{
    CallableAuthorityRank, CallableCandidateId, CallableFamily, CallableGroupIndex,
    CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameterIndex,
    CallableProviderId, DataLastCallableId, FloatWidth, ProjectCallablePath, ProjectNameBinding,
    StdFloatOperation, TraitCallableId,
};

#[derive(Clone, Debug, Eq, Error, Hash, PartialEq)]
pub enum CallableScalarError {
    #[error("{kind:?} cannot be empty")]
    Empty { kind: CallableScalarKind },
    #[error("{kind:?} contains a control character at byte {byte}")]
    Control {
        kind: CallableScalarKind,
        byte: usize,
    },
    #[error("{kind:?} contains separator `{separator}` at byte {byte}")]
    ContainsSeparator {
        kind: CallableScalarKind,
        byte: usize,
        separator: char,
    },
    #[error("{kind:?} index {value} does not fit its backing type")]
    IndexOverflow {
        kind: CallableIndexKind,
        value: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableScalarKind {
    CallableName,
    AdapterPackageId,
    RustItemPath,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableIndexKind {
    Group,
    Parameter,
    Overload,
    Argument,
    ArgumentSlot,
    LexicalBinding,
    FunctionValue,
}

#[derive(Clone, Debug, Eq, Error, Hash, PartialEq)]
pub enum CallablePathError {
    #[error("callable path cannot be empty")]
    Empty,
    #[error("callable path has {actual} segments; maximum is {limit}")]
    TooManySegments { actual: usize, limit: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BuiltinIdentityError {
    #[error("unsupported {width:?} conversion operation {operation:?}")]
    UnsupportedConversion {
        width: FloatWidth,
        operation: StdFloatOperation,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableIdentityError {
    #[error(transparent)]
    Scalar(#[from] CallableScalarError),
    #[error("callable {base:?} cannot use group {group:?} as a curried next group")]
    InvalidCurriedGroup {
        base: Box<CallableCandidateId>,
        group: CallableGroupIndex,
    },
    #[error("callable {base:?} cannot be curried")]
    InvalidCurriedBase { base: Box<CallableCandidateId> },
    #[error("callable {base:?} cannot be used as data-last")]
    InvalidDataLastBase { base: Box<CallableCandidateId> },
    #[error("invalid data-last receiver coordinate {group:?}/{parameter:?}")]
    InvalidDataLastCoordinate {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("data-last receiver coordinate {group:?}/{parameter:?} is a rest parameter")]
    DataLastReceiverIsRest {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("data-last receiver coordinate {group:?}/{parameter:?} is not final")]
    DataLastReceiverNotFinal {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustProvenanceField {
    PackageName,
    PackageVersion,
    MetadataHash,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RustProvenanceError {
    #[error("Rust provenance field {field:?} cannot be empty")]
    Empty { field: RustProvenanceField },
    #[error("Rust provenance field {field:?} contains a control at byte {byte}")]
    Control {
        field: RustProvenanceField,
        byte: usize,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableSchemaError {
    #[error("callable schema must contain an initial parameter group")]
    EmptyGroups,
    #[error("callable has {actual} groups; maximum is {limit}")]
    GroupLimit { actual: usize, limit: usize },
    #[error("callable has {actual} parameters; maximum is {limit}")]
    ParameterLimit { actual: usize, limit: usize },
    #[error("non-contiguous group: expected {expected:?}, found {actual:?}")]
    NonContiguousGroup {
        expected: CallableGroupIndex,
        actual: CallableGroupIndex,
    },
    #[error("invalid parameter group kind at {group:?}")]
    InvalidGroupKind { group: CallableGroupIndex },
    #[error("non-contiguous parameter in {group:?}: expected {expected:?}, found {actual:?}")]
    NonContiguousParameter {
        group: CallableGroupIndex,
        expected: CallableParameterIndex,
        actual: CallableParameterIndex,
    },
    #[error("duplicate parameter name {name:?} in {group:?}")]
    DuplicateParameterName {
        group: CallableGroupIndex,
        name: CallableName,
    },
    #[error("parameter {parameter:?} in {group:?} requires a name")]
    MissingParameterName {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("invalid rest parameter {group:?}/{parameter:?}")]
    InvalidRestParameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("rest parameter {group:?}/{parameter:?} cannot be defaulted")]
    InvalidDefaultedRest {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("parameter source coordinate does not match {group:?}/{parameter:?}")]
    SourceCoordinateMismatch {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("{family:?} schema violates {code:?}")]
    FamilyInvariant {
        family: CallableFamily,
        code: CallableFamilyInvariantCode,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableFamilyInvariantCode {
    InvalidArity,
    InvalidParameterType,
    InvalidResultType,
    InvalidArgumentPolicy,
    InvalidValidator,
    InvalidOwner,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableCatalogError {
    #[error("callable ID and lookup key do not agree")]
    IdKeyMismatch,
    #[error("callable authority and provider do not agree")]
    AuthorityProviderMismatch,
    #[error("project callable record has no source declaration")]
    MissingProjectSource,
    #[error("project callable record cannot carry Rust provenance")]
    UnexpectedProjectRustProvenance,
    #[error("Rust callable record requires Rust provenance")]
    MissingRustProvenance,
    #[error("callable set cannot be empty")]
    EmptyCandidateSet,
    #[error("candidate set contains mismatched lookup keys")]
    CandidateSetKeyMismatch,
    #[error("candidate set has {actual} overloads; maximum is {limit}")]
    OverloadLimit { actual: usize, limit: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallablePublicationError {
    #[error("publication owner does not agree with a record")]
    OwnerMismatch,
    #[error("publication overload indices are invalid")]
    InvalidOverload,
    #[error(transparent)]
    InvalidRecord(#[from] CallableCatalogError),
    #[error(transparent)]
    InvalidSchema(#[from] CallableSchemaError),
    #[error(transparent)]
    InvalidRustProvenance(#[from] RustProvenanceError),
    #[error(transparent)]
    Limit(#[from] CallableBuildLimitError),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableDocumentationError {
    #[error("duplicate documentation for parameter {group:?}/{parameter:?}")]
    DuplicateParameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("parameter documentation cannot be empty")]
    EmptyText,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableSourceError {
    #[error("duplicate source for parameter {group:?}/{parameter:?}")]
    DuplicateParameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    #[error("callable source spans belong to different documents")]
    SourceIdentityMismatch,
    #[error("callable child span lies outside the signature")]
    SpanOutsideSignature,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableBuildLimitError {
    #[error("path has {actual} segments; maximum is {limit}")]
    PathSegments { actual: usize, limit: usize },
    #[error("callable has {actual} groups; maximum is {limit}")]
    Groups { actual: usize, limit: usize },
    #[error("callable has {actual} parameters; maximum is {limit}")]
    Parameters { actual: usize, limit: usize },
    #[error("callable key has {actual} overloads; maximum is {limit}")]
    Overloads { actual: usize, limit: usize },
    #[error("project has {actual} modules; maximum is {limit}")]
    Modules { actual: usize, limit: usize },
    #[error("catalog has {actual} records; maximum is {limit}")]
    Records { actual: usize, limit: usize },
    #[error("catalog work charge {requested} exceeds remaining budget {consumed}/{limit}")]
    Work {
        requested: u64,
        consumed: u64,
        limit: u64,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableQueryLimitError {
    #[error("call produced {actual} candidates; maximum is {limit}")]
    Candidates { actual: usize, limit: usize },
    #[error("signature produced {actual} parameters; maximum is {limit}")]
    Parameters { actual: usize, limit: usize },
    #[error("call depth {actual} exceeds maximum {limit}")]
    NestedCalls { actual: usize, limit: usize },
    #[error("query produced {actual} recovery nodes; maximum is {limit}")]
    RecoveryNodes { actual: usize, limit: usize },
    #[error("query produced {actual} diagnostics; maximum is {limit}")]
    Diagnostics { actual: usize, limit: usize },
    #[error("source has {actual} bytes; maximum is {limit}")]
    SourceBytes { actual: usize, limit: usize },
    #[error("query work charge {requested} exceeds remaining budget {consumed}/{limit}")]
    Work {
        requested: u64,
        consumed: u64,
        limit: u64,
    },
    #[error("query work arithmetic overflow")]
    ArithmeticOverflow,
}

/// Semantic operation whose public signature-query counter overflowed or exceeded a bound.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignatureWorkKind {
    SourceBytes,
    NodeVisits,
    CandidateCalls,
    NestedCalls,
    Arguments,
    RecoveryNodes,
    Resolver,
    ArgumentBindings,
    SpecificityChecks,
    Overloads,
    Parameters,
    DiagnosticConsiderations,
}

/// Public signature-search/result limit category.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignatureLimitKind {
    CandidateCalls,
    Overloads,
    ParametersPerSignature,
    NestedCalls,
    RecoveryNodes,
    SourceBytes,
    Diagnostics,
    WorkUnits,
}

/// Public signature-search/result limit failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("signature query limit exceeded")]
pub struct SignatureLimitExceeded {
    pub kind: SignatureLimitKind,
    pub observed: u64,
    pub maximum: u64,
}

/// Invalid custom signature-query limit configuration.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SignatureLimitConfigurationError {
    #[error("signature query limit {kind:?} must be positive")]
    Zero { kind: SignatureLimitKind },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorruptCallableCatalogReason {
    EmptySet,
    KeyMismatch,
    DuplicateId,
    WrongAuthority,
    MissingRecord,
    InvalidEquivalent,
    Unsorted,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResolveCallError {
    #[error("call resolution was cancelled")]
    Cancelled,
    #[error("call resolution deadline elapsed")]
    DeadlineExceeded,
    #[error("call resolution world does not match the accepted request")]
    WorldMismatch,
    #[error("call source does not match the accepted document")]
    SourceIdentityMismatch,
    #[error("call source span is invalid")]
    InvalidSourceSpan,
    #[error("candidate {candidate:?} has no call group {group:?}")]
    InvalidCallGroup {
        candidate: Box<CallableCandidateId>,
        group: CallableGroupIndex,
    },
    #[error("call produced {actual} candidates; maximum is {limit}")]
    CandidateLimit { actual: usize, limit: usize },
    #[error("call is ambiguous between {candidates:?}")]
    AmbiguousOverload {
        candidates: Arc<[CallableCandidateId]>,
    },
    #[error("trait method is ambiguous between {candidates:?}")]
    AmbiguousTraitMethod { candidates: Arc<[TraitCallableId]> },
    #[error("data-last call is ambiguous between {candidates:?}")]
    DataLastAmbiguity {
        candidates: Arc<[DataLastCallableId]>,
    },
    #[error("callable catalog is corrupt at {key:?}: {reason:?}")]
    CorruptCatalog {
        key: CallableLookupKey,
        reason: CorruptCallableCatalogReason,
    },
    #[error("resolved callable violates an identity invariant")]
    InvalidResolvedCallable,
    #[error(transparent)]
    Work(#[from] CallableQueryLimitError),
    #[error(transparent)]
    SignatureLimit(#[from] SignatureLimitExceeded),
    #[error("signature query counter overflowed")]
    SignatureArithmeticOverflow { counter: SignatureWorkKind },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum CallTargetFactError {
    #[error("focused call-target facts require focused checker mode")]
    FocusedModeRequired,
    #[error("focused call source is not part of the accepted project: {document:?}")]
    FocusedSourceUnavailable { document: SourceDocumentIdentity },
    #[error("focused call target {call:?} was not recorded")]
    FocusedTargetMissing { call: SourceSpan },
    #[error("focused call target {call:?} was recorded more than once")]
    FocusedTargetDuplicate { call: SourceSpan },
    #[error("focused call target {call:?} could not retain checked facts: {reason}")]
    Unavailable {
        call: SourceSpan,
        reason: SemanticSignatureError,
    },
    #[error("focused call target {call:?} could not be resolved: {reason}")]
    Resolve {
        call: SourceSpan,
        reason: Box<ResolveCallError>,
    },
    #[error("focused signature accounting failed: {reason:?}")]
    SignatureAccounting {
        reason: super::SignatureAccountingError,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SemanticSignatureError {
    #[error("semantic signature help cannot be empty")]
    EmptySignatures,
    #[error("active signature is out of bounds")]
    ActiveSignatureOutOfBounds,
    #[error("active parameter is out of bounds")]
    ActiveParameterOutOfBounds,
    #[error("current call group is absent")]
    CurrentGroupMissing,
    #[error("semantic signature contains a duplicate candidate")]
    DuplicateCandidate,
    #[error("semantic signature contains a duplicate equivalent candidate")]
    DuplicateEquivalentCandidate,
    #[error("semantic signature source identity mismatch")]
    SourceIdentityMismatch,
    #[error("semantic signature contains an invalid span")]
    InvalidSpan,
    #[error(transparent)]
    Limit(#[from] CallableQueryLimitError),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDiagnosticCode {
    UnknownCallable,
    UnknownMethod,
    NonCallableTarget,
    UnknownFxConstructor,
    InvalidFxPath,
    AmbiguousOverload,
    NoViableSignature,
    DiagnosticsTruncated,
    AmbiguousTraitMethod,
    DuplicateArgument,
    UnknownNamedArgument,
    MissingArgument,
    TooManyPositionalArguments,
    UnsupportedSpread,
    InvalidCallGroup,
    ArgumentTypeMismatch,
    ResultConstructorExpectedType,
    EnumConstructorExpectedType,
    CharacterOwnerMissing,
    CharacterOwnerTypeMismatch,
    CharacterOwnerUnknownExternal,
    CharacterOwnerUnknownPart,
    PresentationLookOwnerUnavailable,
    DialogueLookOwnerUnavailable,
    DataLastAmbiguity,
    DataLastShadowed,
    VirtualPathRejected,
    CorruptCallableCatalog,
    WorldMismatch,
    SourceIdentityMismatch,
    Cancelled,
    DeadlineExceeded,
    ResourceExhausted,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableCatalogBuildError {
    #[error("duplicate typed callable ID {id:?}")]
    DuplicateTypedId { id: CallableCandidateId },
    #[error("same-rank provider collision for {key:?}")]
    SameRankCollision {
        key: CallableLookupKey,
        rank: CallableAuthorityRank,
        first: CallableProviderId,
        second: CallableProviderId,
    },
    #[error("duplicate provider overload for {key:?}")]
    DuplicateProviderOverload {
        key: CallableLookupKey,
        provider: CallableProviderId,
        overload: CallableOverloadIndex,
    },
    #[error("non-contiguous provider overload for {key:?}")]
    NonContiguousOverloads {
        key: CallableLookupKey,
        provider: CallableProviderId,
        expected: CallableOverloadIndex,
        actual: CallableOverloadIndex,
    },
    #[error("project binding collision at {path:?}")]
    ProjectBindingCollision {
        path: ProjectCallablePath,
        first: ProjectNameBinding,
        second: ProjectNameBinding,
    },
    #[error("project binding target {target:?} has no registered semantic type")]
    MissingProjectBindingType { target: ProjectSymbolTargetId },
    #[error("project module {module:?} has no source")]
    MissingProjectModuleSource { module: CanonicalModulePath },
    #[error("project callable identity mismatch for {declaration:?}")]
    ProjectIdentityMismatch { declaration: CallableDeclarationId },
    #[error(
        "extern Rust alias {path:?} for {package}::{export:?} matches {candidates} callable records"
    )]
    AmbiguousRustExternBinding {
        path: ProjectCallablePath,
        package: String,
        export: CallableName,
        candidates: usize,
    },
    #[error(transparent)]
    InvalidRecord(#[from] CallableCatalogError),
    #[error(transparent)]
    InvalidPublication(#[from] CallablePublicationError),
    #[error(transparent)]
    InvalidSchema(#[from] CallableSchemaError),
    #[error(transparent)]
    Limit(#[from] CallableBuildLimitError),
    #[error("catalog build work arithmetic overflow")]
    WorkOverflow,
}

impl CallableCatalogBuildError {
    pub const fn code(&self) -> CallableDiagnosticCode {
        match self {
            Self::Limit(_) | Self::WorkOverflow => CallableDiagnosticCode::ResourceExhausted,
            Self::DuplicateTypedId { .. }
            | Self::SameRankCollision { .. }
            | Self::DuplicateProviderOverload { .. }
            | Self::NonContiguousOverloads { .. }
            | Self::ProjectBindingCollision { .. }
            | Self::MissingProjectBindingType { .. }
            | Self::MissingProjectModuleSource { .. }
            | Self::ProjectIdentityMismatch { .. }
            | Self::AmbiguousRustExternBinding { .. }
            | Self::InvalidRecord(_)
            | Self::InvalidPublication(_)
            | Self::InvalidSchema(_) => CallableDiagnosticCode::CorruptCallableCatalog,
        }
    }
}

impl ResolveCallError {
    pub const fn code(&self) -> CallableDiagnosticCode {
        match self {
            Self::Cancelled => CallableDiagnosticCode::Cancelled,
            Self::DeadlineExceeded => CallableDiagnosticCode::DeadlineExceeded,
            Self::WorldMismatch => CallableDiagnosticCode::WorldMismatch,
            Self::SourceIdentityMismatch | Self::InvalidSourceSpan => {
                CallableDiagnosticCode::SourceIdentityMismatch
            }
            Self::AmbiguousOverload { .. } => CallableDiagnosticCode::AmbiguousOverload,
            Self::AmbiguousTraitMethod { .. } => CallableDiagnosticCode::AmbiguousTraitMethod,
            Self::DataLastAmbiguity { .. } => CallableDiagnosticCode::DataLastAmbiguity,
            Self::CorruptCatalog { .. } => CallableDiagnosticCode::CorruptCallableCatalog,
            Self::InvalidCallGroup { .. } => CallableDiagnosticCode::InvalidCallGroup,
            Self::CandidateLimit { .. }
            | Self::InvalidResolvedCallable
            | Self::Work(_)
            | Self::SignatureLimit(_)
            | Self::SignatureArithmeticOverflow { .. } => CallableDiagnosticCode::ResourceExhausted,
        }
    }
}
