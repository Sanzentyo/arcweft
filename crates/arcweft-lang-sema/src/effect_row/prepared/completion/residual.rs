//! Exact quantified completion of the retained original relation.
//!
//! Coefficients are not expanded into a guessed finite label basis here. Local
//! functions range over finite-preserving dependency functions supported by the
//! retained labels and the actual exceptions of every Future input. Choices
//! are uniform witnesses outside each lexical finite-row universal scope.

use super::*;

mod view;
pub(crate) use view::*;
mod reduction;
pub(crate) use reduction::{
    ReducedEffectCompletion, ReducedEffectFunction, ReducedEffectPredicate,
    ReducedEffectPredicateView, ResidualFunctionVariable, ResidualRowExpression,
};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ResidualRowScope {
    Scheme(u64),
    Obligation(usize),
    Output(u64),
}

impl ResidualRowScope {
    pub(in crate::effect_row::prepared) fn parameters(
        self,
        graph: &PreparedEffectDefinitions,
    ) -> &[GenericEffectReference] {
        match self {
            Self::Scheme(id) | Self::Output(id) => graph.functions[&id].parameters(),
            Self::Obligation(id) => &graph.obligations[id].parameters,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::effect_row::prepared) struct ResidualNodeId(usize);
impl ResidualNodeId {
    pub(in crate::effect_row::prepared) fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub(in crate::effect_row::prepared) enum ResidualNode {
    True,
    And(Box<[ResidualNodeId]>),
    Not(ResidualNodeId),
    /// One source clause, universally quantified over its ordered finite rows.
    Guarded(ResidualClause),
    ExistsLocalFunctions {
        body: ResidualNodeId,
    },
    ExistsChoices {
        body: ResidualNodeId,
    },
    FiniteFunction(u64),
    /// Membership at the output lambda's own formal inputs and a bound label
    /// probe. The probe is a logical variable, never a synthetic EffectId.
    OutputMembership {
        function: u64,
    },
    /// Simultaneous substitution of every local function with its own lower
    /// comprehension. Future inputs and uniform choices remain untouched.
    SubstituteLower {
        body: ResidualNodeId,
    },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::effect_row::prepared) enum ResidualClause {
    Scheme(u64),
    Obligation(usize),
}

impl ResidualClause {
    pub(in crate::effect_row::prepared) fn scope(self) -> ResidualRowScope {
        match self {
            Self::Scheme(id) => ResidualRowScope::Scheme(id),
            Self::Obligation(id) => ResidualRowScope::Obligation(id),
        }
    }

    pub(in crate::effect_row::prepared) fn source(
        self,
        graph: &PreparedEffectDefinitions,
    ) -> Result<ResidualClauseSource<'_>, PreparedEffectInvariant> {
        Ok(match self {
            Self::Scheme(id) => {
                let scheme = source_scheme(graph, id)?;
                let SchemeState::BodyPrepared { body, obligations } = &scheme.state else {
                    return Err(PreparedEffectInvariant::SchemeState);
                };
                ResidualClauseSource::Scheme {
                    id,
                    domain: &scheme.header.domain,
                    body,
                    obligations,
                }
            }
            Self::Obligation(id) => ResidualClauseSource::Obligation(
                graph
                    .obligations
                    .get(id)
                    .ok_or(PreparedEffectInvariant::UnclosedCell)?,
            ),
        })
    }
}

pub(in crate::effect_row::prepared) enum ResidualClauseSource<'a> {
    Scheme {
        id: u64,
        domain: &'a EffectPredicate,
        body: &'a PreparedEffectRow,
        obligations: &'a [PreparedEffectPredicate],
    },
    Obligation(&'a ScopedObligation),
}

#[derive(Clone, Copy)]
pub(in crate::effect_row::prepared) enum ResidualSourcePredicate<'a> {
    Canonical(&'a EffectPredicate),
    Prepared(&'a PreparedEffectPredicate),
}

pub(in crate::effect_row::prepared) fn source_scheme(
    graph: &PreparedEffectDefinitions,
    id: u64,
) -> Result<&Scheme, PreparedEffectInvariant> {
    match graph.functions.get(&id).map(Arc::as_ref) {
        Some(Function::Scheme(scheme)) => Ok(scheme),
        _ => Err(PreparedEffectInvariant::SchemeState),
    }
}

pub(in crate::effect_row::prepared) fn source_body(
    graph: &PreparedEffectDefinitions,
    id: u64,
) -> Result<&PreparedEffectRow, PreparedEffectInvariant> {
    match &source_scheme(graph, id)?.state {
        SchemeState::BodyPrepared { body, .. } => Ok(body),
        SchemeState::Declared => Err(PreparedEffectInvariant::SchemeState),
    }
}

#[derive(Debug)]
pub(in crate::effect_row::prepared) struct EffectResidualProgram {
    nodes: Vec<ResidualNode>,
    pub(in crate::effect_row::prepared) relation: ResidualNodeId,
    pub(super) satisfiable: ResidualNodeId,
    pub(super) principal: ResidualNodeId,
    pub(super) witnesses: ResidualNodeId,
    pub(super) lower: BTreeMap<u64, ResidualNodeId>,
}

impl EffectResidualProgram {
    fn node<C: DecisionControl>(
        &mut self,
        node: ResidualNode,
        work: &mut Work<'_, C>,
    ) -> Result<ResidualNodeId, PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Node)?;
        let id = ResidualNodeId(self.nodes.len());
        self.nodes.push(node);
        Ok(id)
    }

    fn conjunction<C: DecisionControl>(
        &mut self,
        values: Vec<ResidualNodeId>,
        work: &mut Work<'_, C>,
    ) -> Result<ResidualNodeId, PreparedEffectFailure<C::Error>> {
        self.node(ResidualNode::And(values.into_boxed_slice()), work)
    }

    pub(in crate::effect_row::prepared) fn new<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        Self::with_inventory(graph, closure::FunctionInventory::All, work)
    }

