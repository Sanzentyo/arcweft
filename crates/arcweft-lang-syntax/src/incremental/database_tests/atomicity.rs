use std::fmt::Write as _;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceName, SourceRange};

use super::{private_slots, source_document, source_edit, syntax_database};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::SyntaxKind;
use crate::incremental::{ParseFailure, ParsedSource, SyntaxDatabase, SyntaxLimit};
use crate::parser::ParseOptions;

#[test]
fn fatal_event_validation_failure_is_atomic() {
    let name = SourceName::path("fatal-event-validation.arcw");
    let initial_source = "proof first() = ()\n";
    let next_source = "proof first() = ()\nproof second() = ()\n";
    let mut database = syntax_database();
    let next_lineage = database.transaction.next_lineage_for_test();

    let failed = database.parse_initial_with_event_validation_failure(
        &SourceSnapshotId::initial(name.clone()),
        &source_document(&name, initial_source),
    );
    assert!(matches!(failed, Err(ParseFailure::Invariant(_))));
    assert!(database.lineages.is_empty());
    assert_eq!(database.transaction.next_lineage_for_test(), next_lineage);

    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            ParseOptions::default(),
        )
        .expect("valid initial transaction after rejected malformed events");
    let next_node = database
        .lineages
        .get(&name)
        .expect("accepted lineage")
        .transaction
        .next_node_for_test();
    let next_lineage = database.transaction.next_lineage_for_test();
    let edit = source_edit(
        &initial,
        SourceRange::new(0, initial_source.len()),
        next_source,
    );
    let failed =
        database.reparse_with_event_validation_failure(&initial, std::slice::from_ref(&edit));
    assert!(matches!(failed, Err(ParseFailure::Invariant(_))));
    assert_unchanged(&database, &name, &initial, next_node, next_lineage);

    let accepted = database
        .reparse(&initial, &[edit], ParseOptions::default())
        .expect("valid retry after rejected malformed events");
    let mut control = syntax_database();
    let control_initial = control
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            ParseOptions::default(),
        )
        .expect("control initial transaction");
    let control = control
        .reparse(
            &control_initial,
            &[source_edit(
                &control_initial,
                SourceRange::new(0, initial_source.len()),
                next_source,
            )],
            ParseOptions::default(),
        )
        .expect("control reparse transaction");
    assert_eq!(private_slots(&accepted), private_slots(&control));
}

#[test]
fn fatal_attachment_failure_is_atomic() {
    let name = SourceName::path("fatal-attachment.arcw");
    let initial_source = "proof first() = ()\n";
    let next_source = "proof first() = ()\nproof second() = ()\n";
    let mut database = syntax_database();
    let next_lineage = database.transaction.next_lineage_for_test();

    let failed = database.parse_initial_with_attachment_failure(
        &SourceSnapshotId::initial(name.clone()),
        &source_document(&name, initial_source),
    );
    assert!(matches!(failed, Err(ParseFailure::Attachment(_))));
    assert!(database.lineages.is_empty());
    assert_eq!(database.transaction.next_lineage_for_test(), next_lineage);

    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            ParseOptions::default(),
        )
        .expect("valid initial transaction after rejected attachment");
    let next_node = database
        .lineages
        .get(&name)
        .expect("accepted lineage")
        .transaction
        .next_node_for_test();
    let next_lineage = database.transaction.next_lineage_for_test();
    let edit = source_edit(
        &initial,
        SourceRange::new(0, initial_source.len()),
        next_source,
    );
    let failed = database.reparse_with_attachment_failure(&initial, std::slice::from_ref(&edit));
    assert!(matches!(failed, Err(ParseFailure::Attachment(_))));
    assert_unchanged(&database, &name, &initial, next_node, next_lineage);

    let accepted = database
        .reparse(&initial, &[edit], ParseOptions::default())
        .expect("valid retry after rejected attachment");
    let mut control = syntax_database();
    let control_initial = control
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            ParseOptions::default(),
        )
        .expect("control initial transaction");
    let control = control
        .reparse(
            &control_initial,
            &[source_edit(
                &control_initial,
                SourceRange::new(0, initial_source.len()),
                next_source,
            )],
            ParseOptions::default(),
        )
        .expect("control reparse transaction");
    assert_eq!(private_slots(&accepted), private_slots(&control));
}

