use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::{
    AstNode, AttachedCallableParameter, AttachedCallableParameterKind, AttachedFunctionBody,
    AttachedPredicateBody, AttachedProofBody, FunctionItemKind, PredicateItemKind, ProofItemKind,
    ProofTrustSyntax,
};
use crate::assertion::AssertionMode;
use crate::attachment::node::{AssertionStatementKind, ProofCallStatementKind};
use crate::attachment::source_file::AttachedVisibilityKind;
use crate::attachment::{
    AttachedTypeFamily, BlockTailNode, GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId,
    SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/proof-callable-attachment-test").unwrap(),
            SourceName::path("proof-callable-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(181).unwrap());
    let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
    let snapshot = SyntaxSnapshotId::new(
        lineage,
        SourceSnapshotId::initial(document.display_name().clone()),
    );
    let identities = build
        .index()
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                entry.path().clone(),
                SyntaxNodeId::new(
                    lineage,
                    NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    attach_typed_tree(
        &build,
        &GrammarIdentityMap::new(identities),
        snapshot,
        document,
    )
    .unwrap()
}

fn proof(snapshot: &Arc<SyntaxSnapshotData>) -> AstNode<ProofItemKind> {
    snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::ProofItem)
        .unwrap()
        .cast()
        .unwrap()
}

fn predicate(snapshot: &Arc<SyntaxSnapshotData>) -> AstNode<PredicateItemKind> {
    snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::PredicateItem)
        .unwrap()
        .cast()
        .unwrap()
}

fn function(snapshot: &Arc<SyntaxSnapshotData>) -> AstNode<FunctionItemKind> {
    snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::FunctionItem)
        .unwrap()
        .cast()
        .unwrap()
}

#[test]
fn proof_attachment_owns_final_trust_and_consumes_the_generic_attribute() {
    let verified = attach("proof verified() = ()\n");
    let verified = proof(&verified).semantics().unwrap();
    assert!(matches!(verified.trust(), Some(ProofTrustSyntax::Verified)));
    assert!(verified.prefix().attributes().is_empty());

    let trusted = attach(concat!(
        "#[cfg(debug)]\n",
        "#[verify.trusted(reason = \"  reviewed ✓  \")]\n",
        "proof trusted() = ()\n",
    ));
    let trusted = proof(&trusted).semantics().unwrap();
    let Some(ProofTrustSyntax::Trusted { reason, .. }) = trusted.trust() else {
        panic!("expected typed trusted Proof")
    };
    assert_eq!(reason.as_str(), "  reviewed ✓  ");
    assert_eq!(trusted.prefix().attributes().len(), 1);
    assert_eq!(
        trusted.prefix().attributes()[0].path().segments()[0].source_text(),
        "cfg"
    );
    assert!(trusted.trust_attribute_source_span().is_some());
    assert!(trusted.trust_reason_source_span().is_some());
}

#[test]
fn malformed_trust_metadata_is_explicit_recovery_not_verified() {
    let snapshot = attach(concat!(
        "#[verify.trusted(reason = \"first\")]\n",
        "#[verify.trusted(reason = \"second\")]\n",
        "proof duplicate() = ()\n",
    ));
    let attached = proof(&snapshot).semantics().unwrap();

    assert!(attached.trust().is_none());
    assert!(attached.has_trust_recovery());
    assert!(attached.prefix().attributes().is_empty());
    assert!(attached.trust_attribute_source_span().is_none());
    assert!(attached.trust_reason_source_span().is_none());
}

