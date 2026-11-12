use super::*;
use crate::{
    callable::PRODUCTION_CALLABLE_LIMITS,
    effect_row::{DecisionControl, DecisionWork, EffectPredicate},
    types::{ArrayLength, GenericBinder, GenericScope},
};

#[derive(Debug, thiserror::Error)]
#[error("projection stopped")]
struct Stopped;

struct Control {
    remaining: usize,
    visits: usize,
    max_depth: u64,
    cancelled: bool,
}

impl Control {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            visits: 0,
            max_depth: 0,
            cancelled: false,
        }
    }
}

impl TypeProjectionControl for Control {
    type Error = Stopped;
    fn check(&mut self) -> Result<(), Stopped> {
        if self.cancelled { Err(Stopped) } else { Ok(()) }
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, depth: u64) -> Result<(), Stopped> {
        self.check()?;
        self.remaining = self.remaining.checked_sub(1).ok_or(Stopped)?;
        self.visits += 1;
        self.max_depth = self.max_depth.max(depth);
        Ok(())
    }
    fn visit_binding(&mut self) -> Result<(), Stopped> {
        self.visit_node(TypeProjectionNodeKind::Type, 1)
    }
}

impl DecisionControl for Control {
    type Error = Stopped;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Stopped> {
        self.visit_node(TypeProjectionNodeKind::Effect, 1)
    }
}

fn row(labels: &[&str]) -> EffectRow {
    EffectRow::closed(EffectSet::from_labels(labels.iter().copied()).unwrap())
}

fn schema(
    groups: Vec<Vec<TypeKind>>,
    result: TypeKind,
    issuer: CallableGenericParameterIssuer,
) -> CallableSignatureSchema {
    let groups = groups
        .into_iter()
        .enumerate()
        .map(|(group, types)| {
            let parameters = types
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    CallableParameter::try_new(
                        CallableParameterIndex::try_from_usize(index).unwrap(),
                        None,
                        CallableParameterAdmission::checked(ty),
                        CallableParameterPassing::PositionalOnly,
                        CallableParameterPresence::Required,
                        None,
                        None,
                    )
                    .unwrap()
                })
                .collect();
            CallableParameterGroup::try_new(
                CallableGroupIndex::try_from_usize(group).unwrap(),
                if group == 0 {
                    CallableGroupKind::Initial
                } else {
                    CallableGroupKind::Curried
                },
                parameters,
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .unwrap()
        })
        .collect();
    CallableSignatureSchema::try_new(
        groups,
        result,
        CallableEffectSchema::fixed(row(&[])),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Ordinary,
        issuer,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap()
}

fn staged() -> CallableSignatureSchema {
    schema(
        vec![
            vec![TypeKind::I32, TypeKind::Bool],
            vec![TypeKind::function_with_effects(
                [TypeKind::String],
                TypeKind::I64,
                row(&["fs.read"]),
            )],
            vec![TypeKind::I64],
        ],
        TypeKind::String,
        CallableGenericParameterIssuer::empty(),
    )
}

fn position(group: usize, parameter: usize) -> CallableParameterCoordinate {
    CallableParameterCoordinate::new(
        CallableGroupIndex::try_from_usize(group).unwrap(),
        CallableParameterIndex::try_from_usize(parameter).unwrap(),
    )
}

#[test]
fn a_declared_scheme_builds_the_original_group_chain_before_its_body_is_prepared() {
    let signature = staged();
    let mut graph = PreparedEffectGraph::default();
    let mut control = Control::new(100_000);
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let terminal = graph.application(&scheme, vec![], &mut control).unwrap();
    let before = graph.retained_terms();
    let arrow = signature
        .prepare_function_type_from_position(position(0, 1), &terminal, &graph, &mut control)
        .unwrap();
    assert_eq!(graph.retained_terms(), before);
    let TypeKind::Function {
        params,
        effects,
        return_type,
        ..
    } = &arrow
    else {
        panic!("first group")
    };
    assert_eq!(params, &[TypeKind::Bool]);
    assert_eq!(effects, &PreparedEffectRow::Canonical(row(&[])));
    let TypeKind::Function {
        params,
        effects,
        return_type,
        ..
    } = return_type.as_ref()
    else {
        panic!("second group")
    };
    let TypeKind::Function {
        effects: callback, ..
    } = &params[0]
    else {
        panic!("callback")
    };
    assert_eq!(callback, &PreparedEffectRow::Canonical(row(&["fs.read"])));
    assert_eq!(effects, &PreparedEffectRow::Canonical(row(&[])));
    let TypeKind::Function {
        params,
        effects,
        return_type,
        ..
    } = return_type.as_ref()
    else {
        panic!("terminal group")
    };
    assert_eq!(params, &[TypeKind::I64]);
    assert_eq!(return_type.as_ref(), &TypeKind::String);
    assert_eq!(effects, &terminal);
    let body = graph
        .union(
            vec![
                terminal.clone(),
                PreparedEffectRow::Canonical(row(&["fs.write"])),
            ],
            &mut control,
        )
        .unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.resolve_row(effects, &mut control).unwrap(),
        row(&["fs.write"])
    );
}

