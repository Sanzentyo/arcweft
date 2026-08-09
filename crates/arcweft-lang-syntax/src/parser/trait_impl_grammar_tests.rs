use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/trait-impl-shadow").unwrap(),
        SourceName::path("trait-impl-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn trait_and_impl_members_keep_associated_types_curried_functions_and_bounds() {
    let source = r"trait SourceLike {
    type Item
    fn current(self) -> Self::Item
}

impl<T> SourceLike for Box<T>
where T: Copyable
{
    type Item = T
    fn current(self) -> T { self.value }
}

trait Threshold {
    fn above(self, min: i64)(value: i64) -> bool
}

impl Threshold for Score {
    fn above(self, min: i64)(value: i64) -> bool {
        value >= min
    }
}
";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::TraitItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::ImplItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 4);
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 6);
    assert_eq!(kind_count(entries, SyntaxKind::WherePredicate), 1);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::GenericApplicationType)
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn trait_receivers_use_binding_patterns_without_inventing_parameter_types() {
    let source = r"trait ReceiverForms {
    fn owned(self) -> Self
    fn owned_mut(mut self) -> Self
    fn shared(&self) -> Self
    fn exclusive(&mut self) -> Self
}
";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 4);
    assert_eq!(kind_count(entries, SyntaxKind::BindingPattern), 3);
    assert_eq!(kind_count(entries, SyntaxKind::MutableBindingPattern), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingType), 0);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn member_prefixes_and_semicolon_separators_keep_distinct_typed_members() {
    let source = r"trait Decorated {
    /// associated value
    #[meta(fn)]
    type Item; fn current(&self) -> Self::Item;
}
";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::DocBlock), 1);
    assert_eq!(kind_count(entries, SyntaxKind::OuterAttribute), 1);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 1);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_impl_associated_target_is_typed_without_losing_the_next_item() {
    let source = concat!(
        "impl SourceLike for Broken {\n",
        "    type Item\n",
        "    fn current(self) -> T\n",
        "}\n",
        "proof next() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ImplItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::MissingType)
    );
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 0);
    assert!(
        built.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == "syntax.impl.missing_associated_type_target"
        })
    );
    assert_eq!(built.diagnostics().len(), 1);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn invalid_member_recovers_at_the_next_member_line() {
    let source = r"trait Broken {
    const unsupported = 1
    type Item
    fn current(self) -> Self::Item
}
";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ErrorItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 1);
    assert_eq!(built.diagnostics().len(), 1);
    assert_eq!(
        built.diagnostics()[0].code(),
        "syntax.trait_impl.invalid_member"
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_impl_synchronizes_before_the_following_declaration() {
    let source = concat!(
        "impl SourceLike for Broken {\n",
        "    type Item = String\n",
        "proof next() = ()\n",
    );
    let next = source.find("proof next").unwrap();
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ImplItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.trait_impl.missing_body_close"
            && diagnostic.range().start() == next
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn associated_type_and_method_tails_are_typed_recovery_nodes() {
    let source = concat!(
        "trait Broken {\n",
        "    type Item unexpected\n",
        "    fn current(self) -> T { self } unexpected\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ErrorNode), 2);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.trait_impl.invalid_member_tail")
            .count(),
        2
    );
    assert_eq!(built.green().to_string(), source);
}
