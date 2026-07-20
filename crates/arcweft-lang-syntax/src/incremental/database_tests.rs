use super::{ParseFailure, ParsedSource, SyntaxDatabase, SyntaxIdentityKind, SyntaxNodeId};
use crate::parser::{parse_source, recovery::ParseErrorKind};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};
use core::num::NonZeroU64;
use std::collections::HashSet;
use std::sync::Arc;

use crate::attachment::{PredicateItemKind, SyntaxLookupError, SyntaxNodeHandle};
use crate::grammar::kinds::SyntaxKind as GrammarKind;

fn source_document(name: &SourceName, text: impl Into<Arc<str>>) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(name.display_name()).expect("valid test document id"),
        name.clone(),
        text,
    )
    .expect("test source document")
}

fn source_edit(
    source: &ParsedSource,
    range: SourceRange,
    replacement: impl Into<String>,
) -> SourceEdit {
    SourceEdit::new(
        source
            .document()
            .span(range)
            .expect("valid edit range in the exact source revision"),
        replacement,
    )
}

#[test]
fn syntax_node_ids_retain_the_full_non_zero_slot_domain() {
    let last = SyntaxNodeId(NonZeroU64::new(u64::MAX).unwrap());
    assert_eq!(last.0.get(), u64::MAX);
}

#[test]
fn trivia_reparse_advances_once_and_preserves_every_node_identity() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "flow first {}\nflow second {}\n"),
        )
        .expect("initial parse");
    let initial_ids = initial
        .root()
        .descendants()
        .map(|node| initial.identities().id_for(&node).expect("node identity"))
        .collect::<Vec<_>>();

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(4, 5), "   ")],
        )
        .expect("trivia reparse");
    let reparsed_ids = reparsed
        .root()
        .descendants()
        .map(|node| reparsed.identities().id_for(&node).expect("node identity"))
        .collect::<Vec<_>>();
    assert_eq!(reparsed.snapshot().generation().get(), 2);
    assert_eq!(initial_ids, reparsed_ids);
}

#[test]
fn recovered_line_indentation_does_not_invent_a_parent_or_replace_ids() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "unknown\n    also_unknown\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("recovered source commits");
    let first_id = line_id(&initial, "unknown\n");
    let second_id = line_id(&initial, "also_unknown");

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(8, 12), "        ")],
        )
        .expect("indentation-only recovery edit commits");

    assert_eq!(line_id(&reparsed, "unknown\n"), first_id);
    assert_eq!(line_id(&reparsed, "also_unknown"), second_id);
}

#[test]
fn no_op_replacements_return_the_exact_current_snapshot() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "flow story {}\n"),
        )
        .expect("initial parse");
    let unchanged = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(5, 10), "story")],
        )
        .expect("no-op reparse");
    assert!(Arc::ptr_eq(&initial, &unchanged));
    assert_eq!(unchanged.snapshot().generation().get(), 1);

    let empty = database
        .reparse(&initial, &[])
        .expect("an empty edit transaction is a no-op");
    assert!(Arc::ptr_eq(&initial, &empty));
}

#[test]
fn replaced_subtrees_get_fresh_ids_without_retiring_unchanged_siblings() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow first {\n    log.info(\"old\")\n}\nflow second {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial parse");
    let replaced_id = smallest_node_id_containing(&initial, "log.info(\"old\")");
    let sibling_id = line_id(&initial, "second");
    let start = source.find("old").expect("fixture token");

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(start, start + "old".len()),
                "new",
            )],
        )
        .expect("replacement parses");

    assert_ne!(
        smallest_node_id_containing(&reparsed, "log.info(\"new\")"),
        replaced_id
    );
    assert_eq!(line_id(&reparsed, "second"), sibling_id);
}

