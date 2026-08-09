//! Attached semantic type nodes emitted from the canonical type transaction.

use std::sync::Arc;

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::shadow_recovery::trimmed_end;
use crate::ast::common::TextRange;
use crate::grammar::event::PendingSyntaxDiagnostic;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::types::{
    AuthoredTypeRef, TypeRef, TypeRefNodePath, TypeRefNodeStep, TypeToken, TypeTokenKind,
};

#[derive(Clone)]
pub(super) struct EmittedTypeProjection {
    tree: u64,
    authored: Arc<AuthoredTypeRef>,
    path: TypeRefNodePath,
}

/// One canonical type parse retained inside the active document transaction
/// until its attached nodes are emitted. This is parser-private preparation,
/// not a detached syntax reader.
pub(super) struct PreparedTypeProjection {
    start: usize,
    end: usize,
    authored: Arc<AuthoredTypeRef>,
}

impl PreparedTypeProjection {
    pub(super) const fn start(&self) -> usize {
        self.start
    }

    pub(super) const fn authored(&self) -> &Arc<AuthoredTypeRef> {
        &self.authored
    }

    pub(super) fn whole(&self) -> TextRange {
        *self.authored.root_source().whole()
    }
}

impl EmittedTypeProjection {
    pub(super) const fn tree(&self) -> u64 {
        self.tree
    }

    pub(super) const fn authored(&self) -> &Arc<AuthoredTypeRef> {
        &self.authored
    }

    pub(super) const fn path(&self) -> &TypeRefNodePath {
        &self.path
    }
}

pub(super) fn emit_type(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> EmittedTypeProjection {
    let end = trimmed_end(parser, parser.cursor(), end);
    let start = parser.cursor();
    if start >= end {
        let at = parser.current_offset();
        let authored = Arc::new(AuthoredTypeRef::recovery(
            recovery_index(parser),
            TextRange::new(at, at),
        ));
        let tree = projection_tree(parser);
        parser.start_type(
            SyntaxKind::MissingType,
            role,
            tree,
            Arc::clone(&authored),
            TypeRefNodePath::root(),
        );
        parser.finish();
        return EmittedTypeProjection {
            tree,
            authored,
            path: TypeRefNodePath::root(),
        };
    }

    let whole = significant_range(parser, start, end);
    match prepare_type(parser, start, end) {
        Ok(prepared) => emit_prepared_type(parser, role, prepared),
        Err(error) => {
            let recovery = error.range().unwrap_or(whole);
            let authored = Arc::new(AuthoredTypeRef::recovery_with_source(
                recovery_index(parser),
                whole,
                recovery,
            ));
            let tree = projection_tree(parser);
            parser.start_type(
                SyntaxKind::ErrorType,
                role,
                tree,
                Arc::clone(&authored),
                TypeRefNodePath::root(),
            );
            while parser.cursor() < end {
                parser.bump();
            }
            parser.push(crate::grammar::event::SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    error.code(),
                    SourceRange::new(recovery.start(), recovery.end()),
                    error.to_string(),
                ),
            ));
            parser.finish();
            EmittedTypeProjection {
                tree,
                authored,
                path: TypeRefNodePath::root(),
            }
        }
    }
}

