//! HIR-owned semantic leaf values.
//!
//! These values retain semantic spelling and typed structure only. Exact
//! source spelling, ranges, and revision ownership remain in the HIR source
//! index. Lowering constructs these records from attached typed syntax without
//! reparsing source text.

use crate::identity::{HirLimit, HirSnapshotId, ScopeId, SyntheticKey, SyntheticOwner, TypeId};
use arcweft_lang_syntax::cst::is_identifier;
use thiserror::Error;

/// One parser-validated Arcweft identifier without source trivia.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirName(Box<str>);

impl HirName {
    pub(crate) fn try_new(value: Box<str>) -> Result<Self, HirNameInvariantError> {
        if !is_identifier(&value) {
            return Err(HirNameInvariantError::InvalidIdentifier);
        }
        Ok(Self(value))
    }

    /// Returns the exact semantic code points retained from typed syntax.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Invalid semantic name retained as typed recovery evidence.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirNameInvariantError {
    #[error("HIR name is not one parser-valid identifier")]
    InvalidIdentifier,
}

/// A short-variant name or typed recovery from the same syntax family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirShortVariantName {
    /// One parser-validated variant name.
    Resolved(HirName),
    /// The required name was missing or invalid.
    Recovered(HirNameInvariantError),
}

impl HirShortVariantName {
    /// Returns the resolved semantic name, when construction succeeded.
    pub const fn as_resolved(&self) -> Option<&HirName> {
        match self {
            Self::Resolved(name) => Some(name),
            Self::Recovered(_) => None,
        }
    }

    /// Returns the exact typed recovery issue, when present.
    pub const fn recovery_issue(&self) -> Option<HirNameInvariantError> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered(issue) => Some(*issue),
        }
    }
}

/// One external-project-capable semantic path segment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirProjectSymbolSegment(Box<str>);

impl HirProjectSymbolSegment {
    pub(crate) fn try_new(value: Box<str>) -> Option<Self> {
        (!value.is_empty()
            && value
                .chars()
                .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-')))
        .then_some(Self(value))
    }

    /// Returns the exact semantic code points retained from typed syntax.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A root-preserving semantic path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPath {
    root: HirPathRoot,
    segments: Box<[HirPathSegment]>,
}

impl HirPath {
    pub(crate) fn try_new(
        root: HirPathRoot,
        segments: Box<[HirPathSegment]>,
    ) -> Result<Self, HirPathIssue> {
        if segments.is_empty() {
            return Err(HirPathIssue::Empty);
        }
        let root = match root {
            HirPathRoot::Super { depth: 0 } => HirPathRoot::SelfModule,
            root => root,
        };
        Ok(Self { root, segments })
    }

    /// Returns the authored path-root semantics after canonicalizing
    /// `Super { depth: 0 }` to [`HirPathRoot::SelfModule`].
    pub const fn root(&self) -> HirPathRoot {
        self.root
    }

    /// Returns the non-empty semantic segments in authored order.
    pub fn segments(&self) -> &[HirPathSegment] {
        &self.segments
    }
}

/// Root semantics retained by a HIR path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPathRoot {
    /// An unqualified path whose first segment may resolve through an import
    /// alias before falling back to the crate root.
    ImplicitCrate,
    /// An explicit crate-rooted path.
    Crate,
    /// A path rooted at the owner's module.
    SelfModule,
    /// A path rooted at an exact parent-module depth.
    Super { depth: usize },
}

/// One typed segment of a HIR path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPathSegment {
    /// A canonical Arcweft identifier.
    Identifier(HirName),
    /// An external-project-capable symbol segment.
    ProjectSymbol(HirProjectSymbolSegment),
}

/// Snapshot and lexical owner used when resolving one semantic path.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HirPathResolutionContext {
    snapshot: HirSnapshotId,
    owner_scope: ScopeId,
}

impl HirPathResolutionContext {
    pub(crate) fn new(snapshot: HirSnapshotId, owner_scope: ScopeId) -> Result<Self, HirPathIssue> {
        if owner_scope.module() != snapshot.module() {
            return Err(HirPathIssue::ForeignScope);
        }
        Ok(Self {
            snapshot,
            owner_scope,
        })
    }

    /// Returns the immutable snapshot in which resolution occurs.
    pub const fn snapshot(self) -> HirSnapshotId {
        self.snapshot
    }

    /// Returns the lexical scope from which resolution begins.
    pub const fn owner_scope(self) -> ScopeId {
        self.owner_scope
    }
}

