//! Final-HIR statement producer coverage.
//!
//! The producer is intentionally exercised through the private fixture and
//! publication helpers in the parent test module.  The table below is kept
//! explicit so adding a HIR family requires an obvious test-row change rather
//! than silently falling through a wildcard.

use std::sync::atomic::AtomicBool;

use arcweft_lang_hir::{module::HirModule, stmt::HirStmtKind};

use crate::final_analysis::{
    CheckedStatementPayload, CheckedSuspensionStatement, FinalSemanticAnalysis,
    FinalSemanticAnalysisControl, FinalSemanticAnalysisError, FinalSemanticCatalogs,
    PreparedStatementPayload,
};

use super::{Fixture, fixture};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedPayload {
    Structural,
    Assignment,
    Assertion,
    Defer,
    EvaluatedEffectOrStructural,
    Iteration,
    ControlTransfer,
    Trigger,
    UnsafeAudit,
    Select,
    SourceLocale,
    Scope,
    Include,
    Suspension,
    Yield,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MatrixRow {
    name: &'static str,
    tag: u16,
    payload: ExpectedPayload,
}

const MATRIX: &[MatrixRow] = &[
    MatrixRow {
        name: "Assertion",
        tag: 0x0700,
        payload: ExpectedPayload::Assertion,
    },
    MatrixRow {
        name: "Let",
        tag: 0x0701,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Assign",
        tag: 0x0702,
        payload: ExpectedPayload::Assignment,
    },
    MatrixRow {
        name: "LetElse",
        tag: 0x0703,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Return",
        tag: 0x0704,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Out",
        tag: 0x0705,
        payload: ExpectedPayload::ControlTransfer,
    },
    MatrixRow {
        name: "Goto",
        tag: 0x0706,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Defer",
        tag: 0x0707,
        payload: ExpectedPayload::Defer,
    },
    MatrixRow {
        name: "Yield",
        tag: 0x0708,
        payload: ExpectedPayload::Yield,
    },
    MatrixRow {
        name: "Signal",
        tag: 0x0709,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "LifetimeSet",
        tag: 0x070A,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Wait",
        tag: 0x070B,
        payload: ExpectedPayload::Suspension,
    },
    MatrixRow {
        name: "On",
        tag: 0x070C,
        payload: ExpectedPayload::Trigger,
    },
    MatrixRow {
        name: "UnsafeLifetime",
        tag: 0x070D,
        payload: ExpectedPayload::UnsafeAudit,
    },
    MatrixRow {
        name: "Choice",
        tag: 0x070E,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "If",
        tag: 0x070F,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "IfLet",
        tag: 0x0710,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Match",
        tag: 0x0711,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "While",
        tag: 0x0712,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "WhileLet",
        tag: 0x0713,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "For",
        tag: 0x0714,
        payload: ExpectedPayload::Iteration,
    },
    MatrixRow {
        name: "Close",
        tag: 0x0715,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Select",
        tag: 0x0716,
        payload: ExpectedPayload::Select,
    },
    MatrixRow {
        name: "SourceLocale",
        tag: 0x0717,
        payload: ExpectedPayload::SourceLocale,
    },
    MatrixRow {
        name: "Scope",
        tag: 0x0718,
        payload: ExpectedPayload::Scope,
    },
    MatrixRow {
        name: "Include",
        tag: 0x0719,
        payload: ExpectedPayload::Include,
    },
    MatrixRow {
        name: "Break",
        tag: 0x071A,
        payload: ExpectedPayload::ControlTransfer,
    },
    MatrixRow {
        name: "Continue",
        tag: 0x071B,
        payload: ExpectedPayload::ControlTransfer,
    },
    MatrixRow {
        name: "Expression",
        tag: 0x071C,
        payload: ExpectedPayload::EvaluatedEffectOrStructural,
    },
    MatrixRow {
        name: "ProofCall",
        tag: 0x071D,
        payload: ExpectedPayload::Structural,
    },
    MatrixRow {
        name: "Error",
        tag: 0x071E,
        payload: ExpectedPayload::Reject,
    },
];

#[test]
fn producer_matrix_names_and_tags_are_explicit_for_all_hir_families() {
    assert_eq!(MATRIX.len(), 31);
    assert_eq!(
        MATRIX[..30].iter().map(|row| row.tag).collect::<Vec<_>>(),
        (0x0700_u16..=0x071D).collect::<Vec<_>>()
    );
    assert_eq!(MATRIX[30].tag, 0x071E);
    assert_eq!(
        MATRIX.iter().map(|row| row.name).collect::<Vec<_>>(),
        [
            "Assertion",
            "Let",
            "Assign",
            "LetElse",
            "Return",
            "Out",
            "Goto",
            "Defer",
            "Yield",
            "Signal",
            "LifetimeSet",
            "Wait",
            "On",
            "UnsafeLifetime",
            "Choice",
            "If",
            "IfLet",
            "Match",
            "While",
            "WhileLet",
            "For",
            "Close",
            "Select",
            "SourceLocale",
            "Scope",
            "Include",
            "Break",
            "Continue",
            "Expression",
            "ProofCall",
            "Error",
        ]
    );
}

fn expected_payload_for_kind(kind: &HirStmtKind) -> ExpectedPayload {
    match kind {
        HirStmtKind::Assertion { .. } => ExpectedPayload::Assertion,
        HirStmtKind::Let { .. } => ExpectedPayload::Structural,
        HirStmtKind::Assign { .. } => ExpectedPayload::Assignment,
        HirStmtKind::LetElse { .. } => ExpectedPayload::Structural,
        HirStmtKind::Return { .. } => ExpectedPayload::Structural,
        HirStmtKind::Out { .. } => ExpectedPayload::ControlTransfer,
        HirStmtKind::Goto { .. } => ExpectedPayload::Structural,
        HirStmtKind::Defer { .. } => ExpectedPayload::Defer,
        HirStmtKind::Yield { .. } => ExpectedPayload::Yield,
        HirStmtKind::Signal { .. } => ExpectedPayload::Structural,
        HirStmtKind::LifetimeSet { .. } => ExpectedPayload::Structural,
        HirStmtKind::Wait { .. } => ExpectedPayload::Suspension,
        HirStmtKind::On { .. } => ExpectedPayload::Trigger,
        HirStmtKind::UnsafeLifetime { .. } => ExpectedPayload::UnsafeAudit,
        HirStmtKind::Choice { .. } => ExpectedPayload::Structural,
        HirStmtKind::If(_) => ExpectedPayload::Structural,
        HirStmtKind::IfLet(_) => ExpectedPayload::Structural,
        HirStmtKind::Match(_) => ExpectedPayload::Structural,
        HirStmtKind::While(_) => ExpectedPayload::Structural,
        HirStmtKind::WhileLet(_) => ExpectedPayload::Structural,
        HirStmtKind::For(_) => ExpectedPayload::Iteration,
        HirStmtKind::Close { .. } => ExpectedPayload::Structural,
        HirStmtKind::Select(_) => ExpectedPayload::Select,
        HirStmtKind::SourceLocale(_) => ExpectedPayload::SourceLocale,
        HirStmtKind::Scope(_) => ExpectedPayload::Scope,
        HirStmtKind::Include(_) => ExpectedPayload::Include,
        HirStmtKind::Break { .. } => ExpectedPayload::ControlTransfer,
        HirStmtKind::Continue { .. } => ExpectedPayload::ControlTransfer,
        HirStmtKind::Expression { .. } => ExpectedPayload::EvaluatedEffectOrStructural,
        HirStmtKind::ProofCall { .. } => ExpectedPayload::Structural,
        HirStmtKind::Error => ExpectedPayload::Reject,
    }
}

fn payload_family(payload: &CheckedStatementPayload) -> ExpectedPayload {
    match payload {
        CheckedStatementPayload::Structural => ExpectedPayload::Structural,
        CheckedStatementPayload::Assignment(_) => ExpectedPayload::Assignment,
        CheckedStatementPayload::Assertion(_) => ExpectedPayload::Assertion,
        CheckedStatementPayload::Defer(_) => ExpectedPayload::Defer,
        CheckedStatementPayload::EvaluatedEffect(_) => ExpectedPayload::EvaluatedEffectOrStructural,
        CheckedStatementPayload::Iteration(_) => ExpectedPayload::Iteration,
        CheckedStatementPayload::ControlTransfer(_) => ExpectedPayload::ControlTransfer,
        CheckedStatementPayload::Trigger(_) => ExpectedPayload::Trigger,
        CheckedStatementPayload::UnsafeAudit(_) => ExpectedPayload::UnsafeAudit,
        CheckedStatementPayload::Select(_) => ExpectedPayload::Select,
        CheckedStatementPayload::SourceLocale(_) => ExpectedPayload::SourceLocale,
        CheckedStatementPayload::Scope(_) => ExpectedPayload::Scope,
        CheckedStatementPayload::Include(_) => ExpectedPayload::Include,
        CheckedStatementPayload::Suspension(_) => ExpectedPayload::Suspension,
        CheckedStatementPayload::Yield => ExpectedPayload::Yield,
    }
}

fn payload_matches(expected: ExpectedPayload, actual: ExpectedPayload) -> bool {
    expected == actual
        || matches!(
            (expected, actual),
            (
                ExpectedPayload::EvaluatedEffectOrStructural,
                ExpectedPayload::Structural | ExpectedPayload::EvaluatedEffectOrStructural
            )
        )
}

fn root_module<'a>(fixture: &'a Fixture) -> &'a HirModule {
    fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .next()
        .expect("root module")
        .1
}

