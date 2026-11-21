//! Apply a conjunction of guarded points in one ordered decision walk.

use super::*;

pub(super) struct Conjuncts<'a, V> {
    pub(super) literals: BTreeMap<&'a V, bool>,
    decisions: Vec<DecisionView<'a, V>>,
}

impl<'a, V: Clone + Ord> Conjuncts<'a, V> {
    pub(super) fn new() -> Self {
        Self {
            literals: BTreeMap::new(),
            decisions: Vec::new(),
        }
    }

    pub(super) fn add<C: DecisionControl>(
        &mut self,
        decision: &'a EffectDecision<V>,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        let mut root = decision.root;
        while let Root::Branch(index) = root {
            let node = &decision.nodes[index];
            let Some((value, next)) = node.forced_edge() else {
                control.charge(DecisionWork::Node)?;
                // Every skipped head has just one surviving child. The
                // remaining graph occupies this canonical postorder slice.
                self.decisions.push(DecisionView {
                    root,
                    nodes: &decision.nodes[..=index],
                });
                return Ok(true);
            };
            match self.literals.entry(&node.variable) {
                Entry::Occupied(existing) if *existing.get() != value => return Ok(false),
                Entry::Occupied(_) => {}
                Entry::Vacant(slot) => {
                    control.charge(DecisionWork::Node)?;
                    slot.insert(value);
                }
            }
            root = next;
            if matches!(root, Root::Branch(_)) {
                control.charge(DecisionWork::Visit)?;
            }
        }
        Ok(root == Root::True)
    }

    pub(super) fn import<C: DecisionControl>(
        &self,
        builder: &mut DecisionBuilder<'_, V, C>,
    ) -> Result<Option<Vec<Root>>, C::Error> {
        let mut points = Vec::new();
        for decision in &self.decisions {
            // These assignments are still conjuncts of the returned relation.
            // Under them the residual can omit every test of a fixed variable.
            match builder
                .import_with_values(decision, |variable| self.literals.get(variable).copied())?
            {
                Root::False => return Ok(None),
                Root::True => {}
                point @ Root::Branch(_) => {
                    builder.control.charge(DecisionWork::Node)?;
                    points.push(point);
                }
            }
        }
        let mut literals = Root::True;
        for (&variable, &value) in self.literals.iter().rev() {
            let (low, high) = if value {
                (Root::False, literals)
            } else {
                (literals, Root::False)
            };
            literals = builder.branch(variable.clone(), low, high)?;
        }
        if literals != Root::True {
            builder.control.charge(DecisionWork::Node)?;
            points.push(literals);
        }
        Ok(Some(points))
    }
}

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
    pub(super) fn conjoin(&mut self, points: Vec<Root>) -> Result<Root, C::Error> {
        match points.as_slice() {
            [] => Ok(Root::True),
            [one] => Ok(*one),
            [left, right] => self.conditional(Conditional::new(*left, *right, Root::False)),
            _ => self.conjoin_ordered(points),
        }
    }

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
            suffix = match group {
                [point] => self.conditional(Conditional::new(*point, suffix, Root::False))?,
                _ if suffix == Root::True => self.conjoin_points(
                    group
                        .iter()
                        .map(|&point| GuardedPoint { guard: None, point }),
                )?,
                _ => self.conjoin_same_head_with_suffix(group, suffix)?,
            };
            if suffix == Root::False {
                break;
            }
            remaining = tail;
        }
        Ok(suffix)
    }

    fn conjoin_same_head_with_suffix(
        &mut self,
        group: &[Root],
        suffix: Root,
    ) -> Result<Root, C::Error> {
        let variable = self
            .node(group[0])
            .expect("a nonterminal conjunct")
            .variable
            .clone();
        // The suffix starts after this variable, so it holds in both
        // cofactors. Read the common head once from each input clause.
        let mut cofactors = [(); 2].map(|()| {
            vec![GuardedPoint {
                guard: None,
                point: suffix,
            }]
        });
        for &point in group {
            self.control.charge(DecisionWork::Visit)?;
            let node = self.node(point).expect("a nonterminal conjunct");
            for (clauses, point) in cofactors.iter_mut().zip([node.low, node.high]) {
                if point != Root::True {
                    clauses.push(GuardedPoint { guard: None, point });
                }
            }
        }
        let [low, high] = cofactors;
        let low = self.conjoin_points(low)?;
        let high = self.conjoin_points(high)?;
        self.branch(variable, low, high)
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