#[test]
fn moving_a_node_across_parents_allocates_a_fresh_identity() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow first {\n    log.info(\"move\")\n}\nflow second {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial parse");
    let moved_id = smallest_node_id_containing(&initial, "log.info(\"move\")");
    let moved_source = "flow first {}\nflow second {\n    log.info(\"move\")\n}\n";

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                moved_source,
            )],
        )
        .expect("parent move parses");

    assert_ne!(
        smallest_node_id_containing(&reparsed, "log.info(\"move\")"),
        moved_id
    );
}

#[test]
fn moving_a_node_across_indentation_parents_allocates_a_fresh_identity() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow story {\n    first:\n        log.info(\"move\")\n    second:\n}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("recovered indentation fixture parses");
    let moved_id = smallest_node_id_containing(&initial, "log.info(\"move\")");
    let moved_source = "flow story {\n    first:\n    second:\n        log.info(\"move\")\n}\n";

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                moved_source,
            )],
        )
        .expect("indentation parent move parses");

    assert_ne!(
        smallest_node_id_containing(&reparsed, "log.info(\"move\")"),
        moved_id
    );
}

#[test]
fn reordering_unique_nodes_inside_one_parent_retains_their_identities() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow story {\n    log.info(\"first\")\n    log.info(\"second\")\n}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial parse");
    let first_id = smallest_node_id_containing(&initial, "log.info(\"first\")");
    let second_id = smallest_node_id_containing(&initial, "log.info(\"second\")");
    let reordered = "flow story {\n    log.info(\"second\")\n    log.info(\"first\")\n}\n";

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                reordered,
            )],
        )
        .expect("same-parent reorder parses");

    assert_eq!(
        smallest_node_id_containing(&reparsed, "log.info(\"first\")"),
        first_id
    );
    assert_eq!(
        smallest_node_id_containing(&reparsed, "log.info(\"second\")"),
        second_id
    );
}

#[test]
fn equivalent_recovery_nodes_survive_trivia_edits() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow story {\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("recovered source commits");
    assert_eq!(initial.status(), super::ParseStatus::Recovered);
    let initial_ids = initial
        .root()
        .descendants()
        .map(|node| initial.identities().id_for(&node).expect("node identity"))
        .collect::<Vec<_>>();
    let insertion = source.find("story").expect("fixture token");

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(insertion, insertion),
                "  ",
            )],
        )
        .expect("trivia edit commits");
    let reparsed_ids = reparsed
        .root()
        .descendants()
        .map(|node| reparsed.identities().id_for(&node).expect("node identity"))
        .collect::<Vec<_>>();

    assert_eq!(reparsed.status(), super::ParseStatus::Recovered);
    assert_eq!(initial_ids, reparsed_ids);
}

#[test]
fn unique_moved_siblings_keep_ids_and_copies_get_fresh_ids() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow first {}\nflow second {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial parse");
    let first_id = line_id(&initial, "first");
    let second_id = line_id(&initial, "second");
    let moved_text = "flow second {}\nflow first {}\n";
    let moved = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                moved_text,
            )],
        )
        .expect("moved reparse");
    assert_eq!(line_id(&moved, "first"), first_id);
    assert_eq!(line_id(&moved, "second"), second_id);

    let copied_text = "flow second {}\nflow first {}\nflow first {}\n";
    let copied = database
        .reparse(
            &moved,
            &[source_edit(
                &moved,
                SourceRange::new(0, moved_text.len()),
                copied_text,
            )],
        )
        .expect("copied reparse");
    let copied_first_ids = copied
        .root()
        .children()
        .filter(|node| node.text().to_string().contains("first"))
        .map(|node| copied.identities().id_for(&node).expect("line identity"))
        .collect::<Vec<_>>();
    assert_eq!(copied_first_ids.len(), 2);
    assert_eq!(copied_first_ids[0], first_id);
    assert_ne!(copied_first_ids[1], first_id);
}

