//! Typed semantic identities produced by nominal type resolution.

use arcweft_lang_hir::{
    leaf::{HirPath, HirPathRoot, HirPathSegment},
    symbol::{CallableDeclarationKey, nominal::ProjectNominalDeclarationId},
};
use arcweft_source::SourceSpan;
use std::sync::Arc;

use crate::env::nominal::{AcceptedNominalId, OpenNominalRuleId};

use super::TypeKind;

/// Resolver-local evidence that a semantic type node has already failed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypePoisonId(u32);

/// Declaration or detached scope that owns one semantic generic parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericParameterOwnerId {
    Callable(CallableDeclarationKey),
    Nominal(ProjectNominalDeclarationId),
    AcceptedNominal(AcceptedNominalId),
    AcceptedSource(SourceSpan),
    Detached(DetachedGenericOwnerId),
    LanguageIntrinsic(LanguageIntrinsicGenericOwner),
}

/// Language-owned intrinsic family that owns closed generic parameters.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LanguageIntrinsicGenericOwner {
    OptionConstructor,
    ResultConstructor,
    CollectionMap,
    FxExists,
    AgentSignal,
    AgentMetric,
}

impl LanguageIntrinsicGenericOwner {
    pub const ALL: [Self; 6] = [
        Self::OptionConstructor,
        Self::ResultConstructor,
        Self::CollectionMap,
        Self::FxExists,
        Self::AgentSignal,
        Self::AgentMetric,
    ];

    /// Canonical version-1 semantic tag owned by this closed family.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::OptionConstructor => 0,
            Self::ResultConstructor => 1,
            Self::CollectionMap => 2,
            Self::FxExists => 3,
            Self::AgentSignal => 4,
            Self::AgentMetric => 5,
        }
    }

    const fn source_label(self) -> &'static str {
        match self {
            Self::OptionConstructor => "language.option-constructor",
            Self::ResultConstructor => "language.result-constructor",
            Self::CollectionMap => "language.collection-map",
            Self::FxExists => "language.fx-exists",
            Self::AgentSignal => "language.agent-signal",
            Self::AgentMetric => "language.agent-metric",
        }
    }
}

/// Stable caller-supplied owner for generic parameters resolved without a project.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DetachedGenericOwnerId(u64);

/// Shared declaration coordinate used by the distinct type and constant
/// parameter identity wrappers below.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct GenericParameterCoordinate {
    owner: Arc<GenericParameterOwnerId>,
    ordinal: u16,
}

/// Declaration-relative identity of one generic type parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericTypeParameterId(GenericParameterCoordinate);

/// Declaration-relative identity of one generic constant parameter.
///
/// This is deliberately a different type from [`GenericTypeParameterId`].
/// A declaration coordinate may be shared by the identity implementation,
/// but a type parameter can never be silently reinterpreted as a constant
/// parameter (or vice versa) at a semantic boundary.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericConstParameterId(GenericParameterCoordinate);

/// Instantiation of one source-backed project struct or enum declaration.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectNominalType {
    declaration: Arc<ProjectNominalDeclarationId>,
    arguments: Box<[TypeKind]>,
}

/// Instantiation of one exact accepted environment nominal declaration.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AcceptedNominalType {
    declaration: Arc<AcceptedNominalId>,
    arguments: Box<[TypeKind]>,
}

/// Instantiation admitted by one explicit open-nominal rule.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OpenNominalType {
    rule: Arc<OpenNominalRuleId>,
    path: Arc<HirPath>,
    arguments: Box<[TypeKind]>,
}

impl TypePoisonId {
    /// Creates an identity from its resolver-local allocation index.
    pub(crate) const fn from_index(index: u32) -> Self {
        Self(index)
    }

