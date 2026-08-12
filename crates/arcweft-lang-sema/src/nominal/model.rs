//! Immutable products emitted by nominal type resolution.

use arcweft_lang_hir::{
    identity::TypeId,
    leaf::{HirPath, HirPathRoot, HirPathSegment},
    symbol::{ExternalDeclarationId, ProjectTypeCandidate, nominal::ProjectNominalDeclarationId},
};
use arcweft_source::{SourceRange, SourceSpan};
use thiserror::Error;

use crate::{
    env::nominal::{AcceptedNominalId, OpenNominalRuleId},
    types::{
        AcceptedNominalType, CharacterNominalType, EntityKind, GenericTypeParameterId,
        ProjectNominalType, TypeKind, TypePoisonId,
    },
};

use super::{NominalTypeDiagnostic, TypePoisonRecord};

/// Exact local source range and optional accepted-project span for one type node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeSourceEvidence {
    local: SourceRange,
    project: Option<SourceSpan>,
}

/// Closed language-owned type constructor set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinTypeConstructor {
    Bool,
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
    F32,
    F64,
    String,
    Char,
    Bytes,
    Unit,
    Never,
    CharacterDialogue,
    Vec,
    Slice,
    Seq,
    Option,
    Probe,
    ThreadHandle,
    Shared,
    Array,
    OrderedMap,
    SortedMap,
    BTreeMap,
    Result,
    Need,
    Stream,
    Source,
    Ref,
}

/// Resolution of a project external through the accepted environment owner map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalNominalResolution {
    Accepted {
        external: ExternalDeclarationId,
        nominal: AcceptedNominalType,
    },
    Exact {
        external: ExternalDeclarationId,
        ty: TypeKind,
        accepted: AcceptedNominalId,
    },
    Character {
        external: ExternalDeclarationId,
        nominal: CharacterNominalType,
        accepted: AcceptedNominalId,
    },
}

/// One authored path admitted by an explicit open-nominal rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOpenNominal {
    rule: OpenNominalRuleId,
    path: HirPath,
    arguments: Box<[TypeKind]>,
}

/// One resolved use of a project alias, retaining both identity and normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAliasReference {
    declaration: ProjectNominalDeclarationId,
    arguments: Box<[TypeKind]>,
    normalized: TypeKind,
    use_source: TypeSourceEvidence,
    declaration_source: SourceSpan,
    target_source: TypeSourceEvidence,
}

/// Typed resolution fact for one authored structural type node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeNameResolution {
    Structural(StructuralTypeNodeKind),
    Builtin(BuiltinTypeConstructor),
    EntityFamily(EntityKind),
    Generic(GenericTypeParameterId),
    SelfType(TypeKind),
    TraitHead(HirPath),
    Projection,
    Project(ProjectNominalType),
    Alias(ResolvedAliasReference),
    External(ExternalNominalResolution),
    Accepted(AcceptedNominalType),
    AcceptedExact {
        accepted: AcceptedNominalId,
        ty: TypeKind,
    },
    Open(ResolvedOpenNominal),
    Failed(TypeResolutionFailure),
    Poisoned(TypePoisonId),
    DetachedUnavailable(DetachedNominalEvidence),
}

/// Structural node families that do not perform nominal-name selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuralTypeNodeKind {
    ConstInt,
    Tuple,
    Function,
    Choice,
    Reference,
    Slice,
}

/// Resolution fact tied to its exact structural address and source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeNode {
    node: TypeId,
    source: TypeSourceEvidence,
    terminal_source: Option<TypeSourceEvidence>,
    reference_path: Option<HirPath>,
    recovered: Option<TypeKind>,
    outcome: TypeNameResolution,
}

/// One deterministic project-alias expansion step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasExpansionFact {
    alias: ProjectNominalDeclarationId,
    arguments: Box<[TypeKind]>,
    substitution: Box<[(GenericTypeParameterId, TypeKind)]>,
    normalized: TypeKind,
    use_source: TypeSourceEvidence,
    declaration_source: SourceSpan,
    target_source: TypeSourceEvidence,
}

