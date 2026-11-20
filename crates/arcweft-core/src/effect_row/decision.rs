//! Canonical membership decisions shared by symbolic effect rows and predicates.
//!
//! An atom is a caller-owned Boolean identity, not a concrete effect label.
//! Rows and predicates use this algebra for their default class and overrides;
//! inference owners can also relate several classes in one decision. All graphs
//! are reduced, ordered and numbered by reachable low-before-high postorder.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap, btree_map::Entry},
    sync::Arc,
};

mod conjunction;
use conjunction::{Conjuncts, GuardedPoint};
mod prefix;
use prefix::DecisionPrefix;

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

impl<V> Node<V> {
    fn literal_polarity(&self) -> Option<bool> {
        match (self.low, self.high) {
            (Root::False, Root::True) => Some(true),
            (Root::True, Root::False) => Some(false),
            _ => None,
        }
    }
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

struct DecisionView<'a, V> {
    nodes: &'a [Node<V>],
    root: Root,
}

impl<V> DecisionView<'_, V> {
    fn try_evaluate<E>(
        &self,
        mut value: impl FnMut(&V) -> Result<bool, E>,
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
                    root = if value(&node.variable)? {
                        node.high
                    } else {
                        node.low
                    };
                }
            }
        }
    }

    fn after_forced_prefix<C: DecisionControl>(&self, control: &mut C) -> Result<Self, C::Error> {
        let mut root = self.root;
        while let Root::Branch(index) = root {
            control.charge(DecisionWork::Visit)?;
            let node = &self.nodes[index];
            root = match (node.low, node.high) {
                (Root::False, high) => high,
                (low, Root::False) => low,
                _ => break,
            };
        }
        // Each skipped node has only one accepting edge. Its descendants
        // occupy the entire canonical postorder preceding that node.
        let end = match root {
            Root::Branch(index) => index + 1,
            Root::True | Root::False => 0,
        };
        Ok(Self {
            nodes: &self.nodes[..end],
            root,
        })
    }
}

/// A simultaneous replacement for one source reference in a decision.
pub enum EffectDecisionReplacement<'a, V> {
    Constant(bool),
    Variable(V),
    Decision(&'a EffectDecision<V>),
}

/// A Boolean point requirement before restricting its input references.
pub enum EffectDecisionRequirement<'a, V> {
    Predicate(&'a EffectDecision<V>),
    Implication {
        premise: &'a EffectDecision<V>,
        consequence: &'a EffectDecision<V>,
    },
}

impl<V> EffectDecisionReplacement<'_, V> {
    fn literal(&self) -> Option<(&V, bool)> {
        match self {
            Self::Constant(_) => None,
            Self::Variable(variable) => Some((variable, true)),
            Self::Decision(decision) => decision.literal(),
        }
    }

    fn first_reference(&self) -> Option<&V> {
        match self {
            Self::Constant(_) => None,
            Self::Variable(variable) => Some(variable),
            Self::Decision(decision) => match decision.root {
                Root::Branch(index) => Some(&decision.nodes[index].variable),
                Root::False | Root::True => None,
            },
        }
    }
}

/// Each retained equality can assign a fresh variable to a function of
/// already available inputs. Any earlier equality remains satisfiable after
/// this assignment because it did not read the newly assigned variable.
struct DeferredEqualities<V> {
    mentioned: BTreeSet<V>,
    points: Vec<Root>,
}

impl<V: Clone + Ord> DeferredEqualities<V> {
    fn new() -> Self {
        Self {
            mentioned: BTreeSet::new(),
            points: Vec::new(),
        }
    }

