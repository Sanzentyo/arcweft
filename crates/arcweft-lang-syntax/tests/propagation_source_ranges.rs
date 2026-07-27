use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        flow::{FlowItem, Stmt},
        items::{ImplMember, Item},
    },
    expr::{
        AwaitPropagation, AwaitPropagationSource, Expr, TryOperatorSource,
        collect_expr_source_ranges, parse_expr,
    },
};

fn range_of(source: &str, needle: &str) -> TextRange {
    let start = source
        .find(needle)
        .expect("fixture contains source fragment");
    TextRange::new(start, start + needle.len())
}

fn slice(source: &str, range: TextRange) -> &str {
    &source[range.start()..range.end()]
}

#[test]
fn flow_signature_source_excludes_visibility_contract_and_trivia_at_utf8_offset() {
    let source = concat!(
        "// 前置き\n",
        "    pub flow @flow.audit audit(input: Input) -> Result<Output, Failure>   ",
        "effects { log.write } {\n",
        "        return input\n",
        "    }\n",
    );
    let parsed = parse_propagation_fixture(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Flow(flow) = &parsed.typed_tree().items()[0] else {
        panic!("expected flow item")
    };

    let header = "flow @flow.audit audit(input: Input) -> Result<Output, Failure>";
    let result = "Result<Output, Failure>";
    let retained = flow.signature_source();
    assert_eq!(retained.header(), range_of(source, header));
    assert_eq!(retained.result(), Some(range_of(source, result)));
    assert_eq!(slice(source, retained.header()), header);
    assert_eq!(
        slice(source, retained.result().expect("result source")),
        result
    );
}

#[test]
fn impl_method_reuses_exact_function_signature_source_at_utf8_offset() {
    let source = concat!(
        "// 実装\n",
        "impl Handler {\n",
        "    fn handle(value: Input) -> Result<Output, Failure>   {\n",
        "        try value\n",
        "    }\n",
        "}\n",
    );
    let parsed = parse_propagation_fixture(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Impl(item) = &parsed.typed_tree().items()[0] else {
        panic!("expected impl item")
    };
    let ImplMember::Function {
        signature_source, ..
    } = &item.members()[0]
    else {
        panic!("expected impl function member")
    };

    let signature = "fn handle(value: Input) -> Result<Output, Failure>";
    let result = "Result<Output, Failure>";
    assert_eq!(signature_source.signature(), range_of(source, signature));
    assert_eq!(signature_source.result(), Some(range_of(source, result)));
    assert_eq!(slice(source, signature_source.signature()), signature);
    assert_eq!(slice(source, signature_source.name()), "handle");
    assert_eq!(
        slice(source, signature_source.result().expect("result source")),
        result
    );
}

#[test]
fn closure_source_excludes_surrounding_trivia_and_keeps_utf8_byte_ranges() {
    let source = concat!(
        "// 前置き\n",
        "fn build() -> Handler {\n",
        "  |value: Input| -> Result<Output, Failure>  {\n",
        "    \"値\"\n",
        "  }  \n",
        "}\n",
    );
    let parsed = parse_propagation_fixture(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Function(function) = &parsed.typed_tree().items()[0] else {
        panic!("expected function item")
    };
    let Expr::Closure {
        source: retained, ..
    } = function.body_value().expect("function tail value").expr()
    else {
        panic!("expected closure expression")
    };

    let closure_start = source.find('|').expect("opening pipe");
    let result = "Result<Output, Failure>";
    let result_range = range_of(source, result);
    let body_start = result_range.end()
        + source[result_range.end()..]
            .find('{')
            .expect("closure opening brace");
    let body_end = body_start
        + source[body_start..]
            .find('}')
            .expect("closure closing brace")
        + '}'.len_utf8();

    assert_eq!(retained.whole(), TextRange::new(closure_start, body_end));
    assert_eq!(
        retained.header(),
        TextRange::new(closure_start, result_range.end())
    );
    assert_eq!(retained.result(), Some(result_range));
    assert_eq!(retained.body(), TextRange::new(body_start, body_end));
}

#[test]
fn try_and_await_grouping_follow_one_fixed_precedence_model() {
    let Expr::Try(outer) = parse_expr("(await need)?").expect("grouped await parses") else {
        panic!("expected outer postfix Try")
    };
    assert_eq!(outer.source().whole(), TextRange::new(0, 13));
    assert_eq!(outer.source().operand(), TextRange::new(0, 12));
    assert_eq!(outer.source().operator_range(), TextRange::new(12, 13));
    let Expr::Await(awaited) = outer.operand() else {
        panic!("postfix Try wraps the grouped await")
    };
    assert_eq!(awaited.propagation(), AwaitPropagation::PreserveResult);
    assert_eq!(awaited.source().whole(), TextRange::new(1, 11));

    let Expr::Try(prefix) = parse_expr("try (await need)").expect("general prefix Try parses")
    else {
        panic!("expected general prefix Try")
    };
    assert_eq!(prefix.source().whole(), TextRange::new(0, 16));
    assert_eq!(prefix.source().operand(), TextRange::new(4, 16));
    assert_eq!(prefix.source().operator_range(), TextRange::new(0, 3));
    assert!(matches!(
        prefix.operand(),
        Expr::Await(awaited)
            if awaited.propagation() == AwaitPropagation::PreserveResult
                && awaited.source().whole() == TextRange::new(5, 15)
    ));

    let Expr::Try(prefix) = parse_expr("try value?").expect("nested Try parses") else {
        panic!("expected outer prefix Try")
    };
    assert_eq!(prefix.source().whole(), TextRange::new(0, 10));
    assert_eq!(prefix.source().operand(), TextRange::new(4, 10));
    assert!(matches!(
        prefix.operand(),
        Expr::Try(inner)
            if inner.source().whole() == TextRange::new(4, 10)
                && inner.source().operator_range() == TextRange::new(9, 10)
    ));

    for (source, propagation) in [
        (
            "try await need?",
            AwaitPropagationSource::PrefixTry {
                try_keyword: TextRange::new(0, 3),
            },
        ),
        (
            "await? need?",
            AwaitPropagationSource::AttachedQuestion {
                question: TextRange::new(5, 6),
            },
        ),
    ] {
        let Expr::Await(awaited) = parse_expr(source).expect("nested await operand parses") else {
            panic!("expected one await node")
        };
        assert_eq!(awaited.propagation(), AwaitPropagation::PropagateError);
        assert_eq!(awaited.source().propagation(), Some(propagation));
        assert!(matches!(awaited.operand(), Expr::Try(_)));
    }
}

#[test]
fn try_ranges_exclude_outer_trivia_and_preserve_inner_trivia() {
    let Expr::Try(postfix) = parse_expr("  value?  ").expect("postfix Try with trivia") else {
        panic!("expected postfix Try")
    };
    assert_eq!(postfix.source().whole(), TextRange::new(2, 8));
    assert_eq!(postfix.source().operand(), TextRange::new(2, 7));
    assert_eq!(postfix.source().operator_range(), TextRange::new(7, 8));

    let Expr::Try(prefix) = parse_expr("  try value  ").expect("prefix Try with trivia") else {
        panic!("expected prefix Try")
    };
    assert_eq!(prefix.source().whole(), TextRange::new(2, 11));
    assert_eq!(prefix.source().operand(), TextRange::new(6, 11));
    assert_eq!(prefix.source().operator_range(), TextRange::new(2, 5));

    let source = "try /* policy */\nvalue";
    let Expr::Try(prefix) = parse_expr(source).expect("commented multiline prefix Try") else {
        panic!("expected prefix Try")
    };
    let operand_start = source.find("value").expect("operand");
    assert_eq!(prefix.source().whole(), TextRange::new(0, source.len()));
    assert_eq!(
        prefix.source().operand(),
        TextRange::new(operand_start, source.len())
    );
    assert_eq!(prefix.source().operator_range(), TextRange::new(0, 3));
}

#[test]
fn grouped_multiline_and_utf8_ranges_are_byte_exact() {
    for source in ["(call(\n  value\n))?", "((value))?"] {
        let Expr::Try(try_expr) = parse_expr(source).expect("grouped postfix Try parses") else {
            panic!("expected postfix Try")
        };
        assert_eq!(try_expr.source().whole(), TextRange::new(0, source.len()));
        assert_eq!(
            try_expr.source().operand(),
            TextRange::new(0, source.len() - '?'.len_utf8())
        );
        assert_eq!(
            try_expr.source().operator_range(),
            TextRange::new(source.len() - '?'.len_utf8(), source.len())
        );
    }

    let Expr::Try(utf8) = parse_expr("値?").expect("UTF-8 postfix Try") else {
        panic!("expected postfix Try")
    };
    assert_eq!(utf8.source().whole(), TextRange::new(0, 4));
    assert_eq!(utf8.source().operand(), TextRange::new(0, 3));
    assert_eq!(utf8.source().operator_range(), TextRange::new(3, 4));
}

#[test]
fn dialogue_try_uses_the_ordinary_expression_node_and_source_recursion() {
    let source = r"
flow @flow.dialogue dialogue() -> Result<Unit, LineError> {
    let result = try alice.say()[hello]
}
";
    let parsed = parse_propagation_fixture(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Flow(flow) = &parsed.typed_tree().items()[0] else {
        panic!("expected flow")
    };
    let FlowItem::Stmt(Stmt::Let {
        expr: Expr::Try(try_expr),
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected ordinary Try node around dialogue call")
    };
    assert!(matches!(try_expr.operand(), Expr::DialogueCall { .. }));
    assert!(matches!(
        try_expr.source().operator(),
        TryOperatorSource::PrefixTry { .. }
    ));

    let expression_source = "try /* gap */ (await need)?";
    let expression = parse_expr(expression_source).expect("nested source expression");
    let ranges = collect_expr_source_ranges(
        &expression,
        expression_source,
        TextRange::new(0, expression_source.len()),
    );
    assert!(matches!(ranges[0].expr(), Expr::Try(_)));
    assert_eq!(
        ranges[0].range(),
        TextRange::new(0, expression_source.len())
    );
    assert!(
        ranges
            .iter()
            .any(|entry| matches!(entry.expr(), Expr::Await(_)))
    );
}

#[test]
fn malformed_try_and_await_use_ordinary_zero_width_recovery() {
    for (source, expected) in [
        ("try", TextRange::new(3, 3)),
        ("try /*x*/", TextRange::new(9, 9)),
        ("await?", TextRange::new(6, 6)),
        ("try await", TextRange::new(9, 9)),
    ] {
        let error = parse_expr(source).expect_err("missing operand must fail");
        assert_eq!(error.range(), expected, "{source:?}");
    }

    let plain = parse_expr("value").expect("bare operand remains valid");
    assert!(matches!(plain, Expr::Path(_)));
    let delimiter = parse_expr("try )").expect_err("unexpected delimiter must fail");
    assert!(delimiter.range().start() >= 3);

    let maximum = format!("{}value", "try ".repeat(64));
    assert!(parse_expr(&maximum).is_ok());
    let over_limit = format!("{}value", "try ".repeat(65));
    let error = parse_expr(&over_limit).expect_err("prefix depth is bounded");
    assert_eq!(error.code(), "syntax.expr.prefix_depth_limit");
    assert_eq!(error.range(), TextRange::new(256, 259));
}

fn parse_propagation_fixture(
    source: impl Into<String>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(
                "arcweft-test://syntax/propagation-source-ranges",
            )
            .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path("propagation-source-ranges.arcw"),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