/// Recovered semantic type together with every node and alias fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeProduct {
    root: TypeId,
    recovered: TypeKind,
    nodes: Box<[ResolvedTypeNode]>,
    aliases: Box<[AliasExpansionFact]>,
}

/// Accepted-world result containing authoritative poison causes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoisonedTypeRef {
    product: ResolvedTypeProduct,
    causes: Box<[TypePoisonId]>,
}

/// Detached result that records every node for which project proof was unavailable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetachedTypeRef {
    product: ResolvedTypeProduct,
    unavailable: Box<[TypeId]>,
    causes: Box<[TypePoisonId]>,
}

/// Complete, poisoned, or deliberately non-authoritative resolution outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedTypeRefOutcome {
    Complete(ResolvedTypeProduct),
    Poisoned(PoisonedTypeRef),
    Detached(DetachedTypeRef),
}

/// Immutable result of the one public nominal-resolution operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeResolutionReport {
    outcome: ResolvedTypeRefOutcome,
    diagnostics: Box<[NominalTypeDiagnostic]>,
    poisons: Box<[TypePoisonRecord]>,
    omitted_diagnostics: u64,
    work_charged: u64,
}

/// Borrowed, validated type receiver for one associated callable lookup.
///
/// This projection retains the complete nominal product so aliases, generic
/// identities, and declaration evidence remain available to later tooling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedAssociatedTypeReceiver<'a> {
    product: &'a ResolvedTypeProduct,
    root: &'a ResolvedTypeNode,
    ty: &'a TypeKind,
}

/// Typed reason a nominal product cannot act as an associated-call receiver.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum AssociatedReceiverFailure {
    #[error("poisoned nominal resolution cannot produce an associated type receiver")]
    PoisonedOutcome,
    #[error("detached nominal resolution lacks authoritative receiver evidence")]
    DetachedOutcome,
    #[error("resolved type product is missing its root node")]
    MissingRoot,
    #[error("resolved type node {node:?} is incomplete")]
    IncompleteNode { node: TypeId },
    #[error("resolved type root does not contain a type value")]
    MissingRootType,
    #[error("resolved type root disagrees with the product's normalized type")]
    RootTypeMismatch,
}

/// Type constructor whose authored argument count is checked.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeArityTarget {
    Builtin(BuiltinTypeConstructor),
    Project(ProjectNominalDeclarationId),
    Accepted(AcceptedNominalId),
    Open(OpenNominalRuleId),
}

/// Exact or inclusive valid authored arity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeArityExpectation {
    Exact(u16),
    Inclusive { minimum: u16, maximum: u16 },
}

/// Semantic shape required for one constructor argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeArgumentExpectation {
    Type,
    ConstInt,
    EntityFamily,
}

/// Semantic category supplied to one final-HIR type-constructor argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeArgumentKind {
    Type(TypeKind),
    ConstInt(usize),
    EntityFamily(EntityKind),
}

/// Typed evidence retained when a detached world cannot prove a project name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetachedNominalEvidence {
    path: HirPath,
    source: TypeSourceEvidence,
    reason: DetachedNominalReason,
}

/// Missing accepted-world component that prevented authoritative resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DetachedNominalReason {
    ProjectWorldUnavailable,
    ModuleUnavailable,
}

