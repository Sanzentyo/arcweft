use std::collections::BTreeMap;

use thiserror::Error;

use crate::{effect_model::CallableId, effects::EffectSet};

/// Type-inference variable used as the open tail of an effect row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectVar(u32);

/// Tail state of a set-like effect row.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum EffectRowTail {
    Closed,
    Variable(EffectVar),
    /// Legacy/untyped callable. Calling through it is rejected until resolved.
    #[default]
    Unknown,
}

/// Set-like effect row `{ concrete | tail }`.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct EffectRow {
    concrete: EffectSet,
    tail: EffectRowTail,
}

/// Exact substitutions produced when a polymorphic callable is instantiated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectSubstitution(BTreeMap<EffectVar, EffectSet>);

/// Closed or bounded effect-row evidence for one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectRowSummary {
    callable: CallableId,
    inferred: EffectRow,
    upper_bound: Option<EffectRow>,
    forbidden: EffectRow,
}

/// Stable report projection of callable effect rows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectRowReport {
    summaries: BTreeMap<CallableId, EffectRowSummary>,
}

/// Closed effect-row evidence for one callable at a crate boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedEffectRowSummary {
    callable: CallableId,
    inferred: EffectSet,
    upper_bound: Option<EffectSet>,
    forbidden: EffectSet,
}

/// Boundary-safe projection that contains only resolved effect sets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClosedEffectRowReport {
    summaries: BTreeMap<CallableId, ClosedEffectRowSummary>,
}

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

