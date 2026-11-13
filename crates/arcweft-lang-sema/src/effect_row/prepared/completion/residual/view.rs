//! Borrowed typed cursors expose the quantified program and original operands.

use super::*;

#[derive(Clone, Copy)]
pub(crate) struct EffectResidualPredicate<'a> {
    source: &'a ScopedEffectCompletion,
    node: ResidualNodeId,
}

pub(crate) enum EffectResidualPredicateView<'a> {
    True,
    And(ResidualPredicates<'a>),
    Not(EffectResidualPredicate<'a>),
    Implies {
        domain: EffectResidualPredicate<'a>,
        consequence: EffectResidualPredicate<'a>,
    },
    ForAllFiniteRows {
        scope: ResidualRowScope,
        parameters: &'a [GenericEffectReference],
        body: EffectResidualPredicate<'a>,
    },
    ExistsLocalFunctions {
        functions: ResidualFunctions<'a>,
        support: ResidualSupport<'a>,
        body: EffectResidualPredicate<'a>,
    },
    ExistsUniformChoices {
        choices: ResidualChoices<'a>,
        body: EffectResidualPredicate<'a>,
    },
    SourcePredicate(ResidualPreparedPredicate<'a>),
    BodyEquation {
        function: ResidualFunction<'a>,
        body: ResidualPreparedRow<'a>,
    },
    BodyAdmitted {
        function: ResidualFunction<'a>,
        body: ResidualPreparedRow<'a>,
    },
    FiniteFunction(ResidualFunction<'a>),
    OutputMembership {
        function: ResidualFunction<'a>,
        scope: ResidualRowScope,
        parameters: &'a [GenericEffectReference],
        probe: ResidualLabelProbe<'a>,
    },
    SubstituteLower {
        body: EffectResidualPredicate<'a>,
        bindings: ResidualLowerBindings<'a>,
    },
}

/// The domain's zero-input coefficient is finite; other coefficients may be
/// finite or cofinite. Their exceptions belong to this support expression.
pub(crate) struct ResidualSupport<'a> {
    pub(crate) retained: &'a BTreeSet<EffectId>,
    pub(crate) future_exceptions: ResidualFunctions<'a>,
}

/// A bound comprehension variable, deliberately distinct from EffectId.
pub(crate) struct ResidualLabelProbe<'a> {
    pub(crate) output: ResidualFunction<'a>,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidualFunction<'a> {
    source: &'a ScopedEffectCompletion,
    id: u64,
}

pub(crate) enum ResidualFunctionRole<'a> {
    Scheme {
        domain: &'a EffectPredicate,
        body: ResidualPreparedRow<'a>,
    },
    Bindable,
    Future {
        actual: Option<&'a EffectRow>,
    },
    Imported {
        value: ResidualFunction<'a>,
        principal: EffectResidualPredicate<'a>,
        bindings: ResidualImportBindings<'a>,
    },
}

pub(crate) struct ResidualImportBindings<'a> {
    parent: &'a ScopedEffectCompletion,
    child: &'a ScopedEffectCompletion,
    values: std::collections::btree_map::Iter<'a, u64, PreparedEffectUnknownRef>,
}
impl<'a> Iterator for ResidualImportBindings<'a> {
    type Item = (ResidualFunction<'a>, ResidualFunction<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        let (&future, actual) = self.values.next()?;
        let actual = FunctionRef::Unknown(actual.clone());
        Some((
            ResidualFunction {
                source: self.child,
                id: future,
            },
            ResidualFunction {
                source: self.parent,
                id: actual.reference().ordinal,
            },
        ))
    }
}

