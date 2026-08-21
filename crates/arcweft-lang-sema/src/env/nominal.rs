//! Exact accepted nominal facts and explicitly bounded open-name rules.

use core::{fmt, hash::Hasher};
use std::{collections::BTreeMap, hash::Hash};

use arcweft_character::id::CharacterId;
use arcweft_core::pattern::RuntimeOpaqueTypeProducerId;
use arcweft_lang_syntax::{
    ast::{
        module_path::{CanonicalModulePath, ModulePathRoot},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
    },
    types::TypePath,
};
use arcweft_source::SourceSpan;
use thiserror::Error;

use super::{base::TypeCheckEnv, identity::EnvironmentBindingId};
use crate::{
    nominal::{AcceptedNominalCatalogLimitKind, AcceptedNominalCatalogLimits},
    types::{CharacterNominalType, TypeKind},
};

const MAX_OPEN_NAMESPACE_TAIL: u16 = 16;
const MAX_NOMINAL_ARITY: u16 = 256;

#[derive(Clone, Copy)]
struct StandardOpaqueNominalSpec {
    name: &'static str,
    arity: u16,
    producer: &'static str,
}

const STANDARD_AGENT_ERROR: StandardOpaqueNominalSpec = StandardOpaqueNominalSpec {
    name: "AgentError",
    arity: 0,
    producer: "std.agent_error",
};

const STANDARD_OPAQUE_NOMINALS: [StandardOpaqueNominalSpec; 13] = [
    StandardOpaqueNominalSpec {
        name: "Reduction",
        arity: 1,
        producer: "std.reduction",
    },
    StandardOpaqueNominalSpec {
        name: "Watch",
        arity: 1,
        producer: "std.watch",
    },
    StandardOpaqueNominalSpec {
        name: "Sample",
        arity: 1,
        producer: "std.sample",
    },
    StandardOpaqueNominalSpec {
        name: "VirtualPath",
        arity: 0,
        producer: "std.virtual_path",
    },
    StandardOpaqueNominalSpec {
        name: "ArcError",
        arity: 0,
        producer: "std.arc_error",
    },
    StandardOpaqueNominalSpec {
        name: "ReducerError",
        arity: 0,
        producer: "std.reducer_error",
    },
    STANDARD_AGENT_ERROR,
    StandardOpaqueNominalSpec {
        name: "AssetError",
        arity: 0,
        producer: "std.asset_error",
    },
    StandardOpaqueNominalSpec {
        name: "ContentLoadError",
        arity: 0,
        producer: "std.content_load_error",
    },
    StandardOpaqueNominalSpec {
        name: "DialogueText",
        arity: 0,
        producer: "std.dialogue_text",
    },
    StandardOpaqueNominalSpec {
        name: "ImageHandle",
        arity: 0,
        producer: "std.image_handle",
    },
    StandardOpaqueNominalSpec {
        name: "PresentationLifetime",
        arity: 0,
        producer: "std.presentation_lifetime",
    },
    StandardOpaqueNominalSpec {
        name: "VoiceError",
        arity: 0,
        producer: "std.voice_error",
    },
];

/// Stable identity of one Rust package contributing accepted type exports.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustPackageId(String);

/// Invalid Rust package identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RustPackageIdError {
    #[error("Rust package identity must not be empty")]
    Empty,
    #[error("Rust package identity contains a control character at byte {byte}")]
    Control { byte: usize },
}

/// Owner of an exact accepted nominal declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedNominalOwnerId {
    Standard,
    Environment(EnvironmentBindingId),
    RustPackage(RustPackageId),
    Character(CharacterId),
}

/// Stable typed identity of one exact accepted nominal declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedNominalId {
    owner: AcceptedNominalOwnerId,
    canonical_path: TypePath,
}

/// Source family that contributed an accepted nominal fact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedNominalOrigin {
    Standard,
    Domain,
    NominalRecord,
    EnumInventory,
    RustExport,
    Character,
    Adapter,
    Test,
}

/// Semantic meaning assigned to an exact accepted nominal declaration.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AcceptedNominalSemantics {
    Exact(TypeKind),
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
    },
    Character(CharacterNominalType),
}

/// One exact accepted source-visible nominal declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalRecord {
    id: AcceptedNominalId,
    arity: u16,
    semantics: AcceptedNominalSemantics,
    origin: AcceptedNominalOrigin,
    source: Option<SourceSpan>,
}

