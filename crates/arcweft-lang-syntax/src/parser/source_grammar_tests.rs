use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_document;
use crate::attachment::{BlockKind, ExpressionStatementKind, SourceFileKind, TypedItemNode};
use crate::grammar::build::GrammarBuildError;
use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole};
use crate::incremental::{SyntaxDatabase, SyntaxLimit};

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/source-attached-grammar").unwrap(),
        SourceName::path("source-attached-grammar.arcw"),
        text,
    )
    .unwrap()
}

fn attached(text: &str) -> Arc<crate::attachment::SyntaxSnapshotData> {
    let document = Arc::new(document(text));
    let snapshot = SourceSnapshotId::initial(document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    Arc::clone(
        database
            .parse_initial(snapshot, document, crate::parser::ParseOptions::default())
            .unwrap()
            .attached(),
    )
}

fn attached_items(snapshot: &Arc<crate::attachment::SyntaxSnapshotData>) -> Vec<TypedItemNode> {
    snapshot
        .root_handle()
        .cast::<SourceFileKind>()
        .unwrap()
        .items()
        .unwrap()
}

#[test]
fn source_header_accepts_canonical_identity_forms_and_retains_exact_type() {
    for source in [
        "source events: Source<Event, Error> {}\n",
        "source @source.events: Source<Event, Error> {}\n",
        "source @<source.events>: Source<Event, Error> {}\n",
        "source @.events: Source<Event, Error> {}\n",
        "source @..events: Source<Event, Error> {}\n",
        "source @super.events: Source<Event, Error> {}\n",
        "source @source:.events: Source<Event, Error> {}\n",
        "source @source:..events: Source<Event, Error> {}\n",
        "pub source events: Source<Event, Error> {}\n",
        "source @. events: Source<Event, Error> {}\n",
        "source @source:. events: Source<Event, Error> {}\n",
    ] {
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert!(
            built.diagnostics().is_empty(),
            "{source:?}: {:?}",
            built.diagnostics()
        );
        assert!(!built.has_recovery(), "{source:?}");
        assert!(built.index().entries().iter().any(|entry| {
            entry.kind() == SyntaxKind::SourceItem && entry.role() == SyntaxRole::Element(0)
        }));
        assert!(built.index().entries().iter().any(|entry| {
            entry.kind() == SyntaxKind::GenericApplicationType && entry.role() == SyntaxRole::Type
        }));
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn source_declaration_member_limit_is_enforced_by_the_real_parser_transaction() {
    let maximum = SyntaxLimit::DeclarationMembers.maximum();
    let exact = source_with_members(maximum);
    let built = parse_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("exact Source declaration-member limit builds");
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::OnStatement)
            .count(),
        maximum - 4
    );

    let one_over = source_with_members(maximum + 1);
    assert!(matches!(
        parse_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
    assert!(
        parse_document(
            &document("source events: Source<Event, Error> {}\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok()
    );
}

fn source_with_members(count: usize) -> String {
    assert!(count >= 4);
    let mut source = String::from(concat!(
        "source events: Source<Event, Error> {\n",
        "    from events()\n",
        "    backpressure = latest\n",
        "    replay = none\n",
        "    privacy = private\n",
    ));
    for _ in 4..count {
        source.push_str("    on end => finish()\n");
    }
    source.push_str("}\n");
    source
}

#[test]
fn source_identity_markers_require_names_and_wrong_families_stay_typed() {
    for source in [
        "source @.: Source<Event, Error> {}\n",
        "source @source:.: Source<Event, Error> {}\n",
    ] {
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert!(built.index().entries().iter().any(|entry| {
            entry.kind() == SyntaxKind::MissingName && entry.role() == SyntaxRole::Name
        }));
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "syntax.source.missing_name")
        );
        assert_eq!(built.green().to_string(), source);
    }

    for source in [
        "source @flow:. events: Source<Event, Error> {}\n",
        "source @flow:.events: Source<Event, Error> {}\n",
        "source @<flow.events>: Source<Event, Error> {}\n",
    ] {
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert!(built.index().entries().iter().any(|entry| {
            entry.kind() == SyntaxKind::WrongFamilyReference
                && entry.role() == SyntaxRole::Reference(0)
        }));
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "syntax.source.wrong_family_id")
        );
        assert_eq!(built.green().to_string(), source);
    }

    let malformed = "source @..: Source<Event, Error> {}\n";
    let built =
        parse_document(&document(malformed), crate::parser::ParseOptions::default()).unwrap();
    assert!(built.index().entries().iter().any(|entry| {
        entry.kind() == SyntaxKind::ErrorNode && entry.role() == SyntaxRole::Recovery(0)
    }));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.source.malformed_id"),
        "{:?}",
        built.diagnostics()
    );
    assert_eq!(built.green().to_string(), malformed);

    let unclosed = concat!(
        "source @<source.events: Source<Event, Error> {}\n",
        "proof next() = ()\n",
    );
    let built =
        parse_document(&document(unclosed), crate::parser::ParseOptions::default()).unwrap();
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.source.malformed_id"),
        "{:?}",
        built.diagnostics()
    );
    assert!(built.index().entries().iter().any(|entry| {
        entry.kind() == SyntaxKind::ProofItem && entry.role() == SyntaxRole::Element(1)
    }));
    assert_eq!(built.green().to_string(), unclosed);
}

