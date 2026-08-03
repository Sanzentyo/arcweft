use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::expressions::ExpressionProjection;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::event::PendingSyntaxDiagnostic;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/resource-shadow").unwrap(),
        SourceName::path("resource-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn typed_resource_header_body_and_nested_values_are_lossless() {
    let source = r"/// Configured room image.
#[generated]
pub res @image.room room: std.presentation.Image {
    asset = @asset.bg.room
    bounds = std.presentation.ImageBounds {
        x = 0px
        y = 0px
        width = 1280px
        height = 720px
    }
    visible = true,
}
";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    for expected in [
        SyntaxKind::ResourceDeclarationItem,
        SyntaxKind::DocBlock,
        SyntaxKind::OuterAttribute,
        SyntaxKind::Visibility,
        SyntaxKind::NameDefinition,
        SyntaxKind::PathType,
        SyntaxKind::ResourceBody,
        SyntaxKind::ResourceFieldInitializer,
        SyntaxKind::EntityReferenceExpression,
        SyntaxKind::RecordExpression,
        SyntaxKind::RecordField,
        SyntaxKind::LiteralExpression,
    ] {
        assert!(
            entries.iter().any(|entry| entry.kind() == expected),
            "missing {expected:?}: kinds={:?}, diagnostics={:?}",
            entries
                .iter()
                .map(UnattachedGrammarEntry::kind)
                .collect::<Vec<_>>(),
            built.diagnostics(),
        );
    }
    assert_eq!(kind_count(entries, SyntaxKind::ResourceFieldInitializer), 3);
    assert_eq!(kind_count(entries, SyntaxKind::RecordField), 4);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn generic_head_is_structural_but_non_path_heads_are_rejected() {
    let source = concat!(
        "res weather: WeatherIcon<Heavy> { severity = 3 }\n",
        "res borrowed: &Image { visible = true }\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ResourceDeclarationItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::GenericApplicationType), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ReferenceType), 1);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.resource.invalid_type_head")
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_resource_headers_have_owned_recovery_ranges() {
    let source = concat!(
        "res : Image {}\n",
        "res @.room room Image {}\n",
        "res no_type: {}\n",
        "res no_body: Image\n",
        "proof next() = ()\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();
    let codes = built
        .diagnostics()
        .iter()
        .map(PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();

    assert_eq!(kind_count(entries, SyntaxKind::ResourceDeclarationItem), 4);
    assert_eq!(kind_count(entries, SyntaxKind::MissingName), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingType), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    for expected in [
        "syntax.resource.missing_name",
        "syntax.resource.relative_declaration_id",
        "syntax.resource.missing_colon",
        "syntax.resource.missing_type",
        "syntax.resource.missing_body",
    ] {
        assert!(codes.contains(&expected), "missing {expected}: {codes:?}");
    }
    assert!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code().starts_with("syntax.resource.missing_"))
            .all(|diagnostic| diagnostic.range().is_empty())
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_field_does_not_hide_later_fields_or_items() {
    let source = concat!(
        "res room: Image {\n",
        "    asset @asset.bg.room\n",
        "    visible = true\n",
        "    opacity =\n",
        "}\n",
        "proof next() = ()\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();
    let codes = built
        .diagnostics()
        .iter()
        .map(PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();

    assert_eq!(kind_count(entries, SyntaxKind::ResourceFieldInitializer), 3);
    assert_eq!(kind_count(entries, SyntaxKind::MissingExpression), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(codes.contains(&"syntax.resource.malformed_field"));
    assert!(codes.contains(&"syntax.resource.missing_initializer"));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn comparison_and_missing_initializer_keep_exact_sibling_boundaries() {
    let source = concat!(
        "res room: Image {\n",
        "    less = 1 < 2\n",
        "    opacity =\n",
        "    visible = true\n",
        "}\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ResourceFieldInitializer), 3);
    assert_eq!(kind_count(entries, SyntaxKind::BinaryExpression), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingExpression), 1);
    let missing = entries
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::MissingExpression)
        .expect("one projected missing Resource initializer");
    assert_eq!(
        missing
            .expression_projection()
            .expect("MissingExpression must use the shared expression owner")
            .projection(),
        &ExpressionProjection::Error
    );
    let expected_insertion = source.find("opacity =").unwrap() + "opacity =".len();
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.resource.missing_initializer")
        .expect("missing-initializer diagnostic");
    assert_eq!(
        diagnostic.range(),
        SourceRange::new(expected_insertion, expected_insertion)
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn removed_entity_scaffold_and_old_family_heads_do_not_create_resources() {
    let source = concat!("entity room: Image {}\n", "image room {}\n");
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();

    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::ResourceDeclarationItem),
        0
    );
    assert_eq!(built.green().to_string(), source);
}
