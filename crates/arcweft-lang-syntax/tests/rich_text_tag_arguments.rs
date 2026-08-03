use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        dialogue::{
            DialogueTag, DialogueTagArg, DialogueTagArgValueSurface, DialogueTagPayload,
            DialogueToken, QuoteStyle,
        },
        flow::FlowItem,
        items::Item,
    },
    text::{
        DialogueTextDiagnostic, DialogueTextDiagnosticCode, MAX_RICH_TEXT_CONTENT_ARGUMENTS,
        MAX_RICH_TEXT_CONTENT_TAGS, MAX_RICH_TEXT_TAG_ARGUMENTS, MAX_RICH_TEXT_TAG_BODY_BYTES,
        MAX_RICH_TEXT_TAG_KEY_BYTES, MAX_RICH_TEXT_TAG_VALUE_BYTES, RichTextArgumentIssue,
        parse_dialogue_text,
    },
};

type PayloadPredicate = fn(&DialogueTagPayload) -> bool;

fn first_tag(source: &str) -> (arcweft_lang_syntax::text::DialogueTextParse, &str) {
    (parse_dialogue_text(source), source)
}

fn tag(parsed: &arcweft_lang_syntax::text::DialogueTextParse) -> &DialogueTag {
    parsed
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::Tag(tag) | DialogueToken::InferredTag(tag) => Some(tag),
            _ => None,
        })
        .expect("dialogue tag")
}

fn value(argument: &DialogueTagArg) -> &str {
    argument.value().expect("present argument value").value()
}

#[test]
fn g001_positional_value_retains_token_content_and_full_ranges() {
    let source = "[w 500ms]";
    let (parsed, source) = first_tag(source);
    assert!(parsed.diagnostics().is_empty());
    let tag = tag(&parsed);
    let argument = &tag.arguments()[0];
    let value = argument.value().expect("wait value");

    assert_eq!(argument.name(), None);
    assert_eq!(&source[argument.range().as_range()], "500ms");
    assert_eq!(&source[value.token_range().as_range()], "500ms");
    assert_eq!(&source[value.content_range().as_range()], "500ms");
    assert_eq!(value.quote(), QuoteStyle::Unquoted);
}

#[test]
fn g002_named_value_retains_key_equals_value_and_full_ranges() {
    let source = "[w time=500ms]";
    let parsed = parse_dialogue_text(source);
    assert!(parsed.diagnostics().is_empty());
    let argument = &tag(&parsed).arguments()[0];
    let value = argument.value().expect("wait value");

    assert_eq!(argument.name(), Some("time"));
    assert_eq!(
        &source[argument.name_range().expect("key range").as_range()],
        "time"
    );
    assert_eq!(
        &source[argument.equals_range().expect("equals range").as_range()],
        "="
    );
    assert_eq!(&source[value.range().as_range()], "500ms");
    assert_eq!(&source[argument.range().as_range()], "time=500ms");
}

#[test]
fn g003_explicit_family_retains_selector_then_named_arguments() {
    let source = "[transform .offset x=4px y=-2px]";
    let parsed = parse_dialogue_text(source);
    assert!(parsed.diagnostics().is_empty());
    let arguments = tag(&parsed).arguments();

    assert_eq!(arguments.len(), 3);
    assert_eq!(arguments[0].name(), None);
    assert_eq!(value(&arguments[0]), ".offset");
    assert_eq!(arguments[1].name(), Some("x"));
    assert_eq!(value(&arguments[1]), "4px");
    assert_eq!(arguments[2].name(), Some("y"));
    assert_eq!(value(&arguments[2]), "-2px");
}