#[test]
fn ordinary_function_attachment_owns_curried_parameters_contracts_and_block_only_body() {
    let snapshot = attach(concat!(
        "pub fn map<T>(value: T)(next: fn(T) -> T) -> T\n",
        "requires ready(value)\n",
        "ensures result == next(value)\n",
        "{ next(value) }\n",
    ));
    let attached = function(&snapshot).semantics().unwrap();
    assert_eq!(attached.parameter_groups().len(), 2);
    let parameters = attached.parameters().collect::<Vec<_>>();
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0].source_ordinal(), 0);
    assert_eq!(parameters[0].group_ordinal(), 0);
    assert_eq!(parameters[0].parameter_ordinal(), 0);
    assert_eq!(parameters[1].source_ordinal(), 1);
    assert_eq!(parameters[1].group_ordinal(), 1);
    assert_eq!(parameters[1].parameter_ordinal(), 0);
    assert!(attached.authored_return().is_some());
    assert_eq!(attached.contracts().len(), 2);
    assert!(attached.postcondition_result_source_span().is_some());
    let AttachedFunctionBody::Block { syntax, block } = attached.body() else {
        panic!("expected ordinary function block")
    };
    assert_eq!(syntax.kind(), SyntaxKind::FunctionBody);
    assert_eq!(block.kind(), SyntaxKind::Block);
    assert_ne!(syntax.id(), block.id());
}

#[test]
fn ordinary_function_attachment_preserves_fixed_default_and_rest_groups() {
    let source = "fn staged(first: I64)(second: I64 = seed + 1)(tail: ...I64) -> I64 { first }\n";
    let snapshot = attach(source);
    let attached = function(&snapshot).semantics().unwrap();
    assert!(!attached.has_parameter_shape_recovery());
    let [fixed_group, default_group, rest_group] = attached.parameter_groups() else {
        panic!("expected three authored curried parameter groups")
    };

    assert_eq!(fixed_group.source_ordinal(), 0);
    assert_eq!(default_group.source_ordinal(), 1);
    assert_eq!(rest_group.source_ordinal(), 2);

    let [fixed] = fixed_group.parameters() else {
        panic!("expected one fixed parameter")
    };
    assert_eq!(fixed.source_ordinal(), 0);
    assert_eq!(fixed.group_ordinal(), 0);
    assert_eq!(fixed.parameter_ordinal(), 0);
    assert!(matches!(fixed.kind(), AttachedCallableParameterKind::Fixed));
    assert!(fixed.default().is_none());
    assert!(!fixed.has_recovery());

    let [defaulted] = default_group.parameters() else {
        panic!("expected one defaulted parameter")
    };
    assert_eq!(defaulted.source_ordinal(), 1);
    assert_eq!(defaulted.group_ordinal(), 1);
    assert_eq!(defaulted.parameter_ordinal(), 0);
    assert!(matches!(
        defaulted.kind(),
        AttachedCallableParameterKind::Fixed
    ));
    let default = defaulted.default().expect("authored default");
    assert_eq!(default.equals().source_text(), "=");
    assert_eq!(
        default.value().whole_source_span().range(),
        SourceRange::new(
            source.find("seed + 1").unwrap(),
            source.find("seed + 1").unwrap() + "seed + 1".len(),
        )
    );
    assert_eq!(
        default.value().snapshot_id(),
        defaulted.syntax().snapshot_id()
    );
    assert_eq!(
        default.value().whole_source_span().source(),
        defaulted.syntax().source_span().source()
    );
    assert_ne!(default.value().id(), default.equals().id());
    assert!(!default.has_recovery());
    assert!(!defaulted.has_recovery());

    let [rest] = rest_group.parameters() else {
        panic!("expected one rest parameter")
    };
    assert_eq!(rest.source_ordinal(), 2);
    assert_eq!(rest.group_ordinal(), 2);
    assert_eq!(rest.parameter_ordinal(), 0);
    let AttachedCallableParameterKind::Rest { marker } = rest.kind() else {
        panic!("expected typed rest marker")
    };
    assert!(rest.is_rest());
    assert!(rest.default().is_none());
    assert_eq!(marker.source_text(), "...");
    let marker_start = source.find("...I64").unwrap();
    assert_eq!(
        marker.range(),
        SourceRange::new(marker_start, marker_start + "...".len())
    );
    assert_eq!(marker.snapshot_id(), rest.syntax().snapshot_id());
    assert!(!rest.has_recovery());
}

