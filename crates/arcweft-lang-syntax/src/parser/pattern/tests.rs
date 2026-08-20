use super::*;
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::SyntaxEvent;
use crate::literal::{
    SyntaxCharacterIssue, SyntaxDecimalComponentIssue, SyntaxDecimalIssue, SyntaxDurationIssue,
    SyntaxIntegerIssue, SyntaxLiteralFamily, SyntaxLiteralIssue, SyntaxStringIssue,
    SyntaxUnitNumberIssue,
};
use crate::parser::lexer::DocumentLexer;
use crate::patterns::{
    PatternBindingSyntax, PatternOrBindingIssue, PatternPathSegment, PatternSequenceRestIssue,
    PatternUnqualifiedVariantForm, PatternVariantHead, PatternVariantHeadSyntax,
    PatternVariantPayloadSyntax, VariantPatternPayloadPart,
};

#[test]
fn contextual_choice_keyword_is_an_ordinary_binding_pattern() {
    let events = pattern_events("choice");
    let binding = projection(&events, SyntaxKind::BindingPattern);
    assert!(matches!(
        binding
            .authored()
            .value_at(binding.path())
            .expect("binding value")
            .kind(),
        PatternSyntaxKind::Binding(PatternBindingSyntax::Resolved(name))
            if name.as_str() == "choice"
    ));
}

#[test]
fn missing_variant_payload_close_is_a_required_parser_owned_insertion() {
    let source = ".Some(value";
    let events = pattern_events(source);
    let projection = projection(&events, SyntaxKind::VariantPattern);
    let close = projection
        .authored()
        .source()
        .component_at(
            projection.path(),
            PatternComponentRole::VariantPayload(VariantPatternPayloadPart::CloseDelimiter),
        )
        .expect("present-or-insertion close component");

    assert_eq!(*close, SourceRange::new(source.len(), source.len()));
    let PatternSyntaxKind::Variant(variant) = projection
        .authored()
        .value_at(projection.path())
        .expect("variant value")
        .kind()
    else {
        panic!("expected variant family");
    };
    assert!(matches!(
        variant.payload(),
        PatternVariantPayloadSyntax::Recovered {
            value: Some(_),
            issue: PatternVariantPayloadIssue::MissingCloseDelimiter,
        }
    ));
    assert!(
        events.iter().any(
            |event| matches!(event, SyntaxEvent::MissingToken { at, .. } if *at == source.len())
        )
    );
}

#[test]
fn missing_variant_name_owns_a_zero_width_required_component() {
    for source in [".", "Choice."] {
        let events = pattern_events(source);
        let variant = projection(&events, SyntaxKind::VariantPattern);
        assert_eq!(
            *variant
                .authored()
                .source()
                .component_at(variant.path(), PatternComponentRole::VariantName)
                .expect("required variant-name insertion"),
            SourceRange::new(source.len(), source.len())
        );
    }
}

#[test]
fn bare_variant_grammar_projects_one_generic_expected_type_head() {
    for source in ["Some(value)", "None", "Ok(value)", "Err(error)"] {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::VariantPattern);
        let PatternSyntaxKind::Variant(variant) = projection
            .authored()
            .value_at(projection.path())
            .expect("variant value")
            .kind()
        else {
            panic!("expected variant family");
        };
        assert!(matches!(
            variant.head(),
            PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(
                PatternUnqualifiedVariantForm::BareExpectedType
            ))
        ));
    }

    assert!(pattern_events("Ready").iter().any(|event| matches!(
        event,
        SyntaxEvent::StartNode {
            kind: SyntaxKind::BindingPattern,
            ..
        }
    )));
}

#[test]
fn project_paths_retain_external_capable_segments() {
    let events = pattern_events("vendor-pack::model::Choice.Ready");
    let projection = projection(&events, SyntaxKind::VariantPattern);
    let PatternSyntaxKind::Variant(variant) = projection
        .authored()
        .value_at(projection.path())
        .expect("variant value")
        .kind()
    else {
        panic!("expected variant family");
    };
    let PatternVariantHeadSyntax::Resolved(PatternVariantHead::Qualified(path)) = variant.head()
    else {
        panic!("expected qualified head");
    };
    assert!(matches!(
        path.segments(),
        [PatternPathSegment::ProjectSymbol(project),
         PatternPathSegment::Identifier(module),
         PatternPathSegment::Identifier(owner)]
            if project.as_str() == "vendor-pack"
                && module.as_str() == "model"
                && owner.as_str() == "Choice"
    ));
}

