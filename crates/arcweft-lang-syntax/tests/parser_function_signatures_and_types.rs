use arcweft_lang_syntax::{
    ast::{common::TextRange, items::Item},
    expr::{Expr, TryOperatorSource},
    parser::parse_source,
    reference::BorrowKind,
    types::{
        AuthoredTypeRef, FnParamKind, GenericParam, TypeRef, parse_fn_signature, parse_type_ref,
    },
};

#[test]
fn reference_types_preserve_shared_and_mutable_borrow_kinds() {
    assert!(matches!(
        parse_type_ref("&State").expect("shared reference parses").value(),
        TypeRef::Reference(reference)
            if reference.kind() == BorrowKind::Shared
                && reference.region().name().is_none()
                && matches!(reference.referent(), TypeRef::Path(path) if path.canonical_string() == "State")
    ));
    assert!(matches!(
        parse_type_ref("&'asset mut [Rgba8]").expect("mutable reference parses").value(),
        TypeRef::Reference(reference)
            if reference.kind() == BorrowKind::Mutable
            && reference.region().name().is_some_and(|lifetime| lifetime.name() == "asset")
            && matches!(reference.referent(), TypeRef::Slice(item)
                if matches!(item.as_ref(), TypeRef::Path(path) if path.canonical_string() == "Rgba8"))
    ));
    assert!(matches!(
        parse_type_ref("& mut State").expect("token-separated mutable reference parses").value(),
        TypeRef::Reference(reference) if reference.kind() == BorrowKind::Mutable
    ));
    let missing_referent =
        parse_type_ref("&mut").expect_err("mutable reference without referent is rejected");
    assert_eq!(
        missing_referent.code(),
        "syntax.type.reference_missing_referent"
    );
    assert_eq!(missing_referent.range(), Some(TextRange::new(4, 4)));
    assert!(matches!(
        parse_type_ref("&mutable").expect("identifier prefix remains a shared referent").value(),
        TypeRef::Reference(reference)
            if reference.kind() == BorrowKind::Shared
                && matches!(reference.referent(), TypeRef::Path(path) if path.canonical_string() == "mutable")
    ));

    let authored = parse_type_ref("&'asset mut State").expect("ranged reference parses");
    let TypeRef::Reference(reference) = authored.value() else {
        panic!("expected reference type");
    };
    assert_eq!(reference.amp_range().as_range(), 0..1);
    assert_eq!(reference.region().range().as_range(), 1..7);
    assert_eq!(reference.mut_range().unwrap().as_range(), 8..11);
    assert_eq!(reference.range().as_range(), 0..17);
}

#[test]
fn receiver_kinds_map_to_the_single_reference_borrow_kind() {
    use arcweft_lang_syntax::types::FnReceiverKind;

    assert_eq!(FnReceiverKind::Owned.borrow_kind(), None);
    assert_eq!(
        FnReceiverKind::SharedRef.borrow_kind(),
        Some(BorrowKind::Shared)
    );
    assert_eq!(
        FnReceiverKind::MutRef.borrow_kind(),
        Some(BorrowKind::Mutable)
    );
}

#[test]
fn function_signatures_keep_generics_curried_groups_and_where_clauses() {
    let signature = parse_fn_signature(
        "fn bind<'a, T>(state: &'a State)(route: T) -> ArcResult<T> where T: Clone + Debug",
    )
    .expect("curried generic signature parses");

    assert_eq!(signature.name(), "bind");
    assert!(matches!(
        &signature.generic_params()[0],
        GenericParam::Lifetime(lifetime) if lifetime.name() == "a"
    ));
    assert!(matches!(
        &signature.generic_params()[1],
        GenericParam::Type(param) if param.name().as_str() == "T"
    ));
    assert_eq!(signature.param_groups().len(), 2);
    assert_eq!(signature.param_groups()[0].params().len(), 1);
    assert_eq!(signature.param_groups()[1].params().len(), 1);
    assert!(matches!(
        signature.return_type().map(AuthoredTypeRef::value),
        Some(TypeRef::Generic { base, args }) if base.canonical_string() == "ArcResult" && args.len() == 1
    ));
    assert_eq!(signature.where_clauses().len(), 1);
    assert_eq!(signature.where_clauses()[0].bounds().len(), 2);
}