#[test]
fn canonical_and_prepared_projection_share_prefix_and_terminal_contracts() {
    let signature = staged();
    let graph = PreparedEffectGraph::default();
    let terminal = row(&["fs.write"]);
    for position in [
        position(0, 0),
        position(0, 1),
        position(1, 0),
        position(2, 0),
    ] {
        let canonical = signature
            .declared_function_type_from_position(position, &terminal)
            .unwrap();
        let expected = canonical
            .prepare_effects(
                signature.generic_inventory().template_scope(),
                &mut Control::new(100_000),
            )
            .unwrap();
        let actual = signature
            .prepare_function_type_from_position(
                position,
                &PreparedEffectRow::Canonical(terminal.clone()),
                &graph,
                &mut Control::new(100_000),
            )
            .unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn original_generic_inventory_predicate_and_omitted_result_rows_remain_prepared() {
    let binder = GenericBinder::new(1, 1, 1);
    let scope = GenericScope::default().with_binder(binder);
    let ty = TypeKind::GenericParam(scope.bound_type(0, 0).unwrap());
    let length = ArrayLength::Generic(scope.bound_const(0, 0).unwrap());
    let effect = scope.bound_effect(0, 0).unwrap();
    let required = row(&["fs.read"]);
    let permitted = EffectRow::open(EffectSet::new(), effect.clone());
    let predicate = required
        .formula()
        .unwrap()
        .subset(permitted.formula().unwrap(), &mut Control::new(100_000))
        .unwrap();
    let input = TypeKind::function_with_effects(
        [ty.clone()],
        ty.clone(),
        EffectRow::open(EffectSet::new(), effect.clone()),
    );
    let result = TypeKind::Array {
        item: Box::new(TypeKind::function([ty.clone()], ty)),
        len: length.clone(),
    };
    let signature = schema(
        vec![vec![input]],
        result,
        CallableGenericParameterIssuer::function_scheme(binder, predicate.clone()),
    );
    let inventory = signature.generic_inventory().clone();
    let mut graph = PreparedEffectGraph::default();
    let mut control = Control::new(100_000);
    let scheme = graph
        .declare(vec![effect.clone()], predicate, &mut control)
        .unwrap();
    let terminal = graph
        .application(
            &scheme,
            vec![PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::new(),
                effect,
            ))],
            &mut control,
        )
        .unwrap();
    let arrow = signature
        .prepare_function_type_from_position(position(0, 0), &terminal, &graph, &mut control)
        .unwrap();
    let TypeKind::Function {
        binder: actual_binder,
        params,
        return_type,
        predicate,
        effects,
    } = arrow
    else {
        panic!("prepared arrow")
    };
    assert_eq!(actual_binder, GenericBinder::EMPTY);
    assert_eq!(
        predicate,
        PreparedEffectPredicate::Canonical(signature.effect_predicate().clone())
    );
    assert_eq!(effects, terminal);
    let TypeKind::Function {
        params: callback, ..
    } = &params[0]
    else {
        panic!("generic callback")
    };
    assert_eq!(
        callback,
        &[TypeKind::GenericParam(scope.bound_type(0, 0).unwrap())]
    );
    let TypeKind::Array {
        item: element,
        len: actual_length,
    } = return_type.as_ref()
    else {
        panic!("generic result")
    };
    assert_eq!(actual_length, &length);
    let TypeKind::Function { effects, .. } = element.as_ref() else {
        panic!("omitted result annotation")
    };
    assert_eq!(effects, &PreparedEffectRow::Canonical(EffectRow::unknown()));
    assert_eq!(signature.generic_inventory(), &inventory);
}