/// Typed path construction or resolution failure.
#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPathIssue {
    /// A semantic path contained no segments.
    #[error("a HIR path requires at least one semantic segment")]
    Empty,
    /// Typed lowering rejected a path segment at the given source ordinal.
    #[error("HIR path segment {ordinal} is invalid")]
    InvalidSegment { ordinal: u32 },
    /// A `super` root escaped the current crate.
    #[error("HIR path super depth {depth} exceeds the {available} available parent modules")]
    SuperEscapesCrate { depth: usize, available: usize },
    /// No import alias matched the external-capable first segment.
    #[error("unknown HIR import alias {segment:?}")]
    UnknownAlias { segment: HirProjectSymbolSegment },
    /// More than one import alias matched the first segment.
    #[error("ambiguous HIR import alias {segment:?}")]
    AmbiguousAlias { segment: HirProjectSymbolSegment },
    /// No accepted external project matched the first segment.
    #[error("unknown external HIR project {segment:?}")]
    UnknownExternalProject { segment: HirProjectSymbolSegment },
    /// The path target is not part of the accepted project publication.
    #[error("HIR path target is not published")]
    UnpublishedTarget,
    /// Resolution was attempted against a stale HIR snapshot.
    #[error("HIR path resolution snapshot is stale")]
    StaleSnapshot,
    /// The lexical owner belongs to a different module from the snapshot.
    #[error("HIR path resolution scope belongs to a foreign module")]
    ForeignScope,
}

/// A complete semantic path or typed recovery from the same path family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPathValue {
    /// One valid non-empty path.
    Resolved(HirPath),
    /// A classified path whose semantic segments could not be admitted.
    Recovered(HirPathRecovery),
}

impl HirPathValue {
    /// Returns the resolved semantic path, when construction succeeded.
    pub const fn as_resolved(&self) -> Option<&HirPath> {
        match self {
            Self::Resolved(path) => Some(path),
            Self::Recovered(_) => None,
        }
    }

    /// Returns the typed path recovery, when present.
    pub const fn recovery(&self) -> Option<&HirPathRecovery> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered(recovery) => Some(recovery),
        }
    }
}

/// Root-preserving source-role shape of a recovered path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPathRecovery {
    root: HirPathRoot,
    segment_count: u32,
    issue: HirPathIssue,
}

impl HirPathRecovery {
    pub(crate) const fn new(root: HirPathRoot, segment_count: u32, issue: HirPathIssue) -> Self {
        let root = match root {
            HirPathRoot::Super { depth: 0 } => HirPathRoot::SelfModule,
            root => root,
        };
        Self {
            root,
            segment_count,
            issue,
        }
    }

    /// Returns the exact semantic root selected before segment recovery.
    pub const fn root(&self) -> HirPathRoot {
        self.root
    }

    /// Returns the number of authored segment roles retained by the family.
    pub const fn segment_count(&self) -> u32 {
        self.segment_count
    }

    /// Returns the exact typed path issue.
    pub const fn issue(&self) -> &HirPathIssue {
        &self.issue
    }
}

/// Absolute, relative, or family-relative entity identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdRef {
    /// An absolute entity reference.
    Absolute(HirEntityReference),
    /// A reference relative to an exact parent depth.
    Relative(HirRelativeId),
    /// A family-qualified relative reference.
    FamilyRelative(HirFamilyRelativeId),
}

impl HirIdRef {
    pub(crate) const fn absolute(reference: HirEntityReference) -> Self {
        Self::Absolute(reference)
    }

    pub(crate) const fn relative(relative: HirRelativeId) -> Self {
        Self::Relative(relative)
    }

    pub(crate) const fn family_relative(relative: HirFamilyRelativeId) -> Self {
        Self::FamilyRelative(relative)
    }

    /// Returns the first segment of an absolute reference.
    ///
    /// Declaration families use this typed projection to validate canonical
    /// IDs without reparsing source text or a display-form label.
    pub fn absolute_family(&self) -> Option<&str> {
        match self {
            Self::Absolute(reference) => reference.as_str().split('.').next(),
            Self::Relative(_) | Self::FamilyRelative(_) => None,
        }
    }
}

/// Normalized absolute entity-reference body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEntityReference(Box<str>);

impl HirEntityReference {
    pub(crate) fn try_new(value: Box<str>) -> Result<Self, HirIdRefInvariantError> {
        if value.is_empty() {
            return Err(HirIdRefInvariantError::EmptyAbsolute);
        }
        Ok(Self(value))
    }

    /// Returns the normalized absolute entity-reference body.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the exact number of normalized dot-separated ID segments.
    pub(crate) fn segment_count(&self) -> usize {
        self.0.split('.').count()
    }
}

