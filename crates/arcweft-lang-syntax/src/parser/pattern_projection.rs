//! Semantic Pattern construction coupled to the active grammar transaction.

use std::sync::Arc;

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::lexer::{LexToken, LiteralLexemePart, typed_entity_reference, typed_literal};
use super::shadow_recovery::{find_matching_close, first_significant, token_text};
use super::type_ref::EmittedTypeProjection;
use crate::ast::symbol_path::ProjectSymbolSegment;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::id_ref::SyntaxIdRefSyntax;
use crate::literal::{SyntaxLiteralSyntax, SyntaxLiteralValue};
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::patterns::{
    AuthoredPattern, PatternBindingIssue, PatternBindingSyntax, PatternComponentRole,
    PatternComponentSource, PatternFieldPart, PatternLiteralPart, PatternNameSyntax,
    PatternNodePath, PatternPath, PatternPathIssue, PatternPathRecovery, PatternPathRoot,
    PatternPathSegment, PatternPathSyntax, PatternRecordFieldIssue, PatternRecordFieldSyntax,
    PatternRecoveryIssue, PatternRestPart, PatternSequenceRestIssue, PatternSequenceRestSyntax,
    PatternSyntaxNode, PatternTypeChildRelation, PatternTypeChildSource,
    PatternUnqualifiedVariantForm, PatternVariantHead, PatternVariantHeadSyntax,
    VariantPatternHeadPart, VariantPatternPayloadPart,
};