fn statement_owner_for_tag(fixture: &Fixture, tag: u16) -> arcweft_lang_hir::identity::StmtId {
    root_module(fixture)
        .statements()
        .find_map(|(owner, statement)| {
            (statement.kind().semantic_transcript_tag() == tag).then_some(owner)
        })
        .unwrap_or_else(|| panic!("missing HIR statement row for tag {tag:#06x}"))
}

fn analyze_with_statement_mutation(
    fixture: &Fixture,
    owner: arcweft_lang_hir::identity::StmtId,
    replacement: PreparedStatementPayload,
) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    let cancellation = AtomicBool::new(false);
    super::super::analyzer::analyze_final_project_with_statement_mutation_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
        owner,
        replacement,
    )
}

#[derive(Clone, Copy)]
struct SourceRow {
    matrix: MatrixRow,
    source: &'static str,
}

const CALL_FREE_ROWS: &[SourceRow] = &[
    SourceRow {
        matrix: MATRIX[0],
        source: "flow row { assert.check(true) }\n",
    },
    SourceRow {
        matrix: MATRIX[1],
        source: "flow row { let value = true }\n",
    },
    SourceRow {
        matrix: MATRIX[3],
        source: "flow main() -> String { let value = true else { return \"missing\" } return \"done\" }\n",
    },
    SourceRow {
        matrix: MATRIX[4],
        source: "flow row() -> String { return \"done\" }\n",
    },
    SourceRow {
        matrix: MATRIX[6],
        source: "flow row { goto @flow.done }\nflow done {}\n",
    },
    SourceRow {
        matrix: MATRIX[7],
        source: "flow row { defer () }\n",
    },
    SourceRow {
        matrix: MATRIX[8],
        source: "flow row { yield () }\n",
    },
    SourceRow {
        matrix: MATRIX[9],
        source: "flow row { signal true <- true }\n",
    },
    SourceRow {
        matrix: MATRIX[10],
        source: "flow row { () <- true }\n",
    },
    SourceRow {
        matrix: MATRIX[11],
        source: "flow row { wait(1s) }\n",
    },
    SourceRow {
        matrix: MATRIX[13],
        source: "flow row { unsafe lifetime @unsafe.row { /// SAFETY: test audit\n } }\n",
    },
    SourceRow {
        matrix: MATRIX[15],
        source: "flow row { if true {} }\n",
    },
    SourceRow {
        matrix: MATRIX[16],
        source: "flow row { if let value = true {} }\n",
    },
    SourceRow {
        matrix: MATRIX[17],
        source: "flow row { match true { _ => {} } }\n",
    },
    SourceRow {
        matrix: MATRIX[18],
        source: "flow row { while true {} }\n",
    },
    SourceRow {
        matrix: MATRIX[19],
        source: "flow row { while let value = true {} }\n",
    },
    SourceRow {
        matrix: MATRIX[20],
        source: "flow row { for value in [true, false] {} }\n",
    },
    SourceRow {
        matrix: MATRIX[21],
        source: "flow row { close () }\n",
    },
    SourceRow {
        matrix: MATRIX[23],
        source: "flow row { source locale en-US {} }\n",
    },
    SourceRow {
        matrix: MATRIX[24],
        source: "flow row { scope local {} }\n",
    },
    SourceRow {
        matrix: MATRIX[26],
        source: "flow row { loop { break } }\n",
    },
    SourceRow {
        matrix: MATRIX[27],
        source: "flow row { loop { continue } }\n",
    },
    SourceRow {
        matrix: MATRIX[28],
        source: "flow row {\n    thread { if true {} }\n    return ()\n}\n",
    },
];