#[test]
fn predicate_parameter_limit_is_inclusive_and_atomic() {
    let limit = SyntaxLimit::PredicateParameters;
    assert_eq!(limit.maximum(), 64);
    assert_declaration_limit_is_inclusive_and_atomic(
        limit,
        SyntaxKind::Parameter,
        &predicate_or_proof_with_parameters("predicate", "limited", limit.maximum()),
        &predicate_or_proof_with_parameters("predicate", "limited", limit.maximum() + 1),
        &predicate_or_proof_with_parameters("predicate", "retried", limit.maximum()),
    );
}

#[test]
fn proof_parameter_limit_is_inclusive_and_atomic() {
    let limit = SyntaxLimit::ProofParameters;
    assert_eq!(limit.maximum(), 64);
    assert_declaration_limit_is_inclusive_and_atomic(
        limit,
        SyntaxKind::Parameter,
        &predicate_or_proof_with_parameters("proof", "limited", limit.maximum()),
        &predicate_or_proof_with_parameters("proof", "limited", limit.maximum() + 1),
        &predicate_or_proof_with_parameters("proof", "retried", limit.maximum()),
    );
}

#[test]
fn generic_parameter_limit_is_inclusive_and_atomic() {
    let limit = SyntaxLimit::GenericParameters;
    assert_eq!(limit.maximum(), 256);
    assert_declaration_limit_is_inclusive_and_atomic(
        limit,
        SyntaxKind::GenericParameter,
        &proof_with_generics("limited", limit.maximum()),
        &proof_with_generics("limited", limit.maximum() + 1),
        &proof_with_generics("retried", limit.maximum()),
    );
}

#[test]
fn where_predicate_limit_is_inclusive_and_atomic() {
    let limit = SyntaxLimit::WherePredicates;
    assert_eq!(limit.maximum(), 256);
    assert_declaration_limit_is_inclusive_and_atomic(
        limit,
        SyntaxKind::WherePredicate,
        &proof_with_where_predicates("limited", limit.maximum()),
        &proof_with_where_predicates("limited", limit.maximum() + 1),
        &proof_with_where_predicates("retried", limit.maximum()),
    );
}

#[test]
fn contract_clause_limit_is_inclusive_and_atomic() {
    assert_eq!(SyntaxLimit::ContractClauses.maximum(), 64);
    for keyword in ["predicate", "proof"] {
        let body = if keyword == "predicate" { "true" } else { "()" };
        let exact_source =
            predicate_or_proof_with_clauses(keyword, SyntaxLimit::ContractClauses.maximum(), body);
        let over_source = predicate_or_proof_with_clauses(
            keyword,
            SyntaxLimit::ContractClauses.maximum() + 1,
            body,
        );
        let retry_source = exact_source.replacen("limited", "retried", 1);
        let name = SourceName::path(format!("{keyword}-mixed-clause-limit.arcw"));
        let mut database = syntax_database();
        let exact = database
            .parse_initial(
                SourceSnapshotId::initial(name.clone()),
                source_document(&name, exact_source.clone()),
                ParseOptions::default(),
            )
            .expect("the exact mixed requires/ensures limit commits");
        let requires = exact
            .attached()
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::RequiresClause)
            .count();
        let ensures = exact
            .attached()
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::EnsuresClause)
            .count();
        assert!(requires > 0 && ensures > 0);
        assert_eq!(requires + ensures, SyntaxLimit::ContractClauses.maximum());

        let next_node = database
            .lineages
            .get(&name)
            .expect("accepted lineage")
            .transaction
            .next_node_for_test();
        let next_lineage = database.transaction.next_lineage_for_test();
        let failed = database.reparse(
            &exact,
            &[source_edit(
                &exact,
                SourceRange::new(0, exact_source.len()),
                over_source,
            )],
            ParseOptions::default(),
        );
        assert!(matches!(
            failed,
            Err(ParseFailure::LimitExceeded(SyntaxLimit::ContractClauses))
        ));
        assert_unchanged(&database, &name, &exact, next_node, next_lineage);

        assert_retry_matches_control(database, &name, &exact, &exact_source, &retry_source, None);
    }
}