/// Normalized suffix shared by relative entity-reference forms.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIdSuffix(Box<str>);

impl HirIdSuffix {
    pub(crate) fn try_new(value: Box<str>) -> Result<Self, HirIdRefInvariantError> {
        if value.is_empty() {
            return Err(HirIdRefInvariantError::EmptySuffix);
        }
        if value.contains('@') {
            return Err(HirIdRefInvariantError::AuthoredRelativeMarker);
        }
        if value.split('.').any(str::is_empty) {
            return Err(HirIdRefInvariantError::InvalidSuffix);
        }
        Ok(Self(value))
    }

    /// Returns the normalized relative suffix.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the exact number of normalized dot-separated suffix segments.
    pub(crate) fn segment_count(&self) -> usize {
        self.0.split('.').count()
    }
}

/// Normalized family name in a family-relative entity reference.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIdFamily(Box<str>);

impl HirIdFamily {
    pub(crate) fn try_new(value: Box<str>) -> Result<Self, HirIdRefInvariantError> {
        if !is_identifier(&value) {
            return Err(HirIdRefInvariantError::InvalidFamily);
        }
        Ok(Self(value))
    }

    /// Returns the normalized family name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Relative entity identity and its exact parent depth.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRelativeId {
    suffix: HirIdSuffix,
    parent_depth: usize,
}

impl HirRelativeId {
    pub(crate) const fn new(suffix: HirIdSuffix, parent_depth: usize) -> Self {
        Self {
            suffix,
            parent_depth,
        }
    }

    /// Returns the normalized relative suffix.
    pub const fn suffix(&self) -> &HirIdSuffix {
        &self.suffix
    }

    /// Returns the exact parent depth, including zero and [`usize::MAX`].
    pub const fn parent_depth(&self) -> usize {
        self.parent_depth
    }
}

/// Family-qualified relative entity identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFamilyRelativeId {
    family: HirIdFamily,
    relative: HirRelativeId,
}

impl HirFamilyRelativeId {
    pub(crate) const fn new(family: HirIdFamily, relative: HirRelativeId) -> Self {
        Self { family, relative }
    }

    /// Returns the normalized family name.
    pub const fn family(&self) -> &HirIdFamily {
        &self.family
    }

    /// Returns the complete relative portion.
    pub const fn relative(&self) -> &HirRelativeId {
        &self.relative
    }
}

/// Rejection produced while constructing normalized entity-reference parts.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdRefInvariantError {
    /// An absolute entity-reference body was empty.
    #[error("an absolute HIR entity reference cannot be empty")]
    EmptyAbsolute,
    /// A relative suffix was empty.
    #[error("a relative HIR ID suffix cannot be empty")]
    EmptySuffix,
    /// A semantic relative suffix retained its authored `@` marker.
    #[error("a normalized relative HIR ID cannot retain an authored `@` marker")]
    AuthoredRelativeMarker,
    /// A relative suffix contained an invalid empty segment.
    #[error("a relative HIR ID suffix must contain non-empty dot-separated segments")]
    InvalidSuffix,
    /// A family-relative reference did not contain one canonical identifier.
    #[error("a family-relative HIR ID requires one identifier family")]
    InvalidFamily,
}

/// An entity-reference leaf or typed recovery from the same token family.
///
/// Expression and Pattern HIR share this owner so recovery cannot drift into
/// two ID-reference grammars.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdRefValue {
    /// A complete absolute, relative, or family-relative reference.
    Resolved(HirIdRef),
    /// A classified reference whose valid semantic identity could not be built.
    Recovered(HirIdRefRecovery),
}

impl HirIdRefValue {
    /// Returns the valid semantic reference, when construction succeeded.
    pub const fn as_resolved(&self) -> Option<&HirIdRef> {
        match self {
            Self::Resolved(reference) => Some(reference),
            Self::Recovered(_) => None,
        }
    }

    /// Returns the typed recovery and its source-role shape, when present.
    pub const fn recovery(&self) -> Option<&HirIdRefRecovery> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered(recovery) => Some(recovery),
        }
    }

    pub(crate) const fn recovery_issue(&self) -> Option<HirIdRefIssue> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered(recovery) => Some(recovery.issue()),
        }
    }

    pub(crate) const fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

/// Exact source-role shape and issue retained for one recovered ID reference.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIdRefRecovery {
    shape: HirIdRefShape,
    issue: HirIdRefIssue,
}

impl HirIdRefRecovery {
    pub(crate) const fn new(shape: HirIdRefShape, issue: HirIdRefIssue) -> Self {
        Self { shape, issue }
    }

