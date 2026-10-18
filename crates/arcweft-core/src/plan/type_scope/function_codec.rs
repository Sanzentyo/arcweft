//! Strict source codec for the existing function-type contract. The original
//! incoming scope supplies reference admission; executable functions and task
//! semantic proofs cannot be obtained from this data codec.

pub(in crate::plan) mod formula;

use super::source_codec::RuntimeTypeScopeRef;
use super::{
    RuntimeBoundEffectReference, RuntimeFunctionTypeContract, RuntimeTypeBinder,
    RuntimeTypeScopeError,
};
use crate::effect_row::{
    DecisionEncoding, EffectDecisionDeclaration, EffectDecisionNodeDeclaration,
    EffectDecisionTarget, EffectDeclarationError, EffectFormula, EffectMembershipDeclaration,
    EffectPredicate, MembershipEncoding,
};
use crate::plan::MAX_RUNTIME_PLAN_TYPE_DEPTH;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use arcweft_id::EffectId;

type Decision = EffectDecisionDeclaration<RuntimeBoundEffectReference>;
type Membership = EffectMembershipDeclaration<RuntimeBoundEffectReference>;

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeFunctionTypeSourceLimits {
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) decision_nodes: u32,
    pub(in crate::plan) overrides: u32,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeFunctionTypeSourceError {
    #[error("function type source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("unsupported function type source version {actual}")]
    Version { actual: u8 },
    #[error("invalid original function type scope: {0}")]
    Scope(#[from] RuntimeTypeScopeError),
    #[error("invalid original function effect declaration: {0}")]
    Declaration(#[from] EffectDeclarationError),
    #[error("function type source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("function type source arithmetic overflow")]
    Arithmetic,
    #[error("function type source is truncated")]
    Truncated,
    #[error("function type source has trailing data")]
    Trailing,
    #[error("unknown function effect decision tag {tag}")]
    Tag { tag: u8 },
    #[error("function effect label is not canonical UTF-8 effect source")]
    EffectLabel,
    #[error("function type source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}

impl From<TaskSemanticEncodingError> for RuntimeFunctionTypeSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy)]
struct FunctionScope<'a> {
    incoming: RuntimeTypeScopeRef<'a>,
    binder: RuntimeTypeBinder,
}

impl<'a> FunctionScope<'a> {
    fn new(
        incoming: RuntimeTypeScopeRef<'a>,
        binder: RuntimeTypeBinder,
    ) -> Result<Self, RuntimeFunctionTypeSourceError> {
        if !binder.is_empty() && incoming.binder_count() >= MAX_RUNTIME_PLAN_TYPE_DEPTH {
            return Err(RuntimeTypeScopeError::DepthLimit.into());
        }
        Ok(Self { incoming, binder })
    }

    fn validate(
        self,
        reference: RuntimeBoundEffectReference,
    ) -> Result<(), RuntimeFunctionTypeSourceError> {
        let depth = reference.depth();
        let slot = reference.slot();
        if self.binder.is_empty() {
            self.incoming.bound_effect(depth, slot)?;
        } else if depth == 0 {
            if slot >= self.binder.effects() {
                return Err(RuntimeTypeScopeError::EffectSlot {
                    slot,
                    arity: self.binder.effects(),
                }
                .into());
            }
        } else {
            self.incoming
                .bound_effect(depth - 1, slot)
                .map_err(|error| match error {
                    RuntimeTypeScopeError::UnknownDepth { .. } => {
                        RuntimeTypeScopeError::UnknownDepth { depth }
                    }
                    error => error,
                })?;
        }
        Ok(())
    }
}

struct Counts {
    limits: RuntimeFunctionTypeSourceLimits,
    storage: usize,
    nodes: usize,
    overrides: usize,
}

impl Counts {
    fn new(
        limits: RuntimeFunctionTypeSourceLimits,
        default_graphs: usize,
    ) -> Result<Self, RuntimeFunctionTypeSourceError> {
        let mut counts = Self {
            limits,
            storage: 0,
            nodes: 0,
            overrides: 0,
        };
        // Every membership has a default graph even without decision nodes.
        counts.add_storage(
            Decision::source_admission_storage(0).and_then(|n| n.checked_mul(default_graphs)),
        )?;
        Ok(counts)
    }

    fn add_storage(&mut self, bytes: Option<usize>) -> Result<(), RuntimeFunctionTypeSourceError> {
        self.storage = self
            .storage
            .checked_add(bytes.ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?)
            .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?;
        limit("decoded bytes", self.storage, self.limits.decoded_bytes)
    }

