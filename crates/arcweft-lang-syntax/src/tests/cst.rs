use crate::cst::{
    CstFlowItemKind, CstLetFlowItemKind, CstLine, CstLineKind, CstPunctuationScan, CstStmtKind,
    CstStructuredFlowBlockKind, CstTopLevelItemKind, CstTopLevelLineKind, SyntaxKind,
    classify_stmt, cst_lines, find_last_depth_zero_open_punctuation,
    find_last_top_level_punctuation, find_matching_punctuation,
    find_top_level_matching_punctuation, source_line_count, source_line_iter,
    split_first_string_literal, split_last_top_level_punctuation_sequence_once,
    split_leading_entity_ref_parts, split_leading_lifetime, split_leading_relative_id,
    split_top_level_keyword_once, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once, take_doc_comment_prefix,
};
use crate::{
    ast::flow::{AuthoredExpr, ContractClause, FlowItem, Stmt},
    ast::ids::EntityRef,
    ast::items::Item,
    expr::Expr,
    parser::{ParseOptions, SourceDialect, parse_document, parse_source},
    types::TypeRef,
};

#[test]
fn parsed_source_always_keeps_lossless_syntax() {
    let parsed = parse_source("flow @flow.bad bad {");

    assert!(!parsed.errors().is_empty());
    assert_eq!(parsed.syntax().kind(), SyntaxKind::Root);
    assert_eq!(parsed.syntax().text().to_string(), "flow @flow.bad bad {");
    assert_eq!(parsed.typed_tree().source(), "flow @flow.bad bad {");
}

#[test]
fn agent_dialect_parses_top_level_agent_item() {
    let parsed = parse_document(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe }
{
    observe()
}
",
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    );

    assert_eq!(parsed.errors(), &[]);
    let [Item::Agent(agent)] = parsed.typed_tree().items() else {
        panic!("expected exactly one parsed agent item");
    };
    assert_eq!(agent.name(), "opening_smoke");
    assert_eq!(agent.id().map(EntityRef::body), Some("agent.opening_smoke"));
    assert!(agent.signature().is_some());
    assert_eq!(agent.contracts().len(), 1);
    assert!(matches!(
        agent.body_value().map(AuthoredExpr::expr),
        Some(Expr::Call { .. })
    ));
}

#[test]
fn agent_dialect_parses_multiline_attributes_and_effect_prelude() {
    let parsed = parse_document(
        r"
#[agent(version = 1)]
#[budget(
    timeout = 30s,
    steps = 128usize,
)]
agent @agent.opening.listen opening_listen()
effects {
    agent.observe,
    agent.wait,
}
{
    let first = try observe()
    Ok(first)
}
",
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    );

    assert_eq!(parsed.errors(), &[]);
    let [Item::Agent(agent)] = parsed.typed_tree().items() else {
        panic!("expected exactly one parsed agent item");
    };
    assert_eq!(agent.attrs().len(), 2);
    let [ContractClause::Effects(effects)] = agent.contracts() else {
        panic!("expected a structured effects contract");
    };
    assert_eq!(effects.len(), 2);
    assert!(effects.iter().all(|effect| !matches!(effect, Expr::Raw(_))));
    assert_eq!(agent.body_statements().len(), 1);
    assert!(matches!(
        agent.body_value().map(AuthoredExpr::expr),
        Some(Expr::Call { .. })
    ));
}

#[test]
fn agent_dialect_parses_scope_statement_body() {
    let parsed = parse_document(
        r"
#[agent(version = 1)]
agent @agent.opening.listen opening_listen()
effects {
    agent.observe,
    agent.act.semantic,
}
{
    let first = try observe()
    scope choose_listen {
        let action = try choose(@choice.opening.listen)
        expect(action.accepted)
    }
    Ok(first)
}
",
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    );

    assert_eq!(parsed.errors(), &[]);
    let [Item::Agent(agent)] = parsed.typed_tree().items() else {
        panic!("expected exactly one parsed agent item");
    };
    assert_eq!(agent.body_statements().len(), 2);
    assert!(matches!(
        &agent.body_statements()[1],
        Stmt::Expr {
            expr: Expr::NamedBlock { name, .. },
            ..
        } if name == "scope choose_listen"
    ));
}