impl<'a> ResidualFunction<'a> {
    pub(crate) fn parameters(self) -> &'a [GenericEffectReference] {
        self.source.graph.functions[&self.id].parameters()
    }

    pub(crate) fn role(self) -> Result<ResidualFunctionRole<'a>, PreparedEffectInvariant> {
        Ok(match self.source.graph.functions[&self.id].as_ref() {
            Function::Scheme(scheme) => ResidualFunctionRole::Scheme {
                domain: &scheme.header.domain,
                body: ResidualPreparedRow {
                    source: self.source,
                    scope: ResidualRowScope::Scheme(self.id),
                    value: source_body(&self.source.graph, self.id)?,
                },
            },
            Function::Unknown {
                eligibility: PreparedEffectUnknownEligibility::Bindable,
                ..
            } => ResidualFunctionRole::Bindable,
            Function::Unknown {
                eligibility: PreparedEffectUnknownEligibility::Future,
                ..
            } => ResidualFunctionRole::Future {
                actual: self.source.fixed.get(&self.id),
            },
            Function::Imported(imported) => ResidualFunctionRole::Imported {
                value: ResidualFunction {
                    source: &imported.completion,
                    id: imported.selected.reference().ordinal,
                },
                principal: imported.completion.principal(),
                bindings: ResidualImportBindings {
                    parent: self.source,
                    child: &imported.completion,
                    values: imported.futures.iter(),
                },
            },
        })
    }

    pub(crate) fn is_unknown(
        self,
        reference: &PreparedEffectUnknownRef,
    ) -> Result<bool, PreparedEffectInvariant> {
        let reference = FunctionRef::Unknown(reference.clone());
        self.source.graph.function(&reference)?;
        Ok(self.id == reference.reference().ordinal)
    }
}

pub(crate) struct ResidualPredicates<'a> {
    source: &'a ScopedEffectCompletion,
    nodes: std::slice::Iter<'a, ResidualNodeId>,
}
impl<'a> Iterator for ResidualPredicates<'a> {
    type Item = EffectResidualPredicate<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(EffectResidualPredicate {
            source: self.source,
            node: *self.nodes.next()?,
        })
    }
}

enum FunctionFilter {
    Local,
    Future,
}
pub(crate) struct ResidualFunctions<'a> {
    source: &'a ScopedEffectCompletion,
    functions: std::collections::btree_map::Iter<'a, u64, Arc<Function>>,
    filter: FunctionFilter,
}
impl<'a> Iterator for ResidualFunctions<'a> {
    type Item = ResidualFunction<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        for (&id, function) in self.functions.by_ref() {
            let keep = match self.filter {
                FunctionFilter::Local => {
                    !function.is_future() && !matches!(function.as_ref(), Function::Imported(_))
                }
                FunctionFilter::Future => function.is_future(),
            };
            if keep {
                return Some(ResidualFunction {
                    source: self.source,
                    id,
                });
            }
        }
        None
    }
}

pub(crate) struct ResidualChoices<'a> {
    source: &'a ScopedEffectCompletion,
    choices: std::collections::btree_set::Iter<'a, u64>,
}
impl Iterator for ResidualChoices<'_> {
    type Item = PreparedEffectChoice;
    fn next(&mut self) -> Option<Self::Item> {
        Some(PreparedEffectChoice(
            self.source.graph.reference(*self.choices.next()?),
        ))
    }
}

pub(crate) struct ResidualLowerBindings<'a> {
    source: &'a ScopedEffectCompletion,
    values: std::collections::btree_map::Iter<'a, u64, ResidualNodeId>,
}
impl<'a> Iterator for ResidualLowerBindings<'a> {
    type Item = (ResidualFunction<'a>, EffectResidualPredicate<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        let (&id, &node) = self.values.next()?;
        Some((
            ResidualFunction {
                source: self.source,
                id,
            },
            EffectResidualPredicate {
                source: self.source,
                node,
            },
        ))
    }
}