#[test]
fn inserting_between_unique_siblings_preserves_existing_ids() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow first {}\nflow second {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial parse");
    let first_id = line_id(&initial, "first");
    let second_id = line_id(&initial, "second");
    let insertion = source.find("flow second").expect("second sibling");

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(insertion, insertion),
                "flow inserted {}\n",
            )],
        )
        .expect("sibling insertion parses");

    assert_eq!(line_id(&reparsed, "first"), first_id);
    assert_eq!(line_id(&reparsed, "second"), second_id);
    assert_ne!(line_id(&reparsed, "inserted"), first_id);
    assert_ne!(line_id(&reparsed, "inserted"), second_id);
}

#[test]
fn repeated_identical_siblings_follow_distance_then_old_id_ties() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow story {\n    log.info(\"same\")\n    log.info(\"same\")\n}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial parse");
    let initial_ids = line_ids_containing(&initial, "log.info(\"same\")");
    assert_eq!(initial_ids.len(), 2);
    let copied_source =
        "flow story {\n    log.info(\"same\")\n    log.info(\"same\")\n    log.info(\"same\")\n}\n";

    let copied = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                copied_source,
            )],
        )
        .expect("copy parses");
    let copied_ids = line_ids_containing(&copied, "log.info(\"same\")");

    assert_eq!(copied_ids.len(), 3);
    assert_eq!(&copied_ids[..2], initial_ids.as_slice());
    assert!(!initial_ids.contains(&copied_ids[2]));
}

#[test]
fn invalid_edit_order_overlap_and_foreign_provenance_leave_lineage_unchanged() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let source = "flow café {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial parse");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .allocator
        .next;

    let foreign_name = SourceName::path("other.arcw");
    let foreign = source_document(&foreign_name, source);
    let failures = [
        database.reparse(
            &initial,
            &[
                source_edit(&initial, SourceRange::new(5, 5), "x"),
                source_edit(&initial, SourceRange::new(0, 0), "y"),
            ],
        ),
        database.reparse(
            &initial,
            &[
                source_edit(&initial, SourceRange::new(0, 4), "x"),
                source_edit(&initial, SourceRange::new(3, 5), "y"),
            ],
        ),
        database.reparse(
            &initial,
            &[SourceEdit::new(
                foreign
                    .span(SourceRange::new(0, 0))
                    .expect("valid foreign span"),
                "x",
            )],
        ),
    ];

    assert!(matches!(
        &failures[0],
        Err(ParseFailure::InvalidEdits(
            super::InvalidEditSet::Unsorted { .. }
        ))
    ));
    assert!(matches!(
        &failures[1],
        Err(ParseFailure::InvalidEdits(
            super::InvalidEditSet::Overlapping { .. }
        ))
    ));
    assert!(matches!(&failures[2], Err(ParseFailure::SourceMismatch)));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&current.current, &initial));
    assert_eq!(current.allocator.next, allocator_next);
}

#[test]
fn reparsing_a_stale_snapshot_is_rejected_without_mutation() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let initial_source = "flow story {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
        )
        .expect("initial parse");
    let current = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(5, 10), "current")],
        )
        .expect("current parse");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .allocator
        .next;

    let stale = database.reparse(
        &initial,
        &[source_edit(&initial, SourceRange::new(5, 10), "stale")],
    );

    assert!(matches!(stale, Err(ParseFailure::SourceMismatch)));
    let lineage = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&lineage.current, &current));
    assert_eq!(lineage.allocator.next, allocator_next);
}