/// Parses one exact token interval once so an enclosing grammar family can
/// commit it without reopening source text or running a second type grammar.
pub(super) fn prepare_type(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Result<PreparedTypeProjection, crate::types::TypeParseError> {
    crate::types::parse_tokens(&canonical_tokens(parser, start, end)).map(|authored| {
        PreparedTypeProjection {
            start,
            end,
            authored: Arc::new(authored),
        }
    })
}

/// Returns the exact parser-token boundary of one nominal type prefix.
///
/// Bodyless declaration grammars use this boundary to keep an initializer or
/// other trailing syntax outside the type node. The scan shares the canonical
/// type-token vocabulary and leaves the actual semantic type parse to
/// [`emit_type`], so it neither reopens source text nor creates a second type
/// grammar.
pub(super) fn nominal_type_prefix_end(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> usize {
    let tokens = canonical_indexed_tokens(parser, start, end);
    let Some(first) = tokens.first() else {
        return start;
    };
    if !matches!(first.kind, TypeTokenKind::Identifier(_)) {
        return end;
    }

    let mut cursor = 1;
    while cursor < tokens.len() {
        match tokens[cursor].kind {
            TypeTokenKind::Dot | TypeTokenKind::PathSeparator
                if tokens
                    .get(cursor + 1)
                    .is_some_and(|next| matches!(next.kind, TypeTokenKind::Identifier(_))) =>
            {
                cursor += 2;
            }
            TypeTokenKind::PathSeparator
                if tokens
                    .get(cursor + 1)
                    .is_some_and(|next| matches!(next.kind, TypeTokenKind::OpenAngle)) =>
            {
                cursor += 1;
                break;
            }
            TypeTokenKind::Dot | TypeTokenKind::PathSeparator => {
                return tokens[cursor].parser_index + 1;
            }
            _ => break,
        }
    }

    if tokens
        .get(cursor)
        .is_some_and(|token| matches!(token.kind, TypeTokenKind::OpenAngle))
    {
        let open = cursor;
        if let Some(close) = matching_angle_close(&tokens, open) {
            cursor = close + 1;
        } else {
            let recovery_end = tokens
                .iter()
                .skip(open + 1)
                .find(|token| {
                    matches!(token.kind, TypeTokenKind::Equals | TypeTokenKind::OpenBrace)
                })
                .map_or(end, |token| token.parser_index);
            return recovery_end;
        }
    }

    while tokens
        .get(cursor)
        .is_some_and(|token| matches!(token.kind, TypeTokenKind::PathSeparator))
        && tokens
            .get(cursor + 1)
            .is_some_and(|token| matches!(token.kind, TypeTokenKind::Identifier(_)))
    {
        cursor += 2;
    }

    tokens
        .get(cursor.saturating_sub(1))
        .map_or(start, |token| token.parser_index + 1)
}

/// Emits a type prepared by the same active grammar transaction.
pub(super) fn emit_prepared_type(
    parser: &mut DocumentParser<'_, '_>,
    role: SyntaxRole,
    prepared: PreparedTypeProjection,
) -> EmittedTypeProjection {
    assert_eq!(
        parser.cursor(),
        prepared.start,
        "prepared type emission starts at its canonical token boundary"
    );
    let tree = projection_tree(parser);
    emit_semantic_type(
        parser,
        prepared.end,
        role,
        tree,
        &prepared.authored,
        &TypeRefNodePath::root(),
    );
    EmittedTypeProjection {
        tree,
        authored: prepared.authored,
        path: TypeRefNodePath::root(),
    }
}

/// Emits the recovery type selected by a failed canonical type transaction
/// without reopening the source slice or running a second type parser.
pub(super) fn emit_recovered_type(
    parser: &mut DocumentParser<'_, '_>,
    role: SyntaxRole,
    start: usize,
    end: usize,
    error: &crate::types::TypeParseError,
) -> EmittedTypeProjection {
    assert_eq!(
        parser.cursor(),
        start,
        "recovered type emission starts at its canonical token boundary"
    );
    let whole = significant_range(parser, start, end);
    let recovery = error.range().unwrap_or(whole);
    let authored = Arc::new(AuthoredTypeRef::recovery_with_source(
        recovery_index(parser),
        whole,
        recovery,
    ));
    let tree = projection_tree(parser);
    parser.start_type(
        SyntaxKind::ErrorType,
        role,
        tree,
        Arc::clone(&authored),
        TypeRefNodePath::root(),
    );
    while parser.cursor() < end {
        parser.bump();
    }
    parser.push(crate::grammar::event::SyntaxEvent::Diagnostic(
        PendingSyntaxDiagnostic::new(
            error.code(),
            SourceRange::new(recovery.start(), recovery.end()),
            error.to_string(),
        ),
    ));
    parser.finish();
    EmittedTypeProjection {
        tree,
        authored,
        path: TypeRefNodePath::root(),
    }
}

fn emit_semantic_type(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    tree: u64,
    authored: &Arc<AuthoredTypeRef>,
    path: &TypeRefNodePath,
) {
    let mut pending = vec![SemanticTypeEmission::Enter {
        end,
        role,
        path: path.clone(),
    }];
    while let Some(emission) = pending.pop() {
        match emission {
            SemanticTypeEmission::Enter { end, role, path } => {
                let value = authored
                    .value_at(&path)
                    .expect("canonical source-map paths resolve semantic type nodes");
                parser.start_type(
                    semantic_kind(value),
                    role,
                    tree,
                    Arc::clone(authored),
                    path.clone(),
                );

                let mut children = immediate_children(value, &path, authored.as_ref());
                children.sort_by_key(|child| {
                    authored
                        .source_at(&child.path)
                        .expect("semantic child has source")
                        .whole()
                        .start()
                });
                pending.push(SemanticTypeEmission::FinishNode { end });
                pending.extend(children.into_iter().rev().map(|child| {
                    SemanticTypeEmission::EnterChild {
                        parent_end: end,
                        child,
                    }
                }));
            }
            SemanticTypeEmission::EnterChild { parent_end, child } => {
                let range = *authored
                    .source_at(&child.path)
                    .expect("semantic child has source")
                    .whole();
                bump_before(parser, range.start());
                if child.argument_wrapper {
                    parser.start(SyntaxKind::TypeArgument, child.wrapper_role);
                    pending.push(SemanticTypeEmission::FinishArgumentWrapper);
                }
                let child_end = parser
                    .token_boundary_index(range.end())
                    .unwrap_or_else(|| token_end_boundary(parser, range.end(), parent_end));
                pending.push(SemanticTypeEmission::Enter {
                    end: child_end,
                    role: child.role,
                    path: child.path,
                });
            }
            SemanticTypeEmission::FinishNode { end } => {
                while parser.cursor() < end {
                    parser.bump();
                }
                parser.finish();
            }
            SemanticTypeEmission::FinishArgumentWrapper => parser.finish(),
        }
    }
}

enum SemanticTypeEmission {
    Enter {
        end: usize,
        role: SyntaxRole,
        path: TypeRefNodePath,
    },
    EnterChild {
        parent_end: usize,
        child: SemanticChild,
    },
    FinishNode {
        end: usize,
    },
    FinishArgumentWrapper,
}

#[derive(Clone)]
struct SemanticChild {
    path: TypeRefNodePath,
    role: SyntaxRole,
    argument_wrapper: bool,
    wrapper_role: SyntaxRole,
}

fn immediate_children(
    value: &TypeRef,
    path: &TypeRefNodePath,
    authored: &AuthoredTypeRef,
) -> Vec<SemanticChild> {
    let mut children = Vec::new();
    match value {
        TypeRef::Tuple(items) => {
            indexed_children(
                &mut children,
                items.len(),
                path,
                TypeRefNodeStep::TupleItem,
                false,
            );
        }
        TypeRef::Function {
            params,
            return_type: _,
            ..
        } => {
            indexed_children(
                &mut children,
                params.len(),
                path,
                TypeRefNodeStep::FunctionParameter,
                false,
            );
            children.push(SemanticChild {
                path: path.child(TypeRefNodeStep::FunctionReturn),
                role: SyntaxRole::RightOperand,
                argument_wrapper: false,
                wrapper_role: SyntaxRole::Element(0),
            });
        }
        TypeRef::Choice(items) => {
            indexed_children(
                &mut children,
                items.len(),
                path,
                TypeRefNodeStep::ChoiceAlternative,
                false,
            );
        }
        TypeRef::Generic { args, .. } => {
            indexed_children(
                &mut children,
                args.len(),
                path,
                TypeRefNodeStep::GenericArgument,
                true,
            );
        }
        TypeRef::TraitBound(bound) => {
            indexed_children(
                &mut children,
                bound.args().len(),
                path,
                TypeRefNodeStep::TraitArgument,
                true,
            );
            for index in 0..bound.associated().len() {
                let ordinal = u16::try_from(index).expect("type limits fit structural ordinals");
                children.push(SemanticChild {
                    path: path.child(TypeRefNodeStep::AssociatedBinding(ordinal)),
                    role: SyntaxRole::Type,
                    argument_wrapper: true,
                    wrapper_role: SyntaxRole::Argument(
                        u16::try_from(bound.args().len() + index)
                            .expect("type limits fit argument roles"),
                    ),
                });
            }
        }
        TypeRef::Projection { .. } => children.push(SemanticChild {
            path: path.child(TypeRefNodeStep::ProjectionSubject),
            role: SyntaxRole::Operand,
            argument_wrapper: false,
            wrapper_role: SyntaxRole::Element(0),
        }),
        TypeRef::Reference(_) => children.push(SemanticChild {
            path: path.child(TypeRefNodeStep::ReferenceReferent),
            role: SyntaxRole::Operand,
            argument_wrapper: false,
            wrapper_role: SyntaxRole::Element(0),
        }),
        TypeRef::Slice(_) => children.push(SemanticChild {
            path: path.child(TypeRefNodeStep::SliceItem),
            role: SyntaxRole::Element(0),
            argument_wrapper: false,
            wrapper_role: SyntaxRole::Element(0),
        }),
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => {}
    }
    children.retain(|child| authored.source_at(&child.path).is_some());
    children
}

fn indexed_children(
    output: &mut Vec<SemanticChild>,
    len: usize,
    path: &TypeRefNodePath,
    step: fn(u16) -> TypeRefNodeStep,
    argument_wrapper: bool,
) {
    for index in 0..len {
        let ordinal = u16::try_from(index).expect("type limits fit structural ordinals");
        output.push(SemanticChild {
            path: path.child(step(ordinal)),
            role: if argument_wrapper {
                SyntaxRole::Type
            } else {
                SyntaxRole::Element(u32::from(ordinal))
            },
            argument_wrapper,
            wrapper_role: SyntaxRole::Argument(ordinal),
        });
    }
}

fn semantic_kind(value: &TypeRef) -> SyntaxKind {
    match value {
        TypeRef::Never | TypeRef::ConstInt(_) => SyntaxKind::PrimitiveType,
        TypeRef::Path(_) | TypeRef::Projection { .. } => SyntaxKind::PathType,
        TypeRef::Tuple(_) => SyntaxKind::TupleType,
        TypeRef::Function { .. } => SyntaxKind::FunctionType,
        TypeRef::Choice(_) => SyntaxKind::SumType,
        TypeRef::Generic { .. } | TypeRef::TraitBound(_) => SyntaxKind::GenericApplicationType,
        TypeRef::Reference(_) => SyntaxKind::ReferenceType,
        TypeRef::Slice(_) => SyntaxKind::SliceType,
        TypeRef::Recovery(_) => SyntaxKind::ErrorType,
    }
}

fn canonical_tokens<'source>(
    parser: &DocumentParser<'source, '_>,
    start: usize,
    end: usize,
) -> Vec<TypeToken<'source>> {
    canonical_indexed_tokens(parser, start, end)
        .into_iter()
        .map(|token| token.token)
        .collect()
}