    /// Returns the classified absolute, relative, or family-relative shape.
    pub const fn shape(&self) -> HirIdRefShape {
        self.shape
    }

    /// Returns why no valid semantic ID reference was admitted.
    pub const fn issue(&self) -> HirIdRefIssue {
        self.issue
    }
}

/// Semantic form retained for source-role applicability after ID recovery.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdRefShape {
    /// No reference body was present.
    Missing,
    /// An absolute marker followed by this many suffix segments.
    Absolute { segment_count: u32 },
    /// An exact relative parent depth and suffix cardinality.
    Relative {
        parent_depth: usize,
        suffix_segment_count: u32,
    },
    /// A family prefix, relative parent depth, and suffix cardinality.
    FamilyRelative {
        parent_depth: usize,
        suffix_segment_count: u32,
    },
}

/// Typed reason that an entity-reference leaf did not produce a valid ID.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdRefIssue {
    /// The source family was recognized but its required body was missing.
    #[error("HIR entity reference is missing its reference body")]
    Missing,
    /// One normalized component violated the shared ID-reference invariant.
    #[error("HIR entity reference has invalid structure")]
    Invalid(HirIdRefInvariantError),
}

/// Region identity retained only by HIR type nodes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTypeRegion {
    /// An explicitly named region.
    Named(HirRegionName),
    /// A source-anchored elided region.
    Elided(HirElidedRegion),
}

impl HirTypeRegion {
    pub(crate) const fn named(region: HirRegionName) -> Self {
        Self::Named(region)
    }

    pub(crate) const fn elided(region: HirElidedRegion) -> Self {
        Self::Elided(region)
    }
}

/// Canonical name of an explicit HIR type region.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRegionName(HirName);

impl HirRegionName {
    pub(crate) const fn new(name: HirName) -> Self {
        Self(name)
    }

    /// Returns the canonical region name.
    pub const fn name(&self) -> &HirName {
        &self.0
    }
}

/// Source-derived identity of an elided region on one reference type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirElidedRegion {
    key: SyntheticKey,
}

impl HirElidedRegion {
    pub(crate) fn try_new(owner: TypeId, key: SyntheticKey) -> Result<Self, HirElidedRegionError> {
        let actual = key.owner();
        if actual != SyntheticOwner::Type(owner) {
            return Err(HirElidedRegionError::OwnerMismatch {
                expected: owner,
                actual,
            });
        }
        Ok(Self { key })
    }

    /// Returns the reference type that owns this elided region.
    pub fn owner_type(self) -> TypeId {
        match self.key.owner() {
            SyntheticOwner::Type(owner) => owner,
            _ => unreachable!("HirElidedRegion construction validates its typed owner"),
        }
    }

    /// Returns the source-derived elision key.
    pub const fn key(self) -> SyntheticKey {
        self.key
    }
}

/// Typed owner mismatch while constructing an elided region.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirElidedRegionError {
    /// The supplied key belongs to a different typed owner.
    #[error("elided region key belongs to {actual:?}, expected type {expected:?}")]
    OwnerMismatch {
        expected: TypeId,
        actual: SyntheticOwner,
    },
}

/// Typed poison retained while lowering a type region.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTypeRegionIssue {
    /// The authored named region was invalid.
    #[error("invalid named HIR type region")]
    InvalidNamedRegion,
    /// The elision key did not belong to the reference type.
    #[error("invalid HIR type-region elision owner")]
    InvalidElisionOwner,
}

/// Runtime lifetime-registry path, separate from type-region identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirLifetimeRegistryPath {
    scope: HirLifetimeRegistryScope,
    segments: Box<[HirName]>,
    optional: bool,
}

impl HirLifetimeRegistryPath {
    pub(crate) fn try_new(
        scope: HirLifetimeRegistryScope,
        segments: Box<[HirName]>,
        optional: bool,
    ) -> Self {
        Self {
            scope,
            segments,
            optional,
        }
    }

    /// Returns the registry scope.
    pub const fn scope(&self) -> &HirLifetimeRegistryScope {
        &self.scope
    }

    /// Returns the ordered registry key segments.
    pub fn segments(&self) -> &[HirName] {
        &self.segments
    }

    /// Returns whether a read treats a missing key as optional.
    pub const fn optional(&self) -> bool {
        self.optional
    }
}

/// Runtime registry scope.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLifetimeRegistryScope {
    Frame,
    Tick,
    Cue,
    Line,
    Scene,
    Flow,
    Session,
    Global,
    Persistent,
    Named(HirName),
}

/// Operation performed against a runtime lifetime-registry path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLifetimeRegistryAccessMode {
    Read,
    Write,
    MoveOut,
    Drop,
    Expose,
}