    /// Resolver-local allocation index.
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl DetachedGenericOwnerId {
    /// Creates a detached generic owner from a caller-stable identity.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Caller-stable detached identity.
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl GenericTypeParameterId {
    /// Creates the identity of one declaration-relative generic parameter.
    pub fn new(owner: GenericParameterOwnerId, ordinal: u16) -> Self {
        Self(GenericParameterCoordinate {
            owner: Arc::new(owner),
            ordinal,
        })
    }

    /// Declaration or detached scope that owns this parameter.
    pub fn owner(&self) -> &GenericParameterOwnerId {
        self.0.owner.as_ref()
    }

    /// Zero-based position within the owning generic parameter list.
    pub const fn ordinal(&self) -> u16 {
        self.0.ordinal
    }

    pub(crate) fn source_label(&self) -> String {
        generic_source_label(self.0.owner.as_ref(), self.0.ordinal, "$generic")
    }
}

impl GenericConstParameterId {
    /// Creates the identity of one declaration-relative generic constant.
    pub fn new(owner: GenericParameterOwnerId, ordinal: u16) -> Self {
        Self(GenericParameterCoordinate {
            owner: Arc::new(owner),
            ordinal,
        })
    }

    /// Declaration or detached scope that owns this parameter.
    pub fn owner(&self) -> &GenericParameterOwnerId {
        self.0.owner.as_ref()
    }

    /// Zero-based position within the owning constant-parameter list.
    pub const fn ordinal(&self) -> u16 {
        self.0.ordinal
    }

    pub(crate) fn source_label(&self) -> String {
        generic_source_label(self.0.owner.as_ref(), self.0.ordinal, "$const")
    }
}

fn generic_source_label(owner: &GenericParameterOwnerId, ordinal: u16, prefix: &str) -> String {
    let owner = match owner {
        GenericParameterOwnerId::Callable(owner) => {
            format!("{}::{}::{}", owner.package(), owner.module(), owner.name())
        }
        GenericParameterOwnerId::Nominal(owner) => owner.qualified_name(),
        GenericParameterOwnerId::AcceptedNominal(owner) => owner.source_label(),
        GenericParameterOwnerId::AcceptedSource(source) => {
            let range = source.range();
            format!(
                "{}@{}:{}..{}",
                source.source().id(),
                source.source().revision().to_hex(),
                range.start(),
                range.end()
            )
        }
        GenericParameterOwnerId::Detached(owner) => format!("detached:{}", owner.value()),
        GenericParameterOwnerId::LanguageIntrinsic(owner) => owner.source_label().to_owned(),
    };
    format!("{prefix}<{owner}>#{ordinal}")
}

impl ProjectNominalType {
    /// Creates a checked instantiation of a project nominal declaration.
    pub(crate) fn new(
        declaration: ProjectNominalDeclarationId,
        arguments: impl Into<Box<[TypeKind]>>,
    ) -> Self {
        Self {
            declaration: Arc::new(declaration),
            arguments: arguments.into(),
        }
    }

    /// Original project declaration selected through the symbol table.
    pub fn declaration(&self) -> &ProjectNominalDeclarationId {
        self.declaration.as_ref()
    }

    /// Checked type arguments in authored order.
    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    pub(super) fn source_label(&self) -> String {
        application_label(&self.declaration.qualified_name(), &self.arguments)
    }
}

impl AcceptedNominalType {
    /// Creates a checked instantiation of an accepted nominal declaration.
    pub(crate) fn new(
        declaration: AcceptedNominalId,
        arguments: impl Into<Box<[TypeKind]>>,
    ) -> Self {
        Self {
            declaration: Arc::new(declaration),
            arguments: arguments.into(),
        }
    }

    /// Exact accepted declaration selected from the environment catalog.
    pub fn declaration(&self) -> &AcceptedNominalId {
        self.declaration.as_ref()
    }

    /// Checked type arguments in authored order.
    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    pub(super) fn source_label(&self) -> String {
        application_label(
            &self.declaration.canonical_path().canonical_string(),
            &self.arguments,
        )
    }
}

impl OpenNominalType {
    /// Creates a checked instantiation admitted by an explicit open rule.
    pub(crate) fn new(
        rule: OpenNominalRuleId,
        path: HirPath,
        arguments: impl Into<Box<[TypeKind]>>,
    ) -> Self {
        Self {
            rule: Arc::new(rule),
            path: Arc::new(path),
            arguments: arguments.into(),
        }
    }

    /// Exact environment rule that admitted this type.
    pub fn rule(&self) -> &OpenNominalRuleId {
        self.rule.as_ref()
    }

    /// Root-preserving semantic path retained for deterministic diagnostics.
    pub fn path(&self) -> &HirPath {
        self.path.as_ref()
    }

    /// Checked type arguments in authored order.
    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    pub(super) fn source_label(&self) -> String {
        application_label(&hir_path_label(&self.path), &self.arguments)
    }
}

fn hir_path_label(path: &HirPath) -> String {
    let mut label = match path.root() {
        HirPathRoot::ImplicitCrate => String::new(),
        HirPathRoot::Crate => "crate.".to_owned(),
        HirPathRoot::SelfModule => "self.".to_owned(),
        HirPathRoot::Super { depth } => "super.".repeat(depth),
    };
    label.push_str(
        &path
            .segments()
            .iter()
            .map(|segment| match segment {
                HirPathSegment::Identifier(name) => name.as_str(),
                HirPathSegment::ProjectSymbol(name) => name.as_str(),
            })
            .collect::<Vec<_>>()
            .join("."),
    );
    label
}

fn application_label(head: &str, arguments: &[TypeKind]) -> String {
    if arguments.is_empty() {
        return head.to_owned();
    }
    format!(
        "{head}<{}>",
        arguments
            .iter()
            .map(TypeKind::source_label)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::{
        DetachedGenericOwnerId, GenericConstParameterId, GenericParameterOwnerId,
        GenericTypeParameterId, TypeKind, TypePoisonId,
    };
    use crate::types::TypeMismatchPathSegment;

    #[test]
    fn generic_parameter_identity_retains_owner_and_ordinal() {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(17));
        let first = GenericTypeParameterId::new(owner.clone(), 0);
        let second = GenericTypeParameterId::new(owner, 1);
        let other_owner = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(18)),
            0,
        );

        assert_ne!(first, second);
        assert_ne!(first, other_owner);
        assert_eq!(first.ordinal(), 0);
        let first = TypeKind::GenericParam(first);
        let second = TypeKind::GenericParam(second);
        assert_eq!(
            first
                .first_mismatch(&second)
                .expect("distinct generic identities differ")
                .path(),
            &[TypeMismatchPathSegment::GenericIdentity]
        );
        assert_eq!(first.source_label(), "$generic<detached:17>#0");
    }

    #[test]
    fn poison_is_deterministic_and_recovery_compatible() {
        let poison = TypeKind::Error(TypePoisonId::from_index(9));

        assert_eq!(poison.source_label(), "<type-error:9>");
        assert!(poison.accepts(&TypeKind::I32));
        assert!(TypeKind::I32.accepts(&poison));
    }

    #[test]
    fn type_and_const_parameter_namespaces_are_distinct() {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(19));
        let ty = GenericTypeParameterId::new(owner.clone(), 0);
        let constant = GenericConstParameterId::new(owner, 0);

        assert_eq!(ty.ordinal(), constant.ordinal());
        assert_eq!(ty.owner(), constant.owner());
        assert_ne!(ty.source_label(), constant.source_label());
    }
}