/// Typed reason that one nominal node failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeResolutionFailure {
    Unknown {
        path: HirPath,
    },
    Ambiguous {
        path: HirPath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    Inaccessible {
        path: HirPath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    WrongKind {
        path: HirPath,
        actual: ProjectTypeCandidate,
    },
    WrongArgumentKind {
        target: TypeArityTarget,
        argument: u16,
        expected: TypeArgumentExpectation,
        actual: TypeArgumentKind,
    },
    WrongArity {
        target: TypeArityTarget,
        expected: TypeArityExpectation,
        actual: u16,
    },
    CyclicAlias {
        cycle: Box<[ProjectNominalDeclarationId]>,
    },
    SelfUnavailable,
    Limit {
        kind: super::NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
    WorkOverflow {
        attempted: u64,
        maximum: u64,
    },
}

impl TypeSourceEvidence {
    /// Creates evidence for an accepted project type node.
    pub const fn accepted(local: SourceRange, project: SourceSpan) -> Self {
        Self {
            local,
            project: Some(project),
        }
    }

    /// Creates local-only evidence without fabricating project identity.
    pub const fn detached(local: SourceRange) -> Self {
        Self {
            local,
            project: None,
        }
    }

    pub const fn local(&self) -> SourceRange {
        self.local
    }

    pub const fn project(&self) -> Option<&SourceSpan> {
        self.project.as_ref()
    }
}

impl BuiltinTypeConstructor {
    /// Complete deterministic language-owned constructor inventory.
    pub const ALL: &'static [Self] = &[
        Self::Bool,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::ISize,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::USize,
        Self::F32,
        Self::F64,
        Self::String,
        Self::Char,
        Self::Bytes,
        Self::Unit,
        Self::Never,
        Self::CharacterDialogue,
        Self::Vec,
        Self::Slice,
        Self::Seq,
        Self::Option,
        Self::Probe,
        Self::ThreadHandle,
        Self::Shared,
        Self::Array,
        Self::OrderedMap,
        Self::SortedMap,
        Self::BTreeMap,
        Self::Result,
        Self::Need,
        Self::Stream,
        Self::Source,
        Self::Ref,
    ];

    /// Constructors whose sole argument is a contextual entity-family atom.
    pub const ENTITY_FAMILY_PROJECTIONS: &'static [Self] = &[Self::Ref];

    /// Canonical reserved source spelling.
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::ISize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::USize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::String => "String",
            Self::Char => "char",
            Self::Bytes => "Bytes",
            Self::Unit => "Unit",
            Self::Never => "Never",
            Self::CharacterDialogue => "CharacterDialogue",
            Self::Vec => "Vec",
            Self::Slice => "Slice",
            Self::Seq => "Seq",
            Self::Option => "Option",
            Self::Probe => "Probe",
            Self::ThreadHandle => "ThreadHandle",
            Self::Shared => "Shared",
            Self::Array => "Array",
            Self::OrderedMap => "OrderedMap",
            Self::SortedMap => "SortedMap",
            Self::BTreeMap => "BTreeMap",
            Self::Result => "Result",
            Self::Need => "Need",
            Self::Stream => "Stream",
            Self::Source => "Source",
            Self::Ref => "Ref",
        }
    }

    /// Contractual constructor arity.
    pub const fn arity(self) -> u16 {
        match self {
            Self::Bool
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64
            | Self::I128
            | Self::ISize
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::U128
            | Self::USize
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::Unit
            | Self::Never
            | Self::CharacterDialogue => 0,
            Self::Vec
            | Self::Slice
            | Self::Seq
            | Self::Option
            | Self::Probe
            | Self::ThreadHandle
            | Self::Shared
            | Self::Ref => 1,
            Self::Array
            | Self::OrderedMap
            | Self::SortedMap
            | Self::BTreeMap
            | Self::Result
            | Self::Need
            | Self::Stream
            | Self::Source => 2,
        }
    }

    /// Selects one unqualified language-owned constructor from a final HIR path.
    #[must_use]
    pub fn from_hir_path(path: &HirPath) -> Option<Self> {
        if path.root() != HirPathRoot::ImplicitCrate {
            return None;
        }
        let [segment] = path.segments() else {
            return None;
        };
        let spelling = match segment {
            HirPathSegment::Identifier(name) => name.as_str(),
            HirPathSegment::ProjectSymbol(name) => name.as_str(),
        };
        Self::ALL
            .iter()
            .copied()
            .find(|constructor| constructor.spelling() == spelling)
    }

    /// Expected semantic category for an in-range constructor argument.
    #[must_use]
    pub const fn argument_expectation(self, index: u16) -> Option<TypeArgumentExpectation> {
        if index >= self.arity() {
            return None;
        }
        Some(match (self, index) {
            (Self::Array, 1) => TypeArgumentExpectation::ConstInt,
            (Self::Ref, 0) => TypeArgumentExpectation::EntityFamily,
            _ => TypeArgumentExpectation::Type,
        })
    }

    /// Projects a contextual entity-family atom through this closed constructor.
    #[must_use]
    pub fn project_entity_family(self, family: EntityKind) -> Option<TypeKind> {
        Some(match self {
            Self::Ref => TypeKind::entity_ref(family),
            _ => return None,
        })
    }
}

