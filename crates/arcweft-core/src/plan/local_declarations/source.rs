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

impl RuntimeLocalBindingKind {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Parameter => 0,
            Self::LetBinding => 1,
            Self::PatternBinding => 2,
            Self::ClosureParameter => 3,
            Self::LoopBinding => 4,
            Self::MatchBinding => 5,
            Self::PostconditionResult => 6,
        }
    }
    pub(crate) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::Parameter,
            1 => Self::LetBinding,
            2 => Self::PatternBinding,
            3 => Self::ClosureParameter,
            4 => Self::LoopBinding,
            5 => Self::MatchBinding,
            6 => Self::PostconditionResult,
            _ => return None,
        })
    }
}

/// Authored lifetime policy; execution-frame placement is a separate authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeLocalBindingStorage {
    Derived,
    RetainedState,
}

impl RuntimeLocalBindingStorage {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Derived => 0,
            Self::RetainedState => 1,
        }
    }
    pub(crate) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::Derived,
            1 => Self::RetainedState,
            _ => return None,
        })
    }
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
        encoder.tag(self.kind.semantic_tag());
        encoder.tag(u8::from(self.mutable));
        encoder.tag(self.storage.semantic_tag());
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