#[test]
fn bindings_reject_trailing_tokens_without_fabricating_a_local() {
    for source in ["name extra", "mut name extra", "name extra: Int"] {
        let events = pattern_events(source);
        let projection = events
            .iter()
            .find_map(|event| match event {
                SyntaxEvent::StartNode {
                    projection: crate::grammar::event::PendingStartProjection::Pattern(projection),
                    ..
                } if projection.path().steps().is_empty() => Some(projection),
                _ => None,
            })
            .expect("root Pattern projection");
        let value = projection
            .authored()
            .value_at(projection.path())
            .expect("root Pattern value");
        assert!(value.state().issues().iter().any(|issue| matches!(
            issue,
            PatternRecoveryIssue::Binding(PatternBindingIssue::UnexpectedTrailingInput {
                token_count: 2
            })
        )));
        assert!(
            projection.authored().binding_sites()[0]
                .binding()
                .name()
                .is_none()
        );
    }
}

#[test]
fn sequence_rest_preserves_absent_unbound_binding_and_typed_recovery() {
    let cases = [
        ("[value]", "absent"),
        ("[value, ..]", "unbound"),
        ("[value, ..tail]", "binding"),
        ("[value, ..tail extra]", "invalid"),
        ("[value, ..tail, ..other, ..third]", "multiple"),
    ];
    for (source, expected) in cases {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::SequencePattern);
        let PatternSyntaxKind::BracketSequence(sequence) = projection
            .authored()
            .value_at(projection.path())
            .expect("sequence value")
            .kind()
        else {
            panic!("expected sequence family");
        };
        match expected {
            "absent" => assert!(matches!(sequence.rest(), PatternSequenceRestSyntax::Absent)),
            "unbound" => assert!(matches!(
                sequence.rest(),
                PatternSequenceRestSyntax::Unbound
            )),
            "binding" => assert!(matches!(
                sequence.rest(),
                PatternSequenceRestSyntax::Binding(_)
            )),
            "invalid" => assert!(sequence.rest().issues().iter().any(|issue| matches!(
                issue,
                PatternSequenceRestIssue::InvalidBinding(
                    PatternBindingIssue::UnexpectedTrailingInput { token_count: 2 }
                )
            ))),
            "multiple" => {
                let ordinals = sequence
                    .rest()
                    .issues()
                    .iter()
                    .filter_map(|issue| match issue {
                        PatternSequenceRestIssue::MultipleRest { ordinal } => Some(*ordinal),
                        PatternSequenceRestIssue::InvalidBinding(_) => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(ordinals, [1, 2]);
                assert_eq!(
                    projection
                        .authored()
                        .binding_sites()
                        .iter()
                        .filter(|site| matches!(
                            site.kind(),
                            crate::patterns::PatternBindingSiteKind::SequenceRest
                        ))
                        .count(),
                    1
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn or_patterns_reuse_first_alternative_binding_ordinals_and_poison_mismatch() {
    let matching_events = pattern_events("(left, right) | (left, right)");
    let matching = projection(&matching_events, SyntaxKind::OrPattern);
    assert_eq!(
        matching
            .authored()
            .binding_sites()
            .iter()
            .map(crate::patterns::PatternBindingSite::ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 0, 1]
    );
    assert!(
        matching
            .authored()
            .value_at(matching.path())
            .expect("or value")
            .state()
            .is_valid()
    );

    let mismatched_events = pattern_events("left | (left, extra)");
    let mismatched = projection(&mismatched_events, SyntaxKind::OrPattern);
    assert!(
        mismatched
            .authored()
            .value_at(mismatched.path())
            .expect("or value")
            .state()
            .issues()
            .iter()
            .any(|issue| matches!(
                issue,
                PatternRecoveryIssue::OrBindings(PatternOrBindingIssue::CountMismatch {
                    alternative: 1,
                    expected: 1,
                    actual: 2,
                })
            ))
    );
    assert_eq!(
        mismatched
            .authored()
            .binding_sites()
            .iter()
            .map(crate::patterns::PatternBindingSite::ordinal)
            .collect::<Vec<_>>(),
        [0, 0]
    );
}

#[test]
fn malformed_literals_remain_literal_family_poison() {
    for (source, family, expected) in [
        ("\"\\q\"", SyntaxLiteralFamily::String, "escape"),
        (
            "\"unterminated",
            SyntaxLiteralFamily::String,
            "unterminated",
        ),
        ("\"\"c", SyntaxLiteralFamily::Character, "empty-character"),
        (
            "\"ab\"c",
            SyntaxLiteralFamily::Character,
            "multiple-character-scalars",
        ),
        (
            "\"\\u{41}\\u{42}\"c",
            SyntaxLiteralFamily::Character,
            "multiple-character-scalars",
        ),
        ("\"\\q\"c", SyntaxLiteralFamily::Character, "escape"),
        ("0x", SyntaxLiteralFamily::Integer, "digits"),
        ("1._0", SyntaxLiteralFamily::Decimal, "separator"),
        ("1__px", SyntaxLiteralFamily::UnitNumber, "separator"),
        ("1__ms", SyntaxLiteralFamily::Duration, "separator"),
        ("970milli", SyntaxLiteralFamily::Integer, "digits"),
        ("42unknown", SyntaxLiteralFamily::Integer, "digits"),
    ] {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::LiteralPattern);
        let PatternSyntaxKind::Literal(literal) = projection
            .authored()
            .value_at(projection.path())
            .expect("literal value")
            .kind()
        else {
            panic!("expected literal family");
        };
        assert_eq!(literal.family(), family, "{source}");
        assert!(
            matches!(
                (literal.value().issue(), expected),
                (
                    Some(
                        SyntaxLiteralIssue::String(SyntaxStringIssue::InvalidEscape { .. })
                            | SyntaxLiteralIssue::Character(
                                SyntaxCharacterIssue::InvalidEscape { .. }
                            )
                    ),
                    "escape"
                ) | (
                    Some(SyntaxLiteralIssue::String(
                        SyntaxStringIssue::Unterminated { .. }
                    )),
                    "unterminated"
                ) | (
                    Some(SyntaxLiteralIssue::Character(
                        SyntaxCharacterIssue::Empty { .. }
                    )),
                    "empty-character"
                ) | (
                    Some(SyntaxLiteralIssue::Character(
                        SyntaxCharacterIssue::MultipleScalars { .. }
                    )),
                    "multiple-character-scalars"
                ) | (
                    Some(SyntaxLiteralIssue::Integer(
                        SyntaxIntegerIssue::MissingDigits { .. }
                            | SyntaxIntegerIssue::InvalidDigits { .. }
                    )),
                    "digits"
                ) | (
                    Some(
                        SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                            SyntaxDecimalComponentIssue::InvalidSeparator { .. }
                        )) | SyntaxLiteralIssue::UnitNumber(SyntaxUnitNumberIssue::Decimal(
                            SyntaxDecimalComponentIssue::InvalidSeparator { .. }
                        )) | SyntaxLiteralIssue::Duration(SyntaxDurationIssue::Decimal(
                            SyntaxDecimalComponentIssue::InvalidSeparator { .. }
                        ))
                    ),
                    "separator"
                )
            ),
            "{source}"
        );
    }
}

#[test]
fn unknown_integer_suffix_is_invalid_integer_content_without_unit_admission() {
    let events = pattern_events("970milli");
    let projection = projection(&events, SyntaxKind::LiteralPattern);
    let PatternSyntaxKind::Literal(literal) = projection
        .authored()
        .value_at(projection.path())
        .expect("literal value")
        .kind()
    else {
        panic!("expected literal family");
    };

    assert_eq!(literal.family(), SyntaxLiteralFamily::Integer);
    assert!(matches!(
        literal.value().issue(),
        Some(SyntaxLiteralIssue::Integer(SyntaxIntegerIssue::InvalidDigits { attempted }))
            if attempted.as_ref() == "milli"
    ));
}

#[test]
fn decimal_markers_keep_unknown_suffixes_as_decimal_poison() {
    for (source, expected_suffix) in [("1.0milli", "milli"), ("1e2foo", "foo"), ("1.0i32", "i32")] {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::LiteralPattern);
        let PatternSyntaxKind::Literal(literal) = projection
            .authored()
            .value_at(projection.path())
            .expect("literal value")
            .kind()
        else {
            panic!("expected literal family");
        };

        assert_eq!(literal.family(), SyntaxLiteralFamily::Decimal, "{source}");
        assert!(matches!(
            literal.value().issue(),
            Some(SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::InvalidSuffix { suffix }))
                if suffix.as_ref() == expected_suffix
        ));
    }
}

#[test]
fn incomplete_decimal_components_do_not_roll_back_to_integer() {
    for source in ["1e", "1e+", "1e-", "1."] {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::LiteralPattern);
        let PatternSyntaxKind::Literal(literal) = projection
            .authored()
            .value_at(projection.path())
            .expect("literal value")
            .kind()
        else {
            panic!("expected literal family");
        };

        assert_eq!(literal.family(), SyntaxLiteralFamily::Decimal, "{source}");
        assert!(matches!(
            literal.value().issue(),
            Some(SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                SyntaxDecimalComponentIssue::InvalidDigits { .. }
            )))
        ));
    }
}

