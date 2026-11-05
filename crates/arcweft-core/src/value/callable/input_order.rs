//! Invocation provenance, independent of formal ABI storage positions.

use serde::{Deserialize, Serialize};

/// A complete permutation of input slots in their original application order.
/// Values remain in formal ABI order; retaining or projecting them carries this
/// authority with the same owner packet. An omitted binding still owns a slot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "InputOrderFields")]
pub struct RuntimeInputOrder {
    slots: Box<[u32]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputOrderFields {
    slots: Box<[u32]>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeInputOrderError {
    #[error("input order has {actual} slots, expected {expected}")]
    Arity { expected: usize, actual: usize },
    #[error("input order slot {slot} is outside its {arity} inputs")]
    OutOfRange { slot: u32, arity: usize },
    #[error("input order repeats slot {slot}")]
    Duplicate { slot: u32 },
    #[error("input order exceeds its u32 coordinate domain")]
    CoordinateOverflow,
}

impl TryFrom<InputOrderFields> for RuntimeInputOrder {
    type Error = RuntimeInputOrderError;

    fn try_from(fields: InputOrderFields) -> Result<Self, Self::Error> {
        Self::try_new(fields.slots)
    }
}

impl RuntimeInputOrder {
    pub fn try_new(slots: impl Into<Box<[u32]>>) -> Result<Self, RuntimeInputOrderError> {
        let slots = slots.into();
        u32::try_from(slots.len()).map_err(|_| RuntimeInputOrderError::CoordinateOverflow)?;
        let mut seen = vec![false; slots.len()];
        for &slot in &slots {
            let arity = slots.len();
            let present = seen
                .get_mut(slot as usize)
                .ok_or(RuntimeInputOrderError::OutOfRange { slot, arity })?;
            if std::mem::replace(present, true) {
                return Err(RuntimeInputOrderError::Duplicate { slot });
            }
        }
        Ok(Self { slots })
    }

    /// Synthetic/positional inputs have an explicit canonical order. Restoring
    /// or copying an existing invocation must use its original permutation.
    #[must_use]
    pub fn canonical(arity: u32) -> Self {
        Self {
            slots: (0..arity).collect(),
        }
    }

    pub(crate) fn for_arity(arity: usize) -> Result<Self, RuntimeInputOrderError> {
        Ok(Self::canonical(
            u32::try_from(arity).map_err(|_| RuntimeInputOrderError::CoordinateOverflow)?,
        ))
    }

    #[must_use]
    pub const fn slots(&self) -> &[u32] {
        &self.slots
    }

    pub fn validate_arity(&self, expected: usize) -> Result<(), RuntimeInputOrderError> {
        if self.slots.len() != expected {
            return Err(RuntimeInputOrderError::Arity {
                expected,
                actual: self.slots.len(),
            });
        }
        Ok(())
    }

    /// Concatenates independently admitted applications. Earlier retained
    /// inputs precede the new group without changing either formal layout.
    pub(crate) fn then(&self, following: &Self) -> Result<Self, RuntimeInputOrderError> {
        self.slots
            .len()
            .checked_add(following.slots.len())
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(RuntimeInputOrderError::CoordinateOverflow)?;
        let offset = u32::try_from(self.slots.len())
            .map_err(|_| RuntimeInputOrderError::CoordinateOverflow)?;
        let mut slots = self.slots.to_vec();
        for &slot in &following.slots {
            slots.push(
                offset
                    .checked_add(slot)
                    .ok_or(RuntimeInputOrderError::CoordinateOverflow)?,
            );
        }
        Ok(Self {
            slots: slots.into_boxed_slice(),
        })
    }

    /// Projects output slots through an admitted input mapping. Repeated
    /// unrestricted inputs retain their relative output order; value ownership
    /// admission independently decides whether such duplication is permitted.
    pub(crate) fn project(
        &self,
        sources: impl IntoIterator<Item = usize>,
    ) -> Result<Self, RuntimeInputOrderError> {
        let mut ranks = vec![0; self.slots.len()];
        for (rank, &slot) in self.slots.iter().enumerate() {
            ranks[slot as usize] = rank;
        }
        let mut projected = sources
            .into_iter()
            .enumerate()
            .map(|(output, source)| {
                let slot = u32::try_from(source)
                    .map_err(|_| RuntimeInputOrderError::CoordinateOverflow)?;
                let rank = *ranks
                    .get(source)
                    .ok_or(RuntimeInputOrderError::OutOfRange {
                        slot,
                        arity: ranks.len(),
                    })?;
                let output = u32::try_from(output)
                    .map_err(|_| RuntimeInputOrderError::CoordinateOverflow)?;
                Ok::<_, RuntimeInputOrderError>((rank, output))
            })
            .collect::<Result<Vec<_>, _>>()?;
        projected.sort_unstable();
        Ok(Self {
            slots: projected.into_iter().map(|(_, output)| output).collect(),
        })
    }

    /// Physical operand positions determine the order of logical bindings.
    /// Synthesized omissions follow supplied inputs in formal order.
    pub(crate) fn from_source_positions(
        positions: impl IntoIterator<Item = Option<u32>>,
    ) -> Result<Self, RuntimeInputOrderError> {
        let mut positions = positions
            .into_iter()
            .enumerate()
            .map(|(slot, source)| {
                Ok::<_, RuntimeInputOrderError>((
                    source,
                    u32::try_from(slot).map_err(|_| RuntimeInputOrderError::CoordinateOverflow)?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        positions.sort_unstable_by_key(|&(source, slot)| (source.is_none(), source, slot));
        Ok(Self {
            slots: positions.into_iter().map(|(_, slot)| slot).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_projects_reordered_and_repeated_inputs_without_reordering_values() {
        let order = RuntimeInputOrder::try_new([2, 0, 1]).unwrap();
        assert_eq!(order.project([1, 2, 0, 2]).unwrap().slots(), [1, 3, 2, 0]);
        assert_eq!(
            order
                .then(&RuntimeInputOrder::try_new([1, 0]).unwrap())
                .unwrap()
                .slots(),
            [2, 0, 1, 4, 3]
        );
        assert!(order.project([3]).is_err());
    }

    #[test]
    fn order_serde_requires_a_complete_unique_permutation() {
        let order = RuntimeInputOrder::try_new([1, 0]).unwrap();
        assert_eq!(
            serde_json::from_slice::<RuntimeInputOrder>(&serde_json::to_vec(&order).unwrap())
                .unwrap(),
            order
        );
        for bad in [
            r#"{"slots":[0,0]}"#,
            r#"{"slots":[2,0]}"#,
            r#"{"slots":[0],"ignored":true}"#,
            r#"{}"#,
        ] {
            assert!(
                serde_json::from_str::<RuntimeInputOrder>(bad).is_err(),
                "{bad}"
            );
        }
        assert!(order.validate_arity(1).is_err());
    }

    #[test]
    fn omissions_follow_actual_sources_without_changing_formal_slots() {
        assert_eq!(
            RuntimeInputOrder::from_source_positions([Some(3), None, Some(1), None])
                .unwrap()
                .slots(),
            [2, 0, 1, 3]
        );
    }
}