/// Typed poison retained while lowering a registry path or access.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLifetimeRegistryIssue {
    #[error("invalid named HIR lifetime-registry scope")]
    InvalidNamedScope,
    #[error("invalid HIR lifetime-registry key segment {ordinal}")]
    InvalidKeySegment { ordinal: u32 },
    #[error("optional HIR lifetime-registry access must be Read")]
    OptionalNonReadAccess,
    #[error("HIR lifetime-registry access is missing its scope")]
    MissingScope,
}

/// A runtime lifetime-registry path or typed recovery from the same family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLifetimePathValue {
    /// One valid runtime registry path.
    Resolved(HirLifetimeRegistryPath),
    /// A classified registry path whose valid semantic value could not be built.
    Recovered(HirLifetimePathRecovery),
}

impl HirLifetimePathValue {
    /// Returns the resolved runtime path, when construction succeeded.
    pub const fn as_resolved(&self) -> Option<&HirLifetimeRegistryPath> {
        match self {
            Self::Resolved(path) => Some(path),
            Self::Recovered(_) => None,
        }
    }

    /// Returns the typed registry recovery, when present.
    pub const fn recovery(&self) -> Option<&HirLifetimePathRecovery> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered(recovery) => Some(recovery),
        }
    }
}

/// Source-role shape and issue retained for a recovered runtime registry path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirLifetimePathRecovery {
    scope_present: bool,
    segment_count: u32,
    optional_marker: bool,
    issue: HirLifetimeRegistryIssue,
}

impl HirLifetimePathRecovery {
    pub(crate) const fn new(
        scope_present: bool,
        segment_count: u32,
        optional_marker: bool,
        issue: HirLifetimeRegistryIssue,
    ) -> Self {
        Self {
            scope_present,
            segment_count,
            optional_marker,
            issue,
        }
    }

    /// Returns whether an authored registry-scope component exists.
    pub const fn scope_present(&self) -> bool {
        self.scope_present
    }

    /// Returns the number of authored registry-key segment roles.
    pub const fn segment_count(&self) -> u32 {
        self.segment_count
    }

    /// Returns whether the optional-read marker was authored.
    pub const fn optional_marker(&self) -> bool {
        self.optional_marker
    }

    /// Returns the exact typed registry-path issue.
    pub const fn issue(&self) -> HirLifetimeRegistryIssue {
        self.issue
    }
}

/// Semantic literal retained by expression and pattern arenas.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLiteral {
    String(HirStringLiteral),
    Character(HirCharacterLiteral),
    Integer(HirIntegerLiteral),
    Float(HirFloatLiteral),
    UnitNumber(HirUnitNumberLiteral),
    Boolean(bool),
    Duration(HirDurationLiteral),
}

/// Decoded string value or typed string-family recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStringLiteral {
    Value(Box<str>),
    Invalid(HirStringIssue),
}

/// Decoded Unicode scalar or typed character-family recovery.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCharacterLiteral {
    Value(char),
    Invalid(HirCharacterIssue),
}

/// Canonical non-negative arbitrary-precision integer magnitude.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirBigUint {
    limbs_le: Box<[u32]>,
}

impl HirBigUint {
    pub(crate) fn try_new(limbs_le: Box<[u32]>) -> Option<Self> {
        (limbs_le.last().is_none_or(|limb| *limb != 0)).then_some(Self { limbs_le })
    }

    /// Returns canonical base-2^32 limbs in little-endian order.
    pub fn limbs_le(&self) -> &[u32] {
        &self.limbs_le
    }

    /// Returns whether this magnitude is canonical zero.
    pub fn is_zero(&self) -> bool {
        self.limbs_le.is_empty()
    }
}

/// Exact integer literal or typed integer-family recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIntegerLiteral {
    Value {
        magnitude: HirBigUint,
        radix: HirIntegerRadix,
        suffix: Option<HirIntegerSuffix>,
    },
    Invalid(HirIntegerIssue),
}

/// Authored radix retained for structural HIR identity and diagnostics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIntegerRadix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

/// Explicit integer type suffix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIntegerSuffix {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
}

/// Canonical arbitrary-precision decimal.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDecimal {
    coefficient: HirDecimalDigits,
    scale: u32,
    exponent10: i32,
}

