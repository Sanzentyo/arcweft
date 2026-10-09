//! Plan-owned structured function bodies.

use std::num::NonZeroU32;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::RuntimeExecutableBody;

use crate::pattern::RuntimePattern;
use crate::runtime_id::RuntimePlanTypeId;
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use crate::value::RuntimeExpr;

/// Stable lexical definition identity transported from its semantic owner.
/// These bytes identify a definition; they do not prove body admission or sealing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeFunctionDefinitionIdentity([u8; 32]);

impl RuntimeFunctionDefinitionIdentity {
    /// Projects an identity already issued by the accepted semantic owner.
    #[must_use]
    pub const fn from_accepted_identity(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Identifies one generated recipe under this accepted semantic owner.
    /// Source ordinals address the parent's declaration graph before AWBC allocation.
    pub fn generated_child(self, role: RuntimeGeneratedFunctionRole) -> Self {
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.runtime.generated-function-definition.v1\0");
        hash.update(self.as_bytes());
        match role {
            RuntimeGeneratedFunctionRole::TaskRequest => {
                hash.update(&[0]);
            }
            RuntimeGeneratedFunctionRole::FormatOperand { parameter } => {
                hash.update(&[1, parameter.index() as u8]);
            }
            RuntimeGeneratedFunctionRole::LineActivation => {
                hash.update(&[2]);
            }
            RuntimeGeneratedFunctionRole::LineAction { source_node } => {
                hash.update(&[3]);
                hash.update(&source_node.to_le_bytes());
            }
            RuntimeGeneratedFunctionRole::LineCancellation { source_rule } => {
                hash.update(&[4]);
                hash.update(&source_rule.to_le_bytes());
            }
            RuntimeGeneratedFunctionRole::LineCleanup { exit } => {
                let exit = match exit {
                    crate::line_task::ScopeExit::Completed => 0,
                    crate::line_task::ScopeExit::Cancelled => 1,
                    crate::line_task::ScopeExit::Failed => 2,
                };
                hash.update(&[5, exit]);
            }
        }
        Self(*hash.finalize().as_bytes())
    }
}

/// Closed semantic purposes of generated executable functions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeGeneratedFunctionRole {
    TaskRequest,
    FormatOperand {
        parameter: crate::value::RuntimeFmtParameterId,
    },
    LineActivation,
    LineAction {
        source_node: u32,
    },
    LineCancellation {
        source_rule: u32,
    },
    LineCleanup {
        exit: crate::line_task::ScopeExit,
    },
}

/// Accepted whole-formal identity, independent of its frame local and ABI.
/// These bytes transport semantic identity; they do not prove body admission.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeFunctionParameterIdentity([u8; 32]);

impl RuntimeFunctionParameterIdentity {
    #[must_use]
    pub const fn from_accepted_identity(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable semantic origin of a whole input, separate from its frame ordinal.
/// Binding coordinates and whole-formal identities retain distinct domains.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeFunctionInputOrigin {
    Binding([u8; 32]),
    Parameter(RuntimeFunctionParameterIdentity),
    EvaluatedResult(RuntimeFunctionDefinitionIdentity),
}

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
    Flow,
}

impl RuntimeFunctionSemanticRole {
    pub const ALL: &[Self] = &[
        Self::Ordinary,
        Self::Closure,
        Self::Dialogue,
        Self::Effect,
        Self::Line,
        Self::Stream,
        Self::Flow,
    ];

    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Ordinary),
            1 => Some(Self::Closure),
            2 => Some(Self::Dialogue),
            3 => Some(Self::Effect),
            4 => Some(Self::Line),
            5 => Some(Self::Stream),
            6 => Some(Self::Flow),
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
            Self::Flow => 6,
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
    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Value),
            1 => Some(Self::Shared),
            2 => Some(Self::Affine),
            _ => None,
        }
    }

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

/// Selected value operation when a retained packet is created.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeFunctionCaptureMode {
    Copy,
    SnapshotClone,
    Move,
}

/// Retained value creation is distinct from an extracted body's external binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeFunctionInputTransfer {
    Transferred(RuntimeFunctionCaptureMode),
    ExternalBinding,
    Formal,
}

/// Origin of one function-site input row in the closed call ABI.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeFunctionInputSource {
    Capture {
        position: u32,
    },
    /// A whole formal retained by a continuation or parameter default. Its
    /// value enters through the capture packet without losing formal passing.
    CapturedParameter {
        position: u32,
        passing: RuntimeFunctionParameterPassing,
    },
    Parameter {
        position: u32,
        passing: RuntimeFunctionParameterPassing,
    },
}