#[test]
fn ordinary_function_attachment_retains_a_missing_default_as_typed_recovery() {
    let snapshot = attach("fn recovered(value: Int = ) -> Int { value }\n");
    let attached = function(&snapshot).semantics().unwrap();
    let [group] = attached.parameter_groups() else {
        panic!("one fixed parameter group")
    };
    let [parameter] = group.parameters() else {
        panic!("one defaulted parameter")
    };
    let default = parameter.default().expect("authored equals owns a default");

    assert_eq!(
        default.value().syntax().kind(),
        SyntaxKind::MissingExpression
    );
    assert!(default.value().projection().has_recovery());
    assert!(default.has_recovery());
    assert!(parameter.has_recovery());
}

#[test]
fn ordinary_function_attachment_marks_invalid_rest_structure_as_owner_recovery() {
    for (source, rest_count, default_count) in [
        (
            "fn misplaced(values: ...I64, tail: I64) -> I64 { tail }\n",
            1,
            0,
        ),
        (
            "fn nonfinal(values: ...I64)(tail: I64) -> I64 { tail }\n",
            1,
            0,
        ),
        (
            "fn duplicate(first: ...I64, second: ...I64) -> I64 { first }\n",
            2,
            0,
        ),
        (
            "fn defaulted(values: ...I64 = fallback) -> I64 { fallback }\n",
            1,
            1,
        ),
    ] {
        let snapshot = attach(source);
        let attached = function(&snapshot).semantics().unwrap();

        assert!(
            attached.has_parameter_shape_recovery(),
            "invalid rest structure must recover at the Function owner: {source}"
        );
        assert_eq!(
            attached
                .parameters()
                .filter(|parameter| parameter.is_rest())
                .count(),
            rest_count
        );
        assert_eq!(
            attached
                .parameters()
                .filter(|parameter| parameter.default().is_some())
                .count(),
            default_count
        );
        assert!(
            attached
                .parameters()
                .all(|parameter| !parameter.has_recovery())
        );
        assert!(
            attached
                .parameters()
                .filter_map(|parameter| {
                    let AttachedCallableParameterKind::Rest { marker } = parameter.kind() else {
                        return None;
                    };
                    Some(marker)
                })
                .all(|marker| marker.source_text() == "...")
        );
    }
}

#[test]
fn ordinary_function_attachment_retains_missing_group_type_and_body_recovery() {
    let snapshot = attach("fn missing\n");
    let attached = function(&snapshot).semantics().unwrap();
    let [group] = attached.parameter_groups() else {
        panic!("missing function parameters require one typed recovery group")
    };
    assert!(group.has_recovery());
    assert!(group.parameters().is_empty());
    assert!(attached.authored_return().is_none());
    assert!(matches!(
        attached.body(),
        AttachedFunctionBody::Missing { .. }
    ));
}

#[test]
fn ordinary_function_attachment_owns_trailing_recovery() {
    let snapshot = attach("fn recovered() {} trailing\n");
    let attached = function(&snapshot).semantics().unwrap();
    let [recovery] = attached.trailing_recovery() else {
        panic!("ordinary Function must retain one typed trailing recovery")
    };

    assert_eq!(recovery.kind(), SyntaxKind::ErrorNode);
    assert_eq!(recovery.source_text(), "trailing\n");
}

fn assert_recovered_receiver_parameter(parameter: &AttachedCallableParameter) {
    assert!(parameter.has_recovery());
    assert!(!parameter.pattern().state().is_valid());
    assert_eq!(parameter.ty().family(), AttachedTypeFamily::Recovery);
    assert_eq!(parameter.ty().syntax().kind(), SyntaxKind::MissingType);
}

