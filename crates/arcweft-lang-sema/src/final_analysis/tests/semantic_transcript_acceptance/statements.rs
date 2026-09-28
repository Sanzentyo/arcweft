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
    HirStmtKind::Assertion { .. } => Assertion => Pending,
    HirStmtKind::Let { .. } => Let => Accepted,
    HirStmtKind::Assign { .. } => Assign => Pending,
    HirStmtKind::LetElse { .. } => LetElse => Pending,
    HirStmtKind::Return { .. } => Return => Pending,
    HirStmtKind::Out { .. } => Out => Pending,
    HirStmtKind::Goto { .. } => Goto => Pending,
    HirStmtKind::Defer { .. } => Defer => Pending,
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
    HirStmtKind::For(_) => For => Pending,
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
    CheckedStatementPayload::Assignment(_) => Assignment => Pending,
    CheckedStatementPayload::Assertion(_) => Assertion => Pending,
    CheckedStatementPayload::Defer(_) => Defer => Pending,
    CheckedStatementPayload::EvaluatedEffect(_) => EvaluatedEffect => Pending,
    CheckedStatementPayload::Iteration(_) => Iteration => Pending,
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
    }
    assert!(
        !shapes.is_empty(),
        "Match root has checked statement descendants"
    );
    MatchStatementCorpusObservation { shapes, payloads }
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
