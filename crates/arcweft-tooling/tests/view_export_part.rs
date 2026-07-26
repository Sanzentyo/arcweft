mod support;

use arcweft_tooling::{edit::apply_text_edits, model::FormatOptions};
use support::format_fixture;

#[test]
fn canonicalizes_valid_export_and_local_part_without_reordering() {
    let source = "pub view Card() {\n    export   part  body   as  content\n    // preserve this declaration comment\n    export part header.title as card.heading\n\n    Column {\n        Text(\"Body\")\n            .part( body )\n        Text(\"Title\").part( header.title )\n    }\n}\n";
    let expected = "pub view Card() {\n    export part body as content\n    // preserve this declaration comment\n    export part header.title as card.heading\n\n    Column {\n        Text(\"Body\")\n            .part(body)\n        Text(\"Title\").part(header.title)\n    }\n}\n";

    let report = format_fixture(source, FormatOptions::default()).expect("format report");
    assert_eq!(report.output, expected);
    assert_eq!(apply_text_edits(source, &report.edits).unwrap(), expected);
    assert!(report.diagnostics.is_empty());

    let repeated = format_fixture(&report.output, FormatOptions::default()).expect("second format");
    assert!(!repeated.changed);
    assert!(repeated.edits.is_empty());
}

#[test]
fn malformed_export_is_untouched_while_valid_surrounding_syntax_is_formatted() {
    let source = "pub view Card() {\n    export part body as content\n    export part as broken\n    Text(\"Body\").part( body )\n}\n";

    let report = format_fixture(source, FormatOptions::default()).expect("format report");
    assert!(report.output.contains("    export part as broken\n"));
    assert!(report.output.contains("Text(\"Body\").part(body)"));
    assert!(!report.diagnostics.is_empty());

    let repeated = format_fixture(&report.output, FormatOptions::default()).expect("second format");
    assert_eq!(repeated.output, report.output);
}