#[test]
fn source_contract_clauses_use_the_shared_typed_contract_owner() {
    let source = concat!(
        "source events: Source<Event, Error> {\n",
        "    requires can_open\n",
        "    ensures result.is_ok()\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert!(
        built.has_recovery(),
        "Source contracts stay typed but unsupported until Lang-01.3 removes Source"
    );
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::RequiresClause && entry.role() == SyntaxRole::ContractClause(0)
    }));
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::EnsuresClause && entry.role() == SyntaxRole::ContractClause(1)
    }));
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.role() == SyntaxRole::ContractOperand(0))
            .count(),
        2
    );
    assert!(!entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::ErrorStatement || entry.kind() == SyntaxKind::ErrorNode
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn source_unclosed_generic_recovers_its_body_and_following_declaration() {
    let source = concat!(
        "source broken: Source<Event, Error {\n",
        "    on end => yield ()\n",
        "}\n",
        "proof next() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::SourceItem && entry.role() == SyntaxRole::Element(0)
    }));
    assert!(!entries.iter().any(|entry| matches!(
        entry.kind(),
        SyntaxKind::GenericApplicationType | SyntaxKind::CloseAngleNode
    )));
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::ErrorType && entry.role() == SyntaxRole::Type
    }));
    assert!(
        entries
            .iter()
            .any(|entry| { entry.kind() == SyntaxKind::Block && entry.role() == SyntaxRole::Body })
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::OnStatement)
    );
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::ProofItem && entry.role() == SyntaxRole::Element(1)
    }));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.type.invalid")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn source_public_id_colon_partition_and_attached_accessors_are_exact() {
    let source = "pub source @source.events: Source<Event, Error> {}\n";
    let snapshot = attached(source);
    let item = attached_items(&snapshot).remove(0);
    assert_eq!(item.kind(), SyntaxKind::SourceItem);
    assert_eq!(item.syntax().tag(), AstTag::Item);
    let TypedItemNode::Source(source_item) = &item else {
        panic!("expected source item");
    };

    let public_id = source_item.public_id().unwrap().unwrap();
    assert_eq!(public_id.range(), SourceRange::new(11, 25));
    assert_eq!(
        public_id.syntax().rowan().text().to_string(),
        "@source.events"
    );
    assert!(source_item.name().unwrap().is_none());
    assert_eq!(
        source_item.source_type().unwrap().unwrap().kind(),
        SyntaxKind::GenericApplicationType
    );
    assert_eq!(
        source_item
            .header()
            .unwrap()
            .syntax()
            .rowan()
            .text()
            .to_string(),
        "pub source @source.events: Source<Event, Error>"
    );
    assert!(!source_item.body().unwrap().range().is_empty());
    assert!(!matches!(item, TypedItemNode::Proof(_)));
}

#[test]
fn source_body_uses_shared_typed_statement_expression_and_pattern_owners() {
    let source = concat!(
        "source @source.events: Source<Event, Error> {\n",
        "    from capture.events()\n",
        "    backpressure = bounded(capacity = 8, overflow = drop_oldest)\n",
        "    replay = hash_only\n",
        "    privacy = transient\n",
        "    on item event => yield event\n",
        "    on disconnected => signal.set(@signal.connected, false)\n",
        "    on error error => { log.warn(error) }\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert!(!built.has_recovery(), "{:?}", built.diagnostics());
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::ExpressionStatement && entry.role() == SyntaxRole::Statement(0)
    }));
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::CallExpression && entry.role() == SyntaxRole::Initializer
    }));
    for ordinal in 1..=3 {
        assert!(entries.iter().any(|entry| {
            entry.kind() == SyntaxKind::AssignmentStatement
                && entry.role() == SyntaxRole::Statement(ordinal)
        }));
    }
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::BindingPattern && entry.role() == SyntaxRole::Pattern
    }));
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::PathExpression && entry.role() == SyntaxRole::Condition
    }));
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::YieldStatement && entry.role() == SyntaxRole::Body
    }));
    assert!(
        entries
            .iter()
            .any(|entry| { entry.kind() == SyntaxKind::Block && entry.role() == SyntaxRole::Body })
    );
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::OmittedBlockTail && entry.role() == SyntaxRole::Tail
    }));
    assert_eq!(built.green().to_string(), source);

    let snapshot = attached(source);
    let item = attached_items(&snapshot).remove(0);
    let TypedItemNode::Source(item) = item else {
        panic!("expected source item");
    };
    let block = item
        .syntax()
        .child(SyntaxRole::Body)
        .unwrap()
        .cast::<BlockKind>()
        .unwrap();
    let statements = block.statements().unwrap();
    assert_eq!(statements.len(), 7);
    let from = statements[0].cast::<ExpressionStatementKind>().unwrap();
    assert_eq!(
        from.source_initializer().unwrap().kind(),
        SyntaxKind::CallExpression
    );
}