    pub(in crate::effect_row::prepared) fn with_inventory<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        inventory: closure::FunctionInventory<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut program = Self {
            nodes: Vec::new(),
            relation: ResidualNodeId(0),
            satisfiable: ResidualNodeId(0),
            principal: ResidualNodeId(0),
            witnesses: ResidualNodeId(0),
            lower: BTreeMap::new(),
        };
        let mut relation = Vec::new();
        for (&id, function) in &graph.functions {
            if !inventory.contains(id) {
                continue;
            }
            work.charge(DecisionWork::Visit)?;
            if matches!(function.as_ref(), Function::Imported(_)) {
                continue;
            }
            if let Function::Scheme(scheme) = function.as_ref() {
                let SchemeState::BodyPrepared { .. } = &scheme.state else {
                    return Err(PreparedEffectInvariant::SchemeState.into());
                };
                relation
                    .push(program.node(ResidualNode::Guarded(ResidualClause::Scheme(id)), work)?);
            }
            relation.push(program.node(ResidualNode::FiniteFunction(id), work)?);
        }
        for id in 0..graph.obligations.len() {
            relation
                .push(program.node(ResidualNode::Guarded(ResidualClause::Obligation(id)), work)?);
        }
        if relation.is_empty() {
            relation.push(program.node(ResidualNode::True, work)?);
        }
        program.relation = program.conjunction(relation, work)?;
        let witnesses = program.node(
            ResidualNode::ExistsChoices {
                body: program.relation,
            },
            work,
        )?;
        program.satisfiable =
            program.node(ResidualNode::ExistsLocalFunctions { body: witnesses }, work)?;
        for (&id, function) in &graph.functions {
            if !inventory.contains(id) {
                continue;
            }
            if function.is_input() || matches!(function.as_ref(), Function::Imported(_)) {
                continue;
            }
            let member = program.node(ResidualNode::OutputMembership { function: id }, work)?;
            let missing = program.node(ResidualNode::Not(member), work)?;
            let counterexample = program.conjunction(vec![program.relation, missing], work)?;
            let choices = program.node(
                ResidualNode::ExistsChoices {
                    body: counterexample,
                },
                work,
            )?;
            let exists =
                program.node(ResidualNode::ExistsLocalFunctions { body: choices }, work)?;
            let necessary = program.node(ResidualNode::Not(exists), work)?;
            let lower = program.conjunction(vec![program.satisfiable, necessary], work)?;
            work.charge(DecisionWork::Node)?;
            program.lower.insert(id, lower);
        }
        program.witnesses = program.node(
            ResidualNode::SubstituteLower {
                body: program.relation,
            },
            work,
        )?;
        let choices = program.node(
            ResidualNode::ExistsChoices {
                body: program.witnesses,
            },
            work,
        )?;
        program.principal = program.conjunction(vec![program.satisfiable, choices], work)?;
        Ok(program)
    }

    pub(in crate::effect_row::prepared) fn relation_nodes(&self) -> &[ResidualNode] {
        &self.nodes[..=self.relation.0]
    }
}
