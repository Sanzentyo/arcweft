use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatementCorpusDisposition {
    Accepted,
    Pending,
    RejectOnly,
    #[allow(
        dead_code,
        reason = "no live statement family has been proven unreachable"
    )]
    ProvenUnreachable,
}

// Each inventory row also generates an exhaustive match against its live
// HIR or checked owner enum. New variants must receive a disposition.
macro_rules! statement_family_inventory {
    ($family:ident for $owner:ty, { $($pattern:pat => $variant:ident => $disposition:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        enum $family {
            $($variant),+
        }

        impl $family {
            const INVENTORY: &'static [(Self, StatementCorpusDisposition)] = &[
                $((Self::$variant, StatementCorpusDisposition::$disposition)),+
            ];

            fn of(owner: &$owner) -> Self {
                match owner {
                    $($pattern => Self::$variant),+
                }
            }
        }
    };
}

statement_family_inventory!(StatementShapeFamily for HirStmtKind, {
    HirStmtKind::Assertion { .. } => Assertion => Accepted,
    HirStmtKind::Let { .. } => Let => Accepted,
    HirStmtKind::Assign { .. } => Assign => Accepted,
    HirStmtKind::LetElse { .. } => LetElse => Pending,
    HirStmtKind::Return { .. } => Return => Pending,
    HirStmtKind::Out { .. } => Out => Pending,
    HirStmtKind::Goto { .. } => Goto => Pending,
    HirStmtKind::Defer { .. } => Defer => Accepted,
    HirStmtKind::Yield { .. } => Yield => Pending,
    HirStmtKind::Signal { .. } => Signal => Pending,
    HirStmtKind::LifetimeSet { .. } => LifetimeSet => Pending,
    HirStmtKind::Wait { .. } => Wait => Pending,
    HirStmtKind::On { .. } => On => Pending,
    HirStmtKind::CancelRule { .. } => CancelRule => Pending,
    HirStmtKind::UnsafeLifetime { .. } => UnsafeLifetime => Pending,
    HirStmtKind::Choice { .. } => Choice => Pending,
    HirStmtKind::If(_) => If => Accepted,
    HirStmtKind::IfLet(_) => IfLet => Pending,
    HirStmtKind::Match(_) => Match => Pending,
    HirStmtKind::While(_) => While => Pending,
    HirStmtKind::WhileLet(_) => WhileLet => Pending,
    HirStmtKind::For(_) => For => Accepted,
    HirStmtKind::Close { .. } => Close => Pending,
    HirStmtKind::Select(_) => Select => Pending,
    HirStmtKind::SourceLocale(_) => SourceLocale => Pending,
    HirStmtKind::Scope(_) => Scope => Pending,
    HirStmtKind::Include(_) => Include => Pending,
    HirStmtKind::Break { .. } => Break => Pending,
    HirStmtKind::Continue { .. } => Continue => Pending,
    HirStmtKind::Expression { .. } => Expression => Pending,
    HirStmtKind::ProofCall { .. } => ProofCall => Pending,
    HirStmtKind::Error => Error => RejectOnly,
});

statement_family_inventory!(StatementPayloadFamily for CheckedStatementPayload, {
    CheckedStatementPayload::Structural => Structural => Accepted,
    CheckedStatementPayload::Assignment(_) => Assignment => Accepted,
    CheckedStatementPayload::Assertion(_) => Assertion => Accepted,
    CheckedStatementPayload::Defer(_) => Defer => Accepted,
    CheckedStatementPayload::EvaluatedEffect(_) => EvaluatedEffect => Pending,
    CheckedStatementPayload::Iteration(_) => Iteration => Accepted,
    CheckedStatementPayload::ControlTransfer(_) => ControlTransfer => Pending,
    CheckedStatementPayload::Trigger(_) => Trigger => Pending,
    CheckedStatementPayload::UnsafeAudit(_) => UnsafeAudit => Pending,
    CheckedStatementPayload::Select(_) => Select => Pending,
    CheckedStatementPayload::SourceLocale(_) => SourceLocale => Pending,
    CheckedStatementPayload::Scope(_) => Scope => Pending,
    CheckedStatementPayload::Include(_) => Include => Pending,
    CheckedStatementPayload::Suspension(_) => Suspension => Pending,
    CheckedStatementPayload::Yield => Yield => Pending,
});

struct MatchStatementCorpusObservation {
    shapes: BTreeSet<StatementShapeFamily>,
    payloads: BTreeSet<StatementPayloadFamily>,
    assignments: Vec<CheckedAssignmentCorpusFact>,
    assertion_dispositions: Vec<CheckedAssertionDisposition>,
    defers: Vec<CheckedDeferCorpusFact>,
    iterations: Vec<CheckedIteration>,
    semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedAssignmentCorpusFact {
    field_identity: [u8; 32],
    field_ordinal: u32,
    field_type: crate::types::TypeKind,
    value_type: crate::types::TypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedDeferCorpusFact {
    outcome: arcweft_lang_syntax::ast::line_plan::DeferOutcome,
    captures: Vec<(Vec<u8>, crate::types::TypeKind)>,
}

/// Count only checked statements owned below this accepted Match expression.
fn accepted_match_statement_corpus_observation(source: &str) -> MatchStatementCorpusObservation {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("statement corpus source should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owners = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    let [match_owner] = match_owners.as_slice() else {
        panic!("each statement corpus row has exactly one Match expression");
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, *match_owner);
    assert!(
        !product.arms().is_empty(),
        "accepted Match has checked arms"
    );

    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let match_path = coordinates
        .expression(*match_owner)
        .expect("accepted Match root path");
    let mut shapes = BTreeSet::new();
    let mut payloads = BTreeSet::new();
    let mut assignments = Vec::new();
    let mut assertion_dispositions = Vec::new();
    let mut defers = Vec::new();
    let mut iterations = Vec::new();
    for (owner, hir) in module.statements() {
        let Some(checked) = report.statement(owner) else {
            continue;
        };
        let coordinate = coordinates
            .statement(owner)
            .expect("checked statement owner path");
        if !coordinate.path().is_at_or_below(&match_path) {
            continue;
        }
        shapes.insert(StatementShapeFamily::of(hir.kind()));
        payloads.insert(StatementPayloadFamily::of(checked.payload()));
        match (hir.kind(), checked.payload()) {
            (HirStmtKind::Assign { .. }, CheckedStatementPayload::Assignment(assignment)) => {
                let field_place = assignment
                    .place()
                    .nominal_field()
                    .expect("Match assignment selects its checked nominal field");
                assignments.push(CheckedAssignmentCorpusFact {
                    field_identity: *field_place.field().field().as_bytes(),
                    field_ordinal: field_place.field().declaration_ordinal(),
                    field_type: field_place.field_type().clone(),
                    value_type: assignment.value_type().clone(),
                });
            }
            (HirStmtKind::Assertion { .. }, CheckedStatementPayload::Assertion(disposition)) => {
                assertion_dispositions.push(*disposition)
            }
            (HirStmtKind::Defer { .. }, CheckedStatementPayload::Defer(defer)) => {
                defers.push(CheckedDeferCorpusFact {
                    outcome: defer.outcome(),
                    captures: defer
                        .captures()
                        .iter()
                        .map(|capture| {
                            (
                                capture
                                    .origin()
                                    .canonical_bytes()
                                    .expect("defer capture has a stable checked origin"),
                                capture.ty().clone(),
                            )
                        })
                        .collect(),
                });
            }
            (HirStmtKind::For(_), CheckedStatementPayload::Iteration(iteration)) => {
                iterations.push(iteration.as_ref().clone())
            }
            (HirStmtKind::Assign { .. }, _) => {
                panic!("accepted Assign has its exact checked Assignment payload")
            }
            (HirStmtKind::Assertion { .. }, _) => {
                panic!("accepted Assertion has its exact checked Assertion payload")
            }
            (HirStmtKind::Defer { .. }, _) => {
                panic!("accepted Defer has its exact checked Defer payload")
            }
            (HirStmtKind::For(_), _) => {
                panic!("accepted For has its exact checked Iteration payload")
            }
            _ => {}
        }
    }
    assert!(
        !shapes.is_empty(),
        "Match root has checked statement descendants"
    );
    MatchStatementCorpusObservation {
        shapes,
        payloads,
        assignments,
        assertion_dispositions,
        defers,
        iterations,
        semantic_digest: *product.semantic_digest().as_bytes(),
    }
}

fn match_arm_assignment_source(field: &str) -> String {
    format!(
        "struct Flags {{ left: bool, right: bool }}\n\
         fn root(flags: Flags, flag: bool) -> bool {{\n\
             match flag {{\n\
                 true => {{\n\
                     flags.{field} = flag\n\
                     true\n\
                 }}\n\
                 false => false\n\
             }}\n\
         }}\n"
    )
}

fn match_arm_assertion_source(operand: &str) -> String {
    format!(
        "fn root(flag: bool, other: bool) -> i64 {{\n\
             match flag {{\n\
                 true => {{\n\
                     assert.check({operand})\n\
                     1i64\n\
                 }}\n\
                 false => 0i64\n\
             }}\n\
         }}\n"
    )
}

fn flow_match_statement_source(statement: &str) -> String {
    let statement = statement
        .lines()
        .map(|line| format!("                     {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "flow root(flag: bool, other: bool) {{\n\
             let selected = match flag {{\n\
                 true => {{\n\
{statement}\n\
                     1i64\n\
                 }}\n\
                 false => 0i64\n\
             }}\n\
         }}\n"
    )
}

fn flow_match_defer_source(capture: &str) -> String {
    flow_match_statement_source(&format!("defer {{\n    let captured = {capture};\n}}"))
}

fn assert_statement_inventory<T: Copy + Ord + std::fmt::Debug>(
    axis: &str,
    observed: &BTreeSet<T>,
    inventory: &[(T, StatementCorpusDisposition)],
) {
    for (family, disposition) in inventory {
        assert_eq!(
            observed.contains(family),
            *disposition == StatementCorpusDisposition::Accepted,
            "{axis} {family:?} corpus disposition {disposition:?}",
        );
    }
}

#[test]
fn checked_match_statement_corpus_tracks_accepted_root_families() {
    struct Row {
        name: &'static str,
        source: String,
        shapes: &'static [StatementShapeFamily],
        payloads: &'static [StatementPayloadFamily],
    }
    let rows = [
        Row {
            name: "arm block binding",
            source: match_arm_let_source("1i64"),
            shapes: &[StatementShapeFamily::Let],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "nested If and binding",
            source: nested_if_statement_source("                let first = 1i64"),
            shapes: &[StatementShapeFamily::If, StatementShapeFamily::Let],
            payloads: &[StatementPayloadFamily::Structural],
        },
        Row {
            name: "nominal field assignment in arm block",
            source: match_arm_assignment_source("left"),
            shapes: &[StatementShapeFamily::Assign],
            payloads: &[StatementPayloadFamily::Assignment],
        },
        Row {
            name: "runtime assertion in arm block",
            source: match_arm_assertion_source("flag"),
            shapes: &[StatementShapeFamily::Assertion],
            payloads: &[StatementPayloadFamily::Assertion],
        },
        Row {
            name: "Defer in Flow arm block",
            source: flow_match_defer_source("flag"),
            shapes: &[StatementShapeFamily::Defer, StatementShapeFamily::Let],
            payloads: &[
                StatementPayloadFamily::Defer,
                StatementPayloadFamily::Structural,
            ],
        },
        Row {
            name: "For in Flow arm block",
            source: flow_match_statement_source("for value in [true, false] {}"),
            shapes: &[StatementShapeFamily::For],
            payloads: &[StatementPayloadFamily::Iteration],
        },
    ];
    let mut shapes = BTreeSet::new();
    let mut payloads = BTreeSet::new();
    for row in rows {
        let observation = accepted_match_statement_corpus_observation(&row.source);
        for required in row.shapes {
            assert!(
                observation.shapes.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        for required in row.payloads {
            assert!(
                observation.payloads.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        shapes.extend(observation.shapes);
        payloads.extend(observation.payloads);
    }
    assert_statement_inventory("HIR statement", &shapes, StatementShapeFamily::INVENTORY);
    assert_statement_inventory(
        "checked statement",
        &payloads,
        StatementPayloadFamily::INVENTORY,
    );
}

#[test]
fn checked_match_assignments_retain_same_typed_field_identity_and_digest() {
    let left = accepted_match_statement_corpus_observation(&match_arm_assignment_source("left"));
    let right = accepted_match_statement_corpus_observation(&match_arm_assignment_source("right"));

    assert_eq!(left.shapes, BTreeSet::from([StatementShapeFamily::Assign]));
    assert_eq!(
        left.payloads,
        BTreeSet::from([StatementPayloadFamily::Assignment])
    );
    assert_eq!(left.assignments.len(), 1);
    assert_eq!(right.assignments.len(), 1);
    assert_eq!(left.assignments[0].field_ordinal, 0);
    assert_eq!(right.assignments[0].field_ordinal, 1);
    assert_ne!(
        left.assignments[0].field_identity,
        right.assignments[0].field_identity
    );
    assert_eq!(left.assignments[0].field_type, crate::types::TypeKind::Bool);
    assert_eq!(
        right.assignments[0].field_type,
        crate::types::TypeKind::Bool
    );
    assert_eq!(left.assignments[0].value_type, crate::types::TypeKind::Bool);
    assert_eq!(
        right.assignments[0].value_type,
        crate::types::TypeKind::Bool
    );
    assert_ne!(left.semantic_digest, right.semantic_digest);
}

#[test]
fn checked_match_assertions_retain_disposition_and_condition_meaning() {
    let flag = accepted_match_statement_corpus_observation(&match_arm_assertion_source("flag"));
    let other = accepted_match_statement_corpus_observation(&match_arm_assertion_source("other"));
    let expected =
        CheckedAssertionDisposition::Runtime(crate::assertion::AssertionRuntimePolicy::AlwaysGuard);

    assert_eq!(
        flag.shapes,
        BTreeSet::from([StatementShapeFamily::Assertion])
    );
    assert_eq!(
        flag.payloads,
        BTreeSet::from([StatementPayloadFamily::Assertion])
    );
    assert_eq!(flag.assertion_dispositions, [expected]);
    assert_eq!(other.assertion_dispositions, [expected]);
    assert_ne!(flag.semantic_digest, other.semantic_digest);
}

#[test]
fn checked_match_defer_retains_capture_meaning() {
    let flag = accepted_match_statement_corpus_observation(&flow_match_defer_source("flag"));
    let other = accepted_match_statement_corpus_observation(&flow_match_defer_source("other"));

    assert_eq!(
        flag.shapes,
        BTreeSet::from([StatementShapeFamily::Defer, StatementShapeFamily::Let])
    );
    assert_eq!(
        flag.payloads,
        BTreeSet::from([
            StatementPayloadFamily::Defer,
            StatementPayloadFamily::Structural,
        ])
    );
    assert_eq!(flag.defers.len(), 1);
    assert_eq!(other.defers.len(), 1);
    assert_eq!(
        flag.defers[0].outcome,
        arcweft_lang_syntax::ast::line_plan::DeferOutcome::Always
    );
    assert_eq!(flag.defers[0].captures.len(), 1);
    assert_eq!(other.defers[0].captures.len(), 1);
    assert_eq!(flag.defers[0].captures[0].1, crate::types::TypeKind::Bool);
    assert_eq!(other.defers[0].captures[0].1, crate::types::TypeKind::Bool);
    assert_ne!(flag.defers[0].captures[0].0, other.defers[0].captures[0].0);
    assert_ne!(flag.semantic_digest, other.semantic_digest);
}

#[test]
fn checked_match_for_retains_builtin_iteration_and_iterable_meaning() {
    let ordered = accepted_match_statement_corpus_observation(&flow_match_statement_source(
        "for value in [true, false] {}",
    ));
    let repeated = accepted_match_statement_corpus_observation(&flow_match_statement_source(
        "for value in [true, true] {}",
    ));

    assert_eq!(ordered.shapes, BTreeSet::from([StatementShapeFamily::For]));
    assert_eq!(
        ordered.payloads,
        BTreeSet::from([StatementPayloadFamily::Iteration])
    );
    for observation in [&ordered, &repeated] {
        assert!(
            matches!(
                observation.iterations.as_slice(),
                [CheckedIteration::Builtin {
                    family: CheckedIteratorFamily::Vec,
                    item,
                }] if item == &crate::types::TypeKind::Bool
            ),
            "observed checked iteration: {:?}",
            observation.iterations
        );
    }
    assert_ne!(ordered.semantic_digest, repeated.semantic_digest);
}

#[test]
fn checked_match_statement_corpus_excludes_unrelated_declaration_statements() {
    let source = format!(
        "fn unrelated() -> i64 {{ return 9i64 }}\n{}",
        match_arm_let_source("1i64"),
    );
    let world = super::fixture(&source, None);
    let report = super::analyze(&world).expect("unrelated Return should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    assert!(module.statements().any(|(owner, statement)| {
        matches!(statement.kind(), HirStmtKind::Return { .. }) && report.statement(owner).is_some()
    }));
    let observation = accepted_match_statement_corpus_observation(&source);
    assert!(observation.shapes.contains(&StatementShapeFamily::Let));
    assert!(
        !observation.shapes.contains(&StatementShapeFamily::Return),
        "an unrelated checked Return must not enter the Match corpus",
    );
}
