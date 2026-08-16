use super::{ParseFailure, ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};
use core::num::NonZeroU64;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::Arc;

use crate::assertion::AssertionMode;
use crate::attachment::node::{
    AssertionStatementKind, BinaryExpressionKind, BindingPatternKind, BlockExpressionKind,
    CallArgumentKind, CallExpressionKind, CloseAngleKind, CloseBraceKind, CloseParenKind,
    ColonKind, EnsuresClauseKind, ExpressionBodyKind, FixedParameterGroupKind,
    GenericParameterGroupKind, GenericParameterKind, LetStatementKind, LiteralExpressionKind,
    NameDefinitionKind, OmittedBlockTailKind, OpenAngleKind, OpenBraceKind, OpenParenKind,
    ParameterKind, PathExpressionKind, PathKind, PathTypeKind, PredicateBlockKind,
    PredicateBodyKind, ProofBlockKind, ProofBodyKind, ProofCallStatementKind, ProofItemKind,
    RequiresClauseKind, ReturnTypeKind, ThinArrowKind, TypeParameterKind, TypedBindingPatternKind,
    WhereClauseKind, WherePredicateKind,
};
use crate::attachment::{
    AttachedAssertionMode, AttachedExternCapabilityBody, AttachedPatternComponent,
    AttachedTypeFamily, AttachmentFailure, BlockTailNode, PatternComponentRole, PatternLiteralPart,
    PredicateItemKind, SourceFileKind, SyntaxAccessError, SyntaxLineageId, SyntaxLookupError,
    SyntaxNodeHandle, SyntaxNodeId, TypedItemNode, VariantPatternHeadPart,
};
use crate::expressions::{ExpressionComponentRole, ExpressionLiteralPart, ExpressionProjection};
use crate::grammar::kinds::SyntaxKind as GrammarKind;
use crate::id_ref::SyntaxIdRefPart;
use crate::parser::fragment::{ParseCompletion, ParseOptions};
use crate::parser::unbound_fragment::{
    AttachedFragment, FragmentKind, UnboundFragment, parse_expression_fragment,
    parse_pattern_fragment, parse_statement_fragment, parse_type_fragment,
};
use crate::patterns::{PatternSyntaxFamily, PatternTypeChildRelation};
use crate::types::TypeRefComponentRole;

#[path = "database_tests/atomicity.rs"]
mod atomicity;
#[path = "database_tests/choice.rs"]
mod choice;

fn source_document(name: &SourceName, text: impl Into<Arc<str>>) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(name.display_name()).expect("valid test document id"),
            name.clone(),
            text,
        )
        .expect("test source document"),
    )
}

fn syntax_database() -> SyntaxDatabase {
    SyntaxDatabase::try_new().expect("test syntax database identity")
}

fn source_span(document: &SourceDocument, range: SourceRange) -> arcweft_source::SourceSpan {
    document
        .span(range)
        .expect("valid span in the exact test source revision")
}

fn attach_exact_fragment<K: FragmentKind>(
    database: &mut SyntaxDatabase,
    snapshot: &SourceSnapshotId,
    document: &Arc<SourceDocument>,
    fragment_text: &str,
    fragment: UnboundFragment<K>,
    expected_kind: GrammarKind,
) -> AttachedFragment<K> {
    let start = document
        .text()
        .find(fragment_text)
        .expect("fragment text in target document");
    let span = source_span(
        document,
        SourceRange::new(start, start + fragment_text.len()),
    );
    let attached = database
        .attach_fragment(snapshot.clone(), Arc::clone(document), span, fragment)
        .expect("complete exact-byte fragment attaches");
    assert_eq!(attached.root().kind(), expected_kind);
    assert_eq!(
        attached.root().range(),
        SourceRange::new(start, start + fragment_text.len())
    );
    assert_eq!(
        attached
            .root()
            .syntax()
            .parent()
            .expect("fragment source-file root")
            .rowan()
            .to_string(),
        document.text()
    );
    assert_eq!(attached.root().source_span().source(), document.identity());
    attached
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
fn syntax_no_op_returns_exact_arc_and_consumes_nothing() {
    let name = SourceName::path("story.arcw");
    let mut database = syntax_database();
    let document = source_document(&name, "flow story {}\n");
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            Arc::clone(&document),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial parse");
    let initial_stats = initial.data().stats();
    let initial_diagnostics = initial.diagnostics().to_vec();
    let initial_ids = identity_ids(&initial);
    let next_lineage = database.transaction.next_lineage_for_test();
    let next_node = database
        .lineages
        .get(&name)
        .expect("committed lineage")
        .transaction
        .next_node_for_test();
    assert!(Arc::ptr_eq(initial.data().document(), &document));
    let unchanged = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(5, 10), "story")],
            crate::parser::ParseOptions::default(),
        )
        .expect("no-op reparse");
    assert!(initial.is_same_snapshot(&unchanged));
    assert!(Arc::ptr_eq(initial.data(), unchanged.data()));
    assert!(Arc::ptr_eq(initial.attached(), unchanged.attached()));
    assert!(Arc::ptr_eq(
        initial.document_lease(),
        unchanged.document_lease()
    ));
    assert_eq!(unchanged.snapshot_id(), initial.snapshot_id());
    assert_eq!(unchanged.diagnostics(), initial_diagnostics);
    assert_eq!(identity_ids(&unchanged), initial_ids);
    assert_eq!(unchanged.data().stats(), initial_stats);
    assert_eq!(unchanged.source_snapshot_id().generation().get(), 1);
    assert_eq!(database.transaction.next_lineage_for_test(), next_lineage);
    assert_eq!(
        database
            .lineages
            .get(&name)
            .expect("unchanged lineage")
            .transaction
            .next_node_for_test(),
        next_node
    );

    let empty = database
        .reparse(&initial, &[], crate::parser::ParseOptions::default())
        .expect("an empty edit transaction is a no-op");
    assert!(initial.is_same_snapshot(&empty));
    assert!(Arc::ptr_eq(initial.data(), empty.data()));
    assert!(Arc::ptr_eq(initial.attached(), empty.attached()));
    assert!(Arc::ptr_eq(
        initial.document_lease(),
        empty.document_lease()
    ));
    assert_eq!(empty.snapshot_id(), initial.snapshot_id());
    assert_eq!(empty.diagnostics(), initial_diagnostics);
    assert_eq!(identity_ids(&empty), initial_ids);
    assert_eq!(empty.data().stats(), initial_stats);
    assert_eq!(database.transaction.next_lineage_for_test(), next_lineage);
    assert_eq!(
        database
            .lineages
            .get(&name)
            .expect("unchanged lineage")
            .transaction
            .next_node_for_test(),
        next_node
    );
}

#[test]
fn current_returns_only_the_registered_lineage_snapshot() {
    let name = SourceName::path("current.arcw");
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "flow current {}\n"),
            ParseOptions::default(),
        )
        .expect("initial parse");

    let current = database
        .current(initial.snapshot_id().lineage())
        .expect("registered lineage has a current snapshot");
    assert!(current.is_same_snapshot(&initial));
    let named = database
        .current_for_source(&name)
        .expect("registered source name has a current snapshot");
    assert!(named.is_same_snapshot(&initial));
    assert!(
        database
            .current_for_source(&SourceName::path("absent.arcw"))
            .is_none()
    );

    let foreign = syntax_database();
    assert!(matches!(
        database.current(SyntaxLineageId::from_raw_for_test(
            foreign.database_id(),
            NonZeroU64::MIN,
        )),
        Err(SyntaxLookupError::WrongDatabase { expected, actual })
            if expected == database.database_id() && actual == foreign.database_id()
    ));

    let unknown = SyntaxLineageId::from_raw_for_test(
        database.database_id(),
        NonZeroU64::new(u64::MAX).expect("non-zero unknown lineage"),
    );
    assert!(matches!(
        database.current(unknown),
        Err(SyntaxLookupError::UnknownLineage { lineage }) if lineage == unknown
    ));
}

#[test]
fn stale_generation_current_resolution_is_typed_error() {
    let name = SourceName::path("resolve-current.arcw");
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "proof first() = ()\n"),
            ParseOptions::default(),
        )
        .expect("initial parse");
    let old_name = only_node_with_kind(&initial, GrammarKind::NameDefinition)
        .cast::<NameDefinitionKind>()
        .expect("initial typed proof name");
    let current = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new("proof ".len(), "proof first".len()),
                "second",
            )],
            ParseOptions::default(),
        )
        .expect("current generation");

    assert_eq!(
        current.syntax_node(old_name.id()),
        Err(SyntaxLookupError::MissingNode { id: old_name.id() })
    );
    assert_eq!(
        database.resolve_current(&old_name),
        Err(SyntaxLookupError::StaleGeneration {
            current: current.source_snapshot_id().generation(),
            supplied: initial.source_snapshot_id().generation(),
        })
    );
    let current_name = only_node_with_kind(&current, GrammarKind::NameDefinition)
        .cast::<NameDefinitionKind>()
        .expect("current typed proof name");
    assert_ne!(current_name.id(), old_name.id());
    assert_eq!(
        database
            .resolve_current(&current_name)
            .expect("current typed node resolves"),
        current_name
    );
}

#[test]
fn exact_snapshot_operation_rejects_wrong_snapshot() {
    let name = SourceName::path("exact-snapshot.arcw");
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "proof stable() = ()\n"),
            ParseOptions::default(),
        )
        .expect("initial snapshot");
    let old_raw = initial.root_syntax();
    let old_typed = old_raw
        .cast::<SourceFileKind>()
        .expect("typed initial source-file root");
    let current = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(0, 0), "  ")],
            ParseOptions::default(),
        )
        .expect("new source generation");
    let expected = current.snapshot_id().clone();
    let actual = initial.snapshot_id().clone();

    assert_eq!(
        current.resolve_exact(&old_typed),
        Err(SyntaxLookupError::WrongSnapshot {
            expected: expected.clone(),
            actual: actual.clone(),
        })
    );
    assert_eq!(
        current.resolve_exact_syntax(&old_raw),
        Err(SyntaxLookupError::WrongSnapshot { expected, actual })
    );

    let current_raw = current
        .syntax_node(old_raw.id())
        .expect("stable identity resolves through the receiver snapshot");
    assert_eq!(current_raw.id(), old_raw.id());
    assert_eq!(current_raw.snapshot_id(), current.snapshot_id());
    assert_eq!(
        current_raw.range(),
        SourceRange::new(0, current.source().len())
    );
}

#[test]
fn wrong_typed_kind_returns_kind_mismatch() {
    let name = SourceName::path("wrong-typed-kind.arcw");
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "predicate ready() = true\n"),
            ParseOptions::default(),
        )
        .expect("predicate snapshot");
    let predicate = parsed
        .items()
        .expect("typed item inventory")
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Predicate(node) => Some(node),
            _ => None,
        })
        .expect("typed Predicate item");

    assert_eq!(
        parsed.typed_node::<ProofItemKind>(predicate.id()),
        Err(SyntaxLookupError::KindMismatch {
            id: predicate.id(),
            expected: GrammarKind::ProofItem,
            actual: GrammarKind::PredicateItem,
        })
    );
}