#[test]
fn predicate_and_proof_receiver_shapes_attach_as_typed_parameter_recovery() {
    for receiver in ["self", "mut self", "&self", "&mut self"] {
        let snapshot = attach(&format!("predicate recovered({receiver}) = true\n"));
        let attached = predicate(&snapshot).semantics().unwrap();
        let [parameter] = attached.parameter_group().parameters() else {
            panic!("predicate({receiver}) must retain one parameter")
        };
        assert_recovered_receiver_parameter(parameter);

        let snapshot = attach(&format!("proof recovered({receiver}) = ()\n"));
        let attached = proof(&snapshot).semantics().unwrap();
        let [parameter] = attached.parameter_group().parameters() else {
            panic!("proof({receiver}) must retain one parameter")
        };
        assert_recovered_receiver_parameter(parameter);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the table-driven header matrix keeps exact child ranges and all recovery roles together"
)]
fn predicate_proof_headers_bind_exact_children_and_malformed_recovery_roles() {
    enum RecoveryRole {
        MissingName,
        MissingGenericName,
        MissingParameterClose,
        MissingWhereColon,
        MissingBody,
        ExtraParameterGroup,
    }

    let source = concat!(
        "pub(super) proof parent_visible<'a, T>",
        "((left, right): (T, T), cmp: Comparator<T>) -> Bool ",
        "where T: Ord, Comparator<T>: Ready ",
        "requires cmp.ready() ensures result { left == right }\n",
    );
    let snapshot = attach(source);
    let attached = proof(&snapshot).semantics().unwrap();

    assert_eq!(attached.syntax().range(), SourceRange::new(0, 178));
    let visibility = attached
        .prefix()
        .visibility()
        .expect("authored pub(super) visibility");
    assert_eq!(visibility.kind(), AttachedVisibilityKind::Super);
    assert_eq!(visibility.syntax().range(), SourceRange::new(0, 10));
    assert_eq!(attached.name().syntax().range(), SourceRange::new(17, 31));

    let generics = attached.generics().expect("authored generic group");
    assert_eq!(generics.syntax().range(), SourceRange::new(31, 38));
    assert_eq!(generics.open().range(), SourceRange::new(31, 32));
    assert_eq!(generics.close().range(), SourceRange::new(37, 38));
    assert_eq!(
        generics
            .parameters()
            .iter()
            .map(|parameter| parameter.syntax().range())
            .collect::<Vec<_>>(),
        [SourceRange::new(32, 34), SourceRange::new(36, 37)]
    );
    assert!(!generics.has_recovery());

    let parameters = attached.parameter_group();
    assert_eq!(parameters.syntax().range(), SourceRange::new(38, 81));
    assert_eq!(parameters.open().range(), SourceRange::new(38, 39));
    assert_eq!(parameters.close().range(), SourceRange::new(80, 81));
    assert_eq!(
        parameters
            .parameters()
            .iter()
            .map(|parameter| parameter.syntax().range())
            .collect::<Vec<_>>(),
        [SourceRange::new(39, 60), SourceRange::new(62, 80)]
    );
    assert!(!parameters.has_recovery());

    let [where_clause] = attached.where_clauses() else {
        panic!("expected one where wrapper")
    };
    assert_eq!(where_clause.syntax().range(), SourceRange::new(90, 125));
    let [ordinal, comparator_bound] = where_clause.predicates() else {
        panic!("expected both source-ordered where predicates")
    };
    assert_eq!(ordinal.syntax().range(), SourceRange::new(96, 102));
    assert_eq!(
        comparator_bound.syntax().range(),
        SourceRange::new(104, 124)
    );
    assert!(!where_clause.has_recovery());

    let authored_return = attached
        .authored_return()
        .expect("authored Proof return type");
    assert_eq!(authored_return.syntax().range(), SourceRange::new(82, 89));

    let [requires, ensures] = attached.contracts() else {
        panic!("expected requires then ensures")
    };
    assert!(requires.is_requires());
    assert_eq!(
        requires.syntax_source_span().range(),
        SourceRange::new(125, 145)
    );
    assert!(ensures.is_ensures());
    assert_eq!(
        ensures.syntax_source_span().range(),
        SourceRange::new(146, 160)
    );

    let AttachedProofBody::Block { syntax, block } = attached.body() else {
        panic!("expected attached Proof block")
    };
    assert_eq!(syntax.range(), SourceRange::new(161, 178));
    assert_eq!(block.range(), SourceRange::new(161, 178));

    for (source, role, expected_recovery, expected_following) in [
        (
            "proof () = ()\nproof following() = ()\n",
            RecoveryRole::MissingName,
            SourceRange::new(6, 6),
            SourceRange::new(14, 36),
        ),
        (
            "proof broken<, T>() = ()\nproof following() = ()\n",
            RecoveryRole::MissingGenericName,
            SourceRange::new(13, 13),
            SourceRange::new(25, 47),
        ),
        (
            "proof broken(value: Int\nproof following() = ()\n",
            RecoveryRole::MissingParameterClose,
            SourceRange::new(24, 24),
            SourceRange::new(24, 46),
        ),
        (
            "proof broken() where T = ()\nproof following() = ()\n",
            RecoveryRole::MissingWhereColon,
            SourceRange::new(22, 22),
            SourceRange::new(28, 50),
        ),
        (
            "predicate broken()\nproof following() = ()\n",
            RecoveryRole::MissingBody,
            SourceRange::new(18, 18),
            SourceRange::new(19, 41),
        ),
        (
            "proof staged()(value: Int) = ()\nproof following() = ()\n",
            RecoveryRole::ExtraParameterGroup,
            SourceRange::new(14, 26),
            SourceRange::new(32, 54),
        ),
    ] {
        let snapshot = attach(source);
        let following = snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::ProofItem)
            .last()
            .unwrap()
            .cast::<ProofItemKind>()
            .unwrap();
        assert_eq!(following.range(), expected_following, "{source}");
        following.semantics().unwrap();

        let (owner, recovery, recovery_identity) = match role {
            RecoveryRole::MissingName => {
                let owner = proof(&snapshot).semantics().unwrap();
                (
                    owner.syntax().id(),
                    owner.name().syntax().range(),
                    Some(owner.name().syntax().id()),
                )
            }
            RecoveryRole::MissingGenericName => {
                let owner = proof(&snapshot).semantics().unwrap();
                let recovery = owner.generics().unwrap().parameters()[0].name().syntax();
                (owner.syntax().id(), recovery.range(), Some(recovery.id()))
            }
            RecoveryRole::MissingParameterClose => {
                let owner = proof(&snapshot).semantics().unwrap();
                (
                    owner.syntax().id(),
                    owner.parameter_group().close().range(),
                    Some(owner.parameter_group().close().id()),
                )
            }
            RecoveryRole::MissingWhereColon => {
                let owner = proof(&snapshot).semantics().unwrap();
                let recovery = owner.where_clauses()[0].predicates()[0]
                    .colon()
                    .source_span()
                    .range();
                (owner.syntax().id(), recovery, None)
            }
            RecoveryRole::MissingBody => {
                let owner = predicate(&snapshot).semantics().unwrap();
                let AttachedPredicateBody::Missing { missing, .. } = owner.body() else {
                    panic!("expected typed missing Predicate body")
                };
                (owner.syntax().id(), missing.range(), Some(missing.id()))
            }
            RecoveryRole::ExtraParameterGroup => {
                let owner = proof(&snapshot).semantics().unwrap();
                let [recovery] = owner.trailing_recovery() else {
                    panic!("expected one typed malformed-header recovery")
                };
                (owner.syntax().id(), recovery.range(), Some(recovery.id()))
            }
        };
        assert_eq!(recovery, expected_recovery, "{source}");
        if let Some(recovery_identity) = recovery_identity {
            assert_ne!(recovery_identity, following.id(), "{source}");
        }
        assert_ne!(owner, following.id(), "{source}");
    }
}

