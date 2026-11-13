//! Exact quantified completion of the retained original relation.
//!
//! Coefficients are not expanded into a guessed finite label basis here. Local
//! functions range over finite-preserving dependency functions supported by the
//! retained labels and the actual exceptions of every Future input. Choices
//! are uniform witnesses outside each lexical finite-row universal scope.

use super::*;

mod view;
pub(crate) use view::*;
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
        graph: &PreparedEffectGraph,
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
    Implies {
        domain: ResidualNodeId,
        consequence: ResidualNodeId,
    },
    ForAllFiniteRows {
        scope: ResidualRowScope,
        body: ResidualNodeId,
    },
    ExistsLocalFunctions {
        body: ResidualNodeId,
    },
    ExistsChoices {
        body: ResidualNodeId,
    },
    SourcePredicate(ResidualPredicateSource),
    BodyEquation {
        scheme: u64,
    },
    BodyAdmitted {
        scheme: u64,
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
pub(in crate::effect_row::prepared) enum ResidualPredicateSource {
    SchemeDomain(u64),
    SchemeObligation { scheme: u64, index: usize },
    ObligationDomain(usize),
    ObligationConsequence(usize),
}

impl ResidualPredicateSource {
    pub(in crate::effect_row::prepared) fn scope(self) -> ResidualRowScope {
        match self {
            Self::SchemeDomain(scheme) | Self::SchemeObligation { scheme, .. } => {
                ResidualRowScope::Scheme(scheme)
            }
            Self::ObligationDomain(id) | Self::ObligationConsequence(id) => {
                ResidualRowScope::Obligation(id)
            }
        }
    }

    pub(in crate::effect_row::prepared) fn value(
        self,
        graph: &PreparedEffectGraph,
    ) -> Result<ResidualSourcePredicate<'_>, PreparedEffectInvariant> {
        Ok(match self {
            Self::SchemeDomain(id) => {
                ResidualSourcePredicate::Canonical(&source_scheme(graph, id)?.header.domain)
            }
            Self::SchemeObligation { scheme, index } => {
                let SchemeState::BodyPrepared { obligations, .. } =
                    &source_scheme(graph, scheme)?.state
                else {
                    return Err(PreparedEffectInvariant::SchemeState);
                };
                ResidualSourcePredicate::Prepared(&obligations[index])
            }
            Self::ObligationDomain(id) => {
                ResidualSourcePredicate::Prepared(&graph.obligations[id].domain)
            }
            Self::ObligationConsequence(id) => {
                ResidualSourcePredicate::Prepared(&graph.obligations[id].predicate)
            }
        })
    }
}

#[derive(Clone, Copy)]
pub(in crate::effect_row::prepared) enum ResidualSourcePredicate<'a> {
    Canonical(&'a EffectPredicate),
    Prepared(&'a PreparedEffectPredicate),
}

pub(in crate::effect_row::prepared) fn source_scheme(
    graph: &PreparedEffectGraph,
    id: u64,
) -> Result<&Scheme, PreparedEffectInvariant> {
    match graph.functions.get(&id).map(Arc::as_ref) {
        Some(Function::Scheme(scheme)) => Ok(scheme),
        _ => Err(PreparedEffectInvariant::SchemeState),
    }
}

pub(in crate::effect_row::prepared) fn source_body(
    graph: &PreparedEffectGraph,
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
        graph: &PreparedEffectGraph,
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
        let truth = program.node(ResidualNode::True, work)?;
        let mut relation = Vec::new();
        for (&id, function) in &graph.functions {
            work.charge(DecisionWork::Visit)?;
            if matches!(function.as_ref(), Function::Imported(_)) {
                continue;
            }
            if let Function::Scheme(scheme) = function.as_ref() {
                let SchemeState::BodyPrepared { obligations, .. } = &scheme.state else {
                    return Err(PreparedEffectInvariant::SchemeState.into());
                };
                let scope = ResidualRowScope::Scheme(id);
                let equation = program.node(ResidualNode::BodyEquation { scheme: id }, work)?;
                let admission = program.node(ResidualNode::BodyAdmitted { scheme: id }, work)?;
                let mut body = vec![equation, admission];
                for index in 0..obligations.len() {
                    body.push(program.node(
                        ResidualNode::SourcePredicate(ResidualPredicateSource::SchemeObligation {
                            scheme: id,
                            index,
                        }),
                        work,
                    )?);
                }
                let body = program.conjunction(body, work)?;
                let domain = program.node(
                    ResidualNode::SourcePredicate(ResidualPredicateSource::SchemeDomain(id)),
                    work,
                )?;
                let guarded = program.node(
                    ResidualNode::Implies {
                        domain,
                        consequence: body,
                    },
                    work,
                )?;
                relation.push(program.node(
                    ResidualNode::ForAllFiniteRows {
                        scope,
                        body: guarded,
                    },
                    work,
                )?);
            }
            relation.push(program.node(ResidualNode::FiniteFunction(id), work)?);
        }
        for id in 0..graph.obligations.len() {
            let scope = ResidualRowScope::Obligation(id);
            let domain = program.node(
                ResidualNode::SourcePredicate(ResidualPredicateSource::ObligationDomain(id)),
                work,
            )?;
            let consequence = program.node(
                ResidualNode::SourcePredicate(ResidualPredicateSource::ObligationConsequence(id)),
                work,
            )?;
            let implication = program.node(
                ResidualNode::Implies {
                    domain,
                    consequence,
                },
                work,
            )?;
            relation.push(program.node(
                ResidualNode::ForAllFiniteRows {
                    scope,
                    body: implication,
                },
                work,
            )?);
        }
        if relation.is_empty() {
            relation.push(truth);
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
            if function.is_future() || matches!(function.as_ref(), Function::Imported(_)) {
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