impl TypeArgumentKind {
    pub(crate) fn stable_ordering(&self, other: &Self) -> core::cmp::Ordering {
        fn rank(value: &TypeArgumentKind) -> u8 {
            match value {
                TypeArgumentKind::Type(_) => 0,
                TypeArgumentKind::ConstInt(_) => 1,
                TypeArgumentKind::EntityFamily(_) => 2,
            }
        }

        rank(self)
            .cmp(&rank(other))
            .then_with(|| match (self, other) {
                (Self::Type(left), Self::Type(right)) => left.stable_ordering(right),
                (Self::ConstInt(left), Self::ConstInt(right)) => left.cmp(right),
                (Self::EntityFamily(left), Self::EntityFamily(right)) => {
                    entity_family_ordering(left, right)
                }
                _ => core::cmp::Ordering::Equal,
            })
    }
}

fn entity_family_ordering(left: &EntityKind, right: &EntityKind) -> core::cmp::Ordering {
    match (left.authored_type_name(), right.authored_type_name()) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => core::cmp::Ordering::Less,
        (None, Some(_)) => core::cmp::Ordering::Greater,
        (None, None) => match (left, right) {
            (EntityKind::Other(left), EntityKind::Other(right)) => left.cmp(right),
            _ => core::cmp::Ordering::Equal,
        },
    }
}

impl ResolvedOpenNominal {
    pub(crate) fn new(
        rule: OpenNominalRuleId,
        path: HirPath,
        arguments: impl Into<Box<[TypeKind]>>,
    ) -> Self {
        Self {
            rule,
            path,
            arguments: arguments.into(),
        }
    }

    pub const fn rule(&self) -> &OpenNominalRuleId {
        &self.rule
    }

    pub const fn path(&self) -> &HirPath {
        &self.path
    }

    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }
}

