use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::{
    AstNode, AttachedCallableParameter, AttachedCallableParameterKind, AttachedFunctionBody,
    AttachedProofBody, FunctionItemKind, PredicateItemKind, ProofItemKind,
};
use crate::attachment::{
    AttachedTypeFamily, GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId,
    SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_shadow_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/proof-callable-attachment-test").unwrap(),
            SourceName::path("proof-callable-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
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
            .all(|clause| clause.has_recovery())
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
