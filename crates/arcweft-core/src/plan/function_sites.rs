//! Plan-owned structured function bodies.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::RuntimeExecutableBody;

use crate::pattern::RuntimePattern;
use crate::runtime_id::RuntimePlanTypeId;
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use crate::value::RuntimeExpr;

/// Semantic purpose of an accepted function definition, independent of its
/// expression or control-transfer execution body.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeFunctionSemanticRole {
    Ordinary,
    Closure,
    Dialogue,
    Effect,
    Line,
    Stream,
}

impl RuntimeFunctionSemanticRole {
    pub const ALL: &[Self] = &[
        Self::Ordinary,
        Self::Closure,
        Self::Dialogue,
        Self::Effect,
        Self::Line,
        Self::Stream,
    ];

    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Ordinary),
            1 => Some(Self::Closure),
            2 => Some(Self::Dialogue),
            3 => Some(Self::Effect),
            4 => Some(Self::Line),
            5 => Some(Self::Stream),
            _ => None,
        }
    }

    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Ordinary => 0,
            Self::Closure => 1,
            Self::Dialogue => 2,
            Self::Effect => 3,
            Self::Line => 4,
            Self::Stream => 5,
        }
    }
}

/// Static parameter ownership class. Frame ingress guarantees are recorded
/// independently: a value-dependent affine carrier can require a checked
/// unrestricted supplied value without changing its declared passing class.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeFunctionParameterPassing {
    Value,
    Shared,
    Affine,
}

impl RuntimeFunctionParameterPassing {
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Value => 0,
            Self::Shared => 1,
            Self::Affine => 2,
        }
    }
}

/// The body family reserved by a function-site construction handle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeFunctionSiteBodyKind {
    Expression,
    Executable,
}

/// Origin of one function-site input row in the closed call ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeFunctionInputSource {
    Capture { position: u32 },
    Parameter { position: u32 },
}

/// The live value guarantee required when an input enters a function frame.
/// Function types do not imply this guarantee; the selected caller supplies it.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFunctionInputOwnershipRequirement {
    #[default]
    Owned,
    Unrestricted,
}

/// One synthetic input local and its checked pattern prologue.
///
/// The input local receives the raw ABI value. The pattern then binds the
/// body's actual locals. This keeps destructuring, discard, tuple, record, and
/// rest patterns in the same binder used by ordinary flow operations.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFunctionInputBinding {
    source: RuntimeFunctionInputSource,
    input_local: RuntimeLocalDeclarationId,
    pattern: RuntimePattern,
    ownership: RuntimeFunctionInputOwnershipRequirement,
    unrestricted_bindings: Box<[RuntimeLocalDeclarationId]>,
}

impl RuntimeFunctionInputBinding {
    pub(crate) const fn new(
        source: RuntimeFunctionInputSource,
        input_local: RuntimeLocalDeclarationId,
        pattern: RuntimePattern,
        ownership: RuntimeFunctionInputOwnershipRequirement,
        unrestricted_bindings: Box<[RuntimeLocalDeclarationId]>,
    ) -> Self {
        Self {
            source,
            input_local,
            pattern,
            ownership,
            unrestricted_bindings,
        }
    }

    #[must_use]
    pub const fn source(&self) -> RuntimeFunctionInputSource {
        self.source
    }

    #[must_use]
    pub const fn input_local(&self) -> RuntimeLocalDeclarationId {
        self.input_local
    }

    #[must_use]
    pub const fn pattern(&self) -> &RuntimePattern {
        &self.pattern
    }

    #[must_use]
    pub const fn ownership(&self) -> RuntimeFunctionInputOwnershipRequirement {
        self.ownership
    }

    #[must_use]
    pub const fn unrestricted_bindings(&self) -> &[RuntimeLocalDeclarationId] {
        &self.unrestricted_bindings
    }
}

/// One typed structured function body owned by its complete runtime plan.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeFunctionSiteBody {
    Expression(RuntimeExpr),
    Executable(RuntimeExecutableBody),
}