impl ResolvedAliasReference {
    #[allow(
        clippy::too_many_arguments,
        reason = "an alias resolution fact must retain all use, declaration, and target evidence atomically"
    )]
    pub(crate) fn new(
        declaration: ProjectNominalDeclarationId,
        arguments: impl Into<Box<[TypeKind]>>,
        normalized: TypeKind,
        use_source: TypeSourceEvidence,
        declaration_source: SourceSpan,
        target_source: TypeSourceEvidence,
    ) -> Self {
        Self {
            declaration,
            arguments: arguments.into(),
            normalized,
            use_source,
            declaration_source,
            target_source,
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    pub const fn normalized(&self) -> &TypeKind {
        &self.normalized
    }

    pub const fn use_source(&self) -> &TypeSourceEvidence {
        &self.use_source
    }

    pub const fn declaration_source(&self) -> &SourceSpan {
        &self.declaration_source
    }

    pub const fn target_source(&self) -> &TypeSourceEvidence {
        &self.target_source
    }
}

impl ResolvedTypeNode {
    pub(crate) const fn new(
        node: TypeId,
        source: TypeSourceEvidence,
        terminal_source: Option<TypeSourceEvidence>,
        reference_path: Option<HirPath>,
        recovered: Option<TypeKind>,
        outcome: TypeNameResolution,
    ) -> Self {
        Self {
            node,
            source,
            terminal_source,
            reference_path,
            recovered,
            outcome,
        }
    }

    pub const fn node(&self) -> TypeId {
        self.node
    }

    pub const fn source(&self) -> &TypeSourceEvidence {
        &self.source
    }

    /// Exact final path segment selected by this node's name resolution.
    pub const fn terminal_source(&self) -> Option<&TypeSourceEvidence> {
        self.terminal_source.as_ref()
    }

    /// Validated authored path whose terminal was selected by name resolution.
    pub const fn reference_path(&self) -> Option<&HirPath> {
        self.reference_path.as_ref()
    }

    /// Semantic type recovered for this exact structural node.
    ///
    /// Constant and entity-family argument nodes are deliberately non-type
    /// values and therefore return `None`.
    pub const fn recovered(&self) -> Option<&TypeKind> {
        self.recovered.as_ref()
    }

    pub const fn outcome(&self) -> &TypeNameResolution {
        &self.outcome
    }
}

impl AliasExpansionFact {
    #[allow(
        clippy::too_many_arguments,
        reason = "one alias expansion owns its typed substitution and three distinct source sites"
    )]
    pub(crate) fn new(
        alias: ProjectNominalDeclarationId,
        arguments: impl Into<Box<[TypeKind]>>,
        substitution: impl Into<Box<[(GenericTypeParameterId, TypeKind)]>>,
        normalized: TypeKind,
        use_source: TypeSourceEvidence,
        declaration_source: SourceSpan,
        target_source: TypeSourceEvidence,
    ) -> Self {
        Self {
            alias,
            arguments: arguments.into(),
            substitution: substitution.into(),
            normalized,
            use_source,
            declaration_source,
            target_source,
        }
    }

    pub const fn alias(&self) -> &ProjectNominalDeclarationId {
        &self.alias
    }

    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    pub fn substitution(&self) -> &[(GenericTypeParameterId, TypeKind)] {
        &self.substitution
    }

    pub const fn normalized(&self) -> &TypeKind {
        &self.normalized
    }

    pub const fn use_source(&self) -> &TypeSourceEvidence {
        &self.use_source
    }

    pub const fn declaration_source(&self) -> &SourceSpan {
        &self.declaration_source
    }

    pub const fn target_source(&self) -> &TypeSourceEvidence {
        &self.target_source
    }
}

impl ResolvedTypeProduct {
    pub(crate) fn new(
        root: TypeId,
        recovered: TypeKind,
        nodes: impl Into<Box<[ResolvedTypeNode]>>,
        aliases: impl Into<Box<[AliasExpansionFact]>>,
    ) -> Self {
        Self {
            root,
            recovered,
            nodes: nodes.into(),
            aliases: aliases.into(),
        }
    }

    /// Final HIR type identity that owns this complete resolution graph.
    pub const fn root(&self) -> TypeId {
        self.root
    }

    pub const fn recovered(&self) -> &TypeKind {
        &self.recovered
    }

    pub fn nodes(&self) -> &[ResolvedTypeNode] {
        &self.nodes
    }

    pub fn aliases(&self) -> &[AliasExpansionFact] {
        &self.aliases
    }
}

impl<'a> ResolvedAssociatedTypeReceiver<'a> {
    pub(crate) fn try_from_report(
        report: &'a TypeResolutionReport,
    ) -> Result<Self, AssociatedReceiverFailure> {
        match report.outcome() {
            ResolvedTypeRefOutcome::Complete(product) => Self::try_from_product(product),
            ResolvedTypeRefOutcome::Poisoned(_) => Err(AssociatedReceiverFailure::PoisonedOutcome),
            ResolvedTypeRefOutcome::Detached(_) => Err(AssociatedReceiverFailure::DetachedOutcome),
        }
    }

