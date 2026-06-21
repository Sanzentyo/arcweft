use std::collections::BTreeMap;

use thiserror::Error;

use crate::effects::EffectSet;

/// Type-inference variable used as the open tail of an effect row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectVar(u32);

/// Tail state of a set-like effect row.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EffectRowTail {
    Closed,
    Variable(EffectVar),
    /// Legacy/untyped callable. Calling through it is rejected until resolved.
    #[default]
    Unknown,
}

/// Set-like effect row `{ concrete | tail }`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectRow {
    concrete: EffectSet,
    tail: EffectRowTail,
}

/// Exact substitutions produced when a polymorphic callable is instantiated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectSubstitution(BTreeMap<EffectVar, EffectSet>);

/// Deterministic fresh effect-variable allocator scoped to one type check.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectVarSupply {
    next: u32,
}

/// Failure while binding or resolving an effect row.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectRowError {
    #[error("effect row is unknown and must be annotated before a dynamic call")]
    UnknownRow,
    #[error("effect variable e{variable} is unbound")]
    UnboundVariable { variable: u32 },
    #[error(
        "effect variable e{variable} was already bound to {existing}, cannot rebind it to {requested}"
    )]
    ConflictingBinding {
        variable: u32,
        existing: EffectSet,
        requested: EffectSet,
    },
}

impl EffectVar {
    pub const fn from_index(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

impl EffectRow {
    pub fn unknown() -> Self {
        Self::default()
    }

    pub fn closed(concrete: EffectSet) -> Self {
        Self {
            concrete,
            tail: EffectRowTail::Closed,
        }
    }

    pub fn open(concrete: EffectSet, tail: EffectVar) -> Self {
        Self {
            concrete,
            tail: EffectRowTail::Variable(tail),
        }
    }

    pub const fn concrete(&self) -> &EffectSet {
        &self.concrete
    }

    pub const fn tail(&self) -> EffectRowTail {
        self.tail
    }

    pub const fn is_known(&self) -> bool {
        !matches!(self.tail, EffectRowTail::Unknown)
    }

    pub fn resolve(&self, substitutions: &EffectSubstitution) -> Result<EffectSet, EffectRowError> {
        match self.tail {
            EffectRowTail::Closed => Ok(self.concrete.clone()),
            EffectRowTail::Variable(tail) => substitutions
                .get(tail)
                .map(|tail_effects| self.concrete.union(tail_effects))
                .ok_or(EffectRowError::UnboundVariable {
                    variable: tail.index(),
                }),
            EffectRowTail::Unknown => Err(EffectRowError::UnknownRow),
        }
    }
}

impl EffectSubstitution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind_exact(
        &mut self,
        variable: EffectVar,
        effects: EffectSet,
    ) -> Result<(), EffectRowError> {
        match self.0.get(&variable) {
            Some(existing) if existing != &effects => Err(EffectRowError::ConflictingBinding {
                variable: variable.index(),
                existing: existing.clone(),
                requested: effects,
            }),
            Some(_) => Ok(()),
            None => {
                self.0.insert(variable, effects);
                Ok(())
            }
        }
    }

    pub fn get(&self, variable: EffectVar) -> Option<&EffectSet> {
        self.0.get(&variable)
    }
}

impl EffectVarSupply {
    /// Allocates a fresh effect-row variable.
    ///
    /// # Panics
    ///
    /// Panics if the process exhausts the `u32` effect-variable id space.
    pub fn fresh(&mut self) -> EffectVar {
        let variable = EffectVar::from_index(self.next);
        self.next = self
            .next
            .checked_add(1)
            .expect("effect variable space exhausted");
        variable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectSet;

    #[test]
    fn resolves_a_polymorphic_effect_tail() {
        let mut supply = EffectVarSupply::default();
        let variable = supply.fresh();
        let row = EffectRow::open(
            EffectSet::from_labels(["log.write"]).expect("valid concrete row"),
            variable,
        );
        let mut substitutions = EffectSubstitution::new();
        substitutions
            .bind_exact(
                variable,
                EffectSet::from_labels(["fs.read"]).expect("valid tail row"),
            )
            .expect("fresh variable binds");
        assert_eq!(
            row.resolve(&substitutions)
                .expect("bound row resolves")
                .to_labels(),
            vec!["fs.read", "log.write"]
        );
    }

    #[test]
    fn unknown_row_fails_closed() {
        assert_eq!(
            EffectRow::unknown().resolve(&EffectSubstitution::new()),
            Err(EffectRowError::UnknownRow)
        );
    }
}