impl RuntimeFunctionInputSource {
    /// Stable origin grammar for explicit function-site input rows.
    pub const fn accepts_origin(self, origin: RuntimeFunctionInputOrigin) -> bool {
        matches!(
            (self, origin),
            (
                Self::Capture { .. },
                RuntimeFunctionInputOrigin::Binding(_)
                    | RuntimeFunctionInputOrigin::EvaluatedResult(_)
            ) | (
                Self::Parameter { .. } | Self::CapturedParameter { .. },
                RuntimeFunctionInputOrigin::Parameter(_)
            )
        )
    }

    /// AWBC-generated thunks retain the full origin of the captured local.
    /// Supplied and retained whole formals still require formal identities.
    pub const fn accepts_local_origin(self, origin: super::RuntimeLocalOrigin) -> bool {
        match self {
            Self::Capture { .. } => true,
            Self::Parameter { .. } | Self::CapturedParameter { .. } => {
                matches!(origin, super::RuntimeLocalOrigin::Parameter(_))
            }
        }
    }

    /// Valid creation operations for this semantic input role.
    pub const fn accepts_transfer(self, transfer: RuntimeFunctionInputTransfer) -> bool {
        matches!(
            (self, transfer),
            (
                Self::Capture { .. },
                RuntimeFunctionInputTransfer::Transferred(_)
                    | RuntimeFunctionInputTransfer::ExternalBinding
            ) | (
                Self::Parameter { .. } | Self::CapturedParameter { .. },
                RuntimeFunctionInputTransfer::Formal
            )
        )
    }
}

impl RuntimeFunctionInputOrigin {
    pub(crate) fn encode_semantic_origin(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            Self::Binding(identity) => {
                encoder.tag(0);
                encoder.digest(&identity);
            }
            Self::Parameter(identity) => {
                encoder.tag(1);
                encoder.digest(identity.as_bytes());
            }
            Self::EvaluatedResult(identity) => {
                encoder.tag(2);
                encoder.digest(identity.as_bytes());
            }
        }
    }
}

impl RuntimeFunctionInputTransfer {
    /// Formal passing is retained explicitly; extraction never invents a
    /// language-level capture operation for an external or whole-formal input.
    pub(crate) fn encode_semantic_transfer(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            Self::Transferred(mode) => {
                encoder.tag(0);
                encoder.tag(mode.semantic_tag());
            }
            Self::ExternalBinding => encoder.tag(1),
            Self::Formal => encoder.tag(2),
        }
    }
}

impl RuntimeFunctionCaptureMode {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Copy => 0,
            Self::SnapshotClone => 1,
            Self::Move => 2,
        }
    }
}

impl RuntimeFunctionInputSource {
    pub(crate) fn encode_semantic_source(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            Self::Capture { position } => {
                encoder.tag(0);
                encoder.ordinal(position);
            }
            Self::CapturedParameter { position, passing } => {
                encoder.tag(1);
                encoder.ordinal(position);
                encoder.tag(passing.semantic_tag());
            }
            Self::Parameter { position, passing } => {
                encoder.tag(2);
                encoder.ordinal(position);
                encoder.tag(passing.semantic_tag());
            }
        }
    }
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
    transfer: RuntimeFunctionInputTransfer,
    origin: RuntimeFunctionInputOrigin,
    source: RuntimeFunctionInputSource,
    input_local: RuntimeLocalDeclarationId,
    pattern: RuntimePattern,
    ownership: RuntimeFunctionInputOwnershipRequirement,
    unrestricted_bindings: Box<[RuntimeLocalDeclarationId]>,
}

impl RuntimeFunctionInputBinding {
    pub(crate) const fn new(
        transfer: RuntimeFunctionInputTransfer,
        origin: RuntimeFunctionInputOrigin,
        source: RuntimeFunctionInputSource,
        input_local: RuntimeLocalDeclarationId,
        pattern: RuntimePattern,
        ownership: RuntimeFunctionInputOwnershipRequirement,
        unrestricted_bindings: Box<[RuntimeLocalDeclarationId]>,
    ) -> Self {
        Self {
            transfer,
            origin,
            source,
            input_local,
            pattern,
            ownership,
            unrestricted_bindings,
        }
    }

    #[must_use]
    pub const fn transfer(&self) -> RuntimeFunctionInputTransfer {
        self.transfer
    }

    #[must_use]
    pub const fn origin(&self) -> RuntimeFunctionInputOrigin {
        self.origin
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
    pub(super) definition: RuntimeFunctionDefinitionIdentity,
    pub(super) role: RuntimeFunctionSemanticRole,
    pub(super) function_type: Option<RuntimePlanTypeId>,
    pub(super) inputs: Box<[RuntimeFunctionInputBinding]>,
    pub(super) result: RuntimePlanTypeId,
    pub(super) invocation_effects: super::RuntimeEffectSet,
    pub(super) body: RuntimeFunctionSiteBody,
}

impl RuntimeFunctionSite {
    /// Ordinary expression bodies may be offered to a scalar backend. Other
    /// callable roles keep their invocation and return ownership in the Engine.
    #[must_use]
    pub fn is_eager_pure_candidate(&self) -> bool {
        self.role == RuntimeFunctionSemanticRole::Ordinary
            && self.invocation_effects.is_empty()
            && self.body.expression().is_some()
    }