#[test]
fn diagnostic_limit_is_inclusive_and_atomic() {
    let limit = SyntaxLimit::Diagnostics;
    assert_eq!(limit.maximum(), 1_024);
    let diagnostic = |ordinal, message| {
        SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.test.limit",
            SourceRange::new(ordinal, ordinal + 1),
            message,
        ))
    };
    let mut budget = GrammarBudget::default();
    assert!(budget.event(&diagnostic(0, "first presentation")));
    assert!(budget.event(&diagnostic(0, "same identity, changed presentation")));
    for ordinal in 1..limit.maximum() {
        assert!(budget.event(&diagnostic(ordinal, "unique diagnostic identity")));
    }
    assert_eq!(budget.failure(), None);
    assert!(!budget.event(&diagnostic(limit.maximum(), "one diagnostic identity over")));
    assert_eq!(budget.failure(), Some(limit));

    let name = SourceName::path("diagnostic-limit-atomic.arcw");
    let exact_source = "unknown_top_level\n".repeat(limit.maximum());
    let mut database = syntax_database();
    let exact = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, exact_source.clone()),
            ParseOptions::default(),
        )
        .expect("the 1,024th exact diagnostic identity commits");
    assert_eq!(exact.status(), crate::incremental::ParseStatus::Recovered);
    assert_eq!(exact.diagnostics().len(), limit.maximum());
    assert_eq!(
        exact.syntax_stats().diagnostic_identities(),
        limit.maximum()
    );

    let next_node = database
        .lineages
        .get(&name)
        .expect("accepted lineage")
        .transaction
        .next_node_for_test();
    let next_lineage = database.transaction.next_lineage_for_test();
    let over_source = format!("{exact_source}unknown_top_level\n");
    let failed = database.reparse(
        &exact,
        &[source_edit(
            &exact,
            SourceRange::new(0, exact_source.len()),
            over_source,
        )],
        ParseOptions::default(),
    );
    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(SyntaxLimit::Diagnostics))
    ));
    assert_unchanged(&database, &name, &exact, next_node, next_lineage);

    let retry_source = exact_source.replacen("unknown_top_level", "unknown_retried", 1);
    assert_retry_matches_control(database, &name, &exact, &exact_source, &retry_source, None);
}

#[test]
fn statement_limit_is_inclusive_and_atomic() {
    assert_global_limit_is_inclusive_and_atomic(
        SyntaxLimit::Statements,
        "proof limited() { first(); }\n",
        "proof limited() { first(); second(); }\n",
        "proof retried() { first(); }\n",
    );
}

#[test]
fn expression_limit_is_inclusive_and_atomic() {
    assert_global_limit_is_inclusive_and_atomic(
        SyntaxLimit::Expressions,
        "proof limited() = 1\n",
        "proof limited() = build()\n",
        "proof retried() = 2\n",
    );
}

#[test]
fn type_limit_is_inclusive_and_atomic() {
    assert_global_limit_is_inclusive_and_atomic(
        SyntaxLimit::TypeNodes,
        "proof limited() -> Int = ()\n",
        "proof limited(value: Int) -> Int = ()\n",
        "proof retried() -> Int = ()\n",
    );
}

#[test]
fn pattern_limit_is_inclusive_and_atomic() {
    assert_global_limit_is_inclusive_and_atomic(
        SyntaxLimit::PatternNodes,
        "proof limited(value: Int) = ()\n",
        "proof limited(left: Int, right: Int) = ()\n",
        "proof retried(value: Int) = ()\n",
    );
}

#[test]
fn identity_bearing_node_limit_is_inclusive_and_atomic() {
    assert_global_limit_is_inclusive_and_atomic(
        SyntaxLimit::IdentityBearingNodes,
        "proof limited() = 1\n",
        "pub proof limited() = 1\n",
        "proof retried() = 1\n",
    );
}

fn predicate_or_proof_with_clauses(keyword: &str, count: usize, body: &str) -> String {
    let requires = count / 2;
    let mut source = format!("{keyword} limited()\n");
    for ordinal in 0..requires {
        writeln!(source, "requires requirement_{ordinal}").unwrap();
    }
    for ordinal in requires..count {
        writeln!(source, "ensures guarantee_{ordinal}").unwrap();
    }
    writeln!(source, "= {body}").unwrap();
    source
}