    fn nodes(&mut self, count: usize) -> Result<(), RuntimeFunctionTypeSourceError> {
        self.nodes = self
            .nodes
            .checked_add(count)
            .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?;
        limit(
            "decision nodes",
            self.nodes,
            self.limits.decision_nodes as usize,
        )?;
        self.add_storage(
            Decision::source_admission_storage(count)
                .and_then(|n| n.checked_sub(Decision::source_admission_storage(0)?)),
        )
    }

    fn overrides(&mut self, count: usize) -> Result<(), RuntimeFunctionTypeSourceError> {
        self.overrides = self
            .overrides
            .checked_add(count)
            .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?;
        limit(
            "effect overrides",
            self.overrides,
            self.limits.overrides as usize,
        )?;
        self.add_storage(Membership::source_override_storage(count))?;
        self.add_storage(Decision::source_admission_storage(0).and_then(|n| n.checked_mul(count)))
    }
}

pub(in crate::plan) struct PreparedRuntimeFunctionTypeEncoding<'a> {
    contract: &'a RuntimeFunctionTypeContract,
    scope: FunctionScope<'a>,
    limits: RuntimeFunctionTypeSourceLimits,
    encoded_bytes: usize,
    storage_bytes: usize,
    elements: usize,
}

impl PreparedRuntimeFunctionTypeEncoding<'_> {
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    pub(in crate::plan) fn append_to(
        self,
        output: &mut Vec<u8>,
    ) -> Result<(), RuntimeFunctionTypeSourceError> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(RuntimeFunctionTypeSourceError::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(Some(output), None, self.scope, Counts::new(self.limits, 2)?);
        writer.contract(self.contract)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.storage, self.storage_bytes);
        Ok(())
    }
}

/// Complete bounded source data, not graph/executable authority. Canonical
/// graph admission remains on EffectFormula/EffectPredicate during retention.
pub(in crate::plan) struct PreparedRuntimeFunctionTypeSource<'a> {
    bytes: &'a [u8],
    incoming: RuntimeTypeScopeRef<'a>,
    limits: RuntimeFunctionTypeSourceLimits,
    storage_bytes: usize,
    elements: usize,
}

impl PreparedRuntimeFunctionTypeSource<'_> {
    pub(in crate::plan) fn binder(&self) -> RuntimeTypeBinder {
        RuntimeTypeBinder::new(
            u16::from_le_bytes(
                self.bytes[1..3]
                    .try_into()
                    .expect("prepared type binder arity"),
            ),
            u16::from_le_bytes(
                self.bytes[3..5]
                    .try_into()
                    .expect("prepared const binder arity"),
            ),
            u32::from_le_bytes(
                self.bytes[5..9]
                    .try_into()
                    .expect("prepared effect binder arity"),
            ),
        )
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    pub(in crate::plan) fn materialize(
        self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeFunctionTypeContract, RuntimeFunctionTypeSourceError> {
        meter.status()?;
        let result = (|| {
            let mut reader = Reader::new(self.bytes, self.incoming, Counts::new(self.limits, 2)?);
            let contract = reader
                .contract(true)?
                .expect("retaining prepared function type source");
            debug_assert_eq!(reader.counts.storage, self.storage_bytes);
            Ok(contract)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl RuntimeFunctionTypeContract {
    pub(in crate::plan) fn prepare_source_in_frame<'a>(
        &'a self,
        incoming: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
    ) -> Result<PreparedRuntimeFunctionTypeEncoding<'a>, RuntimeFunctionTypeSourceError> {
        self.count_source_encoding(incoming.into(), limits, None)
    }

    pub(in crate::plan) fn prepare_source_encoding<'a>(
        &'a self,
        incoming: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeFunctionTypeEncoding<'a>, RuntimeFunctionTypeSourceError> {
        meter.status()?;
        self.count_source_encoding(incoming.into(), limits, Some(meter))
    }

    fn count_source_encoding<'a>(
        &'a self,
        incoming: RuntimeTypeScopeRef<'a>,
        limits: RuntimeFunctionTypeSourceLimits,
        mut meter: Option<&mut TaskSemanticMeter>,
    ) -> Result<PreparedRuntimeFunctionTypeEncoding<'a>, RuntimeFunctionTypeSourceError> {
        let result = (|| {
            if self.predicate().is_impossible() {
                return Err(RuntimeTypeScopeError::ImpossiblePredicate.into());
            }
            let scope = FunctionScope::new(incoming, self.binder())?;
            let mut writer =
                Writer::new(None, meter.as_deref_mut(), scope, Counts::new(limits, 2)?);
            writer.contract(self)?;
            let encoded_bytes = writer.length;
            let storage_bytes = writer.counts.storage;
            let elements = writer
                .counts
                .nodes
                .checked_add(writer.counts.overrides)
                .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?;
            if let Some(meter) = &mut meter {
                charge_passes(meter, encoded_bytes, 2)?;
            }
            Ok(PreparedRuntimeFunctionTypeEncoding {
                contract: self,
                scope,
                limits,
                encoded_bytes,
                storage_bytes,
                elements,
            })
        })();
        if result.is_err()
            && let Some(meter) = meter
        {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn preflight_source<'a>(
        bytes: &'a [u8],
        incoming: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeFunctionTypeSource<'a>, RuntimeFunctionTypeSourceError> {
        meter.status()?;
        let incoming = incoming.into();
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_passes(meter, bytes.len(), 3)?;
            Self::preflight_source_in_frame(bytes, incoming, limits)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn preflight_source_in_frame<'a>(
        bytes: &'a [u8],
        incoming: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
    ) -> Result<PreparedRuntimeFunctionTypeSource<'a>, RuntimeFunctionTypeSourceError> {
        let incoming = incoming.into();
        limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
        let mut reader = Reader::new(bytes, incoming, Counts::new(limits, 2)?);
        reader.contract(false)?;
        Ok(PreparedRuntimeFunctionTypeSource {
            bytes,
            incoming,
            limits,
            storage_bytes: reader.counts.storage,
            elements: reader
                .counts
                .nodes
                .checked_add(reader.counts.overrides)
                .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?,
        })
    }
}