pub(super) fn significant_range(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> SourceRange {
    let first = (start..end)
        .filter_map(|index| parser.token_at(index))
        .find(|token| !is_trivia(token.kind()));
    let last = (start..end)
        .rev()
        .filter_map(|index| parser.token_at(index))
        .find(|token| !is_trivia(token.kind()));
    match (first, last) {
        (Some(first), Some(last)) => SourceRange::new(first.range().start(), last.range().end()),
        _ => empty_range(parser),
    }
}

pub(super) fn empty_range(parser: &DocumentParser<'_, '_>) -> SourceRange {
    let at = parser.current_offset();
    SourceRange::new(at, at)
}

pub(super) const fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

/// One semantic Pattern tree being staged by a grammar transaction.
pub(super) struct PatternProjectionTransaction {
    tree: u64,
    nodes: Vec<(PatternNodePath, SourceRange)>,
    components: Vec<PatternComponentSource<SourceRange>>,
    type_children: Vec<PatternTypeChildSource>,
    events: Vec<(Option<usize>, PatternNodePath)>,
}

impl PatternProjectionTransaction {
    pub(super) fn new(parser: &DocumentParser<'_, '_>) -> Self {
        Self {
            tree: u64::try_from(parser.event_position())
                .expect("grammar event limits fit Pattern projection identity"),
            nodes: Vec::new(),
            components: Vec::new(),
            type_children: Vec::new(),
            events: Vec::new(),
        }
    }

    pub(super) fn start_node(
        &mut self,
        parser: &mut DocumentParser<'_, '_>,
        kind: SyntaxKind,
        role: SyntaxRole,
        path: &PatternNodePath,
        whole: SourceRange,
    ) {
        let event = parser.start_pattern(kind, role);
        self.events.push((event, path.clone()));
        self.nodes.push((path.clone(), whole));
        self.component(path, PatternComponentRole::Whole, whole);
    }

    pub(super) fn component(
        &mut self,
        owner: &PatternNodePath,
        role: PatternComponentRole,
        range: SourceRange,
    ) {
        self.components
            .push(PatternComponentSource::new(owner.clone(), role, range));
    }

    pub(super) fn type_child(
        &mut self,
        owner: &PatternNodePath,
        projection: &EmittedTypeProjection,
    ) {
        self.type_children.push(PatternTypeChildSource::new(
            owner.clone(),
            PatternTypeChildRelation::TypedBinding,
            projection.tree(),
            Arc::clone(projection.authored()),
            projection.path().clone(),
        ));
    }

    pub(super) fn finish(self, parser: &mut DocumentParser<'_, '_>, root: PatternSyntaxNode) {
        let authored = Arc::new(
            AuthoredPattern::try_new(root, self.nodes, self.components, self.type_children)
                .expect("Pattern grammar constructs one validated semantic source owner"),
        );
        for (event, path) in self.events {
            parser.set_pattern_projection(event, self.tree, Arc::clone(&authored), path);
        }
    }
}

pub(super) fn binding_syntax(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> (PatternBindingSyntax, Vec<PatternRecoveryIssue>) {
    let significant = significant_indices(parser, start, end);
    let Some(index) = significant.first().copied() else {
        let issue = PatternBindingIssue::MissingName;
        return (
            PatternBindingSyntax::Recovered(issue.clone()),
            vec![PatternRecoveryIssue::Binding(issue)],
        );
    };
    if significant.len() != 1 {
        let issue = PatternBindingIssue::UnexpectedTrailingInput {
            token_count: u32::try_from(significant.len())
                .expect("grammar token limits fit binding token counts"),
        };
        return (
            PatternBindingSyntax::Recovered(issue.clone()),
            vec![PatternRecoveryIssue::Binding(issue)],
        );
    }
    let token = parser
        .token_at(index)
        .expect("significant Pattern token remains in the cursor");
    let spelling = parser.text_of(token);
    let result = if token.kind() == SyntaxKind::KeywordToken && spelling != "choice" {
        Err(PatternBindingIssue::ReservedBindingKeyword {
            spelling: spelling.into(),
        })
    } else {
        SyntaxName::try_new(spelling).map_err(PatternBindingIssue::InvalidName)
    };
    match result {
        Ok(name) => (PatternBindingSyntax::Resolved(name), Vec::new()),
        Err(issue) => (
            PatternBindingSyntax::Recovered(issue.clone()),
            vec![PatternRecoveryIssue::Binding(issue)],
        ),
    }
}

pub(super) fn name_syntax(
    parser: &DocumentParser<'_, '_>,
    index: Option<usize>,
) -> PatternNameSyntax {
    let Some(token) = index.and_then(|index| parser.token_at(index)) else {
        return PatternNameSyntax::Absent;
    };
    match SyntaxName::try_new(parser.text_of(token)) {
        Ok(name) => PatternNameSyntax::Resolved(name),
        Err(issue) => PatternNameSyntax::Recovered(issue),
    }
}

pub(super) fn project_literal(
    parser: &DocumentParser<'_, '_>,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    token: LexToken,
) -> (SyntaxLiteralSyntax, Vec<PatternRecoveryIssue>) {
    let spelling = parser.text_of(token);
    let projection = typed_literal(token, spelling);
    for component in projection.components() {
        transaction.component(
            owner,
            PatternComponentRole::Literal(match component.part() {
                LiteralLexemePart::Body => PatternLiteralPart::Body,
                LiteralLexemePart::Prefix => PatternLiteralPart::Prefix,
                LiteralLexemePart::Suffix => PatternLiteralPart::Suffix,
                LiteralLexemePart::Unit => PatternLiteralPart::Unit,
            }),
            component.range(),
        );
    }
    let issues = match projection.syntax().value() {
        SyntaxLiteralValue::Invalid(issue) => vec![PatternRecoveryIssue::Literal(issue.clone())],
        _ => Vec::new(),
    };
    (projection.into_syntax(), issues)
}

pub(super) fn project_id_ref(
    parser: &DocumentParser<'_, '_>,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    token: LexToken,
) -> (SyntaxIdRefSyntax, Vec<PatternRecoveryIssue>) {
    let spelling = parser.text_of(token);
    let projection = typed_entity_reference(token, spelling);
    for component in projection.components() {
        transaction.component(
            owner,
            PatternComponentRole::EntityReference(component.part()),
            component.range(),
        );
    }
    let issues = projection
        .syntax()
        .value()
        .err()
        .cloned()
        .map(|issue| vec![PatternRecoveryIssue::EntityReference(issue)])
        .unwrap_or_default();
    (projection.into_syntax(), issues)
}

pub(super) fn project_variant_head(
    parser: &DocumentParser<'_, '_>,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    start: usize,
    end: usize,
) -> (
    PatternVariantHeadSyntax,
    PatternNameSyntax,
    Vec<PatternRecoveryIssue>,
) {
    let significant = significant_indices(parser, start, end);
    if significant
        .first()
        .is_some_and(|index| token_text(parser, *index) == Some("."))
    {
        if let Some(token) = significant
            .first()
            .and_then(|index| parser.token_at(*index))
        {
            transaction.component(
                owner,
                PatternComponentRole::VariantHead(VariantPatternHeadPart::DotShorthandMarker),
                token.range(),
            );
        }
        let name_index = significant
            .iter()
            .copied()
            .skip(1)
            .find(|index| is_name_token(parser, *index));
        let name = name_syntax(parser, name_index);
        let name_range = name_index
            .and_then(|index| parser.token_at(index))
            .map_or_else(|| insertion_at_token_boundary(parser, end), LexToken::range);
        transaction.component(owner, PatternComponentRole::VariantName, name_range);
        let issues = name_issue(&name)
            .map(|issue| vec![PatternRecoveryIssue::VariantName(issue)])
            .unwrap_or_default();
        return (
            PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(
                PatternUnqualifiedVariantForm::DotShorthand,
            )),
            name,
            issues,
        );
    }

    let mut parts = token_path_parts(parser, &significant);
    let terminal = (!parts.missing_terminal)
        .then(|| parts.segments.pop())
        .flatten();
    let name = terminal
        .as_ref()
        .map_or(PatternNameSyntax::Absent, |segment| match &segment.name {
            Ok(name) => PatternNameSyntax::Resolved(name.clone()),
            Err(issue) => PatternNameSyntax::Recovered(issue.clone()),
        });
    stage_path_components(transaction, owner, &parts, true);
    transaction.component(
        owner,
        PatternComponentRole::VariantName,
        terminal.as_ref().map_or_else(
            || insertion_at_token_boundary(parser, end),
            |segment| segment.range,
        ),
    );
    let head = if parts.issue.is_none()
        && parts.segments.is_empty()
        && matches!(parts.root, Some(PatternPathRoot::ImplicitCrate) | None)
    {
        PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(
            PatternUnqualifiedVariantForm::BareExpectedType,
        ))
    } else {
        match parts.into_syntax(true) {
            PatternPathSyntax::Resolved(path) => {
                PatternVariantHeadSyntax::Resolved(PatternVariantHead::Qualified(path))
            }
            PatternPathSyntax::Recovered(recovery) => PatternVariantHeadSyntax::Recovered(recovery),
            PatternPathSyntax::Absent => PatternVariantHeadSyntax::Absent,
        }
    };
    let mut issues = Vec::new();
    if let Some(issue) = name_issue(&name) {
        issues.push(PatternRecoveryIssue::VariantName(issue));
    }
    if let PatternVariantHeadSyntax::Recovered(recovery) = &head {
        issues.push(PatternRecoveryIssue::VariantHead(recovery.issue().clone()));
    }
    (head, name, issues)
}