fn predicate_or_proof_with_parameters(keyword: &str, name: &str, count: usize) -> String {
    let parameters = comma_separated(count, |ordinal| format!("p{ordinal}: Bool"));
    let body = if keyword == "predicate" { "true" } else { "()" };
    format!("{keyword} {name}({parameters}) = {body}\n")
}

fn proof_with_generics(name: &str, count: usize) -> String {
    let parameters = comma_separated(count, |ordinal| format!("T{ordinal}"));
    format!("proof {name}<{parameters}>() = ()\n")
}

fn proof_with_where_predicates(name: &str, count: usize) -> String {
    let predicates = comma_separated(count, |ordinal| format!("T{ordinal}: Ord"));
    format!("proof {name}() where {predicates} = ()\n")
}

fn comma_separated(count: usize, element: impl Fn(usize) -> String) -> String {
    (0..count).map(element).collect::<Vec<_>>().join(", ")
}

fn assert_declaration_limit_is_inclusive_and_atomic(
    limit: SyntaxLimit,
    counted_kind: SyntaxKind,
    exact_source: &str,
    over_source: &str,
    retry_source: &str,
) {
    let name = SourceName::path(format!("{limit:?}-atomic.arcw").to_ascii_lowercase());
    let mut database = syntax_database();
    let exact = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, exact_source),
            ParseOptions::default(),
        )
        .expect("the exact declaration-owned production limit commits");
    assert_eq!(exact.status(), crate::incremental::ParseStatus::Clean);
    assert_eq!(
        exact
            .attached()
            .nodes()
            .filter(|node| node.kind() == counted_kind)
            .count(),
        limit.maximum()
    );

    let next_node = database
        .lineages
        .get(&name)
        .expect("accepted lineage")
        .transaction
        .next_node_for_test();
    let next_lineage = database.transaction.next_lineage_for_test();
    let failed = database.reparse(
        &exact,
        &[source_edit(
            &exact,
            SourceRange::new(0, exact_source.len()),
            over_source,
        )],
        ParseOptions::default(),
    );
    assert!(matches!(failed, Err(ParseFailure::LimitExceeded(actual)) if actual == limit));
    assert_unchanged(&database, &name, &exact, next_node, next_lineage);

    assert_retry_matches_control(database, &name, &exact, exact_source, retry_source, None);
}

fn assert_global_limit_is_inclusive_and_atomic(
    limit: SyntaxLimit,
    exact_source: &str,
    over_source: &str,
    retry_source: &str,
) {
    let exact_count = parsed_count(exact_source, limit);
    let over_count = parsed_count(over_source, limit);
    let retry_count = parsed_count(retry_source, limit);
    assert!(exact_count > 0);
    assert_eq!(over_count, exact_count + 1);
    assert_eq!(retry_count, exact_count);
    let already_charged = limit
        .maximum()
        .checked_sub(exact_count)
        .expect("fixture count fits the production limit");

    let name = SourceName::path(format!("{limit:?}-atomic.arcw").to_ascii_lowercase());
    let mut database = syntax_database();
    let exact = database
        .parse_initial_with_global_count(
            &SourceSnapshotId::initial(name.clone()),
            &source_document(&name, exact_source),
            limit,
            already_charged,
        )
        .expect("the exact production limit commits");
    assert_eq!(parsed_count_from_snapshot(&exact, limit), exact_count);

    let next_node = database
        .lineages
        .get(&name)
        .expect("accepted lineage")
        .transaction
        .next_node_for_test();
    let next_lineage = database.transaction.next_lineage_for_test();
    let failed = database.reparse_with_global_count(
        &exact,
        &[source_edit(
            &exact,
            SourceRange::new(0, exact_source.len()),
            over_source,
        )],
        limit,
        already_charged,
    );
    assert!(matches!(failed, Err(ParseFailure::LimitExceeded(actual)) if actual == limit));
    assert_unchanged(&database, &name, &exact, next_node, next_lineage);

    assert_retry_matches_control(
        database,
        &name,
        &exact,
        exact_source,
        retry_source,
        Some((limit, already_charged)),
    );
}

