//! Canonical membership decisions shared by symbolic effect rows and predicates.
//!
//! An atom is a caller-owned Boolean identity, not a concrete effect label.
//! Rows and predicates use this algebra for their default class and overrides;
//! inference owners can also relate several classes in one decision. All graphs
//! are reduced, ordered and numbered by reachable low-before-high postorder.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

/// Work is charged to the surrounding semantic transaction before graph growth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionWork {
    Visit,
    Node,
}

pub trait DecisionControl {
    type Error;

    fn charge(&mut self, work: DecisionWork) -> Result<(), Self::Error>;
}

/// Primitive writes for the one canonical decision grammar. The containing
/// type/call transcript owns byte storage, reference identity and accounting.
pub trait DecisionEncoding<V> {
    type Error;
    fn tag(&mut self, value: u8) -> Result<(), Self::Error>;
    fn count(&mut self, value: usize) -> Result<(), Self::Error>;
    fn variable(&mut self, variable: &V) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum Root {
    False,
    True,
    Branch(usize),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct Node<V> {
    pub(super) variable: V,
    pub(super) low: Root,
    pub(super) high: Root,
}

/// A canonical, immutable Boolean function over caller-owned typed identities.
/// Local node indices are never accepted from another graph or from a consumer.
///
/// ```compile_fail
/// use arcweft_core::effect_row::EffectDecision;
/// let decision = EffectDecision::<u8>::constant(false);
/// let _ = decision.nodes;
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectDecision<V> {
    pub(super) nodes: Box<[Node<V>]>,
    pub(super) root: Root,
}

/// Exact projection of a relation onto its unquantified references, together
/// with its pointwise least witnesses when those witnesses satisfy the relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecisionCompletion<V> {
    pub(super) admissibility: EffectDecision<V>,
    pub(super) least: Option<BTreeMap<V, EffectDecision<V>>>,
}

/// A relation's satisfiable domain and the domain with pointwise least witnesses.
///
/// The lower envelope is a witness only where `principal()` holds. A satisfiable
/// valuation can have several incomparable witnesses, so satisfiability alone
/// never authorizes using these values as a completed solution.
#[derive(Debug, Eq, PartialEq)]
pub struct ConditionalDecisionCompletion<V> {
    satisfiable: EffectDecision<V>,
    principal: EffectDecision<V>,
    lower_envelope: BTreeMap<V, EffectDecision<V>>,
}

impl<V> ConditionalDecisionCompletion<V> {
    /// Valuations admitting at least one tuple of quantified values.
    pub const fn satisfiable(&self) -> &EffectDecision<V> {
        &self.satisfiable
    }

    /// Valuations where the coordinatewise lower envelope satisfies the relation.
    pub const fn principal(&self) -> &EffectDecision<V> {
        &self.principal
    }

    /// Coordinatewise meets of all witnesses, valid as a tuple only under `principal`.
    pub const fn lower_envelope(&self) -> &BTreeMap<V, EffectDecision<V>> {
        &self.lower_envelope
    }
}

struct CompletionRoots<V> {
    admissibility: Root,
    replacements: BTreeMap<V, Root>,
    instantiated: Root,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Conditional {
    condition: Root,
    yes: Root,
    no: Root,
}

impl Conditional {
    const fn new(condition: Root, yes: Root, no: Root) -> Self {
        Self { condition, yes, no }
    }