/// Failure to instantiate one already accepted nominal declaration.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedNominalInstantiationError {
    #[error("accepted nominal `{id}` expects {expected} type argument(s), but received {actual}")]
    WrongArity {
        id: String,
        expected: u16,
        actual: usize,
    },
    #[error("accepted nominal `{id}` has semantics incompatible with its declared arity")]
    InvalidSemantics { id: String },
}

/// Stable identity of one explicitly registered open-name rule.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpenNominalRuleId {
    owner: EnvironmentBindingId,
    ordinal: u32,
}

/// Environment/module scope in which an open-name rule is effective.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalScope {
    AcceptedWorld,
    ExactModule(CanonicalModulePath),
    ModuleSubtree(CanonicalModulePath),
    DetachedOnly,
}

/// Exact path or bounded namespace accepted by an open-name rule.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalPattern {
    Exact(TypePath),
    Namespace {
        prefix: TypePath,
        min_tail_segments: u16,
        max_tail_segments: u16,
    },
}

/// Arity accepted by an open-name rule.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalArity {
    Exact(u16),
    Inclusive { minimum: u16, maximum: u16 },
}

/// One bounded rule for names that intentionally lack an exact catalog row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenNominalRule {
    id: OpenNominalRuleId,
    scope: OpenNominalScope,
    pattern: OpenNominalPattern,
    arity: OpenNominalArity,
    source: Option<SourceSpan>,
}

/// Execution world used when selecting an open-name rule.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalEnvironment {
    Accepted,
    Detached,
}

/// Deterministic digest of all accepted nominal catalog facts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedNominalCatalogDigest([u8; 32]);

/// Immutable exact/open nominal catalog owned by one semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalCatalog {
    exact: BTreeMap<TypePath, AcceptedNominalRecord>,
    open: BTreeMap<OpenNominalRuleId, OpenNominalRule>,
    digest: AcceptedNominalCatalogDigest,
}

/// Invalid bounded open-name pattern.
#[derive(Clone, Debug, Eq, Error, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalPatternError {
    #[error("open nominal namespace prefix must not be empty")]
    EmptyNamespacePrefix,
    #[error("open nominal pattern uses a reserved language type path")]
    ReservedPath,
    #[error("open nominal namespace must require at least one tail segment")]
    ZeroTail,
    #[error("open nominal namespace tail range is inverted")]
    InvertedTailRange,
    #[error("open nominal namespace tail maximum {maximum} exceeds {allowed}")]
    TailMaximumExceeded { maximum: u16, allowed: u16 },
}

/// Invalid accepted/open nominal catalog construction.
#[derive(Clone, Debug, Eq, Error, Ord, PartialEq, PartialOrd)]
pub enum AcceptedNominalCatalogError {
    #[error("duplicate accepted nominal path `{path}`")]
    DuplicateExactPath {
        path: TypePath,
        first: Box<AcceptedNominalId>,
        duplicate: Box<AcceptedNominalId>,
    },
    #[error("accepted nominal path `{path}` is reserved by the language")]
    ReservedPath { path: TypePath },
    #[error("accepted nominal arity {minimum}..={maximum} is invalid")]
    InvalidArity {
        source_span: Option<SourceSpan>,
        minimum: u16,
        maximum: u16,
    },
    #[error("open nominal rule {rule:?} has an invalid pattern: {reason}")]
    InvalidOpenPattern {
        rule: OpenNominalRuleId,
        reason: OpenNominalPatternError,
    },
    #[error("open nominal rules {first:?} and {second:?} overlap")]
    OverlappingOpenRules {
        first: OpenNominalRuleId,
        second: OpenNominalRuleId,
    },
    #[error("open nominal rule {rule:?} has an invalid environment scope {scope:?}")]
    InvalidScope {
        rule: OpenNominalRuleId,
        scope: OpenNominalScope,
    },
    #[error("accepted nominal {kind:?} capacity {maximum} was exceeded by {observed}")]
    Limit {
        kind: AcceptedNominalCatalogLimitKind,
        observed: u64,
        maximum: u64,
    },
}

impl RustPackageId {
    /// Validates and creates a Rust package identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, RustPackageIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RustPackageIdError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(RustPackageIdError::Control { byte });
        }
        Ok(Self(value))
    }

    /// Canonical package spelling used for presentation and manifest matching.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RustPackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AcceptedNominalOwnerId {
    /// Deterministic presentation label for diagnostics and tooling.
    pub fn source_label(&self) -> String {
        match self {
            Self::Standard => "standard".to_owned(),
            Self::Environment(owner) => format!("environment:{owner}"),
            Self::RustPackage(package) => format!("rust:{package}"),
            Self::Character(character) => format!("character:{character}"),
        }
    }
}

