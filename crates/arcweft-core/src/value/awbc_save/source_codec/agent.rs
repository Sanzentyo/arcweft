//! Closed static Agent data and its recursive predicate operands.

use super::super::{
    AwbcRuntimeAgentPredicateSnapshot as Predicate, AwbcRuntimeAgentSnapshot as Agent,
};
use super::{Reader, RuntimeLiteralSourceError as Error, Writer};
use crate::value::{
    AgentPredicateOperands, RuntimeAgentAction, RuntimeAgentActionDispatch,
    RuntimeAgentActionTarget, RuntimeAgentCaptureTarget, RuntimeAgentCompareOp, RuntimeAgentKind,
    RuntimeAgentPath, RuntimeAgentProbe, RuntimeAgentView, RuntimePredicateKind,
    RuntimePredicateView,
};

impl Writer<'_> {
    fn agent_path(&mut self, path: &RuntimeAgentPath) -> Result<(), Error> {
        RuntimeAgentPath::validate_source(path.as_str()).map_err(|source| Error::Agent {
            offset: self.length,
            source,
        })?;
        self.string(path.as_str())
    }

    fn probe(&mut self, probe: &RuntimeAgentProbe) -> Result<(), Error> {
        match probe {
            RuntimeAgentProbe::Signal { target } => {
                self.tag(0)?;
                self.target(target)
            }
            RuntimeAgentProbe::Metric { target } => {
                self.tag(1)?;
                self.target(target)
            }
            RuntimeAgentProbe::StatePath { path } => {
                self.tag(2)?;
                self.agent_path(path)
            }
            RuntimeAgentProbe::ObservationField { path } => {
                self.tag(3)?;
                self.agent_path(path)
            }
        }
    }

    pub(super) fn agent(&mut self, agent: RuntimeAgentView<'_>, depth: usize) -> Result<(), Error> {
        match agent.kind() {
            RuntimeAgentKind::ActionTarget(target) => {
                self.tag(0)?;
                self.target(target.id())?;
                self.target(target.target())?;
                self.tag(match target.action() {
                    RuntimeAgentAction::AdvanceText => 0,
                    RuntimeAgentAction::SelectChoice => 1,
                    RuntimeAgentAction::Invoke => 2,
                    RuntimeAgentAction::Scroll => 3,
                    RuntimeAgentAction::PointerClick => 4,
                })?;
                self.tag(match target.dispatch() {
                    RuntimeAgentActionDispatch::Semantic => 0,
                    RuntimeAgentActionDispatch::Physical => 1,
                })?;
                self.boolean(target.enabled())
            }
            RuntimeAgentKind::CaptureTarget(target) => {
                self.tag(1)?;
                match target {
                    RuntimeAgentCaptureTarget::Viewport => self.tag(0),
                    RuntimeAgentCaptureTarget::Layer { target } => {
                        self.tag(1)?;
                        self.target(target)
                    }
                    RuntimeAgentCaptureTarget::Object { target } => {
                        self.tag(2)?;
                        self.target(target)
                    }
                }
            }
            RuntimeAgentKind::DebugStatePath(path) => {
                self.tag(2)?;
                self.agent_path(path)
            }
            RuntimeAgentKind::ObservationFieldPath(path) => {
                self.tag(3)?;
                self.agent_path(path)
            }
            RuntimeAgentKind::Probe(probe) => {
                self.tag(4)?;
                self.probe(probe)
            }
            RuntimeAgentKind::Diagnostics => self.tag(5),
            RuntimeAgentKind::Predicate(predicate) => {
                self.tag(6)?;
                self.predicate(predicate, depth + 1)
            }
            RuntimeAgentKind::ViewportPoint { x, y } => {
                self.tag(7)?;
                self.u32(x)?;
                self.u32(y)
            }
            RuntimeAgentKind::BinaryData(text) => {
                self.tag(8)?;
                self.string(text)
            }
            RuntimeAgentKind::DataShape(_) => Err(Error::RuntimeCarrier {
                kind: "program-bound DataShape",
                offset: self.length,
            }),
        }
    }

    fn predicate(
        &mut self,
        predicate: RuntimePredicateView<'_>,
        depth: usize,
    ) -> Result<(), Error> {
        self.node(depth)?;
        match predicate.kind() {
            RuntimePredicateKind::Compare { probe, op, value } => {
                self.tag(0)?;
                self.probe(probe)?;
                self.tag(match op {
                    RuntimeAgentCompareOp::Eq => 0,
                    RuntimeAgentCompareOp::NotEq => 1,
                    RuntimeAgentCompareOp::Greater => 2,
                    RuntimeAgentCompareOp::GreaterOrEqual => 3,
                    RuntimeAgentCompareOp::Less => 4,
                    RuntimeAgentCompareOp::LessOrEqual => 5,
                })?;
                self.counts
                    .array::<super::super::AwbcRuntimeValueSnapshot>(1)?;
                self.value(value.view(), depth + 1)
            }
            RuntimePredicateKind::Exists { probe } => {
                self.tag(1)?;
                self.probe(probe)
            }
            RuntimePredicateKind::ActionEnabled { target } => {
                self.tag(2)?;
                self.target(target)
            }
            RuntimePredicateKind::DiagnosticsHasError => self.tag(3),
            RuntimePredicateKind::All(predicates) | RuntimePredicateKind::Any(predicates) => {
                self.tag(
                    if matches!(predicate.kind(), RuntimePredicateKind::All(_)) {
                        4
                    } else {
                        5
                    },
                )?;
                self.counts.array::<Predicate>(predicates.len())?;
                self.count(predicates.len())?;
                for index in 0..predicates.len() {
                    self.predicate(
                        predicates
                            .get(index)
                            .expect("predicate view owns each operand"),
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            RuntimePredicateKind::Not(predicate) => {
                self.tag(6)?;
                self.counts.array::<Predicate>(1)?;
                self.predicate(predicate, depth + 1)
            }
        }
    }
}

impl Reader<'_, '_> {
    fn agent_path(&mut self, materialize: bool) -> Result<Option<RuntimeAgentPath>, Error> {
        let offset = self.offset;
        let text = self.string()?;
        RuntimeAgentPath::validate_source(text)
            .map_err(|source| Error::Agent { offset, source })?;
        if materialize {
            RuntimeAgentPath::try_new(text)
                .map(Some)
                .map_err(|source| Error::Agent { offset, source })
        } else {
            Ok(None)
        }
    }

    fn probe(&mut self, materialize: bool) -> Result<Option<RuntimeAgentProbe>, Error> {
        let offset = self.offset;
        match self.tag()? {
            0 => Ok(self
                .target(materialize)?
                .map(|target| RuntimeAgentProbe::Signal { target })),
            1 => Ok(self
                .target(materialize)?
                .map(|target| RuntimeAgentProbe::Metric { target })),
            2 => Ok(self
                .agent_path(materialize)?
                .map(|path| RuntimeAgentProbe::StatePath { path })),
            3 => Ok(self
                .agent_path(materialize)?
                .map(|path| RuntimeAgentProbe::ObservationField { path })),
            tag => Err(Error::Tag {
                kind: "Agent probe",
                tag,
                offset,
            }),
        }
    }

    pub(super) fn agent(
        &mut self,
        materialize: bool,
        depth: usize,
    ) -> Result<Option<Agent>, Error> {
        let offset = self.offset;
        match self.tag()? {
            0 => {
                let id = self.target(materialize)?;
                let target = self.target(materialize)?;
                let offset = self.offset;
                let action = match self.tag()? {
                    0 => RuntimeAgentAction::AdvanceText,
                    1 => RuntimeAgentAction::SelectChoice,
                    2 => RuntimeAgentAction::Invoke,
                    3 => RuntimeAgentAction::Scroll,
                    4 => RuntimeAgentAction::PointerClick,
                    tag => {
                        return Err(Error::Tag {
                            kind: "Agent action",
                            tag,
                            offset,
                        });
                    }
                };
                let offset = self.offset;
                let dispatch = match self.tag()? {
                    0 => RuntimeAgentActionDispatch::Semantic,
                    1 => RuntimeAgentActionDispatch::Physical,
                    tag => {
                        return Err(Error::Tag {
                            kind: "Agent dispatch",
                            tag,
                            offset,
                        });
                    }
                };
                let enabled = self.boolean()?;
                Ok(materialize.then(|| {
                    Agent::ActionTarget(RuntimeAgentActionTarget::new(
                        id.expect("retained action ID"),
                        target.expect("retained action target"),
                        action,
                        dispatch,
                        enabled,
                    ))
                }))
            }
            1 => {
                let offset = self.offset;
                let target = match self.tag()? {
                    0 => materialize.then_some(RuntimeAgentCaptureTarget::Viewport),
                    1 => self
                        .target(materialize)?
                        .map(|target| RuntimeAgentCaptureTarget::Layer { target }),
                    2 => self
                        .target(materialize)?
                        .map(|target| RuntimeAgentCaptureTarget::Object { target }),
                    tag => {
                        return Err(Error::Tag {
                            kind: "Agent capture target",
                            tag,
                            offset,
                        });
                    }
                };
                Ok(target.map(Agent::CaptureTarget))
            }
            2 => Ok(self.agent_path(materialize)?.map(Agent::DebugStatePath)),
            3 => Ok(self
                .agent_path(materialize)?
                .map(Agent::ObservationFieldPath)),
            4 => Ok(self.probe(materialize)?.map(Agent::Probe)),
            5 => Ok(materialize.then_some(Agent::Diagnostics)),
            6 => Ok(self
                .predicate(materialize, depth + 1)?
                .map(Agent::Predicate)),
            7 => {
                let x = self.u32()?;
                let y = self.u32()?;
                Ok(materialize.then_some(Agent::ViewportPoint { x, y }))
            }
            8 => {
                let text = self.string()?;
                Ok(materialize.then(|| Agent::BinaryData(text.to_owned())))
            }
            9 => Err(Error::RuntimeCarrier {
                kind: "program-bound DataShape",
                offset,
            }),
            tag => Err(Error::Tag {
                kind: "Agent value",
                tag,
                offset,
            }),
        }
    }

    fn predicate(&mut self, materialize: bool, depth: usize) -> Result<Option<Predicate>, Error> {
        self.node(depth)?;
        let offset = self.offset;
        match self.tag()? {
            0 => {
                let probe = self.probe(materialize)?;
                let offset = self.offset;
                let op = match self.tag()? {
                    0 => RuntimeAgentCompareOp::Eq,
                    1 => RuntimeAgentCompareOp::NotEq,
                    2 => RuntimeAgentCompareOp::Greater,
                    3 => RuntimeAgentCompareOp::GreaterOrEqual,
                    4 => RuntimeAgentCompareOp::Less,
                    5 => RuntimeAgentCompareOp::LessOrEqual,
                    tag => {
                        return Err(Error::Tag {
                            kind: "Agent comparison",
                            tag,
                            offset,
                        });
                    }
                };
                self.counts
                    .array::<super::super::AwbcRuntimeValueSnapshot>(1)?;
                let value = self.value(materialize, depth + 1)?;
                Ok(materialize.then(|| Predicate::Compare {
                    probe: probe.expect("retained predicate probe"),
                    op,
                    value: Box::new(value.expect("retained comparison value")),
                }))
            }
            1 => Ok(self
                .probe(materialize)?
                .map(|probe| Predicate::Exists { probe })),
            2 => Ok(self
                .target(materialize)?
                .map(|target| Predicate::ActionEnabled { target })),
            3 => Ok(materialize.then_some(Predicate::DiagnosticsHasError)),
            tag @ (4 | 5) => {
                let count = self.count()?;
                AgentPredicateOperands::<Predicate>::validate_source_count(count)
                    .map_err(|source| Error::PredicateOperands { offset, source })?;
                self.counts.array::<Predicate>(count)?;
                let mut predicates = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let predicate = self.predicate(materialize, depth + 1)?;
                    if let Some(predicates) = &mut predicates {
                        predicates.push(predicate.expect("retained predicate operand"));
                    }
                }
                if let Some(predicates) = predicates {
                    let predicates = AgentPredicateOperands::try_from(predicates)
                        .map_err(|source| Error::PredicateOperands { offset, source })?;
                    Ok(Some(if tag == 4 {
                        Predicate::All { predicates }
                    } else {
                        Predicate::Any { predicates }
                    }))
                } else {
                    Ok(None)
                }
            }
            6 => {
                self.counts.array::<Predicate>(1)?;
                Ok(self
                    .predicate(materialize, depth + 1)?
                    .map(|predicate| Predicate::Not {
                        predicate: Box::new(predicate),
                    }))
            }
            tag => Err(Error::Tag {
                kind: "Agent predicate",
                tag,
                offset,
            }),
        }
    }
}