#[test]
fn reparsing_a_snapshot_from_another_database_is_rejected_without_mutation() {
    let name = SourceName::path("story.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let source: Arc<str> = Arc::from("flow story {}\n");
    let mut local = SyntaxDatabase::default();
    let local_initial = local
        .parse_initial(
            snapshot.clone(),
            source_document(&name, Arc::clone(&source)),
        )
        .expect("local initial parse");
    let mut foreign = SyntaxDatabase::default();
    let foreign_initial = foreign
        .parse_initial(snapshot, source_document(&name, source))
        .expect("foreign initial parse");
    let allocator_next = local
        .lineages
        .get(&name)
        .expect("local lineage")
        .allocator
        .next;

    let rejected = local.reparse(
        &foreign_initial,
        &[source_edit(
            &foreign_initial,
            SourceRange::new(5, 10),
            "foreign",
        )],
    );

    assert!(matches!(rejected, Err(ParseFailure::SourceMismatch)));
    let lineage = local.lineages.get(&name).expect("local lineage");
    assert!(Arc::ptr_eq(&lineage.current, &local_initial));
    assert_eq!(lineage.allocator.next, allocator_next);
}

#[test]
fn diagnostic_limit_is_inclusive_and_one_over_rolls_back() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let initial_source = "flow story {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
        )
        .expect("initial parse");
    let at_limit = core::iter::repeat_n("unknown_top_level\n", 1_024).collect::<String>();
    let recovered = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, initial_source.len()),
                at_limit.clone(),
            )],
        )
        .expect("the 1,024th diagnostic commits");
    assert_eq!(recovered.status(), super::ParseStatus::Recovered);
    assert_eq!(recovered.diagnostics().len(), 1_024);
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .allocator
        .next;
    let over_limit = format!("{at_limit}unknown_top_level\n");

    let failed = database.reparse(
        &recovered,
        &[source_edit(
            &recovered,
            SourceRange::new(0, at_limit.len()),
            over_limit,
        )],
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(super::SyntaxLimit::Diagnostics))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&current.current, &recovered));
    assert_eq!(current.current.snapshot().generation().get(), 2);
    assert_eq!(current.allocator.next, allocator_next);
}

#[test]
fn top_level_item_budget_accepts_the_maximum_and_rolls_back_one_over() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::with_test_limits(super::SyntaxTransactionLimits {
        top_level_items: 1,
        ..super::SyntaxTransactionLimits::default()
    });
    let initial_source = "flow first {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
        )
        .expect("the configured maximum succeeds");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .allocator
        .next;
    let one_over = "flow first {}\nflow second {}\n";

    let failed = database.reparse(
        &initial,
        &[source_edit(
            &initial,
            SourceRange::new(0, initial_source.len()),
            one_over,
        )],
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(
            super::SyntaxLimit::TopLevelItems
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&current.current, &initial));
    assert_eq!(current.allocator.next, allocator_next);
    assert_eq!(current.current.snapshot().generation().get(), 1);
}

#[test]
fn prefix_depth_limit_is_fatal_and_rolls_back_the_transaction() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let initial_source = format!(
        "flow story {{\n    let value = {}input\n}}\n",
        "& ".repeat(64)
    );
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source.clone()),
        )
        .expect("the inclusive prefix maximum succeeds");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .allocator
        .next;
    let one_over = format!(
        "flow story {{\n    let value = {}input\n}}\n",
        "& ".repeat(65)
    );

    let failed = database.reparse(
        &initial,
        &[source_edit(
            &initial,
            SourceRange::new(0, initial_source.len()),
            one_over,
        )],
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(super::SyntaxLimit::PrefixDepth))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&current.current, &initial));
    assert_eq!(current.allocator.next, allocator_next);
    assert_eq!(current.current.snapshot().generation().get(), 1);
}

#[test]
fn prefix_depth_diagnostics_are_typed_and_counted_across_recovery_modes() {
    let maximum = format!(
        "flow story {{\n    let value = {}input\n}}\n",
        "& ".repeat(64)
    );
    let accepted = parse_source(maximum);
    assert!(accepted.errors().is_empty());
    assert_eq!(accepted.syntax_stats().prefix_depth_limit_failures, 0);

    for source in [
        format!(
            "flow story {{\n    let value = {}input\n}}\n",
            "& ".repeat(65)
        ),
        format!(
            "flow story {{\n    let value = consume({}input, fallback)\n}}\n",
            "& ".repeat(65)
        ),
    ] {
        let parsed = parse_source(source);
        assert_eq!(
            parsed
                .errors()
                .iter()
                .filter(|error| { error.kind() == ParseErrorKind::ExpressionPrefixDepthLimit })
                .count(),
            1
        );
        assert_eq!(parsed.syntax_stats().prefix_depth_limit_failures, 1);
    }
}