impl AcceptedNominalId {
    /// Creates an accepted nominal identity from typed owner and path facts.
    pub const fn new(owner: AcceptedNominalOwnerId, canonical_path: TypePath) -> Self {
        Self {
            owner,
            canonical_path,
        }
    }

    /// Accepted environment owner of this identity.
    pub const fn owner(&self) -> &AcceptedNominalOwnerId {
        &self.owner
    }

    /// Canonical typed source path of this identity.
    pub const fn canonical_path(&self) -> &TypePath {
        &self.canonical_path
    }

    /// Deterministic presentation label; never parse this back into identity.
    pub fn source_label(&self) -> String {
        format!(
            "{}::{}",
            self.owner.source_label(),
            self.canonical_path.canonical_string()
        )
    }
}

impl AcceptedNominalRecord {
    /// Validates and creates one exact accepted nominal record.
    pub fn try_new(
        id: AcceptedNominalId,
        arity: u16,
        semantics: AcceptedNominalSemantics,
        origin: AcceptedNominalOrigin,
        source: Option<SourceSpan>,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        if arity > MAX_NOMINAL_ARITY
            || (arity != 0
                && matches!(
                    semantics,
                    AcceptedNominalSemantics::Exact(_) | AcceptedNominalSemantics::Character(_)
                ))
        {
            return Err(AcceptedNominalCatalogError::InvalidArity {
                source_span: source,
                minimum: arity,
                maximum: arity,
            });
        }
        if is_reserved_path(id.canonical_path()) {
            return Err(AcceptedNominalCatalogError::ReservedPath {
                path: id.canonical_path().clone(),
            });
        }
        Ok(Self {
            id,
            arity,
            semantics,
            origin,
            source,
        })
    }

    /// Validates and creates one producer-bearing opaque accepted record.
    pub fn try_new_opaque(
        id: AcceptedNominalId,
        arity: u16,
        producer: RuntimeOpaqueTypeProducerId,
        origin: AcceptedNominalOrigin,
        source: Option<SourceSpan>,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        Self::try_new(
            id,
            arity,
            AcceptedNominalSemantics::Opaque { producer },
            origin,
            source,
        )
    }

    pub const fn id(&self) -> &AcceptedNominalId {
        &self.id
    }

    pub const fn arity(&self) -> u16 {
        self.arity
    }

    pub const fn semantics(&self) -> &AcceptedNominalSemantics {
        &self.semantics
    }

    pub const fn origin(&self) -> AcceptedNominalOrigin {
        self.origin
    }

    pub const fn source(&self) -> Option<&SourceSpan> {
        self.source.as_ref()
    }

    /// Instantiates this exact record without performing another name lookup.
    pub(crate) fn try_instantiate(
        &self,
        arguments: impl Into<Box<[TypeKind]>>,
    ) -> Result<TypeKind, AcceptedNominalInstantiationError> {
        let arguments = arguments.into();
        if arguments.len() != usize::from(self.arity) {
            return Err(AcceptedNominalInstantiationError::WrongArity {
                id: self.id.source_label(),
                expected: self.arity,
                actual: arguments.len(),
            });
        }
        match &self.semantics {
            AcceptedNominalSemantics::Exact(ty) if arguments.is_empty() => Ok(ty.clone()),
            AcceptedNominalSemantics::Opaque { producer } => Ok(TypeKind::AcceptedNominal(
                crate::types::AcceptedNominalType::new(
                    self.id.clone(),
                    arguments,
                    producer.clone(),
                ),
            )),
            AcceptedNominalSemantics::Character(character) if arguments.is_empty() => {
                Ok(TypeKind::CharacterNominal(character.clone()))
            }
            AcceptedNominalSemantics::Exact(_) | AcceptedNominalSemantics::Character(_) => {
                Err(AcceptedNominalInstantiationError::InvalidSemantics {
                    id: self.id.source_label(),
                })
            }
        }
    }
}

impl OpenNominalRuleId {
    /// Creates a stable rule identity within one environment owner.
    pub const fn new(owner: EnvironmentBindingId, ordinal: u32) -> Self {
        Self { owner, ordinal }
    }

