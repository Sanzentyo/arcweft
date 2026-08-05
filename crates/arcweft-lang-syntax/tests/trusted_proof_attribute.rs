use arcweft_lang_syntax::{
    ast::common::TextRange,
    ast::{items::Item, proof::ProofTrust},
    parser::recovery::ParseErrorKind,
};

fn assert_trusted_proof_rejected(source: &str, expected: ParseErrorKind) {
    let parsed = parse_trusted_proof_fixture(source);
    assert!(
        parsed.errors().iter().any(|error| error.kind() == expected),
        "missing {expected:?} in {:?}",
        parsed.errors()
    );
    assert!(
        !parsed
            .typed_tree()
            .items()
            .iter()
            .any(|item| matches!(item, Item::Proof(_))),
        "invalid trust metadata produced a typed proof"
    );
}

#[test]
fn trusted_proof_retains_the_exact_nonempty_reason() {
    let parsed = parse_trusted_proof_fixture(
        r#"
#[verify.trusted(reason = "  signed external review  ")]
proof external_review {
    check external_review_is_valid()
}
"#,
    );

    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let [Item::Proof(proof)] = parsed.typed_tree().items() else {
        panic!("expected one proof declaration");
    };
    assert!(matches!(
        proof.trust(),
        ProofTrust::Trusted { reason, attribute_range }
            if reason == "  signed external review  "
                && *attribute_range == TextRange::new(1, 57)
    ));

    let escaped = parse_trusted_proof_fixture(
        r#"
#[verify.trusted(reason = "line\nreview $(literal text)")]
proof escaped_review {
    check external_review_is_valid()
}
"#,
    );
    assert!(escaped.errors().is_empty(), "{:?}", escaped.errors());
    let [Item::Proof(proof)] = escaped.typed_tree().items() else {
        panic!("expected one proof declaration");
    };
    assert!(matches!(
        proof.trust(),
        ProofTrust::Trusted { reason, .. } if reason == "line\nreview $(literal text)"
    ));
}

#[test]
fn bare_proof_name_normalizes_to_proof_family_with_exact_name_range() {
    let source = "  proof external_review {\n    check valid()\n}\n";
    let parsed = parse_trusted_proof_fixture(source);

    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let [Item::Proof(proof)] = parsed.typed_tree().items() else {
        panic!("expected one proof declaration");
    };
    assert_eq!(proof.id().body(), "proof.external_review");
    assert_eq!(&source[proof.id().range().as_range()], "external_review");
}

#[test]
fn explicit_proof_identity_is_accepted_with_the_local_name() {
    for (source, expected_id, expected_id_source) in [
        (
            "proof @proof.external_review external_review {\n    check valid()\n}\n",
            "proof.external_review",
            "@proof.external_review",
        ),
        (
            "proof @proof:.relative_review relative_review {\n    check valid()\n}\n",
            "proof.relative_review",
            "@proof:.relative_review",
        ),
    ] {
        let parsed = parse_trusted_proof_fixture(source);

        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let [Item::Proof(proof)] = parsed.typed_tree().items() else {
            panic!("expected one proof declaration");
        };
        assert_eq!(proof.id().body(), expected_id);
        assert_eq!(proof.name(), expected_id.rsplit('.').next().unwrap());
        assert_eq!(&source[proof.id().range().as_range()], expected_id_source);
    }
}

#[test]
fn malformed_trusted_proof_arguments_have_typed_diagnostics() {
    for (source, expected) in [
        (
            r"
#[verify.trusted]
proof missing_reason {
    check valid()
}
",
            ParseErrorKind::ProofTrustedReasonMissing,
        ),
        (
            r"
#[verify.trusted()]
proof empty_arguments {
    check valid()
}
",
            ParseErrorKind::ProofTrustedReasonMissing,
        ),
        (
            r#"
#[verify.trusted(reason = "first")]
#[verify.trusted(reason = "second")]
proof duplicate_attribute {
    check valid()
}
"#,
            ParseErrorKind::ProofTrustedDuplicate,
        ),
        (
            r#"
#[verify.trusted(reason = "first", reason = "second")]
proof duplicate_reason {
    check valid()
}
"#,
            ParseErrorKind::ProofTrustedReasonDuplicate,
        ),
        (
            r"
#[verify.trusted(reason = true)]
proof non_string_reason {
    check valid()
}
",
            ParseErrorKind::ProofTrustedReasonNotString,
        ),
        (
            r"
#[verify.trusted(reason = build_review_reason())]
proof expression_reason {
    check valid()
}
",
            ParseErrorKind::ProofTrustedReasonNotString,
        ),
        (
            r#"
#[verify.trusted(reason = "")]
proof empty_reason {
    check valid()
}
"#,
            ParseErrorKind::ProofTrustedReasonEmpty,
        ),
        (
            "
#[verify.trusted(reason = \"\u{2003}\u{00a0}\")]
proof whitespace_reason {
    check valid()
}
",
            ParseErrorKind::ProofTrustedReasonEmpty,
        ),
        (
            r#"
#[verify.trusted(evidence = "external")]
proof unknown_argument {
    check valid()
}
"#,
            ParseErrorKind::ProofTrustedUnknownArgument,
        ),
        (
            r#"
#[verify.trusted("external")]
proof positional_argument {
    check valid()
}
"#,
            ParseErrorKind::ProofTrustedPositionalArgument,
        ),
    ] {
        assert_trusted_proof_rejected(source, expected);
    }
}

#[test]
fn trusted_attribute_is_reserved_for_proofs() {
    let parsed = parse_trusted_proof_fixture(
        r#"
#[verify.trusted(reason = "external")]
fn ordinary() {}
"#,
    );

    assert!(parsed.errors().iter().any(|error| {
        error.kind() == ParseErrorKind::ProofTrustedNotProof
            && error.code() == "syntax.proof.trusted.not_proof"
    }));
    let [Item::Function(function)] = parsed.typed_tree().items() else {
        panic!("expected the ordinary function to remain parseable");
    };
    assert!(
        function
            .attrs()
            .iter()
            .all(|attribute| attribute.name() != "verify.trusted")
    );

    let source_attribute = parse_trusted_proof_fixture(
        r#"
#![verify.trusted(reason = "external")]
fn ordinary() {}
"#,
    );
    assert!(
        source_attribute
            .errors()
            .iter()
            .any(|error| { error.kind() == ParseErrorKind::ProofTrustedNotProof })
    );
    assert!(source_attribute.typed_tree().attrs().is_empty());
}

#[test]
fn removed_trusted_axiom_tokens_use_ordinary_grammar_recovery() {
    let parsed = parse_trusted_proof_fixture(
        r#"
trusted axiom @axiom.external {
    reason = "external"
}
"#,
    );

    assert!(!parsed.errors().is_empty());
    assert!(
        parsed
            .errors()
            .iter()
            .all(|error| error.kind() == ParseErrorKind::Generic)
    );
    assert!(
        !parsed
            .typed_tree()
            .items()
            .iter()
            .any(|item| matches!(item, Item::Proof(_)))
    );
}

fn parse_trusted_proof_fixture(
    source: impl Into<String>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(
                "arcweft-test://syntax/trusted-proof-attribute",
            )
            .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path("trusted-proof-attribute.arcw"),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