pub(super) fn project_record_path(
    parser: &DocumentParser<'_, '_>,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    start: usize,
    end: usize,
) -> PatternPathSyntax {
    let significant = significant_indices(parser, start, end);
    if significant.is_empty() {
        return PatternPathSyntax::Absent;
    }
    let parts = token_path_parts(parser, &significant);
    stage_path_components(transaction, owner, &parts, false);
    parts.into_syntax(false)
}

pub(super) fn stage_variant_payload(
    parser: &mut DocumentParser<'_, '_>,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    end: usize,
) {
    let start = parser.cursor();
    let whole = significant_range(parser, start, end);
    transaction.component(
        owner,
        PatternComponentRole::VariantPayload(VariantPatternPayloadPart::Whole),
        whole,
    );
    if let Some(open) = parser.current() {
        transaction.component(
            owner,
            PatternComponentRole::VariantPayload(VariantPatternPayloadPart::OpenDelimiter),
            open.range(),
        );
    }
    let close = parser
        .current_text()
        .and_then(|opening| find_matching_close(parser, start + 1, opening))
        .filter(|close| *close < end)
        .and_then(|close| parser.token_at(close))
        .map_or_else(
            || {
                let at = parser
                    .offset_at_token_boundary(end)
                    .unwrap_or_else(|| parser.current_offset());
                SourceRange::new(at, at)
            },
            LexToken::range,
        );
    transaction.component(
        owner,
        PatternComponentRole::VariantPayload(VariantPatternPayloadPart::CloseDelimiter),
        close,
    );
}

