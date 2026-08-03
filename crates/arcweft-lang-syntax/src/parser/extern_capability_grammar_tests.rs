use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

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

fn kind_roles(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> Vec<SyntaxRole> {
    entries
        .iter()
        .filter(|entry| entry.kind() == kind)
        .map(UnattachedGrammarEntry::role)
        .collect()
}

#[test]
fn capability_function_receiver_shape_requires_a_typed_pattern_annotation() {
    let source = "extern capability host {\n    fn invalid(self) -> Unit\n}\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();
    let pattern = entries
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::BindingPattern)
        .expect("receiver-shaped source retains a Binding Pattern");

    assert!(pattern.pattern_projection().is_some());
    assert_eq!(kind_count(entries, SyntaxKind::MissingType), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.parameter.missing_type")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn empty_capability_is_lossless_and_has_no_members() {
    let source = "extern capability empty {}\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::ExternCapabilityItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 0);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 0);
    assert_eq!(kind_count(entries, SyntaxKind::ErrorItem), 0);
    assert!(built.diagnostics().is_empty());
    assert_eq!(built.green().to_string(), source);
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
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    assert_eq!(
        entries
            .iter()
            .filter(|entry| {
                entry.kind() == SyntaxKind::PathExpression && entry.role() == SyntaxRole::Target
            })
            .count(),
        2
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn effect_braces_do_not_steal_grouped_expression_ordinals() {
    let source = r"extern capability host {
    fn send() effects { (net.connect), net.send }
}
";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(
        kind_roles(entries, SyntaxKind::SelectExpression),
        vec![SyntaxRole::Element(0), SyntaxRole::Element(1)]
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::PathExpression),
        vec![SyntaxRole::Target, SyntaxRole::Target]
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn documented_visible_members_interleave_and_keep_trailing_effect_commas() {
    let source = r"/// Selected-host boundary.
#[audit(external)]
pub extern capability host {
    /// First host-owned type.
    #[opaque]
    pub type Request

    /// First operation.
    #[audit(call)]
    pub fn send(request: Request) -> Unit
        effects { net.connect, net.send, }

    pub type Response = Result<String, HostError>
    pub fn finish(request: Request)(response: Response) -> Unit
}
";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(
        kind_roles(entries, SyntaxKind::TypeAliasItem),
        vec![SyntaxRole::Element(0), SyntaxRole::Element(2)]
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::FunctionItem),
        vec![SyntaxRole::Element(1), SyntaxRole::Element(3)]
    );
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 3);
    assert_eq!(
        entries
            .iter()
            .filter(|entry| {
                entry.kind() == SyntaxKind::PathExpression && entry.role() == SyntaxRole::Target
            })
            .count(),
        2
    );
    assert_eq!(kind_count(entries, SyntaxKind::ErrorItem), 0);
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
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
fn deleted_candidate_shapes_are_one_generic_error_member_each() {
    let candidates = [
        "policy",
        "policy {}",
        "policy legacy {}",
        "policy legacy { allow }",
        "policy legacy { allow = }",
        "policy legacy allow = fs.read",
        "policy legacy { allow = [fs.read",
        "policy legacy { allow = [{ effect = fs.read",
        "policy legacy { unknown = mystery }",
    ];

    for candidate in candidates {
        let source =
            format!("extern capability host {{\n    {candidate}\n    fn valid() -> Unit\n}}\n");
        let built =
            parse_shadow_document(&document(&source), crate::parser::ParseOptions::default())
                .unwrap();
        let entries = built.index().entries();
        let invalid = built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.capability.invalid_member")
            .collect::<Vec<_>>();

        assert_eq!(
            kind_roles(entries, SyntaxKind::ErrorItem),
            vec![SyntaxRole::Element(0)],
            "candidate: {candidate}"
        );
        assert_eq!(
            kind_roles(entries, SyntaxKind::FunctionItem),
            vec![SyntaxRole::Element(1)],
            "candidate: {candidate}"
        );
        assert_eq!(invalid.len(), 1, "candidate: {candidate}");
        assert_eq!(
            invalid[0].message(),
            "external capability bodies accept type and function declarations"
        );
        assert_eq!(
            &source[invalid[0].range().start()..invalid[0].range().end()],
            candidate,
            "candidate: {candidate}"
        );
        assert!(
            built.diagnostics().iter().all(|diagnostic| {
                let message = diagnostic.message().to_ascii_lowercase();
                !message.contains("removed") && !message.contains("deprecated")
            }),
            "candidate: {candidate}"
        );
        assert_eq!(built.green().to_string(), source, "candidate: {candidate}");
    }
}

#[test]
fn duplicate_contradictory_and_prefixed_candidates_remain_ordinary_members() {
    let source = r"extern capability host {
    policy duplicate
    policy duplicate
    policy allow = fs.read
    policy deny = fs.read
    /// Unsupported candidate remains attached to its recovery node.
    #[audit(external)]
    policy prefixed
    fn valid() -> Unit
}
";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();
    let invalid = built
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code() == "syntax.capability.invalid_member")
        .collect::<Vec<_>>();

    assert_eq!(invalid.len(), 5);
    assert!(
        invalid
            .windows(2)
            .all(|pair| pair[0].range().start() < pair[1].range().start())
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::ErrorItem),
        vec![
            SyntaxRole::Element(0),
            SyntaxRole::Element(1),
            SyntaxRole::Element(2),
            SyntaxRole::Element(3),
            SyntaxRole::Element(4),
        ]
    );
    assert_eq!(
        kind_roles(entries, SyntaxKind::FunctionItem),
        vec![SyntaxRole::Element(5)]
    );
    let prefixed = &invalid[4];
    assert_eq!(
        &source[prefixed.range().start()..prefixed.range().end()],
        "/// Unsupported candidate remains attached to its recovery node.\n    #[audit(external)]\n    policy prefixed"
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn candidate_tails_stay_inside_retained_type_and_function_members() {
    let source = r"extern capability host {
    type Request policy legacy
    fn send(request: Request) policy legacy
    fn valid() -> Unit
}
";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();
    let tails = built
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code() == "syntax.capability.invalid_member_tail")
        .collect::<Vec<_>>();

    assert_eq!(kind_count(entries, SyntaxKind::TypeAliasItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FunctionItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::ErrorItem), 0);
    assert_eq!(tails.len(), 2);
    assert_eq!(
        &source[tails[0].range().start()..tails[0].range().end()],
        "policy legacy"
    );
    assert_eq!(
        &source[tails[1].range().start()..tails[1].range().end()],
        "policy legacy"
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
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