    /// Declared invocation permissions; execution effects remain on the body.
    #[must_use]
    pub const fn invocation_effects(&self) -> &super::RuntimeEffectSet {
        &self.invocation_effects
    }

    #[must_use]
    pub const fn definition(&self) -> RuntimeFunctionDefinitionIdentity {
        self.definition
    }

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

    /// Returns the retained packet's capture and captured-parameter rows in ABI order.
    pub fn capture_inputs(&self) -> impl Iterator<Item = &RuntimeFunctionInputBinding> {
        self.inputs.iter().filter(|input| {
            matches!(
                input.source(),
                RuntimeFunctionInputSource::Capture { .. }
                    | RuntimeFunctionInputSource::CapturedParameter { .. }
            )
        })
    }

    /// Returns parameters supplied at this invocation in their checked ABI order.
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
    sites: Box<[Arc<RuntimeFunctionSite>]>,
}

impl RuntimeFunctionSiteTable {
    #[must_use]
    pub fn get(&self, id: RuntimeFunctionSiteId) -> Option<&RuntimeFunctionSite> {
        usize::try_from(id.get().get() - 1)
            .ok()
            .and_then(|index| self.sites.get(index))
            .map(Arc::as_ref)
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
        self.sites.iter().map(Arc::as_ref)
    }

    /// Iterates admitted site identities together with their original rows.
    ///
    /// # Panics
    ///
    /// Panics while iterating if a row has no representable nonzero `u32`
    /// ordinal. Such a row violates the catalog admission invariant.
    pub fn iter_with_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = (RuntimeFunctionSiteId, &RuntimeFunctionSite)> {
        self.sites.iter().enumerate().map(|(index, site)| {
            let ordinal = u32::try_from(index + 1)
                .ok()
                .and_then(NonZeroU32::new)
                .expect("admitted function-site ordinal");
            (
                RuntimeFunctionSiteId::from_accepted_ordinal(ordinal),
                site.as_ref(),
            )
        })
    }

    pub(crate) fn shared(&self, id: RuntimeFunctionSiteId) -> Option<Arc<RuntimeFunctionSite>> {
        usize::try_from(id.get().get() - 1)
            .ok()
            .and_then(|index| self.sites.get(index))
            .map(Arc::clone)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeFunctionSiteTableBuilder {
    sites: Vec<Arc<RuntimeFunctionSite>>,
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
        site: RuntimeFunctionSite,
    ) -> Result<RuntimeFunctionSiteId, RuntimeFunctionSiteError> {
        let ordinal = self
            .sites
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeFunctionSiteError::IdentityExhausted)?;
        self.sites.push(Arc::new(site));
        Ok(RuntimeFunctionSiteId::from_accepted_ordinal(ordinal))
    }

    #[must_use]
    pub(crate) fn finish(self) -> RuntimeFunctionSiteTable {
        RuntimeFunctionSiteTable {
            sites: self.sites.into_boxed_slice(),
        }
    }
}

#[cfg(test)]
mod generated_definition_tests {
    use super::*;

    #[test]
    fn generated_function_definitions_separate_parent_role_and_source_position() {
        let parent = RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x31; 32]);
        let other = RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x32; 32]);
        let mut roles = vec![
            RuntimeGeneratedFunctionRole::TaskRequest,
            RuntimeGeneratedFunctionRole::LineActivation,
            RuntimeGeneratedFunctionRole::LineAction { source_node: 0 },
            RuntimeGeneratedFunctionRole::LineAction { source_node: 1 },
            RuntimeGeneratedFunctionRole::LineCancellation { source_rule: 0 },
            RuntimeGeneratedFunctionRole::LineCancellation { source_rule: 1 },
        ];
        roles.extend(
            (0..9).map(|index| RuntimeGeneratedFunctionRole::FormatOperand {
                parameter: crate::value::RuntimeFmtParameterId::from_index(index).unwrap(),
            }),
        );
        roles.extend(
            [
                crate::line_task::ScopeExit::Completed,
                crate::line_task::ScopeExit::Cancelled,
                crate::line_task::ScopeExit::Failed,
            ]
            .map(|exit| RuntimeGeneratedFunctionRole::LineCleanup { exit }),
        );
        let definitions = roles
            .iter()
            .map(|role| parent.generated_child(*role))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(definitions.len(), roles.len());
        assert!(!definitions.contains(&parent));
        for role in roles {
            assert_eq!(
                parent.generated_child(role),
                RuntimeFunctionDefinitionIdentity::from_accepted_identity(*parent.as_bytes())
                    .generated_child(role)
            );
            assert_ne!(parent.generated_child(role), other.generated_child(role));
        }
    }
}
