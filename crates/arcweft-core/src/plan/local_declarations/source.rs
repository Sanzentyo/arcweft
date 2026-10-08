//! Static declaration source, independent of runtime slot occupancy.

use serde::{Deserialize, Serialize};

use super::{RuntimeGeneratedLocalOrigin, RuntimeLocalOrigin};

/// Semantic kind of an authored binding declaration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeLocalBindingKind {
    Parameter,
    LetBinding,
    PatternBinding,
    ClosureParameter,
    LoopBinding,
    MatchBinding,
    PostconditionResult,
}

/// Authored lifetime policy; execution-frame placement is a separate authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeLocalBindingStorage {
    Derived,
    RetainedState,
}

/// Exact authored properties carried from the checked declaration owner.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLocalBindingDeclaration {
    kind: RuntimeLocalBindingKind,
    mutable: bool,
    storage: RuntimeLocalBindingStorage,
}

impl RuntimeLocalBindingDeclaration {
    fn encode_semantic_properties(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        encoder.tag(match self.kind {
            RuntimeLocalBindingKind::Parameter => 0,
            RuntimeLocalBindingKind::LetBinding => 1,
            RuntimeLocalBindingKind::PatternBinding => 2,
            RuntimeLocalBindingKind::ClosureParameter => 3,
            RuntimeLocalBindingKind::LoopBinding => 4,
            RuntimeLocalBindingKind::MatchBinding => 5,
            RuntimeLocalBindingKind::PostconditionResult => 6,
        });
        encoder.tag(u8::from(self.mutable));
        encoder.tag(match self.storage {
            RuntimeLocalBindingStorage::Derived => 0,
            RuntimeLocalBindingStorage::RetainedState => 1,
        });
    }

    #[must_use]
    pub const fn new(
        kind: RuntimeLocalBindingKind,
        mutable: bool,
        storage: RuntimeLocalBindingStorage,
    ) -> Self {
        Self {
            kind,
            mutable,
            storage,
        }
    }

    #[must_use]
    pub const fn kind(self) -> RuntimeLocalBindingKind {
        self.kind
    }

    #[must_use]
    pub const fn is_mutable(self) -> bool {
        self.mutable
    }

    #[must_use]
    pub const fn storage(self) -> RuntimeLocalBindingStorage {
        self.storage
    }
}

/// Complete static source of one runtime declaration request.
/// An identity-only Binding cannot be admitted without its declared properties.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeLocalDeclarationSource {
    Binding {
        identity: [u8; 32],
        declaration: RuntimeLocalBindingDeclaration,
    },
    Parameter(crate::plan::RuntimeFunctionParameterIdentity),
    EvaluatedResult(crate::plan::RuntimeFunctionDefinitionIdentity),
    Generated(RuntimeGeneratedLocalOrigin),
}

impl RuntimeLocalDeclarationSource {
    pub(crate) fn encode_semantic_source(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            Self::Binding {
                identity,
                declaration,
            } => {
                encoder.tag(0);
                encoder.digest(&identity);
                declaration.encode_semantic_properties(encoder);
            }
            Self::Parameter(identity) => {
                encoder.tag(1);
                encoder.digest(identity.as_bytes());
            }
            Self::EvaluatedResult(identity) => {
                encoder.tag(2);
                encoder.digest(identity.as_bytes());
            }
            Self::Generated(identity) => {
                encoder.tag(3);
                encoder.digest(identity.as_bytes());
            }
        }
    }

    /// Identity projection for existing coordinate/correlation consumers.
    #[must_use]
    pub const fn origin(self) -> RuntimeLocalOrigin {
        match self {
            Self::Binding { identity, .. } => RuntimeLocalOrigin::Binding(identity),
            Self::Parameter(parameter) => RuntimeLocalOrigin::Parameter(parameter),
            Self::EvaluatedResult(definition) => RuntimeLocalOrigin::EvaluatedResult(definition),
            Self::Generated(origin) => RuntimeLocalOrigin::Generated(origin),
        }
    }

    #[must_use]
    pub const fn binding(self) -> Option<RuntimeLocalBindingDeclaration> {
        match self {
            Self::Binding { declaration, .. } => Some(declaration),
            Self::Parameter(_) | Self::EvaluatedResult(_) | Self::Generated(_) => None,
        }
    }
}