#[test]
fn parsed_source_public_lease_owns_one_typed_source_file_and_all_lowering_families() {
    let name = SourceName::path("public-attached-source.arcw");
    let source = concat!(
        "flow story {\n",
        "    let value: Int = input\n",
        "    assert.check(value)\n",
        "}\n",
    );
    let document = source_document(&name, source);
    let source_snapshot = SourceSnapshotId::initial(name);
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            source_snapshot.clone(),
            Arc::clone(&document),
            crate::parser::ParseOptions::default(),
        )
        .expect("attached source parses");

    assert_eq!(parsed.source_snapshot_id(), &source_snapshot);
    assert_eq!(parsed.snapshot_id().source(), &source_snapshot);
    assert_eq!(
        parsed.snapshot_id().lineage().database(),
        database.database_id()
    );
    assert!(Arc::ptr_eq(parsed.document_lease(), &document));

    let root_syntax = parsed.root_syntax();
    let root = parsed
        .typed_node::<SourceFileKind>(root_syntax.id())
        .expect("typed source-file root");
    assert_eq!(root.id(), root_syntax.id());
    assert_eq!(root.source_text(), source);
    assert_eq!(root.source_span().source(), document.identity());
    assert_eq!(
        parsed.typed_node::<SourceFileKind>(root.id()).unwrap(),
        root.clone()
    );
    assert_eq!(parsed.bind_rowan(root_syntax.rowan()).unwrap(), root_syntax);
    assert_eq!(parsed.items().unwrap().len(), 1);

    for (kind, assert_typed) in [
        (GrammarKind::TypedBindingPattern, 0_u8),
        (GrammarKind::PathType, 1),
        (GrammarKind::PathExpression, 2),
        (GrammarKind::AssertionStatement, 3),
    ] {
        let id = root_syntax
            .rowan()
            .descendants()
            .filter_map(|node| parsed.bind_rowan(&node).ok())
            .find(|node| node.kind() == kind)
            .unwrap_or_else(|| panic!("missing {kind:?}"))
            .id();
        match assert_typed {
            0 => assert_eq!(
                parsed
                    .typed_node::<TypedBindingPatternKind>(id)
                    .unwrap()
                    .id(),
                id
            ),
            1 => assert_eq!(parsed.typed_node::<PathTypeKind>(id).unwrap().id(), id),
            2 => assert_eq!(
                parsed.typed_node::<PathExpressionKind>(id).unwrap().id(),
                id
            ),
            3 => assert_eq!(
                parsed
                    .typed_node::<AssertionStatementKind>(id)
                    .unwrap()
                    .id(),
                id
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn assertion_projection_rebinds_mode_on_same_lineage_and_retains_typed_recovery() {
    fn assertion(source: &ParsedSource) -> crate::attachment::AstNode<AssertionStatementKind> {
        let id = source
            .root_syntax()
            .rowan()
            .descendants()
            .filter_map(|node| source.bind_rowan(&node).ok())
            .find(|node| node.kind() == GrammarKind::AssertionStatement)
            .expect("attached assertion statement")
            .id();
        source
            .typed_node::<AssertionStatementKind>(id)
            .expect("typed assertion statement")
    }

    let name = SourceName::path("assertion-projection.arcw");
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, "flow story {\n    assert.check(true)\n}\n"),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial assertion parses");
    assert!(matches!(
        assertion(&initial).semantics().unwrap().mode(),
        AttachedAssertionMode::Resolved {
            value: AssertionMode::Check,
            ..
        }
    ));

    let check_start = initial.document().text().find("check").unwrap();
    let prove = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(check_start, check_start + "check".len()),
                "prove",
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("same-lineage mode replacement parses");
    assert_eq!(
        initial.snapshot_id().lineage(),
        prove.snapshot_id().lineage()
    );
    assert!(matches!(
        assertion(&prove).semantics().unwrap().mode(),
        AttachedAssertionMode::Resolved {
            value: AssertionMode::Prove,
            ..
        }
    ));
    assert!(matches!(
        assertion(&initial).semantics().unwrap().mode(),
        AttachedAssertionMode::Resolved {
            value: AssertionMode::Check,
            ..
        }
    ));

    let prove_start = prove.document().text().find("prove").unwrap();
    let recovered = database
        .reparse(
            &prove,
            &[source_edit(
                &prove,
                SourceRange::new(prove_start, prove_start + "prove".len()),
                "unknown",
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("unknown assertion mode remains typed recovery");
    assert_eq!(
        prove.snapshot_id().lineage(),
        recovered.snapshot_id().lineage()
    );
    let assertion = assertion(&recovered).semantics().unwrap();
    assert!(matches!(
        assertion.mode(),
        AttachedAssertionMode::Recovered { .. }
    ));
    assert!(assertion.has_recovery());
}

#[test]
fn invalid_edit_order_overlap_and_foreign_provenance_leave_lineage_unchanged() {
    let name = SourceName::path("story.arcw");
    let mut database = syntax_database();
    let source = "flow café {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial parse");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();

    let foreign_name = SourceName::path("other.arcw");
    let foreign = source_document(&foreign_name, source);
    let failures = [
        database.reparse(
            &initial,
            &[
                source_edit(&initial, SourceRange::new(5, 5), "x"),
                source_edit(&initial, SourceRange::new(0, 0), "y"),
            ],
            ParseOptions::default(),
        ),
        database.reparse(
            &initial,
            &[
                source_edit(&initial, SourceRange::new(0, 4), "x"),
                source_edit(&initial, SourceRange::new(3, 5), "y"),
            ],
            ParseOptions::default(),
        ),
        database.reparse(
            &initial,
            &[SourceEdit::new(
                foreign
                    .span(SourceRange::new(0, 0))
                    .expect("valid foreign span"),
                "x",
            )],
            crate::parser::ParseOptions::default(),
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
    assert!(current.current.is_same_snapshot(&initial));
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn reparsing_a_stale_snapshot_is_rejected_without_mutation() {
    let name = SourceName::path("story.arcw");
    let mut database = syntax_database();
    let initial_source = "flow story {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial parse");
    let current = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(5, 10), "current")],
            crate::parser::ParseOptions::default(),
        )
        .expect("current parse");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();

    let stale = database.reparse(
        &initial,
        &[source_edit(&initial, SourceRange::new(5, 10), "stale")],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        stale,
        Err(ParseFailure::StaleSnapshot {
            current: current_id,
            supplied: supplied_id,
        }) if &current_id == current.snapshot_id() && &supplied_id == initial.snapshot_id()
    ));
    let lineage = database.lineages.get(&name).expect("lineage current");
    assert!(lineage.current.is_same_snapshot(&current));
    assert_eq!(lineage.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn reparsing_a_snapshot_from_another_database_is_rejected_without_mutation() {
    let name = SourceName::path("story.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let source: Arc<str> = Arc::from("flow story {}\n");
    let mut local = syntax_database();
    let local_initial = local
        .parse_initial(
            snapshot.clone(),
            source_document(&name, Arc::clone(&source)),
            crate::parser::ParseOptions::default(),
        )
        .expect("local initial parse");
    let mut foreign = syntax_database();
    let foreign_initial = foreign
        .parse_initial(
            snapshot,
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("foreign initial parse");
    let allocator_next = local
        .lineages
        .get(&name)
        .expect("local lineage")
        .transaction
        .next_node_for_test();

    let rejected = local.reparse(
        &foreign_initial,
        &[source_edit(
            &foreign_initial,
            SourceRange::new(5, 10),
            "foreign",
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(rejected, Err(ParseFailure::SourceMismatch)));
    let lineage = local.lineages.get(&name).expect("local lineage");
    assert!(lineage.current.is_same_snapshot(&local_initial));
    assert_eq!(lineage.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn diagnostic_limit_is_inclusive_and_one_over_rolls_back() {
    let name = SourceName::path("story.arcw");
    let mut database = syntax_database();
    let initial_source = "flow story {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            crate::parser::ParseOptions::default(),
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
            crate::parser::ParseOptions::default(),
        )
        .expect("the 1,024th diagnostic commits");
    assert_eq!(recovered.status(), super::ParseStatus::Recovered);
    assert_eq!(recovered.data().diagnostics().len(), 1_024);
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
    let over_limit = format!("{at_limit}unknown_top_level\n");

    let failed = database.reparse(
        &recovered,
        &[source_edit(
            &recovered,
            SourceRange::new(0, at_limit.len()),
            over_limit,
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(super::SyntaxLimit::Diagnostics))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&recovered));
    assert_eq!(current.current.source_snapshot_id().generation().get(), 2);
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn flow_heterogeneous_contract_limit_accepts_exact_and_rejects_one_over_atomically() {
    fn flow_with_contracts(count: usize) -> String {
        let mut source = String::from("flow contract_limit(state: State)\n");
        for ordinal in 0..count {
            match ordinal % 9 {
                0 => writeln!(source, "requires state.ready_{ordinal}").unwrap(),
                1 => writeln!(source, "effects {{ effect_{ordinal} }}").unwrap(),
                2 => writeln!(source, "ensures state.valid_{ordinal}").unwrap(),
                3 => writeln!(source, "reads {{ state.value_{ordinal} }}").unwrap(),
                4 => writeln!(source, "invariant state.invariant_{ordinal}").unwrap(),
                5 => writeln!(source, "ensures no_effect network.request_{ordinal}").unwrap(),
                6 => writeln!(source, "modifies {{ state.value_{ordinal} }}").unwrap(),
                7 => writeln!(source, "assume external_ok_{ordinal}").unwrap(),
                8 => writeln!(source, "decreases state.remaining_{ordinal}").unwrap(),
                _ => unreachable!("modulo nine is exhaustive"),
            }
        }
        source.push_str("{}\n");
        source
    }

    let name = SourceName::path("flow-contract-limit.arcw");
    let mut database = syntax_database();
    let exact_source = flow_with_contracts(super::SyntaxLimit::ContractClauses.maximum());
    let exact = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, exact_source.clone()),
            crate::parser::ParseOptions::default(),
        )
        .expect("the exact heterogeneous Flow contract limit commits");
    assert_eq!(exact.status(), super::ParseStatus::Clean);
    assert_eq!(
        exact
            .attached()
            .nodes()
            .filter(|node| node.kind().is_contract_clause())
            .count(),
        super::SyntaxLimit::ContractClauses.maximum()
    );
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
    let one_over_source = flow_with_contracts(
        super::SyntaxLimit::ContractClauses
            .maximum()
            .checked_add(1)
            .unwrap(),
    );

    let failed = database.reparse(
        &exact,
        &[source_edit(
            &exact,
            SourceRange::new(0, exact_source.len()),
            one_over_source,
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(
            super::SyntaxLimit::ContractClauses
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&exact));
    assert_eq!(current.current.source_snapshot_id().generation().get(), 1);
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn grouped_use_member_limit_accepts_exactly_1024_and_rolls_back_one_over() {
    let name = SourceName::path("grouped-use-limit.arcw");
    let mut database = syntax_database();
    let members = (0..super::SyntaxLimit::DeclarationMembers.maximum())
        .map(|ordinal| format!("name_{ordinal}"))
        .collect::<Vec<_>>()
        .join(", ");
    let initial_source = format!("use crate.large.{{{members}}}\n");
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source.clone()),
            crate::parser::ParseOptions::default(),
        )
        .expect("the exact grouped-use member limit commits");
    assert_eq!(initial.status(), super::ParseStatus::Clean);
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
    let one_over_source = format!(
        "use crate.large.{{{members}, name_{}}}\n",
        super::SyntaxLimit::DeclarationMembers.maximum()
    );

    let failed = database.reparse(
        &initial,
        &[source_edit(
            &initial,
            SourceRange::new(0, initial_source.len()),
            one_over_source,
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(
            super::SyntaxLimit::DeclarationMembers
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&initial));
    assert_eq!(current.current.source_snapshot_id().generation().get(), 1);
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn extern_capability_member_limit_accepts_exactly_1024_and_rolls_back_one_over() {
    let capability_source = |member_count: usize| {
        let mut source = String::from("extern capability host {\n");
        for ordinal in 0..member_count {
            writeln!(source, "    type T{ordinal}").unwrap();
        }
        source.push_str("}\n");
        source
    };
    let name = SourceName::path("extern-capability-member-limit.arcw");
    let mut database = syntax_database();
    let exact_source = capability_source(super::SyntaxLimit::DeclarationMembers.maximum());
    let exact = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, exact_source.clone()),
            crate::parser::ParseOptions::default(),
        )
        .expect("the exact external-capability member limit commits");
    assert_eq!(exact.status(), super::ParseStatus::Clean);
    let items = exact.items().unwrap();
    let [TypedItemNode::ExternCapability(capability)] = items.as_slice() else {
        panic!("one typed ExternCapability item")
    };
    let attached = capability.semantics().unwrap();
    let AttachedExternCapabilityBody::Braced { members, .. } = attached.body() else {
        panic!("the exact-limit capability retains its braced body")
    };
    assert_eq!(
        members.len(),
        super::SyntaxLimit::DeclarationMembers.maximum()
    );
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
    let one_over_source = capability_source(
        super::SyntaxLimit::DeclarationMembers
            .maximum()
            .checked_add(1)
            .unwrap(),
    );

    let failed = database.reparse(
        &exact,
        &[source_edit(
            &exact,
            SourceRange::new(0, exact_source.len()),
            one_over_source,
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(
            super::SyntaxLimit::DeclarationMembers
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&exact));
    assert_eq!(current.current.source_snapshot_id().generation().get(), 1);
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn trait_member_limit_accepts_exactly_1024_and_rolls_back_one_over() {
    let trait_source = |member_count: usize| {
        let mut source = String::from("trait Large {\n");
        for ordinal in 0..member_count {
            writeln!(source, "    type T{ordinal}").unwrap();
        }
        source.push_str("}\n");
        source
    };
    let name = SourceName::path("trait-member-limit.arcw");
    let mut database = syntax_database();
    let exact_source = trait_source(super::SyntaxLimit::DeclarationMembers.maximum());
    let exact = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, exact_source.clone()),
            crate::parser::ParseOptions::default(),
        )
        .expect("the exact Trait member limit commits");
    assert_eq!(exact.status(), super::ParseStatus::Clean);
    let items = exact.items().unwrap();
    let [TypedItemNode::Trait(declaration)] = items.as_slice() else {
        panic!("one typed Trait item")
    };
    assert_eq!(
        declaration.semantics().unwrap().body().members().len(),
        super::SyntaxLimit::DeclarationMembers.maximum()
    );
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
    let one_over_source = trait_source(
        super::SyntaxLimit::DeclarationMembers
            .maximum()
            .checked_add(1)
            .unwrap(),
    );

    let failed = database.reparse(
        &exact,
        &[source_edit(
            &exact,
            SourceRange::new(0, exact_source.len()),
            one_over_source,
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(
            super::SyntaxLimit::DeclarationMembers
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&exact));
    assert_eq!(current.current.source_snapshot_id().generation().get(), 1);
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn prefix_depth_limit_is_fatal_and_rolls_back_the_transaction() {
    let name = SourceName::path("story.arcw");
    let mut database = syntax_database();
    let initial_source = format!(
        "flow story {{\n    let value = {}input\n}}\n",
        "& ".repeat(64)
    );
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source.clone()),
            crate::parser::ParseOptions::default(),
        )
        .expect("the inclusive prefix maximum succeeds");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
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
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(super::SyntaxLimit::PrefixDepth))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&initial));
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
    assert_eq!(current.current.source_snapshot_id().generation().get(), 1);
}

#[test]
fn ordinary_expression_nesting_does_not_consume_prefix_depth() {
    let name = SourceName::path("nested.arcw");
    let source = format!(
        "flow story {{\n    let value = {}input{}\n}}\n",
        "(".repeat(65),
        ")".repeat(65)
    );
    let mut database = syntax_database();

    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("parenthesized expression owners do not form a prefix chain");

    assert_eq!(parsed.status(), super::ParseStatus::Clean);
}

#[test]
fn prefix_depth_tracks_active_ancestors_through_parentheses() {
    let name = SourceName::path("nested-prefix.arcw");
    let exact_expression = format!(
        "{}input{}",
        "&(".repeat(super::SyntaxLimit::PrefixDepth.maximum()),
        ")".repeat(super::SyntaxLimit::PrefixDepth.maximum())
    );
    let initial_source = format!("flow story {{\n    let value = {exact_expression}\n}}\n");
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source.clone()),
            crate::parser::ParseOptions::default(),
        )
        .expect("the 64th active prefix ancestor commits");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
    let one_over_expression = format!(
        "{}input{}",
        "&(".repeat(super::SyntaxLimit::PrefixDepth.maximum() + 1),
        ")".repeat(super::SyntaxLimit::PrefixDepth.maximum() + 1)
    );
    let one_over_source = format!("flow story {{\n    let value = {one_over_expression}\n}}\n");

    let failed = database.reparse(
        &initial,
        &[source_edit(
            &initial,
            SourceRange::new(0, initial_source.len()),
            one_over_source,
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(super::SyntaxLimit::PrefixDepth))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&initial));
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
fn try_await_emits_nested_typed_prefix_nodes() {
    for (ordinal, expression) in ["try await task()"].into_iter().enumerate() {
        let name = SourceName::path(format!("propagating-await-{ordinal}.arcw"));
        let source = format!("flow story {{\n    let value = {expression}\n}}\n");
        let mut database = syntax_database();
        let parsed = database
            .parse_initial(
                SourceSnapshotId::initial(name.clone()),
                source_document(&name, source),
                crate::parser::ParseOptions::default(),
            )
            .expect("propagating await commits");
        let nodes = parsed.attached().nodes().collect::<Vec<_>>();

        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.kind() == GrammarKind::AwaitExpression)
                .count(),
            1
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.kind() == GrammarKind::TryExpression)
                .count(),
            1
        );
    }

    let name = SourceName::path("grouped-try-await.arcw");
    let source = "flow story {\n    let value = try (await task())\n}\n";
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("explicit grouping retains two ordinary prefix nodes");
    let nodes = parsed.attached().nodes().collect::<Vec<_>>();
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.kind() == GrammarKind::AwaitExpression)
            .count(),
        1
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.kind() == GrammarKind::TryExpression)
            .count(),
        1
    );
}

#[test]
fn prefix_await_heads_consume_one_prefix_level_per_head() {
    for (ordinal, head) in ["try ", "await "].into_iter().enumerate() {
        let exact_name = SourceName::path(format!("await-depth-exact-{ordinal}.arcw"));
        let exact_expression = format!(
            "{}task()",
            head.repeat(super::SyntaxLimit::PrefixDepth.maximum())
        );
        let exact_source = format!("flow story {{\n    let value = {exact_expression}\n}}\n");
        let mut exact_database = syntax_database();
        exact_database
            .parse_initial(
                SourceSnapshotId::initial(exact_name.clone()),
                source_document(&exact_name, exact_source),
                crate::parser::ParseOptions::default(),
            )
            .expect("64 propagating await heads consume exactly 64 prefix levels");

        let one_over_name = SourceName::path(format!("await-depth-over-{ordinal}.arcw"));
        let one_over_expression = format!(
            "{}task()",
            head.repeat(super::SyntaxLimit::PrefixDepth.maximum() + 1)
        );
        let one_over_source = format!("flow story {{\n    let value = {one_over_expression}\n}}\n");
        let mut one_over_database = syntax_database();
        let failed = one_over_database.parse_initial(
            SourceSnapshotId::initial(one_over_name.clone()),
            source_document(&one_over_name, one_over_source),
            crate::parser::ParseOptions::default(),
        );

        assert!(matches!(
            failed,
            Err(ParseFailure::LimitExceeded(super::SyntaxLimit::PrefixDepth))
        ));
        assert!(!one_over_database.lineages.contains_key(&one_over_name));
    }
}

#[test]
fn assertion_condition_limit_accepts_exactly_64_and_rolls_back_one_over() {
    let name = SourceName::path("story.arcw");
    let mut database = syntax_database();
    let conditions = core::iter::repeat_n("true", 64)
        .collect::<Vec<_>>()
        .join(", ");
    let initial_source = format!("flow assertions {{\n    assert.check({conditions})\n}}\n");
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source.clone()),
            crate::parser::ParseOptions::default(),
        )
        .expect("the inclusive assertion-condition maximum succeeds");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();
    let one_over = format!("{conditions}, true");
    let one_over_source = format!("flow assertions {{\n    assert.check({one_over})\n}}\n");

    let failed = database.reparse(
        &initial,
        &[source_edit(
            &initial,
            SourceRange::new(0, initial_source.len()),
            one_over_source,
        )],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::LimitExceeded(
            super::SyntaxLimit::AssertionConditions
        ))
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&initial));
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
    assert_eq!(current.current.source_snapshot_id().generation().get(), 1);
}

