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
    pub(super) fn conjoin_ordered(&mut self, mut clauses: Vec<Root>) -> Result<Root, C::Error> {
        // Attach later decisions first. Independent earlier conditions can
        // then attach to the finished suffix without rebuilding an earlier
        // prefix on each insertion.
        let head = |point| &self.node(point).expect("a nonterminal conjunct").variable;
        clauses.sort_unstable_by(|left, right| {
            head(*right).cmp(head(*left)).then_with(|| left.cmp(right))
        });
        clauses.dedup();

        let mut suffix = Root::True;
        let mut remaining = clauses.as_slice();
        while let Some(&first) = remaining.first() {
            let end = remaining.partition_point(|&point| {
                self.node(point).map(|node| &node.variable)
                    == self.node(first).map(|node| &node.variable)
            });
            let (group, tail) = remaining.split_at(end);
            let point = if let [point] = group {
                *point
            } else {
                self.conjoin_points(
                    group
                        .iter()
                        .map(|&point| GuardedPoint { guard: None, point }),
                )?
            };
            suffix = self.conditional(Conditional::new(point, suffix, Root::False))?;
            if suffix == Root::False {
                break;
            }
            remaining = tail;
        }
        Ok(suffix)
    }

    pub(super) fn conjoin_guarded(
        &mut self,
        clauses: &[GuardedPoint<V>],
    ) -> Result<Root, C::Error> {
        self.conjoin_points(clauses.iter().map(|clause| GuardedPoint {
            guard: clause.guard.as_ref(),
            point: clause.point,
        }))
    }

    fn conjoin_points<'a>(
        &mut self,
        clauses: impl IntoIterator<Item = GuardedPoint<&'a V>>,
    ) -> Result<Root, C::Error>
    where
        V: 'a,
    {
        let mut initial = Vec::new();
        for clause in clauses {
            self.control.charge(DecisionWork::Visit)?;
            initial.push(clause);
        }
        if initial.is_empty() {
            return Ok(Root::True);
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
