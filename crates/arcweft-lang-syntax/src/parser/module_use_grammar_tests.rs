use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::PendingPathRoot;
use crate::incremental::SyntaxLimit;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/module-use-shadow").unwrap(),
        SourceName::path("module-use-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn green_kind_count(node: &rowan::GreenNodeData, kind: SyntaxKind) -> usize {
    usize::from(node.kind() == rowan::SyntaxKind(kind as u16))
        + node
            .children()
            .map(|child| match child {
                rowan::NodeOrToken::Node(child) => green_kind_count(child, kind),
                rowan::NodeOrToken::Token(_) => 0,
            })
            .sum::<usize>()
}

fn grouped_use(member_count: usize) -> String {
    let members = (0..member_count)
        .map(|ordinal| format!("name_{ordinal}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("use crate.large.{{{members}}}\n")
}

#[test]
fn module_and_use_families_emit_paths_groups_names_aliases_and_globs_losslessly() {
    let source = concat!(
        "mod crate.game.story\n",
        "pub use self.characters.{alice, bob as narrator}\n",
        "use super.common.route_gate as gate\n",
        "use crate.game.prelude.*\n",
        "fn next() {}\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ModuleDeclaration)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::UseDeclaration)
            .count(),
        3
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::Path)
            .count(),
        4
    );
    assert!(kinds.contains(&SyntaxKind::Visibility));
    assert_eq!(
        green_kind_count(built.green(), SyntaxKind::DelimitedGroup),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::NameReference)
            .count(),
        2
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::NameDefinition)
            .count(),
        3
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorNode));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_group_close_synchronizes_before_the_following_declaration() {
    let source = "use crate.game.{Hero, Villain\nproof next() = ()\n";
    let next_start = source.find("proof next").unwrap();
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::UseDeclaration)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        1
    );
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.use.missing_group_close"
            && diagnostic.range().start() == next_start
            && diagnostic.range().end() == next_start
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_module_path_does_not_consume_the_following_use() {
    let source = "mod\nuse self.characters.alice\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ModuleDeclaration)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::UseDeclaration)
            .count(),
        1
    );
    assert!(kinds.contains(&SyntaxKind::MissingName));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.module.missing_path")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_alias_name_is_typed_without_losing_the_import() {
    let source = "use crate.game.View as\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::Path));
    assert!(kinds.contains(&SyntaxKind::MissingName));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.use.missing_alias")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn grouped_use_member_limit_is_inclusive_and_one_over_is_fatal() {
    let exact = grouped_use(SyntaxLimit::DeclarationMembers.maximum());
    let built = parse_shadow_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("the exact grouped-use member limit builds");
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::NameReference)
            .count(),
        SyntaxLimit::DeclarationMembers.maximum()
    );

    let one_over = grouped_use(SyntaxLimit::DeclarationMembers.maximum() + 1);
    assert!(matches!(
        parse_shadow_document(&document(&one_over), crate::parser::ParseOptions::default(),),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
}

#[test]
fn source_header_phase_recovers_duplicate_and_late_headers_as_ordinary_items() {
    let source = concat!(
        "mod crate.story\n",
        "use self.characters\n",
        "fn first() {}\n",
        "mod crate.duplicate\n",
        "use self.late\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let module = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == SyntaxKind::ModuleDeclaration)
        .collect::<Vec<_>>();
    let uses = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == SyntaxKind::UseDeclaration)
        .collect::<Vec<_>>();
    let items = built
        .index()
        .entries()
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind(),
                SyntaxKind::FunctionItem | SyntaxKind::ErrorItem
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(module.len(), 1);
    assert_eq!(module[0].role(), SyntaxRole::Target);
    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].role(), SyntaxRole::Reference(0));
    assert_eq!(
        items
            .iter()
            .map(|entry| (entry.kind(), entry.role()))
            .collect::<Vec<_>>(),
        [
            (SyntaxKind::FunctionItem, SyntaxRole::Element(0)),
            (SyntaxKind::ErrorItem, SyntaxRole::Element(1)),
            (SyntaxKind::ErrorItem, SyntaxRole::Element(2)),
        ]
    );
    assert!(
        built.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == "syntax.source.duplicate_module_declaration"
        })
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.source.late_use_declaration")
    );
    assert_eq!(built.green().to_string(), source);

    let late_module_source = "use self.characters\nmod crate.late\n";
    let late_module = parse_shadow_document(
        &document(late_module_source),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert!(
        !late_module
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::ModuleDeclaration)
    );
    assert!(
        late_module
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.source.late_module_declaration" })
    );
    assert_eq!(late_module.green().to_string(), late_module_source);
}

#[test]
fn parent_root_normalizes_once_and_explicit_root_only_paths_recover_in_place() {
    let source = concat!(
        "mod parent.story\n",
        "use parent.parent\n",
        "use super.super.route\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let paths = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == SyntaxKind::Path)
        .map(|entry| entry.path_projection().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 3);
    assert!(matches!(
        paths[0].root(),
        PendingPathRoot::Super(levels) if levels.len() == 1
    ));
    assert!(matches!(
        paths[1].root(),
        PendingPathRoot::Super(levels) if levels.len() == 1
    ));
    assert_eq!(paths[1].segments().len(), 1);
    assert!(matches!(
        paths[2].root(),
        PendingPathRoot::Super(levels) if levels.len() == 2
    ));
    assert_eq!(built.green().to_string(), source);

    let root_only_source = concat!("mod crate\n", "use self\n", "use super\n", "use parent\n");
    let root_only = parse_shadow_document(
        &document(root_only_source),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
        root_only
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::MissingName)
            .count(),
        4
    );
    assert_eq!(
        root_only
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.path.missing_segment")
            .count(),
        4
    );
    assert_eq!(root_only.green().to_string(), root_only_source);
}

#[test]
fn visibility_projection_is_typed_and_invalid_scopes_use_ordinary_recovery() {
    let source = concat!(
        "pub use crate.public\n",
        "pub(crate) use crate.internal\n",
        "pub(super) use crate.parent\n",
        "pub(other) use crate.invalid\n",
    );
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let visibilities = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == SyntaxKind::Visibility)
        .collect::<Vec<_>>();
    assert_eq!(visibilities.len(), 4);
    assert!(
        visibilities
            .iter()
            .all(|entry| entry.visibility_projection().is_some())
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.visibility.invalid_scope")
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), source);
}