#[test]
fn radix_prefix_remains_integer_authority_for_conflicting_suffixes() {
    for source in ["0b10f32", "0x10px", "0x10ms"] {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::LiteralPattern);
        let PatternSyntaxKind::Literal(literal) = projection
            .authored()
            .value_at(projection.path())
            .expect("literal value")
            .kind()
        else {
            panic!("expected literal family");
        };

        assert_eq!(literal.family(), SyntaxLiteralFamily::Integer, "{source}");
        assert!(literal.shape().has_prefix(), "{source}");
        assert!(matches!(
            literal.value().issue(),
            Some(SyntaxLiteralIssue::Integer(
                SyntaxIntegerIssue::InvalidDigits { .. }
            ))
        ));
    }
}

#[test]
fn numeric_digit_accounting_excludes_prefix_separators_suffix_and_unit() {
    for (source, expected) in [
        ("0xdead_beef_u128", Some(8)),
        ("1_2.3_4e+5_6f64", Some(6)),
        ("5ms", Some(1)),
        ("970milli", Some(3)),
        ("0b10f32", Some(2)),
        ("0x10px", Some(2)),
        ("\"\\u{41}\"c", None),
    ] {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::LiteralPattern);
        let PatternSyntaxKind::Literal(literal) = projection
            .authored()
            .value_at(projection.path())
            .expect("literal value")
            .kind()
        else {
            panic!("expected literal family");
        };

        assert_eq!(literal.numeric_digit_count(), expected, "{source}");
    }
}