#[test]
fn g004_every_contract_whitespace_scalar_separates_arguments() {
    let whitespace = [
        '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{0085}',
        '\u{00A0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}',
        '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}',
        '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}',
    ];
    for separator in whitespace {
        let source = format!("[effect .wave{separator}amp=2]");
        let parsed = parse_dialogue_text(&source);
        assert!(
            parsed.diagnostics().is_empty(),
            "{separator:?}: {:?}",
            parsed.diagnostics()
        );
        let arguments = tag(&parsed).arguments();
        assert_eq!(arguments.len(), 2, "{separator:?}");
        assert_eq!(value(&arguments[0]), ".wave");
        assert_eq!(value(&arguments[1]), "2");
        assert_eq!(
            &source[arguments[1].range().as_range()],
            "amp=2",
            "{separator:?}"
        );
    }
}

#[test]
fn g005_commas_and_semicolons_remain_inside_values() {
    let source = "[transform .offset dir=0,1 note=a;b]";
    let parsed = parse_dialogue_text(source);
    assert!(parsed.diagnostics().is_empty());
    let arguments = tag(&parsed).arguments();

    assert_eq!(arguments.len(), 3);
    assert_eq!(value(&arguments[1]), "0,1");
    assert_eq!(value(&arguments[2]), "a;b");
}

#[test]
fn g006_only_first_unescaped_unquoted_equal_selects_the_key() {
    let source = r#"[effect .wave pattern=a=b quoted="a=b"]"#;
    let parsed = parse_dialogue_text(source);
    assert!(parsed.diagnostics().is_empty());
    let arguments = tag(&parsed).arguments();

    assert_eq!(arguments[1].name(), Some("pattern"));
    assert_eq!(value(&arguments[1]), "a=b");
    assert_eq!(arguments[2].name(), Some("quoted"));
    assert_eq!(value(&arguments[2]), "a=b");
}

#[test]
fn g007_supported_escapes_decode_without_losing_authored_source() {
    let source = r#"[effect .wave text="a b\"c\=d\[e\]" bare=a\ b equals=a\=b quote=a\"]"#;
    let parsed = parse_dialogue_text(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let arguments = tag(&parsed).arguments();

    assert_eq!(value(&arguments[1]), "a b\"c=d[e]");
    assert_eq!(
        arguments[1].value().expect("quoted value").source(),
        r#""a b\"c\=d\[e\]""#
    );
    assert_eq!(value(&arguments[2]), "a b");
    assert_eq!(value(&arguments[3]), "a=b");
    assert_eq!(value(&arguments[4]), "a\"");
}

#[test]
fn g008_single_and_double_quotes_decode_equally_but_remain_typed() {
    let source = r#"[font single='日本 語' double="日本 語"]"#;
    let parsed = parse_dialogue_text(source);
    assert!(parsed.diagnostics().is_empty());
    let arguments = tag(&parsed).arguments();
    let single = arguments[0].value().expect("single-quoted value");
    let double = arguments[1].value().expect("double-quoted value");

    assert_eq!(single.value(), double.value());
    assert_eq!(single.quote(), QuoteStyle::Single);
    assert_eq!(double.quote(), QuoteStyle::Double);
    assert_eq!(
        &source[single
            .opening_quote_range()
            .expect("opening quote")
            .as_range()],
        "'"
    );
    assert_eq!(
        &source[single
            .closing_quote_range()
            .expect("closing quote")
            .as_range()],
        "'"
    );
    assert_eq!(&source[single.content_range().as_range()], "日本 語");
}

#[test]
fn g009_missing_and_present_empty_values_are_distinct_surfaces() {
    let source = r#"[effect .wave missing= empty=""]"#;
    let parsed = parse_dialogue_text(source);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .map(DialogueTextDiagnostic::code)
            .collect::<Vec<_>>(),
        [DialogueTextDiagnosticCode::RichTextAttributeMissingValue]
    );
    let arguments = tag(&parsed).arguments();

    assert!(matches!(
        arguments[1].value_surface(),
        Some(DialogueTagArgValueSurface::Missing { .. })
    ));
    assert!(arguments[1].value().is_none());
    assert_eq!(value(&arguments[2]), "");
    assert_eq!(
        arguments[2].value().expect("present empty").quote(),
        QuoteStyle::Double
    );
}

