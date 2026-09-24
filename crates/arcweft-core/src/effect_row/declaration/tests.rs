use super::*;
use crate::effect_row::{DecisionControl, DecisionWork, EffectSet};

struct Meter;

impl DecisionControl for Meter {
    type Error = std::convert::Infallible;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn variable(slot: u8) -> EffectFormula<u8> {
    EffectFormula::variable(slot, &mut Meter).unwrap()
}

fn constant(value: bool) -> EffectDecisionDeclaration<u8> {
    EffectDecisionDeclaration {
        nodes: Box::new([]),
        root: if value {
            EffectDecisionTarget::True
        } else {
            EffectDecisionTarget::False
        },
    }
}

fn node(
    variable: u8,
    low: EffectDecisionTarget,
    high: EffectDecisionTarget,
) -> EffectDecisionNodeDeclaration<u8> {
    EffectDecisionNodeDeclaration {
        variable,
        low,
        high,
    }
}

fn graph(
    nodes: Vec<EffectDecisionNodeDeclaration<u8>>,
    root: EffectDecisionTarget,
) -> EffectDecisionDeclaration<u8> {
    EffectDecisionDeclaration {
        nodes: nodes.into_boxed_slice(),
        root,
    }
}

#[test]
fn formula_and_predicate_declarations_round_trip_the_shared_algebra() {
    let p = variable(0);
    let q = variable(1);
    let read = EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None);
    let row = p
        .difference(&q, &mut Meter)
        .unwrap()
        .union(&read, &mut Meter)
        .unwrap();
    let declaration = EffectMembershipDeclaration::try_from(&row).unwrap();
    assert_eq!(EffectFormula::try_from(declaration.clone()).unwrap(), row);
    let encoded = serde_json::to_vec(&row).unwrap();
    assert_eq!(
        serde_json::from_slice::<EffectFormula<u8>>(&encoded).unwrap(),
        row
    );
    assert_eq!(
        serde_json::to_vec(&EffectFormula::try_from(declaration).unwrap()).unwrap(),
        encoded
    );

    let predicate = p
        .subset(&q.union(&read, &mut Meter).unwrap(), &mut Meter)
        .unwrap();
    let encoded = serde_json::to_vec(&predicate).unwrap();
    assert_eq!(
        serde_json::from_slice::<EffectPredicate<u8>>(&encoded).unwrap(),
        predicate
    );
    let reconstructed =
        EffectPredicate::try_from(EffectMembershipDeclaration::try_from(&predicate).unwrap())
            .unwrap();
    let rows = BTreeMap::from([(0, read.clone()), (1, EffectFormula::empty())]);
    assert!(
        reconstructed
            .substitute(&rows, &mut Meter)
            .unwrap()
            .is_unconstrained()
    );
}

#[test]
fn decision_admission_rejects_invalid_edges_order_reduction_and_reachability() {
    use EffectDecisionTarget::{False, Node, True};
    let cases = [
        (
            graph(vec![node(0, False, Node(0))], Node(0)),
            EffectDeclarationError::InvalidEdge,
        ),
        (
            graph(vec![node(0, False, True)], Node(1)),
            EffectDeclarationError::InvalidEdge,
        ),
        (
            graph(vec![node(0, True, True)], Node(0)),
            EffectDeclarationError::UnreducedNode,
        ),
        (
            graph(vec![node(0, False, True), node(1, False, Node(0))], Node(1)),
            EffectDeclarationError::UnorderedVariable,
        ),
        (
            graph(vec![node(1, False, True), node(1, False, True)], Node(1)),
            EffectDeclarationError::DuplicateNode,
        ),
        (
            graph(vec![node(1, False, True)], False),
            EffectDeclarationError::NonCanonicalPostorder,
        ),
        (
            graph(
                vec![
                    node(2, False, True),
                    node(1, False, True),
                    node(0, Node(1), Node(0)),
                ],
                Node(2),
            ),
            EffectDeclarationError::NonCanonicalPostorder,
        ),
    ];
    for (declaration, expected) in cases {
        assert_eq!(
            EffectDecision::<u8>::try_from(declaration).unwrap_err(),
            expected
        );
    }
}

#[test]
fn membership_admission_preserves_finite_rows_and_canonical_predicates() {
    let infinite = EffectMembershipDeclaration {
        default: constant(true),
        overrides: Box::new([]),
    };
    assert_eq!(
        EffectFormula::try_from(infinite).unwrap_err(),
        EffectDeclarationError::InfiniteRow
    );
    let false_at_empty = EffectMembershipDeclaration {
        default: graph(
            vec![node(
                0,
                EffectDecisionTarget::False,
                EffectDecisionTarget::True,
            )],
            EffectDecisionTarget::Node(0),
        ),
        overrides: Box::new([]),
    };
    assert_eq!(
        EffectPredicate::try_from(false_at_empty).unwrap_err(),
        EffectDeclarationError::NonCanonicalPredicate
    );

    let impossible = EffectMembershipDeclaration {
        default: constant(false),
        overrides: Box::new([]),
    };
    assert!(
        EffectPredicate::try_from(impossible)
            .unwrap()
            .is_impossible()
    );
    let rejected_label = EffectId::parse("fs.read").unwrap();
    let labelled = EffectMembershipDeclaration {
        default: constant(true),
        overrides: Box::new([(rejected_label.clone(), constant(false))]),
    };
    let predicate = EffectPredicate::try_from(labelled).unwrap();
    assert!(predicate.is_impossible());
    assert_eq!(
        predicate
            .rejected_labels(&mut Meter)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        [rejected_label]
    );
}

#[test]
fn serde_cannot_bypass_canonical_graph_and_override_admission() {
    let label = EffectId::parse("fs.read").unwrap();
    let repeated = EffectMembershipDeclaration {
        default: constant(false),
        overrides: Box::new([
            (label.clone(), constant(true)),
            (label.clone(), constant(true)),
        ]),
    };
    assert!(
        serde_json::from_value::<EffectFormula<u8>>(serde_json::to_value(repeated).unwrap())
            .is_err()
    );
    let redundant = EffectMembershipDeclaration {
        default: constant(false),
        overrides: Box::new([(label, constant(false))]),
    };
    assert_eq!(
        EffectFormula::try_from(redundant).unwrap_err(),
        EffectDeclarationError::RedundantOverride
    );
    let cyclic = EffectMembershipDeclaration {
        default: graph(
            vec![node(
                0,
                EffectDecisionTarget::False,
                EffectDecisionTarget::Node(0),
            )],
            EffectDecisionTarget::Node(0),
        ),
        overrides: Box::new([]),
    };
    assert!(
        serde_json::from_value::<EffectPredicate<u8>>(serde_json::to_value(cyclic).unwrap())
            .is_err()
    );
}