#[test]
fn source_handler_family_matrix_uses_shared_pattern_condition_and_body_owners() {
    let source = concat!(
        "source events: Source<Event, Error> {\n",
        "    on item item => yield item\n",
        "    on error error => yield error\n",
        "    on progress progress => yield progress\n",
        "    on disconnected => reconnect()\n",
        "    on permission_revoked => revoke()\n",
        "    on end => { finish() }\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert!(!built.has_recovery(), "{:?}", built.diagnostics());
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::OnStatement)
            .count(),
        6
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.role() == SyntaxRole::Pattern)
            .count(),
        3
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.role() == SyntaxRole::Condition)
            .count(),
        3
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn source_handler_recovery_preserves_later_handlers_and_items() {
    let source = concat!(
        "source events: Source<Event, Error> {\n",
        "    on item => yield ()\n",
        "    on error error\n",
        "    on progress progress =>\n",
        "    on disconnected => { reconnect()\n",
        "    on end => finish()\n",
        "}\n",
        "proof next() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::MissingPattern && entry.role() == SyntaxRole::Pattern
    }));
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.source.missing_handler_arrow")
            .count(),
        1
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.source.missing_handler_body")
            .count(),
        1
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.source.missing_handler_close"),
        "{:?}",
        built.diagnostics()
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::OnStatement)
            .count(),
        5
    );
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::ProofItem && entry.role() == SyntaxRole::Element(1)
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn source_recovery_keeps_missing_components_and_following_items_exact() {
    let source = concat!(
        "source : {\n",
        "    on item value\n",
        "    on end =>\n",
        "proof next() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    for expected in [
        SyntaxKind::MissingName,
        SyntaxKind::MissingType,
        SyntaxKind::MissingBody,
        SyntaxKind::CloseBraceNode,
    ] {
        assert!(
            entries.iter().any(|entry| entry.kind() == expected),
            "missing {expected:?}: {:?}",
            built.diagnostics()
        );
    }
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::ProofItem && entry.role() == SyntaxRole::Element(1)
    }));
    for code in [
        "syntax.source.missing_name",
        "syntax.source.missing_type",
        "syntax.source.missing_handler_arrow",
        "syntax.source.missing_handler_body",
        "syntax.source.missing_block_close",
    ] {
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == code),
            "missing {code}: {:?}",
            built.diagnostics()
        );
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn source_missing_body_and_noncanonical_function_shape_are_ordinary_recovery() {
    let missing_body = "source events: Source<Event, Error>\nproof next() = ()\n";
    let built = parse_document(
        &document(missing_body),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert!(built.index().entries().iter().any(|entry| {
        entry.kind() == SyntaxKind::MissingBody && entry.role() == SyntaxRole::Body
    }));
    assert!(built.index().entries().iter().any(|entry| {
        entry.kind() == SyntaxKind::ProofItem && entry.role() == SyntaxRole::Element(1)
    }));
    assert_eq!(built.green().to_string(), missing_body);

    let function_like = "source events() -> Source<Event, Error> {}\n";
    let built = parse_document(
        &document(function_like),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert!(built.has_recovery());
    assert!(built.index().entries().iter().any(|entry| {
        entry.kind() == SyntaxKind::SourceItem && entry.role() == SyntaxRole::Element(0)
    }));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.source.missing_colon")
    );
    assert_eq!(built.green().to_string(), function_like);

    let malformed_id = "source @ : Source<Event, Error> {}\n";
    let built = parse_document(
        &document(malformed_id),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert!(built.has_recovery());
    assert!(built.index().entries().iter().any(|entry| {
        entry.kind() == SyntaxKind::SourceItem && entry.role() == SyntaxRole::Element(0)
    }));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.source.malformed_id"),
        "unexpected diagnostics: {:?}",
        built.diagnostics()
    );
    assert_eq!(built.green().to_string(), malformed_id);
}