#[test]
fn proof_attachment_owns_return_contracts_and_proof_statement_families() {
    let snapshot = attach(concat!(
        "pub proof established<T>(value: T) -> T\n",
        "requires ready(value)\n",
        "ensures result == value\n",
        "{\n",
        "    lemma(value);\n",
        "    assert.prove(result == value);\n",
        "    value\n",
        "}\n",
    ));
    let attached = proof(&snapshot).semantics().unwrap();
    assert!(attached.authored_return().is_some());
    assert_eq!(attached.contracts().len(), 2);
    assert!(attached.contracts()[0].is_requires());
    assert!(attached.contracts()[1].is_ensures());

    let AttachedProofBody::Block { syntax, block } = attached.body() else {
        panic!("expected proof block");
    };
    assert_eq!(syntax.kind(), SyntaxKind::ProofBody);
    assert_ne!(syntax.id(), block.id());
    let statements = block.statements().unwrap();
    assert_eq!(statements.len(), 2);
    assert_eq!(statements[0].kind(), SyntaxKind::ProofCallStatement);
    assert_eq!(statements[1].kind(), SyntaxKind::AssertionStatement);
}

#[test]
fn proof_attachment_uses_parameter_end_for_the_implicit_unit_return() {
    let snapshot = attach("proof unit(value: Int) = ()\n");
    let attached = proof(&snapshot).semantics().unwrap();
    assert!(attached.authored_return().is_none());
    assert_eq!(
        attached.implicit_return_source_span(),
        attached.parameter_group().end_source_span()
    );
    assert_eq!(
        attached.ensures_scope_source_span(),
        attached.parameter_group().end_source_span()
    );
    let AttachedProofBody::Expression { syntax, expression } = attached.body() else {
        panic!("expected proof expression");
    };
    assert_eq!(syntax.kind(), SyntaxKind::ProofBody);
    assert_ne!(syntax.id(), expression.syntax().id());
}