#[test]
fn function_types_are_right_associative_and_preserve_call_groups() {
    let authored = parse_type_ref("A -> B -> C").expect("function type parses");
    let TypeRef::Function {
        params,
        return_type,
        ..
    } = authored.value()
    else {
        panic!("expected function type");
    };
    assert!(matches!(params.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "A"));
    let authored = parse_type_ref("(A, B) -> C").expect("call-group function type parses");
    let TypeRef::Function {
        params,
        return_type,
        ..
    } = return_type.as_ref()
    else {
        panic!("expected right-associative return function");
    };
    assert!(matches!(params.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "B"));
    assert!(matches!(return_type.as_ref(), TypeRef::Path(path) if path.canonical_string() == "C"));

    let TypeRef::Function {
        params,
        return_type,
        ..
    } = authored.value()
    else {
        panic!("expected function type");
    };
    assert!(matches!(
        params.as_slice(),
        [TypeRef::Path(first), TypeRef::Path(second)]
            if first.canonical_string() == "A" && second.canonical_string() == "B"
    ));
    assert!(matches!(return_type.as_ref(), TypeRef::Path(path) if path.canonical_string() == "C"));

    assert!(matches!(
        parse_type_ref("(A, B)").expect("tuple type parses").value(),
        TypeRef::Tuple(items) if items.len() == 2
    ));

    let authored = parse_type_ref("Pair<A -> B, C -> D> -> E")
        .expect("outer function arrow ignores generic argument function arrows");
    let TypeRef::Function {
        params,
        return_type,
        ..
    } = authored.value()
    else {
        panic!("expected outer function type");
    };
    assert!(matches!(
        params.as_slice(),
        [TypeRef::Generic { base, args }]
            if base.canonical_string() == "Pair"
                && matches!(
                    args.as_slice(),
                    [
                        TypeRef::Function { params: first_params, return_type: first_return, .. },
                        TypeRef::Function { params: second_params, return_type: second_return, .. },
                    ] if matches!(first_params.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "A")
                        && matches!(first_return.as_ref(), TypeRef::Path(path) if path.canonical_string() == "B")
                        && matches!(second_params.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "C")
                        && matches!(second_return.as_ref(), TypeRef::Path(path) if path.canonical_string() == "D")
                )
    ));
    assert!(matches!(return_type.as_ref(), TypeRef::Path(path) if path.canonical_string() == "E"));
}

#[test]
fn canonical_type_labels_round_trip_precedence_sensitive_structure() {
    for source in [
        "&(A | B)",
        "(A -> B) | C",
        "(A | B)::Item",
        "(A -> B) -> C",
        "A -> (B -> C effects { io.read })",
        "A | (B | C)",
        "Vec<(A -> B) | C>",
    ] {
        let parsed = parse_type_ref(source).expect("precedence-sensitive type parses");
        let label = parsed.value().canonical_label();
        let reparsed = parse_type_ref(&label).unwrap_or_else(|error| {
            panic!("canonical label `{label}` for `{source}` must parse: {error}")
        });
        assert_eq!(
            reparsed.value(),
            parsed.value(),
            "canonical label changed `{source}`"
        );
        assert_eq!(reparsed.value().canonical_label(), label);
    }
}

#[test]
fn canonical_type_labels_do_not_collapse_distinct_type_trees() {
    for (left, right) in [
        ("&(A | B)", "&A | B"),
        ("(A -> B) | C", "A -> B | C"),
        ("(A | B)::Item", "A | B::Item"),
        ("(A -> B) -> C", "A -> B -> C"),
        (
            "A -> (B -> C effects { io.read })",
            "A -> B -> C effects { io.read }",
        ),
    ] {
        let left = parse_type_ref(left).expect("left type parses");
        let right = parse_type_ref(right).expect("right type parses");
        assert_ne!(
            left.value(),
            right.value(),
            "fixtures must describe distinct type trees"
        );
        assert_ne!(
            left.value().canonical_label(),
            right.value().canonical_label(),
            "distinct type trees need distinct canonical labels"
        );
    }
}

#[test]
fn function_signatures_keep_function_typed_parameters() {
    let signature = parse_fn_signature("fn map<A, B>(f: A -> B)(xs: Vec<A>) -> Vec<B>")
        .expect("function-typed curried signature parses");

    assert_eq!(signature.param_groups().len(), 2);
    assert!(matches!(
        signature.param_groups()[0].params()[0]
            .ty()
            .map(AuthoredTypeRef::value),
        Some(TypeRef::Function { params, return_type, .. })
            if params.len() == 1
                && matches!(&params[0], TypeRef::Path(path) if path.canonical_string() == "A")
                && matches!(return_type.as_ref(), TypeRef::Path(path) if path.canonical_string() == "B")
    ));
}