#[test]
fn agent_dialect_rejects_removed_line_command_fallback() {
    let parsed = parse_document(
        "observe\n",
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    );

    assert!(parsed.typed_tree().items().is_empty());
    assert!(parsed.errors().iter().any(|error| {
        error
            .message()
            .contains("unsupported top-level item in Agent dialect")
    }));
}

#[test]
fn cst_preserves_comments_doc_comments_entity_refs_and_newlines() {
    let source = "/// Doc\n// comment\nflow @flow.opening opening {}\n";
    let parsed = parse_source(source);
    let token_kinds = parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .map(|token| token.kind())
        .collect::<Vec<_>>();

    assert!(token_kinds.contains(&SyntaxKind::DocComment));
    assert!(token_kinds.contains(&SyntaxKind::Comment));
    assert!(token_kinds.contains(&SyntaxKind::EntityRef));
    assert!(token_kinds.contains(&SyntaxKind::Newline));
    assert_eq!(parsed.line_index().line_col(source.len()), (3, 0));
}

#[test]
fn cst_line_events_classify_trivia_docs_and_code() {
    let root = crate::cst::parse_cst("   \n// comment\n/// Doc\nflow @flow.opening opening {}\n");
    let lines = cst_lines(&root);

    assert_eq!(lines.get(0).map(CstLine::kind), Some(CstLineKind::Blank));
    assert_eq!(lines.get(1).map(CstLine::kind), Some(CstLineKind::Comment));
    assert_eq!(
        lines.get(2).map(CstLine::kind),
        Some(CstLineKind::DocComment)
    );
    assert_eq!(
        lines.get(2).and_then(CstLine::doc_comment_text),
        Some("Doc")
    );
    assert_eq!(lines.get(3).map(CstLine::kind), Some(CstLineKind::Code));
}

#[test]
fn cst_text_helpers_split_lines_and_doc_prefixes() {
    assert_eq!(
        source_line_iter("a\r\nb\n").collect::<Vec<_>>(),
        vec!["a", "b", ""]
    );
    assert_eq!(source_line_count("a\nb"), 2);

    let doc = take_doc_comment_prefix("/// First\n/// Second\nvalue: Type")
        .expect("doc prefix is detected");
    assert_eq!(doc.lines(), &["First".to_owned(), "Second".to_owned()]);
    assert_eq!(doc.consumed(), "/// First\n/// Second\n".len());
}

#[test]
fn cst_line_events_classify_top_level_dispatch() {
    let root = crate::cst::parse_cst(
        "@memo old\n#[build(tool)]\n#![generated(tool)]\nmod game.routes\npub use crate.prelude.*\npub source @source.frames: Source<T, E> {}\nalice: hello\n",
    );
    let lines = cst_lines(&root);

    assert_eq!(
        lines.get(0).map(CstLine::top_level_line_kind),
        Some(CstTopLevelLineKind::Item)
    );
    assert_eq!(
        lines.get(1).map(CstLine::top_level_line_kind),
        Some(CstTopLevelLineKind::Attribute)
    );
    assert_eq!(
        lines.get(2).map(CstLine::top_level_line_kind),
        Some(CstTopLevelLineKind::Attribute)
    );
    assert_eq!(
        lines.get(3).map(CstLine::top_level_line_kind),
        Some(CstTopLevelLineKind::Module)
    );
    assert_eq!(
        lines.get(4).map(CstLine::top_level_line_kind),
        Some(CstTopLevelLineKind::Use)
    );
    assert_eq!(
        lines.get(5).map(CstLine::top_level_item_kind),
        Some(CstTopLevelItemKind::Source)
    );
    assert_eq!(
        lines.get(6).map(CstLine::top_level_item_kind),
        Some(CstTopLevelItemKind::FlowBodyItemOrRaw)
    );
}