impl<'a> EffectResidualPredicate<'a> {
    fn at(self, node: ResidualNodeId) -> Self {
        Self {
            source: self.source,
            node,
        }
    }
    pub(crate) fn view(self) -> Result<EffectResidualPredicateView<'a>, PreparedEffectInvariant> {
        let function = |id| ResidualFunction {
            source: self.source,
            id,
        };
        Ok(match &self.source.program.nodes[self.node.0] {
            ResidualNode::True => EffectResidualPredicateView::True,
            ResidualNode::And(nodes) => EffectResidualPredicateView::And(ResidualPredicates {
                source: self.source,
                nodes: nodes.iter(),
            }),
            ResidualNode::Not(node) => EffectResidualPredicateView::Not(self.at(*node)),
            ResidualNode::Implies {
                domain,
                consequence,
            } => EffectResidualPredicateView::Implies {
                domain: self.at(*domain),
                consequence: self.at(*consequence),
            },
            ResidualNode::ForAllFiniteRows { scope, body } => {
                EffectResidualPredicateView::ForAllFiniteRows {
                    scope: *scope,
                    parameters: scope.parameters(&self.source.graph),
                    body: self.at(*body),
                }
            }
            ResidualNode::ExistsLocalFunctions { body } => {
                EffectResidualPredicateView::ExistsLocalFunctions {
                    functions: ResidualFunctions {
                        source: self.source,
                        functions: self.source.graph.functions.iter(),
                        filter: FunctionFilter::Local,
                    },
                    support: ResidualSupport {
                        retained: &self.source.support,
                        future_exceptions: ResidualFunctions {
                            source: self.source,
                            functions: self.source.graph.functions.iter(),
                            filter: FunctionFilter::Future,
                        },
                    },
                    body: self.at(*body),
                }
            }
            ResidualNode::ExistsChoices { body } => {
                EffectResidualPredicateView::ExistsUniformChoices {
                    choices: ResidualChoices {
                        source: self.source,
                        choices: self.source.graph.choices.iter(),
                    },
                    body: self.at(*body),
                }
            }
            ResidualNode::SourcePredicate(source) => {
                EffectResidualPredicateView::SourcePredicate(ResidualPreparedPredicate {
                    source: self.source,
                    scope: source.scope(),
                    value: source.value(&self.source.graph)?,
                })
            }
            ResidualNode::BodyEquation { scheme } => EffectResidualPredicateView::BodyEquation {
                function: function(*scheme),
                body: ResidualPreparedRow {
                    source: self.source,
                    scope: ResidualRowScope::Scheme(*scheme),
                    value: source_body(&self.source.graph, *scheme)?,
                },
            },
            ResidualNode::BodyAdmitted { scheme } => EffectResidualPredicateView::BodyAdmitted {
                function: function(*scheme),
                body: ResidualPreparedRow {
                    source: self.source,
                    scope: ResidualRowScope::Scheme(*scheme),
                    value: source_body(&self.source.graph, *scheme)?,
                },
            },
            ResidualNode::FiniteFunction(id) => {
                EffectResidualPredicateView::FiniteFunction(function(*id))
            }
            ResidualNode::OutputMembership { function: id } => {
                EffectResidualPredicateView::OutputMembership {
                    function: function(*id),
                    scope: ResidualRowScope::Output(*id),
                    parameters: function(*id).parameters(),
                    probe: ResidualLabelProbe {
                        output: function(*id),
                    },
                }
            }
            ResidualNode::SubstituteLower { body } => {
                EffectResidualPredicateView::SubstituteLower {
                    body: self.at(*body),
                    bindings: ResidualLowerBindings {
                        source: self.source,
                        values: self.source.program.lower.iter(),
                    },
                }
            }
        })
    }
}

impl ScopedEffectCompletion {
    pub(crate) fn satisfiable(&self) -> EffectResidualPredicate<'_> {
        EffectResidualPredicate {
            source: self,
            node: self.program.satisfiable,
        }
    }
    pub(crate) fn principal(&self) -> EffectResidualPredicate<'_> {
        EffectResidualPredicate {
            source: self,
            node: self.program.principal,
        }
    }
    pub(crate) fn witnesses(&self) -> EffectResidualPredicate<'_> {
        EffectResidualPredicate {
            source: self,
            node: self.program.witnesses,
        }
    }
}