    pub const fn owner(&self) -> &EnvironmentBindingId {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl OpenNominalArity {
    pub const fn minimum(self) -> u16 {
        match self {
            Self::Exact(arity) => arity,
            Self::Inclusive { minimum, .. } => minimum,
        }
    }

    pub const fn maximum(self) -> u16 {
        match self {
            Self::Exact(arity) => arity,
            Self::Inclusive { maximum, .. } => maximum,
        }
    }

    pub const fn contains(self, arity: u16) -> bool {
        self.minimum() <= arity && arity <= self.maximum()
    }
}

impl OpenNominalRule {
    /// Validates and creates one explicit bounded open-name rule.
    pub fn try_new(
        id: OpenNominalRuleId,
        scope: OpenNominalScope,
        pattern: OpenNominalPattern,
        arity: OpenNominalArity,
        source: Option<SourceSpan>,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        validate_open_pattern(&id, &pattern)?;
        let minimum = arity.minimum();
        let maximum = arity.maximum();
        if minimum > maximum || maximum > MAX_NOMINAL_ARITY {
            return Err(AcceptedNominalCatalogError::InvalidArity {
                source_span: source,
                minimum,
                maximum,
            });
        }
        Ok(Self {
            id,
            scope,
            pattern,
            arity,
            source,
        })
    }

    pub const fn id(&self) -> &OpenNominalRuleId {
        &self.id
    }

    pub const fn scope(&self) -> &OpenNominalScope {
        &self.scope
    }

    pub const fn pattern(&self) -> &OpenNominalPattern {
        &self.pattern
    }

    pub const fn arity(&self) -> OpenNominalArity {
        self.arity
    }

    pub const fn source(&self) -> Option<&SourceSpan> {
        self.source.as_ref()
    }

    /// Whether this rule accepts the supplied typed path and arity in context.
    pub fn matches(
        &self,
        environment: OpenNominalEnvironment,
        current_module: Option<&CanonicalModulePath>,
        path: &TypePath,
        arity: u16,
    ) -> bool {
        scope_matches(&self.scope, environment, current_module)
            && pattern_matches(&self.pattern, path)
            && self.arity.contains(arity)
    }
}

impl AcceptedNominalCatalogDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Default for AcceptedNominalCatalog {
    fn default() -> Self {
        Self::try_new(
            std::iter::empty(),
            std::iter::empty(),
            AcceptedNominalCatalogLimits::PRODUCTION,
        )
        .expect("an empty accepted nominal catalog is valid")
    }
}

impl AcceptedNominalCatalog {
    /// Atomically validates and freezes exact records and open-name rules.
    pub fn try_new(
        exact: impl IntoIterator<Item = AcceptedNominalRecord>,
        open: impl IntoIterator<Item = OpenNominalRule>,
        limits: AcceptedNominalCatalogLimits,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        let mut exact = exact.into_iter().collect::<Vec<_>>();
        exact.sort_by(|left, right| {
            left.id
                .canonical_path
                .cmp(&right.id.canonical_path)
                .then_with(|| left.id.cmp(&right.id))
        });
        enforce_capacity(
            AcceptedNominalCatalogLimitKind::ExactRecords,
            exact.len(),
            limits.exact_records(),
        )?;
        let mut exact_by_path = BTreeMap::<TypePath, AcceptedNominalRecord>::new();
        for record in exact {
            if let Some(first) = exact_by_path.get(record.id.canonical_path()) {
                return Err(AcceptedNominalCatalogError::DuplicateExactPath {
                    path: record.id.canonical_path().clone(),
                    first: Box::new(first.id().clone()),
                    duplicate: Box::new(record.id().clone()),
                });
            }
            exact_by_path.insert(record.id.canonical_path().clone(), record);
        }

        let mut open = open.into_iter().collect::<Vec<_>>();
        open.sort_by(|left, right| left.id.cmp(&right.id));
        enforce_capacity(
            AcceptedNominalCatalogLimitKind::OpenRules,
            open.len(),
            limits.open_rules(),
        )?;
        for (index, first) in open.iter().enumerate() {
            validate_open_pattern(first.id(), first.pattern())?;
            if first.arity.minimum() > first.arity.maximum()
                || first.arity.maximum() > MAX_NOMINAL_ARITY
            {
                return Err(AcceptedNominalCatalogError::InvalidArity {
                    source_span: first.source.clone(),
                    minimum: first.arity.minimum(),
                    maximum: first.arity.maximum(),
                });
            }
            if let Some(second) = open[index + 1..]
                .iter()
                .find(|second| rules_overlap(first, second))
            {
                return Err(AcceptedNominalCatalogError::OverlappingOpenRules {
                    first: first.id.clone(),
                    second: second.id.clone(),
                });
            }
        }
        let open_by_id = open
            .into_iter()
            .map(|rule| (rule.id.clone(), rule))
            .collect::<BTreeMap<_, _>>();
        let digest = catalog_digest(&exact_by_path, &open_by_id);
        Ok(Self {
            exact: exact_by_path,
            open: open_by_id,
            digest,
        })
    }

    /// Atomically returns a catalog containing one additional exact record.
    pub fn try_with_record(
        &self,
        record: AcceptedNominalRecord,
        limits: AcceptedNominalCatalogLimits,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        Self::try_new(
            self.exact.values().cloned().chain([record]),
            self.open.values().cloned(),
            limits,
        )
    }

    /// Atomically returns a catalog containing one additional open rule.
    pub fn try_with_open_rule(
        &self,
        rule: OpenNominalRule,
        limits: AcceptedNominalCatalogLimits,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        Self::try_new(
            self.exact.values().cloned(),
            self.open.values().cloned().chain([rule]),
            limits,
        )
    }

    /// Exact accepted record for a typed canonical path.
    pub fn exact(&self, path: &TypePath) -> Option<&AcceptedNominalRecord> {
        self.exact.get(path)
    }

    /// Open rule by stable typed identity.
    pub fn open_rule(&self, id: &OpenNominalRuleId) -> Option<&OpenNominalRule> {
        self.open.get(id)
    }

    /// Deterministically ordered exact records.
    pub fn exact_records(
        &self,
    ) -> impl ExactSizeIterator<Item = &AcceptedNominalRecord> + DoubleEndedIterator {
        self.exact.values()
    }

    /// Deterministically ordered exact records contributed by one typed owner.
    pub fn exact_records_for_owner<'a>(
        &'a self,
        owner: &'a AcceptedNominalOwnerId,
    ) -> impl Iterator<Item = &'a AcceptedNominalRecord> {
        self.exact
            .values()
            .filter(move |record| record.id.owner() == owner)
    }

