//! Stable binding coordinates for admitted runtime patterns.
//!
//! [`RuntimePatternBindingCoordinate::encode`] and
//! [`RuntimePatternBindingCoordinate::decode`] are the sole canonical byte
//! grammar for this coordinate. Enclosing plan and AWBC codecs reuse this
//! grammar rather than maintaining parallel field/tag readers.

use std::num::NonZeroU32;

use thiserror::Error;

use crate::plan::RuntimeLocalDeclarationTable;
use crate::runtime_id::RuntimeLocalDeclarationId;

/// Maximum number of structural descents in one pattern binding coordinate.
pub const MAX_RUNTIME_PATTERN_BINDING_DEPTH: usize = 64;

const RUNTIME_PATTERN_BINDING_SCHEMA: u8 = 1;

/// One structural descent from a pattern root to a local binding.
///
/// Ordinals are zero-based positions in the admitted runtime pattern, not HIR
/// arena indices or source byte offsets. Record ordinals follow the retained
/// pattern-field order, including shorthand fields but excluding a rest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePatternBindingStep {
    /// The binding consumes the complete value at the current pattern root.
    Whole,
    TupleElement(u32),
    RecordField(u32),
    SequenceElement(u32),
    /// The binding consumes the unmatched suffix of a sequence pattern.
    SequenceRest,
    /// The binding is below the sole payload edge of a selected variant case.
    VariantPayload,
}

/// A validated non-empty path from one pattern root to a local binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimePatternBindingPath(Box<[RuntimePatternBindingStep]>);

/// Structural failure while constructing a pattern binding path.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePatternBindingPathError {
    #[error("runtime pattern binding path must not be empty")]
    Empty,
    #[error("runtime pattern binding path depth {actual} exceeds the maximum {maximum}")]
    TooDeep { actual: usize, maximum: usize },
    #[error("a whole-value binding step must be the sole path step")]
    WholeNotExclusive,
    #[error("a sequence-rest binding step must be unique")]
    DuplicateRest,
    #[error("a sequence-rest binding step must be terminal")]
    RestNotTerminal,
}

impl RuntimePatternBindingPath {
    /// Validates one complete structural binding path.
    pub fn try_from_steps(
        steps: impl IntoIterator<Item = RuntimePatternBindingStep>,
    ) -> Result<Self, RuntimePatternBindingPathError> {
        let steps = steps.into_iter().collect::<Box<[_]>>();
        if steps.is_empty() {
            return Err(RuntimePatternBindingPathError::Empty);
        }
        if steps.len() > MAX_RUNTIME_PATTERN_BINDING_DEPTH {
            return Err(RuntimePatternBindingPathError::TooDeep {
                actual: steps.len(),
                maximum: MAX_RUNTIME_PATTERN_BINDING_DEPTH,
            });
        }
        if steps.contains(&RuntimePatternBindingStep::Whole) && steps.len() != 1 {
            return Err(RuntimePatternBindingPathError::WholeNotExclusive);
        }
        let rest_positions = steps
            .iter()
            .enumerate()
            .filter_map(|(index, step)| {
                (*step == RuntimePatternBindingStep::SequenceRest).then_some(index)
            })
            .collect::<Box<[_]>>();
        if rest_positions.len() > 1 {
            return Err(RuntimePatternBindingPathError::DuplicateRest);
        }
        if rest_positions
            .first()
            .is_some_and(|index| index + 1 != steps.len())
        {
            return Err(RuntimePatternBindingPathError::RestNotTerminal);
        }
        Ok(Self(steps))
    }

    /// Constructs the root whole-value binding path.
    #[must_use]
    pub fn whole() -> Self {
        Self(Box::new([RuntimePatternBindingStep::Whole]))
    }

    #[must_use]
    pub const fn steps(&self) -> &[RuntimePatternBindingStep] {
        &self.0
    }
}

/// Exact plan-local destination and structural site of one pattern binding.
///
/// Final HIR patterns bind locals only. Closure captures are projected from
/// those locals by the capture owner and are deliberately not a second pattern
/// destination family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimePatternBindingCoordinate {
    local: RuntimeLocalDeclarationId,
    path: RuntimePatternBindingPath,
}