const ANALYZER_ROWS: &[SourceRow] = &[
    SourceRow {
        matrix: MATRIX[5],
        source: concat!(
            "pub character @character.akane Akane as akane {}\n",
            "flow line() -> String {\n",
            "    let (_, cue) = akane(voice=auto)[聞いて。[p]]\n",
            "    with:\n",
            "        let actor = akane.stage.acquire(scope=line)\n",
            "        let cue = at(0.42s):\n",
            "            actor.look(.normal, crossfade=120ms)\n",
            "        let voice = line.voice_handle()\n",
            "        out (voice, cue)\n",
            "    return \"done\"\n",
            "}\n",
        ),
    },
    SourceRow {
        matrix: MATRIX[14],
        source: "flow main { choice @.first { @.next \"Next\" -> @flow.done } }\nflow done() -> String { return \"done\" }\n",
    },
    SourceRow {
        matrix: MATRIX[29],
        source: "proof row() { helper(); () }\nproof helper() {}\n",
    },
    SourceRow {
        matrix: MATRIX[12],
        source: "flow row { on true => defer () }\n",
    },
    SourceRow {
        matrix: MATRIX[22],
        source: concat!(
            "flow row() {\n",
            "    select {\n",
            "        value = true => {}\n",
            "    }\n",
            "}\n",
        ),
    },
    SourceRow {
        matrix: MATRIX[25],
        source: "flow row() { include @flow.shared }\nflow shared() {}\n",
    },
];

