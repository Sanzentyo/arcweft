use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};
use std::sync::Arc;

use super::document::parse_document;
use crate::attachment::{AttachedEnumVariantPayload, AttachedTypeFamily, TypedItemNode};
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
    assert!(matches!(
        choice.body().variants()[0].payload(),
        AttachedEnumVariantPayload::Unit
    ));
    assert!(matches!(
        choice.body().variants()[1].payload(),
        AttachedEnumVariantPayload::Tuple(_)
    ));
}

#[test]
fn enum_variant_payloads_preserve_unit_tuple_and_ordered_record_shapes() {
    let source = concat!(
        "enum GameEvent {\n",
        "    ChoiceSelected { id: i32, label: String },\n",
        "    EmptyRecord {},\n",
        "    StartGame,\n",
        "    Legacy (i32, String),\n",
        "}\n",
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
    let [TypedItemNode::Enum(game_event)] = items.as_slice() else {
        panic!("expected one enum declaration")
    };
    let declaration = game_event.semantics().unwrap();
    let variants = declaration.body().variants();
    assert_eq!(variants.len(), 4);

    let AttachedEnumVariantPayload::Record(record) = variants[0].payload() else {
        panic!("ChoiceSelected has an inline record payload")
    };
    let record_payload = "{ id: i32, label: String }";
    let record_payload_start = source.find(record_payload).unwrap();
    assert_eq!(
        record.source_span().range(),
        SourceRange::new(
            record_payload_start,
            record_payload_start + record_payload.len(),
        )
    );
    assert_eq!(record.fields().len(), 2);
    assert_eq!(
        record
            .fields()
            .iter()
            .map(|field| field.name().value().unwrap().as_str())
            .collect::<Vec<_>>(),
        ["id", "label"]
    );
    assert_eq!(record.fields()[0].syntax().source_text(), "id: i32,");
    assert_eq!(record.fields()[0].name().syntax().source_text(), "id");
    assert_eq!(record.fields()[1].syntax().source_text(), "label: String ");

    let AttachedEnumVariantPayload::Record(empty_record) = variants[1].payload() else {
        panic!("EmptyRecord retains Record distinctly from Unit")
    };
    assert!(empty_record.fields().is_empty());
    let empty_record_start = source.find("{}").unwrap();
    assert_eq!(
        empty_record.source_span().range(),
        SourceRange::new(empty_record_start, empty_record_start + 2)
    );
    assert!(matches!(
        variants[2].payload(),
        AttachedEnumVariantPayload::Unit
    ));
    assert!(matches!(
        variants[3].payload(),
        AttachedEnumVariantPayload::Tuple(_)
    ));
}

#[test]
fn malformed_enum_record_fields_recover_within_the_variant_body() {
    let source = concat!(
        "enum Broken { Record { id i32 }, Good }\n",
        "proof following() = ()\n",
    );
    let document = Arc::new(document(source));
    let snapshot = SourceSnapshotId::initial(document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    let parsed = database
        .parse_initial(snapshot, document, crate::parser::ParseOptions::default())
        .unwrap();

    assert_eq!(parsed.items().unwrap().len(), 2);
    let items = parsed.items().unwrap();
    let [TypedItemNode::Enum(broken), TypedItemNode::Proof(_)] = items.as_slice() else {
        panic!("malformed record payload must not consume the following item")
    };
    let declaration = broken.semantics().unwrap();
    let [record_variant, good_variant] = declaration.body().variants() else {
        panic!("both enum variants survive field recovery")
    };
    assert!(record_variant.has_recovery());
    assert_eq!(record_variant.syntax().source_text(), "Record { id i32 },");
    let AttachedEnumVariantPayload::Record(record) = record_variant.payload() else {
        panic!("malformed field remains inside a record payload")
    };
    assert_eq!(record.fields()[0].syntax().source_text(), "id i32 ");
    let missing_type = source.find("i32").unwrap();
    assert_eq!(good_variant.name().value().unwrap().as_str(), "Good");
    assert!(parsed.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.nominal.missing_field_type"
            && diagnostic.primary().range() == SourceRange::new(missing_type, missing_type)
    }));
}

#[test]
fn missing_enum_record_close_recovers_before_the_following_item() {
    let source = concat!(
        "enum Broken { Record { id: i32\n",
        "proof following() = ()\n",
    );
    let document = Arc::new(document(source));
    let snapshot = SourceSnapshotId::initial(document.display_name().clone());
    let mut database = SyntaxDatabase::try_new().unwrap();
    let parsed = database
        .parse_initial(snapshot, document, crate::parser::ParseOptions::default())
        .unwrap();

    let items = parsed.items().unwrap();
    let [TypedItemNode::Enum(broken), TypedItemNode::Proof(_)] = items.as_slice() else {
        panic!("missing record close must not consume the following item")
    };
    let declaration = broken.semantics().unwrap();
    let [record_variant] = declaration.body().variants() else {
        panic!("one recovered enum variant")
    };
    let AttachedEnumVariantPayload::Record(record) = record_variant.payload() else {
        panic!("record payload remains attached with its missing close")
    };
    assert!(record.has_recovery());
    let following = source.find("\nproof following").unwrap();
    assert!(
        parsed.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == "syntax.nominal.missing_variant_record_close"
                && diagnostic.primary().range() == SourceRange::new(following, following)
        }),
        "{:?}",
        parsed.diagnostics()
    );
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