/// Failure while resolving a report into closed boundary evidence.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectRowCloseError {
    #[error("effect row report could not resolve `{callable}`: {source}")]
    Unresolved {
        callable: CallableId,
        #[source]
        source: EffectRowError,
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

    pub fn display_label(&self) -> String {
        match self.tail {
            EffectRowTail::Unknown => "unknown".to_owned(),
            EffectRowTail::Closed => format_effect_set(&self.concrete),
            EffectRowTail::Variable(variable) if self.concrete.is_empty() => {
                format!("{{ | e{} }}", variable.index())
            }
            EffectRowTail::Variable(variable) => {
                format!(
                    "{{ {} | e{} }}",
                    effect_labels(&self.concrete),
                    variable.index()
                )
            }
        }
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

impl EffectRowSummary {
    pub fn closed(
        callable: CallableId,
        inferred: EffectSet,
        upper_bound: Option<EffectSet>,
        forbidden: EffectSet,
    ) -> Self {
        Self {
            callable,
            inferred: EffectRow::closed(inferred),
            upper_bound: upper_bound.map(EffectRow::closed),
            forbidden: EffectRow::closed(forbidden),
        }
    }

    /// Records an inferred callable row whose residual tail is closed by the
    /// owning analysis substitution after fixed-point propagation.
    pub fn open_inferred(
        callable: CallableId,
        inferred: EffectSet,
        tail: EffectVar,
        upper_bound: Option<EffectSet>,
        forbidden: EffectSet,
    ) -> Self {
        Self {
            callable,
            inferred: EffectRow::open(inferred, tail),
            upper_bound: upper_bound.map(EffectRow::closed),
            forbidden: EffectRow::closed(forbidden),
        }
    }

    pub const fn callable(&self) -> &CallableId {
        &self.callable
    }

    pub const fn inferred(&self) -> &EffectRow {
        &self.inferred
    }

    pub const fn upper_bound(&self) -> Option<&EffectRow> {
        self.upper_bound.as_ref()
    }

    pub const fn forbidden(&self) -> &EffectRow {
        &self.forbidden
    }

    pub fn resolve_closed(
        &self,
        substitutions: &EffectSubstitution,
    ) -> Result<ClosedEffectRowSummary, EffectRowError> {
        Ok(ClosedEffectRowSummary::new(
            self.callable.clone(),
            self.inferred.resolve(substitutions)?,
            self.upper_bound
                .as_ref()
                .map(|row| row.resolve(substitutions))
                .transpose()?,
            self.forbidden.resolve(substitutions)?,
        ))
    }
}

impl EffectRowReport {
    pub fn new(summaries: impl IntoIterator<Item = EffectRowSummary>) -> Self {
        let summaries = summaries
            .into_iter()
            .map(|summary| (summary.callable.clone(), summary))
            .collect();
        Self { summaries }
    }

    pub fn summary(&self, callable: &CallableId) -> Option<&EffectRowSummary> {
        self.summaries.get(callable)
    }

    pub fn summaries(&self) -> impl ExactSizeIterator<Item = (&CallableId, &EffectRowSummary)> {
        self.summaries.iter()
    }

    pub fn resolve_closed(
        &self,
        substitutions: &EffectSubstitution,
    ) -> Result<ClosedEffectRowReport, EffectRowCloseError> {
        let summaries = self
            .summaries
            .values()
            .map(|summary| {
                summary
                    .resolve_closed(substitutions)
                    .map(|closed| (closed.callable.clone(), closed))
                    .map_err(|source| EffectRowCloseError::Unresolved {
                        callable: summary.callable.clone(),
                        source,
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(ClosedEffectRowReport { summaries })
    }
}

impl ClosedEffectRowSummary {
    pub fn new(
        callable: CallableId,
        inferred: EffectSet,
        upper_bound: Option<EffectSet>,
        forbidden: EffectSet,
    ) -> Self {
        Self {
            callable,
            inferred,
            upper_bound,
            forbidden,
        }
    }

    pub const fn callable(&self) -> &CallableId {
        &self.callable
    }

    pub const fn inferred(&self) -> &EffectSet {
        &self.inferred
    }

    pub const fn upper_bound(&self) -> Option<&EffectSet> {
        self.upper_bound.as_ref()
    }

    pub const fn forbidden(&self) -> &EffectSet {
        &self.forbidden
    }
}

impl ClosedEffectRowReport {
    pub fn new(summaries: impl IntoIterator<Item = ClosedEffectRowSummary>) -> Self {
        let summaries = summaries
            .into_iter()
            .map(|summary| (summary.callable.clone(), summary))
            .collect();
        Self { summaries }
    }

    pub fn summary(&self, callable: &CallableId) -> Option<&ClosedEffectRowSummary> {
        self.summaries.get(callable)
    }

    pub fn summaries(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CallableId, &ClosedEffectRowSummary)> {
        self.summaries.iter()
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

    pub(crate) fn close_fresh_inferred_tail(&mut self, variable: EffectVar) {
        let previous = self.0.insert(variable, EffectSet::new());
        debug_assert!(previous.is_none(), "fresh effect-row tail was reused");
    }

    pub fn get(&self, variable: EffectVar) -> Option<&EffectSet> {
        self.0.get(&variable)
    }
}

fn format_effect_set(effects: &EffectSet) -> String {
    let labels = effect_labels(effects);
    if labels.is_empty() {
        "{ }".to_owned()
    } else {
        format!("{{ {labels} }}")
    }
}

fn effect_labels(effects: &EffectSet) -> String {
    effects.to_labels().join(", ")
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
    fn effect_row_display_label_covers_closed_open_and_unknown_rows() {
        let variable = EffectVar::from_index(3);
        assert_eq!(EffectRow::unknown().display_label(), "unknown");
        assert_eq!(EffectRow::closed(EffectSet::new()).display_label(), "{ }");
        assert_eq!(
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).expect("valid row"))
                .display_label(),
            "{ fs.read }"
        );
        assert_eq!(
            EffectRow::open(EffectSet::new(), variable).display_label(),
            "{ | e3 }"
        );
        assert_eq!(
            EffectRow::open(
                EffectSet::from_labels(["log.write"]).expect("valid row"),
                variable
            )
            .display_label(),
            "{ log.write | e3 }"
        );
    }

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

    #[test]
    fn report_resolves_to_closed_boundary_rows() {
        let mut supply = EffectVarSupply::default();
        let variable = supply.fresh();
        let callable = CallableId::new("fn.with_open_row");
        let row = EffectRowSummary {
            callable: callable.clone(),
            inferred: EffectRow::open(
                EffectSet::from_labels(["log.write"]).expect("valid concrete row"),
                variable,
            ),
            upper_bound: Some(EffectRow::closed(
                EffectSet::from_labels(["fs.read", "log.write"]).expect("valid bound row"),
            )),
            forbidden: EffectRow::closed(EffectSet::from_labels(["net.open"]).expect("valid row")),
        };
        let mut substitutions = EffectSubstitution::new();
        substitutions
            .bind_exact(
                variable,
                EffectSet::from_labels(["fs.read"]).expect("valid tail row"),
            )
            .expect("fresh variable binds");

        let report = EffectRowReport::new([row])
            .resolve_closed(&substitutions)
            .expect("report resolves");
        let summary = report.summary(&callable).expect("callable summary");
        assert_eq!(summary.inferred().to_labels(), vec!["fs.read", "log.write"]);
        assert_eq!(
            summary.upper_bound().expect("upper bound").to_labels(),
            vec!["fs.read", "log.write"]
        );
        assert_eq!(summary.forbidden().to_labels(), vec!["net.open"]);
    }

    #[test]
    fn report_close_error_names_unresolved_callable() {
        let variable = EffectVar::from_index(7);
        let callable = CallableId::new("fn.needs_row");
        let report = EffectRowReport::new([EffectRowSummary {
            callable: callable.clone(),
            inferred: EffectRow::open(EffectSet::new(), variable),
            upper_bound: None,
            forbidden: EffectRow::closed(EffectSet::new()),
        }]);

        assert_eq!(
            report.resolve_closed(&EffectSubstitution::new()),
            Err(EffectRowCloseError::Unresolved {
                callable,
                source: EffectRowError::UnboundVariable { variable: 7 },
            })
        );
    }
}