#[test]
fn assertion_condition_limit_accepts_exactly_64_and_rolls_back_one_over() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let conditions = core::iter::repeat_n("true", 64)
        .collect::<Vec<_>>()
        .join(", ");
    let initial_source = format!("flow assertions {{\n    assert.check({conditions})\n}}\n");
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source.clone()),
        )
        .expect("the inclusive assertion-condition maximum succeeds");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .allocator
        .next;
    let one_over = format!("{conditions}, true");
    let one_over_source = format!("flow assertions {{\n    assert.check({one_over})\n}}\n");

    let failed = database.reparse(
        &initial,
        &[source_edit(
            &initial,
            SourceRange::new(0, initial_source.len()),
            one_over_source,
        )],
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(
            super::SyntaxLimit::AssertionConditions
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&current.current, &initial));
    assert_eq!(current.allocator.next, allocator_next);
    assert_eq!(current.current.snapshot().generation().get(), 1);
}

#[test]
fn source_generation_exhaustion_rolls_back_the_transaction() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::with_test_limits(super::SyntaxTransactionLimits {
        source_generation: 1,
        ..super::SyntaxTransactionLimits::default()
    });
    let source = "flow story {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial generation commits");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .allocator
        .next;

    let failed = database.reparse(
        &initial,
        &[source_edit(&initial, SourceRange::new(5, 10), "changed")],
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::IdentityExhausted(
            SyntaxIdentityKind::SourceGeneration
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&current.current, &initial));
    assert_eq!(current.current.snapshot().generation().get(), 1);
    assert_eq!(current.allocator.next, allocator_next);
}

#[test]
fn invalid_edits_and_exhausted_allocation_commit_nothing() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "flow story {}\n"),
        )
        .expect("initial parse");
    let invalid = database.reparse(
        &initial,
        &[
            source_edit(&initial, SourceRange::new(5, 8), "one"),
            source_edit(&initial, SourceRange::new(7, 10), "two"),
        ],
    );
    assert!(matches!(invalid, Err(ParseFailure::InvalidEdits(_))));

    database
        .lineages
        .get_mut(&name)
        .expect("lineage")
        .allocator
        .next = NonZeroU64::new(u64::MAX);
    let with_last_id = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(initial.source().len(), initial.source().len()),
                "flow final {}\n",
            )],
        )
        .expect("the final non-zero ID is usable");
    let failed = database.reparse(
        &with_last_id,
        &[source_edit(
            &with_last_id,
            SourceRange::new(with_last_id.source().len(), with_last_id.source().len()),
            "flow overflow {}\n",
        )],
    );
    assert!(matches!(
        failed,
        Err(ParseFailure::IdentityExhausted(SyntaxIdentityKind::Node))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(Arc::ptr_eq(&current.current, &with_last_id));
    assert_eq!(current.current.snapshot().generation().get(), 2);
}

#[test]
fn same_line_descendants_receive_distinct_private_grammar_ids() {
    let name = SourceName::path("identity.arcw");
    let source = "proof distinct((a, b): (Int, Int), c: Int) = a + b + c\n";
    let mut database = SyntaxDatabase::default();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("same-line predicate attaches");

    let nodes = parsed.attached().nodes().collect::<Vec<_>>();
    let ids = nodes
        .iter()
        .map(SyntaxNodeHandle::id)
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), nodes.len());
    assert!(
        nodes
            .iter()
            .filter(|node| node.kind() == GrammarKind::Parameter)
            .count()
            >= 2
    );
    assert!(
        nodes
            .iter()
            .filter(|node| node.kind() == GrammarKind::BindingPattern)
            .count()
            >= 3
    );
    assert!(
        nodes
            .iter()
            .filter(|node| node.kind() == GrammarKind::PrimitiveType)
            .count()
            >= 3
    );
    assert!(
        nodes
            .iter()
            .filter(|node| node.kind() == GrammarKind::PathExpression)
            .count()
            >= 2
    );
    assert!(nodes.iter().all(|node| node.range().end() <= source.len()));
}

