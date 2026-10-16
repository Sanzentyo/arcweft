//! Private source grammar for a type declaration's original lexical scope.
//! Decoded scopes are data; the owning declaration still validates all bound
//! references and executable roots before the common image seal can publish.

use super::{
    RuntimeBoundConstReference, RuntimeBoundEffectReference, RuntimeBoundTypeReference,
    RuntimeTypeBinder, RuntimeTypeScope, RuntimeTypeScopeError,
};
use crate::plan::MAX_RUNTIME_PLAN_TYPE_DEPTH;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const HEADER_BYTES: usize = 1 + 4;
const BINDER_BYTES: usize = 2 + 2 + 4;

/// Read-only original scope storage. Only an admitted scope or a completely
/// checked immutable source frame can create this view; it owns no binder Vec.
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeTypeScopeRef<'a>(ScopeBinders<'a>);

#[derive(Clone, Copy)]
enum ScopeBinders<'a> {
    Retained(&'a [RuntimeTypeBinder]),
    Encoded(&'a [[u8; BINDER_BYTES]]),
}

impl<'a> From<&'a RuntimeTypeScope> for RuntimeTypeScopeRef<'a> {
    fn from(scope: &'a RuntimeTypeScope) -> Self {
        Self(ScopeBinders::Retained(scope.binders()))
    }
}

impl RuntimeTypeScopeRef<'_> {
    pub(in crate::plan) const fn binder_count(self) -> usize {
        match self.0 {
            ScopeBinders::Retained(binders) => binders.len(),
            ScopeBinders::Encoded(binders) => binders.len(),
        }
    }

    fn binder(self, depth: u32) -> Result<RuntimeTypeBinder, RuntimeTypeScopeError> {
        let index = usize::try_from(depth)
            .ok()
            .and_then(|depth| self.binder_count().checked_sub(depth.checked_add(1)?))
            .ok_or(RuntimeTypeScopeError::UnknownDepth { depth })?;
        Ok(match self.0 {
            ScopeBinders::Retained(binders) => binders[index],
            ScopeBinders::Encoded(binders) => binder(binders[index]),
        })
    }

    pub(in crate::plan) fn bound_type(
        self,
        depth: u32,
        slot: u16,
    ) -> Result<RuntimeBoundTypeReference, RuntimeTypeScopeError> {
        let arity = self.binder(depth)?.types();
        if slot >= arity {
            return Err(RuntimeTypeScopeError::TypeSlot { slot, arity });
        }
        Ok(RuntimeBoundTypeReference::from_coordinates(depth, slot))
    }

    pub(in crate::plan) fn bound_const(
        self,
        depth: u32,
        slot: u16,
    ) -> Result<RuntimeBoundConstReference, RuntimeTypeScopeError> {
        let arity = self.binder(depth)?.const_lengths();
        if slot >= arity {
            return Err(RuntimeTypeScopeError::ConstSlot { slot, arity });
        }
        Ok(RuntimeBoundConstReference::from_coordinates(depth, slot))
    }

    pub(super) fn bound_effect(
        self,
        depth: u32,
        slot: u32,
    ) -> Result<RuntimeBoundEffectReference, RuntimeTypeScopeError> {
        let arity = self.binder(depth)?.effects();
        if slot >= arity {
            return Err(RuntimeTypeScopeError::EffectSlot { slot, arity });
        }
        Ok(RuntimeBoundEffectReference::from_coordinates(depth, slot))
    }
}

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeTypeScopeCodecLimits {
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeTypeScopeCodecError {
    #[error("type scope source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("unsupported type scope source version {actual}")]
    Version { actual: u8 },
    #[error("invalid type scope source: {0}")]
    Scope(#[from] RuntimeTypeScopeError),
    #[error("type scope source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("type scope source count arithmetic overflow")]
    Arithmetic,
    #[error("type scope source is truncated")]
    Truncated,
    #[error("type scope source has trailing bytes")]
    Trailing,
    #[error("type scope source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}

impl From<TaskSemanticEncodingError> for RuntimeTypeScopeCodecError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

/// A consuming capability over the actual immutable scope. Preparing the child
/// charges its complete source passes to the parent's bank before output grows.
pub(in crate::plan) struct PreparedRuntimeTypeScopeEncoding<'a> {
    scope: &'a RuntimeTypeScope,
    encoded_bytes: usize,
    storage_bytes: usize,
}

impl PreparedRuntimeTypeScopeEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }

    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    /// Append only into capacity already reserved by the enclosing source
    /// frame. Insufficient capacity leaves the caller's prefix unchanged.
    pub(in crate::plan) fn append_to(
        self,
        output: &mut Vec<u8>,
    ) -> Result<(), RuntimeTypeScopeCodecError> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(RuntimeTypeScopeCodecError::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let count = u32::try_from(self.scope.binders().len())
            .map_err(|_| RuntimeTypeScopeCodecError::Arithmetic)?;
        output.push(1);
        output.extend_from_slice(&count.to_le_bytes());
        for binder in self.scope.binders() {
            output.extend_from_slice(&binder.types().to_le_bytes());
            output.extend_from_slice(&binder.const_lengths().to_le_bytes());
            output.extend_from_slice(&binder.effects().to_le_bytes());
        }
        Ok(())
    }
}

/// A checked borrowed frame. It owns no published type identity, reference,
/// function contract, semantic proof, or additional work bank.
pub(in crate::plan) struct ValidatedRuntimeTypeScopeSource<'a> {
    bytes: &'a [u8],
    count: usize,
    storage_bytes: usize,
}

