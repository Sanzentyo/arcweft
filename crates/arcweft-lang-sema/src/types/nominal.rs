//! Typed semantic identities produced by nominal type resolution.

use arcweft_lang_hir::symbol::{CallableDeclarationId, nominal::ProjectNominalDeclarationId};
use arcweft_lang_syntax::types::TypePath;
use arcweft_source::SourceSpan;
use std::sync::Arc;

use crate::env::nominal::{AcceptedNominalId, OpenNominalRuleId};

use super::TypeKind;

/// Resolver-local evidence that a semantic type node has already failed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypePoisonId(u32);

/// Declaration or detached scope that owns one semantic generic parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericTypeOwnerId {
    Callable(CallableDeclarationId),
    Nominal(ProjectNominalDeclarationId),
    AcceptedSource(SourceSpan),
    Detached(DetachedTypeOwnerId),
}

/// Stable caller-supplied owner for generic parameters resolved without a project.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DetachedTypeOwnerId(u64);

/// Declaration-relative identity of one generic type parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericTypeParameterId {
    owner: Arc<GenericTypeOwnerId>,
    ordinal: u16,
}

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
    path: Arc<TypePath>,
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

impl DetachedTypeOwnerId {
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
    pub fn new(owner: GenericTypeOwnerId, ordinal: u16) -> Self {
        Self {
            owner: Arc::new(owner),
            ordinal,
        }
    }

    /// Declaration or detached scope that owns this parameter.
    pub fn owner(&self) -> &GenericTypeOwnerId {
        self.owner.as_ref()
    }

    /// Zero-based position within the owning generic parameter list.
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    pub(super) fn source_label(&self) -> String {
        let owner = match self.owner.as_ref() {
            GenericTypeOwnerId::Callable(owner) => owner.qualified_name(),
            GenericTypeOwnerId::Nominal(owner) => owner.qualified_name(),
            GenericTypeOwnerId::AcceptedSource(source) => {
                let range = source.range();
                format!(
                    "{}@{}:{}..{}",
                    source.source().id(),
                    source.source().revision().to_hex(),
                    range.start(),
                    range.end()
                )
            }
            GenericTypeOwnerId::Detached(owner) => format!("detached:{}", owner.value()),
        };
        format!("$generic<{owner}>#{}", self.ordinal)
    }
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
        application_label(&self.declaration.source_label(), &self.arguments)
    }
}

impl OpenNominalType {
    /// Creates a checked instantiation admitted by an explicit open rule.
    pub(crate) fn new(
        rule: OpenNominalRuleId,
        path: TypePath,
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

    /// Authored typed path retained for deterministic diagnostics.
    pub fn path(&self) -> &TypePath {
        self.path.as_ref()
    }

    /// Checked type arguments in authored order.
    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    pub(super) fn source_label(&self) -> String {
        application_label(&self.path.canonical_string(), &self.arguments)
    }
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
        DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId, TypeKind, TypePoisonId,
    };
    use crate::types::TypeMismatchPathSegment;

    #[test]
    fn generic_parameter_identity_retains_owner_and_ordinal() {
        let owner = GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(17));
        let first = GenericTypeParameterId::new(owner.clone(), 0);
        let second = GenericTypeParameterId::new(owner, 1);
        let other_owner = GenericTypeParameterId::new(
            GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(18)),
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
}