impl RuntimeFunctionSiteBody {
    #[must_use]
    pub const fn kind(&self) -> RuntimeFunctionSiteBodyKind {
        match self {
            Self::Expression(_) => RuntimeFunctionSiteBodyKind::Expression,
            Self::Executable(_) => RuntimeFunctionSiteBodyKind::Executable,
        }
    }

    #[must_use]
    pub const fn expression(&self) -> Option<&RuntimeExpr> {
        match self {
            Self::Expression(expression) => Some(expression),
            Self::Executable(_) => None,
        }
    }

    #[must_use]
    pub const fn executable(&self) -> Option<&RuntimeExecutableBody> {
        match self {
            Self::Expression(_) => None,
            Self::Executable(executable) => Some(executable),
        }
    }

    #[must_use]
    pub fn is_effect_free(&self) -> bool {
        match self {
            Self::Expression(_) => true,
            Self::Executable(body) => body.is_effect_free(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFunctionSite {
    role: RuntimeFunctionSemanticRole,
    function_type: Option<RuntimePlanTypeId>,
    inputs: Box<[RuntimeFunctionInputBinding]>,
    result: RuntimePlanTypeId,
    body: RuntimeFunctionSiteBody,
}

impl RuntimeFunctionSite {
    #[must_use]
    pub const fn role(&self) -> RuntimeFunctionSemanticRole {
        self.role
    }

    #[must_use]
    pub const fn function_type(&self) -> Option<RuntimePlanTypeId> {
        self.function_type
    }
    #[must_use]
    pub const fn body(&self) -> &RuntimeFunctionSiteBody {
        &self.body
    }

    #[must_use]
    pub const fn inputs(&self) -> &[RuntimeFunctionInputBinding] {
        &self.inputs
    }

    /// Returns capture-input rows in their checked ABI order.
    pub fn capture_inputs(&self) -> impl Iterator<Item = &RuntimeFunctionInputBinding> {
        self.inputs
            .iter()
            .filter(|input| matches!(input.source(), RuntimeFunctionInputSource::Capture { .. }))
    }

    /// Returns logical parameter-input rows in their checked ABI order.
    pub fn parameter_inputs(&self) -> impl Iterator<Item = &RuntimeFunctionInputBinding> {
        self.inputs
            .iter()
            .filter(|input| matches!(input.source(), RuntimeFunctionInputSource::Parameter { .. }))
    }

    #[must_use]
    pub const fn result(&self) -> RuntimePlanTypeId {
        self.result
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeFunctionSiteTable {
    sites: Box<[RuntimeFunctionSite]>,
}

impl RuntimeFunctionSiteTable {
    #[must_use]
    pub fn get(&self, id: RuntimeFunctionSiteId) -> Option<&RuntimeFunctionSite> {
        usize::try_from(id.get().get() - 1)
            .ok()
            .and_then(|index| self.sites.get(index))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sites.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &RuntimeFunctionSite> {
        self.sites.iter()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeFunctionSiteTableBuilder {
    sites: Vec<RuntimeFunctionSite>,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeFunctionSiteError {
    #[error("runtime function-site identity space is exhausted")]
    IdentityExhausted,
}

impl RuntimeFunctionSiteTableBuilder {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self { sites: Vec::new() }
    }

    pub(crate) fn push(
        &mut self,
        role: RuntimeFunctionSemanticRole,
        function_type: Option<RuntimePlanTypeId>,
        inputs: Box<[RuntimeFunctionInputBinding]>,
        result: RuntimePlanTypeId,
        body: RuntimeFunctionSiteBody,
    ) -> Result<RuntimeFunctionSiteId, RuntimeFunctionSiteError> {
        let ordinal = self
            .sites
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeFunctionSiteError::IdentityExhausted)?;
        self.sites.push(RuntimeFunctionSite {
            role,
            function_type,
            inputs,
            result,
            body,
        });
        Ok(RuntimeFunctionSiteId::from_accepted_ordinal(ordinal))
    }

    #[must_use]
    pub(crate) fn finish(self) -> RuntimeFunctionSiteTable {
        RuntimeFunctionSiteTable {
            sites: self.sites.into_boxed_slice(),
        }
    }
}
