use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

use super::document::parse_document;
use crate::attachment::{AttachedTypeFamily, TypedItemNode};
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;
use crate::incremental::SyntaxDatabase;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/nominal-type-shadow").unwrap(),
        SourceName::path("nominal-type-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn nominal_type_families_emit_typed_fields_payloads_targets_and_constraints() {
    let source = r"#[derive(Clone, Debug, Format, Serialize, Eq)]
pub enum GameEvent<T> where T: Format {
    StartGame,
    ChoiceSelected Ref<ChoiceOption>,
    Detailed Result<T, ParseError>,
}

pub struct SettingsInput {
    text_speed: f32,
    master_volume: f32,
}

pub type PlayerName<T> = Result<T, ParseError>
where T: Format
where ParseError: Error
";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EnumItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::StructItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::RecordField), 5);
    assert_eq!(kind_count(entries, SyntaxKind::WherePredicate), 3);
    assert_eq!(kind_count(entries, SyntaxKind::GenericApplicationType), 3);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::OuterAttribute)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::PathType)
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_fields_and_missing_alias_target_recover_before_following_items() {
    let source = concat!(
        "struct Broken {\n",
        "    value Int\n",
        "}\n",
        "type Missing =\n",
        "proof next() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::StructItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::MissingType)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::ErrorNode)
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.nominal.missing_field_type" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_enum_payload_and_body_closes_do_not_consume_the_next_declaration() {
    let source = concat!(
        "enum Broken {\n",
        "    Detailed Result<String\n",
        "proof next() = ()\n",
    );
    let next = source.find("proof next").unwrap();
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EnumItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::CloseBraceNode)
    );
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.nominal.missing_body_close"
            && diagnostic.range().start() == next
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn nominal_declarations_attach_their_exact_bodies_and_members() {
    let source = concat!(
        "type Alias<T> = Result<T, Error> where T: Format\n",
        "struct Record<T> where T: Format { value: T }\n",
        "enum Choice<T> where T: Format { Empty, Value T }\n",
    );
    let document = Arc::new(document(source));
    let snapshot = SourceSnapshotId::initial(document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    let parsed = database
        .parse_initial(snapshot, document, crate::parser::ParseOptions::default())
        .unwrap();
    let items = parsed.items().unwrap();

    let [
        TypedItemNode::TypeAlias(alias),
        TypedItemNode::Struct(record),
        TypedItemNode::Enum(choice),
    ] = items.as_slice()
    else {
        panic!("expected the three nominal declaration families");
    };
    let alias = alias.semantics().unwrap();
    assert_eq!(alias.generics().unwrap().parameters().len(), 1);
    assert_eq!(alias.where_clauses()[0].predicates().len(), 1);

    let record = record.semantics().unwrap();
    assert_eq!(record.body().fields().len(), 1);
    assert_eq!(record.where_clauses()[0].predicates().len(), 1);

    let choice = choice.semantics().unwrap();
    assert_eq!(choice.body().variants().len(), 2);
    assert!(choice.body().variants()[0].payload().is_none());
    assert!(choice.body().variants()[1].payload().is_some());
}

#[test]
fn nominal_member_names_accept_keyword_spellings_in_their_unambiguous_namespace() {
    let source = concat!(
        "struct DialogueInput { character: Character, content: DialogueContent }\n",
        "enum Control { return, await String }\n",
    );
    let document = Arc::new(document(source));
    let snapshot = SourceSnapshotId::initial(document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    let parsed = database
        .parse_initial(snapshot, document, crate::parser::ParseOptions::default())
        .unwrap();

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let items = parsed.items().unwrap();
    let [TypedItemNode::Struct(record), TypedItemNode::Enum(choice)] = items.as_slice() else {
        panic!("expected struct and enum declarations");
    };
    let record = record.semantics().unwrap();
    assert_eq!(
        record
            .body()
            .fields()
            .iter()
            .map(|field| field.name().value().unwrap().as_str())
            .collect::<Vec<_>>(),
        ["character", "content"]
    );
    let choice = choice.semantics().unwrap();
    assert_eq!(
        choice
            .body()
            .variants()
            .iter()
            .map(|variant| variant.name().value().unwrap().as_str())
            .collect::<Vec<_>>(),
        ["return", "await"]
    );
}

#[test]
fn nominal_where_predicates_attach_missing_bounds_as_typed_recovery() {
    let source = concat!(
        "type Empty = Value where T:\n",
        "type Trailing = Value where T: Bound +\n",
    );
    let document = Arc::new(document(source));
    let snapshot = SourceSnapshotId::initial(document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    let parsed = database
        .parse_initial(snapshot, document, crate::parser::ParseOptions::default())
        .unwrap();
    let items = parsed.items().unwrap();

    let [
        TypedItemNode::TypeAlias(empty),
        TypedItemNode::TypeAlias(trailing),
    ] = items.as_slice()
    else {
        panic!("expected the two type aliases");
    };
    let empty = empty.semantics().unwrap();
    let empty_bounds = empty.where_clauses()[0].predicates()[0].bounds();
    assert_eq!(empty_bounds.len(), 1);
    assert_eq!(empty_bounds[0].family(), AttachedTypeFamily::Recovery);

    let trailing = trailing.semantics().unwrap();
    let trailing_bounds = trailing.where_clauses()[0].predicates()[0].bounds();
    assert_eq!(trailing_bounds.len(), 2);
    assert_ne!(trailing_bounds[0].family(), AttachedTypeFamily::Recovery);
    assert_eq!(trailing_bounds[1].family(), AttachedTypeFamily::Recovery);
}