pub(crate) struct ResidualFunctionBody<'a> {
    pub(crate) function: ResidualFunction<'a>,
    pub(crate) principal: EffectResidualPredicate<'a>,
    pub(crate) lower: Option<EffectResidualPredicate<'a>>,
}
impl GuardedEffectFunction {
    pub(crate) fn residual(&self) -> ResidualFunctionBody<'_> {
        let id = self.selected.reference().ordinal;
        ResidualFunctionBody {
            function: ResidualFunction {
                source: &self.completion,
                id,
            },
            principal: self.completion.principal(),
            lower: self
                .completion
                .program
                .lower
                .get(&id)
                .map(|&node| EffectResidualPredicate {
                    source: &self.completion,
                    node,
                }),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ResidualPreparedRow<'a> {
    source: &'a ScopedEffectCompletion,
    scope: ResidualRowScope,
    value: &'a PreparedEffectRow,
}
pub(crate) enum ResidualPreparedRowView<'a> {
    Canonical(&'a EffectRow),
    Application {
        function: ResidualFunction<'a>,
        arguments: ResidualRows<'a>,
    },
    Formula {
        formula: &'a EffectFormula<GenericEffectReference>,
        arguments: ResidualRowBindings<'a>,
    },
    Union(ResidualRows<'a>),
    Select {
        choice: &'a PreparedEffectChoice,
        yes: ResidualPreparedRow<'a>,
        no: ResidualPreparedRow<'a>,
    },
}

pub(crate) struct ResidualRows<'a> {
    source: &'a ScopedEffectCompletion,
    scope: ResidualRowScope,
    rows: std::slice::Iter<'a, PreparedEffectRow>,
}
impl<'a> Iterator for ResidualRows<'a> {
    type Item = ResidualPreparedRow<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(ResidualPreparedRow {
            source: self.source,
            scope: self.scope,
            value: self.rows.next()?,
        })
    }
}
pub(crate) struct ResidualRowBindings<'a> {
    source: &'a ScopedEffectCompletion,
    scope: ResidualRowScope,
    rows: std::collections::btree_map::Iter<'a, GenericEffectReference, PreparedEffectRow>,
}
impl<'a> Iterator for ResidualRowBindings<'a> {
    type Item = (&'a GenericEffectReference, ResidualPreparedRow<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        let (key, value) = self.rows.next()?;
        Some((
            key,
            ResidualPreparedRow {
                source: self.source,
                scope: self.scope,
                value,
            },
        ))
    }
}