#[test]
fn parses_retained_style_item() {
    let parsed = parse_source(
        r#"
style native_text_input_sample {
  token font.jp_sans_stack: FontFamilyList = ["Yu Gothic", system_font(.Ui)]

  TextField {
    font-family = token(font.jp_sans_stack)
    border-color = system_color(.Border)
  }

  TextField:focus-visible {
    focus-ring-width = 2px
  }
}
"#,
    );

    assert_eq!(parsed.errors(), &[]);
    let [Item::Style(style)] = parsed.typed_tree().items() else {
        panic!("expected a typed style item");
    };
    assert_eq!(style.id().body(), "style.native_text_input_sample");
    let sheet = style.sheet();
    assert_eq!(sheet.tokens().len(), 1);
    assert_eq!(sheet.rules().len(), 2);
}

#[test]
fn cst_line_events_classify_flow_item_dispatch() {
    let root = crate::cst::parse_cst(
        "@choice old\nchoice @choice.opening {}\nlet selected = choice @choice.next {}\nlet bg = try await load_bg() with:\nlet route = load_route()?\ninclude @flow.next\nscope local {}\n",
    );
    let lines = cst_lines(&root);

    assert_eq!(
        lines.get(0).map(CstLine::flow_item_kind),
        Some(CstFlowItemKind::Other)
    );
    assert_eq!(
        lines.get(1).map(CstLine::flow_item_kind),
        Some(CstFlowItemKind::StructuredBlock(
            CstStructuredFlowBlockKind::Choice
        ))
    );
    assert_eq!(
        lines.get(2).map(CstLine::flow_item_kind),
        Some(CstFlowItemKind::Let(CstLetFlowItemKind::Choice))
    );
    assert_eq!(
        lines.get(3).map(CstLine::flow_item_kind),
        Some(CstFlowItemKind::Let(CstLetFlowItemKind::AwaitWith))
    );
    assert_eq!(
        lines.get(4).map(CstLine::flow_item_kind),
        Some(CstFlowItemKind::Let(CstLetFlowItemKind::Plain))
    );
    assert_eq!(
        lines.get(5).map(CstLine::flow_item_kind),
        Some(CstFlowItemKind::Include)
    );
    assert_eq!(
        lines.get(6).map(CstLine::flow_item_kind),
        Some(CstFlowItemKind::StructuredBlock(
            CstStructuredFlowBlockKind::Scope
        ))
    );
}

#[test]
fn cst_statement_classifier_covers_typed_statement_heads() {
    assert_eq!(
        classify_stmt("'line.voice <- voice"),
        CstStmtKind::LifetimeSet
    );
    assert_eq!(classify_stmt("let voice = load_voice()"), CstStmtKind::Let);
    assert_eq!(
        classify_stmt("return Ok(done)"),
        CstStmtKind::ControlTransfer
    );
    assert_eq!(
        classify_stmt("defer on cancelled { close line }"),
        CstStmtKind::DeferBlock
    );
    assert_eq!(classify_stmt("defer close line"), CstStmtKind::Defer);
    assert_eq!(
        classify_stmt("ensure(ready, \"not ready\")"),
        CstStmtKind::Expr
    );
    assert_eq!(classify_stmt("ensure ready"), CstStmtKind::Expr);
    assert_eq!(classify_stmt("panic \"todo\""), CstStmtKind::Expr);
    assert_eq!(classify_stmt("on item => yield item"), CstStmtKind::On);
    assert_eq!(
        classify_stmt("unsafe lifetime @unsafe.borrow { promote(value) }"),
        CstStmtKind::UnsafeLifetime
    );
    assert_eq!(
        classify_stmt("thread loader { wait(mark(.done)) }"),
        CstStmtKind::Braced
    );
    assert_eq!(
        classify_stmt("bg.ref(@slot.background.main)"),
        CstStmtKind::Expr
    );
    assert_eq!(classify_stmt("scene @scene.loading"), CstStmtKind::Expr);
    assert_eq!(classify_stmt("if ready"), CstStmtKind::AmbiguousBlockHead);
}