#[test]
fn source_generation_exhaustion_rolls_back_the_transaction() {
    let name = SourceName::path("story.arcw");
    let mut database = SyntaxDatabase::with_test_limits(super::SyntaxTransactionLimits {
        source_generation: 1,
    });
    let source = "flow story {}\n";
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial generation commits");
    let allocator_next = database
        .lineages
        .get(&name)
        .expect("lineage")
        .transaction
        .next_node_for_test();

    let failed = database.reparse(
        &initial,
        &[source_edit(&initial, SourceRange::new(5, 10), "changed")],
        crate::parser::ParseOptions::default(),
    );

    assert!(matches!(
        failed,
        Err(ParseFailure::SourceGenerationExhausted)
    ));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&initial));
    assert_eq!(current.current.source_snapshot_id().generation().get(), 1);
    assert_eq!(current.transaction.next_node_for_test(), allocator_next);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "identity exhaustion and rollback form one atomicity proof"
)]
fn syntax_identity_exhaustion_is_atomic() {
    let name = SourceName::path("story.arcw");
    let initial_source = "flow story {}\n";
    let addition = "flow final {}\n";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial parse");
    let invalid = database.reparse(
        &initial,
        &[
            source_edit(&initial, SourceRange::new(5, 8), "one"),
            source_edit(&initial, SourceRange::new(7, 10), "two"),
        ],
        ParseOptions::default(),
    );
    assert!(matches!(invalid, Err(ParseFailure::InvalidEdits(_))));
    assert!(
        database
            .current_for_source(&name)
            .expect("invalid edit retains current snapshot")
            .is_same_snapshot(&initial)
    );

    let mut control = syntax_database();
    let control_initial = control
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, initial_source),
            crate::parser::ParseOptions::default(),
        )
        .expect("control initial parse");
    let before = control
        .lineages
        .get(&name)
        .expect("control lineage")
        .transaction
        .next_node_for_test()
        .expect("control next node");
    control
        .reparse(
            &control_initial,
            &[source_edit(
                &control_initial,
                SourceRange::new(initial_source.len(), initial_source.len()),
                addition,
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("control reparse");
    let after = control
        .lineages
        .get(&name)
        .expect("control lineage")
        .transaction
        .next_node_for_test()
        .expect("control next node");
    let allocated = after
        .get()
        .checked_sub(before.get())
        .filter(|count| *count > 0)
        .expect("reparse allocates grammar identities");
    let last_start = u64::MAX
        .checked_sub(allocated - 1)
        .and_then(NonZeroU64::new)
        .expect("last allocatable grammar identity range");
    database
        .lineages
        .get_mut(&name)
        .expect("lineage")
        .transaction
        .set_next_node_for_test(Some(last_start));
    let with_last_id = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(initial.source().len(), initial.source().len()),
                addition,
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("the final non-zero ID is usable");
    let accepted_ids = identity_ids(&with_last_id);
    let accepted_diagnostics = with_last_id.diagnostics().to_vec();
    let accepted_status = with_last_id.status();
    let accepted_syntax_stats = with_last_id.syntax_stats();
    let next_node = database
        .lineages
        .get(&name)
        .expect("accepted lineage")
        .transaction
        .next_node_for_test();
    let next_lineage = database.transaction.next_lineage_for_test();
    let failed = database.reparse(
        &with_last_id,
        &[source_edit(
            &with_last_id,
            SourceRange::new(with_last_id.source().len(), with_last_id.source().len()),
            "flow overflow {}\n",
        )],
        crate::parser::ParseOptions::default(),
    );
    assert!(matches!(failed, Err(ParseFailure::NodeIdentityExhausted)));
    let current = database.lineages.get(&name).expect("lineage current");
    assert!(current.current.is_same_snapshot(&with_last_id));
    assert!(Arc::ptr_eq(current.current.data(), with_last_id.data()));
    assert!(Arc::ptr_eq(
        current.transaction.current(),
        with_last_id.data()
    ));
    assert_eq!(current.current.source_snapshot_id().generation().get(), 2);
    assert_eq!(current.current.diagnostics(), accepted_diagnostics);
    assert_eq!(current.current.status(), accepted_status);
    assert_eq!(current.current.syntax_stats(), accepted_syntax_stats);
    assert_eq!(identity_ids(&current.current), accepted_ids);
    assert_eq!(current.transaction.next_node_for_test(), next_node);
    assert_eq!(database.transaction.next_lineage_for_test(), next_lineage);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "same-line descendant identity cases form one collision matrix"
)]
fn same_line_descendants_receive_distinct_syntax_ids() {
    let name = SourceName::path("identity.arcw");
    let source = concat!(
        "predicate same((a, b): (Int, Int), c: Int) ",
        "requires a < c ensures b < c = (a + c) == (b + c)\n",
    );
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("same-line Predicate attaches");

    let nodes = parsed.attached().nodes().collect::<Vec<_>>();
    let ids = nodes
        .iter()
        .map(SyntaxNodeHandle::id)
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), nodes.len());
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::FixedParameterGroup,
        &[SourceRange::new(14, 42)],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::Parameter,
        &[SourceRange::new(15, 33), SourceRange::new(35, 41)],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::TuplePattern,
        &[SourceRange::new(15, 21)],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::BindingPattern,
        &[
            SourceRange::new(16, 17),
            SourceRange::new(19, 20),
            SourceRange::new(35, 36),
        ],
    );
    assert_ranges_for_kind(&parsed, GrammarKind::TupleType, &[SourceRange::new(23, 33)]);
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::PathType,
        &[
            SourceRange::new(24, 27),
            SourceRange::new(29, 32),
            SourceRange::new(38, 41),
        ],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::RequiresClause,
        &[SourceRange::new(43, 57)],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::EnsuresClause,
        &[SourceRange::new(58, 71)],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::PredicateBody,
        &[SourceRange::new(72, 92)],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::ExpressionBody,
        &[SourceRange::new(72, 92)],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::BinaryExpression,
        &[
            SourceRange::new(52, 57),
            SourceRange::new(66, 71),
            SourceRange::new(74, 92),
            SourceRange::new(75, 80),
            SourceRange::new(86, 91),
        ],
    );
    assert_ranges_for_kind(
        &parsed,
        GrammarKind::PathExpression,
        &[
            SourceRange::new(52, 53),
            SourceRange::new(56, 57),
            SourceRange::new(66, 67),
            SourceRange::new(70, 71),
            SourceRange::new(75, 76),
            SourceRange::new(79, 80),
            SourceRange::new(86, 87),
            SourceRange::new(90, 91),
        ],
    );
    assert_eq!(
        node_with_kind_and_range(
            &parsed,
            GrammarKind::NameDefinition,
            SourceRange::new(10, 14),
        )
        .range(),
        SourceRange::new(10, 14)
    );
    assert!(nodes.iter().all(|node| node.range().end() <= source.len()));
    assert!(nodes.iter().all(|node| {
        node.kind().identity_class() == crate::grammar::kinds::IdentityClass::IdentityBearing
    }));
}