    /// Deterministically ordered open-name rules.
    pub fn open_rules(
        &self,
    ) -> impl ExactSizeIterator<Item = &OpenNominalRule> + DoubleEndedIterator {
        self.open.values()
    }

    /// Selects the sole explicitly matching open rule, if one exists.
    pub fn matching_open_rule(
        &self,
        environment: OpenNominalEnvironment,
        current_module: Option<&CanonicalModulePath>,
        path: &TypePath,
        arity: u16,
    ) -> Option<&OpenNominalRule> {
        self.open
            .values()
            .find(|rule| rule.matches(environment, current_module, path, arity))
    }

    /// Verifies that world-only rule scopes are legal for an environment kind.
    pub fn validate_scopes_for(
        &self,
        environment: OpenNominalEnvironment,
    ) -> Result<(), AcceptedNominalCatalogError> {
        if let Some(rule) = self.open.values().find(|rule| {
            matches!(
                (&rule.scope, environment),
                (
                    OpenNominalScope::AcceptedWorld,
                    OpenNominalEnvironment::Detached
                ) | (
                    OpenNominalScope::DetachedOnly,
                    OpenNominalEnvironment::Accepted
                )
            )
        }) {
            return Err(AcceptedNominalCatalogError::InvalidScope {
                rule: rule.id.clone(),
                scope: rule.scope.clone(),
            });
        }
        Ok(())
    }

    pub const fn digest(&self) -> AcceptedNominalCatalogDigest {
        self.digest
    }
}