#[test]
fn independent_databases_cannot_resolve_equal_private_raw_slots() {
    let name = SourceName::path("same.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let mut first_database = SyntaxDatabase::default();
    let mut second_database = SyntaxDatabase::default();
    let first = first_database
        .parse_initial(
            snapshot.clone(),
            source_document(&name, "proof valid() = ()\n"),
        )
        .expect("first database");
    let second = second_database
        .parse_initial(snapshot, source_document(&name, "proof valid() = ()\n"))
        .expect("second database");
    let first_root = first.attached().root_handle();
    let second_root = second.attached().root_handle();

    assert_eq!(first_root.id().slot(), second_root.id().slot());
    assert_ne!(first_root.id(), second_root.id());
    assert!(matches!(
        first.attached().syntax_node(second_root.id()),
        Err(SyntaxLookupError::WrongDatabase { .. })
    ));
}

#[test]
fn trivia_reparse_preserves_private_descendant_ids_and_old_snapshot_ranges() {
    let name = SourceName::path("predicate.arcw");
    let source = "predicate ready(value: Int) requires value > 0 = value == 1\n";
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial predicate");
    let old_nodes = initial.attached().nodes().collect::<Vec<_>>();
    let old_item = old_nodes
        .iter()
        .find(|node| node.kind() == GrammarKind::PredicateItem)
        .expect("predicate item")
        .clone();
    let old_typed = initial
        .attached()
        .typed_node::<PredicateItemKind>(old_item.id())
        .unwrap();
    let old_range = old_typed.range();

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(9, 9), "  ")],
        )
        .expect("trivia reparse");
    let new_nodes = reparsed.attached().nodes().collect::<Vec<_>>();
    assert_eq!(
        old_nodes
            .iter()
            .map(SyntaxNodeHandle::id)
            .collect::<Vec<_>>(),
        new_nodes
            .iter()
            .map(SyntaxNodeHandle::id)
            .collect::<Vec<_>>()
    );
    let new_item = new_nodes
        .iter()
        .find(|node| node.kind() == GrammarKind::PredicateItem)
        .expect("reparsed predicate item")
        .clone();
    let new_typed = reparsed
        .attached()
        .typed_node::<PredicateItemKind>(new_item.id())
        .unwrap();
    assert!(old_typed.is_same_reconciled_node(&new_typed));
    assert_eq!(old_typed.range(), old_range);
    assert_eq!(new_typed.range().start(), old_range.start());
    assert_eq!(new_typed.range().end(), old_range.end() + 2);
    assert!(matches!(
        reparsed.attached().resolve_exact(&old_item),
        Err(SyntaxLookupError::WrongSnapshot { .. })
    ));
}

#[test]
fn unique_private_grammar_siblings_retain_ids_when_reordered() {
    let name = SourceName::path("reordered-proofs.arcw");
    let source = "proof first() = 1\nproof second() = 2\n";
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial proofs");
    let first = private_id_containing(&initial, GrammarKind::ProofItem, "proof first");
    let second = private_id_containing(&initial, GrammarKind::ProofItem, "proof second");
    let reordered_source = "proof second() = 2\nproof first() = 1\n";

    let reordered = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                reordered_source,
            )],
        )
        .expect("reordered proofs");

    assert_eq!(
        private_id_containing(&reordered, GrammarKind::ProofItem, "proof first"),
        first
    );
    assert_eq!(
        private_id_containing(&reordered, GrammarKind::ProofItem, "proof second"),
        second
    );
}