    pub(crate) fn try_from_product(
        product: &'a ResolvedTypeProduct,
    ) -> Result<Self, AssociatedReceiverFailure> {
        let root = product
            .nodes()
            .iter()
            .find(|node| node.node() == product.root())
            .ok_or(AssociatedReceiverFailure::MissingRoot)?;

        if let Some(node) = product.nodes().iter().find(|node| {
            matches!(
                node.outcome(),
                TypeNameResolution::Failed(_)
                    | TypeNameResolution::Poisoned(_)
                    | TypeNameResolution::DetachedUnavailable(_)
            )
        }) {
            return Err(AssociatedReceiverFailure::IncompleteNode { node: node.node() });
        }

        let ty = root
            .recovered()
            .ok_or(AssociatedReceiverFailure::MissingRootType)?;
        if ty != product.recovered() {
            return Err(AssociatedReceiverFailure::RootTypeMismatch);
        }

        Ok(Self { product, root, ty })
    }

    pub(crate) const fn product(&self) -> &'a ResolvedTypeProduct {
        self.product
    }

    pub(crate) const fn root(&self) -> &'a ResolvedTypeNode {
        self.root
    }

    pub(crate) const fn ty(&self) -> &'a TypeKind {
        self.ty
    }
}

impl PoisonedTypeRef {
    pub(crate) fn new(
        product: ResolvedTypeProduct,
        causes: impl Into<Box<[TypePoisonId]>>,
    ) -> Self {
        let mut causes = causes.into().into_vec();
        causes.sort_unstable();
        causes.dedup();
        Self {
            product,
            causes: causes.into_boxed_slice(),
        }
    }

    pub const fn product(&self) -> &ResolvedTypeProduct {
        &self.product
    }

    pub fn causes(&self) -> &[TypePoisonId] {
        &self.causes
    }
}

impl DetachedTypeRef {
    pub(crate) fn new(
        product: ResolvedTypeProduct,
        unavailable: impl Into<Box<[TypeId]>>,
        causes: impl Into<Box<[TypePoisonId]>>,
    ) -> Self {
        let mut unavailable = unavailable.into().into_vec();
        unavailable.sort_unstable();
        unavailable.dedup();
        let mut causes = causes.into().into_vec();
        causes.sort_unstable();
        causes.dedup();
        Self {
            product,
            unavailable: unavailable.into_boxed_slice(),
            causes: causes.into_boxed_slice(),
        }
    }

    pub const fn product(&self) -> &ResolvedTypeProduct {
        &self.product
    }

    pub fn unavailable(&self) -> &[TypeId] {
        &self.unavailable
    }

    pub fn causes(&self) -> &[TypePoisonId] {
        &self.causes
    }
}

impl ResolvedTypeRefOutcome {
    pub const fn product(&self) -> &ResolvedTypeProduct {
        match self {
            Self::Complete(product) => product,
            Self::Poisoned(poisoned) => poisoned.product(),
            Self::Detached(detached) => detached.product(),
        }
    }
}

impl TypeResolutionReport {
    pub(crate) fn new(
        outcome: ResolvedTypeRefOutcome,
        diagnostics: impl Into<Box<[NominalTypeDiagnostic]>>,
        poisons: impl Into<Box<[TypePoisonRecord]>>,
        omitted_diagnostics: u64,
        work_charged: u64,
    ) -> Self {
        Self {
            outcome,
            diagnostics: diagnostics.into(),
            poisons: poisons.into(),
            omitted_diagnostics,
            work_charged,
        }
    }

    pub const fn outcome(&self) -> &ResolvedTypeRefOutcome {
        &self.outcome
    }

    pub fn diagnostics(&self) -> &[NominalTypeDiagnostic] {
        &self.diagnostics
    }

    pub fn poisons(&self) -> &[TypePoisonRecord] {
        &self.poisons
    }

    pub const fn omitted_diagnostics(&self) -> u64 {
        self.omitted_diagnostics
    }