impl<'a> ResidualPreparedRow<'a> {
    pub(crate) fn parameters(self) -> &'a [GenericEffectReference] {
        self.scope.parameters(&self.source.graph)
    }
    pub(crate) fn view(self) -> Result<ResidualPreparedRowView<'a>, PreparedEffectInvariant> {
        let rows = |values: &'a [PreparedEffectRow]| ResidualRows {
            source: self.source,
            scope: self.scope,
            rows: values.iter(),
        };
        let row = |value| Self { value, ..self };
        Ok(match self.value {
            PreparedEffectRow::Canonical(value) => ResidualPreparedRowView::Canonical(value),
            PreparedEffectRow::Term(reference) => match self.source.graph.row(reference)? {
                RowTerm::Application {
                    function,
                    arguments,
                } => ResidualPreparedRowView::Application {
                    function: ResidualFunction {
                        source: self.source,
                        id: function.reference().ordinal,
                    },
                    arguments: rows(arguments),
                },
                RowTerm::Substitution { formula, arguments } => ResidualPreparedRowView::Formula {
                    formula,
                    arguments: ResidualRowBindings {
                        source: self.source,
                        scope: self.scope,
                        rows: arguments.iter(),
                    },
                },
                RowTerm::Union(values) => ResidualPreparedRowView::Union(rows(values)),
                RowTerm::Select { choice, yes, no } => ResidualPreparedRowView::Select {
                    choice,
                    yes: row(yes),
                    no: row(no),
                },
            },
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ResidualPreparedPredicate<'a> {
    source: &'a ScopedEffectCompletion,
    scope: ResidualRowScope,
    value: ResidualSourcePredicate<'a>,
}
pub(crate) enum ResidualPreparedPredicateView<'a> {
    Canonical(&'a EffectPredicate),
    Formula {
        predicate: &'a EffectPredicate,
        arguments: ResidualRowBindings<'a>,
    },
    Relation {
        relation: PreparedEffectRelation,
        actual: ResidualPreparedRow<'a>,
        permitted: ResidualPreparedRow<'a>,
    },
    And(ResidualPreparedPredicates<'a>),
    Or(ResidualPreparedPredicates<'a>),
    Not(ResidualPreparedPredicate<'a>),
    Select {
        choice: &'a PreparedEffectChoice,
        yes: ResidualPreparedPredicate<'a>,
        no: ResidualPreparedPredicate<'a>,
    },
}
pub(crate) struct ResidualPreparedPredicates<'a> {
    source: &'a ScopedEffectCompletion,
    scope: ResidualRowScope,
    values: std::slice::Iter<'a, PreparedEffectPredicate>,
}
impl<'a> Iterator for ResidualPreparedPredicates<'a> {
    type Item = ResidualPreparedPredicate<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(ResidualPreparedPredicate {
            source: self.source,
            scope: self.scope,
            value: ResidualSourcePredicate::Prepared(self.values.next()?),
        })
    }
}
impl<'a> ResidualPreparedPredicate<'a> {
    pub(crate) fn parameters(self) -> &'a [GenericEffectReference] {
        self.scope.parameters(&self.source.graph)
    }
    pub(crate) fn view(self) -> Result<ResidualPreparedPredicateView<'a>, PreparedEffectInvariant> {
        let value = match self.value {
            ResidualSourcePredicate::Canonical(value) => {
                return Ok(ResidualPreparedPredicateView::Canonical(value));
            }
            ResidualSourcePredicate::Prepared(value) => value,
        };
        let predicate = |value| Self {
            value: ResidualSourcePredicate::Prepared(value),
            ..self
        };
        let row = |value| ResidualPreparedRow {
            source: self.source,
            scope: self.scope,
            value,
        };
        let children = |values: &'a [PreparedEffectPredicate]| ResidualPreparedPredicates {
            source: self.source,
            scope: self.scope,
            values: values.iter(),
        };
        Ok(match value {
            PreparedEffectPredicate::Canonical(value) => {
                ResidualPreparedPredicateView::Canonical(value)
            }
            PreparedEffectPredicate::Term(reference) => match self
                .source
                .graph
                .predicate(reference)?
            {
                PredicateTerm::Substitution {
                    predicate,
                    arguments,
                } => ResidualPreparedPredicateView::Formula {
                    predicate,
                    arguments: ResidualRowBindings {
                        source: self.source,
                        scope: self.scope,
                        rows: arguments.iter(),
                    },
                },
                PredicateTerm::Relation {
                    relation,
                    actual,
                    permitted,
                } => ResidualPreparedPredicateView::Relation {
                    relation: *relation,
                    actual: row(actual),
                    permitted: row(permitted),
                },
                PredicateTerm::And(values) => ResidualPreparedPredicateView::And(children(values)),
                PredicateTerm::Or(values) => ResidualPreparedPredicateView::Or(children(values)),
                PredicateTerm::Not(value) => ResidualPreparedPredicateView::Not(predicate(value)),
                PredicateTerm::Select { choice, yes, no } => {
                    ResidualPreparedPredicateView::Select {
                        choice,
                        yes: predicate(yes),
                        no: predicate(no),
                    }
                }
            },
        })
    }
}