#[test]
fn lossless_root_round_trips_every_utf8_byte() {
    let name = SourceName::path("lossless-utf8.arcw");
    let source = concat!(
        "/// 語りの文書\r\n",
        "\r\n",
        "flow café {\r\n",
        "    // 注釈と空白\r\n",
        "    ??? === `未完\r\n",
        "}\r\n",
    );
    let document = source_document(&name, source);
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("malformed UTF-8-safe source remains lossless and recoverable");
    let root = parsed.root_syntax();

    assert_eq!(parsed.status(), super::ParseStatus::Recovered);
    assert_eq!(parsed.source().as_bytes(), source.as_bytes());
    assert_eq!(parsed.document().text().as_bytes(), source.as_bytes());
    assert_eq!(root.rowan().to_string().as_bytes(), source.as_bytes());
    assert_eq!(root.source_text().as_bytes(), source.as_bytes());
    assert_eq!(root.range(), SourceRange::new(0, source.len()));
    assert_eq!(root.source_span().source(), document.identity());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "typed/Rowan round-trip rows form one exhaustive ownership matrix"
)]
fn typed_to_rowan_and_rowan_to_typed_round_trip() {
    let name = SourceName::path("typed-rowan-round-trip.arcw");
    let source = concat!(
        "predicate expression(value: Bool) = value\n",
        "predicate ready<T>(value: T) where T: Ord\n",
        "requires value == value\n",
        "ensures result\n",
        "{ let checked: Bool = true; checked }\n",
        "proof checked(value: Bool) -> Bool\n",
        "requires verify(value)\n",
        "ensures result\n",
        "{ let typed: Bool = verify(value); assert.check(typed); verify_nested(typed); }\n",
    );
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            ParseOptions::default(),
        )
        .expect("typed source snapshot");
    assert_eq!(parsed.status(), super::ParseStatus::Clean);

    for typed_syntax in parsed.attached().nodes() {
        assert_eq!(
            typed_syntax.kind().identity_class(),
            crate::grammar::kinds::IdentityClass::IdentityBearing
        );
        let rebound = parsed
            .bind_rowan(typed_syntax.rowan())
            .expect("Rowan node binds only through its exact snapshot owner");
        assert_eq!(rebound, typed_syntax);
        assert_eq!(
            parsed
                .syntax_node(rebound.id())
                .expect("stable identity resolves to the same typed syntax"),
            typed_syntax
        );
    }

    macro_rules! assert_round_trip_matrix {
        ($($marker:ty => $kind:ident),+ $(,)?) => {
            $(assert_exact_typed_round_trip::<$marker>(&parsed, GrammarKind::$kind);)+
        };
    }
    assert_round_trip_matrix! {
        SourceFileKind => SourceFile,
        PredicateItemKind => PredicateItem,
        ProofItemKind => ProofItem,
        NameDefinitionKind => NameDefinition,
        GenericParameterGroupKind => GenericParameterGroup,
        GenericParameterKind => GenericParameter,
        TypeParameterKind => TypeParameter,
        FixedParameterGroupKind => FixedParameterGroup,
        ParameterKind => Parameter,
        BindingPatternKind => BindingPattern,
        TypedBindingPatternKind => TypedBindingPattern,
        PathTypeKind => PathType,
        WhereClauseKind => WhereClause,
        WherePredicateKind => WherePredicate,
        ReturnTypeKind => ReturnType,
        RequiresClauseKind => RequiresClause,
        EnsuresClauseKind => EnsuresClause,
        ExpressionBodyKind => ExpressionBody,
        PredicateBodyKind => PredicateBody,
        ProofBodyKind => ProofBody,
        PredicateBlockKind => PredicateBlock,
        ProofBlockKind => ProofBlock,
        OpenAngleKind => OpenAngleNode,
        CloseAngleKind => CloseAngleNode,
        OpenParenKind => OpenParenNode,
        CloseParenKind => CloseParenNode,
        ColonKind => ColonNode,
        ThinArrowKind => ThinArrowNode,
        OpenBraceKind => OpenBraceNode,
        CloseBraceKind => CloseBraceNode,
        LetStatementKind => LetStatement,
        AssertionStatementKind => AssertionStatement,
        ProofCallStatementKind => ProofCallStatement,
        BinaryExpressionKind => BinaryExpression,
        LiteralExpressionKind => LiteralExpression,
        CallExpressionKind => CallExpression,
        CallArgumentKind => CallArgument,
        PathExpressionKind => PathExpression,
        PathKind => Path,
        OmittedBlockTailKind => OmittedBlockTail,
    }

    let predicate_block = only_node_with_kind(&parsed, GrammarKind::PredicateBlock)
        .cast::<PredicateBlockKind>()
        .expect("typed Predicate block");
    assert_eq!(
        predicate_block
            .statements()
            .expect("typed Predicate statement list")
            .iter()
            .map(crate::attachment::family::FamilyNode::kind)
            .collect::<Vec<_>>(),
        [GrammarKind::LetStatement]
    );
    assert!(matches!(
        predicate_block.tail().expect("typed Predicate tail"),
        BlockTailNode::Expression(_)
    ));

    let proof_block = only_node_with_kind(&parsed, GrammarKind::ProofBlock)
        .cast::<ProofBlockKind>()
        .expect("typed Proof block");
    assert_eq!(
        proof_block
            .statements()
            .expect("typed Proof statement list")
            .iter()
            .map(crate::attachment::family::FamilyNode::kind)
            .collect::<Vec<_>>(),
        [
            GrammarKind::LetStatement,
            GrammarKind::AssertionStatement,
            GrammarKind::ProofCallStatement,
        ]
    );
    assert!(matches!(
        proof_block.tail().expect("typed Proof tail"),
        BlockTailNode::Omitted(_)
    ));
    assert_eq!(
        GrammarKind::StatementList.identity_class(),
        crate::grammar::kinds::IdentityClass::StructuralWrapper
    );
}

#[test]
fn missing_child_identity_is_role_and_anchor_specific() {
    let name = SourceName::path("missing-child-identity.arcw");
    let source = "proof unit() {";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            ParseOptions::default(),
        )
        .expect("missing Proof children recover");
    assert_eq!(initial.status(), super::ParseStatus::Recovered);
    let old_close = only_node_with_kind(&initial, GrammarKind::CloseBraceNode);
    let old_tail = only_node_with_kind(&initial, GrammarKind::OmittedBlockTail);
    assert_eq!(old_close.range(), SourceRange::new(14, 14));
    assert_eq!(old_tail.range(), SourceRange::new(14, 14));
    assert_eq!(
        old_close.role(),
        crate::grammar::kinds::SyntaxRole::CloseDelimiter
    );
    assert_eq!(old_tail.role(), crate::grammar::kinds::SyntaxRole::Tail);
    assert_ne!(old_close.id(), old_tail.id());
    assert_eq!(
        old_close.parent().expect("ProofBlock close owner").id(),
        old_tail.parent().expect("ProofBlock tail owner").id()
    );
    old_close
        .cast::<CloseBraceKind>()
        .expect("typed missing close brace");
    old_tail
        .cast::<OmittedBlockTailKind>()
        .expect("typed omitted tail");

    let trivia = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(13, 13), "  ")],
            ParseOptions::default(),
        )
        .expect("trivia-only missing-child reparse");
    let trivia_close = only_node_with_kind(&trivia, GrammarKind::CloseBraceNode);
    let trivia_tail = only_node_with_kind(&trivia, GrammarKind::OmittedBlockTail);
    assert_eq!(trivia_close.id(), old_close.id());
    assert_eq!(trivia_tail.id(), old_tail.id());
    assert_eq!(trivia_close.range(), SourceRange::new(16, 16));
    assert_eq!(trivia_tail.range(), SourceRange::new(16, 16));

    let different_parent_source = "proof unit() {}\nproof other() {";
    let different_parent = database
        .reparse(
            &trivia,
            &[source_edit(
                &trivia,
                SourceRange::new(0, trivia.source().len()),
                different_parent_source,
            )],
            ParseOptions::default(),
        )
        .expect("same missing role under a different semantic parent recovers");
    let different_parent_close = node_with_kind_and_range(
        &different_parent,
        GrammarKind::CloseBraceNode,
        SourceRange::new(different_parent_source.len(), different_parent_source.len()),
    );
    assert_eq!(
        different_parent_close.role(),
        crate::grammar::kinds::SyntaxRole::CloseDelimiter
    );
    assert_ne!(
        different_parent_close
            .parent()
            .expect("new ProofBlock close owner")
            .id(),
        old_close.parent().expect("old ProofBlock close owner").id()
    );
    assert_ne!(different_parent_close.id(), old_close.id());

    let changed_source = "proof unit() = (";
    let changed = database
        .reparse(
            &different_parent,
            &[source_edit(
                &different_parent,
                SourceRange::new(0, different_parent.source().len()),
                changed_source,
            )],
            ParseOptions::default(),
        )
        .expect("different expected delimiter class recovers");
    let changed_close = node_with_kind_and_range(
        &changed,
        GrammarKind::CloseParenNode,
        SourceRange::new(16, 16),
    );
    assert_eq!(changed_close.range(), SourceRange::new(16, 16));
    assert_ne!(changed_close.id(), old_close.id());
    changed_close
        .cast::<CloseParenKind>()
        .expect("typed missing close parenthesis");
    assert!(matches!(
        changed.syntax_node(old_close.id()),
        Err(SyntaxLookupError::MissingNode { id }) if id == old_close.id()
    ));
}

#[test]
fn private_bound_product_retains_the_attached_snapshot_and_grammar_diagnostics() {
    let name = SourceName::path("bound-recovery.arcw");
    let source = "proof () = ()\n";
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("recovered proof commits one private bound product");
    let bound = parsed.data();

    assert_eq!(bound.snapshot_id().source(), parsed.source_snapshot_id());
    assert_eq!(bound.document().identity(), parsed.document().identity());
    assert_eq!(bound.document().text(), source);
    assert_eq!(bound.status(), super::ParseStatus::Recovered);
    let missing_name = bound
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.declaration.missing_name")
        .unwrap_or_else(|| panic!("missing bound diagnostic: {:?}", bound.diagnostics()));
    assert_eq!(
        missing_name.primary().source(),
        parsed.document().identity()
    );
    assert_eq!(
        missing_name.primary().range(),
        SourceRange::new("proof ".len(), "proof ".len())
    );
    assert!(missing_name.related().is_none());
    assert!(!missing_name.message().is_empty());
    assert!(Arc::ptr_eq(bound.syntax(), parsed.attached()));
}

#[test]
fn private_bound_diagnostic_spans_share_the_exact_committed_source_revision() {
    let name = SourceName::path("bound-related-diagnostic.arcw");
    let source = concat!(
        "character Alice {\n",
        "    display_name = \"Alice\"\n",
        "    display_name = \"Other\"\n",
        "}\n",
    );
    let display_name_offsets = source
        .match_indices("display_name")
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    let mut database = syntax_database();
    let parsed = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("duplicate member commits one private bound product");
    let duplicate = parsed
        .data()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.character.duplicate_member")
        .unwrap_or_else(|| {
            panic!(
                "missing duplicate-member diagnostic: {:?}",
                parsed.data().diagnostics()
            )
        });
    let related = duplicate
        .related()
        .expect("duplicate member retains its first declaration");

    assert_eq!(duplicate.primary().source(), parsed.document().identity());
    assert_eq!(related.source(), parsed.document().identity());
    assert_eq!(
        duplicate.primary().range(),
        SourceRange::new(
            display_name_offsets[1],
            display_name_offsets[1] + "display_name".len()
        )
    );
    assert_eq!(
        related.range(),
        SourceRange::new(
            display_name_offsets[0],
            display_name_offsets[0] + "display_name".len()
        )
    );
}

#[test]
fn private_bound_reparse_replaces_diagnostics_without_mutating_the_old_snapshot() {
    let name = SourceName::path("bound-reparse.arcw");
    let source = "proof () = ()\n";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("recovered initial proof");
    let old_bound = Arc::clone(initial.data());
    assert_eq!(old_bound.status(), super::ParseStatus::Recovered);

    let repaired = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new("proof ".len(), "proof ".len()),
                "fixed",
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("repair commits a fresh private bound product");

    assert!(!Arc::ptr_eq(&old_bound, repaired.data()));
    assert_eq!(old_bound.document().text(), source);
    assert_eq!(old_bound.status(), super::ParseStatus::Recovered);
    assert_eq!(repaired.data().document().text(), "proof fixed() = ()\n");
    assert_eq!(repaired.data().status(), super::ParseStatus::Clean);
    assert!(repaired.data().diagnostics().is_empty());
    assert_eq!(
        repaired.data().snapshot_id().source(),
        repaired.source_snapshot_id()
    );
}

#[test]
fn attached_expression_fragment_owns_one_fresh_lineage_and_exact_source_span() {
    let name = SourceName::path("attached-expression-fragment.arcw");
    let source = "before try value after";
    let fragment_text = "try value";
    let fragment_start = source.find(fragment_text).expect("fragment text");
    let fragment_end = fragment_start + fragment_text.len();
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source);
    let span = source_span(&document, SourceRange::new(fragment_start, fragment_end));
    let mut database = syntax_database();

    let unbound = parse_expression_fragment(fragment_text, ParseOptions::default());
    assert_eq!(unbound.text(), fragment_text);
    assert_eq!(unbound.completion(), &ParseCompletion::Complete);
    assert!(unbound.diagnostics().is_empty());
    let fragment = database
        .attach_fragment(
            snapshot.clone(),
            Arc::clone(&document),
            span.clone(),
            unbound,
        )
        .expect("complete expression attaches without reparsing");

    assert_eq!(fragment.snapshot_id().source(), &snapshot);
    assert_eq!(fragment.root().kind(), GrammarKind::TryExpression);
    assert_eq!(
        fragment.root().range(),
        SourceRange::new(fragment_start, fragment_end)
    );
    assert_eq!(
        fragment
            .root()
            .syntax()
            .parent()
            .expect("source-file root")
            .kind(),
        GrammarKind::SourceFile
    );
    assert_eq!(
        fragment
            .root()
            .syntax()
            .parent()
            .expect("fragment source-file root")
            .rowan()
            .to_string(),
        source
    );

    let second = database
        .attach_fragment(
            snapshot.clone(),
            Arc::clone(&document),
            span,
            parse_expression_fragment(fragment_text, ParseOptions::default()),
        )
        .expect("each explicit attachment owns a fresh lineage");
    assert_ne!(
        fragment.snapshot_id().lineage(),
        second.snapshot_id().lineage()
    );
    assert_ne!(fragment.root().id(), second.root().id());
}