impl ValidatedRuntimeTypeScopeSource<'_> {
    pub(in crate::plan) fn as_scope_ref(&self) -> RuntimeTypeScopeRef<'_> {
        RuntimeTypeScopeRef(ScopeBinders::Encoded(
            self.bytes[HEADER_BYTES..].as_chunks::<BINDER_BYTES>().0,
        ))
    }

    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    pub(in crate::plan) fn materialize(self) -> RuntimeTypeScope {
        let mut binders = Vec::with_capacity(self.count);
        // Whole-frame admission has checked the header, exact extent and every
        // binder. The borrowed immutable bytes cannot change between passes.
        for bytes in self.bytes[HEADER_BYTES..].as_chunks::<BINDER_BYTES>().0 {
            binders.push(binder(*bytes));
        }
        RuntimeTypeScope(binders)
    }
}

impl RuntimeTypeScope {
    /// Reuses the same child admission after the enclosing frame has prepaid
    /// all source passes. It grants no independent budget or executable proof.
    pub(in crate::plan) fn prepare_source_in_frame(
        &self,
        limits: RuntimeTypeScopeCodecLimits,
    ) -> Result<PreparedRuntimeTypeScopeEncoding<'_>, RuntimeTypeScopeCodecError> {
        let (encoded_bytes, storage_bytes) = extent(self.binders().len(), limits)?;
        if self.binders().iter().any(|binder| binder.is_empty()) {
            return Err(RuntimeTypeScopeError::EmptyBinder.into());
        }
        Ok(PreparedRuntimeTypeScopeEncoding {
            scope: self,
            encoded_bytes,
            storage_bytes,
        })
    }

    pub(in crate::plan) fn prepare_source_encoding(
        &self,
        limits: RuntimeTypeScopeCodecLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeTypeScopeEncoding<'_>, RuntimeTypeScopeCodecError> {
        meter.status()?;
        let result = (|| {
            let (encoded_bytes, _) = extent(self.binders().len(), limits)?;
            charge_passes(meter, encoded_bytes)?;
            self.prepare_source_in_frame(limits)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    /// Admit the complete original scope before allocating its binder storage.
    /// Later type-row admission must still check every reference against this
    /// scope; source decoding alone does not make a row executable.
    pub(in crate::plan) fn preflight_source<'a>(
        bytes: &'a [u8],
        limits: RuntimeTypeScopeCodecLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<ValidatedRuntimeTypeScopeSource<'a>, RuntimeTypeScopeCodecError> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_passes(meter, bytes.len())?;
            Self::preflight_source_in_frame(bytes, limits)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn preflight_source_in_frame(
        bytes: &[u8],
        limits: RuntimeTypeScopeCodecLimits,
    ) -> Result<ValidatedRuntimeTypeScopeSource<'_>, RuntimeTypeScopeCodecError> {
        limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
        let header = bytes
            .get(..HEADER_BYTES)
            .ok_or(RuntimeTypeScopeCodecError::Truncated)?;
        if header[0] != 1 {
            return Err(RuntimeTypeScopeCodecError::Version { actual: header[0] });
        }
        let count = u32::from_le_bytes(header[1..].try_into().expect("four-byte count"));
        let count = usize::try_from(count).map_err(|_| RuntimeTypeScopeCodecError::Arithmetic)?;
        let (encoded_bytes, storage_bytes) = extent(count, limits)?;
        if bytes.len() < encoded_bytes {
            return Err(RuntimeTypeScopeCodecError::Truncated);
        }
        if bytes.len() > encoded_bytes {
            return Err(RuntimeTypeScopeCodecError::Trailing);
        }
        for bytes in bytes[HEADER_BYTES..].as_chunks::<BINDER_BYTES>().0 {
            if binder(*bytes).is_empty() {
                return Err(RuntimeTypeScopeError::EmptyBinder.into());
            }
        }
        Ok(ValidatedRuntimeTypeScopeSource {
            bytes,
            count,
            storage_bytes,
        })
    }
}

fn extent(
    count: usize,
    limits: RuntimeTypeScopeCodecLimits,
) -> Result<(usize, usize), RuntimeTypeScopeCodecError> {
    if count > MAX_RUNTIME_PLAN_TYPE_DEPTH {
        return Err(RuntimeTypeScopeError::DepthLimit.into());
    }
    let encoded_bytes = count
        .checked_mul(BINDER_BYTES)
        .and_then(|bytes| bytes.checked_add(HEADER_BYTES))
        .ok_or(RuntimeTypeScopeCodecError::Arithmetic)?;
    let storage_bytes = count
        .checked_mul(size_of::<RuntimeTypeBinder>())
        .ok_or(RuntimeTypeScopeCodecError::Arithmetic)?;
    limit("encoded bytes", encoded_bytes, limits.encoded_bytes)?;
    limit("decoded bytes", storage_bytes, limits.decoded_bytes)?;
    Ok((encoded_bytes, storage_bytes))
}

fn limit(
    kind: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RuntimeTypeScopeCodecError> {
    if actual > maximum {
        return Err(RuntimeTypeScopeCodecError::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn charge_passes(
    meter: &mut TaskSemanticMeter,
    encoded_bytes: usize,
) -> Result<(), RuntimeTypeScopeCodecError> {
    let work = u64::try_from(encoded_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_mul(3))
        .ok_or(RuntimeTypeScopeCodecError::Arithmetic)?;
    meter.charge_work(work)?;
    Ok(())
}

fn binder(bytes: [u8; BINDER_BYTES]) -> RuntimeTypeBinder {
    RuntimeTypeBinder::new(
        u16::from_le_bytes(bytes[..2].try_into().expect("two-byte type arity")),
        u16::from_le_bytes(bytes[2..4].try_into().expect("two-byte const arity")),
        u32::from_le_bytes(bytes[4..8].try_into().expect("four-byte effect arity")),
    )
}

#[cfg(test)]
mod tests;