#[test]
fn cst_flow_block_event_keeps_effects_prelude_out_of_body() {
    let root = crate::cst::parse_cst(
        "flow @flow.opening opening\nrequires ready\n effects { asset.read }\n{\n    goto @flow.next\n}\n",
    );
    let lines = cst_lines(&root);
    let block = lines.collect_flow_block(0);

    assert!(block.ok);
    assert!(block.head.contains("effects { asset.read }"));
    assert!(!block.body.contains("effects { asset.read }"));
    assert!(block.body.contains("goto @flow.next"));
    assert_eq!(block.body_line_range, Some(4..5));
}

#[test]
fn cst_flow_block_event_reuses_complete_body_line_events_only() {
    let source = "flow @flow.opening opening {\n    goto @flow.next\n}\n";
    let root = crate::cst::parse_cst(source);
    let lines = crate::cst::cst_lines_for_source(&root, source);
    let block = lines.collect_flow_block(0);

    assert!(block.ok);
    assert_eq!(block.body_line_range, Some(1..2));
    let base = source.find('{').expect("open brace");
    let nested = lines
        .relative_line_slice(
            block.body_line_range.clone().expect("body line range"),
            base,
        )
        .expect("relative nested lines");
    let line = nested.get(0).expect("nested line");
    assert_eq!(line.text(), "    goto @flow.next");
    assert_eq!(line.start(), 2);

    let inline_source = "flow @flow.inline inline { goto @flow.next }\n";
    let inline_root = crate::cst::parse_cst(inline_source);
    let inline_lines = crate::cst::cst_lines_for_source(&inline_root, inline_source);
    let inline = inline_lines.collect_flow_block(0);
    assert!(inline.ok);
    assert_eq!(inline.body_line_range, None);
}

#[test]
fn successful_parse_exposes_typed_tree_and_hash() {
    let parsed = parse_source("alice: おはよう。[p]");

    assert!(parsed.is_ok());
    assert_eq!(parsed.source_hash().as_bytes().len(), 32);
    assert_eq!(parsed.source_hash().to_string().len(), 64);
    assert_eq!(
        parsed.source_hash().to_hex(),
        parsed.source_hash().to_string()
    );
    assert!(matches!(parsed.typed_tree().items(), [Item::FlowItem(_)]));
}

