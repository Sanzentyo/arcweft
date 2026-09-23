//! Structural cycles are forbidden; named nominal back-edges stay finite.

use super::{AwbcProgram, AwbcTypeId, AwbcTypeProjectionError};

#[derive(Clone, Copy)]
enum State {
    Unseen,
    Visiting,
    Complete,
}

enum Step {
    Enter(AwbcTypeId),
    Leave(AwbcTypeId),
}

impl AwbcProgram {
    /// Checks each structural edge once without expanding shared type subgraphs or
    /// reconstructing source schemas from their erased executable projection.
    pub(crate) fn validate_type_graph(&self) -> Result<(), AwbcTypeProjectionError> {
        let mut states = vec![State::Unseen; self.runtime_types.len()];
        let mut pending = Vec::new();
        for root in 0..self.runtime_types.len() {
            if matches!(states[root], State::Complete) {
                continue;
            }
            let root = AwbcTypeId(u32::try_from(root).expect("AWBC runtime type IDs were bounded"));
            pending.push(Step::Enter(root));
            while let Some(step) = pending.pop() {
                match step {
                    Step::Enter(ty) => {
                        let state = states.get_mut(ty.index()).ok_or(
                            AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: ty.0 },
                        )?;
                        match state {
                            State::Visiting => {
                                return Err(AwbcTypeProjectionError::CheckedTypeCycle {
                                    index: ty.0,
                                });
                            }
                            State::Complete => continue,
                            State::Unseen => *state = State::Visiting,
                        }
                        pending.push(Step::Leave(ty));
                        let children_start = pending.len();
                        self.runtime_types[ty.index()]
                            .shape()
                            .visit_structural_type_refs(&mut |child| {
                                pending.push(Step::Enter(child))
                            });
                        pending[children_start..].reverse();
                    }
                    Step::Leave(ty) => states[ty.index()] = State::Complete,
                }
            }
        }
        Ok(())
    }
}