#[test]
fn grouped_expression_fragment_selects_inner_identity_and_retains_outer_span() {
    let name = SourceName::path("attached-grouped-expression-fragment.arcw");
    let source = "before ((value)) after";
    let fragment_text = "((value))";
    let fragment_start = source.find(fragment_text).expect("fragment text");
    let fragment_end = fragment_start + fragment_text.len();
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source);
    let span = source_span(&document, SourceRange::new(fragment_start, fragment_end));
    let mut database = syntax_database();

    let fragment = database
        .attach_fragment(
            snapshot,
            Arc::clone(&document),
            span.clone(),
            parse_expression_fragment(fragment_text, ParseOptions::default()),
        )
        .expect("grouped expression attaches through its semantic root");

    assert_eq!(fragment.root().kind(), GrammarKind::PathExpression);
    assert_eq!(fragment.root().syntax().source_text(), "value");
    assert_eq!(
        fragment.root().syntax().role(),
        crate::grammar::kinds::SyntaxRole::Element(0)
    );
    assert_eq!(fragment.whole_source_span(), &span);
    assert_eq!(
        fragment.root().range(),
        SourceRange::new(fragment_start + 2, fragment_end - 2)
    );
}

#[test]
fn attached_leaf_fragment_rebases_semantic_components_into_the_target_revision() {
    let name = SourceName::path("attached-leaf-fragment.arcw");
    let source = "prefix\n42ms\nsuffix";
    let fragment_text = "42ms";
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source);
    let mut database = syntax_database();
    let fragment = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        fragment_text,
        parse_expression_fragment(fragment_text, ParseOptions::default()),
        GrammarKind::LiteralExpression,
    );

    let semantic = fragment
        .root()
        .semantic()
        .expect("attached leaf retains its semantic projection");
    assert!(matches!(
        semantic.projection(),
        ExpressionProjection::Literal(_)
    ));
    assert_eq!(
        &source[semantic.whole_source_span().range().as_range()],
        fragment_text
    );
    let body = semantic
        .component(ExpressionComponentRole::Literal(
            ExpressionLiteralPart::Body,
        ))
        .expect("rebased literal body");
    let unit = semantic
        .component(ExpressionComponentRole::Literal(
            ExpressionLiteralPart::Unit,
        ))
        .expect("rebased literal unit");
    assert_eq!(&source[body.range().as_range()], "42");
    assert_eq!(&source[unit.range().as_range()], "ms");
    assert_eq!(body.source(), document.identity());
    assert_eq!(unit.source(), document.identity());
}

#[test]
fn parsed_expression_lookup_is_exact_and_rejects_stale_or_foreign_ownership() {
    let name = SourceName::path("parsed-expression-identity.arcw");
    let source = "predicate leaf() = 42ms\n";
    let document = source_document(&name, source);
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("initial attached expression source");
    let literal = initial
        .attached()
        .nodes()
        .find(|node| node.kind() == GrammarKind::LiteralExpression)
        .expect("literal expression identity");
    let semantic = initial
        .attached_expression(literal.id())
        .expect("ParsedSource resolves the exact attached expression");
    assert_eq!(semantic.whole_source_span().source(), document.identity());
    assert_eq!(semantic.whole_source_span().range(), literal.range());

    let literal_start = source.find("42ms").expect("literal spelling");
    let current = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(literal_start, literal_start + "42ms".len()),
                "43ms",
            )],
            ParseOptions::default(),
        )
        .expect("current expression generation");
    assert!(matches!(
        current.resolve_exact_syntax(&semantic.syntax().syntax()),
        Err(SyntaxLookupError::WrongSnapshot { .. })
    ));

    let mut foreign_database = syntax_database();
    let foreign = foreign_database
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            ParseOptions::default(),
        )
        .expect("foreign attached expression source");
    let foreign_id = foreign
        .attached()
        .nodes()
        .find(|node| node.kind() == GrammarKind::LiteralExpression)
        .expect("foreign literal identity")
        .id();
    assert!(matches!(
        initial.attached_expression(foreign_id),
        Err(SyntaxAccessError::Lookup(
            SyntaxLookupError::WrongDatabase { .. }
        ))
    ));
}

#[test]
fn every_final_fragment_family_attaches_its_typed_root_at_the_exact_span() {
    let name = SourceName::path("attached-fragment-families.arcw");
    let source = concat!(
        "prefix\n",
        "left + right\n",
        "Result<String, Error>\n",
        ".Some(mut value)\n",
        "let answer: I32 = call(1);\n",
        "suffix",
    );
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source);
    let mut database = syntax_database();

    let expression_text = "left + right";
    let attached_expression = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        expression_text,
        parse_expression_fragment(expression_text, ParseOptions::default()),
        GrammarKind::BinaryExpression,
    );

    let type_text = "Result<String, Error>";
    let attached_type = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        type_text,
        parse_type_fragment(type_text, ParseOptions::default()),
        GrammarKind::GenericApplicationType,
    );
    let semantic_type = attached_type
        .root()
        .semantic()
        .expect("attached semantic type");
    assert_eq!(semantic_type.family(), AttachedTypeFamily::Generic);
    assert_eq!(semantic_type.children().unwrap().len(), 2);
    let whole = semantic_type.whole_source_span();
    assert_eq!(&source[whole.range().as_range()], type_text);
    let open = semantic_type
        .component(TypeRefComponentRole::GenericOpen)
        .expect("rebased generic open source");
    assert_eq!(&source[open.range().as_range()], "<");

    let pattern_text = ".Some(mut value)";
    let attached_pattern = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        pattern_text,
        parse_pattern_fragment(pattern_text, ParseOptions::default()),
        GrammarKind::VariantPattern,
    );
    let semantic_pattern = attached_pattern
        .root()
        .semantic()
        .expect("attached semantic Pattern");
    assert_eq!(semantic_pattern.family(), PatternSyntaxFamily::Variant);
    assert_eq!(semantic_pattern.children().unwrap().len(), 1);
    assert_eq!(
        semantic_pattern.children().unwrap()[0]
            .pattern()
            .expect("variant payload Pattern child")
            .family(),
        PatternSyntaxFamily::Tuple
    );
    let whole = semantic_pattern.whole_source_span();
    assert_eq!(&source[whole.range().as_range()], pattern_text);
    let root_syntax = attached_pattern.root().syntax();
    let root_projection = root_syntax
        .pattern_projection()
        .expect("root Pattern projection");
    let payload = semantic_pattern.children().unwrap()[0]
        .pattern()
        .expect("variant payload Pattern child")
        .clone();
    let payload_syntax = payload.syntax();
    let payload_projection = payload_syntax
        .pattern_projection()
        .expect("payload Pattern projection");
    assert!(Arc::ptr_eq(
        root_projection.authored(),
        payload_projection.authored()
    ));

    let statement_text = "let answer: I32 = call(1);";
    let attached_statement = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        statement_text,
        parse_statement_fragment(statement_text, ParseOptions::default()),
        GrammarKind::LetStatement,
    );
    let lineages = HashSet::from([
        attached_expression.snapshot_id().lineage(),
        attached_type.snapshot_id().lineage(),
        attached_pattern.snapshot_id().lineage(),
        attached_statement.snapshot_id().lineage(),
    ]);
    assert_eq!(lineages.len(), 4);
}

// One shared attachment transaction is the subject of this cross-family matrix.
#[test]
#[allow(clippy::too_many_lines)]
fn attached_pattern_projection_keeps_parser_owned_semantic_families_and_components() {
    let name = SourceName::path("attached-pattern-projection.arcw");
    let source = concat!(
        "name\n",
        "left | right\n",
        "whole .Some(value)\n",
        "typed: Vec<I32>\n",
    );
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source);
    let mut database = syntax_database();

    let binding = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        "name",
        parse_pattern_fragment("name", ParseOptions::default()),
        GrammarKind::BindingPattern,
    )
    .root()
    .semantic()
    .unwrap();
    assert_eq!(binding.family(), PatternSyntaxFamily::Binding);
    assert_eq!(
        binding
            .component(PatternComponentRole::Name)
            .unwrap()
            .range(),
        SourceRange::new(0, 4)
    );

    let or = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        "left | right",
        parse_pattern_fragment("left | right", ParseOptions::default()),
        GrammarKind::OrPattern,
    )
    .root()
    .semantic()
    .unwrap();
    assert_eq!(or.family(), PatternSyntaxFamily::Or);
    assert_eq!(or.children().unwrap().len(), 2);
    assert!(
        or.component(PatternComponentRole::Element { ordinal: 0 })
            .is_some()
    );
    assert!(
        or.component(PatternComponentRole::Element { ordinal: 1 })
            .is_some()
    );

    let whole = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        "whole .Some(value)",
        parse_pattern_fragment("whole .Some(value)", ParseOptions::default()),
        GrammarKind::WholeBindingPattern,
    )
    .root()
    .semantic()
    .unwrap();
    assert_eq!(whole.family(), PatternSyntaxFamily::WholeBinding);
    assert_eq!(whole.children().unwrap().len(), 1);
    assert_eq!(
        whole.children().unwrap()[0]
            .pattern()
            .expect("whole-binding nested Pattern")
            .family(),
        PatternSyntaxFamily::Variant
    );
    assert!(
        whole
            .component(PatternComponentRole::WholeBindingName)
            .is_some()
    );
    assert!(
        whole
            .component(PatternComponentRole::NestedPattern)
            .is_some()
    );

    let typed_fragment = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        "typed: Vec<I32>",
        parse_pattern_fragment("typed: Vec<I32>", ParseOptions::default()),
        GrammarKind::TypedBindingPattern,
    );
    let typed = typed_fragment.root().semantic().unwrap();
    assert_eq!(typed.family(), PatternSyntaxFamily::TypedBinding);
    assert!(
        typed
            .component(PatternComponentRole::TypedBindingColon)
            .is_some()
    );
    assert!(
        typed
            .component(PatternComponentRole::TypedBindingType)
            .is_some()
    );
    let typed_children = typed.children().unwrap();
    assert_eq!(typed_children.len(), 1);
    let type_child = typed_children[0]
        .type_ref()
        .expect("typed-binding type child");
    let pattern_syntax = typed_fragment.root().syntax();
    let pattern_projection = pattern_syntax
        .pattern_projection()
        .expect("typed-binding Pattern projection");
    let type_syntax = type_child.syntax();
    let type_projection = type_syntax
        .type_projection()
        .expect("typed-binding type projection");
    let owned_type = pattern_projection
        .authored()
        .source()
        .type_child_at(
            pattern_projection.path(),
            PatternTypeChildRelation::TypedBinding,
        )
        .expect("Pattern source map type child");
    assert!(Arc::ptr_eq(
        type_projection.authored(),
        owned_type.authored()
    ));
}

#[test]
fn attached_pattern_projection_covers_all_twelve_authored_families() {
    let cases = [
        (
            "binding",
            GrammarKind::BindingPattern,
            PatternSyntaxFamily::Binding,
        ),
        (
            "mut changing",
            GrammarKind::MutableBindingPattern,
            PatternSyntaxFamily::MutableBinding,
        ),
        (
            "42",
            GrammarKind::LiteralPattern,
            PatternSyntaxFamily::Literal,
        ),
        (
            "@flow.main",
            GrammarKind::EntityReferencePattern,
            PatternSyntaxFamily::EntityReference,
        ),
        (
            ".Some(payload)",
            GrammarKind::VariantPattern,
            PatternSyntaxFamily::Variant,
        ),
        (
            "_",
            GrammarKind::WildcardPattern,
            PatternSyntaxFamily::Discard,
        ),
        (
            "(tuple_left, tuple_right)",
            GrammarKind::TuplePattern,
            PatternSyntaxFamily::Tuple,
        ),
        (
            "Point { x, y: mut record_value, ..record_rest }",
            GrammarKind::RecordPattern,
            PatternSyntaxFamily::Record,
        ),
        (
            "[sequence_head, ..sequence_rest]",
            GrammarKind::SequencePattern,
            PatternSyntaxFamily::BracketSequence,
        ),
        (
            "whole .Some(nested)",
            GrammarKind::WholeBindingPattern,
            PatternSyntaxFamily::WholeBinding,
        ),
        (
            "or_left | or_right",
            GrammarKind::OrPattern,
            PatternSyntaxFamily::Or,
        ),
        (
            "typed_name: Vec<I32>",
            GrammarKind::TypedBindingPattern,
            PatternSyntaxFamily::TypedBinding,
        ),
    ];
    let source = cases
        .iter()
        .map(|(text, _, _)| *text)
        .collect::<Vec<_>>()
        .join("\n");
    let name = SourceName::path("attached-pattern-family-matrix.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source);
    let mut database = syntax_database();

    for (text, kind, family) in cases {
        let fragment = parse_pattern_fragment(text, ParseOptions::default());
        assert_eq!(fragment.completion(), &ParseCompletion::Complete, "{text}");
        let semantic =
            attach_exact_fragment(&mut database, &snapshot, &document, text, fragment, kind)
                .root()
                .semantic()
                .expect("every final Pattern family owns a semantic projection");
        assert_eq!(semantic.family(), family, "{text}");
        assert_eq!(
            semantic
                .components()
                .first()
                .map(AttachedPatternComponent::role),
            Some(PatternComponentRole::Whole),
            "{text}"
        );
    }
}

#[test]
fn attached_absolute_and_family_entity_patterns_project_final_id_components() {
    let absolute = assert_attached_pattern_components(
        "@flow.opening.next",
        GrammarKind::EntityReferencePattern,
        &[
            (id_part(SyntaxIdRefPart::AbsoluteMarker), 0, 1),
            (id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }), 1, 5),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 1 }),
                6,
                13,
            ),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 2 }),
                14,
                18,
            ),
        ],
    );
    assert!(
        absolute
            .component(id_part(SyntaxIdRefPart::Family))
            .is_none()
    );

    let family = assert_attached_pattern_components(
        "@flow:..opening.next",
        GrammarKind::EntityReferencePattern,
        &[
            (id_part(SyntaxIdRefPart::Family), 1, 5),
            (id_part(SyntaxIdRefPart::FamilySeparator), 5, 6),
            (id_part(SyntaxIdRefPart::ParentMarker { ordinal: 0 }), 7, 8),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }),
                8,
                15,
            ),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 1 }),
                16,
                20,
            ),
        ],
    );
    assert!(
        family
            .component(id_part(SyntaxIdRefPart::AbsoluteMarker))
            .is_none()
    );

    assert_attached_pattern_components(
        "@<flow.opening@sem:abc>",
        GrammarKind::EntityReferencePattern,
        &[
            (id_part(SyntaxIdRefPart::AbsoluteMarker), 0, 2),
            (id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }), 2, 6),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 1 }),
                7,
                22,
            ),
        ],
    );
}