#[derive(Clone, Copy)]
struct IndexedTypeToken<'source> {
    parser_index: usize,
    kind: TypeTokenKind<'source>,
    token: TypeToken<'source>,
}

fn canonical_indexed_tokens<'source>(
    parser: &DocumentParser<'source, '_>,
    start: usize,
    end: usize,
) -> Vec<IndexedTypeToken<'source>> {
    (start..end)
        .filter_map(|parser_index| {
            let token = parser.token_at(parser_index)?;
            let spelling = parser.text_of(token);
            let kind = match token.kind() {
                SyntaxKind::WhitespaceToken
                | SyntaxKind::NewlineToken
                | SyntaxKind::CommentToken
                | SyntaxKind::DocCommentToken => return None,
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken => {
                    TypeTokenKind::Identifier(spelling)
                }
                SyntaxKind::LifetimeToken => TypeTokenKind::Lifetime(spelling),
                SyntaxKind::NumberToken => TypeTokenKind::Integer(spelling),
                SyntaxKind::PunctuationToken => punctuation(spelling),
                _ => TypeTokenKind::Other,
            };
            Some(IndexedTypeToken {
                parser_index,
                kind,
                token: TypeToken::from_parser(
                    kind,
                    TextRange::new(token.range().start(), token.range().end()),
                ),
            })
        })
        .collect()
}

