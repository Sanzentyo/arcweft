use super::super::lexer::lex_source;
use super::super::{TypeTokenCursor, TypeTokenKind};
use crate::types::{TypeRef, TypeRefLexemeKind};

#[test]
fn typed_receiver_transaction_commits_only_on_success() {
    for (source, expected_end, expected_generic) in [
        ("String.with_capacity(64)", 6, false),
        ("pkg::types::Vec<I32>.with_capacity(8)", 20, true),
        ("Vec::<I32>::with_capacity(8)", 10, true),
    ] {
        let tokens = lex_source(source, 0).expect("fixture lexes");
        let cursor = TypeTokenCursor::try_new(&tokens, 0).expect("token view is valid");
        let first = cursor
            .parse_receiver()
            .expect("transaction is well-formed")
            .expect("fixture has a type receiver");
        let second = cursor
            .parse_receiver()
            .expect("repeated transaction is well-formed")
            .expect("immutable cursor returns the same receiver");

        assert_eq!(first.receiver_end(), expected_end);
        assert_eq!(first.explicit_generic(), expected_generic);
        assert_eq!(first.next_index(), second.next_index());
        assert_eq!(first.authored(), second.authored());
        assert_eq!(tokens[first.next_index()].range.start(), expected_end);
    }

    for source in [
        "String::with_capacity(64)",
        "Bytes::with_capacity(8)",
        "Vec::with_capacity(8)",
        "factory().with_capacity(8)",
        "a < b > (c)",
    ] {
        let tokens = lex_source(source, 0).expect("ordinary fixture lexes");
        let cursor = TypeTokenCursor::try_new(&tokens, 0).expect("token view is valid");
        assert!(
            cursor
                .parse_receiver()
                .expect("ordinary fixture is not a type parse failure")
                .is_none(),
            "{source} must not publish an associated receiver",
        );
    }

    let malformed = lex_source("Vec<,T>.with_capacity(8)", 0).expect("tokens are retained");
    assert!(
        TypeTokenCursor::try_new(&malformed, 0)
            .expect("token view is valid")
            .parse_receiver()
            .is_err(),
        "malformed generic syntax publishes no partial receiver",
    );
}

#[test]
fn ordinary_turbofish_generic_callee_uses_same_token_grammar() {
    for source in [
        "foo::<T>(value)",
        "registry::resolve::<Option<Result<T,E>>>(value)",
    ] {
        let tokens = lex_source(source, 0).expect("fixture lexes");
        let parsed = TypeTokenCursor::try_new(&tokens, 0)
            .expect("token view is valid")
            .parse_generic_callee()
            .expect("generic callee transaction succeeds")
            .expect("fixture has an ordinary turbofish callee");
        assert!(matches!(parsed.authored().value(), TypeRef::Generic { .. }));
        assert!(
            parsed
                .authored()
                .source()
                .lexemes()
                .iter()
                .any(|lexeme| { lexeme.kind() == &TypeRefLexemeKind::TurbofishSeparator })
        );
        assert!(matches!(
            tokens[parsed.next_index()].kind,
            TypeTokenKind::OpenParen
        ));
    }

    for source in [
        "a < b > (c)",
        "foo<T>(value)",
        "factory(value)",
        "items.map::<T>(value)",
    ] {
        let tokens = lex_source(source, 0).expect("ordinary fixture lexes");
        assert!(
            TypeTokenCursor::try_new(&tokens, 0)
                .expect("token view is valid")
                .parse_generic_callee()
                .expect("ordinary fixture is not a type parse failure")
                .is_none(),
            "{source} must remain ordinary expression grammar",
        );
    }
}

#[test]
fn selected_generic_member_commits_only_before_a_call() {
    for source in [
        "collect<Vec<T>>()",
        "resolve<Option<Result<T,E>>>(value)",
        "map::<Result<T, E>>(value)",
    ] {
        let tokens = lex_source(source, 0).expect("fixture lexes");
        let parsed = TypeTokenCursor::try_new(&tokens, 0)
            .expect("token view is valid")
            .parse_generic_member()
            .expect("generic member transaction succeeds")
            .expect("fixture has a generic member");
        assert!(matches!(parsed.authored().value(), TypeRef::Generic { .. }));
        assert!(matches!(
            tokens[parsed.next_index()].kind,
            TypeTokenKind::OpenParen
        ));
    }

    for source in ["member < T", "member(value)"] {
        let tokens = lex_source(source, 0).expect("ordinary fixture lexes");
        assert!(
            TypeTokenCursor::try_new(&tokens, 0)
                .expect("token view is valid")
                .parse_generic_member()
                .expect("ordinary fixture is not a type parse failure")
                .is_none(),
            "{source} must remain outside direct generic member grammar",
        );
    }
}