#[test]
fn foreign_stale_and_unknown_terminal_rows_cannot_issue_a_prepared_arrow() {
    let signature = staged();
    let mut foreign = PreparedEffectGraph::default();
    let mut graph = PreparedEffectGraph::default();
    let mut control = Control::new(100_000);
    let scheme = foreign
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let row = foreign.application(&scheme, vec![], &mut control).unwrap();
    assert!(matches!(
        signature.prepare_function_type_from_position(position(0, 0), &row, &graph, &mut control),
        Err(CallableFunctionTypeProjectionError::Projection(
            PreparedCallableFunctionTypeFailure::Effect(PreparedEffectInvariant::ForeignIssuer)
        ))
    ));
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let stale = graph.application(&scheme, vec![], &mut control).unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    assert!(matches!(
        signature.prepare_function_type_from_position(position(0, 0), &stale, &graph, &mut control),
        Err(CallableFunctionTypeProjectionError::Projection(
            PreparedCallableFunctionTypeFailure::Effect(PreparedEffectInvariant::StaleReference)
        ))
    ));
    assert!(matches!(
        signature.prepare_function_type_from_position(
            position(0, 0),
            &PreparedEffectRow::Canonical(EffectRow::unknown()),
            &graph,
            &mut control
        ),
        Err(CallableFunctionTypeProjectionError::Projection(
            PreparedCallableFunctionTypeFailure::UnknownTerminalRow
        ))
    ));
    assert_eq!(graph.retained_terms(), (0, 0));
}

#[test]
fn complete_projection_has_one_inclusive_budget_and_preserves_abort_and_depth() {
    let signature = staged();
    let graph = PreparedEffectGraph::default();
    let terminal = PreparedEffectRow::Canonical(row(&["fs.write"]));
    let mut measured = Control::new(100_000);
    signature
        .prepare_function_type_from_position(position(0, 0), &terminal, &graph, &mut measured)
        .unwrap();
    assert_eq!(measured.max_depth, 4);
    for available in 0..measured.visits {
        assert!(matches!(
            signature.prepare_function_type_from_position(
                position(0, 0),
                &terminal,
                &graph,
                &mut Control::new(available)
            ),
            Err(CallableFunctionTypeProjectionError::Projection(
                PreparedCallableFunctionTypeFailure::Projection(TypeProjectionError::Control(
                    Stopped
                ))
            ))
        ));
        assert_eq!(graph.retained_terms(), (0, 0));
    }
    signature
        .prepare_function_type_from_position(
            position(0, 0),
            &terminal,
            &graph,
            &mut Control::new(measured.visits),
        )
        .unwrap();
    let mut cancelled = Control::new(100_000);
    cancelled.cancelled = true;
    assert!(matches!(
        signature.prepare_function_type_from_position(
            position(0, 0),
            &terminal,
            &graph,
            &mut cancelled
        ),
        Err(CallableFunctionTypeProjectionError::Projection(
            PreparedCallableFunctionTypeFailure::Projection(TypeProjectionError::Control(Stopped))
        ))
    ));
    assert_eq!(cancelled.visits, 0);
    let mut control = Control::new(100_000);
    assert!(matches!(
        ProjectionDepth {
            control: &mut control,
            offset: u64::MAX
        }
        .visit_node(TypeProjectionNodeKind::Type, 1),
        Err(TypeProjectionError::Instantiation(
            crate::types::TypeInstantiationError::DepthOverflow
        ))
    ));
    assert_eq!(control.visits, 0);
}

#[test]
fn invalid_prefix_positions_fail_before_result_projection_and_empty_groups_remain_valid() {
    let signature = staged();
    let called = std::cell::Cell::new(false);
    let terminal = row(&[]);
    assert!(matches!(
        signature.project_function_type_from_position(
            position(0, 2),
            &terminal,
            || {
                called.set(true);
                Ok::<_, std::convert::Infallible>(TypeKind::Unit)
            },
            |_, _| unreachable!()
        ),
        Err(CallableFunctionTypeProjectionError::MissingParameter { .. })
    ));
    assert!(!called.get());
    let graph = PreparedEffectGraph::default();
    let mut control = Control::new(100_000);
    assert!(matches!(
        signature.prepare_function_type_from_position(
            position(3, 0),
            &PreparedEffectRow::Canonical(terminal.clone()),
            &graph,
            &mut control
        ),
        Err(CallableFunctionTypeProjectionError::MissingGroup { .. })
    ));
    assert_eq!(control.visits, 0);
    let empty = schema(
        vec![vec![]],
        TypeKind::Unit,
        CallableGenericParameterIssuer::empty(),
    );
    let arrow = empty
        .prepare_function_type_from_position(
            position(0, 0),
            &PreparedEffectRow::Canonical(terminal),
            &graph,
            &mut control,
        )
        .unwrap();
    assert!(matches!(arrow, TypeKind::Function { params, .. } if params.is_empty()));
}