impl HirDecimal {
    pub(crate) fn try_new(
        coefficient: HirDecimalDigits,
        scale: u32,
        exponent10: i32,
    ) -> Result<Self, HirDecimalInvariantError> {
        let coefficient_digits = coefficient.digits().len();
        if coefficient_digits > HirLimit::DecimalCoefficientDigits.maximum() {
            return Err(HirDecimalInvariantError::CoefficientDigits {
                observed: coefficient_digits,
                maximum: HirLimit::DecimalCoefficientDigits.maximum(),
            });
        }
        let scale_observed = usize::try_from(scale).expect("u32 scale fits usize");
        if scale_observed > HirLimit::DecimalScale.maximum() {
            return Err(HirDecimalInvariantError::Scale {
                observed: scale_observed,
                maximum: HirLimit::DecimalScale.maximum(),
            });
        }
        let exponent_observed = usize::try_from(exponent10.unsigned_abs()).unwrap_or(usize::MAX);
        if exponent_observed > HirLimit::DecimalExponentAbs.maximum() {
            return Err(HirDecimalInvariantError::ExponentAbs {
                observed: exponent_observed,
                maximum: HirLimit::DecimalExponentAbs.maximum(),
            });
        }
        if coefficient.digits() == [0] && (scale != 0 || exponent10 != 0) {
            return Err(HirDecimalInvariantError::NonCanonicalZero);
        }
        Ok(Self {
            coefficient,
            scale,
            exponent10,
        })
    }

    /// Returns the canonical coefficient digits.
    pub const fn coefficient(&self) -> &HirDecimalDigits {
        &self.coefficient
    }

    /// Returns the canonical decimal scale.
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// Returns the canonical base-10 exponent.
    pub const fn exponent10(&self) -> i32 {
        self.exponent10
    }
}

/// Closed-constructor failure for one canonical decimal payload.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDecimalInvariantError {
    #[error("decimal coefficient has {observed} digits, maximum {maximum}")]
    CoefficientDigits { observed: usize, maximum: usize },
    #[error("decimal scale is {observed}, maximum {maximum}")]
    Scale { observed: usize, maximum: usize },
    #[error("decimal exponent absolute value is {observed}, maximum {maximum}")]
    ExponentAbs { observed: usize, maximum: usize },
    #[error("zero decimal must have zero scale and exponent")]
    NonCanonicalZero,
}

/// Canonical decimal coefficient digits in most-significant-first order.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDecimalDigits(Box<[u8]>);

impl HirDecimalDigits {
    pub(crate) fn try_new(digits: Box<[u8]>) -> Option<Self> {
        if digits.is_empty() || digits.iter().any(|digit| *digit > 9) {
            return None;
        }
        let canonical = if digits.as_ref() == [0] {
            true
        } else {
            digits.first().is_some_and(|digit| *digit != 0)
                && digits.last().is_some_and(|digit| *digit != 0)
        };
        canonical.then_some(Self(digits))
    }

    /// Returns canonical coefficient digits in most-significant-first order.
    pub fn digits(&self) -> &[u8] {
        &self.0
    }
}

/// Exact decimal float literal or typed float-family recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFloatLiteral {
    Value {
        decimal: HirDecimal,
        explicit_width: Option<HirFloatWidth>,
    },
    Invalid(HirFloatIssue),
}

/// Explicit IEEE-754 width requested by an authored float suffix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFloatWidth {
    F32,
    F64,
}

/// Exact checked IEEE-754 bits.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFloatBits {
    F32(u32),
    F64(u64),
}

/// Checker-admitted float value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedFloatLiteral {
    bits: HirFloatBits,
}

impl CheckedFloatLiteral {
    pub(crate) const fn new(bits: HirFloatBits) -> Self {
        Self { bits }
    }

    /// Returns the exact admitted IEEE-754 bits.
    pub const fn bits(&self) -> HirFloatBits {
        self.bits
    }
}

/// Exact decimal unit-number literal or typed unit-family recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnitNumberLiteral {
    Value {
        decimal: HirDecimal,
        unit: HirUnitNumberUnit,
    },
    Invalid(HirUnitNumberIssue),
}

/// Normalized unit-number unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnitNumberUnit {
    Percent,
    Px,
    Pt,
    Em,
    Rem,
    Vw,
    Vh,
    Deg,
    Rad,
    Turn,
    Db,
    Lufs,
    Bpm,
    Bars,
}

/// Exact Duration literal or typed Duration-family recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDurationLiteral {
    Value(HirDurationValue),
    Invalid(HirDurationIssue),
}

/// Structural Duration value retaining its normalized authored unit.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDurationValue {
    semantic: HirDurationSemanticValue,
    authored_unit: HirDurationUnit,
}

impl HirDurationValue {
    pub(crate) const fn new(
        semantic: HirDurationSemanticValue,
        authored_unit: HirDurationUnit,
    ) -> Self {
        Self {
            semantic,
            authored_unit,
        }
    }