impl RuntimePatternBindingCoordinate {
    /// Binds one validated structural path to a local admitted by this plan.
    pub fn try_new(
        local: RuntimeLocalDeclarationId,
        path: RuntimePatternBindingPath,
        locals: &RuntimeLocalDeclarationTable,
    ) -> Result<Self, RuntimePatternBindingCoordinateError> {
        if !locals.contains(local) {
            return Err(RuntimePatternBindingCoordinateError::UnknownLocal { local });
        }
        Ok(Self { local, path })
    }

    #[must_use]
    pub const fn local(&self) -> RuntimeLocalDeclarationId {
        self.local
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimePatternBindingPath {
        &self.path
    }

    /// Encodes the canonical schema-1 standalone coordinate grammar.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(7 + self.path.steps().len() * 5);
        bytes.push(RUNTIME_PATTERN_BINDING_SCHEMA);
        bytes.extend_from_slice(&self.local.get().get().to_le_bytes());
        bytes.push(u8::try_from(self.path.steps().len()).unwrap_or(u8::MAX));
        for step in self.path.steps() {
            match step {
                RuntimePatternBindingStep::Whole => bytes.push(0),
                RuntimePatternBindingStep::TupleElement(ordinal) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&ordinal.to_le_bytes());
                }
                RuntimePatternBindingStep::RecordField(ordinal) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&ordinal.to_le_bytes());
                }
                RuntimePatternBindingStep::SequenceElement(ordinal) => {
                    bytes.push(3);
                    bytes.extend_from_slice(&ordinal.to_le_bytes());
                }
                RuntimePatternBindingStep::SequenceRest => bytes.push(4),
                RuntimePatternBindingStep::VariantPayload => bytes.push(5),
            }
        }
        bytes
    }

    /// Decodes and validates the canonical schema-1 standalone coordinate.
    pub fn decode(
        bytes: &[u8],
        locals: &RuntimeLocalDeclarationTable,
    ) -> Result<Self, RuntimePatternBindingWireError> {
        let mut reader = BindingReader::new(bytes);
        let schema = reader.byte()?;
        if schema != RUNTIME_PATTERN_BINDING_SCHEMA {
            return Err(RuntimePatternBindingWireError::UnsupportedSchema { schema });
        }
        let local = NonZeroU32::new(reader.u32()?)
            .ok_or(RuntimePatternBindingWireError::ZeroLocalIdentity)
            .map(RuntimeLocalDeclarationId::from_accepted_ordinal)?;
        let step_count = usize::from(reader.byte()?);
        let mut steps = Vec::with_capacity(step_count);
        for _ in 0..step_count {
            let tag = reader.byte()?;
            steps.push(match tag {
                0 => RuntimePatternBindingStep::Whole,
                1 => RuntimePatternBindingStep::TupleElement(reader.u32()?),
                2 => RuntimePatternBindingStep::RecordField(reader.u32()?),
                3 => RuntimePatternBindingStep::SequenceElement(reader.u32()?),
                4 => RuntimePatternBindingStep::SequenceRest,
                5 => RuntimePatternBindingStep::VariantPayload,
                tag => return Err(RuntimePatternBindingWireError::UnknownStepTag { tag }),
            });
        }
        if !reader.is_empty() {
            return Err(RuntimePatternBindingWireError::TrailingBytes {
                count: reader.remaining(),
            });
        }
        let path = RuntimePatternBindingPath::try_from_steps(steps)?;
        Self::try_new(local, path, locals).map_err(Into::into)
    }
}

/// Failure to correlate a structurally valid coordinate with its plan table.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePatternBindingCoordinateError {
    #[error("runtime pattern binding references unknown local declaration {local}")]
    UnknownLocal { local: RuntimeLocalDeclarationId },
}