#[test]
fn proof_attachment_uses_fallback_anchors_when_all_contracts_are_recovered() {
    let source = "proof fallback() -> Int\nrequires\nensures\n= 1\n";
    let snapshot = attach(source);
    let attached = proof(&snapshot).semantics().unwrap();
    assert_eq!(attached.contracts().len(), 2);
    assert!(
        attached
            .contracts()
            .iter()
            .all(super::AttachedCallableContractClause::has_recovery)
    );

    let parameter_end = source.find(" -> Int").unwrap();
    let return_end = source.find("Int").unwrap() + "Int".len();
    assert_eq!(
        attached.requires_scope_source_span().range().start(),
        parameter_end
    );
    assert_eq!(
        attached.ensures_scope_source_span().range().start(),
        return_end
    );
    assert_eq!(
        attached
            .postcondition_result_source_span()
            .expect("recovered ensures still owns result")
            .range()
            .start(),
        return_end
    );
}

#[test]
fn expression_body_and_one_expression_block_are_observably_distinct() {
    let expression_snapshot = attach("proof expression() = 1\n");
    let expression = proof(&expression_snapshot).semantics().unwrap();
    let AttachedProofBody::Expression {
        syntax: expression_body,
        expression: value,
    } = expression.body()
    else {
        panic!("expected expression Proof body")
    };
    assert_eq!(expression_body.kind(), SyntaxKind::ProofBody);
    assert_eq!(value.syntax().kind(), SyntaxKind::LiteralExpression);
    assert_ne!(expression_body.id(), value.syntax().id());

    let block_snapshot = attach("proof block() { 1 }\n");
    let block = proof(&block_snapshot).semantics().unwrap();
    assert_ne!(expression.body(), block.body());
    let AttachedProofBody::Block {
        syntax: block_body,
        block,
    } = block.body()
    else {
        panic!("expected block Proof body")
    };
    let BlockTailNode::Expression(tail) = block.tail().unwrap() else {
        panic!("one-expression block must retain an authored tail")
    };
    let distinct = [
        block_body.id(),
        block.id(),
        block.open_delimiter().unwrap().id(),
        block.close_delimiter().unwrap().id(),
        tail.id(),
    ];
    assert!(
        distinct
            .iter()
            .enumerate()
            .all(|(index, id)| distinct[..index].iter().all(|prior| prior != id))
    );
}