    fn admit<C: DecisionControl>(
        &mut self,
        pair: &[EffectDecisionReplacement<'_, V>; 2],
        control: &mut C,
    ) -> Result<bool, C::Error> {
        for (output, input) in [(&pair[0], &pair[1]), (&pair[1], &pair[0])] {
            if let Some((variable, _)) = output.literal()
                && !self.mentioned.contains(variable)
            {
                if !self.record_input(input, Some(variable), control)? {
                    return Ok(false);
                }
                self.remember(variable, control)?;
                return Ok(true);
            }
        }
        // A single nonterminal condition is satisfiable. It can supply the
        // initial input domain, provided later definitions do not change any
        // of its references. The caller still handles constant false eagerly.
        if self.points.is_empty() {
            self.record_input(&pair[0], None, control)?;
            self.record_input(&pair[1], None, control)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn record_input<C: DecisionControl>(
        &mut self,
        input: &EffectDecisionReplacement<'_, V>,
        output: Option<&V>,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        if let Some((variable, _)) = input.literal() {
            if Some(variable) == output {
                return Ok(false);
            }
            self.remember(variable, control)?;
        } else if let EffectDecisionReplacement::Decision(decision) = input {
            for variable in decision.variables() {
                control.charge(DecisionWork::Visit)?;
                if Some(variable) == output {
                    return Ok(false);
                }
                self.remember(variable, control)?;
            }
        }
        Ok(true)
    }

    fn remember<C: DecisionControl>(
        &mut self,
        variable: &V,
        control: &mut C,
    ) -> Result<(), C::Error> {
        if !self.mentioned.contains(variable) {
            control.charge(DecisionWork::Node)?;
            self.mentioned.insert(variable.clone());
        }
        Ok(())
    }
}

impl<V> EffectDecision<V> {
    fn view(&self) -> DecisionView<'_, V> {
        DecisionView {
            nodes: &self.nodes,
            root: self.root,
        }
    }

    fn literal(&self) -> Option<(&V, bool)> {
        match (self.root, self.nodes.as_ref()) {
            (Root::Branch(0), [node]) => Some((&node.variable, node.literal_polarity()?)),
            _ => None,
        }
    }
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
    lower_envelope: DecisionLowerEnvelope<V>,
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
    pub const fn lower_envelope(&self) -> &DecisionLowerEnvelope<V> {
        &self.lower_envelope
    }

    /// Moves the satisfiable domain, principal domain, and coordinatewise meet.
    pub fn into_parts(
        self,
    ) -> (
        EffectDecision<V>,
        EffectDecision<V>,
        DecisionLowerEnvelope<V>,
    ) {
        (self.satisfiable, self.principal, self.lower_envelope)
    }
}

/// A total tuple over the admitted coordinates, stored by nonzero support.
/// An admitted coordinate with no stored decision is always false. A reference
/// outside the bound domain is not a coefficient of this tuple. Coordinates
/// derived from the same canonical root share its immutable decision.
#[derive(Debug, Eq, PartialEq)]
pub struct DecisionLowerEnvelope<V> {
    domain: Arc<BTreeSet<V>>,
    nonzero: BTreeMap<V, Arc<EffectDecision<V>>>,
    zero: EffectDecision<V>,
}

impl<V: Clone + Ord> DecisionLowerEnvelope<V> {
    /// All coordinates admitted for minimization, including constant false values.
    pub fn domain(&self) -> &BTreeSet<V> {
        &self.domain
    }

    /// The coordinate's decision, or `None` outside the admitted domain.
    pub fn get(&self, coordinate: &V) -> Option<&EffectDecision<V>> {
        self.nonzero
            .get(coordinate)
            .map(Arc::as_ref)
            .or_else(|| self.domain.contains(coordinate).then_some(&self.zero))
    }

    /// Simultaneously instantiates the bound coordinates in a relation.
    pub fn instantiate<C: DecisionControl>(
        &self,
        relation: &EffectDecision<V>,
        control: &mut C,
    ) -> Result<EffectDecision<V>, C::Error> {
        if self.domain.is_empty() || relation.node_count() == 0 {
            return relation.copy_with_control(control);
        }
        relation.substitute_references(control, |coordinate, _| {
            Ok(match self.get(coordinate) {
                Some(value) => EffectDecisionReplacement::Decision(value),
                None => EffectDecisionReplacement::Variable(coordinate.clone()),
            })
        })
    }

    /// Fixes free references to Boolean values while retaining the bound domain.
    pub fn specialize<C: DecisionControl>(
        &self,
        assignment: &BTreeMap<V, bool>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let mut nonzero = BTreeMap::new();
        // Local input identity only. Allocation addresses never enter the tuple
        // or determine the order in which its coordinates are processed.
        let mut specialized = BTreeMap::<_, Option<_>>::new();
        for (coordinate, value) in &self.nonzero {
            control.charge(DecisionWork::Visit)?;
            let value = if assignment.is_empty() {
                Some(value)
            } else {
                match specialized.entry(Arc::as_ptr(value)) {
                    Entry::Occupied(entry) => entry.into_mut().as_ref(),
                    Entry::Vacant(entry) => {
                        let restricted = value.restrict(assignment, control)?;
                        control.charge(DecisionWork::Node)?;
                        entry
                            .insert((!restricted.is_constant(false)).then(|| Arc::new(restricted)))
                            .as_ref()
                    }
                }
            };
            if let Some(value) = value {
                control.charge(DecisionWork::Node)?;
                nonzero.insert(coordinate.clone(), Arc::clone(value));
            }
        }
        control.charge(DecisionWork::Node)?;
        Ok(Self {
            domain: Arc::clone(&self.domain),
            nonzero,
            zero: EffectDecision::constant(false),
        })
    }

    /// Whether the tuple has any coefficient that still depends on a reference.
    pub fn is_closed<C: DecisionControl>(&self, control: &mut C) -> Result<bool, C::Error> {
        for value in self.nonzero.values() {
            control.charge(DecisionWork::Visit)?;
            if value.node_count() != 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

struct CompletionRoots<V> {
    admissibility: Root,
    replacements: BTreeMap<V, Root>,
    instantiated: Root,
}

#[derive(Clone, Copy)]
enum Quantifier {
    Exists,
    ForAll,
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

#[derive(Clone, Copy, Default)]
struct CofactorChange {
    sweep: usize,
    result: Option<Root>,
}

/// Reusable storage for one false cofactor at a time. Scheduling each source
/// index at most once per sweep bounds both tables by the source node count.
struct CofactorSweep {
    parents: Vec<Vec<usize>>,
    changes: Vec<CofactorChange>,
    pending: BinaryHeap<Reverse<usize>>,
    number: usize,
}

impl CofactorSweep {
    fn new<C: DecisionControl>(
        parents: Vec<Vec<usize>>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        let count = parents.len();
        for _ in 0..count {
            control.charge(DecisionWork::Node)?;
            control.charge(DecisionWork::Node)?;
        }
        Ok(Self {
            parents,
            changes: vec![CofactorChange::default(); count],
            pending: BinaryHeap::with_capacity(count),
            number: 0,
        })
    }

    fn schedule(&mut self, index: usize) {
        let change = &mut self.changes[index];
        if change.sweep != self.number {
            *change = CofactorChange {
                sweep: self.number,
                result: None,
            };
            self.pending.push(Reverse(index));
        }
    }

    fn projection(&self, root: Root, ordinary: &[Root]) -> Root {
        match root {
            Root::Branch(index) => {
                let change = &self.changes[index];
                if change.sweep == self.number {
                    change.result.unwrap_or(ordinary[index])
                } else {
                    ordinary[index]
                }
            }
            terminal => terminal,
        }
    }
}

impl<V: Clone + Ord> EffectDecision<V> {
    /// Number of immutable decision nodes available for owning admission checks.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn admit_copy<C: DecisionControl>(&self, control: &mut C) -> Result<(), C::Error> {
        control.charge(DecisionWork::Visit)?;
        for _ in self.nodes.iter() {
            control.charge(DecisionWork::Node)?;
        }
        Ok(())
    }

    /// Copy an immutable canonical graph after admitting its root and nodes.
    pub fn copy_with_control<C: DecisionControl>(&self, control: &mut C) -> Result<Self, C::Error> {
        self.admit_copy(control)?;
        Ok(self.clone())
    }

    /// Canonical postorder makes the roots and corresponding nodes sufficient
    /// for equality. Charge each comparison before inspecting the pair.
    pub(super) fn equal_with<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        control.charge(DecisionWork::Visit)?;
        if self.root != other.root || self.nodes.len() != other.nodes.len() {
            return Ok(false);
        }
        for (left, right) in self.nodes.iter().zip(&other.nodes) {
            control.charge(DecisionWork::Visit)?;
            if left != right {
                return Ok(false);
            }
        }
        Ok(true)
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
        control.charge(DecisionWork::Node)?;
        Ok(Self::reference(variable))
    }

    /// Tests whether the whole reduced function is the requested constant.
    pub fn is_constant(&self, value: bool) -> bool {
        self.root == if value { Root::True } else { Root::False }
    }

    pub(super) fn evaluate(&self, mut value: impl FnMut(&V) -> bool) -> bool {
        self.try_evaluate(
            |reference| Ok(value(reference)),
            || Ok::<_, std::convert::Infallible>(()),
        )
        .unwrap_or_else(|never| match never {})
    }

    /// Evaluates one valuation, charging before each visited node or terminal.
    pub fn evaluate_with_control<C: DecisionControl>(
        &self,
        mut value: impl FnMut(&V) -> bool,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        self.try_evaluate_with_control(|reference| Ok(value(reference)), control)
    }

    /// Evaluates a valuation that may reject an encountered reference.
    /// The path is visited in decision order, with each step charged first.
    pub fn try_evaluate_with_control<C: DecisionControl>(
        &self,
        value: impl FnMut(&V) -> Result<bool, C::Error>,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        self.try_evaluate(value, || control.charge(DecisionWork::Visit))
    }

    fn try_evaluate<E>(
        &self,
        value: impl FnMut(&V) -> Result<bool, E>,
        visit: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, E> {
        self.view().try_evaluate(value, visit)
    }

    /// Node references in canonical order; a variable may occur in several nodes.
    pub fn variables(&self) -> impl ExactSizeIterator<Item = &V> {
        self.nodes.iter().map(|node| &node.variable)
    }

    /// Maps references through their owning scope operation. The same reduced
    /// builder restores order when opening or reification changes it.
    pub fn map_references<U: Clone + Ord, C: DecisionControl>(
        &self,
        control: &mut C,
        mapping: &mut impl FnMut(&V, &mut C) -> Result<U, C::Error>,
    ) -> Result<EffectDecision<U>, C::Error> {
        if let [node] = self.nodes.as_ref() {
            control.charge(DecisionWork::Visit)?;
            let variable = mapping(&node.variable, control)?;
            control.charge(DecisionWork::Node)?;
            return Ok(EffectDecision {
                nodes: Box::new([Node {
                    variable,
                    low: node.low,
                    high: node.high,
                }]),
                root: self.root,
            });
        }
        self.rebuild_nodes(control, |reference, low, high, builder| {
            let variable = mapping(reference, builder.control)?;
            builder.branch_on_variable(variable, low, high)
        })
    }

    /// Resolve each reachable reference once and compose with the supplied
    /// target decisions. The replacements are already in the target scope.
    pub fn substitute_references<'r, U: Clone + Ord + 'r, C: DecisionControl>(
        &self,
        control: &mut C,
        mut binding: impl FnMut(&V, &mut C) -> Result<EffectDecisionReplacement<'r, U>, C::Error>,
    ) -> Result<EffectDecision<U>, C::Error> {
        control.charge(DecisionWork::Visit)?;
        if let Some(reference) = self.single_reference() {
            return match binding(reference, control)? {
                EffectDecisionReplacement::Constant(value) => Ok(EffectDecision::constant(value)),
                EffectDecisionReplacement::Variable(variable) => {
                    control.charge(DecisionWork::Node)?;
                    Ok(EffectDecision::reference(variable))
                }
                EffectDecisionReplacement::Decision(value) => value.copy_with_control(control),
            };
        }
        let mut replacements = BTreeMap::<&V, Root>::new();
        self.rebuild_nodes(control, |reference, low, high, builder| {
            let condition = if let Some(&root) = replacements.get(&reference) {
                root
            } else {
                let value = binding(reference, builder.control)?;
                let root = builder.operand(value)?;
                builder.control.charge(DecisionWork::Node)?;
                replacements.insert(reference, root);
                root
            };
            builder.conditional(Conditional::new(condition, high, low))
        })
    }

    fn rebuild_nodes<'s, U: Clone + Ord, C: DecisionControl>(
        &'s self,
        control: &mut C,
        mut rebuild: impl FnMut(
            &'s V,
            Root,
            Root,
            &mut DecisionBuilder<'_, U, C>,
        ) -> Result<Root, C::Error>,
    ) -> Result<EffectDecision<U>, C::Error> {
        let mut builder = DecisionBuilder::new(control);
        let mut indices = Vec::new();
        let mut preserves_layout = true;
        for (index, node) in self.nodes.iter().enumerate() {
            builder.control.charge(DecisionWork::Visit)?;
            let low = Self::mapped_root(node.low, &indices);
            let high = Self::mapped_root(node.high, &indices);
            let root = rebuild(&node.variable, low, high, &mut builder)?;
            preserves_layout = preserves_layout
                && root == Root::Branch(index)
                && builder.nodes[index].low == low
                && builder.nodes[index].high == high;
            indices.push(root);
        }
        // Matching every edge and index makes the input's canonical postorder
        // the output's order as well. Move the admitted nodes into the output.
        if preserves_layout && builder.nodes.len() == indices.len() {
            return Ok(EffectDecision {
                nodes: builder.nodes.into_boxed_slice(),
                root: self.root,
            });
        }
        builder.finish(Self::mapped_root(self.root, &indices))
    }

    /// Builds `self ? yes : no` from canonical operands.
    pub fn conditional<C: DecisionControl>(
        &self,
        yes: &Self,
        no: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        // Branch indices are local to each operand. Before import, give
        // each nonterminal root a distinct slot so only reductions justified
        // by the terminal structure can select a complete existing operand.
        let operands = [self, yes, no];
        let [condition_root, yes_root, no_root] = [0, 1, 2].map(|slot| match operands[slot].root {
            Root::Branch(_) => Root::Branch(slot),
            terminal => terminal,
        });
        if let Some(root) = Conditional::new(condition_root, yes_root, no_root).reduced() {
            return match root {
                Root::False | Root::True => {
                    // A purely terminal operation uses no decision nodes.
                    // Retain the existing cost of that case for runtime
                    // admission and charge a visit when an operand is a graph.
                    if operands.iter().any(|operand| !operand.nodes.is_empty()) {
                        control.charge(DecisionWork::Visit)?;
                    }
                    Ok(Self::constant(root == Root::True))
                }
                Root::Branch(slot) => operands[slot].copy_with_control(control),
            };
        }
        if yes_root == Root::False && no_root == Root::True {
            return self.not(control);
        }
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

    /// Conjoin a sequence inside one interner, publishing only the final decision.
    /// Conditions with the same leading reference are reduced together.
    pub fn all<'a, C: DecisionControl>(
        decisions: impl IntoIterator<Item = &'a Self>,
        control: &mut C,
    ) -> Result<Self, C::Error>
    where
        V: 'a,
    {
        let mut decisions = decisions.into_iter();
        let mut nontrivial = || -> Result<Option<&'a Self>, C::Error> {
            for decision in decisions.by_ref() {
                control.charge(DecisionWork::Visit)?;
                if decision.root != Root::True {
                    return Ok(Some(decision));
                }
            }
            Ok(None)
        };
        let Some(first) = nontrivial()? else {
            return Ok(Self::constant(true));
        };
        if first.root == Root::False {
            return Ok(Self::constant(false));
        }
        let Some(second) = nontrivial()? else {
            return first.copy_with_control(control);
        };
        if second.root == Root::False {
            return Ok(Self::constant(false));
        }
        let Some(third) = nontrivial()? else {
            return first.and(second, control);
        };
        let mut conjuncts = Conjuncts::new();
        for decision in [first, second, third] {
            if !conjuncts.add(decision, control)? {
                return Ok(Self::constant(false));
            }
        }
        for decision in decisions {
            control.charge(DecisionWork::Visit)?;
            if !conjuncts.add(decision, control)? {
                return Ok(Self::constant(false));
            }
        }
        let mut builder = DecisionBuilder::new(control);
        let Some(points) = conjuncts.import(&mut builder)? else {
            return Ok(Self::constant(false));
        };
        let root = builder.conjoin(points)?;
        builder.finish(root)
    }

    /// Select an entry in a Boolean truth table. Selector zero supplies the
    /// least significant bit of the entry index. Every operand lives in the
    /// same reference scope. Entries are read in mask order, then selector
    /// descriptors are read once in descending bit order for a nonuniform table.
    /// Returns `None` if the table size is not exactly two to the power of
    /// the selector count.
    pub fn select_table<'a, C: DecisionControl>(
        selectors: usize,
        entries: usize,
        control: &mut C,
        mut selector: impl FnMut(usize) -> EffectDecisionReplacement<'a, V>,
        mut entry: impl FnMut(usize, &mut C) -> Result<EffectDecisionReplacement<'a, V>, C::Error>,
    ) -> Result<Option<Self>, C::Error>
    where
        V: 'a,
    {
        if u32::try_from(selectors)
            .ok()
            .and_then(|bits| 1usize.checked_shl(bits))
            != Some(entries)
        {
            return Ok(None);
        }
        if selectors == 0 {
            return Ok(Some(match entry(0, control)? {
                EffectDecisionReplacement::Constant(value) => Self::constant(value),
                EffectDecisionReplacement::Variable(variable) => Self::variable(variable, control)?,
                EffectDecisionReplacement::Decision(value) => value.copy_with_control(control)?,
            }));
        }

        let mut builder = DecisionBuilder::new(control);
        let mut frontier = Vec::new();
        let mut uniform = true;
        for index in 0..entries {
            builder.control.charge(DecisionWork::Node)?;
            let value = entry(index, builder.control)?;
            let root = builder.operand(value)?;
            uniform &= frontier.first().is_none_or(|first| *first == root);
            frontier.push(root);
        }
        if uniform {
            return builder.finish(frontier[0]).map(Some);
        }

        // Fold constants first, then later references before earlier ones.
        // Table coordinates stay fixed even when arguments permute the formals.
        let mut order = Vec::new();
        for bit in (0..selectors).rev() {
            builder.control.charge(DecisionWork::Node)?;
            order.push((bit, selector(bit)));
        }
        order.sort_unstable_by(|(left_bit, left), (right_bit, right)| {
            let left = left.first_reference();
            let right = right.first_reference();
            left.is_some()
                .cmp(&right.is_some())
                .then_with(|| right.cmp(&left))
                .then_with(|| right_bit.cmp(left_bit))
        });
        let mut remaining = entries - 1;
        for (bit, selector) in order {
            let source_bit = 1usize << bit;
            let position = (remaining & (source_bit - 1)).count_ones();
            let high_bit = 1usize << position;
            let lower_bits = high_bit - 1;
            remaining &= !source_bit;
            let mut condition = None;
            let mut selector = Some(selector);
            let next_count = frontier.len() / 2;
            for index in 0..next_count {
                builder.control.charge(DecisionWork::Visit)?;
                // Insert this axis back into the compacted index. The low
                // index is at least `index`, so writing in place is safe.
                let low_index = (index & lower_bits) | ((index & !lower_bits) << 1);
                let low = frontier[low_index];
                let high = frontier[low_index | high_bit];
                if low == high {
                    frontier[index] = low;
                    continue;
                }
                if let Some((variable, positive)) = selector
                    .as_ref()
                    .and_then(EffectDecisionReplacement::literal)
                    && builder.children_follow(variable, low, high)
                {
                    // The pair visit admitted this ordered branch; intern
                    // its result directly, including a literal result.
                    let (low, high) = if positive { (low, high) } else { (high, low) };
                    frontier[index] = builder.intern_branch(Node {
                        variable: variable.clone(),
                        low,
                        high,
                    })?;
                    continue;
                }
                let test = match condition {
                    Some(test) => test,
                    None => {
                        let test =
                            builder.operand(selector.take().expect("selector is lowered once"))?;
                        condition = Some(test);
                        test
                    }
                };
                frontier[index] = builder.conditional(Conditional::new(test, high, low))?;
            }
            frontier.truncate(next_count);
        }
        builder.finish(frontier[0]).map(Some)
    }

    /// Disjoins two functions using the shared controlled decision builder.
    pub fn or<C: DecisionControl>(&self, other: &Self, control: &mut C) -> Result<Self, C::Error> {
        self.conditional(&Self::constant(true), other, control)
    }

    /// The valuations under which both functions return the same value.
    pub fn equivalent<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        if self.is_constant(true) {
            return other.copy_with_control(control);
        }
        if other.is_constant(true) {
            return self.copy_with_control(control);
        }
        if self.is_constant(false) {
            return other.not(control);
        }
        if other.is_constant(false) {
            return self.not(control);
        }
        let mut builder = DecisionBuilder::new(control);
        let left = builder.import(self)?;
        let right = builder.import(other)?;
        let same = builder.equivalent_roots(left, right)?;
        builder.finish(same)
    }

    /// The valuations that satisfy every pair of equivalent operands. Build
    /// their conjunction in one arena so shared coefficients are interned once.
    pub fn all_equalities<'a, C: DecisionControl>(
        equations: impl IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
        control: &mut C,
    ) -> Result<Self, C::Error>
    where
        V: 'a,
    {
        let mut builder = DecisionBuilder::new(control);
        let root = builder.conjoin_equalities(equations)?;
        builder.finish(root)
    }