/// Failure to decode the canonical pattern-binding coordinate grammar.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePatternBindingWireError {
    #[error("runtime pattern binding schema {schema} is unsupported; expected 1")]
    UnsupportedSchema { schema: u8 },
    #[error("runtime pattern binding wire value is truncated")]
    Truncated,
    #[error("runtime pattern binding local identity must be nonzero")]
    ZeroLocalIdentity,
    #[error("runtime pattern binding step tag {tag} is unknown")]
    UnknownStepTag { tag: u8 },
    #[error("runtime pattern binding wire value has {count} trailing bytes")]
    TrailingBytes { count: usize },
    #[error(transparent)]
    InvalidPath(#[from] RuntimePatternBindingPathError),
    #[error(transparent)]
    InvalidCoordinate(#[from] RuntimePatternBindingCoordinateError),
}

struct BindingReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BindingReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn byte(&mut self) -> Result<u8, RuntimePatternBindingWireError> {
        let byte = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(RuntimePatternBindingWireError::Truncated)?;
        self.offset += 1;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, RuntimePatternBindingWireError> {
        let end = self
            .offset
            .checked_add(4)
            .ok_or(RuntimePatternBindingWireError::Truncated)?;
        let bytes: [u8; 4] = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimePatternBindingWireError::Truncated)?
            .try_into()
            .expect("the exact four-byte slice was checked");
        self.offset = end;
        Ok(u32::from_le_bytes(bytes))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::RuntimeLocalDeclarationTableBuilder;

    fn locals() -> (RuntimeLocalDeclarationId, RuntimeLocalDeclarationTable) {
        let mut builder = RuntimeLocalDeclarationTableBuilder::new();
        let local = builder.push().expect("first local identity");
        (local, builder.finish())
    }

    #[test]
    fn coordinate_round_trips_the_exact_schema_one_grammar() {
        let (local, locals) = locals();
        let coordinate = RuntimePatternBindingCoordinate::try_new(
            local,
            RuntimePatternBindingPath::try_from_steps([
                RuntimePatternBindingStep::TupleElement(2),
                RuntimePatternBindingStep::RecordField(7),
                RuntimePatternBindingStep::VariantPayload,
                RuntimePatternBindingStep::SequenceRest,
            ])
            .expect("valid nested path"),
            &locals,
        )
        .expect("local belongs to table");
        let expected = [1, 1, 0, 0, 0, 4, 1, 2, 0, 0, 0, 2, 7, 0, 0, 0, 5, 4];

        assert_eq!(coordinate.encode(), expected);
        assert_eq!(
            RuntimePatternBindingCoordinate::decode(&expected, &locals),
            Ok(coordinate)
        );
    }

    #[test]
    fn path_validation_has_deterministic_precedence() {
        assert_eq!(
            RuntimePatternBindingPath::try_from_steps([]),
            Err(RuntimePatternBindingPathError::Empty)
        );
        assert_eq!(
            RuntimePatternBindingPath::try_from_steps([
                RuntimePatternBindingStep::Whole,
                RuntimePatternBindingStep::TupleElement(0),
            ]),
            Err(RuntimePatternBindingPathError::WholeNotExclusive)
        );
        assert_eq!(
            RuntimePatternBindingPath::try_from_steps([
                RuntimePatternBindingStep::SequenceRest,
                RuntimePatternBindingStep::SequenceRest,
            ]),
            Err(RuntimePatternBindingPathError::DuplicateRest)
        );
        assert_eq!(
            RuntimePatternBindingPath::try_from_steps([
                RuntimePatternBindingStep::SequenceRest,
                RuntimePatternBindingStep::TupleElement(0),
            ]),
            Err(RuntimePatternBindingPathError::RestNotTerminal)
        );
    }

    #[test]
    fn wire_decoder_rejects_noncanonical_or_invalid_inputs() {
        let (_, locals) = locals();
        assert_eq!(
            RuntimePatternBindingCoordinate::decode(&[2], &locals),
            Err(RuntimePatternBindingWireError::UnsupportedSchema { schema: 2 })
        );
        assert_eq!(
            RuntimePatternBindingCoordinate::decode(&[1, 0, 0, 0, 0, 1, 0], &locals),
            Err(RuntimePatternBindingWireError::ZeroLocalIdentity)
        );
        assert_eq!(
            RuntimePatternBindingCoordinate::decode(&[1, 1, 0, 0, 0, 1, 9], &locals),
            Err(RuntimePatternBindingWireError::UnknownStepTag { tag: 9 })
        );
        assert_eq!(
            RuntimePatternBindingCoordinate::decode(&[1, 1, 0, 0, 0, 0], &locals),
            Err(RuntimePatternBindingWireError::InvalidPath(
                RuntimePatternBindingPathError::Empty
            ))
        );
    }
}