    /// Returns the unit-insensitive whole-nanosecond semantic value.
    pub const fn semantic_value(&self) -> &HirDurationSemanticValue {
        &self.semantic
    }

    /// Returns the normalized unit authored for this value.
    pub const fn authored_unit(&self) -> HirDurationUnit {
        self.authored_unit
    }
}

/// Unit-insensitive whole-nanosecond Duration value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDurationSemanticValue {
    nanoseconds: HirBigUint,
}

impl HirDurationSemanticValue {
    pub(crate) const fn try_new(nanoseconds: HirBigUint) -> Self {
        Self { nanoseconds }
    }

    /// Returns the exact arbitrary-precision whole-nanosecond magnitude.
    pub const fn nanoseconds(&self) -> &HirBigUint {
        &self.nanoseconds
    }
}

/// Normalized authored Duration unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDurationUnit {
    Nanos,
    Micros,
    Millis,
    Seconds,
    Minutes,
    Hours,
}

/// Compact ordered numeric literal elements owned by one expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirNumericSequence {
    elements: Box<[HirNumericSequenceElement]>,
    common_suffix: Option<HirIntegerSuffix>,
    recovery: HirNumericSequenceRecovery,
}

impl HirNumericSequence {
    pub(crate) fn try_new(
        elements: Box<[HirNumericSequenceElement]>,
        common_suffix: Option<HirIntegerSuffix>,
        recovery: HirNumericSequenceRecovery,
    ) -> Result<Self, HirNumericSequenceInvariantError> {
        let retained_len = u32::try_from(elements.len()).map_err(|_| {
            HirNumericSequenceInvariantError::RetainedElementCountOverflow {
                observed: elements.len(),
            }
        })?;
        match &recovery {
            HirNumericSequenceRecovery::Complete => {}
            HirNumericSequenceRecovery::MissingFinalElement { ordinal }
                if *ordinal == retained_len => {}
            HirNumericSequenceRecovery::MissingFinalElement { ordinal } => {
                return Err(HirNumericSequenceInvariantError::MissingFinalOrdinal {
                    expected: retained_len,
                    actual: *ordinal,
                });
            }
            HirNumericSequenceRecovery::InvalidElement { ordinal, .. }
                if *ordinal == retained_len => {}
            HirNumericSequenceRecovery::InvalidElement { ordinal, .. } => {
                return Err(HirNumericSequenceInvariantError::InvalidElementOrdinal {
                    maximum: retained_len,
                    actual: *ordinal,
                });
            }
            HirNumericSequenceRecovery::ConflictingSuffix {
                ordinal,
                first,
                conflicting,
            } if *ordinal < retained_len
                && Some(*first) == common_suffix
                && first != conflicting => {}
            HirNumericSequenceRecovery::ConflictingSuffix {
                first, conflicting, ..
            } if first == conflicting => {
                return Err(HirNumericSequenceInvariantError::IdenticalConflictingSuffix);
            }
            HirNumericSequenceRecovery::ConflictingSuffix { ordinal, .. }
                if *ordinal >= retained_len =>
            {
                return Err(HirNumericSequenceInvariantError::ConflictingSuffixOrdinal {
                    retained_len,
                    actual: *ordinal,
                });
            }
            HirNumericSequenceRecovery::ConflictingSuffix { first, .. }
                if Some(*first) != common_suffix =>
            {
                return Err(HirNumericSequenceInvariantError::ConflictingSuffixCommonMismatch);
            }
            HirNumericSequenceRecovery::ConflictingSuffix { ordinal, .. } => {
                return Err(HirNumericSequenceInvariantError::ConflictingSuffixOrdinal {
                    retained_len,
                    actual: *ordinal,
                });
            }
        }
        Ok(Self {
            elements,
            common_suffix,
            recovery,
        })
    }

    /// Returns the ID-less elements in authored order.
    pub fn elements(&self) -> &[HirNumericSequenceElement] {
        &self.elements
    }

    /// Returns the one normalized suffix shared by all valid elements.
    pub const fn common_suffix(&self) -> Option<HirIntegerSuffix> {
        self.common_suffix
    }

    /// Returns the typed sequence recovery state.
    pub const fn recovery(&self) -> &HirNumericSequenceRecovery {
        &self.recovery
    }

    /// Returns the contiguous authored/recovered element-role domain.
    ///
    /// A missing or invalid element is omitted from the semantic element slice
    /// but still owns one exact source component. Conflicting suffix recovery
    /// retains the affected semantic element and therefore adds no slot.
    pub(crate) fn source_element_count(&self) -> usize {
        self.elements.len()
            + usize::from(matches!(
                self.recovery,
                HirNumericSequenceRecovery::MissingFinalElement { .. }
                    | HirNumericSequenceRecovery::InvalidElement { .. }
            ))
    }
}

