use crate::{
    ast::common::TextRange,
    expr::{Expr, parse_expr_at},
};

#[test]
fn expression_entity_ranges_are_rebased_to_their_nonzero_source_base() {
    let Expr::EntityRef(absolute) =
        parse_expr_at("@entry.main", 40).expect("absolute entity expression")
    else {
        panic!("expected absolute entity reference");
    };
    let absolute = absolute.as_absolute().expect("absolute entity");
    assert_eq!(*absolute.range(), TextRange::new(40, 51));
    assert_eq!(absolute.authored_body_range(), Some(TextRange::new(41, 51)));

    let Expr::EntityRef(delimited) =
        parse_expr_at("@<entry.main>", 60).expect("delimited entity expression")
    else {
        panic!("expected delimited entity reference");
    };
    let delimited = delimited.as_absolute().expect("absolute entity");
    assert_eq!(*delimited.range(), TextRange::new(60, 73));
    assert_eq!(
        delimited.authored_body_range(),
        Some(TextRange::new(62, 72))
    );
}

#[test]
fn family_relative_expression_ranges_rebase_without_claiming_an_absolute_body() {
    let Expr::EntityRef(relative) =
        parse_expr_at("@entry:.main", 25).expect("family-relative entity expression")
    else {
        panic!("expected family-relative entity reference");
    };
    assert_eq!(*relative.range(), TextRange::new(25, 37));
    let relative = relative.family_relative_ref().expect("relative entity");
    assert_eq!(*relative.relative().range(), TextRange::new(32, 37));
    assert_eq!(relative.canonical_body(), "entry.main");
}
