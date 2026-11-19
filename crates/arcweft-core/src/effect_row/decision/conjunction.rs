//! Apply a conjunction of guarded points in one ordered decision walk.

use super::*;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct GuardedPoint<V> {
    pub(super) point: Root,
    pub(super) guard: Option<V>,
}

enum Task<'a, V> {
    Enter(Vec<GuardedPoint<&'a V>>),
    Join {
        clauses: Vec<GuardedPoint<&'a V>>,
        variable: V,
    },
}

impl<'c, V: Clone + Ord, C: DecisionControl> DecisionBuilder<'c, V, C> {
    pub(super) fn conjoin_guarded(
        &mut self,
        clauses: &[GuardedPoint<V>],
    ) -> Result<Root, C::Error> {
        if clauses.is_empty() {
            return Ok(Root::True);
        }
        let mut initial = Vec::new();
        for clause in clauses {
            self.control.charge(DecisionWork::Visit)?;
            initial.push(GuardedPoint {
                guard: clause.guard.as_ref(),
                point: clause.point,
            });
        }
        let mut tasks = vec![Task::Enter(initial)];
        let mut memo = BTreeMap::new();
        let mut results = Vec::new();
        while let Some(task) = tasks.pop() {
            self.control.charge(DecisionWork::Visit)?;
            match task {
                Task::Enter(mut clauses) => {
                    clauses.sort_unstable();
                    // An unguarded occurrence of a point implies every
                    // guarded occurrence of that same point.
                    clauses.dedup_by(|later, earlier| {
                        later.point == earlier.point
                            && (earlier.guard.is_none() || earlier.guard == later.guard)
                    });
                    if clauses
                        .first()
                        .is_some_and(|clause| clause.guard.is_none() && clause.point == Root::False)
                    {
                        results.push(Root::False);
                        continue;
                    }
                    match clauses.as_slice() {
                        [] => results.push(Root::True),
                        [one] => results.push(match one.guard {
                            Some(guard) => {
                                self.branch_on_variable(guard.clone(), Root::True, one.point)?
                            }
                            None => one.point,
                        }),
                        [a, b] if a.guard.is_none() && b.guard.is_none() => {
                            results.push(self.conditional(Conditional::new(
                                a.point,
                                b.point,
                                Root::False,
                            ))?);
                        }
                        _ => {
                            if let Some(&root) = memo.get(&clauses) {
                                results.push(root);
                                continue;
                            }
                            let variable = self.next_guarded_variable(&clauses)?;
                            let mut low = Vec::new();
                            let mut high = Vec::new();
                            for &clause in &clauses {
                                self.control.charge(DecisionWork::Visit)?;
                                let at_guard = clause.guard == Some(&variable);
                                if !at_guard {
                                    let point = self.cofactor(clause.point, &variable, false);
                                    if point != Root::True {
                                        low.push(GuardedPoint { point, ..clause });
                                    }
                                }
                                let point = self.cofactor(clause.point, &variable, true);
                                if point != Root::True {
                                    let guard = clause.guard.filter(|guard| *guard != &variable);
                                    high.push(GuardedPoint { guard, point });
                                }
                            }
                            self.control.charge(DecisionWork::Node)?;
                            tasks.push(Task::Join { clauses, variable });
                            tasks.push(Task::Enter(high));
                            tasks.push(Task::Enter(low));
                        }
                    }
                }
                Task::Join { clauses, variable } => {
                    let high = results
                        .pop()
                        .expect("high conjunction completed before join");
                    let low = results
                        .pop()
                        .expect("low conjunction completed before join");
                    let root = self.branch(variable, low, high)?;
                    self.control.charge(DecisionWork::Node)?;
                    memo.insert(clauses, root);
                    results.push(root);
                }
            }
        }
        Ok(results.pop().expect("root conjunction completed"))
    }

    fn next_guarded_variable(&mut self, clauses: &[GuardedPoint<&V>]) -> Result<V, C::Error> {
        let mut first = None;
        for clause in clauses {
            self.control.charge(DecisionWork::Visit)?;
            let point = match clause.point {
                Root::Branch(index) => Some(&self.nodes[index].variable),
                Root::False | Root::True => None,
            };
            first = first.into_iter().chain(clause.guard).chain(point).min();
        }
        Ok(first
            .expect("an unreduced conjunction contains a reference")
            .clone())
    }
}
