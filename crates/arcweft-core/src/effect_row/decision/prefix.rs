//! Reduction over assigned prefixes of ordered effect decisions.

use super::*;

pub(super) struct DecisionPrefix<'a, V> {
    source: &'a EffectDecision<V>,
    suffix: BTreeMap<usize, Root>,
}

impl<'a, V: Clone + Ord> DecisionPrefix<'a, V> {
    pub(super) fn new(source: &'a EffectDecision<V>) -> Self {
        Self {
            source,
            suffix: BTreeMap::new(),
        }
    }

    fn node(&self, root: Root) -> Option<&'a Node<V>> {
        match root {
            Root::Branch(index) => Some(&self.source.nodes[index]),
            Root::False | Root::True => None,
        }
    }

    fn import_root<C: DecisionControl>(
        &mut self,
        root: Root,
        builder: &mut DecisionBuilder<'_, V, C>,
    ) -> Result<Root, C::Error> {
        match root {
            Root::Branch(index) => match self.suffix.get(&index) {
                Some(&mapped) => Ok(mapped),
                None => self.import_suffix(index, builder),
            },
            terminal => Ok(terminal),
        }
    }

    fn import_suffix<C: DecisionControl>(
        &mut self,
        mut index: usize,
        builder: &mut DecisionBuilder<'_, V, C>,
    ) -> Result<Root, C::Error> {
        // The caller admitted the first node. Its descendants are all in
        // the unassigned suffix, so a child-first walk can share their roots.
        let mut parents = Vec::new();
        loop {
            let node = &self.source.nodes[index];
            let next = [node.low, node.high]
                .into_iter()
                .find_map(|root| match root {
                    Root::Branch(child) if !self.suffix.contains_key(&child) => Some(child),
                    _ => None,
                });
            if let Some(child) = next {
                builder.control.charge(DecisionWork::Visit)?;
                parents.push(index);
                index = child;
                continue;
            }
            let root = builder.intern_branch(Node {
                variable: node.variable.clone(),
                low: DecisionBuilder::<V, C>::remap(node.low, &self.suffix),
                high: DecisionBuilder::<V, C>::remap(node.high, &self.suffix),
            })?;
            self.suffix.insert(index, root);
            let Some(parent) = parents.pop() else {
                return Ok(root);
            };
            index = parent;
        }
    }

    pub(super) fn select<C: DecisionControl>(
        &mut self,
        through: &V,
        case: usize,
        select: &mut impl FnMut(usize, &V) -> Result<bool, C::Error>,
        builder: &mut DecisionBuilder<'_, V, C>,
    ) -> Result<Root, C::Error> {
        let mut root = self.source.root;
        while let Root::Branch(index) = root {
            builder.control.charge(DecisionWork::Visit)?;
            if let Some(&suffix) = self.suffix.get(&index) {
                return Ok(suffix);
            }
            let node = &self.source.nodes[index];
            if node.variable > *through {
                return self.import_suffix(index, builder);
            }
            root = if select(case, &node.variable)? {
                node.high
            } else {
                node.low
            };
        }
        Ok(root)
    }
}

impl<'a, V: Clone + Ord> EffectDecisionRequirement<'a, V> {
    fn operands(&self) -> (&'a EffectDecision<V>, Option<&'a EffectDecision<V>>) {
        match *self {
            Self::Predicate(point) => (point, None),
            Self::Implication {
                premise,
                consequence,
            } => (premise, Some(consequence)),
        }
    }

