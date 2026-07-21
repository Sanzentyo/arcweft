use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

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
    let built = parse_shadow_document(&document(source)).unwrap();
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
    let built = parse_shadow_document(&document(source)).unwrap();
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
    let built = parse_shadow_document(&document(source)).unwrap();
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