pub(super) fn stage_sequence_rest(
    parser: &mut DocumentParser<'_, '_>,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    start: usize,
    end: usize,
) -> (PatternSequenceRestSyntax, Vec<PatternRecoveryIssue>) {
    transaction.component(
        owner,
        PatternComponentRole::SequenceRest(PatternRestPart::Whole),
        significant_range(parser, start, end),
    );
    if let Some(marker) = parser.token_at(start) {
        transaction.component(
            owner,
            PatternComponentRole::SequenceRest(PatternRestPart::Marker),
            marker.range(),
        );
    }
    let binding_start = first_significant(parser, start + 1, end);
    if binding_start
        .and_then(|index| parser.token_at(index))
        .is_some()
    {
        transaction.component(
            owner,
            PatternComponentRole::SequenceRest(PatternRestPart::Binding),
            significant_range(parser, binding_start.unwrap_or(end), end),
        );
        let (binding, binding_issues) = binding_syntax(parser, binding_start.unwrap_or(end), end);
        if let Some(issue) = binding.issue().cloned() {
            let issue = PatternSequenceRestIssue::InvalidBinding(issue);
            return (
                PatternSequenceRestSyntax::Recovered {
                    binding: Some(binding),
                    issues: Box::new([issue.clone()]),
                },
                vec![PatternRecoveryIssue::SequenceRest(issue)],
            );
        }
        debug_assert!(binding_issues.is_empty());
        return (PatternSequenceRestSyntax::Binding(binding), Vec::new());
    }
    (PatternSequenceRestSyntax::Unbound, Vec::new())
}

pub(super) fn stage_record_rest(
    parser: &mut DocumentParser<'_, '_>,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    field: u32,
    start: usize,
    end: usize,
) -> (PatternRecordFieldSyntax, Option<PatternRecordFieldIssue>) {
    let whole = significant_range(parser, start, end);
    for (part, range) in [
        (PatternFieldPart::Whole, whole),
        (
            PatternFieldPart::RestMarker,
            parser
                .token_at(start)
                .map_or_else(|| empty_range(parser), LexToken::range),
        ),
    ] {
        transaction.component(
            owner,
            PatternComponentRole::PatternField { field, part },
            range,
        );
    }
    let binding_start = first_significant(parser, start + 1, end);
    if binding_start
        .and_then(|index| parser.token_at(index))
        .is_some()
    {
        transaction.component(
            owner,
            PatternComponentRole::PatternField {
                field,
                part: PatternFieldPart::RestBinding,
            },
            significant_range(parser, binding_start.unwrap_or(end), end),
        );
        let (binding, issues) = binding_syntax(parser, binding_start.unwrap_or(end), end);
        let issue = issues.into_iter().find_map(|issue| match issue {
            PatternRecoveryIssue::Binding(PatternBindingIssue::MissingName) => {
                Some(PatternRecordFieldIssue::MissingName)
            }
            PatternRecoveryIssue::Binding(
                issue @ (PatternBindingIssue::InvalidName(_)
                | PatternBindingIssue::ReservedBindingKeyword { .. }
                | PatternBindingIssue::UnexpectedTrailingInput { .. }),
            ) => Some(PatternRecordFieldIssue::InvalidRestBinding(issue)),
            _ => None,
        });
        return (PatternRecordFieldSyntax::Rest(Some(binding)), issue);
    }
    (PatternRecordFieldSyntax::Rest(None), None)
}

fn source_at<'source>(parser: &DocumentParser<'source, '_>, range: SourceRange) -> &'source str {
    &parser.source()[range.as_range()]
}