    /// Requires all the conclusions when all the hypotheses hold. Both
    /// groups and the implication share one decision arena.
    pub fn all_equalities_under<'a, C: DecisionControl>(
        hypotheses: impl IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
        conclusions: impl IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
        control: &mut C,
    ) -> Result<Self, C::Error>
    where
        V: 'a,
    {
        let mut builder = DecisionBuilder::new(control);
        let root = builder.guarded_equalities(hypotheses, conclusions)?;
        builder.finish(root)
    }

    /// Conjoins existing relations and groups of guarded equalities in one
    /// arena. Each group's conclusions must hold whenever all its hypotheses
    /// agree. Only the joint relation is emitted as a canonical decision.
    pub fn all_with_guarded_equalities<'a, C, H, E>(
        relations: impl IntoIterator<Item = &'a Self>,
        groups: impl IntoIterator<Item = (H, E)>,
        control: &mut C,
    ) -> Result<Self, C::Error>
    where
        C: DecisionControl,
        H: IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
        E: IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
        V: 'a,
    {
        let mut conjuncts = Conjuncts::new();
        for relation in relations {
            control.charge(DecisionWork::Visit)?;
            if !conjuncts.add(relation, control)? {
                return Ok(Self::constant(false));
            }
        }
        let mut builder = DecisionBuilder::new(control);
        let Some(mut points) = conjuncts.import(&mut builder)? else {
            return Ok(Self::constant(false));
        };
        // Every retained base conjunct must hold. Within guarded groups, its
        // literal value can stand in for either polarity of that reference.
        let fixed_operands = |equation: Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>| {
            equation.map(|pair| {
                pair.map(|operand| {
                    if let Some((reference, polarity)) = operand.literal()
                        && let Some(&value) = conjuncts.literals.get(reference)
                    {
                        EffectDecisionReplacement::Constant(value == polarity)
                    } else {
                        operand
                    }
                })
            })
        };
        for (hypotheses, conclusions) in groups {
            builder.control.charge(DecisionWork::Visit)?;
            match builder.guarded_equalities(
                hypotheses.into_iter().map(fixed_operands),
                conclusions.into_iter().map(fixed_operands),
            )? {
                Root::False => return Ok(Self::constant(false)),
                Root::True => {}
                root @ Root::Branch(_) => {
                    builder.control.charge(DecisionWork::Node)?;
                    points.push(root);
                }
            }
        }
        let root = builder.conjoin(points)?;
        builder.finish(root)
    }

    /// The condition under which every assignment in `cases` yields its
    /// expected coefficient. Each case assigns the ordered references up to
    /// and including `through`; later references remain free.
    pub fn matches_prefix_table<'a, C: DecisionControl>(
        &self,
        through: &V,
        cases: impl IntoIterator<Item = usize>,
        control: &mut C,
        mut select: impl FnMut(usize, &V) -> Result<bool, C::Error>,
        mut expected: impl FnMut(usize) -> EffectDecisionReplacement<'a, V>,
    ) -> Result<Self, C::Error>
    where
        V: 'a,
    {
        let mut builder = DecisionBuilder::new(control);
        let mut prefix = DecisionPrefix::new(self);
        let mut equations = Vec::new();
        for case in cases {
            builder.control.charge(DecisionWork::Visit)?;
            let point = prefix.select(through, case, &mut select, &mut builder)?;
            let coefficient = builder.operand(expected(case))?;
            let equation = builder.equivalent_roots(point, coefficient)?;
            match equation {
                Root::False => return Ok(Self::constant(false)),
                Root::True => {}
                Root::Branch(_) => {
                    builder.control.charge(DecisionWork::Node)?;
                    equations.push(equation);
                }
            }
        }
        let result = builder.conjoin_ordered(equations)?;
        builder.finish(result)
    }

    /// Complements a Boolean function without interpreting it as a finite row.
    pub fn not<C: DecisionControl>(&self, control: &mut C) -> Result<Self, C::Error> {
        if self.nodes.is_empty() {
            return Ok(Self::constant(self.root == Root::False));
        }
        self.admit_copy(control)?;
        // Swapping terminals is a bijection. Branch indices, distinct
        // children and the reachable low-before-high order are preserved.
        let complement = |root| match root {
            Root::False => Root::True,
            Root::True => Root::False,
            branch => branch,
        };
        Ok(Self {
            nodes: self
                .nodes
                .iter()
                .map(|node| Node {
                    variable: node.variable.clone(),
                    low: complement(node.low),
                    high: complement(node.high),
                })
                .collect(),
            root: self.root,
        })
    }

    /// Existentially eliminates the supplied caller-owned variables.
    pub fn exists<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.quantify(quantified, Quantifier::Exists, control)
    }

    /// Universally eliminates the supplied caller-owned variables.
    pub fn forall<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        self.quantify(quantified, Quantifier::ForAll, control)
    }

    /// Valuations of the remaining variables for which changing any of the
    /// supplied variables cannot change this decision's result.
    pub fn independence_region<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        if quantified.is_empty() || self.nodes.is_empty() {
            return Ok(Self::constant(true));
        }
        let mut builder = DecisionBuilder::new(control);
        let relation = self.view();
        let possible = builder.project(&relation, quantified, Quantifier::Exists, None)?;
        let necessary = builder.project(&relation, quantified, Quantifier::ForAll, None)?;
        let possible = Self::mapped_root(self.root, &possible);
        let necessary = Self::mapped_root(self.root, &necessary);
        let region = builder.conditional(Conditional::new(possible, necessary, Root::True))?;
        builder.finish(region)
    }

    fn quantify<C: DecisionControl>(
        &self,
        quantified: &BTreeSet<V>,
        quantifier: Quantifier,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        match self.root {
            Root::False => return Ok(Self::constant(false)),
            Root::True => return Ok(Self::constant(true)),
            Root::Branch(_) => {}
        }
        if quantified.is_empty() {
            return self.copy_with_control(control);
        }
        let mut builder = DecisionBuilder::new(control);
        let indices = builder.project(&self.view(), quantified, quantifier, None)?;
        builder.finish(Self::mapped_root(self.root, &indices))
    }

    fn mapped_root(root: Root, indices: &[Root]) -> Root {
        match root {
            Root::Branch(index) => indices[index],
            terminal => terminal,
        }
    }

    /// Simultaneous substitution: references inside a replacement are not
    /// recursively processed by the same substitution.
    /// Only replacements for variables reachable in this decision are imported.
    pub fn substitute<C: DecisionControl>(
        &self,
        replacements: &BTreeMap<V, Self>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        if replacements.is_empty() || self.nodes.is_empty() {
            return self.copy_with_control(control);
        }
        self.substitute_references(control, |variable, _| {
            Ok(match replacements.get(variable) {
                Some(replacement) => EffectDecisionReplacement::Decision(replacement),
                None => EffectDecisionReplacement::Variable(variable.clone()),
            })
        })
    }

    /// Assigns Boolean values simultaneously. Unassigned references remain free.
    pub fn restrict<C: DecisionControl>(
        &self,
        assignment: &BTreeMap<V, bool>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        if assignment.is_empty() {
            return self.copy_with_control(control);
        }
        self.restrict_references(|variable| assignment.get(variable).copied(), control)
    }

    /// Assigns each visited reference the Boolean value returned by the
    /// resolver. It must return the same value for repeated occurrences of a
    /// reference. Returning `None` leaves that reference free.
    pub fn restrict_references<C: DecisionControl>(
        &self,
        assignment: impl Fn(&V) -> Option<bool>,
        control: &mut C,
    ) -> Result<Self, C::Error> {
        if self.nodes.is_empty() {
            return self.copy_with_control(control);
        }

        control.charge(DecisionWork::Visit)?;
        // An assigned prefix only selects branches. Its nodes cannot appear
        // below the first unassigned node in the ordered graph.
        let mut root = self.root;
        let (first_free, node) = loop {
            control.charge(DecisionWork::Visit)?;
            let Root::Branch(index) = root else {
                return Ok(Self::constant(root == Root::True));
            };
            let node = &self.nodes[index];
            root = match assignment(&node.variable) {
                Some(true) => node.high,
                Some(false) => node.low,
                None => break (index, node),
            };
        };

        // Discover the residual graph before rebuilding it in child-first order.
        let mut pending = vec![node.high, node.low];
        let mut selected = BTreeMap::from([(first_free, None)]);
        while let Some(root) = pending.pop() {
            control.charge(DecisionWork::Visit)?;
            let Root::Branch(index) = root else {
                continue;
            };
            if selected.contains_key(&index) {
                continue;
            }
            let node = &self.nodes[index];
            let child =
                assignment(&node.variable).map(|value| if value { node.high } else { node.low });
            selected.insert(index, child);
            if let Some(child) = child {
                pending.push(child);
            } else {
                pending.push(node.high);
                pending.push(node.low);
            }
        }

        let mut builder = DecisionBuilder::new(control);
        let mut indices = BTreeMap::new();
        for (index, child) in selected {
            builder.control.charge(DecisionWork::Visit)?;
            let restricted = if let Some(child) = child {
                DecisionBuilder::<V, C>::remap(child, &indices)
            } else {
                let node = &self.nodes[index];
                let low = DecisionBuilder::<V, C>::remap(node.low, &indices);
                let high = DecisionBuilder::<V, C>::remap(node.high, &indices);
                builder.branch(node.variable.clone(), low, high)?
            };
            indices.insert(index, restricted);
        }
        builder.finish(DecisionBuilder::<V, C>::remap(
            Root::Branch(first_free),
            &indices,
        ))
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
        let CompletionRoots {
            admissibility,
            replacements,
            instantiated,
        } = builder.completion_roots(self, quantified)?;
        let check =
            builder.conditional(Conditional::new(admissibility, instantiated, Root::True))?;
        let least = if check == Root::True {
            let mut least = BTreeMap::new();
            for variable in quantified {
                builder.control.charge(DecisionWork::Visit)?;
                let root = replacements.get(variable).copied().unwrap_or(Root::False);
                least.insert(variable.clone(), builder.finish(root)?);
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
        quantified: impl Into<Arc<BTreeSet<V>>>,
        control: &mut C,
    ) -> Result<ConditionalDecisionCompletion<V>, C::Error> {
        control.charge(DecisionWork::Node)?;
        let quantified = quantified.into();
        let mut builder = DecisionBuilder::new(control);
        let CompletionRoots {
            admissibility,
            replacements,
            instantiated,
        } = builder.completion_roots(self, &quantified)?;
        let principal =
            builder.conditional(Conditional::new(admissibility, instantiated, Root::False))?;
        let mut nonzero = BTreeMap::new();
        let mut decisions = BTreeMap::new();
        for (variable, root) in replacements {
            builder.control.charge(DecisionWork::Node)?;
            let decision = match decisions.entry(root) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let decision = builder.finish(root)?;
                    builder.control.charge(DecisionWork::Node)?;
                    entry.insert(Arc::new(decision))
                }
            };
            nonzero.insert(variable, Arc::clone(decision));
        }
        Ok(ConditionalDecisionCompletion {
            satisfiable: builder.finish(admissibility)?,
            principal: builder.finish(principal)?,
            lower_envelope: DecisionLowerEnvelope {
                domain: quantified,
                nonzero,
                zero: Self::constant(false),
            },
        })
    }
}