#[test]
fn valid_literals_retain_the_same_typed_family_inventory() {
    for (source, expected) in [
        ("true", SyntaxLiteralFamily::Bool),
        ("\"value\"", SyntaxLiteralFamily::String),
        ("\"x\"c", SyntaxLiteralFamily::Character),
        ("\"\\u{41}\"c", SyntaxLiteralFamily::Character),
        ("42", SyntaxLiteralFamily::Integer),
        ("1.5", SyntaxLiteralFamily::Decimal),
        ("2px", SyntaxLiteralFamily::UnitNumber),
        ("5ms", SyntaxLiteralFamily::Duration),
    ] {
        let events = pattern_events(source);
        let projection = projection(&events, SyntaxKind::LiteralPattern);
        let PatternSyntaxKind::Literal(literal) = projection
            .authored()
            .value_at(projection.path())
            .expect("literal value")
            .kind()
        else {
            panic!("expected literal family");
        };
        assert_eq!(literal.family(), expected, "{source}");
        assert!(literal.value().issue().is_none(), "{source}");
    }
}

#[test]
fn trailing_or_retains_a_missing_alternative_projection() {
    let source = "left |";
    let events = pattern_events(source);
    let or = projection(&events, SyntaxKind::OrPattern);
    let missing = projection(&events, SyntaxKind::MissingPattern);

    assert_eq!(
        or.authored().value_at(or.path()).unwrap().family(),
        crate::patterns::PatternSyntaxFamily::Or
    );
    assert_eq!(
        missing
            .authored()
            .value_at(missing.path())
            .unwrap()
            .family(),
        crate::patterns::PatternSyntaxFamily::Error
    );
    assert_eq!(
        *or.authored()
            .source()
            .component_at(or.path(), PatternComponentRole::Element { ordinal: 1 })
            .expect("second alternative component"),
        SourceRange::new(source.len(), source.len())
    );
    assert_eq!(
        *missing
            .authored()
            .source()
            .component_at(missing.path(), PatternComponentRole::Recovery)
            .expect("missing alternative recovery component"),
        SourceRange::new(source.len(), source.len())
    );
}

fn projection(
    events: &[SyntaxEvent],
    expected: SyntaxKind,
) -> &crate::grammar::event::PendingPatternProjection {
    events
        .iter()
        .find_map(|event| match event {
            SyntaxEvent::StartNode {
                kind,
                projection: crate::grammar::event::PendingStartProjection::Pattern(projection),
                ..
            } if *kind == expected => Some(projection),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {expected:?} projection"))
}

fn pattern_events(source: &str) -> Vec<SyntaxEvent> {
    let tokens = DocumentLexer::new(source).lex();
    let mut events = Vec::new();
    let mut budget = GrammarBudget::default();
    {
        let mut parser = DocumentParser::new(source, &tokens, &mut events, &mut budget);
        emit_pattern(&mut parser, tokens.len(), SyntaxRole::Element(0));
    }
    assert!(budget.failure().is_none());
    events
}