fn insertion_at_token_boundary(parser: &DocumentParser<'_, '_>, index: usize) -> SourceRange {
    let at = parser
        .offset_at_token_boundary(index)
        .unwrap_or_else(|| parser.current_offset());
    SourceRange::new(at, at)
}

#[derive(Clone)]
struct TokenPathSegment {
    name: Result<SyntaxName, SyntaxNameIssue>,
    value: Result<PatternPathSegment, SyntaxNameIssue>,
    spelling: Box<str>,
    range: SourceRange,
}

struct TokenPathParts {
    root: Option<PatternPathRoot>,
    root_range: Option<SourceRange>,
    segments: Vec<TokenPathSegment>,
    issue: Option<PatternPathIssue>,
    missing_terminal: bool,
}

impl TokenPathParts {
    fn into_syntax(self, _allow_explicit_root_only: bool) -> PatternPathSyntax {
        let root = self.root.unwrap_or(PatternPathRoot::ImplicitCrate);
        let attempted = self
            .segments
            .iter()
            .map(|segment| segment.spelling.clone())
            .collect::<Vec<_>>();
        let issue = self.issue.or_else(|| {
            self.segments
                .is_empty()
                .then_some(PatternPathIssue::MissingSegment)
        });
        if let Some(issue) = issue {
            return PatternPathSyntax::Recovered(PatternPathRecovery::new(
                Some(root),
                attempted,
                issue,
            ));
        }
        let segments = self
            .segments
            .into_iter()
            .map(|segment| {
                segment
                    .value
                    .expect("path without recovery owns validated segments")
            })
            .collect();
        PatternPathSyntax::Resolved(PatternPath::new(root, segments))
    }
}

fn token_path_parts(parser: &DocumentParser<'_, '_>, significant: &[usize]) -> TokenPathParts {
    let mut cursor = 0_usize;
    let mut root = Some(PatternPathRoot::ImplicitCrate);
    let mut root_range = None;
    if significant
        .first()
        .is_some_and(|index| token_text(parser, *index) == Some("::"))
    {
        root = Some(PatternPathRoot::Crate);
        root_range = significant
            .first()
            .and_then(|index| parser.token_at(*index))
            .map(LexToken::range);
        cursor = 1;
    } else if significant.len() >= 2
        && matches!(token_text(parser, significant[0]), Some("crate" | "self"))
        && path_separator(parser, significant[1])
    {
        root = Some(if token_text(parser, significant[0]) == Some("crate") {
            PatternPathRoot::Crate
        } else {
            PatternPathRoot::SelfModule
        });
        root_range = parser.token_at(significant[0]).map(LexToken::range);
        cursor = 2;
    } else {
        let mut levels = 0_usize;
        let mut root_end = None;
        while cursor + 1 < significant.len()
            && token_text(parser, significant[cursor]) == Some("super")
            && path_separator(parser, significant[cursor + 1])
        {
            levels = levels
                .checked_add(1)
                .expect("grammar token limits fit path root depth");
            root_end = parser
                .token_at(significant[cursor])
                .map(|token| token.range().end());
            cursor = cursor
                .checked_add(2)
                .expect("grammar token limits fit path traversal");
        }
        if levels > 0 {
            root = Some(PatternPathRoot::Super(levels));
            if let (Some(first), Some(end)) = (
                significant
                    .first()
                    .and_then(|index| parser.token_at(*index)),
                root_end,
            ) {
                root_range = Some(SourceRange::new(first.range().start(), end));
            }
        }
    }
    let mut segments = Vec::new();
    let mut issue = root.is_none().then_some(PatternPathIssue::InvalidRootDepth);
    let mut group = Vec::new();
    for index in significant.iter().copied().skip(cursor) {
        if path_separator(parser, index) {
            if group.is_empty() {
                issue.get_or_insert(PatternPathIssue::MissingSegment);
            } else {
                push_path_segment(parser, &group, &mut segments, &mut issue);
                group.clear();
            }
        } else {
            group.push(index);
        }
    }
    let missing_terminal = group.is_empty() && cursor < significant.len();
    if group.is_empty() {
        if cursor < significant.len() {
            issue.get_or_insert(PatternPathIssue::MissingSegment);
        }
    } else {
        push_path_segment(parser, &group, &mut segments, &mut issue);
    }
    TokenPathParts {
        root,
        root_range,
        segments,
        issue,
        missing_terminal,
    }
}