    fn reduced(self) -> Option<Root> {
        match self {
            Self {
                condition: Root::True,
                yes,
                ..
            } => Some(yes),
            Self {
                condition: Root::False,
                no,
                ..
            } => Some(no),
            Self { yes, no, .. } if yes == no => Some(yes),
            Self {
                condition,
                yes: Root::True,
                no: Root::False,
            } => Some(condition),
            Self {
                condition,
                yes: Root::False,
                no,
            } if condition == no => Some(Root::False),
            Self {
                condition,
                yes: Root::True,
                no,
            } if condition == no => Some(condition),
            Self {
                condition,
                yes,
                no: Root::True,
            } if condition == yes => Some(Root::True),
            Self {
                condition,
                yes,
                no: Root::False,
            } if condition == yes => Some(condition),
            _ => None,
        }
    }
}

enum ConditionalTask<V> {
    Enter(Conditional),
    Join {
        key: Conditional,
        variable: V,
        low: Conditional,
        high: Conditional,
    },
}

/// A single operation owns its private interner, memo and budget borrow.
/// A failed operation cannot alter any input or publish a partially built graph.
struct DecisionBuilder<'c, V, C> {
    nodes: Vec<Node<V>>,
    interned: BTreeMap<Node<V>, Root>,
    conditionals: BTreeMap<Conditional, Root>,
    control: &'c mut C,
}

impl<V: Clone + Ord> EffectDecision<V> {
    /// Number of immutable decision nodes available for owning admission checks.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn encode<E: DecisionEncoding<V>>(&self, encoder: &mut E) -> Result<(), E::Error> {
        fn reference<V, E: DecisionEncoding<V>>(
            root: Root,
            encoder: &mut E,
        ) -> Result<(), E::Error> {
            match root {
                Root::False => encoder.tag(0),
                Root::True => encoder.tag(1),
                Root::Branch(index) => {
                    encoder.tag(2)?;
                    encoder.count(index)
                }
            }
        }
        encoder.count(self.nodes.len())?;
        for node in &self.nodes {
            encoder.variable(&node.variable)?;
            reference(node.low, encoder)?;
            reference(node.high, encoder)?;
        }
        reference(self.root, encoder)
    }
    /// One already admitted reference; no relation or substitution is run.
    pub(super) fn reference(variable: V) -> Self {
        Self {
            nodes: Box::new([Node {
                variable,
                low: Root::False,
                high: Root::True,
            }]),
            root: Root::Branch(0),
        }
    }

    pub(super) fn single_reference(&self) -> Option<&V> {
        match (&self.root, self.nodes.as_ref()) {
            (
                Root::Branch(0),
                [
                    Node {
                        variable,
                        low: Root::False,
                        high: Root::True,
                    },
                ],
            ) => Some(variable),
            _ => None,
        }
    }
    /// Constructs a Boolean constant without allocating decision nodes.
    pub fn constant(value: bool) -> Self {
        Self {
            nodes: Box::new([]),
            root: if value { Root::True } else { Root::False },
        }
    }