impl<'c, V: Clone + Ord, C: DecisionControl> DecisionBuilder<'c, V, C> {
    /// The one lower-envelope calculation shared by both completion policies.
    fn completion_roots(
        &mut self,
        relation: &EffectDecision<V>,
        quantified: &BTreeSet<V>,
    ) -> Result<CompletionRoots<V>, C::Error> {
        if quantified.is_empty() {
            let relation = self.import(relation)?;
            return Ok(CompletionRoots {
                admissibility: relation,
                replacements: BTreeMap::new(),
                instantiated: relation,
            });
        }
        let relation = relation.view();
        let tail = relation.after_forced_prefix(self.control)?;
        let forced = &relation.nodes[tail.nodes.len()..];
        let mut completion = self.complete_tail(&tail, quantified)?;
        if forced.is_empty() || completion.admissibility == Root::False {
            return Ok(completion);
        }

        let mut guard = Root::True;
        for node in forced {
            self.control.charge(DecisionWork::Visit)?;
            if !quantified.contains(&node.variable) {
                let (low, high) = if node.low == Root::False {
                    (Root::False, guard)
                } else {
                    (guard, Root::False)
                };
                guard = self.branch(node.variable.clone(), low, high)?;
            }
        }
        if guard != Root::True {
            completion.admissibility = self.conditional(Conditional::new(
                guard,
                completion.admissibility,
                Root::False,
            ))?;
            for root in completion.replacements.values_mut() {
                self.control.charge(DecisionWork::Visit)?;
                *root = self.conditional(Conditional::new(guard, *root, Root::False))?;
            }
        }
        for node in forced {
            self.control.charge(DecisionWork::Visit)?;
            if node.low == Root::False && quantified.contains(&node.variable) {
                self.control.charge(DecisionWork::Node)?;
                completion
                    .replacements
                    .insert(node.variable.clone(), completion.admissibility);
            }
        }
        // The reconstructed prefix already holds on the admissible domain.
        // The tail's instantiated predicate suffices to check principality there.
        Ok(completion)
    }