#[test]
fn attached_relative_entity_patterns_project_parents_suffixes_and_recovery() {
    assert_attached_pattern_components(
        "@...outer.leaf",
        GrammarKind::EntityReferencePattern,
        &[
            (id_part(SyntaxIdRefPart::ParentMarker { ordinal: 0 }), 2, 3),
            (id_part(SyntaxIdRefPart::ParentMarker { ordinal: 1 }), 3, 4),
            (id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }), 4, 9),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 1 }),
                10,
                14,
            ),
        ],
    );
    assert_attached_pattern_components(
        "@super.super.outer.leaf",
        GrammarKind::EntityReferencePattern,
        &[
            (id_part(SyntaxIdRefPart::ParentMarker { ordinal: 0 }), 1, 6),
            (id_part(SyntaxIdRefPart::ParentMarker { ordinal: 1 }), 7, 12),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }),
                13,
                18,
            ),
            (
                id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 1 }),
                19,
                23,
            ),
        ],
    );
    assert_attached_pattern_components(
        "@...",
        GrammarKind::EntityReferencePattern,
        &[
            (id_part(SyntaxIdRefPart::ParentMarker { ordinal: 0 }), 2, 3),
            (id_part(SyntaxIdRefPart::ParentMarker { ordinal: 1 }), 3, 4),
            (id_part(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }), 4, 4),
        ],
    );
}

#[test]
fn attached_numeric_literal_patterns_project_prefix_body_suffix_and_unit() {
    let integer = assert_attached_pattern_components(
        "0xff_u8",
        GrammarKind::LiteralPattern,
        &[
            (literal_part(PatternLiteralPart::Prefix), 0, 2),
            (literal_part(PatternLiteralPart::Body), 2, 5),
            (literal_part(PatternLiteralPart::Suffix), 5, 7),
        ],
    );
    assert!(
        integer
            .component(literal_part(PatternLiteralPart::Unit))
            .is_none()
    );

    let float = assert_attached_pattern_components(
        "2.0f32",
        GrammarKind::LiteralPattern,
        &[
            (literal_part(PatternLiteralPart::Body), 0, 3),
            (literal_part(PatternLiteralPart::Suffix), 3, 6),
        ],
    );
    assert!(
        float
            .component(literal_part(PatternLiteralPart::Prefix))
            .is_none()
    );

    for text in ["10ms", "50%"] {
        let unit = assert_attached_pattern_components(
            text,
            GrammarKind::LiteralPattern,
            &[
                (literal_part(PatternLiteralPart::Body), 0, 2),
                (literal_part(PatternLiteralPart::Unit), 2, text.len()),
            ],
        );
        assert!(
            unit.component(literal_part(PatternLiteralPart::Suffix))
                .is_none()
        );
    }

    assert_attached_pattern_components(
        "0x",
        GrammarKind::LiteralPattern,
        &[
            (literal_part(PatternLiteralPart::Prefix), 0, 2),
            (literal_part(PatternLiteralPart::Body), 2, 2),
        ],
    );
}

#[test]
fn attached_text_and_boolean_literal_patterns_project_only_authored_parts() {
    assert_attached_pattern_components(
        "r##\"raw body\"##",
        GrammarKind::LiteralPattern,
        &[
            (literal_part(PatternLiteralPart::Prefix), 0, 3),
            (literal_part(PatternLiteralPart::Body), 4, 12),
        ],
    );

    for (text, start, end) in [("\"quoted\"", 1, 7), ("true", 0, 4)] {
        let plain = assert_attached_pattern_components(
            text,
            GrammarKind::LiteralPattern,
            &[(literal_part(PatternLiteralPart::Body), start, end)],
        );
        for absent in [
            PatternLiteralPart::Prefix,
            PatternLiteralPart::Suffix,
            PatternLiteralPart::Unit,
        ] {
            assert!(plain.component(literal_part(absent)).is_none());
        }
    }

    let character = assert_attached_pattern_components(
        "\"x\"c",
        GrammarKind::LiteralPattern,
        &[
            (literal_part(PatternLiteralPart::Body), 1, 2),
            (literal_part(PatternLiteralPart::Suffix), 3, 4),
        ],
    );
    for absent in [PatternLiteralPart::Prefix, PatternLiteralPart::Unit] {
        assert!(character.component(literal_part(absent)).is_none());
    }
}