#[test]
fn type_parser_preserves_unregistered_names_as_nominal_type_paths() {
    assert!(matches!(
        parse_type_ref("ProjectFlag").expect("nominal type path parses").value(),
        TypeRef::Path(path) if path.canonical_string() == "ProjectFlag"
    ));
    assert!(matches!(
        parse_type_ref("Vec<ProjectFlag>").expect("nominal generic argument parses").value(),
        TypeRef::Generic { base, args }
            if base.canonical_string() == "Vec"
                && matches!(args.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "ProjectFlag")
    ));
    assert!(matches!(
        parse_type_ref("domain.ProjectFlag").expect("qualified nominal type parses").value(),
        TypeRef::Path(path) if path.canonical_string() == "domain.ProjectFlag"
    ));
}

#[test]
fn source_parser_uses_the_same_open_nominal_grammar_on_owned_type_surfaces() {
    for source in [
        "flow bad { let value: ProjectFlag = true }",
        "type Bad = ProjectFlag",
        "struct Bad { value: ProjectFlag }",
        "entry game @entry.bad {\nstate = ProjectFlag\ninitializer = init\nevent = ProjectFlag\nreducer = reduce\ngoto @flow.main\n}",
        "flow bad(value: ProjectFlag) {}",
        "fn bad(value: ProjectFlag) -> Unit {}",
        "trait Bad { fn value(input: ProjectFlag) -> Unit }",
        "impl Bad for Thing { fn value(input: ProjectFlag) -> Unit {} }",
        "extern rust mod bad from crate \"bad\" { pub fn value(input: ProjectFlag) -> Unit }",
    ] {
        let parsed = parse_source(source);
        assert!(
            parsed.errors().is_empty(),
            "nominal type path should parse uniformly for `{source}`: {:?}",
            parsed.errors()
        );
    }
}

#[test]
fn function_types_keep_closed_effect_rows() {
    let authored = parse_type_ref("String -> String effects { fs.read, state.write('flow) }")
        .expect("function type effect row parses");
    let TypeRef::Function {
        params,
        return_type,
        effects,
    } = authored.value()
    else {
        panic!("expected function type");
    };

    assert!(
        matches!(params.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "String")
    );
    assert!(
        matches!(return_type.as_ref(), TypeRef::Path(path) if path.canonical_string() == "String")
    );
    assert_eq!(
        effects.as_ref().expect("effect row is present").effects(),
        &["fs.read".to_owned(), "state.write('flow)".to_owned()]
    );

    let authored = parse_type_ref("(String -> String) effects { fs.read }")
        .expect("parenthesized function type effect row parses");
    let TypeRef::Function {
        effects,
        return_type,
        ..
    } = authored.value()
    else {
        panic!("expected function type");
    };
    assert_eq!(
        effects
            .as_ref()
            .expect("outer effect row is present")
            .effects(),
        &["fs.read".to_owned()]
    );
    assert!(
        matches!(return_type.as_ref(), TypeRef::Path(path) if path.canonical_string() == "String")
    );

    assert!(
        parse_type_ref("String effects { fs.read }")
            .expect_err("non-function effect row is rejected")
            .to_string()
            .contains("function type")
    );
    assert!(matches!(
        parse_type_ref("effects").expect("plain path named effects parses").value(),
        TypeRef::Path(path) if path.canonical_string() == "effects"
    ));
}

#[test]
fn flow_signatures_reject_curried_parameter_groups() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
flow opening(x: i32)(y: i32) {
  return x
}
",
    );

    assert_eq!(parsed.errors().len(), 1);
    assert!(parsed.errors()[0].message().contains("cannot be curried"));
    let Item::Flow(flow) = &parsed.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    assert!(flow.signature().is_none());
}

#[test]
fn flow_signature_separates_inline_effect_contract_after_return_type() {
    let parsed = parse_source(
        r"
flow health(req: HttpRequest) -> HttpResponse effects { http.respond } {
  return req
}
",
    );

    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Flow(flow) = &parsed.typed_tree().items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        flow.signature()
            .and_then(|signature| signature.return_type())
            .map(AuthoredTypeRef::value),
        Some(TypeRef::Path(path)) if path.canonical_string() == "HttpResponse"
    ));
    assert_eq!(flow.contracts().len(), 1);
}

#[test]
fn function_signatures_reject_trailing_garbage() {
    let error = parse_fn_signature("fn f(x: i32) -> i32 unexpected")
        .expect_err("trailing tokens after return type are rejected");

    assert!(error.to_string().contains("unexpected"));
}