    fn complete_tail(
        &mut self,
        relation: &DecisionView<'_, V>,
        quantified: &BTreeSet<V>,
    ) -> Result<CompletionRoots<V>, C::Error> {
        if let Some(closed) = self.complete_bound_relation(relation, quantified)? {
            return Ok(closed);
        }
        let mut seeds = BTreeMap::<&V, Vec<usize>>::new();
        let projected = self.project(relation, quantified, Quantifier::Exists, Some(&mut seeds))?;
        let admissibility = EffectDecision::<V>::mapped_root(relation.root, &projected);
        let mut replacements = BTreeMap::new();
        if !seeds.is_empty() {
            let mut parents = Vec::<Vec<usize>>::new();
            for (index, node) in relation.nodes.iter().enumerate() {
                self.control.charge(DecisionWork::Visit)?;
                for child in [node.low, node.high] {
                    if let Root::Branch(child) = child {
                        self.control.charge(DecisionWork::Node)?;
                        parents[child].push(index);
                    }
                }
                parents.push(Vec::new());
            }
            let mut sweep = CofactorSweep::new(parents, self.control)?;
            for (variable, seeds) in seeds {
                self.control.charge(DecisionWork::Visit)?;
                let possible_without = self.project_with_false(
                    relation, &projected, quantified, variable, &mut sweep, seeds,
                )?;
                let required = self.conditional(Conditional::new(
                    possible_without,
                    Root::False,
                    admissibility,
                ))?;
                if required != Root::False {
                    self.control.charge(DecisionWork::Node)?;
                    replacements.insert(variable.clone(), required);
                }
            }
        }
        let instantiated = self.instantiate(relation, quantified, &replacements)?;
        Ok(CompletionRoots {
            admissibility,
            replacements,
            instantiated,
        })
    }