#[test]
fn g010_invalid_keys_and_escapes_are_ordered_invalid_records() {
    let source = r"[effect .wave =value Upper=1 日本=2 bad=\q]";
    let parsed = parse_dialogue_text(source);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .map(DialogueTextDiagnostic::code)
            .collect::<Vec<_>>(),
        [
            DialogueTextDiagnosticCode::RichTextAttributeEmptyKey,
            DialogueTextDiagnosticCode::RichTextAttributeInvalidKey,
            DialogueTextDiagnosticCode::RichTextAttributeInvalidKey,
            DialogueTextDiagnosticCode::RichTextAttributeInvalidEscape,
        ]
    );
    let arguments = tag(&parsed).arguments();
    assert!(matches!(
        arguments[1].issue(),
        Some(RichTextArgumentIssue::EmptyKey)
    ));
    assert!(matches!(
        arguments[2].issue(),
        Some(RichTextArgumentIssue::InvalidKey)
    ));
    assert!(matches!(
        arguments[3].issue(),
        Some(RichTextArgumentIssue::InvalidKey)
    ));
    assert!(matches!(
        arguments[4].issue(),
        Some(RichTextArgumentIssue::InvalidEscape)
    ));
    assert_eq!(
        arguments[1..=4]
            .iter()
            .map(DialogueTagArg::issue_range)
            .collect::<Vec<_>>(),
        [
            Some(TextRange::new(14, 14)),
            Some(TextRange::new(21, 26)),
            Some(TextRange::new(29, 35)),
            Some(TextRange::new(42, 44)),
        ]
    );
}

#[test]
fn g011_unterminated_quote_recovers_before_the_following_tag() {
    let source = r#"[effect .wave note="unfinished][.sparkle]next[/]"#;
    let parsed = parse_dialogue_text(source);
    assert_eq!(
        parsed.diagnostics()[0].code(),
        DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote
    );
    assert!(matches!(
        tag(&parsed).arguments()[1].issue(),
        Some(RichTextArgumentIssue::UnterminatedQuote)
    ));
    assert_eq!(
        tag(&parsed).arguments()[1].issue_range(),
        Some(TextRange::new(19, 30))
    );
    assert!(
        parsed.tokens().iter().any(
            |token| matches!(token, DialogueToken::InferredTag(tag) if tag.name() == ".sparkle")
        )
    );
}

#[test]
fn g012_unicode_values_preserve_scalar_sequence_and_utf8_ranges() {
    let source = r#"[font value="游ゴシック" selector=.日本語]"#;
    let parsed = parse_dialogue_text(source);
    assert!(parsed.diagnostics().is_empty());
    let arguments = tag(&parsed).arguments();

    assert_eq!(value(&arguments[0]), "游ゴシック");
    assert_eq!(value(&arguments[1]), ".日本語");
    assert_eq!(
        &source[arguments[0]
            .value()
            .expect("font value")
            .content_range()
            .as_range()],
        "游ゴシック"
    );
}

#[test]
fn g013_b002_crlf_and_indentation_project_all_argument_ranges() {
    let source =
        "flow opening {\r\n    narrator:\r\n        [.wave\u{3000}amp=\"二 px\"]text[/]\r\n}\r\n";
    let parsed = parse_rich_text_fixture(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let tree = parsed.typed_tree();
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("speaker line");
    };
    let content = line.content();
    let tag = content
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::InferredTag(tag) => Some(tag),
            _ => None,
        })
        .expect("inferred tag");
    let argument = &tag.arguments()[0];
    let value = argument.value().expect("amp value");

    for range in [
        tag.range(),
        tag.name_range(),
        argument.range(),
        argument.name_range().expect("key range"),
        argument.equals_range().expect("equals range"),
        value.token_range(),
        value.content_range(),
        value.opening_quote_range().expect("opening quote"),
        value.closing_quote_range().expect("closing quote"),
    ] {
        let projected = content.source_range(range).expect("projected range");
        assert_eq!(
            &content.raw()[range.as_range()],
            &source[projected.as_range()]
        );
    }
    let cr = source.find("\r\n").expect("CRLF");
    assert_eq!(content.content_offset(cr + 1), None);
}