impl TypeCheckEnv {
    /// Publishes the standard exact domain atoms used by source annotations.
    pub(super) fn with_standard_accepted_nominals(self) -> Self {
        [
            ("DataFormat", TypeKind::DataFormat),
            ("DataShape", TypeKind::DataShape),
            ("AgentValue", TypeKind::AgentValue),
            (
                "ObservedObjectId",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::ObservedObjectId),
            ),
            (
                "CaptureFormat",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureFormat),
            ),
            (
                "CaptureKind",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureKind),
            ),
            (
                "Diagnostics",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::Diagnostics),
            ),
            (
                "WaitError",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::WaitError),
            ),
            (
                "ViewportPoint",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::ViewportPoint),
            ),
            (
                "PointerButton",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::PointerButton),
            ),
            (
                "RagError",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::RagError),
            ),
            ("TextCluster", TypeKind::TextCluster),
            ("Duration", TypeKind::Duration),
            ("DebugStatePath", TypeKind::DebugStatePath),
            ("ObservationFieldPath", TypeKind::ObservationFieldPath),
            ("DisplayText", TypeKind::DisplayText),
        ]
        .into_iter()
        .fold(self, |environment, (name, semantics)| {
            environment
                .try_with_nominal_record(
                    standard_exact_record(name, semantics, AcceptedNominalOrigin::Domain)
                        .expect("standard domain atoms have valid exact typed identities"),
                )
                .expect("standard domain atoms have distinct non-reserved paths")
        })
        .with_standard_opaque_nominals()
    }

    fn with_standard_opaque_nominals(self) -> Self {
        STANDARD_OPAQUE_NOMINALS
            .into_iter()
            .fold(self, |environment, spec| {
                environment
                    .try_with_nominal_record(
                        standard_opaque_record(spec, AcceptedNominalOrigin::Domain)
                            .expect("standard opaque atom has valid typed evidence"),
                    )
                    .expect("standard opaque atoms have distinct non-reserved paths")
            })
    }

    /// Atomically registers one exact source-visible accepted nominal record.
    pub fn try_with_nominal_record(
        mut self,
        record: AcceptedNominalRecord,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        self.try_insert_nominal_record(record)?;
        Ok(self)
    }

    pub(crate) fn try_insert_nominal_record(
        &mut self,
        record: AcceptedNominalRecord,
    ) -> Result<(), AcceptedNominalCatalogError> {
        self.nominal_catalog = self
            .nominal_catalog
            .try_with_record(record, AcceptedNominalCatalogLimits::PRODUCTION)?;
        Ok(())
    }

    /// Atomically registers one explicitly bounded open nominal rule.
    pub fn try_with_open_nominal_rule(
        mut self,
        rule: OpenNominalRule,
    ) -> Result<Self, AcceptedNominalCatalogError> {
        self.nominal_catalog = self
            .nominal_catalog
            .try_with_open_rule(rule, AcceptedNominalCatalogLimits::PRODUCTION)?;
        Ok(self)
    }

    /// Exact accepted/open nominal facts visible to authored type resolution.
    pub const fn nominal_catalog(&self) -> &AcceptedNominalCatalog {
        &self.nominal_catalog
    }
}

pub(super) fn standard_exact_record(
    name: &str,
    semantics: TypeKind,
    origin: AcceptedNominalOrigin,
) -> Result<AcceptedNominalRecord, AcceptedNominalCatalogError> {
    let segment = ProjectSymbolSegment::try_new(name.to_owned())
        .expect("environment-owned type names are validated project-symbol segments");
    let path = ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, [segment])
        .expect("one validated segment is a valid project-symbol path")
        .into();
    AcceptedNominalRecord::try_new(
        AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path),
        0,
        AcceptedNominalSemantics::Exact(semantics),
        origin,
        None,
    )
}

fn standard_opaque_record(
    spec: StandardOpaqueNominalSpec,
    origin: AcceptedNominalOrigin,
) -> Result<AcceptedNominalRecord, AcceptedNominalCatalogError> {
    let segment = ProjectSymbolSegment::try_new(spec.name.to_owned())
        .expect("environment-owned type names are validated project-symbol segments");
    let path = ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, [segment])
        .expect("one validated segment is a valid project-symbol path")
        .into();
    AcceptedNominalRecord::try_new_opaque(
        AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path),
        spec.arity,
        RuntimeOpaqueTypeProducerId::try_new(spec.producer)
            .expect("fixed standard opaque producer IDs are valid"),
        origin,
        None,
    )
}

pub(crate) fn standard_agent_error_type() -> TypeKind {
    standard_opaque_record(STANDARD_AGENT_ERROR, AcceptedNominalOrigin::Domain)
        .expect("AgentError has valid fixed standard opaque evidence")
        .try_instantiate([])
        .expect("AgentError is a zero-argument standard opaque nominal")
}

fn validate_open_pattern(
    rule: &OpenNominalRuleId,
    pattern: &OpenNominalPattern,
) -> Result<(), AcceptedNominalCatalogError> {
    let invalid = match pattern {
        OpenNominalPattern::Exact(path) => {
            is_reserved_path(path).then_some(OpenNominalPatternError::ReservedPath)
        }
        OpenNominalPattern::Namespace {
            prefix,
            min_tail_segments,
            max_tail_segments,
        } => {
            if prefix.segments().is_empty() {
                Some(OpenNominalPatternError::EmptyNamespacePrefix)
            } else if is_reserved_path(prefix) {
                Some(OpenNominalPatternError::ReservedPath)
            } else if *min_tail_segments == 0 {
                Some(OpenNominalPatternError::ZeroTail)
            } else if min_tail_segments > max_tail_segments {
                Some(OpenNominalPatternError::InvertedTailRange)
            } else if *max_tail_segments > MAX_OPEN_NAMESPACE_TAIL {
                Some(OpenNominalPatternError::TailMaximumExceeded {
                    maximum: *max_tail_segments,
                    allowed: MAX_OPEN_NAMESPACE_TAIL,
                })
            } else {
                None
            }
        }
    };
    match invalid {
        Some(reason) => Err(AcceptedNominalCatalogError::InvalidOpenPattern {
            rule: rule.clone(),
            reason,
        }),
        None => Ok(()),
    }
}

