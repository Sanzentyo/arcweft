use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/extern-capability-shadow").unwrap(),
        SourceName::path("extern-capability-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn capability_types_functions_effects_and_curried_parameters_are_typed_and_lossless() {
    let source = r"/// host filesystem boundary
#[audit(external)]
pub extern capability fs {
    type FsError

    fn read_text(path: VirtualPath) -> Need<String, FsError>
        effects { fs.read, log.write }

    fn combine<T>(left: T)(right: T) -> T
}
";
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ExternCapabilityItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::DocBlock), 1);
    assert_eq!(kind_count(entries, SyntaxKind::OuterAttribute), 1);
    assert_eq!(kind_count(entries, SyntaxKind::Visibility), 1);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 3);
    assert_eq!(kind_count(entries, SyntaxKind::GenericParameterGroup), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ReturnType), 2);
    assert_eq!(kind_count(entries, SyntaxKind::PathExpression), 2);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_capability_name_and_body_have_zero_width_owned_recovery() {
    let source = concat!(
        "extern capability {}\n",
        "extern capability fs\n",
        "proof next() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ExternCapabilityItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingName), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 1);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.decl.missing_name"
            && diagnostic.range() == SourceRange::new(18, 18)
    }));
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.capability.missing_body"
            && diagnostic.range() == SourceRange::new(42, 42)
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn invalid_members_and_unbraced_effects_recover_before_later_functions() {
    let source = r"pub extern capability fs {
    const unsupported = 1
    fn broken(path: String)
        effects fs.read
    fn valid(path: String) -> String
}
";
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ExternCapabilityItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ErrorItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 2);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.capability.invalid_member" })
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.capability.effects_requires_braces" })
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.capability.invalid_member_tail" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_effects_recover_before_the_next_member_without_stealing_the_outer_close() {
    let source = r"extern capability fs {
    fn broken(path: String)
        effects { fs.read
    fn valid(path: String) -> String
}
";
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ExternCapabilityItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 2);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.capability.missing_effects_close" })
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .all(|diagnostic| { diagnostic.code() != "syntax.capability.missing_body_close" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_capability_body_synchronizes_before_the_following_proof() {
    let source = concat!(
        "extern capability fs {\n",
        "    type FsError\n",
        "proof next() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ExternCapabilityItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.capability.missing_body_close"
            && diagnostic.range() == SourceRange::new(40, 40)
    }));
    assert_eq!(built.green().to_string(), source);
}