/// Impossible relationship between compact numeric elements and recovery.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirNumericSequenceInvariantError {
    #[error("numeric-sequence retained element count {observed} does not fit a source ordinal")]
    RetainedElementCountOverflow { observed: usize },
    #[error("missing final numeric element has ordinal {actual}, expected {expected}")]
    MissingFinalOrdinal { expected: u32, actual: u32 },
    #[error("invalid numeric element has ordinal {actual}, maximum {maximum}")]
    InvalidElementOrdinal { maximum: u32, actual: u32 },
    #[error(
        "conflicting numeric suffix has ordinal {actual}, outside retained length {retained_len}"
    )]
    ConflictingSuffixOrdinal { retained_len: u32, actual: u32 },
    #[error("numeric-sequence suffix conflict repeats the same suffix")]
    IdenticalConflictingSuffix,
    #[error("numeric-sequence suffix conflict does not name the retained common suffix")]
    ConflictingSuffixCommonMismatch,
}

/// One ID-less compact numeric-sequence element.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirNumericSequenceElement {
    magnitude: HirBigUint,
    radix: HirIntegerRadix,
}

impl HirNumericSequenceElement {
    pub(crate) const fn new(magnitude: HirBigUint, radix: HirIntegerRadix) -> Self {
        Self { magnitude, radix }
    }

    /// Returns the exact arbitrary-precision element magnitude.
    pub const fn magnitude(&self) -> &HirBigUint {
        &self.magnitude
    }

    /// Returns the element's authored radix.
    pub const fn radix(&self) -> HirIntegerRadix {
        self.radix
    }
}

/// Typed recovery retained on a compact numeric sequence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirNumericSequenceRecovery {
    Complete,
    MissingFinalElement {
        ordinal: u32,
    },
    InvalidElement {
        ordinal: u32,
        issue: HirIntegerIssue,
    },
    ConflictingSuffix {
        ordinal: u32,
        first: HirIntegerSuffix,
        conflicting: HirIntegerSuffix,
    },
}

/// Literal-family poison projected without source spelling.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLiteralIssue {
    #[error(transparent)]
    String(HirStringIssue),
    #[error(transparent)]
    Character(HirCharacterIssue),
    #[error(transparent)]
    Integer(HirIntegerIssue),
    #[error(transparent)]
    Float(HirFloatIssue),
    #[error(transparent)]
    UnitNumber(HirUnitNumberIssue),
    #[error(transparent)]
    Duration(HirDurationIssue),
}

/// String literal recovery issue below hard resource limits.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStringIssue {
    #[error("invalid string escape")]
    InvalidEscape,
    #[error("unterminated string literal")]
    Unterminated,
}

/// Character literal recovery issue below hard resource limits.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCharacterIssue {
    #[error("invalid character escape")]
    InvalidEscape,
    #[error("unterminated character literal")]
    Unterminated,
    #[error("empty character literal")]
    Empty,
    #[error("character literal contains multiple Unicode scalars")]
    MultipleScalars,
}

/// Integer literal recovery issue below hard resource limits.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIntegerIssue {
    #[error("integer literal is missing digits")]
    MissingDigits,
    #[error("integer literal contains an invalid digit")]
    InvalidDigit,
}

/// Decimal recovery issue below hard resource limits.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDecimalIssue {
    #[error("decimal literal is missing its coefficient")]
    MissingCoefficient,
    #[error("decimal literal contains an invalid digit")]
    InvalidDigit,
}

/// Float literal recovery issue below hard resource limits.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFloatIssue {
    #[error(transparent)]
    Decimal(HirDecimalIssue),
    #[error("float literal is non-finite")]
    NonFinite,
    #[error("float literal has an invalid suffix")]
    InvalidSuffix,
}

/// Unit-number recovery issue below hard resource limits.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnitNumberIssue {
    #[error(transparent)]
    Decimal(HirDecimalIssue),
    #[error("unit-number literal has an invalid unit")]
    InvalidUnit,
}

/// Duration recovery issue below hard resource limits and runtime admission.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDurationIssue {
    #[error(transparent)]
    Decimal(HirDecimalIssue),
    #[error("Duration literal has an invalid unit")]
    InvalidUnit,
    #[error("Duration literal has a fractional nanosecond")]
    FractionalNanosecond,
}

#[cfg(test)]
#[path = "leaf/tests.rs"]
mod tests;