#[test]
fn g014_argument_key_value_and_tag_body_limits_are_exact() {
    let args_32 = (0..MAX_RICH_TEXT_TAG_ARGUMENTS)
        .map(|index| format!("k{index}=v"))
        .collect::<Vec<_>>()
        .join(" ");
    let accepted = parse_dialogue_text(&format!("[effect {args_32}]"));
    assert!(accepted.diagnostics().is_empty());
    assert_eq!(
        tag(&accepted).arguments().len(),
        MAX_RICH_TEXT_TAG_ARGUMENTS
    );

    let args_33 = format!("{args_32} excess=v");
    let rejected = parse_dialogue_text(&format!("[effect {args_33}]"));
    assert_eq!(
        tag(&rejected).arguments().len(),
        MAX_RICH_TEXT_TAG_ARGUMENTS
    );
    assert_eq!(
        rejected.diagnostics()[0].code(),
        DialogueTextDiagnosticCode::RichTextAttributeTooMany
    );

    let key_64 = format!("a{}", "b".repeat(MAX_RICH_TEXT_TAG_KEY_BYTES - 1));
    let key_65 = format!("{key_64}c");
    assert!(
        parse_dialogue_text(&format!("[effect {key_64}=v]"))
            .diagnostics()
            .is_empty()
    );
    assert_eq!(
        parse_dialogue_text(&format!("[effect {key_65}=v]")).diagnostics()[0].code(),
        DialogueTextDiagnosticCode::RichTextAttributeKeyTooLong
    );

    let value_4096 = "v".repeat(MAX_RICH_TEXT_TAG_VALUE_BYTES);
    let value_4097 = format!("{value_4096}v");
    assert!(
        parse_dialogue_text(&format!("[effect key={value_4096}]"))
            .diagnostics()
            .is_empty()
    );
    assert_eq!(
        parse_dialogue_text(&format!("[effect key={value_4097}]")).diagnostics()[0].code(),
        DialogueTextDiagnosticCode::RichTextAttributeValueTooLong
    );

    let exact_body = exact_tag_body(MAX_RICH_TEXT_TAG_BODY_BYTES);
    assert!(
        parse_dialogue_text(&format!("[{exact_body}]"))
            .diagnostics()
            .iter()
            .all(|diagnostic| {
                diagnostic.code() != DialogueTextDiagnosticCode::RichTextTagBodyTooLong
            })
    );
    let oversized_body = format!("{exact_body}x");
    assert_eq!(
        parse_dialogue_text(&format!("[{oversized_body}]")).diagnostics()[0].code(),
        DialogueTextDiagnosticCode::RichTextTagBodyTooLong
    );
}

fn exact_tag_body(bytes: usize) -> String {
    let mut body = String::from("effect");
    let prefixes = (0..MAX_RICH_TEXT_TAG_ARGUMENTS)
        .map(|index| format!(" k{index}="))
        .collect::<Vec<_>>();
    let fixed = body.len() + prefixes.iter().map(String::len).sum::<usize>();
    let value_bytes = bytes - fixed;
    let per_value = value_bytes / prefixes.len();
    let extra = value_bytes % prefixes.len();
    for (index, prefix) in prefixes.iter().enumerate() {
        body.push_str(prefix);
        body.push_str(&"v".repeat(per_value + usize::from(index < extra)));
    }
    assert_eq!(body.len(), bytes);
    body
}

