use arcweft_lang_syntax::{
    ast::items::Item,
    parser::parse_source,
    types::{FnParamKind, GenericParam, TypeRef, parse_fn_signature, parse_type_ref},
};

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
        GenericParam::Type(param) if param.name() == "T"
    ));
    assert_eq!(signature.param_groups().len(), 2);
    assert_eq!(signature.param_groups()[0].params().len(), 1);
    assert_eq!(signature.param_groups()[1].params().len(), 1);
    assert!(matches!(
        signature.return_type(),
        Some(TypeRef::Generic { base, args }) if base == "ArcResult" && args.len() == 1
    ));
    assert_eq!(signature.where_clauses().len(), 1);
    assert_eq!(signature.where_clauses()[0].bounds().len(), 2);
}

#[test]
fn function_types_are_right_associative_and_preserve_call_groups() {
    let TypeRef::Function {
        params,
        return_type,
        ..
    } = parse_type_ref("A -> B -> C").expect("function type parses")
    else {
        panic!("expected function type");
    };
    assert_eq!(params, vec![TypeRef::Path("A".to_owned())]);
    let TypeRef::Function {
        params,
        return_type,
        ..
    } = return_type.as_ref()
    else {
        panic!("expected right-associative return function");
    };
    assert_eq!(params, &[TypeRef::Path("B".to_owned())]);
    assert_eq!(return_type.as_ref(), &TypeRef::Path("C".to_owned()));

    let TypeRef::Function {
        params,
        return_type,
        ..
    } = parse_type_ref("(A, B) -> C").expect("call-group function type parses")
    else {
        panic!("expected function type");
    };
    assert_eq!(
        params,
        vec![TypeRef::Path("A".to_owned()), TypeRef::Path("B".to_owned())]
    );
    assert_eq!(return_type.as_ref(), &TypeRef::Path("C".to_owned()));

    assert!(matches!(
        parse_type_ref("(A, B)").expect("tuple type parses"),
        TypeRef::Tuple(items) if items.len() == 2
    ));

    let TypeRef::Function {
        params,
        return_type,
        ..
    } = parse_type_ref("Pair<A -> B, C -> D> -> E")
        .expect("outer function arrow ignores generic argument function arrows")
    else {
        panic!("expected outer function type");
    };
    assert!(matches!(
        params.as_slice(),
        [TypeRef::Generic { base, args }]
            if base == "Pair"
                && matches!(
                    args.as_slice(),
                    [
                        TypeRef::Function { params: first_params, return_type: first_return, .. },
                        TypeRef::Function { params: second_params, return_type: second_return, .. },
                    ] if first_params == &[TypeRef::Path("A".to_owned())]
                        && first_return.as_ref() == &TypeRef::Path("B".to_owned())
                        && second_params == &[TypeRef::Path("C".to_owned())]
                        && second_return.as_ref() == &TypeRef::Path("D".to_owned())
                )
    ));
    assert_eq!(return_type.as_ref(), &TypeRef::Path("E".to_owned()));
}

#[test]
fn function_signatures_keep_function_typed_parameters() {
    let signature = parse_fn_signature("fn map<A, B>(f: A -> B)(xs: Vec<A>) -> Vec<B>")
        .expect("function-typed curried signature parses");

    assert_eq!(signature.param_groups().len(), 2);
    assert!(matches!(
        signature.param_groups()[0].params()[0].ty(),
        TypeRef::Function { params, return_type, .. }
            if params.len() == 1
                && matches!(&params[0], TypeRef::Path(path) if path == "A")
                && matches!(return_type.as_ref(), TypeRef::Path(path) if path == "B")
    ));
}

#[test]
fn type_parser_preserves_unregistered_names_as_nominal_type_paths() {
    assert!(matches!(
        parse_type_ref("ProjectFlag").expect("nominal type path parses"),
        TypeRef::Path(path) if path == "ProjectFlag"
    ));
    assert!(matches!(
        parse_type_ref("Vec<ProjectFlag>").expect("nominal generic argument parses"),
        TypeRef::Generic { base, args }
            if base == "Vec" && args == vec![TypeRef::Path("ProjectFlag".to_owned())]
    ));
    assert!(matches!(
        parse_type_ref("domain.ProjectFlag").expect("qualified nominal type parses"),
        TypeRef::Path(path) if path == "domain.ProjectFlag"
    ));
}

#[test]
fn source_parser_uses_the_same_open_nominal_grammar_on_owned_type_surfaces() {
    for source in [
        "flow bad { let value: ProjectFlag = true }",
        "type Bad = ProjectFlag",
        "struct Bad { value: ProjectFlag }",
        "state Bad { value: ProjectFlag = true }",
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
    let TypeRef::Function {
        params,
        return_type,
        effects,
    } = parse_type_ref("String -> String effects { fs.read, state.write('flow) }")
        .expect("function type effect row parses")
    else {
        panic!("expected function type");
    };

    assert_eq!(params, vec![TypeRef::Path("String".to_owned())]);
    assert_eq!(return_type.as_ref(), &TypeRef::Path("String".to_owned()));
    assert_eq!(
        effects.expect("effect row is present").effects(),
        &["fs.read".to_owned(), "state.write('flow)".to_owned()]
    );

    let TypeRef::Function {
        effects,
        return_type,
        ..
    } = parse_type_ref("(String -> String) effects { fs.read }")
        .expect("parenthesized function type effect row parses")
    else {
        panic!("expected function type");
    };
    assert_eq!(
        effects.expect("outer effect row is present").effects(),
        &["fs.read".to_owned()]
    );
    assert_eq!(return_type.as_ref(), &TypeRef::Path("String".to_owned()));

    assert!(
        parse_type_ref("String effects { fs.read }")
            .expect_err("non-function effect row is rejected")
            .to_string()
            .contains("function type")
    );
    assert!(matches!(
        parse_type_ref("effects").expect("plain path named effects parses"),
        TypeRef::Path(path) if path == "effects"
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
        flow.signature().and_then(|signature| signature.return_type()),
        Some(TypeRef::Path(path)) if path == "HttpResponse"
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
    assert!(matches!(params[1].ty(), TypeRef::Path(path) if path == "LogField"));
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