fn is_reserved_path(path: &TypePath) -> bool {
    if path.root() != ModulePathRoot::ImplicitCrate {
        return false;
    }
    let [segment] = path.segments() else {
        return false;
    };
    matches!(
        segment.as_str(),
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "String"
            | "char"
            | "Bytes"
            | "Unit"
            | "Never"
            | "Vec"
            | "Slice"
            | "Seq"
            | "Option"
            | "Probe"
            | "ThreadHandle"
            | "Shared"
            | "Array"
            | "OrderedMap"
            | "SortedMap"
            | "BTreeMap"
            | "Result"
            | "Need"
            | "Stream"
            | "Ref"
    )
}

fn scope_matches(
    scope: &OpenNominalScope,
    environment: OpenNominalEnvironment,
    current_module: Option<&CanonicalModulePath>,
) -> bool {
    match scope {
        OpenNominalScope::AcceptedWorld => environment == OpenNominalEnvironment::Accepted,
        OpenNominalScope::DetachedOnly => environment == OpenNominalEnvironment::Detached,
        OpenNominalScope::ExactModule(module) => current_module == Some(module),
        OpenNominalScope::ModuleSubtree(root) => {
            current_module.is_some_and(|module| module_has_prefix(module, root))
        }
    }
}

fn pattern_matches(pattern: &OpenNominalPattern, path: &TypePath) -> bool {
    match pattern {
        OpenNominalPattern::Exact(expected) => expected == path,
        OpenNominalPattern::Namespace {
            prefix,
            min_tail_segments,
            max_tail_segments,
        } => path_tail_length(prefix, path).is_some_and(|tail| {
            usize::from(*min_tail_segments) <= tail && tail <= usize::from(*max_tail_segments)
        }),
    }
}

fn rules_overlap(first: &OpenNominalRule, second: &OpenNominalRule) -> bool {
    first.id == second.id
        || (scopes_overlap(&first.scope, &second.scope)
            && patterns_overlap(&first.pattern, &second.pattern)
            && intervals_overlap(
                first.arity.minimum(),
                first.arity.maximum(),
                second.arity.minimum(),
                second.arity.maximum(),
            ))
}

fn scopes_overlap(first: &OpenNominalScope, second: &OpenNominalScope) -> bool {
    match (first, second) {
        (OpenNominalScope::AcceptedWorld, OpenNominalScope::DetachedOnly)
        | (OpenNominalScope::DetachedOnly, OpenNominalScope::AcceptedWorld) => false,
        (OpenNominalScope::ExactModule(left), OpenNominalScope::ExactModule(right)) => {
            left == right
        }
        (OpenNominalScope::ExactModule(module), OpenNominalScope::ModuleSubtree(root))
        | (OpenNominalScope::ModuleSubtree(root), OpenNominalScope::ExactModule(module)) => {
            module_has_prefix(module, root)
        }
        (OpenNominalScope::ModuleSubtree(left), OpenNominalScope::ModuleSubtree(right)) => {
            module_has_prefix(left, right) || module_has_prefix(right, left)
        }
        _ => true,
    }
}

fn patterns_overlap(first: &OpenNominalPattern, second: &OpenNominalPattern) -> bool {
    match (first, second) {
        (OpenNominalPattern::Exact(left), OpenNominalPattern::Exact(right)) => left == right,
        (OpenNominalPattern::Exact(path), OpenNominalPattern::Namespace { .. }) => {
            pattern_matches(second, path)
        }
        (OpenNominalPattern::Namespace { .. }, OpenNominalPattern::Exact(path)) => {
            pattern_matches(first, path)
        }
        (
            OpenNominalPattern::Namespace {
                prefix: left,
                min_tail_segments: left_min,
                max_tail_segments: left_max,
            },
            OpenNominalPattern::Namespace {
                prefix: right,
                min_tail_segments: right_min,
                max_tail_segments: right_max,
            },
        ) => {
            if let Some(delta) = path_tail_length(left, right) {
                intervals_overlap_usize(
                    usize::from(*left_min),
                    usize::from(*left_max),
                    delta + usize::from(*right_min),
                    delta + usize::from(*right_max),
                )
            } else if let Some(delta) = path_tail_length(right, left) {
                intervals_overlap_usize(
                    delta + usize::from(*left_min),
                    delta + usize::from(*left_max),
                    usize::from(*right_min),
                    usize::from(*right_max),
                )
            } else {
                false
            }
        }
    }
}