const ASSIGNMENT_SOURCE: &str = concat!(
    "struct Point { x: i64, active: bool }\n",
    "fn update(point: Point) -> bool {\n",
    "    point.active = true\n",
    "    point.active\n",
    "}\n",
);

#[test]
fn checked_statement_producer_matrix_p24_publishes_analyzer_rows() {
    for row in ANALYZER_ROWS {
        let fixture = if matches!(row.matrix.name, "Out") {
            super::character_nominal_fixture(row.source)
        } else {
            fixture(row.source, None)
        };
        let report = super::analyze(&fixture)
            .unwrap_or_else(|error| panic!("{} analysis failed: {error:?}", row.matrix.name));
        let owner = statement_owner_for_tag(&fixture, row.matrix.tag);
        let hir = root_module(&fixture)
            .resolve_stmt(owner)
            .expect("matrix HIR statement");
        assert_eq!(
            expected_payload_for_kind(hir.kind()),
            row.matrix.payload,
            "wrong HIR family for {}",
            row.matrix.name
        );
        let statement = report
            .statement(owner)
            .unwrap_or_else(|| panic!("missing checked statement for {}", row.matrix.name));
        let actual = payload_family(statement.payload());
        assert!(
            payload_matches(row.matrix.payload, actual),
            "wrong checked payload family for {}: expected {:?}, got {:?}",
            row.matrix.name,
            row.matrix.payload,
            actual
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn checked_statement_producer_matrix_p24_publishes_exact_rows() {
    for row in CALL_FREE_ROWS {
        let fixture = fixture(row.source, None);
        let report = super::analyze(&fixture)
            .unwrap_or_else(|error| panic!("{} analysis failed: {error:?}", row.matrix.name));
        let owner = statement_owner_for_tag(&fixture, row.matrix.tag);
        let hir = root_module(&fixture)
            .resolve_stmt(owner)
            .expect("matrix HIR statement");
        assert_eq!(
            expected_payload_for_kind(hir.kind()),
            row.matrix.payload,
            "wrong HIR family for {}",
            row.matrix.name
        );
        let statement = report
            .statement(owner)
            .unwrap_or_else(|| panic!("missing checked statement for {}", row.matrix.name));
        let actual = payload_family(statement.payload());
        assert!(
            payload_matches(row.matrix.payload, actual),
            "wrong checked payload family for {}: expected {:?}, got {:?}",
            row.matrix.name,
            row.matrix.payload,
            actual
        );
    }
}

#[test]
fn p28_wait_producer_requires_a_duration_operand() {
    let valid = fixture("flow row { wait(1s) }\n", None);
    let report = super::analyze(&valid).expect("duration wait analysis");
    let owner = statement_owner_for_tag(&valid, MATRIX[11].tag);
    assert!(matches!(
        report.statement(owner).expect("checked wait statement").payload(),
        CheckedStatementPayload::Suspension(suspension)
            if matches!(suspension.as_ref(), CheckedSuspensionStatement::Wait)
    ));

    let wrong_type = fixture("flow row { wait(true) }\n", None);
    assert!(matches!(
        super::analyze(&wrong_type),
        Err(FinalSemanticAnalysisError::StatementOperandTypeMismatch {
            expected,
            actual,
            ..
        }) if *expected == crate::types::TypeKind::Duration
            && *actual == crate::types::TypeKind::Bool
    ));

    let mark_operand = fixture("flow row { wait(mark(@.checkpoint)) }\n", None);
    assert!(
        super::analyze(&mark_operand).is_err(),
        "unresolved mark operand must not publish a wait report"
    );
}

#[test]
fn checked_statement_producer_matrix_p24_rejects_error_family() {
    let fixture = fixture("fn bad() { ??? }\n", None);
    assert!(
        fixture.project.executable_view().is_err(),
        "Error HIR family must never reach checked publication"
    );
}

fn wrong_prepared_payload(expected: ExpectedPayload) -> PreparedStatementPayload {
    match expected {
        ExpectedPayload::Assertion
        | ExpectedPayload::Iteration
        | ExpectedPayload::Suspension
        | ExpectedPayload::Yield => PreparedStatementPayload::HirOwned,
        ExpectedPayload::Structural
        | ExpectedPayload::Defer
        | ExpectedPayload::ControlTransfer
        | ExpectedPayload::UnsafeAudit
        | ExpectedPayload::SourceLocale
        | ExpectedPayload::Scope
        | ExpectedPayload::EvaluatedEffectOrStructural => PreparedStatementPayload::Assertion(
            crate::final_analysis::CheckedAssertionDisposition::Discharged,
        ),
        ExpectedPayload::Assignment | ExpectedPayload::Reject => PreparedStatementPayload::HirOwned,
        ExpectedPayload::Trigger | ExpectedPayload::Select | ExpectedPayload::Include => {
            PreparedStatementPayload::Assertion(
                crate::final_analysis::CheckedAssertionDisposition::Discharged,
            )
        }
    }
}

#[test]
fn checked_statement_producer_matrix_n16_rejects_one_explicit_payload_mutation_per_row() {
    for row in CALL_FREE_ROWS {
        let fixture = fixture(row.source, None);
        let owner = statement_owner_for_tag(&fixture, row.matrix.tag);
        let result = analyze_with_statement_mutation(
            &fixture,
            owner,
            wrong_prepared_payload(row.matrix.payload),
        );
        assert!(
            result.is_err(),
            "payload mutation for {} must be rejected",
            row.matrix.name
        );
    }
    for row in ANALYZER_ROWS {
        let fixture = if matches!(row.matrix.name, "Out") {
            super::character_nominal_fixture(row.source)
        } else {
            fixture(row.source, None)
        };
        let owner = statement_owner_for_tag(&fixture, row.matrix.tag);
        let result = analyze_with_statement_mutation(
            &fixture,
            owner,
            wrong_prepared_payload(row.matrix.payload),
        );
        assert!(
            result.is_err(),
            "payload mutation for {} must be rejected",
            row.matrix.name
        );
    }
}

#[test]
fn checked_statement_producer_assignment_row_n16_rejects_hir_owned_mutation() {
    let fixture = fixture(ASSIGNMENT_SOURCE, None);
    let report = super::analyze(&fixture).expect("typed assignment producer");
    let module = root_module(&fixture);
    let owner = module
        .statements()
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::Assign { .. }).then_some(owner)
        })
        .expect("assignment HIR row");
    assert!(matches!(
        report
            .statement(owner)
            .expect("checked assignment statement")
            .payload(),
        CheckedStatementPayload::Assignment(_)
    ));
    assert!(
        analyze_with_statement_mutation(&fixture, owner, PreparedStatementPayload::HirOwned)
            .is_err(),
        "Assignment must reject a HIR-owned payload mutation"
    );
}