    /// Admits one caller-owned Boolean variable through the supplied work control.
    pub fn variable<C: DecisionControl>(variable: V, control: &mut C) -> Result<Self, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let root = builder.branch(variable, Root::False, Root::True)?;
        builder.finish(root)
    }

    /// Tests whether the whole reduced function is the requested constant.
    pub fn is_constant(&self, value: bool) -> bool {
        self.root == if value { Root::True } else { Root::False }
    }

    pub(super) fn evaluate(&self, mut value: impl FnMut(&V) -> bool) -> bool {
        self.try_evaluate(&mut value, || Ok::<_, std::convert::Infallible>(()))
            .unwrap_or_else(|never| match never {})
    }

    /// Evaluates one valuation, charging before each visited node or terminal.
    pub fn evaluate_with_control<C: DecisionControl>(
        &self,
        mut value: impl FnMut(&V) -> bool,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        self.try_evaluate(&mut value, || control.charge(DecisionWork::Visit))
    }

    fn try_evaluate<E>(
        &self,
        value: &mut impl FnMut(&V) -> bool,
        mut visit: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, E> {
        let mut root = self.root;
        loop {
            visit()?;
            match root {
                Root::False => return Ok(false),
                Root::True => return Ok(true),
                Root::Branch(index) => {
                    let node = &self.nodes[index];
                    root = if value(&node.variable) {
                        node.high
                    } else {
                        node.low
                    };
                }
            }
        }
    }

    /// Node references in canonical order; a variable may occur in several nodes.
    pub fn variables(&self) -> impl ExactSizeIterator<Item = &V> {
        self.nodes.iter().map(|node| &node.variable)
    }

    /// Maps references through their owning scope operation. Rebuild through
    /// conditional construction because opening or reification may change the
    /// reference order or merge previously distinct references.
    pub fn map_references<U: Clone + Ord, C: DecisionControl>(
        &self,
        control: &mut C,
        mapping: &mut impl FnMut(&V, &mut C) -> Result<U, C::Error>,
    ) -> Result<EffectDecision<U>, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let mut indices = BTreeMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            builder.control.charge(DecisionWork::Visit)?;
            let variable = mapping(&node.variable, builder.control)?;
            let low = DecisionBuilder::<U, C>::remap(node.low, &indices);
            let high = DecisionBuilder::<U, C>::remap(node.high, &indices);
            let condition = builder.branch(variable, Root::False, Root::True)?;
            let root = builder.conditional(Conditional::new(condition, high, low))?;
            indices.insert(index, root);
        }
        let root = DecisionBuilder::<U, C>::remap(self.root, &indices);
        builder.finish(root)
    }

    /// `self ? yes : no` is the only Boolean construction primitive.
    pub fn conditional<C: DecisionControl>(
        &self,
        yes: &Self,
        no: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let condition = builder.import(self)?;
        let yes = builder.import(yes)?;
        let no = builder.import(no)?;
        let root = builder.conditional(Conditional::new(condition, yes, no))?;
        builder.finish(root)
    }

    /// Conjoins two functions using the shared controlled decision builder.
    pub fn and<C: DecisionControl>(&self, other: &Self, control: &mut C) -> Result<Self, C::Error> {
        self.conditional(other, &Self::constant(false), control)
    }

    /// Disjoins two functions using the shared controlled decision builder.
    pub fn or<C: DecisionControl>(&self, other: &Self, control: &mut C) -> Result<Self, C::Error> {
        self.conditional(&Self::constant(true), other, control)
    }

    /// Complements a Boolean function without interpreting it as a finite row.
    pub fn not<C: DecisionControl>(&self, control: &mut C) -> Result<Self, C::Error> {
        self.conditional(&Self::constant(false), &Self::constant(true), control)
    }

    /// Existentially eliminates the supplied caller-owned variables.
    pub fn exists<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let root = builder.import(self)?;
        let root = builder.exists(root, quantified)?;
        builder.finish(root)
    }

    /// Universally eliminates the supplied caller-owned variables.
    pub fn forall<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.not(control)?.exists(quantified, control)?.not(control)
    }

    /// Simultaneous substitution: references inside a replacement are not
    /// recursively processed by the same substitution.
    pub fn substitute<C: DecisionControl>(
        &self,
        replacements: &BTreeMap<V, Self>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let root = builder.import(self)?;
        let mut roots = BTreeMap::new();
        for (variable, replacement) in replacements {
            builder.control.charge(DecisionWork::Visit)?;
            roots.insert(variable.clone(), builder.import(replacement)?);
        }
        let root = builder.substitute(root, &roots)?;
        builder.finish(root)
    }

    /// Eliminate current existential references without selecting an arbitrary
    /// witness. `least == None` means the intersection of all solutions is not
    /// itself a solution on the admitted domain.
    ///
    /// A false admissibility root is unsatisfiable, even though its guarded
    /// witness tuple is vacuously least. The caller tests admissibility against
    /// its captured givens before accepting or retaining a residual predicate.
    pub(super) fn complete<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<DecisionCompletion<V>, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let relation = builder.import(self)?;
        let CompletionRoots {
            admissibility,
            replacements,
            instantiated,
        } = builder.completion_roots(relation, quantified)?;
        let check =
            builder.conditional(Conditional::new(admissibility, instantiated, Root::True))?;
        let least = if check == Root::True {
            let mut least = BTreeMap::new();
            for (variable, root) in replacements {
                builder.control.charge(DecisionWork::Visit)?;
                least.insert(variable, builder.finish(root)?);
            }
            Some(least)
        } else {
            None
        };
        Ok(DecisionCompletion {
            admissibility: builder.finish(admissibility)?,
            least,
        })
    }

    /// Retains exactly where a quantified relation has its pointwise least tuple.
    ///
    /// No finite-effect-set meaning is assigned to these Boolean values. The
    /// surrounding owner keeps `principal` with the envelope until its remaining
    /// variables are resolved; unsatisfiable and non-principal domains stay distinct.
    pub fn complete_conditionally<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<ConditionalDecisionCompletion<V>, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let relation = builder.import(self)?;
        let CompletionRoots {
            admissibility,
            replacements,
            instantiated,
        } = builder.completion_roots(relation, quantified)?;
        let principal =
            builder.conditional(Conditional::new(admissibility, instantiated, Root::False))?;
        let mut lower_envelope = BTreeMap::new();
        for (variable, root) in replacements {
            builder.control.charge(DecisionWork::Visit)?;
            lower_envelope.insert(variable, builder.finish(root)?);
        }
        Ok(ConditionalDecisionCompletion {
            satisfiable: builder.finish(admissibility)?,
            principal: builder.finish(principal)?,
            lower_envelope,
        })
    }
}