#[test]
fn a_private_grammar_copy_is_fresh_while_the_original_retains_its_id() {
    let name = SourceName::path("copied-proof.arcw");
    let source = "proof same() = ()\n";
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial proof");
    let original = private_ids_containing(&initial, GrammarKind::ProofItem, "proof same");
    assert_eq!(original.len(), 1);
    let copied_source = "proof same() = ()\nproof same() = ()\n";

    let copied = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                copied_source,
            )],
        )
        .expect("copied proof");
    let copied_ids = private_ids_containing(&copied, GrammarKind::ProofItem, "proof same");

    assert_eq!(copied_ids.len(), 2);
    assert_eq!(copied_ids[0], original[0]);
    assert_ne!(copied_ids[1], original[0]);
}

#[test]
fn moving_a_private_grammar_node_across_block_parents_allocates_a_fresh_id() {
    let name = SourceName::path("moved-expression.arcw");
    let source =
        "proof relocate() -> Int { let first: Int = { target() }; let second: Int = { 0 }; 0 }\n";
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial nested expression");
    let target = private_id_containing(&initial, GrammarKind::CallExpression, "target()");
    let moved_source =
        "proof relocate() -> Int { let first: Int = { 0 }; let second: Int = { target() }; 0 }\n";

    let moved = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                moved_source,
            )],
        )
        .expect("moved nested expression");

    assert_ne!(
        private_id_containing(&moved, GrammarKind::CallExpression, "target()"),
        target
    );
}

#[test]
fn changed_private_grammar_node_is_fresh_while_its_sibling_survives() {
    let name = SourceName::path("changed-proof.arcw");
    let source = "proof first() = ()\nproof second() = ()\n";
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial proofs");
    let first_name = private_id_containing(&initial, GrammarKind::NameDefinition, "first");
    let second = private_id_containing(&initial, GrammarKind::ProofItem, "proof second");
    let first_start = source.find("first").expect("first proof name");

    let changed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(first_start, first_start + "first".len()),
                "changed",
            )],
        )
        .expect("renamed proof");

    assert_ne!(
        private_id_containing(&changed, GrammarKind::NameDefinition, "changed"),
        first_name
    );
    assert_eq!(
        private_id_containing(&changed, GrammarKind::ProofItem, "proof second"),
        second
    );
}

#[test]
fn missing_and_error_nodes_reconcile_by_recovery_role() {
    let name = SourceName::path("recovery.arcw");
    let source = "proof () = ()\nunknown surface\n";
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("recovered source attaches");
    let old_recovery = recovery_ids(&initial);
    assert!(old_recovery.len() >= 2);
    assert_eq!(
        old_recovery.iter().copied().collect::<HashSet<_>>().len(),
        old_recovery.len()
    );

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(0, 0), " ")],
        )
        .expect("recovered trivia reparse");
    assert_eq!(recovery_ids(&reparsed), old_recovery);
}

#[test]
fn fatal_private_attachment_failure_rolls_back_initial_transaction() {
    let name = SourceName::path("attachment-failure.arcw");
    let mut database = SyntaxDatabase::default();
    let lineage_before = database.shadow.next_lineage_for_test();
    let failed = database.parse_initial_with_attachment_failure(
        &SourceSnapshotId::initial(name.clone()),
        source_document(&name, "proof invalid() = ()\n"),
    );

    assert!(matches!(failed, Err(ParseFailure::InternalInvariant)));
    assert!(database.lineages.is_empty());
    assert_eq!(database.shadow.next_lineage_for_test(), lineage_before);

    let accepted = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "proof valid() = ()\n"),
        )
        .expect("next valid transaction uses the unconsumed lineage");
    let control_name = SourceName::path("control.arcw");
    let mut control = SyntaxDatabase::default();
    let control = control
        .parse_initial(
            SourceSnapshotId::initial(control_name.clone()),
            source_document(&control_name, "proof valid() = ()\n"),
        )
        .expect("control transaction");
    assert_eq!(
        accepted.attached().root_handle().id().slot(),
        control.attached().root_handle().id().slot()
    );
}

