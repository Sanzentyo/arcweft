use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/flow-shadow").unwrap(),
        SourceName::path("flow-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn flow_header_and_body_share_typed_declaration_descendants() {
    let source = r"/// Opens the generated route.
#[generated]
pub flow @flow.opening opening<'a, T>(state: &'a State) -> Result<T, Error>
where T: Clone + Debug
effects { asset.read, audio.play }
requires state.ready()
ensures result.is_ok()
{
    let next: T = state.current
    return next
}
";
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    for expected in [
        SyntaxKind::FlowItem,
        SyntaxKind::DocBlock,
        SyntaxKind::OuterAttribute,
        SyntaxKind::Visibility,
        SyntaxKind::NameDefinition,
        SyntaxKind::GenericParameterGroup,
        SyntaxKind::LifetimeParameter,
        SyntaxKind::TypeParameter,
        SyntaxKind::FixedParameterGroup,
        SyntaxKind::ReturnType,
        SyntaxKind::WhereClause,
        SyntaxKind::RequiresClause,
        SyntaxKind::EnsuresClause,
        SyntaxKind::FlowBody,
        SyntaxKind::Block,
        SyntaxKind::LetStatement,
        SyntaxKind::ReturnStatement,
    ] {
        assert!(
            entries.iter().any(|entry| entry.kind() == expected),
            "missing {expected:?}: kinds={:?}, diagnostics={:?}",
            entries
                .iter()
                .map(UnattachedGrammarEntry::kind)
                .collect::<Vec<_>>(),
            built.diagnostics(),
        );
    }
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 1);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_identity_forms_distinguish_authored_and_implicit_names() {
    let source = concat!(
        "flow opening {}\n",
        "flow @flow.other {}\n",
        "flow @flow.generated generated {}\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 3);
    assert_eq!(kind_count(entries, SyntaxKind::NameDefinition), 2);
    assert_eq!(kind_count(entries, SyntaxKind::MissingName), 0);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn curried_flow_group_is_recovery_and_does_not_hide_the_following_item() {
    let source = concat!(
        "flow invalid(first: Int)(second: Int) -> Int { return first }\n",
        "proof next() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 2);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::ErrorNode)
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.decl.invalid_header")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_flow_identity_and_body_recover_before_the_following_item() {
    let source = "flow\nproof next() = ()\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingName), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.decl.missing_name")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.flow.missing_body")
    );
    assert_eq!(built.green().to_string(), source);
}
