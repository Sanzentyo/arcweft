//! Top-level item classification for the private full-source grammar.

use super::lexer::LexToken;
use crate::grammar::kinds::SyntaxKind;

pub(super) fn classify_top_level_item(source: &str, tokens: &[LexToken]) -> Option<SyntaxKind> {
    let significant = tokens
        .iter()
        .filter(|token| !is_trivia_kind(token.kind))
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
    if let Some((declaration_keyword, kind)) = declaration_kind_at_start(source, &significant) {
        if significant
            .get(declaration_keyword + 1)
            .is_some_and(|token| matches!(&source[token.range.as_range()], "." | "::"))
        {
            return Some(SyntaxKind::ErrorItem);
        }
        if matches!(kind, SyntaxKind::PredicateItem | SyntaxKind::ProofItem)
            && declaration_name_is_entity_reference(source, &significant)
        {
            return Some(SyntaxKind::ErrorItem);
        }
        return Some(kind);
    }
    Some(SyntaxKind::ErrorItem)
}

fn declaration_kind_at_start(source: &str, tokens: &[&LexToken]) -> Option<(usize, SyntaxKind)> {
    let mut keyword = 0_usize;
    if token_text(source, tokens, keyword) == Some("pub") {
        keyword += 1;
        if token_text(source, tokens, keyword) == Some("(") {
            let mut depth = 1_usize;
            keyword += 1;
            while let Some(spelling) = token_text(source, tokens, keyword) {
                match spelling {
                    "(" => depth += 1,
                    ")" => {
                        depth -= 1;
                        if depth == 0 {
                            keyword += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                keyword += 1;
            }
            if depth != 0 {
                return None;
            }
        }
    }

    let spelling = token_text(source, tokens, keyword)?;
    let kind = declaration_kind_from_head(
        spelling,
        token_text(source, tokens, keyword + 1) == Some("capability"),
    )?;
    Some((keyword, kind))
}

fn token_text<'source>(
    source: &'source str,
    tokens: &[&LexToken],
    index: usize,
) -> Option<&'source str> {
    let token = tokens.get(index)?;
    Some(&source[token.range.as_range()])
}

fn declaration_name_is_entity_reference(source: &str, tokens: &[&LexToken]) -> bool {
    tokens
        .iter()
        .position(|token| matches!(&source[token.range.as_range()], "predicate" | "proof"))
        .and_then(|keyword| tokens.get(keyword + 1))
        .is_some_and(|token| token.kind == SyntaxKind::EntityReferenceToken)
}

fn declaration_kind_from_head(keyword: &str, extern_capability: bool) -> Option<SyntaxKind> {
    Some(match keyword {
        "mod" => SyntaxKind::ModuleDeclaration,
        "use" => SyntaxKind::UseDeclaration,
        "flow" => SyntaxKind::FlowItem,
        "fn" => SyntaxKind::FunctionItem,
        "predicate" => SyntaxKind::PredicateItem,
        "proof" => SyntaxKind::ProofItem,
        "trait" => SyntaxKind::TraitItem,
        "impl" => SyntaxKind::ImplItem,
        "enum" => SyntaxKind::EnumItem,
        "struct" => SyntaxKind::StructItem,
        "type" => SyntaxKind::TypeAliasItem,
        "res" => SyntaxKind::ResourceDeclarationItem,
        "character" => SyntaxKind::CharacterDeclarationItem,
        "view" => SyntaxKind::ViewDeclarationItem,
        "action" => SyntaxKind::ActionDeclarationItem,
        "activity" => SyntaxKind::ActivityDeclarationItem,
        "signal" => SyntaxKind::SignalDeclarationItem,
        "metric" => SyntaxKind::MetricDeclarationItem,
        "layer" => SyntaxKind::LayerDeclarationItem,
        "entry" => SyntaxKind::EntryDeclarationItem,
        "extern" if extern_capability => SyntaxKind::ExternCapabilityItem,
        "test" => SyntaxKind::TestItem,
        "bench" => SyntaxKind::BenchItem,
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
            | SyntaxKind::TraitItem
            | SyntaxKind::ImplItem
            | SyntaxKind::EnumItem
            | SyntaxKind::StructItem
            | SyntaxKind::TypeAliasItem
            | SyntaxKind::ResourceDeclarationItem
            | SyntaxKind::CharacterDeclarationItem
            | SyntaxKind::ViewDeclarationItem
            | SyntaxKind::ActionDeclarationItem
            | SyntaxKind::ActivityDeclarationItem
            | SyntaxKind::SignalDeclarationItem
            | SyntaxKind::MetricDeclarationItem
            | SyntaxKind::LayerDeclarationItem
            | SyntaxKind::EntryDeclarationItem
            | SyntaxKind::ExternCapabilityItem
            | SyntaxKind::TestItem
            | SyntaxKind::BenchItem
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
