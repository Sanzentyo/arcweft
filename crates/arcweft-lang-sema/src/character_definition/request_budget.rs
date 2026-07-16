//! Shared, request-local work accounting for character definition.

use crate::registration::{CharacterDefinitionLimitKind, CharacterDefinitionLimits};

use super::CharacterDefinitionResourceError;

/// One category in the shared character-definition request work transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionWorkKind {
    ParserFact,
    ProjectSymbolCandidate,
    TypedMemberCandidate,
    CursorFact,
    DeclarationCopy,
    SourceAdaptation,
    IdentityCheck,
    AdmittedErrorCandidate,
}

/// Opaque position in one request-local budget transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterDefinitionBudgetCheckpoint {
    consumed: u64,
    sequence_len: usize,
}

/// Ordered logical work of one successful cacheable computation.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDefinitionWorkReceipt {
    total: u64,
    sequence: Box<[CharacterDefinitionWorkKind]>,
}

impl CharacterDefinitionWorkReceipt {
    pub const fn total(&self) -> u64 {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.sequence.is_empty()
    }
}

/// Single-owner, terminal-on-error budget for one complete request.
#[must_use = "the same budget must be threaded through sema and LSP adaptation"]
#[derive(Debug)]
pub struct CharacterDefinitionRequestBudget {
    maximum: u64,
    consumed: u64,
    sequence: Vec<CharacterDefinitionWorkKind>,
    terminal: Option<CharacterDefinitionResourceError>,
}

impl CharacterDefinitionRequestBudget {
    /// Creates the sole production budget with the canonical query-work limit.
    pub fn for_request() -> Self {
        Self::with_maximum(CharacterDefinitionLimits::PRODUCTION.query_work())
    }

    pub const fn maximum(&self) -> u64 {
        self.maximum
    }

    /// Returns all admitted units, including the unit that proves one-over.
    pub const fn consumed(&self) -> u64 {
        self.consumed
    }

    pub fn checkpoint(&self) -> CharacterDefinitionBudgetCheckpoint {
        CharacterDefinitionBudgetCheckpoint {
            consumed: self.consumed,
            sequence_len: self.sequence.len(),
        }
    }

    pub fn charge(
        &mut self,
        kind: CharacterDefinitionWorkKind,
    ) -> Result<(), CharacterDefinitionResourceError> {
        if let Some(error) = &self.terminal {
            return Err(error.clone());
        }

        let Some(observed) = self.consumed.checked_add(1) else {
            return Err(self.enter_terminal(Self::arithmetic_overflow()));
        };
        let sequence_observed = match Self::checked_next_sequence_count(self.sequence.len()) {
            Ok(sequence_observed) => sequence_observed,
            Err(error) => return Err(self.enter_terminal(error)),
        };
        if sequence_observed != observed {
            return Err(self.enter_terminal(Self::arithmetic_overflow()));
        }

        self.sequence.push(kind);
        self.consumed = observed;
        if observed > self.maximum {
            return Err(
                self.enter_terminal(CharacterDefinitionResourceError::Limit {
                    kind: CharacterDefinitionLimitKind::QueryWork,
                    observed,
                    maximum: self.maximum,
                }),
            );
        }
        Ok(())
    }

    pub fn receipt_since(
        &self,
        checkpoint: CharacterDefinitionBudgetCheckpoint,
    ) -> Result<CharacterDefinitionWorkReceipt, CharacterDefinitionResourceError> {
        if let Some(error) = &self.terminal {
            return Err(error.clone());
        }
        let Some(sequence) = self.sequence.get(checkpoint.sequence_len..) else {
            return Err(Self::arithmetic_overflow());
        };
        let Some(total) = self.consumed.checked_sub(checkpoint.consumed) else {
            return Err(Self::arithmetic_overflow());
        };
        let sequence_total =
            u64::try_from(sequence.len()).map_err(|_| Self::arithmetic_overflow())?;
        if sequence_total != total {
            return Err(Self::arithmetic_overflow());
        }
        Ok(CharacterDefinitionWorkReceipt {
            total,
            sequence: sequence.into(),
        })
    }

    pub fn replay(
        &mut self,
        receipt: &CharacterDefinitionWorkReceipt,
    ) -> Result<(), CharacterDefinitionResourceError> {
        if let Some(error) = &self.terminal {
            return Err(error.clone());
        }
        let sequence_total = match u64::try_from(receipt.sequence.len()) {
            Ok(sequence_total) if sequence_total == receipt.total => sequence_total,
            Ok(_) | Err(_) => return Err(self.enter_terminal(Self::arithmetic_overflow())),
        };
        debug_assert_eq!(sequence_total, receipt.total);
        for kind in receipt.sequence.iter().copied() {
            self.charge(kind)?;
        }
        Ok(())
    }

    fn with_maximum(maximum: u64) -> Self {
        Self {
            maximum,
            consumed: 0,
            sequence: Vec::new(),
            terminal: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_maximum_for_test(maximum: u64) -> Self {
        Self::with_maximum(maximum)
    }

    fn checked_next_sequence_count<T>(count: T) -> Result<u64, CharacterDefinitionResourceError>
    where
        u64: TryFrom<T>,
    {
        u64::try_from(count)
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or_else(Self::arithmetic_overflow)
    }

    fn arithmetic_overflow() -> CharacterDefinitionResourceError {
        CharacterDefinitionResourceError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::QueryWork,
        }
    }

    fn enter_terminal(
        &mut self,
        error: CharacterDefinitionResourceError,
    ) -> CharacterDefinitionResourceError {
        self.terminal = Some(error.clone());
        error
    }
}

#[cfg(test)]
#[path = "tests/budget.rs"]
mod tests;