#[test]
fn function_signatures_keep_rest_parameters() {
    let signature = parse_fn_signature("fn log(message: String, fields: ...LogField) -> Unit")
        .expect("rest parameter signature parses");
    let params = signature.param_groups()[0].params();

    assert_eq!(params[0].kind(), FnParamKind::Fixed);
    assert_eq!(params[1].kind(), FnParamKind::Rest);
    assert!(
        matches!(params[1].ty().map(AuthoredTypeRef::value), Some(TypeRef::Path(path)) if path.canonical_string() == "LogField")
    );
}

#[test]
fn function_signatures_reject_misplaced_rest_parameters() {
    let in_middle = parse_fn_signature("fn f(xs: ...i32, y: i32) -> Unit")
        .expect_err("rest in the middle is rejected");
    let curried = parse_fn_signature("fn f(xs: ...i32)(y: i32) -> Unit")
        .expect_err("rest before a curried group is rejected");
    let defaulted = parse_fn_signature("fn f(xs: ...i32 = []) -> Unit")
        .expect_err("defaulted rest is rejected");

    assert!(in_middle.to_string().contains("last parameter"));
    assert!(curried.to_string().contains("final group"));
    assert!(defaulted.to_string().contains("default"));
}

#[test]
fn source_function_annotations_keep_document_absolute_type_ranges() {
    let source = concat!(
        "// UTF-8 prefix 名前\n",
        "fn inspect<T: Missing>(名前: Missing, pair: (Missing, Missing)) -> Missing where T: Missing + Bound {\n",
        "    pair\n",
        "}\n",
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Function(function) = &parsed.typed_tree().items()[0] else {
        panic!("fixture must parse as a function")
    };
    let signature = function.signature();
    let params = signature.param_groups()[0].params();

    let slice = |range: TextRange| &source[range.start()..range.end()];
    assert_eq!(
        slice(
            *params[0]
                .ty()
                .expect("first annotation")
                .root_source()
                .whole()
        ),
        "Missing"
    );
    assert_eq!(
        slice(
            *params[1]
                .ty()
                .expect("second annotation")
                .root_source()
                .whole()
        ),
        "(Missing, Missing)"
    );
    assert_eq!(
        slice(
            *signature
                .return_type()
                .expect("return annotation")
                .root_source()
                .whole(),
        ),
        "Missing"
    );
    assert_eq!(
        slice(
            function
                .signature_source()
                .result()
                .expect("function signature return range"),
        ),
        "Missing"
    );
    let GenericParam::Type(type_parameter) = &signature.generic_params()[0] else {
        panic!("fixture generic must be a type parameter")
    };
    assert_eq!(slice(type_parameter.name_range()), "T");
    assert_eq!(slice(type_parameter.range()), "T: Missing");
    assert_eq!(
        slice(*type_parameter.bounds()[0].root_source().whole()),
        "Missing"
    );
    let predicate = &signature.where_clauses()[0];
    assert_eq!(slice(predicate.range()), "T: Missing + Bound");
    assert_eq!(
        predicate
            .bounds()
            .iter()
            .map(|bound| slice(*bound.root_source().whole()))
            .collect::<Vec<_>>(),
        vec!["Missing", "Bound"]
    );
}

#[test]
fn source_function_defaults_keep_document_absolute_expression_ranges() {
    let source = concat!(
        "// UTF-8 prefix 名前\n",
        "#[fx]\n",
        "fn inspect(value: i64 = input?) -> Unit {}\n",
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Function(function) = &parsed.typed_tree().items()[0] else {
        panic!("fixture must parse as a function")
    };
    let default = function.signature().param_groups()[0].params()[0]
        .default()
        .expect("default expression");
    let Expr::Try(tried) = default else {
        panic!("default must retain the typed Try node")
    };
    let operator = source.find('?').expect("question token");

    assert_eq!(
        tried.source().operator(),
        TryOperatorSource::PostfixQuestion {
            question: TextRange::new(operator, operator + 1),
        }
    );
    assert_eq!(
        &source[tried.source().whole().start()..tried.source().whole().end()],
        "input?"
    );
}

#[test]
fn function_signature_source_keeps_exact_result_range_without_a_prefix_line() {
    let source = "fn demo(value: Result<i64, String>) -> Result<i64, i64> {\n    Ok(value?)\n}\n";
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let Item::Function(function) = &parsed.typed_tree().items()[0] else {
        panic!("fixture must parse as a function")
    };
    let result = function
        .signature_source()
        .result()
        .expect("declared result source");

    assert_eq!(result, TextRange::new(39, 55));
    assert_eq!(&source[result.start()..result.end()], "Result<i64, i64>");
}