    /// A closed ordered relation admits a variable set to false if some
    /// accepting path takes its low edge or never tests it. Every source node
    /// is reachable, and every non-false child can reach the true terminal.
    /// Mark skipped intervals over the ordered support without enumerating
    /// accepting paths or projecting the relation once per variable.
    fn complete_bound_relation(
        &mut self,
        relation: &DecisionView<'_, V>,
        quantified: &BTreeSet<V>,
    ) -> Result<Option<CompletionRoots<V>>, C::Error> {
        if relation.nodes.is_empty() {
            return Ok(Some(CompletionRoots {
                admissibility: relation.root,
                replacements: BTreeMap::new(),
                instantiated: relation.root,
            }));
        }
        let mut ranks = BTreeMap::new();
        for node in relation.nodes.iter() {
            self.control.charge(DecisionWork::Visit)?;
            if !quantified.contains(&node.variable) {
                return Ok(None);
            }
            if let std::collections::btree_map::Entry::Vacant(slot) = ranks.entry(&node.variable) {
                self.control.charge(DecisionWork::Node)?;
                slot.insert(0);
            }
        }
        let mut optional_until = Vec::new();
        for (rank, slot) in ranks.values_mut().enumerate() {
            self.control.charge(DecisionWork::Node)?;
            *slot = rank;
            optional_until.push(0);
        }
        for node in relation.nodes.iter() {
            self.control.charge(DecisionWork::Visit)?;
            let rank = ranks[&node.variable];
            if node.low != Root::False {
                optional_until[rank] = optional_until[rank].max(rank + 1);
            }
            for child in [node.low, node.high] {
                let end = match child {
                    Root::False => continue,
                    Root::True => ranks.len(),
                    Root::Branch(index) => ranks[&relation.nodes[index].variable],
                };
                let start = rank + 1;
                if start < end {
                    optional_until[start] = optional_until[start].max(end);
                }
            }
        }
        let mut replacements = BTreeMap::new();
        let mut optional_end = 0;
        for ((variable, rank), end) in ranks.into_iter().zip(optional_until) {
            self.control.charge(DecisionWork::Visit)?;
            optional_end = optional_end.max(end);
            if rank >= optional_end {
                self.control.charge(DecisionWork::Node)?;
                replacements.insert(variable.clone(), Root::True);
            }
        }
        let instantiated = if relation.try_evaluate(
            |variable| Ok(replacements.contains_key(variable)),
            || self.control.charge(DecisionWork::Visit),
        )? {
            Root::True
        } else {
            Root::False
        };
        Ok(Some(CompletionRoots {
            admissibility: Root::True,
            replacements,
            instantiated,
        }))
    }