#[test]
fn attached_pattern_paths_separate_module_roots_from_semantic_segments() {
    let cases = [
        "crate.Choice.Ready",
        "self.Choice.Ready",
        "super.super.Choice.Ready",
        "super::super::model::Point { x }",
    ];
    let source = cases.join("\n");
    let name = SourceName::path("attached-pattern-path-roots.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source.clone());
    let mut database = syntax_database();

    for (text, root_end) in [(cases[0], 5), (cases[1], 4), (cases[2], 11)] {
        let variant = attach_exact_fragment(
            &mut database,
            &snapshot,
            &document,
            text,
            parse_pattern_fragment(text, ParseOptions::default()),
            GrammarKind::VariantPattern,
        )
        .root()
        .semantic()
        .unwrap();
        assert_pattern_component(
            &source,
            text,
            &variant,
            PatternComponentRole::VariantHead(VariantPatternHeadPart::QualifiedRoot),
            0,
            root_end,
        );
        let choice = text.find("Choice").unwrap();
        assert_pattern_component(
            &source,
            text,
            &variant,
            PatternComponentRole::VariantHead(VariantPatternHeadPart::QualifiedSegment {
                ordinal: 0,
            }),
            choice,
            choice + "Choice".len(),
        );
        let ready = text.find("Ready").unwrap();
        assert_pattern_component(
            &source,
            text,
            &variant,
            PatternComponentRole::VariantName,
            ready,
            ready + "Ready".len(),
        );
        assert!(
            variant
                .component(PatternComponentRole::VariantHead(
                    VariantPatternHeadPart::QualifiedSegment { ordinal: 1 },
                ))
                .is_none()
        );
    }

    let record = attach_exact_fragment(
        &mut database,
        &snapshot,
        &document,
        cases[3],
        parse_pattern_fragment(cases[3], ParseOptions::default()),
        GrammarKind::RecordPattern,
    )
    .root()
    .semantic()
    .unwrap();
    assert_pattern_component(
        &source,
        cases[3],
        &record,
        PatternComponentRole::RecordPathRoot,
        0,
        12,
    );
    for (ordinal, segment) in ["model", "Point"].into_iter().enumerate() {
        let start = cases[3].find(segment).unwrap();
        assert_pattern_component(
            &source,
            cases[3],
            &record,
            PatternComponentRole::RecordPathSegment {
                ordinal: u32::try_from(ordinal).unwrap(),
            },
            start,
            start + segment.len(),
        );
    }
}

const fn id_part(part: SyntaxIdRefPart) -> PatternComponentRole {
    PatternComponentRole::EntityReference(part)
}

const fn literal_part(part: PatternLiteralPart) -> PatternComponentRole {
    PatternComponentRole::Literal(part)
}

fn assert_attached_pattern_components(
    text: &str,
    kind: GrammarKind,
    expected: &[(PatternComponentRole, usize, usize)],
) -> crate::attachment::AttachedPatternNode {
    let name = SourceName::path("attached-pattern-components.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, text);
    let mut database = syntax_database();
    let fragment = parse_pattern_fragment(text, ParseOptions::default());
    assert_eq!(fragment.completion(), &ParseCompletion::Complete, "{text}");
    let pattern = attach_exact_fragment(&mut database, &snapshot, &document, text, fragment, kind)
        .root()
        .semantic()
        .expect("attached Pattern owns its parser projection");
    for &(role, start, end) in expected {
        assert_pattern_component(text, text, &pattern, role, start, end);
    }
    pattern
}

fn assert_pattern_component(
    source: &str,
    fragment: &str,
    pattern: &crate::attachment::AttachedPatternNode,
    role: PatternComponentRole,
    expected_start: usize,
    expected_end: usize,
) {
    let base = source
        .find(fragment)
        .expect("fragment occurs in test source");
    let component = pattern
        .component(role)
        .unwrap_or_else(|| panic!("missing Pattern component {role:?} in {fragment:?}"));
    assert_eq!(
        component.range(),
        SourceRange::new(base + expected_start, base + expected_end),
        "{role:?} in {fragment:?}"
    );
}

#[test]
fn incomplete_invalid_mismatched_and_foreign_fragments_commit_no_lineage() {
    let name = SourceName::path("fragment-attachment-gates.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let mut database = syntax_database();
    let lineage_before = database.transaction.next_lineage_for_test();

    let incomplete_source = "call(";
    let incomplete_document = source_document(&name, incomplete_source);
    let incomplete_span = source_span(
        &incomplete_document,
        SourceRange::new(0, incomplete_source.len()),
    );
    let incomplete = parse_expression_fragment(incomplete_source, ParseOptions::default());
    assert!(matches!(
        incomplete.completion(),
        ParseCompletion::Incomplete { .. }
    ));
    let rejected = database.attach_fragment(
        snapshot.clone(),
        Arc::clone(&incomplete_document),
        incomplete_span,
        incomplete,
    );
    assert!(matches!(
        rejected,
        Err(ParseFailure::Attachment(
            AttachmentFailure::FragmentNotComplete {
                completion: ParseCompletion::Incomplete { .. }
            }
        ))
    ));
    assert_eq!(database.transaction.next_lineage_for_test(), lineage_before);

    let invalid_source = ")";
    let invalid_document = source_document(&name, invalid_source);
    let invalid_span = source_span(&invalid_document, SourceRange::new(0, invalid_source.len()));
    let rejected = database.attach_fragment(
        snapshot.clone(),
        Arc::clone(&invalid_document),
        invalid_span,
        parse_expression_fragment(invalid_source, ParseOptions::default()),
    );
    assert!(matches!(
        rejected,
        Err(ParseFailure::Attachment(
            AttachmentFailure::FragmentNotComplete {
                completion: ParseCompletion::Invalid
            }
        ))
    ));
    assert_eq!(database.transaction.next_lineage_for_test(), lineage_before);

    let mismatch_text = "value";
    let mismatch_document = source_document(&name, "other");
    let mismatch_span = source_span(&mismatch_document, SourceRange::new(0, mismatch_text.len()));
    let rejected = database.attach_fragment(
        snapshot.clone(),
        Arc::clone(&mismatch_document),
        mismatch_span,
        parse_expression_fragment(mismatch_text, ParseOptions::default()),
    );
    assert!(matches!(
        rejected,
        Err(ParseFailure::Attachment(
            AttachmentFailure::FragmentTextMismatch
        ))
    ));
    assert_eq!(database.transaction.next_lineage_for_test(), lineage_before);

    let exact_document = source_document(&name, mismatch_text);
    let foreign_name = SourceName::path("foreign-fragment.arcw");
    let foreign_document = source_document(&foreign_name, mismatch_text);
    let foreign_span = source_span(&foreign_document, SourceRange::new(0, mismatch_text.len()));
    let rejected = database.attach_fragment(
        snapshot.clone(),
        Arc::clone(&exact_document),
        foreign_span,
        parse_expression_fragment(mismatch_text, ParseOptions::default()),
    );
    assert!(matches!(rejected, Err(ParseFailure::SourceMismatch)));
    assert_eq!(database.transaction.next_lineage_for_test(), lineage_before);
}

#[test]
fn fragment_attachment_failure_consumes_no_lineage_or_node_identity() {
    let name = SourceName::path("fragment-attachment-rollback.arcw");
    let source = "value + 1";
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, source);
    let span = source_span(&document, SourceRange::new(0, source.len()));
    let mut database = syntax_database();
    let lineage_before = database.transaction.next_lineage_for_test();

    let failed = database.attach_fragment_with_attachment_failure(
        &snapshot,
        &document,
        &span,
        parse_expression_fragment(source, ParseOptions::default()),
    );
    assert!(matches!(failed, Err(ParseFailure::Attachment(_))));
    assert_eq!(database.transaction.next_lineage_for_test(), lineage_before);

    let accepted = database
        .attach_fragment(
            snapshot.clone(),
            Arc::clone(&document),
            span.clone(),
            parse_expression_fragment(source, ParseOptions::default()),
        )
        .expect("valid retry uses the unconsumed lineage");
    let mut control = syntax_database();
    let control = control
        .attach_fragment(
            snapshot,
            document,
            span,
            parse_expression_fragment(source, ParseOptions::default()),
        )
        .expect("control fragment");
    assert_eq!(accepted.root().id().slot(), control.root().id().slot());
}

#[test]
fn independent_databases_cannot_resolve_equal_raw_slots() {
    let name = SourceName::path("same.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let mut first_database = syntax_database();
    let mut second_database = syntax_database();
    let first = first_database
        .parse_initial(
            snapshot.clone(),
            source_document(&name, "proof valid() = ()\n"),
            crate::parser::ParseOptions::default(),
        )
        .expect("first database");
    let second = second_database
        .parse_initial(
            snapshot,
            source_document(&name, "proof valid() = ()\n"),
            crate::parser::ParseOptions::default(),
        )
        .expect("second database");
    let first_root = first.attached().root_handle();
    let second_root = second.attached().root_handle();

    assert_eq!(first_root.id().slot(), second_root.id().slot());
    assert_ne!(first_root.id(), second_root.id());
    assert_eq!(
        first.syntax_node(second_root.id()),
        Err(SyntaxLookupError::WrongDatabase {
            expected: first_database.database_id(),
            actual: second_database.database_id(),
        })
    );
    assert_eq!(
        first.typed_node::<SourceFileKind>(second_root.id()),
        Err(SyntaxLookupError::WrongDatabase {
            expected: first_database.database_id(),
            actual: second_database.database_id(),
        })
    );
    assert_eq!(
        first.bind_rowan(second_root.rowan()),
        Err(SyntaxLookupError::ForeignRowanRoot {
            expected: first.snapshot_id().clone(),
        })
    );
    assert_eq!(
        first
            .bind_rowan(first_root.rowan())
            .expect("first Rowan allocation binds in its owner"),
        first_root
    );
    assert_eq!(
        second
            .bind_rowan(second_root.rowan())
            .expect("second Rowan allocation binds in its owner"),
        second_root
    );
}

#[test]
fn foreign_lineage_in_same_database_is_rejected() {
    let first_name = SourceName::path("first-lineage.arcw");
    let second_name = SourceName::path("second-lineage.arcw");
    let source = "proof same() = ()\n";
    let mut database = syntax_database();
    let first = database
        .parse_initial(
            SourceSnapshotId::initial(first_name.clone()),
            source_document(&first_name, source),
            ParseOptions::default(),
        )
        .expect("first lineage");
    let second = database
        .parse_initial(
            SourceSnapshotId::initial(second_name.clone()),
            source_document(&second_name, source),
            ParseOptions::default(),
        )
        .expect("second lineage");
    let first_root = first.root_syntax();
    let second_root = second.root_syntax();

    assert_eq!(first_root.id().slot(), second_root.id().slot());
    assert_eq!(
        first.syntax_node(second_root.id()),
        Err(SyntaxLookupError::WrongLineage {
            expected: first.snapshot_id().lineage(),
            actual: second.snapshot_id().lineage(),
        })
    );
    assert_eq!(
        second.typed_node::<SourceFileKind>(first_root.id()),
        Err(SyntaxLookupError::WrongLineage {
            expected: second.snapshot_id().lineage(),
            actual: first.snapshot_id().lineage(),
        })
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "trivia reparse identity and range rows form one transactional matrix"
)]
fn trivia_only_reparse_preserves_predicate_proof_descendant_ids_and_updates_ranges() {
    let name = SourceName::path("predicate-proof-trivia.arcw");
    let source = concat!(
        "/// predicate docs\n",
        "predicate ready(value: Int)\n",
        "requires value > 0\n",
        "ensures value > 0\n",
        "= value == 1\n",
        "/// proof docs\n",
        "proof checked(value: Int) {\n",
        "    assert.check(value > 0);\n",
        "    value == 1\n",
        "}\n",
    );
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial Predicate and Proof");
    let old_nodes = initial.attached().nodes().collect::<Vec<_>>();
    let old_ranges = old_nodes
        .iter()
        .map(|node| (node.clone(), node.range()))
        .collect::<Vec<_>>();
    let old_ids = identity_ids(&initial);
    let old_items = initial.items().expect("initial typed item inventory");
    let old_predicate = typed_predicate_item(&old_items);
    let old_proof = typed_proof_item(&old_items);
    let old_names = [
        old_predicate
            .semantics()
            .expect("initial Predicate semantics")
            .name()
            .syntax()
            .syntax(),
        old_proof
            .semantics()
            .expect("initial Proof semantics")
            .name()
            .syntax()
            .syntax(),
    ];
    assert_eq!(
        old_names
            .iter()
            .map(SyntaxNodeHandle::range)
            .collect::<Vec<_>>(),
        [SourceRange::new(29, 34), SourceRange::new(118, 125)]
    );

    let trivia_source = concat!(
        "  /// predicate docs\n",
        "predicate ready( value: Int )\n",
        "requires   value > 0\n",
        "// retained comment\n",
        "ensures value > 0\n",
        "= value == 1\n",
        "\n",
        "    /// proof docs\n",
        "proof checked(value: Int) {\n",
        "        assert.check(value > 0);\n",
        "        value == 1\n",
        "}\n",
    );

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                trivia_source,
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("trivia reparse");
    assert_eq!(initial.source_snapshot_id().generation().get(), 1);
    assert_eq!(reparsed.source_snapshot_id().generation().get(), 2);
    assert_eq!(identity_ids(&reparsed), old_ids);

    for (old_node, old_range) in old_ranges {
        assert_eq!(old_node.range(), old_range);
        assert_eq!(
            old_node.source_span().source(),
            initial.document().identity()
        );
        let new_node = reparsed
            .syntax_node(old_node.id())
            .expect("every Predicate/Proof descendant survives trivia-only reparse");
        assert_eq!(new_node.kind(), old_node.kind());
        assert_eq!(new_node.role(), old_node.role());
        assert_eq!(
            new_node.source_span().source(),
            reparsed.document().identity()
        );
    }

    let new_items = reparsed.items().expect("reparsed typed item inventory");
    let new_predicate = typed_predicate_item(&new_items);
    let new_proof = typed_proof_item(&new_items);
    assert!(old_predicate.is_same_reconciled_node(&new_predicate));
    assert!(old_proof.is_same_reconciled_node(&new_proof));
    let new_names = [
        new_predicate
            .semantics()
            .expect("reparsed Predicate semantics")
            .name()
            .syntax()
            .syntax(),
        new_proof
            .semantics()
            .expect("reparsed Proof semantics")
            .name()
            .syntax()
            .syntax(),
    ];
    assert_eq!(
        new_names
            .iter()
            .map(SyntaxNodeHandle::range)
            .collect::<Vec<_>>(),
        [SourceRange::new(31, 36), SourceRange::new(149, 156)]
    );
    assert_eq!(new_names[0].id(), old_names[0].id());
    assert_eq!(new_names[1].id(), old_names[1].id());
    assert_eq!(old_names[0].range(), SourceRange::new(29, 34));
    assert_eq!(old_names[1].range(), SourceRange::new(118, 125));
    assert_eq!(
        reparsed.resolve_exact_syntax(&old_names[0]),
        Err(SyntaxLookupError::WrongSnapshot {
            expected: reparsed.snapshot_id().clone(),
            actual: initial.snapshot_id().clone(),
        })
    );
}

#[test]
fn same_parent_unique_reorder_preserves_ids() {
    let name = SourceName::path("reordered-proof-calls.arcw");
    let source = "proof order() { first(); second(); }\n";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial Proof call statements");
    let old_block = only_node_with_kind(&initial, GrammarKind::ProofBlock)
        .cast::<ProofBlockKind>()
        .expect("typed Proof block");
    let old_statements = old_block
        .statements()
        .expect("typed statement iterator")
        .into_iter()
        .map(|statement| {
            statement
                .cast::<ProofCallStatementKind>()
                .expect("Proof block contains only proof-call statements")
        })
        .collect::<Vec<_>>();
    assert_eq!(old_statements.len(), 2);
    assert_eq!(old_statements[0].range(), SourceRange::new(16, 24));
    assert_eq!(old_statements[1].range(), SourceRange::new(25, 34));
    let old_subtrees = old_statements
        .iter()
        .map(|statement| subtree_ids(&statement.syntax()))
        .collect::<Vec<_>>();
    let reordered_source = "proof order() { second(); first(); }\n";

    let reordered = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                reordered_source,
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("reordered Proof call statements");
    let new_block = only_node_with_kind(&reordered, GrammarKind::ProofBlock)
        .cast::<ProofBlockKind>()
        .expect("typed reordered Proof block");
    let new_statements = new_block
        .statements()
        .expect("typed reordered statement iterator")
        .into_iter()
        .map(|statement| {
            statement
                .cast::<ProofCallStatementKind>()
                .expect("reordered statement remains a proof call")
        })
        .collect::<Vec<_>>();
    assert_eq!(new_statements.len(), 2);
    assert_eq!(new_statements[0].range(), SourceRange::new(16, 25));
    assert_eq!(new_statements[1].range(), SourceRange::new(26, 34));
    assert_eq!(new_statements[0].id(), old_statements[1].id());
    assert_eq!(new_statements[1].id(), old_statements[0].id());
    assert_eq!(subtree_ids(&new_statements[0].syntax()), old_subtrees[1]);
    assert_eq!(subtree_ids(&new_statements[1].syntax()), old_subtrees[0]);
    assert_eq!(new_block.id(), old_block.id());
}

#[test]
fn copied_subtree_preserves_one_original_and_allocates_fresh_copy() {
    let name = SourceName::path("copied-assertion-condition.arcw");
    let source = "flow copied { assert.check(first == second) }\n";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial assertion condition");
    let old_assertion = only_node_with_kind(&initial, GrammarKind::AssertionStatement)
        .cast::<AssertionStatementKind>()
        .expect("typed assertion statement");
    let old_conditions = old_assertion
        .conditions()
        .expect("typed assertion conditions");
    assert_eq!(old_conditions.len(), 1);
    assert_eq!(old_conditions[0].range(), SourceRange::new(27, 42));
    let original_ids = subtree_ids(&old_conditions[0].syntax());
    let copied_source = "flow copied { assert.check(first == second, first == second) }\n";

    let copied = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                copied_source,
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("copied assertion condition");
    let new_assertion = only_node_with_kind(&copied, GrammarKind::AssertionStatement)
        .cast::<AssertionStatementKind>()
        .expect("typed copied assertion statement");
    let new_conditions = new_assertion.conditions().expect("typed copied conditions");
    assert_eq!(new_conditions.len(), 2);
    assert_eq!(new_conditions[0].range(), SourceRange::new(27, 42));
    assert_eq!(new_conditions[1].range(), SourceRange::new(44, 59));
    assert_eq!(subtree_ids(&new_conditions[0].syntax()), original_ids);
    let copied_ids = subtree_ids(&new_conditions[1].syntax());
    assert!(copied_ids.iter().all(|id| !original_ids.contains(id)));
    assert_eq!(
        identity_ids(&copied).len(),
        identity_ids(&copied)
            .into_iter()
            .collect::<HashSet<_>>()
            .len()
    );
}

#[test]
fn cross_parent_move_allocates_fresh_ids() {
    let name = SourceName::path("moved-let-statement.arcw");
    let source = concat!(
        "proof relocate() -> Int { ",
        "let first: Int = { let moved: Int = 1; 0 }; ",
        "let second: Int = { 0 }; ",
        "let third: Int = { let stay: Int = 2; 0 }; 0 }\n",
    );
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial nested let statements");
    let old_moved = node_with_kind_and_range(
        &initial,
        GrammarKind::LetStatement,
        SourceRange::new(45, 64),
    )
    .cast::<LetStatementKind>()
    .expect("typed moved let statement");
    let old_stay = node_with_kind_and_range(
        &initial,
        GrammarKind::LetStatement,
        SourceRange::new(114, 132),
    )
    .cast::<LetStatementKind>()
    .expect("typed unaffected let statement");
    let old_unaffected_block = node_with_kind_and_range(
        &initial,
        GrammarKind::BlockExpression,
        SourceRange::new(112, 136),
    )
    .cast::<BlockExpressionKind>()
    .expect("typed unaffected block expression");
    let old_moved_ids = subtree_ids(&old_moved.syntax());
    let old_stay_ids = subtree_ids(&old_stay.syntax());
    let moved_source = concat!(
        "proof relocate() -> Int { ",
        "let first: Int = { 0 }; ",
        "let second: Int = { let moved: Int = 1; 0 }; ",
        "let third: Int = { let stay: Int = 2; 0 }; 0 }\n",
    );

    let moved = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(0, source.len()),
                moved_source,
            )],
            crate::parser::ParseOptions::default(),
        )
        .expect("cross-parent let move");
    let new_moved =
        node_with_kind_and_range(&moved, GrammarKind::LetStatement, SourceRange::new(70, 89))
            .cast::<LetStatementKind>()
            .expect("typed moved let statement in new parent");
    let new_stay = node_with_kind_and_range(
        &moved,
        GrammarKind::LetStatement,
        SourceRange::new(114, 132),
    )
    .cast::<LetStatementKind>()
    .expect("typed unaffected let statement after move");
    let new_unaffected_block = node_with_kind_and_range(
        &moved,
        GrammarKind::BlockExpression,
        SourceRange::new(112, 136),
    )
    .cast::<BlockExpressionKind>()
    .expect("typed unaffected block expression after move");

    assert_ne!(new_moved.id(), old_moved.id());
    assert_ne!(
        new_moved.syntax().parent().expect("new block parent").id(),
        old_moved.syntax().parent().expect("old block parent").id()
    );
    assert!(
        subtree_ids(&new_moved.syntax())
            .iter()
            .all(|id| !old_moved_ids.contains(id))
    );
    assert_eq!(subtree_ids(&new_stay.syntax()), old_stay_ids);
    assert_eq!(new_unaffected_block.id(), old_unaffected_block.id());
}