#[test]
fn syntax_content_limits_stop_retaining_excess_nodes_and_arguments() {
    let excess_tags = 3;
    let tags = "[p]".repeat(MAX_RICH_TEXT_CONTENT_TAGS + excess_tags);
    let parsed = parse_dialogue_text(&tags);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code() == DialogueTextDiagnosticCode::RichTextContentTagLimit
            })
            .count(),
        1
    );
    assert!(matches!(
        parsed.tokens().last(),
        Some(DialogueToken::Text(text)) if text == &"[p]".repeat(excess_tags)
    ));

    let one_tag = format!(
        "[effect {}]",
        (0..MAX_RICH_TEXT_TAG_ARGUMENTS)
            .map(|index| format!("k{index}=v"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let content = one_tag.repeat(MAX_RICH_TEXT_CONTENT_ARGUMENTS / MAX_RICH_TEXT_TAG_ARGUMENTS + 3);
    let parsed = parse_dialogue_text(&content);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code() == DialogueTextDiagnosticCode::RichTextContentArgumentLimit
            })
            .count(),
        1
    );
    assert_eq!(
        parsed
            .tokens()
            .iter()
            .filter_map(|token| match token {
                DialogueToken::Tag(tag) | DialogueToken::InferredTag(tag) => {
                    Some(tag.arguments().len())
                }
                _ => None,
            })
            .sum::<usize>(),
        MAX_RICH_TEXT_CONTENT_ARGUMENTS
    );
}

#[test]
fn g015_fx_call_dialogue_call_and_condition_have_dedicated_payloads() {
    let inputs: [(&str, PayloadPredicate); 3] = [
        (
            "[fx warning(accent=\"urgent\")]",
            |payload: &DialogueTagPayload| matches!(payload, DialogueTagPayload::FxCall(_)),
        ),
        ("[call flash(level=2)]", |payload: &DialogueTagPayload| {
            matches!(payload, DialogueTagPayload::DialogueCall(_))
        }),
        ("[if player.ready]", |payload: &DialogueTagPayload| {
            matches!(payload, DialogueTagPayload::Condition(_))
        }),
    ];

    for (source, expected) in inputs {
        let parsed = parse_dialogue_text(source);
        assert!(parsed.diagnostics().is_empty(), "{source}");
        let tag = tag(&parsed);
        assert!(expected(tag.payload()), "{source}: {:?}", tag.payload());
        assert!(tag.arguments().is_empty(), "{source}");
    }

    let shorthand = parse_dialogue_text("[! flash(level=2)]");
    let shorthand = tag(&shorthand);
    assert_eq!(shorthand.source_name(), "!");
    assert_eq!(shorthand.canonical_name(), Some("call"));
    assert!(matches!(
        shorthand.payload(),
        DialogueTagPayload::DialogueCall(_)
    ));
}

#[test]
fn g016_invalid_argument_bytes_remain_lossless_in_the_syntax_tree() {
    let source = r"[effect .wave Bad=\q]";
    let parsed = parse_dialogue_text(source);
    let tag = tag(&parsed);

    assert_eq!(&source[tag.range().as_range()], source);
    assert_eq!(tag.arguments()[1].invalid_source(), Some(r"Bad=\q"));
    assert_eq!(&source[tag.arguments()[1].range().as_range()], r"Bad=\q");
}

#[test]
fn source_alias_and_canonical_tag_names_are_both_retained() {
    let parsed = parse_dialogue_text("[page]");
    let tag = tag(&parsed);
    assert_eq!(tag.source_name(), "page");
    assert_eq!(tag.name(), "p");
    assert_eq!(tag.canonical_name(), Some("p"));
    assert_eq!(tag.name_range(), TextRange::new(1, 5));
}

fn parse_rich_text_fixture(source: impl Into<String>) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(
                "arcweft-test://syntax/rich-text-tag-arguments",
            )
            .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path("rich-text-tag-arguments.arcw"),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