impl<'c, V: Clone + Ord, C: DecisionControl> DecisionBuilder<'c, V, C> {
    /// The one lower-envelope calculation shared by both completion policies.
    fn completion_roots(
        &mut self,
        relation: Root,
        quantified: &BTreeSet<V>,
    ) -> Result<CompletionRoots<V>, C::Error> {
        let admissibility = self.exists(relation, quantified)?;
        let mut replacements = BTreeMap::new();
        for variable in quantified {
            self.control.charge(DecisionWork::Visit)?;
            let atom = self.branch(variable.clone(), Root::False, Root::True)?;
            let without = self.conditional(Conditional::new(atom, Root::False, relation))?;
            let possible_without = self.exists(without, quantified)?;
            let required = self.conditional(Conditional::new(
                possible_without,
                Root::False,
                admissibility,
            ))?;
            replacements.insert(variable.clone(), required);
        }
        let instantiated = self.substitute(relation, &replacements)?;
        Ok(CompletionRoots {
            admissibility,
            replacements,
            instantiated,
        })
    }

    fn new(control: &'c mut C) -> Self {
        Self {
            nodes: Vec::new(),
            interned: BTreeMap::new(),
            conditionals: BTreeMap::new(),
            control,
        }
    }

    fn node(&self, root: Root) -> Option<&Node<V>> {
        match root {
            Root::Branch(index) => Some(&self.nodes[index]),
            Root::False | Root::True => None,
        }
    }

    fn branch(&mut self, variable: V, low: Root, high: Root) -> Result<Root, C::Error> {
        self.control.charge(DecisionWork::Visit)?;
        if low == high {
            return Ok(low);
        }
        debug_assert!(self.node(low).is_none_or(|child| child.variable > variable));
        debug_assert!(
            self.node(high)
                .is_none_or(|child| child.variable > variable)
        );
        let node = Node {
            variable,
            low,
            high,
        };
        if let Some(&root) = self.interned.get(&node) {
            return Ok(root);
        }
        self.control.charge(DecisionWork::Node)?;
        let root = Root::Branch(self.nodes.len());
        self.nodes.push(node.clone());
        self.interned.insert(node, root);
        Ok(root)
    }

    fn remap(root: Root, indices: &BTreeMap<usize, Root>) -> Root {
        match root {
            Root::Branch(index) => indices[&index],
            terminal => terminal,
        }
    }

    fn import(&mut self, decision: &EffectDecision<V>) -> Result<Root, C::Error> {
        let mut indices = BTreeMap::new();
        for (index, node) in decision.nodes.iter().enumerate() {
            self.control.charge(DecisionWork::Visit)?;
            let low = Self::remap(node.low, &indices);
            let high = Self::remap(node.high, &indices);
            indices.insert(index, self.branch(node.variable.clone(), low, high)?);
        }
        Ok(Self::remap(decision.root, &indices))
    }

    fn known(&self, key: Conditional) -> Option<Root> {
        key.reduced()
            .or_else(|| self.conditionals.get(&key).copied())
    }

    fn cofactor(&self, root: Root, variable: &V, high: bool) -> Root {
        self.node(root)
            .filter(|node| &node.variable == variable)
            .map_or(root, |node| if high { node.high } else { node.low })
    }