fn path_tail_length(prefix: &TypePath, path: &TypePath) -> Option<usize> {
    (prefix.root() == path.root() && segments_have_prefix(path.segments(), prefix.segments()))
        .then(|| path.segments().len() - prefix.segments().len())
}

fn segments_have_prefix(path: &[ProjectSymbolSegment], prefix: &[ProjectSymbolSegment]) -> bool {
    path.get(..prefix.len()) == Some(prefix)
}

fn module_has_prefix(module: &CanonicalModulePath, prefix: &CanonicalModulePath) -> bool {
    module.segments().get(..prefix.segments().len()) == Some(prefix.segments())
}

const fn intervals_overlap(
    first_min: u16,
    first_max: u16,
    second_min: u16,
    second_max: u16,
) -> bool {
    first_min <= second_max && second_min <= first_max
}

const fn intervals_overlap_usize(
    first_min: usize,
    first_max: usize,
    second_min: usize,
    second_max: usize,
) -> bool {
    first_min <= second_max && second_min <= first_max
}

fn enforce_capacity(
    kind: AcceptedNominalCatalogLimitKind,
    observed: usize,
    maximum: u16,
) -> Result<(), AcceptedNominalCatalogError> {
    if observed > usize::from(maximum) {
        return Err(AcceptedNominalCatalogError::Limit {
            kind,
            observed: u64::try_from(observed).unwrap_or(u64::MAX),
            maximum: u64::from(maximum),
        });
    }
    Ok(())
}

fn catalog_digest(
    exact: &BTreeMap<TypePath, AcceptedNominalRecord>,
    open: &BTreeMap<OpenNominalRuleId, OpenNominalRule>,
) -> AcceptedNominalCatalogDigest {
    let mut hasher = CatalogDigestHasher::new();
    exact.len().hash(&mut hasher);
    for (path, record) in exact {
        path.hash(&mut hasher);
        record.id.hash(&mut hasher);
        record.arity.hash(&mut hasher);
        record.semantics.hash(&mut hasher);
        record.origin.hash(&mut hasher);
    }
    open.len().hash(&mut hasher);
    for (id, rule) in open {
        id.hash(&mut hasher);
        rule.scope.hash(&mut hasher);
        rule.pattern.hash(&mut hasher);
        rule.arity.hash(&mut hasher);
    }
    AcceptedNominalCatalogDigest(hasher.finalize())
}

struct CatalogDigestHasher(blake3::Hasher);

impl CatalogDigestHasher {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.accepted-nominal-catalog.v1\0");
        Self(hasher)
    }

    fn finalize(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

impl Hasher for CatalogDigestHasher {
    fn finish(&self) -> u64 {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(&self.0.clone().finalize().as_bytes()[..8]);
        u64::from_le_bytes(bytes)
    }

    fn write(&mut self, bytes: &[u8]) {
        let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        self.0.update(&length.to_le_bytes());
        self.0.update(bytes);
    }

    fn write_u8(&mut self, value: u8) {
        self.write(&value.to_le_bytes());
    }

    fn write_u16(&mut self, value: u16) {
        self.write(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.write(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn write_u128(&mut self, value: u128) {
        self.write(&value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(u64::try_from(value).unwrap_or(u64::MAX));
    }

    fn write_i8(&mut self, value: i8) {
        self.write(&value.to_le_bytes());
    }

    fn write_i16(&mut self, value: i16) {
        self.write(&value.to_le_bytes());
    }

    fn write_i32(&mut self, value: i32) {
        self.write(&value.to_le_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.write(&value.to_le_bytes());
    }

    fn write_i128(&mut self, value: i128) {
        self.write(&value.to_le_bytes());
    }

    fn write_isize(&mut self, value: isize) {
        self.write_i64(i64::try_from(value).unwrap_or_else(|_| {
            if value.is_negative() {
                i64::MIN
            } else {
                i64::MAX
            }
        }));
    }
}