#[test]
fn proof_block_exact_shapes_and_ranges() {
    let source = "proof p() -> Int { let x: Int = 1; assert.prove(x == 1); x }\n";
    let snapshot = attach(source);
    let attached = proof(&snapshot).semantics().unwrap();
    assert_eq!(attached.syntax().range(), SourceRange::new(0, 60));
    assert_eq!(attached.syntax().source_text(), &source[..60]);
    let AttachedProofBody::Block { syntax, block } = attached.body() else {
        panic!("expected Proof block")
    };
    assert_ne!(syntax.id(), block.id());
    assert_eq!(block.range(), SourceRange::new(17, 60));
    assert_eq!(
        block.open_delimiter().unwrap().range(),
        SourceRange::new(17, 18)
    );
    assert_eq!(
        block.close_delimiter().unwrap().range(),
        SourceRange::new(59, 60)
    );

    let statements = block.statements().unwrap();
    assert_eq!(statements.len(), 2);
    assert_eq!(statements[0].kind(), SyntaxKind::LetStatement);
    assert_eq!(statements[0].range(), SourceRange::new(19, 34));
    assert_eq!(statements[1].kind(), SyntaxKind::AssertionStatement);
    assert_eq!(statements[1].range(), SourceRange::new(35, 56));
    let assertion = statements[1]
        .clone()
        .cast::<AssertionStatementKind>()
        .unwrap()
        .semantics()
        .unwrap();
    assert_eq!(assertion.mode().value(), Some(AssertionMode::Prove));
    assert_eq!(assertion.conditions().len(), 1);
    assert_eq!(
        assertion.conditions()[0].syntax().range(),
        SourceRange::new(48, 54)
    );
    let BlockTailNode::Expression(tail) = block.tail().unwrap() else {
        panic!("expected authored Proof tail")
    };
    assert_eq!(tail.range(), SourceRange::new(57, 58));
}

#[test]
fn predicate_block_exact_shapes_and_ranges() {
    let source = "predicate p(x: Int) { let y: Int = x; y > 0 }\n";
    let snapshot = attach(source);
    let attached = predicate(&snapshot).semantics().unwrap();
    assert_eq!(attached.syntax().range(), SourceRange::new(0, 45));
    assert_eq!(attached.syntax().source_text(), &source[..45]);
    let AttachedPredicateBody::Block { syntax, block } = attached.body() else {
        panic!("expected Predicate block")
    };
    assert_ne!(syntax.id(), block.id());
    assert_eq!(block.range(), SourceRange::new(20, 45));
    let statements = block.statements().unwrap();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0].kind(), SyntaxKind::LetStatement);
    assert_eq!(statements[0].range(), SourceRange::new(22, 37));
    assert!(
        statements
            .iter()
            .all(|statement| statement.kind() != SyntaxKind::AssertionStatement)
    );
    let BlockTailNode::Expression(tail) = block.tail().unwrap() else {
        panic!("expected authored Predicate tail")
    };
    assert_eq!(tail.range(), SourceRange::new(38, 43));
}

#[test]
fn missing_proof_block_close_does_not_absorb_the_following_item() {
    let source = "proof broken() -> Int { let x = ;\nproof next() = ()\n";
    let snapshot = attach(source);
    let proofs = snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::ProofItem)
        .map(|node| node.cast::<ProofItemKind>().unwrap())
        .collect::<Vec<_>>();
    let [broken, following] = proofs.as_slice() else {
        panic!("expected the recovered and following Proof items")
    };

    let missing_close = snapshot
        .nodes()
        .find(|node| {
            node.kind() == SyntaxKind::CloseBraceNode && node.range() == SourceRange::new(34, 34)
        })
        .expect("recovered Proof block owns a zero-width close at synchronization");
    assert_eq!(missing_close.range(), SourceRange::new(34, 34));
    assert_eq!(broken.range(), SourceRange::new(0, 34));
    assert_eq!(following.range(), SourceRange::new(34, 51));
    assert_eq!(following.source_text(), "proof next() = ()");
    assert_eq!(broken.source_text(), "proof broken() -> Int { let x = ;\n");
    assert!(!broken.source_text().contains("proof next"));
}

