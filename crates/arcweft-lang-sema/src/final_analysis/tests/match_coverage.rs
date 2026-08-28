use std::sync::atomic::AtomicBool;

use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use super::*;
use crate::final_analysis::match_coverage::*;
use crate::final_analysis::semantic_transcript::{CheckedMatch, SemanticTranscriptError};
use crate::final_analysis::{CheckedRecordPatternOwner, CheckedRecordPatternRest};
use crate::final_analysis::{FinalSemanticAnalysisControl, FinalSemanticAnalysisError};

fn build_only_checked_match(
    source: &str,
    limits: CheckedMatchLimits,
) -> Result<CheckedMatch, SemanticTranscriptError> {
    let fixture = fixture(source, None);
    let report = analyze(&fixture).expect("focused Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("focused Match expression");
    report.build_checked_match_for_ref(
        project,
        &fixture.symbols,
        checked_match_reference(&report, module, &fixture.symbols, owner),
        limits,
    )
}

#[test]
fn checked_match_matrix_reports_the_deepest_redundant_or_alternative() {
    let product = build_only_checked_match(
        r"
fn root(pair: (bool, bool)) -> i64 {
    match pair {
        (true | false, true | true) => 1i64
        _ => 0i64
    }
}
",
        CheckedMatchLimits::PRODUCTION,
    )
    .expect("nested Or product coverage");
    assert!(product.coverage().exhaustive());
    let redundant = product
        .coverage()
        .unreachable()
        .iter()
        .find(|row| row.reason() == CheckedUnreachableReason::CoveredByEarlierOrAlternative)
        .expect("nested duplicate Or alternative");
    let coordinate = redundant.alternative().expect("precise Or coordinate");
    assert!(matches!(
        coordinate.steps(),
        [
            crate::semantic_coordinate::StablePatternCoordinateStep::TupleElement(1),
            crate::semantic_coordinate::StablePatternCoordinateStep::OrAlternative(1)
        ]
    ));
}

#[test]
fn checked_match_dynamic_guard_checks_intra_arm_or_without_committing_coverage() {
    let product = build_only_checked_match(
        r"
fn root(flag: bool, ready: bool) -> i64 {
    match flag {
        true | true when ready => 1i64
        false when false => 2i64
        _ => 3i64
    }
}
",
        CheckedMatchLimits::PRODUCTION,
    )
    .expect("dynamic and false guard coverage");
    assert!(product.coverage().exhaustive());
    assert!(product.coverage().unreachable().iter().any(|row| {
        row.reason() == CheckedUnreachableReason::CoveredByEarlierOrAlternative
            && row.arm().ordinal() == 0
            && row.alternative().is_some()
    }));
    assert!(product.coverage().unreachable().iter().any(|row| {
        row.reason() == CheckedUnreachableReason::ConstantFalseGuard && row.arm().ordinal() == 1
    }));
}

#[test]
fn checked_match_matrix_covers_result_payloads_and_choice_injections() {
    let result = build_only_checked_match(
        r"
fn root(value: Result<bool, String>) -> i64 {
    match value {
        .Ok(value) => 1i64
        .Err(error) => 2i64
    }
}
",
        CheckedMatchLimits::PRODUCTION,
    )
    .expect("Result payload constructors");
    assert!(result.coverage().exhaustive());

    let choice = build_only_checked_match(
        r"
fn root(value: String | Bytes) -> i64 {
    match value {
        text: String => 1i64
        bytes: Bytes => 2i64
    }
}
",
        CheckedMatchLimits::PRODUCTION,
    )
    .expect("Choice typed-binding constructors");
    assert!(choice.coverage().exhaustive());
}

#[test]
fn checked_match_matrix_covers_fixed_and_symbolic_sequence_domains() {
    for source in [
        r"
fn root(items: Array<bool, 2>) -> i64 {
    match items {
        [true, true] => 1i64
        [_, ..] => 2i64
    }
}
",
        r"
fn root(items: Vec<bool>) -> i64 {
    match items {
        [] => 0i64
        [_, ..] => 1i64
    }
}
",
        r"
fn root(items: Seq<bool>) -> i64 {
    match items {
        [] => 0i64
        [_, ..] => 1i64
    }
}
",
        r"
fn root(items: Slice<bool>) -> i64 {
    match items {
        [] => 0i64
        [_, ..] => 1i64
    }
}
",
    ] {
        let product = build_only_checked_match(source, CheckedMatchLimits::PRODUCTION)
            .expect("fixed/symbolic sequence coverage");
        assert!(product.coverage().exhaustive());
    }
}

#[test]
fn checked_match_open_literal_domain_retains_other_witness() {
    let result = build_only_checked_match(
        "fn root(value: i64) -> i64 { match value { 1i64 => 1i64 } }\n",
        CheckedMatchLimits::PRODUCTION,
    );
    assert!(matches!(
        result,
        Err(SemanticTranscriptError::NonExhaustive {
            witness: CheckedCoverageWitness::Other { .. }
        })
    ));
}

#[test]
fn checked_match_unobserved_closed_domains_use_exact_catalog_shapes() {
    for (family, source) in [
        (
            "project enum",
            r"
enum Route {
    Opening,
    Closing,
}
fn root(value: Route) -> i64 { match value {} }
",
        ),
        (
            "named environment enum",
            "fn root(value: PresentationLifetime) -> i64 { match value {} }\n",
        ),
        (
            "agent builtin enum",
            "fn root(value: CaptureFormat) -> i64 { match value {} }\n",
        ),
    ] {
        let result = build_only_checked_match(source, CheckedMatchLimits::PRODUCTION);
        assert!(
            matches!(
                &result,
                Err(SemanticTranscriptError::NonExhaustive {
                    witness: CheckedCoverageWitness::Variant { .. }
                })
            ),
            "{family}: {result:?}"
        );
    }
}

#[test]
fn checked_match_unobserved_record_domains_use_full_exact_field_order() {
    let project = build_only_checked_match(
        r"
struct Pair { first: bool, second: bool }
fn root(value: Pair) -> i64 { match value {} }
",
        CheckedMatchLimits::PRODUCTION,
    );
    assert!(matches!(
        project,
        Err(SemanticTranscriptError::NonExhaustive {
            witness: CheckedCoverageWitness::Record { fields, .. }
        }) if matches!(fields.as_ref(), [
            CheckedCoverageWitness::Bool(false),
            CheckedCoverageWitness::Bool(false),
        ])
    ));

    let environment = build_only_checked_match(
        "fn root(value: Transform2D) -> i64 { match value {} }\n",
        CheckedMatchLimits::PRODUCTION,
    );
    assert!(matches!(
        environment,
        Err(SemanticTranscriptError::NonExhaustive {
            witness: CheckedCoverageWitness::Record { fields, .. }
        }) if fields.len() == 10
    ));
}

fn assert_drop_policy_payload_types(report: &FinalSemanticAnalysis) {
    let mut payload_records = report
        .patterns()
        .filter_map(|(_, pattern)| match pattern.resolution() {
            CheckedPatternResolution::Record(record)
                if matches!(
                    record.owner(),
                    CheckedRecordPatternOwner::VariantPayload { .. }
                ) =>
            {
                Some((pattern, record))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    payload_records.sort_by_key(|(_, record)| record.has_rest());
    assert_eq!(
        payload_records.len(),
        2,
        "outer payload and direct re-match"
    );
    let payload_type = payload_records[0].0.ty();
    assert!(matches!(payload_type, TypeKind::VariantPayload(payload)
        if matches!(payload.shape(), crate::types::VariantPayloadShape::Record(fields)
            if matches!(fields.as_ref(), [field]
                if field.diagnostic_name() == "fade" && field.ty() == &TypeKind::Duration))));
    assert_eq!(payload_records[1].0.ty(), payload_type);
    let rest = payload_records
        .iter()
        .find_map(|(_, record)| match record.rest() {
            CheckedRecordPatternRest::Binding(binding) => Some(binding),
            CheckedRecordPatternRest::Absent | CheckedRecordPatternRest::Ignore => None,
        })
        .expect("outer record payload retains the complete rest binding");
    assert_eq!(
        report
            .local(rest.raw())
            .expect("record-rest local is published")
            .ty(),
        payload_type
    );
}

fn assert_drop_policy_coverage(fixture: &Fixture, report: &FinalSemanticAnalysis) {
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
        report.accepted_root_catalog(),
        report,
    );
    let cancellation = AtomicBool::new(false);
    let mut coverage_count = 0;
    for (owner, expression) in module.expressions() {
        let HirExprKind::Match(authored) = expression.kind() else {
            continue;
        };
        let fact = report
            .expression(owner)
            .and_then(|checked| checked.match_fact())
            .expect("ordinary Match fact");
        let match_path = coordinates.expression(owner).expect("stable Match path");
        let arms = authored
            .arms()
            .iter()
            .zip(fact.arms())
            .enumerate()
            .map(|(ordinal, (authored, checked))| {
                assert_eq!(authored.guard(), None);
                assert_eq!(checked.guard(), None);
                CoverageArmInput {
                    coordinate: StableMatchArmCoordinate::new(
                        match_path.clone(),
                        u32::try_from(ordinal).expect("fixture arm ordinal"),
                    ),
                    pattern: authored.pattern(),
                    guard: CheckedGuardClass::Absent,
                }
            })
            .collect::<Vec<_>>();
        let scrutinee_type = report
            .expression(authored.scrutinee())
            .expect("checked Match scrutinee")
            .ty();
        let mut budget = CheckedMatchBudget::new(CheckedMatchLimits::PRODUCTION);
        let coverage = MatchCoverageAnalyzer::new(
            report,
            module,
            FinalSemanticAnalysisControl::new(&cancellation),
            &mut budget,
            crate::semantic_coordinate::StableSemanticCoordinate::new(match_path),
            Vec::new(),
        )
        .analyze(scrutinee_type, &arms)
        .expect("DropPolicy/product coverage is admitted independently of C3 transcription");
        assert!(coverage.exhaustive());
        assert!(coverage.witness().is_none());
        coverage_count += 1;
    }
    assert_eq!(coverage_count, 2, "outer variant and inner payload Matches");
}

#[test]
fn checked_drop_policy_record_payload_rest_keeps_one_internal_product_type() {
    let source = r"
fn root() -> i64 {
    match stop_now {
        .Cancel => 0i64
        .Stop { fade, ..whole } => match whole {
            { fade: _ } => 1i64
        }
        .Finish => 2i64
        .Release => 3i64
        .Detach => 4i64
    }
}
";
    let fixture = fixture(source, None);
    let report = analyze(&fixture).expect("DropPolicy record payload and rest binding analysis");
    assert_drop_policy_payload_types(&report);
    assert_drop_policy_coverage(&fixture, &report);
}

#[test]
fn checked_drop_policy_record_payload_rejects_unknown_missing_and_tuple_shapes() {
    for (label, stop_pattern) in [
        ("unknown field", ".Stop { unknown: _, .. }"),
        ("missing field", ".Stop {}"),
        ("tuple payload", ".Stop(_)"),
    ] {
        let source = format!(
            "fn root() -> i64 {{\n    match stop_now {{\n        {stop_pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
        );
        assert!(
            matches!(
                analyze(&fixture(&source, None)),
                Err(FinalSemanticAnalysisError::PatternTypeUnavailable { .. })
            ),
            "{label}"
        );
    }
}

#[test]
fn checked_match_never_domain_is_exhaustive_without_a_witness() {
    let product = build_only_checked_match(
        "fn root(value: Never) -> i64 { match value {} }\n",
        CheckedMatchLimits::PRODUCTION,
    )
    .expect("empty Never domain");
    assert!(product.coverage().exhaustive());
    assert!(product.coverage().witness().is_none());
}

#[test]
fn checked_match_observes_cancellation_and_matrix_limits_atomically() {
    let source = r"
fn root(flag: bool) -> i64 {
    match flag {
        true => 1i64
        false => 2i64
    }
}
";
    let fixture = fixture(source, None);
    let report = analyze(&fixture).expect("cancellation Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        report.build_checked_match_for_ref_with_control(
            project,
            &fixture.symbols,
            checked_match_reference(&report, module, &fixture.symbols, owner),
            CheckedMatchLimits::PRODUCTION,
            FinalSemanticAnalysisControl::new(&cancelled),
        ),
        Err(SemanticTranscriptError::Generation(error))
            if matches!(*error, FinalSemanticAnalysisError::Cancelled)
    ));

    let no_matrix_rows =
        CheckedMatchLimits::PRODUCTION.with_limit(CheckedMatchLimitKind::MatrixRows, 0);
    for _ in 0..2 {
        assert!(matches!(
            build_only_checked_match(source, no_matrix_rows),
            Err(SemanticTranscriptError::MatchBuild(
                CheckedMatchBuildError::LimitExceeded {
                    kind: CheckedMatchLimitKind::MatrixRows,
                    limit: 0,
                    attempted: 1,
                }
            ))
        ));
    }
}