#[test]
fn cst_balanced_scan_ignores_nested_punctuation_and_strings() {
    let (name, value) =
        split_top_level_punctuation_once(r#"call(a = 1, text = "=") = result"#, '=')
            .expect("top-level assignment");

    assert_eq!(name, r#"call(a = 1, text = "=")"#);
    assert_eq!(value, "result");
}

#[test]
fn cst_matching_punctuation_uses_token_offsets() {
    let source = r"alice.say()[text [raw]inner[/raw]]";
    let open = source.find('[').expect("dialogue content open");
    let close = find_matching_punctuation(source, open, '[', ']').expect("matching close");

    assert_eq!(&source[close..=close], "]");
    assert_eq!(&source[open + 1..close], "text [raw]inner[/raw]");
}

#[test]
fn cst_punctuation_scan_deltas_ignore_strings_and_comments() {
    let source = r#"let msg = "[not syntax]" // {not a block}"#;

    let deltas = CstPunctuationScan::new(source).deltas();
    assert_eq!(deltas.bracket, 0);
    assert_eq!(deltas.brace, 0);
    assert_eq!(
        CstPunctuationScan::new("with { cue { call() }")
            .deltas()
            .brace,
        1
    );
}

#[test]
fn cst_punctuation_scan_reuses_fragment_tokens() {
    let source = r#"outer { call("[not]") } trailing"#;
    let scan = CstPunctuationScan::new(source);
    let open = scan
        .find_top_level_punctuation('{')
        .expect("top-level brace");
    let close = scan
        .find_matching_punctuation(open, '{', '}')
        .expect("matching brace");

    assert_eq!(&source[open..=open], "{");
    assert_eq!(&source[close..=close], "}");
    assert_eq!(scan.deltas().brace, 0);
}

#[test]
fn cst_punctuation_scan_reports_line_deltas_from_one_fragment_scan() {
    let source = "call(\n    text = \"ignored )\"\n)\nnext()";
    let scan = CstPunctuationScan::new(source);
    let deltas = scan.line_deltas(source);

    assert_eq!(deltas.len(), 4);
    assert_eq!(deltas[0].paren, 1);
    assert_eq!(deltas[1].paren, 0);
    assert_eq!(deltas[2].paren, -1);
    assert_eq!(deltas[3].paren, 0);
}

#[test]
fn cst_top_level_matching_punctuation_uses_one_fragment_scan() {
    let source = r#"outer(call("[not]"), nested(value))"#;
    let (open, close) =
        find_top_level_matching_punctuation(source, '(', ')').expect("top-level call group");

    assert_eq!(&source[open..=open], "(");
    assert_eq!(&source[close..=close], ")");
    assert_eq!(&source[open + 1..close], r#"call("[not]"), nested(value)"#);
}

#[test]
fn cst_line_projection_records_path_free_parse_stats() {
    let parsed = parse_source(
        r"
flow @flow.opening opening {
    alice.say()[本文です。[p]]
}
",
    );
    let stats = parsed.syntax_stats();

    assert_eq!(stats.cst_lex_passes, 1);
    assert!(stats.punctuation_scans >= 3);
    assert!(stats.punctuation_scan_bytes > 0);
    assert_eq!(stats.line_owned_bytes, 0);
    assert_eq!(stats.block_owned_bytes, 0);
    assert_eq!(stats.wiki_scan_performed, 0);
}

#[test]
fn parse_stats_count_numeric_sequence_summaries_from_flow_body() {
    let parsed = parse_source(
        r"
flow @flow.opening opening {
    let values: Vec<i64> = [1i64, 2i64, 3i64]
}
",
    );

    assert_eq!(parsed.syntax_stats().numeric_seq_summaries, 1);
}

#[test]
fn flow_let_value_continuation_keeps_effect_row_type_ascription_with_closure() {
    let parsed = parse_source(
        r"
flow @flow.closure_expected_row closure_expected_row
effects { }
{
    let later: String -> String effects { fs.read } =
        |path: String| -> String {
            adapter.read_text(path = path)
        }
}
",
    );
    assert!(
        parsed.errors().is_empty(),
        "source should parse without recovery errors: {:?}",
        parsed.errors()
    );
    let [Item::Flow(flow)] = parsed.typed_tree().items() else {
        panic!("expected one flow item");
    };
    let [
        FlowItem::Stmt(Stmt::Let {
            ty: Some(TypeRef::Function { effects, .. }),
            expr_source: Some(expr_source),
            ..
        }),
    ] = flow.body()
    else {
        panic!("expected one typed let flow statement: {:?}", flow.body());
    };

    let effects = effects.as_ref().expect("function type effect row");
    assert_eq!(effects.effects(), &["fs.read".to_owned()]);
    assert!(
        expr_source
            .trim_start()
            .starts_with("|path: String| -> String"),
        "closure expression should stay attached to the let statement: {expr_source}"
    );
}

#[test]
fn cst_depth_zero_open_finds_function_body_not_nested_scope() {
    let source = "task fn load() -> Result<T, E> { with { nested() } }";
    let open = find_last_depth_zero_open_punctuation(source, '{', '}').expect("function body open");

    assert_eq!(source[..open].trim_end(), "task fn load() -> Result<T, E>");
}

#[test]
fn cst_top_level_tail_helpers_ignore_nested_syntax() {
    let source = r#"target(call["not it"])[content]"#;
    let open = find_last_top_level_punctuation(source, '[').expect("top-level bracket");
    let (path, name) =
        split_last_top_level_punctuation_sequence_once("Result::Nested<Option::Some>", &[":", ":"])
            .expect("last path separator");

    assert_eq!(&source[open..], "[content]");
    assert_eq!(path, "Result");
    assert_eq!(name, "Nested<Option::Some>");
}

#[test]
fn cst_leading_reference_helpers_keep_precise_tails() {
    let (lifetime, rest) = split_leading_lifetime("'frame [u8]").expect("lifetime");
    let entity = split_leading_entity_ref_parts("@asset.bg.room trailing").expect("entity");
    let at_entity = split_leading_entity_ref_parts("@asset.bg.room trailing").expect("at entity");
    let delimited_hash_entity = split_leading_entity_ref_parts("#<asset.bg.room> trailing");
    let relative = split_leading_relative_id("@.opening.next)").expect("relative id");
    let explicit_relative =
        split_leading_relative_id("@...opening.next)").expect("explicit relative id");
    let super_relative =
        split_leading_relative_id("@super.super.opening.next)").expect("super relative id");
    let bare_relative = split_leading_relative_id(".opening.next)");
    let family_relative =
        crate::cst::split_leading_relative_entity_ref("@flow:.next tail").expect("family relative");

    assert_eq!(lifetime, "'frame");
    assert_eq!(rest, "[u8]");
    assert_eq!(entity.raw, "@asset.bg.room");
    assert_eq!(entity.body, "asset.bg.room");
    assert_eq!(entity.rest, " trailing");
    assert_eq!(at_entity.raw, "@asset.bg.room");
    assert_eq!(at_entity.body, "asset.bg.room");
    assert_eq!(at_entity.rest, " trailing");
    assert_eq!(delimited_hash_entity, None);
    assert_eq!(relative.body, "opening.next");
    assert_eq!(relative.parent_depth, 0);
    assert_eq!(relative.rest, ")");
    assert_eq!(explicit_relative.body, "opening.next");
    assert_eq!(explicit_relative.parent_depth, 2);
    assert_eq!(explicit_relative.rest, ")");
    assert_eq!(super_relative.body, "opening.next");
    assert_eq!(super_relative.parent_depth, 2);
    assert_eq!(super_relative.rest, ")");
    assert_eq!(bare_relative, None);
    assert_eq!(family_relative.raw, "@flow:.next");
    assert_eq!(family_relative.family, "flow");
    assert_eq!(family_relative.relative.body, "next");
    assert_eq!(family_relative.rest, " tail");
    assert!(split_leading_entity_ref_parts("@flow:.next").is_none());
}

#[test]
fn cst_keyword_split_ignores_nested_guards_and_strings() {
    let (head, tail) =
        split_top_level_keyword_once(r#"opt.map(|v| v.when_ready()) when flag != "when""#, "when");

    assert_eq!(head.trim(), "opt.map(|v| v.when_ready())");
    assert_eq!(tail, Some(r#"flag != "when""#));
}

#[test]
fn cst_punctuation_sequence_split_ignores_nested_operators() {
    let (head, tail) = split_top_level_punctuation_sequence_once(
        r#"choice.map(|v| v => "raw") => out"#,
        &["=", ">"],
    )
    .expect("top-level choice arm separator");

    assert_eq!(head, r#"choice.map(|v| v => "raw")"#);
    assert_eq!(tail, "out");
}

#[test]
fn cst_string_literal_split_returns_body_and_tail() {
    let (body, tail) =
        split_first_string_literal(r#"before "quoted value" if enabled"#).expect("string literal");

    assert_eq!(body, "quoted value");
    assert_eq!(tail, " if enabled");
}
