//! Top-level item classification for the private full-source grammar.

use super::lexer::LexToken;
use crate::grammar::kinds::SyntaxKind;

pub(super) fn classify_top_level_item(source: &str, tokens: &[LexToken]) -> Option<SyntaxKind> {
    let significant = tokens
        .iter()
        .filter(|token| !is_trivia_kind(token.kind))
        .collect::<Vec<_>>();
    let spellings = significant
        .iter()
        .copied()
        .filter(|token| token.kind == SyntaxKind::KeywordToken)
        .map(|token| &source[token.range.as_range()])
        .collect::<Vec<_>>();
    let first = *significant.first()?;
    let first_text = &source[first.range.as_range()];
    if first_text == "#" {
        return Some(
            significant
                .get(1)
                .copied()
                .filter(|token| &source[token.range.as_range()] == "!")
                .map_or(SyntaxKind::OuterAttribute, |_| SyntaxKind::InnerAttribute),
        );
    }
    if let Some(kind) = declaration_kind(&spellings) {
        if matches!(kind, SyntaxKind::PredicateItem | SyntaxKind::ProofItem)
            && declaration_name_is_entity_reference(source, &significant)
        {
            return Some(SyntaxKind::ErrorItem);
        }
        return Some(kind);
    }
    Some(
        if is_flow_statement_head(first_text)
            || matches!(
                first.kind,
                SyntaxKind::IdentifierToken | SyntaxKind::EntityReferenceToken
            )
        {
            SyntaxKind::TopLevelFlowItem
        } else {
            SyntaxKind::ErrorItem
        },
    )
}

fn declaration_name_is_entity_reference(source: &str, tokens: &[&LexToken]) -> bool {
    tokens
        .iter()
        .position(|token| matches!(&source[token.range.as_range()], "predicate" | "proof"))
        .and_then(|keyword| tokens.get(keyword + 1))
        .is_some_and(|token| token.kind == SyntaxKind::EntityReferenceToken)
}

pub(super) fn declaration_kind(keywords: &[&str]) -> Option<SyntaxKind> {
    let keyword = keywords
        .iter()
        .copied()
        .find(|keyword| !matches!(*keyword, "pub" | "crate" | "super"))?;
    Some(match keyword {
        "mod" => SyntaxKind::ModuleDeclaration,
        "use" => SyntaxKind::UseDeclaration,
        "flow" => SyntaxKind::FlowItem,
        "fn" => SyntaxKind::FunctionItem,
        "predicate" => SyntaxKind::PredicateItem,
        "proof" => SyntaxKind::ProofItem,
        "agent" => SyntaxKind::AgentItem,
        "callable" => SyntaxKind::CallableItem,
        "state" => SyntaxKind::StateItem,
        "trait" => SyntaxKind::TraitItem,
        "impl" => SyntaxKind::ImplItem,
        "enum" => SyntaxKind::EnumItem,
        "struct" => SyntaxKind::StructItem,
        "type" => SyntaxKind::TypeAliasItem,
        "entity" => SyntaxKind::EntityDeclarationItem,
        "entry" => SyntaxKind::EntryDeclarationItem,
        "extern" if keywords.contains(&"capability") => SyntaxKind::ExternCapabilityItem,
        "extern" if keywords.contains(&"mod") => SyntaxKind::ExternModuleItem,
        "hook" => SyntaxKind::HookItem,
        "dialogue" if keywords.contains(&"defaults") => SyntaxKind::DialogueDefaultsItem,
        "memo" if keywords.contains(&"fn") => SyntaxKind::MemoFunctionItem,
        "test" => SyntaxKind::TestItem,
        "bench" => SyntaxKind::BenchItem,
        "parser" => SyntaxKind::ParserItem,
        "source" => SyntaxKind::SourceItem,
        "style" => SyntaxKind::StyleItem,
        _ => return None,
    })
}

pub(super) const fn is_declaration_item_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ModuleDeclaration
            | SyntaxKind::UseDeclaration
            | SyntaxKind::FlowItem
            | SyntaxKind::FunctionItem
            | SyntaxKind::PredicateItem
            | SyntaxKind::ProofItem
            | SyntaxKind::AgentItem
            | SyntaxKind::CallableItem
            | SyntaxKind::StateItem
            | SyntaxKind::TraitItem
            | SyntaxKind::ImplItem
            | SyntaxKind::EnumItem
            | SyntaxKind::StructItem
            | SyntaxKind::TypeAliasItem
            | SyntaxKind::EntityDeclarationItem
            | SyntaxKind::EntryDeclarationItem
            | SyntaxKind::ExternCapabilityItem
            | SyntaxKind::ExternModuleItem
            | SyntaxKind::HookItem
            | SyntaxKind::DialogueDefaultsItem
            | SyntaxKind::MemoFunctionItem
            | SyntaxKind::TestItem
            | SyntaxKind::BenchItem
            | SyntaxKind::ParserItem
            | SyntaxKind::SourceItem
            | SyntaxKind::StyleItem
    )
}

const fn is_trivia_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

fn is_flow_statement_head(spelling: &str) -> bool {
    matches!(
        spelling,
        "assert"
            | "await"
            | "break"
            | "choice"
            | "close"
            | "continue"
            | "defer"
            | "for"
            | "goto"
            | "if"
            | "let"
            | "loop"
            | "match"
            | "on"
            | "out"
            | "return"
            | "select"
            | "signal"
            | "thread"
            | "unsafe"
            | "wait"
            | "while"
            | "yield"
    )
}