#[test]
fn changed_grammar_node_gets_fresh_id_while_unchanged_siblings_survive() {
    let name = SourceName::path("changed-proof.arcw");
    let source = "proof changed() { let first: Int = 1; let second: Int = 2; () }\n";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("initial sibling statements");
    let old_first = node_with_kind_and_range(
        &initial,
        GrammarKind::LetStatement,
        SourceRange::new(18, 37),
    );
    let old_literal = node_with_kind_and_range(
        &initial,
        GrammarKind::LiteralExpression,
        SourceRange::new(35, 36),
    )
    .cast::<LiteralExpressionKind>()
    .expect("typed changed literal");
    let old_second = node_with_kind_and_range(
        &initial,
        GrammarKind::LetStatement,
        SourceRange::new(38, 58),
    );
    let old_second_ids = subtree_ids(&old_second);

    let changed = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(35, 36), "first + 1")],
            crate::parser::ParseOptions::default(),
        )
        .expect("changed initializer grammar");
    let new_first = node_with_kind_and_range(
        &changed,
        GrammarKind::LetStatement,
        SourceRange::new(18, 45),
    );
    let new_binary = node_with_kind_and_range(
        &changed,
        GrammarKind::BinaryExpression,
        SourceRange::new(35, 44),
    )
    .cast::<BinaryExpressionKind>()
    .expect("typed changed binary initializer");
    let new_second = node_with_kind_and_range(
        &changed,
        GrammarKind::LetStatement,
        SourceRange::new(46, 66),
    );

    assert_ne!(new_first.id(), old_first.id());
    assert_ne!(new_binary.id(), old_literal.id());
    assert!(matches!(
        changed.syntax_node(old_literal.id()),
        Err(SyntaxLookupError::MissingNode { id }) if id == old_literal.id()
    ));
    assert_eq!(new_second.id(), old_second.id());
    assert_eq!(subtree_ids(&new_second), old_second_ids);
    assert_eq!(new_second.range(), SourceRange::new(46, 66));
    assert_eq!(old_second.range(), SourceRange::new(38, 58));
}

#[test]
fn recovered_equivalent_node_survives_trivia_change() {
    let name = SourceName::path("recovered-expression-trivia.arcw");
    let source = "proof broken() = target.42\n";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("current-grammar recovery attaches");
    assert_eq!(initial.status(), super::ParseStatus::Recovered);

    let old_error = only_node_with_kind(&initial, GrammarKind::ErrorExpression);
    let old_missing = only_node_with_kind(&initial, GrammarKind::MissingName);
    assert_eq!(old_error.role(), crate::grammar::kinds::SyntaxRole::Body);
    assert_eq!(old_error.range(), SourceRange::new(17, 26));
    assert_eq!(
        old_missing.role(),
        crate::grammar::kinds::SyntaxRole::Field(0)
    );
    assert_eq!(old_missing.range(), SourceRange::new(24, 24));
    let old_select = old_missing
        .parent()
        .expect("missing member remains attached to its Select owner");
    assert_eq!(old_select.kind(), GrammarKind::SelectExpression);
    assert_eq!(
        old_select
            .parent()
            .expect("Select remains attached to its generic recovery owner")
            .id(),
        old_error.id()
    );
    let old_error_id = old_error.id();
    let old_missing_id = old_missing.id();

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(0, 0), "  ")],
            crate::parser::ParseOptions::default(),
        )
        .expect("equivalent recovered expression reparses after trivia");
    assert_eq!(reparsed.status(), super::ParseStatus::Recovered);

    let new_error = only_node_with_kind(&reparsed, GrammarKind::ErrorExpression);
    let new_missing = only_node_with_kind(&reparsed, GrammarKind::MissingName);
    assert_eq!(new_error.id(), old_error_id);
    assert_eq!(new_missing.id(), old_missing_id);
    assert_eq!(new_error.role(), old_error.role());
    assert_eq!(new_missing.role(), old_missing.role());
    let new_select = new_missing
        .parent()
        .expect("reconciled missing member remains attached to its Select owner");
    assert_eq!(new_select.kind(), GrammarKind::SelectExpression);
    assert_eq!(new_select.id(), old_select.id());
    assert_eq!(
        new_select
            .parent()
            .expect("reconciled Select remains attached to generic recovery")
            .id(),
        new_error.id()
    );
    assert_eq!(old_error.range(), SourceRange::new(17, 26));
    assert_eq!(old_missing.range(), SourceRange::new(24, 24));
    assert_eq!(new_error.range(), SourceRange::new(19, 28));
    assert_eq!(new_missing.range(), SourceRange::new(26, 26));
}

#[test]
fn generic_error_nodes_are_distinct_and_deterministic() {
    let name = SourceName::path("deterministic-error-statements.arcw");
    let source = "proof broken() { ???; ???; }\n";
    let mut database = syntax_database();
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("two generic statement recoveries attach");
    assert_eq!(initial.status(), super::ParseStatus::Recovered);
    let old_errors = initial
        .attached()
        .nodes()
        .filter(|node| node.kind() == GrammarKind::ErrorStatement)
        .collect::<Vec<_>>();
    assert_eq!(old_errors.len(), 2);
    assert_ne!(old_errors[0].id(), old_errors[1].id());
    assert_eq!(
        old_errors[0].role(),
        crate::grammar::kinds::SyntaxRole::Statement(0)
    );
    assert_eq!(
        old_errors[1].role(),
        crate::grammar::kinds::SyntaxRole::Statement(1)
    );
    assert_eq!(old_errors[0].range(), SourceRange::new(17, 21));
    assert_eq!(old_errors[1].range(), SourceRange::new(22, 26));
    let old_ids = old_errors
        .iter()
        .map(SyntaxNodeHandle::id)
        .collect::<Vec<_>>();

    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(&initial, SourceRange::new(0, 0), "  ")],
            crate::parser::ParseOptions::default(),
        )
        .expect("identical generic recoveries reparse after trivia");
    assert_eq!(reparsed.status(), super::ParseStatus::Recovered);
    let new_errors = reparsed
        .attached()
        .nodes()
        .filter(|node| node.kind() == GrammarKind::ErrorStatement)
        .collect::<Vec<_>>();
    assert_eq!(new_errors.len(), 2);
    assert_eq!(
        new_errors
            .iter()
            .map(SyntaxNodeHandle::id)
            .collect::<Vec<_>>(),
        old_ids
    );
    assert_eq!(new_errors[0].range(), SourceRange::new(19, 23));
    assert_eq!(new_errors[1].range(), SourceRange::new(24, 28));
    assert_eq!(old_errors[0].range(), SourceRange::new(17, 21));
    assert_eq!(old_errors[1].range(), SourceRange::new(22, 26));
}

#[test]
fn rich_text_attachment_failure_rolls_back_lineage_and_node_slots() {
    let name = SourceName::path("rich-text-attachment-failure.arcw");
    let source = concat!(
        "flow opening {\n",
        "    let line = alice[本文。[effect .wave amp=2 label=\"強い\"]]\n",
        "}\n",
    );
    let mut database = syntax_database();
    let lineage_before = database.transaction.next_lineage_for_test();
    let failed = database.parse_initial_with_attachment_failure(
        &SourceSnapshotId::initial(name.clone()),
        &source_document(&name, source),
    );

    assert!(matches!(failed, Err(ParseFailure::Attachment(_))));
    assert!(database.lineages.is_empty());
    assert_eq!(database.transaction.next_lineage_for_test(), lineage_before);

    let accepted = database
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(&name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("valid retry uses the unconsumed RichText lineage and slots");
    let mut control_database = syntax_database();
    let control_name = SourceName::path("rich-text-attachment-failure.arcw");
    let control = control_database
        .parse_initial(
            SourceSnapshotId::initial(control_name.clone()),
            source_document(&control_name, source),
            crate::parser::ParseOptions::default(),
        )
        .expect("control RichText transaction");

    let slots = |parsed: &ParsedSource| {
        parsed
            .attached()
            .nodes()
            .filter(|node| {
                matches!(
                    node.kind(),
                    GrammarKind::RichTextTag
                        | GrammarKind::RichTextArgumentPayload
                        | GrammarKind::RichTextPositionalArgument
                        | GrammarKind::RichTextNamedArgument
                        | GrammarKind::RichTextArgumentValue
                )
            })
            .map(|node| node.id().slot())
            .collect::<Vec<_>>()
    };
    assert_eq!(slots(&accepted), slots(&control));
}

fn identity_ids(source: &ParsedSource) -> Vec<SyntaxNodeId> {
    let mut ids = source
        .attached()
        .nodes()
        .map(|node| node.id())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

fn nodes_with_kind(source: &ParsedSource, kind: GrammarKind) -> Vec<SyntaxNodeHandle> {
    let mut nodes = source
        .attached()
        .nodes()
        .filter(|node| node.kind() == kind)
        .collect::<Vec<_>>();
    nodes.sort_by_key(|node| (node.range().start(), node.range().end(), node.id().slot()));
    nodes
}

fn node_with_kind_and_range(
    source: &ParsedSource,
    kind: GrammarKind,
    range: SourceRange,
) -> SyntaxNodeHandle {
    let mut nodes = source
        .attached()
        .nodes()
        .filter(|node| node.kind() == kind && node.range() == range);
    let node = nodes
        .next()
        .unwrap_or_else(|| panic!("missing exact {kind:?} at {range:?}"));
    assert!(
        nodes.next().is_none(),
        "expected exactly one {kind:?} at {range:?}"
    );
    node
}

fn subtree_ids(root: &SyntaxNodeHandle) -> Vec<SyntaxNodeId> {
    fn collect(node: &SyntaxNodeHandle, ids: &mut Vec<SyntaxNodeId>) {
        ids.push(node.id());
        for child in node.children() {
            collect(&child, ids);
        }
    }

    let mut ids = Vec::new();
    collect(root, &mut ids);
    ids
}

fn assert_ranges_for_kind(source: &ParsedSource, kind: GrammarKind, expected: &[SourceRange]) {
    let actual = nodes_with_kind(source, kind)
        .iter()
        .map(SyntaxNodeHandle::range)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "unexpected ranges for {kind:?}");
}

fn assert_exact_typed_round_trip<K: crate::attachment::ExactAstKind>(
    source: &ParsedSource,
    kind: GrammarKind,
) {
    let nodes = nodes_with_kind(source, kind);
    assert!(!nodes.is_empty(), "missing {kind:?} round-trip fixture");
    for syntax in nodes {
        let typed = syntax.cast::<K>().expect("exact typed cast");
        assert_eq!(typed.id(), syntax.id());
        let typed_syntax = typed.syntax();
        let rebound_syntax = source
            .bind_rowan(typed_syntax.rowan())
            .expect("typed Rowan node binds through exact snapshot");
        let rebound = rebound_syntax
            .cast::<K>()
            .expect("rebound Rowan node retains exact typed kind");
        assert_eq!(rebound, typed);
    }
}

fn typed_predicate_item(items: &[TypedItemNode]) -> crate::attachment::AstNode<PredicateItemKind> {
    items
        .iter()
        .find_map(|item| match item {
            TypedItemNode::Predicate(node) => Some(node.clone()),
            _ => None,
        })
        .expect("typed Predicate item")
}

fn typed_proof_item(items: &[TypedItemNode]) -> crate::attachment::AstNode<ProofItemKind> {
    items
        .iter()
        .find_map(|item| match item {
            TypedItemNode::Proof(node) => Some(node.clone()),
            _ => None,
        })
        .expect("typed Proof item")
}

fn private_slots(source: &super::ParsedSource) -> Vec<NonZeroU64> {
    source
        .attached()
        .nodes()
        .map(|node| node.id().slot())
        .collect()
}

fn only_node_with_kind(source: &ParsedSource, kind: GrammarKind) -> SyntaxNodeHandle {
    let mut nodes = source.attached().nodes().filter(|node| node.kind() == kind);
    let node = nodes
        .next()
        .unwrap_or_else(|| panic!("missing exact {kind:?} test node"));
    assert!(
        nodes.next().is_none(),
        "expected exactly one {kind:?} test node"
    );
    node
}