fn push_path_segment(
    parser: &DocumentParser<'_, '_>,
    indices: &[usize],
    segments: &mut Vec<TokenPathSegment>,
    issue: &mut Option<PatternPathIssue>,
) {
    let first = parser
        .token_at(indices[0])
        .expect("significant path segment start remains present");
    let last = parser
        .token_at(*indices.last().expect("path segment is non-empty"))
        .expect("significant path segment end remains present");
    let range = SourceRange::new(first.range().start(), last.range().end());
    let spelling = source_at(parser, range);
    let name = SyntaxName::try_new(spelling);
    let contiguous = indices.windows(2).all(|pair| {
        parser.token_at(pair[0]).is_some_and(|left| {
            parser
                .token_at(pair[1])
                .is_some_and(|right| left.range().end() == right.range().start())
        })
    });
    let external_shape = contiguous
        && indices.len() >= 3
        && indices.iter().enumerate().all(|(position, index)| {
            if position % 2 == 0 {
                is_name_token(parser, *index)
            } else {
                token_text(parser, *index) == Some("-")
            }
        });
    let value = if indices.len() == 1 && is_name_token(parser, indices[0]) {
        name.clone().map(PatternPathSegment::Identifier)
    } else if external_shape {
        ProjectSymbolSegment::try_new(spelling.to_owned())
            .map(PatternPathSegment::ProjectSymbol)
            .map_err(|_| SyntaxNameIssue::InvalidContinuation {
                spelling: spelling.into(),
            })
    } else {
        Err(SyntaxNameIssue::InvalidContinuation {
            spelling: spelling.into(),
        })
    };
    if let Err(name_issue) = &value
        && issue.is_none()
    {
        *issue = Some(PatternPathIssue::InvalidSegment {
            ordinal: u32::try_from(segments.len())
                .expect("grammar limits fit path segment ordinals"),
            issue: name_issue.clone(),
        });
    }
    segments.push(TokenPathSegment {
        name,
        value,
        spelling: spelling.into(),
        range,
    });
}

fn stage_path_components(
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
    parts: &TokenPathParts,
    variant: bool,
) {
    if let Some(root) = parts.root_range {
        transaction.component(
            owner,
            if variant {
                PatternComponentRole::VariantHead(VariantPatternHeadPart::QualifiedRoot)
            } else {
                PatternComponentRole::RecordPathRoot
            },
            root,
        );
    }
    for (ordinal, segment) in parts.segments.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).expect("grammar limits fit path component ordinals");
        transaction.component(
            owner,
            if variant {
                PatternComponentRole::VariantHead(VariantPatternHeadPart::QualifiedSegment {
                    ordinal,
                })
            } else {
                PatternComponentRole::RecordPathSegment { ordinal }
            },
            segment.range,
        );
    }
}

fn name_issue(name: &PatternNameSyntax) -> Option<SyntaxNameIssue> {
    match name {
        PatternNameSyntax::Resolved(_) => None,
        PatternNameSyntax::Recovered(issue) => Some(issue.clone()),
        PatternNameSyntax::Absent => Some(SyntaxNameIssue::Missing),
    }
}

fn significant_indices(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> Vec<usize> {
    (start..end)
        .filter(|index| {
            parser.token_at(*index).is_some_and(|token| {
                !matches!(
                    token.kind(),
                    SyntaxKind::WhitespaceToken
                        | SyntaxKind::NewlineToken
                        | SyntaxKind::CommentToken
                        | SyntaxKind::DocCommentToken
                )
            })
        })
        .collect()
}

fn is_name_token(parser: &DocumentParser<'_, '_>, index: usize) -> bool {
    parser.token_at(index).is_some_and(|token| {
        matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        )
    })
}

fn path_separator(parser: &DocumentParser<'_, '_>, index: usize) -> bool {
    matches!(token_text(parser, index), Some("." | "::"))
}