fn parsed_count(source: &str, limit: SyntaxLimit) -> usize {
    let name = SourceName::path("syntax-limit-count.arcw");
    let parsed = syntax_database()
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            ParseOptions::default(),
        )
        .expect("fixture parses under production limits");
    parsed_count_from_snapshot(&parsed, limit)
}

fn parsed_count_from_snapshot(parsed: &ParsedSource, limit: SyntaxLimit) -> usize {
    let stats = parsed.syntax_stats();
    match limit {
        SyntaxLimit::Statements => stats.statements(),
        SyntaxLimit::Expressions => stats.expressions(),
        SyntaxLimit::TypeNodes => stats.type_nodes(),
        SyntaxLimit::PatternNodes => stats.pattern_nodes(),
        SyntaxLimit::IdentityBearingNodes => stats.identity_bearing_nodes(),
        _ => panic!("{limit:?} is not a global grammar-node budget"),
    }
}

fn assert_retry_matches_control(
    mut database: SyntaxDatabase,
    name: &SourceName,
    exact: &ParsedSource,
    exact_source: &str,
    retry_source: &str,
    seed: Option<(SyntaxLimit, usize)>,
) {
    let expected_generation = exact
        .source_snapshot_id()
        .generation()
        .get()
        .checked_add(1)
        .expect("test generation remains representable");
    let retry_edit = source_edit(exact, SourceRange::new(0, exact_source.len()), retry_source);
    let accepted = match seed {
        Some((limit, already_charged)) => {
            database.reparse_with_global_count(exact, &[retry_edit], limit, already_charged)
        }
        None => database.reparse(exact, &[retry_edit], ParseOptions::default()),
    }
    .expect("valid retry after rejected transaction");

    let mut control_database = syntax_database();
    let control_exact = match seed {
        Some((limit, already_charged)) => control_database.parse_initial_with_global_count(
            &SourceSnapshotId::initial(name.clone()),
            &source_document(name, exact_source),
            limit,
            already_charged,
        ),
        None => control_database.parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(name, exact_source),
            ParseOptions::default(),
        ),
    }
    .expect("control exact-limit transaction");
    let control_edit = source_edit(
        &control_exact,
        SourceRange::new(0, exact_source.len()),
        retry_source,
    );
    let control = match seed {
        Some((limit, already_charged)) => control_database.reparse_with_global_count(
            &control_exact,
            &[control_edit],
            limit,
            already_charged,
        ),
        None => control_database.reparse(&control_exact, &[control_edit], ParseOptions::default()),
    }
    .expect("control retry transaction");

    assert_eq!(
        accepted.source_snapshot_id().generation().get(),
        expected_generation
    );
    assert_eq!(private_slots(&accepted), private_slots(&control));
    assert_eq!(
        database
            .lineages
            .get(name)
            .expect("accepted retry lineage")
            .transaction
            .next_node_for_test(),
        control_database
            .lineages
            .get(name)
            .expect("control retry lineage")
            .transaction
            .next_node_for_test()
    );
    assert_eq!(
        database.transaction.next_lineage_for_test(),
        control_database.transaction.next_lineage_for_test()
    );
}

fn assert_unchanged(
    database: &SyntaxDatabase,
    name: &SourceName,
    expected: &ParsedSource,
    next_node: Option<core::num::NonZeroU64>,
    next_lineage: Option<core::num::NonZeroU64>,
) {
    let current = database.lineages.get(name).expect("current lineage");
    assert!(current.current.is_same_snapshot(expected));
    assert!(Arc::ptr_eq(current.current.data(), expected.data()));
    assert!(Arc::ptr_eq(current.transaction.current(), expected.data()));
    assert_eq!(current.current.document(), expected.document());
    assert_eq!(current.current.diagnostics(), expected.diagnostics());
    assert_eq!(current.current.status(), expected.status());
    assert_eq!(current.current.syntax_stats(), expected.syntax_stats());
    assert_eq!(current.transaction.next_node_for_test(), next_node);
    assert_eq!(database.transaction.next_lineage_for_test(), next_lineage);
}
