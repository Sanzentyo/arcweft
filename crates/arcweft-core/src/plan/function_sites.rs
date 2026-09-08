//! Plan-owned structured function bodies.

use std::num::NonZeroU32;

use thiserror::Error;

use arcweft_id::EffectId;

use super::FlowOp;
use crate::pattern::RuntimePattern;
use crate::runtime_id::RuntimePlanTypeId;
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use crate::value::RuntimeExpr;

/// The one runtime projection of a checked closed effect row.
///
/// The aggregate function-site table owns this row.  It is deliberately a
/// typed slice of the foundational `EffectId` values rather than a debug
/// label, source spelling, or encoded integer.  Callers must provide the
/// canonical set; the checked constructor rejects duplicate members.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFunctionEffectSet(Box<[EffectId]>);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeFunctionEffectSetError {
    #[error("runtime function effect set contains duplicate effect `{effect}`")]
    Duplicate { effect: EffectId },
}

impl RuntimeFunctionEffectSet {
    #[must_use]
    pub fn empty() -> Self {
        Self(Vec::new().into_boxed_slice())
    }

    /// Constructs the canonical sorted/unique runtime projection of one
    /// checked effect row.
    pub fn try_from_effects(
        effects: impl IntoIterator<Item = EffectId>,
    ) -> Result<Self, RuntimeFunctionEffectSetError> {
        let mut effects = effects.into_iter().collect::<Vec<_>>();
        effects.sort();
        if let Some(effect) = effects
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then(|| pair[0].clone()))
        {
            return Err(RuntimeFunctionEffectSetError::Duplicate { effect });
        }
        Ok(Self(effects.into_boxed_slice()))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[EffectId] {
        &self.0
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &EffectId> + DoubleEndedIterator {
        self.0.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
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
}

impl RuntimeFunctionInputBinding {
    pub(crate) const fn new(
        source: RuntimeFunctionInputSource,
        input_local: RuntimeLocalDeclarationId,
        pattern: RuntimePattern,
    ) -> Self {
        Self {
            source,
            input_local,
            pattern,
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
}

/// An executable structured function body.  Its flow operations are lowered
/// through the same closed `FlowOp` algebra used by ordinary plan roots.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFunctionExecutableBody {
    effects: RuntimeFunctionEffectSet,
    ops: Box<[FlowOp]>,
}

impl RuntimeFunctionExecutableBody {
    pub(crate) fn new(effects: RuntimeFunctionEffectSet, ops: Box<[FlowOp]>) -> Self {
        Self { effects, ops }
    }

    #[must_use]
    pub const fn effects(&self) -> &RuntimeFunctionEffectSet {
        &self.effects
    }

    #[must_use]
    pub fn is_effect_free(&self) -> bool {
        self.effects.is_empty()
    }

    #[must_use]
    pub fn ops(&self) -> &[FlowOp] {
        &self.ops
    }
}

/// One typed structured function body owned by its complete runtime plan.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeFunctionSiteBody {
    Expression(RuntimeExpr),
    Executable(RuntimeFunctionExecutableBody),
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
    pub const fn executable(&self) -> Option<&RuntimeFunctionExecutableBody> {
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
    inputs: Box<[RuntimeFunctionInputBinding]>,
    result: RuntimePlanTypeId,
    body: RuntimeFunctionSiteBody,
}

impl RuntimeFunctionSite {
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