    /// Existential projection with one quantified variable fixed to false.
    /// Propagate departures from the ordinary projection only to their parents.
    /// The source indices are child-first, so a parent's changed children have
    /// all been visited by the time the parent is popped.
    fn project_with_false(
        &mut self,
        relation: &DecisionView<'_, V>,
        projected: &[Root],
        quantified: &BTreeSet<V>,
        variable: &V,
        sweep: &mut CofactorSweep,
        seeds: Vec<usize>,
    ) -> Result<Root, C::Error> {
        debug_assert!(sweep.pending.is_empty());
        sweep.number += 1;
        for seed in seeds {
            self.control.charge(DecisionWork::Visit)?;
            sweep.schedule(seed);
        }
        while let Some(Reverse(index)) = sweep.pending.pop() {
            self.control.charge(DecisionWork::Visit)?;
            let node = &relation.nodes[index];
            let mapped = if &node.variable == variable {
                EffectDecision::<V>::mapped_root(node.low, projected)
            } else {
                debug_assert!(&node.variable < variable);
                let low = sweep.projection(node.low, projected);
                let high = sweep.projection(node.high, projected);
                self.project_branch(
                    &node.variable,
                    low,
                    high,
                    quantified.contains(&node.variable),
                    Quantifier::Exists,
                )?
            };
            if mapped != projected[index] {
                sweep.changes[index].result = Some(mapped);
                for slot in 0..sweep.parents[index].len() {
                    self.control.charge(DecisionWork::Visit)?;
                    sweep.schedule(sweep.parents[index][slot]);
                }
            }
        }
        Ok(sweep.projection(relation.root, projected))
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
        self.intern_branch(Node {
            variable,
            low,
            high,
        })
    }

    fn children_follow(&self, variable: &V, low: Root, high: Root) -> bool {
        [low, high].into_iter().all(|root| {
            self.node(root)
                .is_none_or(|child| &child.variable > variable)
        })
    }

    fn branch_on_variable(&mut self, variable: V, low: Root, high: Root) -> Result<Root, C::Error> {
        if low == high || self.children_follow(&variable, low, high) {
            return self.branch(variable, low, high);
        }
        let condition = self.branch(variable, Root::False, Root::True)?;
        self.conditional(Conditional::new(condition, high, low))
    }

    fn intern_branch(&mut self, node: Node<V>) -> Result<Root, C::Error> {
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
        self.import_with_values(decision, |_| None)
    }

    fn import_with_values(
        &mut self,
        decision: &EffectDecision<V>,
        mut value: impl FnMut(&V) -> Option<bool>,
    ) -> Result<Root, C::Error> {
        let mut indices = Vec::new();
        for node in decision.nodes.iter() {
            self.control.charge(DecisionWork::Visit)?;
            let low = EffectDecision::<V>::mapped_root(node.low, &indices);
            let high = EffectDecision::<V>::mapped_root(node.high, &indices);
            let root = match value(&node.variable) {
                Some(false) => low,
                Some(true) => high,
                None if low == high => low,
                None => self.intern_branch(Node {
                    variable: node.variable.clone(),
                    low,
                    high,
                })?,
            };
            indices.push(root);
        }
        Ok(EffectDecision::<V>::mapped_root(decision.root, &indices))
    }

    fn operand(&mut self, operand: EffectDecisionReplacement<'_, V>) -> Result<Root, C::Error> {
        match operand {
            EffectDecisionReplacement::Constant(false) => Ok(Root::False),
            EffectDecisionReplacement::Constant(true) => Ok(Root::True),
            EffectDecisionReplacement::Variable(variable) => {
                self.branch(variable, Root::False, Root::True)
            }
            EffectDecisionReplacement::Decision(decision) => self.import(decision),
        }
    }

    fn known(&self, key: Conditional) -> Option<Root> {
        key.reduced()
            .or_else(|| self.conditionals.get(&key).copied())
    }

    fn equivalent_roots(&mut self, left: Root, right: Root) -> Result<Root, C::Error> {
        if left == right {
            return Ok(Root::True);
        }
        if left == Root::True {
            return Ok(right);
        }
        if right == Root::True {
            return Ok(left);
        }
        // Use the earlier literal as the condition so both branches can
        // attach directly to it. Distinct literals of one reference disagree.
        let (left, right) = match (self.node(left), self.node(right)) {
            (Some(first), Some(second))
                if first.literal_polarity().is_some() && second.literal_polarity().is_some() =>
            {
                match first.variable.cmp(&second.variable) {
                    std::cmp::Ordering::Less => (left, right),
                    std::cmp::Ordering::Equal => return Ok(Root::False),
                    std::cmp::Ordering::Greater => (right, left),
                }
            }
            _ => (left, right),
        };
        let negated = self.conditional(Conditional::new(right, Root::False, Root::True))?;
        self.conditional(Conditional::new(left, right, negated))
    }