struct Writer<'a, 'scope> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    scope: FunctionScope<'scope>,
    length: usize,
    counts: Counts,
}

impl<'a, 'scope> Writer<'a, 'scope> {
    fn new(
        output: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        scope: FunctionScope<'scope>,
        counts: Counts,
    ) -> Self {
        Self {
            output,
            meter,
            scope,
            length: 0,
            counts,
        }
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeFunctionTypeSourceError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?;
        limit(
            "encoded bytes",
            self.length,
            self.counts.limits.encoded_bytes,
        )?;
        if let Some(meter) = &mut self.meter {
            meter.charge_work(
                u64::try_from(bytes.len())
                    .map_err(|_| RuntimeFunctionTypeSourceError::Arithmetic)?,
            )?;
        }
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn contract(
        &mut self,
        contract: &RuntimeFunctionTypeContract,
    ) -> Result<(), RuntimeFunctionTypeSourceError> {
        self.tag(1)?;
        self.bytes(&contract.binder().types().to_le_bytes())?;
        self.bytes(&contract.binder().const_lengths().to_le_bytes())?;
        self.bytes(&contract.binder().effects().to_le_bytes())?;
        contract.predicate().encode(self)?;
        contract.invocation().encode(self)
    }
}

impl DecisionEncoding<RuntimeBoundEffectReference> for Writer<'_, '_> {
    type Error = RuntimeFunctionTypeSourceError;
    fn tag(&mut self, value: u8) -> Result<(), Self::Error> {
        self.bytes(&[value])
    }
    fn count(&mut self, value: usize) -> Result<(), Self::Error> {
        let count = u32::try_from(value).map_err(|_| RuntimeFunctionTypeSourceError::Arithmetic)?;
        self.bytes(&count.to_le_bytes())
    }
    fn variable(&mut self, variable: &RuntimeBoundEffectReference) -> Result<(), Self::Error> {
        self.scope.validate(*variable)?;
        self.counts.nodes(1)?;
        self.bytes(&variable.depth().to_le_bytes())?;
        self.bytes(&variable.slot().to_le_bytes())
    }
}

impl MembershipEncoding<RuntimeBoundEffectReference> for Writer<'_, '_> {
    fn effect(&mut self, effect: &EffectId) -> Result<(), Self::Error> {
        self.counts.overrides(1)?;
        self.counts.add_storage(Some(effect.as_str().len()))?;
        self.count(effect.as_str().len())?;
        self.bytes(effect.as_str().as_bytes())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    incoming: RuntimeTypeScopeRef<'a>,
    counts: Counts,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], incoming: RuntimeTypeScopeRef<'a>, counts: Counts) -> Self {
        Self {
            bytes,
            offset: 0,
            incoming,
            counts,
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RuntimeFunctionTypeSourceError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeFunctionTypeSourceError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn tag(&mut self) -> Result<u8, RuntimeFunctionTypeSourceError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, RuntimeFunctionTypeSourceError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two-byte arity"),
        ))
    }
    fn u32(&mut self) -> Result<u32, RuntimeFunctionTypeSourceError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four-byte count"),
        ))
    }
    fn count(&mut self) -> Result<usize, RuntimeFunctionTypeSourceError> {
        usize::try_from(self.u32()?).map_err(|_| RuntimeFunctionTypeSourceError::Arithmetic)
    }

    fn edge(
        &mut self,
        maximum: usize,
    ) -> Result<EffectDecisionTarget, RuntimeFunctionTypeSourceError> {
        Ok(match self.tag()? {
            0 => EffectDecisionTarget::False,
            1 => EffectDecisionTarget::True,
            2 => {
                let index = self.u32()?;
                if index as usize >= maximum {
                    return Err(EffectDeclarationError::InvalidEdge.into());
                }
                EffectDecisionTarget::Node(index)
            }
            tag => return Err(RuntimeFunctionTypeSourceError::Tag { tag }),
        })
    }

    fn decision(
        &mut self,
        scope: FunctionScope<'_>,
        materialize: bool,
    ) -> Result<Option<Decision>, RuntimeFunctionTypeSourceError> {
        let count = self.count()?;
        self.counts.nodes(count)?;
        let mut nodes = materialize.then(|| Vec::with_capacity(count));
        for ordinal in 0..count {
            let variable = RuntimeBoundEffectReference::from_coordinates(self.u32()?, self.u32()?);
            scope.validate(variable)?;
            let low = self.edge(ordinal)?;
            let high = self.edge(ordinal)?;
            if low == high {
                return Err(EffectDeclarationError::UnreducedNode.into());
            }
            if let Some(nodes) = &mut nodes {
                nodes.push(EffectDecisionNodeDeclaration {
                    variable,
                    low,
                    high,
                });
            }
        }
        let root = self.edge(count)?;
        Ok(nodes.map(|nodes| Decision {
            nodes: nodes.into_boxed_slice(),
            root,
        }))
    }

    fn membership(
        &mut self,
        scope: FunctionScope<'_>,
        materialize: bool,
    ) -> Result<Option<Membership>, RuntimeFunctionTypeSourceError> {
        let default = self.decision(scope, materialize)?;
        let count = self.count()?;
        self.counts.overrides(count)?;
        let mut overrides = materialize.then(|| Vec::with_capacity(count));
        let mut previous = None;
        for _ in 0..count {
            let length = self.count()?;
            self.counts.add_storage(Some(length))?;
            let label = std::str::from_utf8(self.take(length)?)
                .map_err(|_| RuntimeFunctionTypeSourceError::EffectLabel)?;
            EffectId::validate_source(label)
                .map_err(|_| RuntimeFunctionTypeSourceError::EffectLabel)?;
            if previous.is_some_and(|previous: &str| previous >= label) {
                return Err(EffectDeclarationError::NonCanonicalOverrides.into());
            }
            previous = Some(label);
            let decision = self.decision(scope, materialize)?;
            if let Some(overrides) = &mut overrides {
                let effect = EffectId::parse(label)
                    .map_err(|_| RuntimeFunctionTypeSourceError::EffectLabel)?;
                overrides.push((
                    effect,
                    decision.expect("retaining complete decision source"),
                ));
            }
        }
        Ok(overrides.map(|overrides| Membership {
            default: default.expect("retaining default decision source"),
            overrides: overrides.into_boxed_slice(),
        }))
    }

    fn contract(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeFunctionTypeContract>, RuntimeFunctionTypeSourceError> {
        let version = self.tag()?;
        if version != 1 {
            return Err(RuntimeFunctionTypeSourceError::Version { actual: version });
        }
        let binder = RuntimeTypeBinder::new(self.u16()?, self.u16()?, self.u32()?);
        let scope = FunctionScope::new(self.incoming, binder)?;
        let predicate = self.membership(scope, materialize)?;
        let invocation = self.membership(scope, materialize)?;
        if self.offset != self.bytes.len() {
            return Err(RuntimeFunctionTypeSourceError::Trailing);
        }
        if !materialize {
            return Ok(None);
        }
        let predicate = EffectPredicate::try_from(predicate.expect("retaining predicate source"))?;
        let invocation = EffectFormula::try_from(invocation.expect("retaining invocation source"))?;
        if predicate.is_impossible() {
            return Err(RuntimeTypeScopeError::ImpossiblePredicate.into());
        }
        Ok(Some(RuntimeFunctionTypeContract::new(
            binder, predicate, invocation,
        )))
    }
}

fn limit(
    kind: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RuntimeFunctionTypeSourceError> {
    if actual > maximum {
        return Err(RuntimeFunctionTypeSourceError::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn charge_passes(
    meter: &mut TaskSemanticMeter,
    bytes: usize,
    passes: u64,
) -> Result<(), RuntimeFunctionTypeSourceError> {
    let work = u64::try_from(bytes)
        .ok()
        .and_then(|bytes| bytes.checked_mul(passes))
        .ok_or(RuntimeFunctionTypeSourceError::Arithmetic)?;
    meter.charge_work(work)?;
    Ok(())
}

#[cfg(test)]
mod tests;
