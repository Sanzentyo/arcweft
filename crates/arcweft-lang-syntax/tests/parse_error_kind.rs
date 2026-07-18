use std::collections::BTreeSet;

use arcweft_lang_syntax::parser::recovery::ParseErrorKind;

#[test]
fn parse_error_kind_registry_is_complete_unique_and_stable() {
    let expected = [
        (ParseErrorKind::Generic, "syntax.parse", "Parse error"),
        (
            ParseErrorKind::AssertionUnknownMode,
            "syntax.assert.unknown_mode",
            "Unknown assertion mode",
        ),
        (
            ParseErrorKind::AssertionInvalidArgument,
            "syntax.assert.invalid_argument",
            "Invalid assertion argument",
        ),
        (
            ParseErrorKind::AssertionUnclosedArguments,
            "syntax.assert.unclosed_arguments",
            "Unclosed assertion argument list",
        ),
        (
            ParseErrorKind::AssertionEmptyConditions,
            "syntax.assert.empty_conditions",
            "Empty assertion condition list",
        ),
        (
            ParseErrorKind::AssertionTooManyConditions,
            "syntax.assert.too_many_conditions",
            "Too many assertion conditions",
        ),
        (
            ParseErrorKind::EntryMissingKind,
            "syntax.entry.missing_kind",
            "Missing entry kind",
        ),
        (
            ParseErrorKind::EntryMissingId,
            "syntax.entry.missing_id",
            "Missing entry public ID",
        ),
        (
            ParseErrorKind::EntryIdFamily,
            "syntax.entry.id_family",
            "Invalid entry public ID family",
        ),
        (
            ParseErrorKind::EntryTrailingHead,
            "syntax.entry.trailing_head",
            "Trailing syntax in entry declaration head",
        ),
        (
            ParseErrorKind::EntryDuplicateRole,
            "syntax.entry.duplicate_role",
            "Duplicate entry role",
        ),
        (
            ParseErrorKind::EntryIncompatibleRole,
            "syntax.entry.incompatible_role",
            "Entry role is incompatible with its kind",
        ),
        (
            ParseErrorKind::EntryDuplicateGoto,
            "syntax.entry.duplicate_goto",
            "Duplicate entry initial target",
        ),
        (
            ParseErrorKind::EntryIncompatibleGoto,
            "syntax.entry.incompatible_goto",
            "Entry initial target is incompatible with its kind",
        ),
        (
            ParseErrorKind::EntryIncompatibleRoute,
            "syntax.entry.incompatible_route",
            "Entry route is incompatible with its kind",
        ),
        (
            ParseErrorKind::EntryMissingRole,
            "syntax.entry.missing_role",
            "Missing required entry role",
        ),
        (
            ParseErrorKind::EntryMissingGoto,
            "syntax.entry.missing_goto",
            "Missing entry initial target",
        ),
        (
            ParseErrorKind::EntryRoleBinding,
            "syntax.entry.role_binding",
            "Malformed entry role binding",
        ),
        (
            ParseErrorKind::EntryRoleValue,
            "syntax.entry.role_value",
            "Missing entry role value",
        ),
        (
            ParseErrorKind::EntryRolePath,
            "syntax.entry.role_path",
            "Invalid entry role symbol path",
        ),
        (
            ParseErrorKind::NominalInvalidGenericParameters,
            "syntax.nominal.invalid_generic_parameters",
            "Invalid nominal generic parameter list",
        ),
        (
            ParseErrorKind::StyleInlineSelectorNotSupported,
            "style::inline_selector_not_supported",
            "Selector rule in inline Style",
        ),
        (
            ParseErrorKind::StyleMalformedSelector,
            "style::malformed_selector",
            "Malformed Style selector",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedOpenParen,
            "syntax.parse.style_environment.expected_open_paren",
            "Expected environment opening parenthesis",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedField,
            "syntax.parse.style_environment.expected_field",
            "Expected environment field",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedComparison,
            "syntax.parse.style_environment.expected_comparison",
            "Expected environment comparison",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedValue,
            "syntax.parse.style_environment.expected_value",
            "Expected environment value",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedCommaOrCloseParen,
            "syntax.parse.style_environment.expected_comma_or_close_paren",
            "Expected environment clause separator",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedOpenBrace,
            "syntax.parse.style_environment.expected_open_brace",
            "Expected environment body opening brace",
        ),
        (
            ParseErrorKind::StyleEnvironmentUnterminatedCondition,
            "syntax.parse.style_environment.unterminated_condition",
            "Unterminated environment condition",
        ),
        (
            ParseErrorKind::StyleEnvironmentUnsupportedValue,
            "syntax.parse.style_environment.unsupported_value",
            "Unsupported environment value",
        ),
        (
            ParseErrorKind::StyleEnvironmentTokenNotAllowed,
            "syntax.parse.style_environment.token_not_allowed",
            "Style token in environment body",
        ),
        (
            ParseErrorKind::ViewExportPartMisplaced,
            "view::export_part_misplaced",
            "Misplaced View part export",
        ),
        (
            ParseErrorKind::ViewDuplicatePartModifier,
            "view::duplicate_part_modifier",
            "Duplicate View part modifier",
        ),
        (
            ParseErrorKind::ViewExportPartMissingPart,
            "view::export_part_missing_part",
            "Missing `part` keyword in View export",
        ),
        (
            ParseErrorKind::ViewExportPartDuplicateAs,
            "view::export_part_duplicate_as",
            "Duplicate `as` keyword in View part export",
        ),
        (
            ParseErrorKind::ViewExportPartTrailingSyntax,
            "view::export_part_trailing_syntax",
            "Trailing syntax in View part export",
        ),
        (
            ParseErrorKind::ViewExportPartMissingLocal,
            "view::export_part_missing_local",
            "Missing local View part name",
        ),
        (
            ParseErrorKind::ViewExportPartInvalidLocalName,
            "view::export_part_invalid_local_name",
            "Invalid local View part name",
        ),
        (
            ParseErrorKind::ViewExportPartMissingAs,
            "view::export_part_missing_as",
            "Missing `as` keyword in View part export",
        ),
        (
            ParseErrorKind::ViewExportPartMissingPublic,
            "view::export_part_missing_public",
            "Missing public View part name",
        ),
        (
            ParseErrorKind::ViewExportPartInvalidPublicName,
            "view::export_part_invalid_public_name",
            "Invalid public View part name",
        ),
        (
            ParseErrorKind::ViewPartMissingName,
            "view::part_missing_name",
            "Missing View part modifier name",
        ),
        (
            ParseErrorKind::ViewPartTrailingSyntax,
            "view::part_trailing_syntax",
            "Trailing syntax in View part modifier",
        ),
        (
            ParseErrorKind::ViewPartInvalidLocalName,
            "view::part_invalid_local_name",
            "Invalid View part modifier name",
        ),
    ];

    assert_eq!(ParseErrorKind::ALL, expected.map(|entry| entry.0));
    assert_eq!(ParseErrorKind::ALL.len(), 45);
    assert_eq!(
        ParseErrorKind::ALL
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        ParseErrorKind::ALL.len()
    );
    assert_eq!(
        ParseErrorKind::ALL
            .iter()
            .map(|kind| kind.code())
            .collect::<BTreeSet<_>>()
            .len(),
        ParseErrorKind::ALL.len()
    );
    for (kind, code, label) in expected {
        assert_eq!(kind.code(), code);
        assert_eq!(kind.label(), label);
        assert!(!label.is_empty());
    }
}