    fn guarded_equalities<'a>(
        &mut self,
        hypotheses: impl IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
        conclusions: impl IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
    ) -> Result<Root, C::Error>
    where
        V: 'a,
    {
        let hypothesis = self.conjoin_equalities(hypotheses)?;
        if hypothesis == Root::False {
            return Ok(Root::True);
        }
        let conclusion = self.conjoin_equalities(conclusions)?;
        self.conditional(Conditional::new(hypothesis, conclusion, Root::True))
    }

    fn conjoin_equalities<'a>(
        &mut self,
        equations: impl IntoIterator<Item = Result<[EffectDecisionReplacement<'a, V>; 2], C::Error>>,
    ) -> Result<Root, C::Error>
    where
        V: 'a,
    {
        let equations = equations.into_iter();
        let mut deferred = (equations.size_hint().0 >= 3).then(DeferredEqualities::new);
        let mut root = Root::True;
        for equation in equations {
            self.control.charge(DecisionWork::Visit)?;
            let pair = equation?;
            let can_defer = match &mut deferred {
                Some(deferred) => deferred.admit(&pair, self.control)?,
                None => false,
            };
            let [left, right] = pair;
            let left = self.operand(left)?;
            let right = self.operand(right)?;
            let same = self.equivalent_roots(left, right)?;
            match same {
                Root::False => return Ok(Root::False),
                Root::True => continue,
                Root::Branch(_) => {}
            }
            if can_defer && let Some(deferred) = &mut deferred {
                self.control.charge(DecisionWork::Node)?;
                deferred.points.push(same);
                continue;
            }
            // Once a point may constrain an earlier assignment, check it
            // before reading another fallible equation.
            if let Some(deferred) = deferred.take() {
                root = self.conjoin_ordered(deferred.points)?;
            }
            root = self.conditional(Conditional::new(root, same, Root::False))?;
            if root == Root::False {
                return Ok(Root::False);
            }
        }
        match deferred {
            Some(deferred) => self.conjoin_ordered(deferred.points),
            None => Ok(root),
        }
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
        if let Some(condition) = self.node(key.condition)
            && let Some(positive) = condition.literal_polarity()
            && self.children_follow(&condition.variable, key.no, key.yes)
        {
            let (low, high) = if positive {
                (key.no, key.yes)
            } else {
                (key.yes, key.no)
            };
            return self.branch(condition.variable.clone(), low, high);
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

    /// Compile an already child-first decision without interning the source.
    /// Intermediate roots index the builder and the input indices stay local
    /// to the canonical decision.
    fn project<'a>(
        &mut self,
        source: &DecisionView<'a, V>,
        bound: &BTreeSet<V>,
        quantifier: Quantifier,
        mut false_cofactors: Option<&mut BTreeMap<&'a V, Vec<usize>>>,
    ) -> Result<Vec<Root>, C::Error> {
        let mut indices = Vec::new();
        for (index, node) in source.nodes.iter().enumerate() {
            self.control.charge(DecisionWork::Visit)?;
            let low = EffectDecision::<V>::mapped_root(node.low, &indices);
            let high = EffectDecision::<V>::mapped_root(node.high, &indices);
            let is_bound = bound.contains(&node.variable);
            let mapped = self.project_branch(&node.variable, low, high, is_bound, quantifier)?;
            // A false cofactor only has to propagate from a tested variable
            // when it changes that node's ordinary existential projection.
            if is_bound
                && low != mapped
                && let Some(seeds) = &mut false_cofactors
            {
                let entries = match seeds.entry(&node.variable) {
                    Entry::Occupied(slot) => slot.into_mut(),
                    Entry::Vacant(slot) => {
                        self.control.charge(DecisionWork::Node)?;
                        slot.insert(Vec::new())
                    }
                };
                self.control.charge(DecisionWork::Node)?;
                entries.push(index);
            }
            indices.push(mapped);
        }
        Ok(indices)
    }

    fn project_branch(
        &mut self,
        variable: &V,
        low: Root,
        high: Root,
        is_bound: bool,
        quantifier: Quantifier,
    ) -> Result<Root, C::Error> {
        if low == high {
            return Ok(low);
        }
        if !is_bound {
            return self.branch(variable.clone(), low, high);
        }
        let combine = match quantifier {
            Quantifier::Exists => Conditional::new(low, Root::True, high),
            Quantifier::ForAll => Conditional::new(low, high, Root::False),
        };
        self.conditional(combine)
    }

    fn instantiate(
        &mut self,
        source: &DecisionView<'_, V>,
        bound: &BTreeSet<V>,
        replacements: &BTreeMap<V, Root>,
    ) -> Result<Root, C::Error> {
        let mut indices = Vec::new();
        for node in source.nodes.iter() {
            self.control.charge(DecisionWork::Visit)?;
            let low = EffectDecision::<V>::mapped_root(node.low, &indices);
            let high = EffectDecision::<V>::mapped_root(node.high, &indices);
            let condition = if bound.contains(&node.variable) {
                replacements
                    .get(&node.variable)
                    .copied()
                    .unwrap_or(Root::False)
            } else {
                self.branch(node.variable.clone(), Root::False, Root::True)?
            };
            let mapped = self.conditional(Conditional::new(condition, high, low))?;
            indices.push(mapped);
        }
        Ok(EffectDecision::<V>::mapped_root(source.root, &indices))
    }

    fn finish(&mut self, root: Root) -> Result<EffectDecision<V>, C::Error> {
        enum Task {
            Visit(usize),
            Emit(usize),
        }

        let Root::Branch(index) = root else {
            return Ok(EffectDecision::constant(root == Root::True));
        };
        let mut task = Task::Visit(index);
        let mut pending = Vec::new();
        let mut nodes = Vec::new();
        let mut indices = BTreeMap::new();
        loop {
            match task {
                Task::Visit(index) if !indices.contains_key(&index) => {
                    // One visit admits this branch and its two child edges.
                    self.control.charge(DecisionWork::Visit)?;
                    let node = &self.nodes[index];
                    pending.push(Task::Emit(index));
                    for child in [node.high, node.low] {
                        if let Root::Branch(child) = child {
                            pending.push(Task::Visit(child));
                        }
                    }
                }
                Task::Visit(_) => {}
                Task::Emit(index) => {
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
            }
            let Some(next) = pending.pop() else {
                break;
            };
            task = next;
        }
        Ok(EffectDecision {
            nodes: nodes.into_boxed_slice(),
            root: Self::remap(root, &indices),
        })
    }
}