fn matching_angle_close(tokens: &[IndexedTypeToken<'_>], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.kind {
            TypeTokenKind::OpenAngle => depth = depth.checked_add(1)?,
            TypeTokenKind::CloseAngle => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn punctuation(spelling: &str) -> TypeTokenKind<'_> {
    match spelling {
        "!" => TypeTokenKind::Bang,
        "&" => TypeTokenKind::Ampersand,
        "(" => TypeTokenKind::OpenParen,
        ")" => TypeTokenKind::CloseParen,
        "[" => TypeTokenKind::OpenBracket,
        "]" => TypeTokenKind::CloseBracket,
        "{" => TypeTokenKind::OpenBrace,
        "}" => TypeTokenKind::CloseBrace,
        "<" => TypeTokenKind::OpenAngle,
        ">" => TypeTokenKind::CloseAngle,
        "," => TypeTokenKind::Comma,
        "." => TypeTokenKind::Dot,
        "::" => TypeTokenKind::PathSeparator,
        ":" => TypeTokenKind::Colon,
        "=" => TypeTokenKind::Equals,
        "|" => TypeTokenKind::Pipe,
        "->" => TypeTokenKind::ThinArrow,
        _ => TypeTokenKind::Other,
    }
}

fn significant_range(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> TextRange {
    let first = (start..end)
        .filter_map(|index| parser.token_at(index))
        .find(|token| !is_trivia(token.kind()));
    let last = (start..end)
        .rev()
        .filter_map(|index| parser.token_at(index))
        .find(|token| !is_trivia(token.kind()));
    if let (Some(first), Some(last)) = (first, last) {
        TextRange::new(first.range().start(), last.range().end())
    } else {
        let at = parser.offset_at_token_boundary(start).unwrap_or(0);
        TextRange::new(at, at)
    }
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

fn bump_before(parser: &mut DocumentParser<'_, '_>, offset: usize) {
    while parser
        .current()
        .is_some_and(|token| token.range().end() <= offset)
    {
        parser.bump();
    }
}

fn token_end_boundary(
    parser: &DocumentParser<'_, '_>,
    offset: usize,
    fallback_end: usize,
) -> usize {
    (parser.cursor()..fallback_end)
        .find(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| token.range().start() >= offset)
        })
        .unwrap_or(fallback_end)
}

fn projection_tree(parser: &DocumentParser<'_, '_>) -> u64 {
    u64::try_from(parser.event_position()).expect("grammar event limits fit projection identity")
}

fn recovery_index(parser: &DocumentParser<'_, '_>) -> u32 {
    u32::try_from(parser.event_position()).unwrap_or(u32::MAX)
}