    /// References used in the requirement's operands.
    pub fn variables(&self) -> impl Iterator<Item = &'a V> {
        let (first, second) = self.operands();
        first
            .variables()
            .chain(second.into_iter().flat_map(EffectDecision::variables))
    }

    /// Resolves the requirement without assigning any input references.
    pub fn decision<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<EffectDecision<V>, C::Error> {
        let (first, second) = self.operands();
        match second {
            Some(second) => first.conditional(second, &EffectDecision::constant(true), control),
            None => first.copy_with_control(control),
        }
    }

    /// Conjoins the requirements in a common decision arena. Their operands
    /// can share nodes, and no intermediate requirement graph is published.
    pub fn all<C: DecisionControl>(
        requirements: impl IntoIterator<Item = Self>,
        control: &mut C,
    ) -> Result<EffectDecision<V>, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let mut clauses = Vec::new();
        for requirement in requirements {
            builder.control.charge(DecisionWork::Visit)?;
            let (first, second) = requirement.operands();
            let point = match second {
                Some(second) => {
                    if first.is_constant(false) || second.is_constant(true) {
                        continue;
                    }
                    let premise = builder.import(first)?;
                    let consequence = builder.import(second)?;
                    builder.conditional(Conditional::new(premise, consequence, Root::True))?
                }
                None => builder.import(first)?,
            };
            match point {
                Root::False => return Ok(EffectDecision::constant(false)),
                Root::True => {}
                Root::Branch(_) => {
                    builder.control.charge(DecisionWork::Node)?;
                    clauses.push(point);
                }
            }
        }
        let result = builder.conjoin_ordered(clauses)?;
        builder.finish(result)
    }

    /// Requires this point for every assignment to references at or before `through`.
    /// References after the prefix remain free. Both operands follow the same
    /// assignment, and each reachable pair of source roots is expanded once.
    pub fn forall_prefix<C: DecisionControl>(
        &self,
        through: &V,
        control: &mut C,
    ) -> Result<EffectDecision<V>, C::Error> {
        let truth = EffectDecision::constant(true);
        let (premise, consequence) = match *self {
            Self::Predicate(point) => (&truth, point),
            Self::Implication {
                premise,
                consequence,
            } => (premise, consequence),
        };
        if premise.root == Root::False
            || consequence.root == Root::True
            || std::ptr::eq(premise, consequence)
        {
            return Ok(EffectDecision::constant(true));
        }

        let mut next = Some((premise.root, consequence.root));
        let mut premise = DecisionPrefix::new(premise);
        let mut consequence = DecisionPrefix::new(consequence);
        let mut builder = DecisionBuilder::new(control);
        let mut pending = BTreeSet::new();
        let mut clauses = Vec::new();
        while let Some((left, right)) = next.take().or_else(|| pending.pop_last()) {
            builder.control.charge(DecisionWork::Visit)?;
            if left == Root::False || right == Root::True {
                continue;
            }
            if left == Root::True && right == Root::False {
                return Ok(EffectDecision::constant(false));
            }
            let left_node = premise.node(left);
            let right_node = consequence.node(right);
            let variable = left_node
                .into_iter()
                .chain(right_node)
                .map(|node| &node.variable)
                .min();
            if let Some(variable) = variable.filter(|variable| *variable <= through) {
                let split = |root, node: Option<&Node<V>>| {
                    node.filter(|node| &node.variable == variable)
                        .map_or((root, root), |node| (node.low, node.high))
                };
                let (left_low, left_high) = split(left, left_node);
                let (right_low, right_high) = split(right, right_node);
                // Each edge decreases at least one root index and increases
                // neither. Removing the greatest pair visits every parent
                // before its children, so the pending set also deduplicates.
                for pair in [(left_low, right_low), (left_high, right_high)] {
                    if !pending.contains(&pair) {
                        builder.control.charge(DecisionWork::Node)?;
                        pending.insert(pair);
                    }
                }
                continue;
            }

            let condition = premise.import_root(left, &mut builder)?;
            let consequence = consequence.import_root(right, &mut builder)?;
            let point =
                builder.conditional(Conditional::new(condition, consequence, Root::True))?;
            if point == Root::False {
                return Ok(EffectDecision::constant(false));
            }
            if point != Root::True {
                builder.control.charge(DecisionWork::Node)?;
                clauses.push(point);
            }
        }
        let result = builder.conjoin_ordered(clauses)?;
        builder.finish(result)
    }

    /// Conjoins cases that assign every reference up to and including `through`.
    ///
    /// A guard is a free Boolean reference, or `None` for an unconditional
    /// requirement. Cases run in iterator order and may stop once the
    /// conjunction is false. Each selected suffix is shared across cases.
    /// Repeated selection of a reference in one case uses the same assignment.
    pub fn all_guarded_prefixes<C: DecisionControl>(
        &self,
        through: &V,
        cases: impl IntoIterator<Item = usize>,
        control: &mut C,
        mut select: impl FnMut(usize, &V) -> Result<bool, C::Error>,
        mut guard: impl FnMut(usize) -> Option<V>,
    ) -> Result<EffectDecision<V>, C::Error> {
        let mut cases = cases.into_iter().peekable();
        let (first, second) = self.operands();
        let trivially_true = match second {
            None => first.root == Root::True,
            Some(second) => {
                first.root == Root::False
                    || second.root == Root::True
                    || std::ptr::eq(first, second)
            }
        };
        if trivially_true || cases.peek().is_none() {
            return Ok(EffectDecision::constant(true));
        }
        let mut builder = DecisionBuilder::new(control);
        let mut first = DecisionPrefix::new(first);
        let mut second = second.map(DecisionPrefix::new);
        let mut clauses = Vec::new();
        for case in cases {
            builder.control.charge(DecisionWork::Visit)?;
            let mut point = first.select(through, case, &mut select, &mut builder)?;
            if let Some(second) = &mut second {
                if point == Root::False {
                    continue;
                }
                let consequence = second.select(through, case, &mut select, &mut builder)?;
                point = builder.conditional(Conditional::new(point, consequence, Root::True))?;
            }
            if point == Root::True {
                continue;
            }
            let guard = guard(case);
            if point == Root::False && guard.is_none() {
                return Ok(EffectDecision::constant(false));
            }
            builder.control.charge(DecisionWork::Node)?;
            clauses.push(GuardedPoint { guard, point });
        }
        let result = builder.conjoin_guarded(&clauses)?;
        builder.finish(result)
    }
}