#[test]
fn empty_block_has_distinct_braces_and_omitted_tail() {
    let snapshot = attach("proof unit() {}\n");
    let attached = proof(&snapshot).semantics().unwrap();
    let AttachedProofBody::Block { syntax, block } = attached.body() else {
        panic!("expected empty Proof block")
    };
    let open = block.open_delimiter().unwrap();
    let close = block.close_delimiter().unwrap();
    let BlockTailNode::Omitted(tail) = block.tail().unwrap() else {
        panic!("empty Proof block must own an omitted tail")
    };
    assert_eq!(block.range(), SourceRange::new(13, 15));
    assert_eq!(open.range(), SourceRange::new(13, 14));
    assert_eq!(close.range(), SourceRange::new(14, 15));
    assert_eq!(tail.range(), SourceRange::new(14, 14));
    let identities = [syntax.id(), block.id(), open.id(), close.id(), tail.id()];
    assert!(
        identities
            .iter()
            .enumerate()
            .all(|(index, id)| identities[..index].iter().all(|prior| prior != id))
    );
}

#[test]
fn one_expression_block_retains_authored_tail_identity() {
    let snapshot = attach("proof unit() { 1 }\n");
    let attached = proof(&snapshot).semantics().unwrap();
    let AttachedProofBody::Block { block, .. } = attached.body() else {
        panic!("expected Proof block")
    };
    assert_eq!(block.range(), SourceRange::new(13, 18));
    assert!(block.statements().unwrap().is_empty());
    let BlockTailNode::Expression(tail) = block.tail().unwrap() else {
        panic!("authored tail must not become an omitted-tail node")
    };
    assert_eq!(tail.range(), SourceRange::new(15, 16));
    assert_eq!(tail.kind(), SyntaxKind::LiteralExpression);
}

#[test]
fn proof_call_statement_uses_existing_call_expression() {
    let snapshot = attach("proof calls(x: Int) { lemma(x); }\n");
    let attached = proof(&snapshot).semantics().unwrap();
    let AttachedProofBody::Block { block, .. } = attached.body() else {
        panic!("expected Proof block")
    };
    let statements = block.statements().unwrap();
    let [statement] = statements.as_slice() else {
        panic!("expected one Proof-call statement")
    };
    let proof_call = statement.clone().cast::<ProofCallStatementKind>().unwrap();
    let call = proof_call.callee().unwrap();
    assert_eq!(call.kind(), SyntaxKind::CallExpression);
    assert_eq!(call.range(), SourceRange::new(22, 30));
    assert_ne!(proof_call.id(), call.id());
}

#[test]
fn assert_prove_uses_existing_assertion_authority() {
    let source = "proof checks(left: Bool, right: Bool) { assert.prove(left, right); }\n";
    let snapshot = attach(source);
    let attached = proof(&snapshot).semantics().unwrap();
    let AttachedProofBody::Block { block, .. } = attached.body() else {
        panic!("expected Proof block")
    };
    let statements = block.statements().unwrap();
    let [statement] = statements.as_slice() else {
        panic!("expected one assertion statement")
    };
    let assertion = statement
        .clone()
        .cast::<AssertionStatementKind>()
        .unwrap()
        .semantics()
        .unwrap();
    assert_eq!(assertion.mode().value(), Some(AssertionMode::Prove));
    assert_eq!(
        assertion
            .conditions()
            .iter()
            .map(|condition| condition.syntax().source_text())
            .collect::<Vec<_>>(),
        ["left", "right"]
    );
    assert_eq!(
        assertion.conditions()[0].syntax().range(),
        SourceRange::new(53, 57)
    );
    assert_eq!(
        assertion.conditions()[1].syntax().range(),
        SourceRange::new(59, 64)
    );
    assert_ne!(
        assertion.syntax().id(),
        assertion.conditions()[0].syntax().id()
    );
    assert_ne!(
        assertion.conditions()[0].syntax().id(),
        assertion.conditions()[1].syntax().id()
    );
}
