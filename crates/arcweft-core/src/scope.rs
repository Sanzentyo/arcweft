//! Accepted lexical namespace carried by executable scopes.

use arcweft_id::DeclarationName;
use serde::{Deserialize, Serialize};

/// Namespace contribution of one lexical scope. Static executable coordinates,
/// rather than this optional name, distinguish separate scopes with the same name.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeScopeIdentity {
    #[default]
    Anonymous,
    Named(DeclarationName),
}

impl RuntimeScopeIdentity {
    pub const fn name(&self) -> Option<&DeclarationName> {
        match self {
            Self::Anonymous => None,
            Self::Named(name) => Some(name),
        }
    }
}