    pub const fn work_charged(&self) -> u64 {
        self.work_charged
    }
}

impl TypeArityExpectation {
    pub const fn contains(self, actual: u16) -> bool {
        match self {
            Self::Exact(expected) => actual == expected,
            Self::Inclusive { minimum, maximum } => actual >= minimum && actual <= maximum,
        }
    }

    pub const fn minimum(self) -> u16 {
        match self {
            Self::Exact(exact) => exact,
            Self::Inclusive { minimum, .. } => minimum,
        }
    }

    pub const fn maximum(self) -> u16 {
        match self {
            Self::Exact(exact) => exact,
            Self::Inclusive { maximum, .. } => maximum,
        }
    }
}

impl DetachedNominalEvidence {
    pub(crate) const fn new(
        path: HirPath,
        source: TypeSourceEvidence,
        reason: DetachedNominalReason,
    ) -> Self {
        Self {
            path,
            source,
            reason,
        }
    }

    pub const fn path(&self) -> &HirPath {
        &self.path
    }

    pub const fn source(&self) -> &TypeSourceEvidence {
        &self.source
    }

    pub const fn reason(&self) -> DetachedNominalReason {
        self.reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_table_has_contractual_spellings_and_arities() {
        assert_eq!(
            BuiltinTypeConstructor::ALL
                .iter()
                .filter(|constructor| **constructor == BuiltinTypeConstructor::Ref)
                .count(),
            1
        );
        assert_eq!(BuiltinTypeConstructor::Bool.spelling(), "bool");
        assert_eq!(BuiltinTypeConstructor::Bool.arity(), 0);
        assert_eq!(
            BuiltinTypeConstructor::CharacterDialogue.spelling(),
            "CharacterDialogue"
        );
        assert_eq!(BuiltinTypeConstructor::CharacterDialogue.arity(), 0);
        assert_eq!(BuiltinTypeConstructor::Vec.arity(), 1);
        assert_eq!(BuiltinTypeConstructor::Array.arity(), 2);
        assert_eq!(BuiltinTypeConstructor::Ref.spelling(), "Ref");
        assert_eq!(BuiltinTypeConstructor::Ref.arity(), 1);
        assert_eq!(
            BuiltinTypeConstructor::Ref.argument_expectation(0),
            Some(TypeArgumentExpectation::EntityFamily)
        );
        assert_eq!(BuiltinTypeConstructor::Ref.argument_expectation(1), None);
        assert_eq!(
            BuiltinTypeConstructor::ENTITY_FAMILY_PROJECTIONS,
            &[BuiltinTypeConstructor::Ref]
        );
        assert_eq!(
            BuiltinTypeConstructor::Ref.project_entity_family(EntityKind::Flow),
            Some(TypeKind::entity_ref(EntityKind::Flow))
        );
    }

    #[test]
    fn type_argument_kind_has_a_total_stable_category_order() {
        let mut values = [
            TypeArgumentKind::EntityFamily(EntityKind::Flow),
            TypeArgumentKind::ConstInt(3),
            TypeArgumentKind::Type(TypeKind::String),
            TypeArgumentKind::EntityFamily(EntityKind::Character),
        ];
        values.sort_by(TypeArgumentKind::stable_ordering);
        assert_eq!(
            values,
            [
                TypeArgumentKind::Type(TypeKind::String),
                TypeArgumentKind::ConstInt(3),
                TypeArgumentKind::EntityFamily(EntityKind::Character),
                TypeArgumentKind::EntityFamily(EntityKind::Flow),
            ]
        );
    }

    #[test]
    fn arity_expectations_are_inclusive_at_both_boundaries() {
        let expectation = TypeArityExpectation::Inclusive {
            minimum: 1,
            maximum: 3,
        };
        assert!(!expectation.contains(0));
        assert!(expectation.contains(1));
        assert!(expectation.contains(3));
        assert!(!expectation.contains(4));
    }
}
