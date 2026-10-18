//! Original pattern source data, without a completed executable proof.
//! Prefix rows are counted iteratively, then indexed and consumed in reverse
//! into the existing seed algebra under the actual construction issuer.

mod counts;
mod cursor;
mod decoding;
mod encoding;

#[cfg(test)]
mod tests;

use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::{RuntimeLiteralSourceError, RuntimeLiteralSourceLimits};
use counts::Counts;
use cursor::{Cursor, Kind, Row, Tag, WireRest};

const DOMAIN: &[u8] = b"arcweft.runtime.pattern-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimePatternSourceLimits {
    pub(in crate::plan) nodes: u32,
    pub(in crate::plan) elements: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
    pub(in crate::plan) local_count: usize,
}

impl RuntimePatternSourceLimits {
    fn literal(self) -> RuntimeLiteralSourceLimits {
        RuntimeLiteralSourceLimits {
            encoded_bytes: self.encoded_bytes,
            decoded_bytes: self.decoded_bytes,
            elements: self.elements,
        }
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimePatternSourceError {
    #[error("pattern source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid pattern source domain")]
    Domain,
    #[error("unsupported pattern source version {actual}")]
    Version { actual: u8 },
    #[error("pattern source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("pattern source arithmetic overflow")]
    Arithmetic,
    #[error("pattern source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("pattern source has trailing data at {offset}")]
    Trailing { offset: usize },
    #[error("unknown pattern source tag {tag}")]
    Tag { tag: u8 },
    #[error("pattern source {kind} reference {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("pattern source flag {actual} is not zero or one")]
    Flag { actual: u8 },
    #[error("pattern source has {actual} roots instead of one")]
    Roots { actual: usize },
    #[error("pattern source Or has {actual} alternatives instead of at least two")]
    OrArity { actual: usize },
    #[error("pattern source ends with {remaining} unfilled child roles")]
    MissingChildren { remaining: usize },
    #[error("pattern source node needs {children} operands but has {available}")]
    Operands { children: usize, available: usize },
    #[error("pattern source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("pattern source entity row contains another value family")]
    Entity,
    #[error("pattern source binding coordinate does not match its structural position")]
    Coordinate,
    #[error("pattern source literal is invalid: {0}")]
    Literal(#[from] RuntimeLiteralSourceError),
    #[error("pattern source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}

impl From<TaskSemanticEncodingError> for RuntimePatternSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

impl RuntimePatternSourceError {
    fn reject_in_bank(self, meter: &mut TaskSemanticMeter) -> Self {
        let error = match meter.status() {
            Err(cause) if cause != TaskSemanticEncodingError::OwnerRejected => {
                Self::Resource(cause)
            }
            _ => self,
        };
        meter.reject_owner();
        error
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    retained: usize,
    nodes: usize,
    elements: usize,
    bindings: usize,
    binding_steps: usize,
    record_fields: usize,
    literals: usize,
}

/// Borrowed complete framing; no decoded pattern, binding or proof is retained.
pub(in crate::plan) struct PreparedRuntimePatternSource<'a> {
    bytes: &'a [u8],
    limits: RuntimePatternSourceLimits,
    footprint: Footprint,
    storage_bytes: usize,
}

impl PreparedRuntimePatternSource<'_> {
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
}

impl RuntimePattern {
    pub(in crate::plan) fn preflight_sources<'a>(
        bytes: &'a [u8],
        limits: RuntimePatternSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimePatternSource<'a>, RuntimePatternSourceError> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = Cursor::new(bytes, limits);
            let nodes = cursor.header()?;
            let mut counts = Counts::new(limits, nodes)?;
            // The header's complete scratch requirement is checked first.
            let mut scopes = Vec::with_capacity(nodes);
            scopes.push(PrefixScope {
                remaining: 1,
                depth: 0,
            });
            for _ in 0..nodes {
                while scopes.last().is_some_and(|scope| scope.remaining == 0) {
                    scopes.pop();
                }
                let Some(scope) = scopes.last_mut() else {
                    return Err(RuntimePatternSourceError::Roots { actual: 2 });
                };
                scope.remaining -= 1;
                let depth = scope.depth;
                let row = cursor.row()?;
                counts.row(&row, depth, resources, meter)?;
                let children = row.kind.children();
                if children > 0 {
                    scopes.push(PrefixScope {
                        remaining: children,
                        depth: depth
                            .checked_add(usize::from(row.kind.descends()))
                            .ok_or(RuntimePatternSourceError::Arithmetic)?,
                    });
                }
            }
            let remaining = scopes
                .iter()
                .try_fold(0usize, |sum, scope| sum.checked_add(scope.remaining))
                .ok_or(RuntimePatternSourceError::Arithmetic)?;
            if remaining != 0 {
                return Err(RuntimePatternSourceError::MissingChildren { remaining });
            }
            cursor.finish()?;
            Ok(PreparedRuntimePatternSource {
                bytes,
                limits,
                storage_bytes: counts.storage_bytes()?,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: RuntimePatternSourceError| error.reject_in_bank(meter))
    }
}

struct PrefixScope {
    remaining: usize,
    depth: usize,
}

fn limit(
    kind: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RuntimePatternSourceError> {
    if actual > maximum {
        return Err(RuntimePatternSourceError::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn reference(
    kind: &'static str,
    actual: u32,
    maximum: usize,
) -> Result<(), RuntimePatternSourceError> {
    if actual == 0 || actual as usize > maximum {
        return Err(RuntimePatternSourceError::Reference {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), RuntimePatternSourceError> {
    let bytes = u64::try_from(bytes).map_err(|_| RuntimePatternSourceError::Arithmetic)?;
    meter.charge_work(
        bytes
            .checked_mul(3)
            .ok_or(RuntimePatternSourceError::Arithmetic)?,
    )?;
    Ok(())
}