    fn conditional(&mut self, key: Conditional) -> Result<Root, C::Error> {
        if let Some(root) = self.known(key) {
            return Ok(root);
        }
        self.control.charge(DecisionWork::Visit)?;
        let mut pending = vec![ConditionalTask::Enter(key)];
        while let Some(task) = pending.pop() {
            self.control.charge(DecisionWork::Visit)?;
            match task {
                ConditionalTask::Enter(key) => {
                    if self.known(key).is_some() {
                        continue;
                    }
                    let variable = [key.condition, key.yes, key.no]
                        .into_iter()
                        .filter_map(|root| self.node(root).map(|node| &node.variable))
                        .min()
                        .expect("a nonreduced conditional contains a decision")
                        .clone();
                    let split = |high| {
                        Conditional::new(
                            self.cofactor(key.condition, &variable, high),
                            self.cofactor(key.yes, &variable, high),
                            self.cofactor(key.no, &variable, high),
                        )
                    };
                    let low = split(false);
                    let high = split(true);
                    if let (Some(low), Some(high)) = (self.known(low), self.known(high)) {
                        let root = self.branch(variable, low, high)?;
                        self.control.charge(DecisionWork::Visit)?;
                        self.conditionals.insert(key, root);
                        continue;
                    }
                    self.control.charge(DecisionWork::Visit)?;
                    pending.push(ConditionalTask::Join {
                        key,
                        variable,
                        low,
                        high,
                    });
                    pending.push(ConditionalTask::Enter(high));
                    pending.push(ConditionalTask::Enter(low));
                }
                ConditionalTask::Join {
                    key,
                    variable,
                    low,
                    high,
                } => {
                    let low = self
                        .known(low)
                        .expect("low conditional completed before join");
                    let high = self
                        .known(high)
                        .expect("high conditional completed before join");
                    let root = self.branch(variable, low, high)?;
                    self.control.charge(DecisionWork::Visit)?;
                    self.conditionals.insert(key, root);
                }
            }
        }
        Ok(self.known(key).expect("root conditional completed"))
    }

    /// Iterative traversal returns only the reachable child-first inventory.
    fn postorder(&mut self, root: Root) -> Result<Vec<usize>, C::Error> {
        if !matches!(root, Root::Branch(_)) {
            return Ok(Vec::new());
        }
        self.control.charge(DecisionWork::Visit)?;
        let mut pending = vec![(root, false)];
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();
        while let Some((root, joined)) = pending.pop() {
            self.control.charge(DecisionWork::Visit)?;
            let Root::Branch(index) = root else {
                continue;
            };
            if seen.contains(&index) {
                continue;
            }
            if joined {
                seen.insert(index);
                ordered.push(index);
            } else {
                let node = &self.nodes[index];
                pending.push((root, true));
                pending.push((node.high, false));
                pending.push((node.low, false));
            }
        }
        Ok(ordered)
    }

    fn exists(&mut self, root: Root, quantified: &BTreeSet<V>) -> Result<Root, C::Error> {
        let order = self.postorder(root)?;
        let mut indices = BTreeMap::new();
        for index in order {
            self.control.charge(DecisionWork::Visit)?;
            let node = self.nodes[index].clone();
            let low = Self::remap(node.low, &indices);
            let high = Self::remap(node.high, &indices);
            let mapped = if quantified.contains(&node.variable) {
                self.conditional(Conditional::new(low, Root::True, high))?
            } else {
                self.branch(node.variable, low, high)?
            };
            indices.insert(index, mapped);
        }
        Ok(Self::remap(root, &indices))
    }

    fn substitute(
        &mut self,
        root: Root,
        replacements: &BTreeMap<V, Root>,
    ) -> Result<Root, C::Error> {
        let order = self.postorder(root)?;
        let mut indices = BTreeMap::new();
        for index in order {
            self.control.charge(DecisionWork::Visit)?;
            let node = self.nodes[index].clone();
            let low = Self::remap(node.low, &indices);
            let high = Self::remap(node.high, &indices);
            let condition = match replacements.get(&node.variable) {
                Some(&replacement) => replacement,
                None => self.branch(node.variable, Root::False, Root::True)?,
            };
            let mapped = self.conditional(Conditional::new(condition, high, low))?;
            indices.insert(index, mapped);
        }
        Ok(Self::remap(root, &indices))
    }

    fn finish(&mut self, root: Root) -> Result<EffectDecision<V>, C::Error> {
        let order = self.postorder(root)?;
        let mut nodes = Vec::new();
        let mut indices = BTreeMap::new();
        for index in order {
            self.control.charge(DecisionWork::Node)?;
            let node = &self.nodes[index];
            let mapped = Node {
                variable: node.variable.clone(),
                low: Self::remap(node.low, &indices),
                high: Self::remap(node.high, &indices),
            };
            indices.insert(index, Root::Branch(nodes.len()));
            nodes.push(mapped);
        }
        Ok(EffectDecision {
            nodes: nodes.into_boxed_slice(),
            root: Self::remap(root, &indices),
        })
    }
}