#[test]
fn fatal_private_attachment_failure_rolls_back_reparse_transaction() {
    let name = SourceName::path("reparse-attachment-failure.arcw");
    let source = "proof first() = ()\n";
    let addition = "proof second() = ()\n";
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("initial source");
    let edit = source_edit(
        &initial,
        SourceRange::new(source.len(), source.len()),
        addition,
    );
    let next_before = database
        .lineages
        .get(&name)
        .expect("lineage")
        .shadow
        .next_node_for_test();

    let failed = database.reparse_with_attachment_failure(&initial, std::slice::from_ref(&edit));
    assert!(matches!(failed, Err(ParseFailure::InternalInvariant)));
    let current = database.lineages.get(&name).expect("lineage");
    assert!(Arc::ptr_eq(&current.current, &initial));
    assert!(Arc::ptr_eq(current.shadow.current(), initial.attached()));
    assert_eq!(current.shadow.next_node_for_test(), next_before);

    let accepted = database
        .reparse(&initial, &[edit])
        .expect("valid retry after failed attachment");
    let mut control_database = SyntaxDatabase::default();
    let control_initial = control_database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
        )
        .expect("control initial");
    let control_edit = source_edit(
        &control_initial,
        SourceRange::new(source.len(), source.len()),
        addition,
    );
    let control = control_database
        .reparse(&control_initial, &[control_edit])
        .expect("control reparse");
    assert_eq!(private_slots(&accepted), private_slots(&control));
}

fn recovery_ids(source: &super::ParsedSource) -> Vec<crate::attachment::SyntaxNodeId> {
    source
        .attached()
        .nodes()
        .filter(|node| node.kind().is_missing_node() || node.kind().is_error_node())
        .map(|node| node.id())
        .collect()
}

fn private_slots(source: &super::ParsedSource) -> Vec<NonZeroU64> {
    source
        .attached()
        .nodes()
        .map(|node| node.id().slot())
        .collect()
}

fn private_id_containing(
    source: &super::ParsedSource,
    kind: GrammarKind,
    needle: &str,
) -> crate::attachment::SyntaxNodeId {
    private_ids_containing(source, kind, needle)
        .into_iter()
        .min_by_key(|id| {
            let range = source
                .attached()
                .syntax_node(*id)
                .expect("attached private grammar identity")
                .range();
            range.end() - range.start()
        })
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} containing {needle:?}; containing nodes: {:?}",
                source
                    .attached()
                    .nodes()
                    .filter(|node| node.rowan().text().to_string().contains(needle))
                    .map(|node| (node.kind(), node.rowan().text().to_string()))
                    .collect::<Vec<_>>()
            )
        })
}

fn private_ids_containing(
    source: &super::ParsedSource,
    kind: GrammarKind,
    needle: &str,
) -> Vec<crate::attachment::SyntaxNodeId> {
    source
        .attached()
        .nodes()
        .filter(|node| node.kind() == kind && node.rowan().text().to_string().contains(needle))
        .map(|node| node.id())
        .collect()
}

fn line_id(source: &super::ParsedSource, needle: &str) -> SyntaxNodeId {
    let line = source
        .root()
        .children()
        .find(|node| node.text().to_string().contains(needle))
        .expect("matching line");
    source.identities().id_for(&line).expect("line identity")
}

fn smallest_node_id_containing(source: &super::ParsedSource, needle: &str) -> SyntaxNodeId {
    let node = source
        .root()
        .descendants()
        .filter(|node| node.text().to_string().contains(needle))
        .min_by_key(|node| node.text().len())
        .expect("matching syntax node");
    source.identities().id_for(&node).expect("node identity")
}

fn line_ids_containing(source: &super::ParsedSource, needle: &str) -> Vec<SyntaxNodeId> {
    source
        .root()
        .children()
        .filter(|node| node.text().to_string().contains(needle))
        .map(|node| source.identities().id_for(&node).expect("line identity"))
        .collect()
}
