//! Exact fragment rebasing without reconstructing candidate events or source.

use super::{
    PendingCandidateGraph, PendingCandidateNode, PendingCandidateSemantic, SourceRange,
    SyntaxAttachedContentApplicationForm, SyntaxAttachedContentApplicationProjection,
    SyntaxBracketTerminator, SyntaxDialogueContent, SyntaxDialogueContentProjection,
    SyntaxDialogueContentRecoveryBoundary, SyntaxDialogueNodeProjection,
    SyntaxDialoguePointActionIdentity, SyntaxIdRefComponent, SyntaxIndexProjection,
    SyntaxPostfixBracketProjection, SyntaxPostfixBracketRecoveryBoundary,
    SyntaxPostfixCandidateFailure, SyntaxPostfixCandidateFailureSite,
    SyntaxPostfixDialogueCandidate, SyntaxPostfixIndexCandidate, SyntaxRawLiteralBody,
};
use crate::grammar::event::ProjectionRebaseContext;

fn range(source: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        source.start().checked_add(offset)?,
        source.end().checked_add(offset)?,
    ))
}

impl SyntaxBracketTerminator {
    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Closed => Self::Closed,
            Self::RecoveredMissing(boundary) => Self::RecoveredMissing(match boundary {
                SyntaxPostfixBracketRecoveryBoundary::EndOfExpression { anchor } => {
                    SyntaxPostfixBracketRecoveryBoundary::EndOfExpression {
                        anchor: anchor.checked_add(offset)?,
                    }
                }
                SyntaxPostfixBracketRecoveryBoundary::OwnerEnd { anchor } => {
                    SyntaxPostfixBracketRecoveryBoundary::OwnerEnd {
                        anchor: anchor.checked_add(offset)?,
                    }
                }
                SyntaxPostfixBracketRecoveryBoundary::LineEnding { range: source } => {
                    SyntaxPostfixBracketRecoveryBoundary::LineEnding {
                        range: range(*source, offset)?,
                    }
                }
                SyntaxPostfixBracketRecoveryBoundary::Token {
                    token,
                    range: source,
                } => SyntaxPostfixBracketRecoveryBoundary::Token {
                    token: *token,
                    range: range(*source, offset)?,
                },
                SyntaxPostfixBracketRecoveryBoundary::PlanKeyword { range: source } => {
                    SyntaxPostfixBracketRecoveryBoundary::PlanKeyword {
                        range: range(*source, offset)?,
                    }
                }
            }),
        })
    }
}

impl SyntaxIndexProjection {
    pub(in crate::expressions) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            target: self.target,
            index: self.index,
            terminator: self.terminator.rebased(offset)?,
        })
    }
}

impl SyntaxAttachedContentApplicationProjection {
    pub(in crate::expressions) fn rebased(&self, offset: usize) -> Option<Self> {
        let form = match &self.form {
            SyntaxAttachedContentApplicationForm::Bracket { terminator } => {
                SyntaxAttachedContentApplicationForm::Bracket {
                    terminator: terminator.rebased(offset)?,
                }
            }
            SyntaxAttachedContentApplicationForm::Hash => {
                SyntaxAttachedContentApplicationForm::Hash
            }
            SyntaxAttachedContentApplicationForm::Colon => {
                SyntaxAttachedContentApplicationForm::Colon
            }
        };
        Some(Self {
            form,
            content: self.content.rebased(offset)?,
            has_plan: self.has_plan,
        })
    }
}

impl SyntaxDialogueContentProjection {
    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::RawLiteral(body) => Self::RawLiteral(SyntaxRawLiteralBody {
                value: body.value.clone(),
                range: range(body.range, offset)?,
            }),
            Self::Missing { boundary } => Self::Missing {
                boundary: match boundary {
                    SyntaxDialogueContentRecoveryBoundary::CloseBracket { range: source } => {
                        SyntaxDialogueContentRecoveryBoundary::CloseBracket {
                            range: range(*source, offset)?,
                        }
                    }
                    SyntaxDialogueContentRecoveryBoundary::MissingBracketClose { insertion } => {
                        SyntaxDialogueContentRecoveryBoundary::MissingBracketClose {
                            insertion: insertion.checked_add(offset)?,
                        }
                    }
                    SyntaxDialogueContentRecoveryBoundary::Inline { insertion } => {
                        SyntaxDialogueContentRecoveryBoundary::Inline {
                            insertion: insertion.checked_add(offset)?,
                        }
                    }
                    SyntaxDialogueContentRecoveryBoundary::Indented { insertion } => {
                        SyntaxDialogueContentRecoveryBoundary::Indented {
                            insertion: insertion.checked_add(offset)?,
                        }
                    }
                },
            },
            Self::Present(content) => Self::Present(SyntaxDialogueContent {
                nodes: content
                    .nodes
                    .iter()
                    .map(|node| {
                        Some(match node {
                            SyntaxDialogueNodeProjection::PointAction(action) => {
                                let mut action = action.clone();
                                if let SyntaxDialoguePointActionIdentity::Mark(mark) =
                                    &mut action.identity
                                {
                                    mark.range = range(mark.range, offset)?;
                                    mark.components = mark
                                        .components
                                        .iter()
                                        .map(|component| {
                                            Some(SyntaxIdRefComponent::new(
                                                component.part(),
                                                range(component.range(), offset)?,
                                            ))
                                        })
                                        .collect::<Option<Box<[_]>>>()?;
                                }
                                SyntaxDialogueNodeProjection::PointAction(action)
                            }
                            SyntaxDialogueNodeProjection::Text(_)
                            | SyntaxDialogueNodeProjection::Escape(_)
                            | SyntaxDialogueNodeProjection::Ruby { .. }
                            | SyntaxDialogueNodeProjection::Interpolation(_)
                            | SyntaxDialogueNodeProjection::ContentApplication(_)
                            | SyntaxDialogueNodeProjection::LineBreak(_)
                            | SyntaxDialogueNodeProjection::Error(_) => node.clone(),
                        })
                    })
                    .collect::<Option<Box<[_]>>>()?,
            }),
        })
    }
}

impl PendingCandidateGraph {
    fn rebased(&self, offset: usize, context: &mut ProjectionRebaseContext) -> Option<Self> {
        let nodes = self
            .nodes
            .iter()
            .map(|node| {
                let semantic = match &node.semantic {
                    PendingCandidateSemantic::Expression(value) => {
                        PendingCandidateSemantic::Expression(value.rebased(offset, context)?)
                    }
                    PendingCandidateSemantic::Type(value) => {
                        PendingCandidateSemantic::Type(value.rebased(offset, context)?)
                    }
                    PendingCandidateSemantic::Pattern(value) => {
                        PendingCandidateSemantic::Pattern(value.rebased(offset, context)?)
                    }
                    PendingCandidateSemantic::Path(value) => {
                        PendingCandidateSemantic::Path(value.rebased(offset)?)
                    }
                    PendingCandidateSemantic::Assertion(value) => {
                        PendingCandidateSemantic::Assertion(*value)
                    }
                    PendingCandidateSemantic::KeywordStatement(value) => {
                        PendingCandidateSemantic::KeywordStatement(value.clone())
                    }
                    PendingCandidateSemantic::KindOnly => PendingCandidateSemantic::KindOnly,
                };
                Some(PendingCandidateNode {
                    kind: node.kind,
                    role: node.role,
                    parent: node.parent,
                    children: node.children,
                    source: range(node.source, offset)?,
                    semantic,
                })
            })
            .collect::<Option<Box<[_]>>>()?;
        Some(Self {
            roots: self.roots.clone(),
            nodes,
            child_edges: self.child_edges.clone(),
            type_index: self.type_index.clone(),
            pattern_index: self.pattern_index.clone(),
            missing_tokens: self
                .missing_tokens
                .iter()
                .map(|(token, at)| Some((*token, at.checked_add(offset)?)))
                .collect::<Option<Box<[_]>>>()?,
            diagnostics: self
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.rebased(offset))
                .collect::<Option<Box<[_]>>>()?,
        })
    }
}

impl SyntaxPostfixBracketProjection {
    pub(in crate::expressions) fn rebased(
        &self,
        offset: usize,
        context: &mut ProjectionRebaseContext,
    ) -> Option<Self> {
        Some(match self {
            Self::Ambiguous { index, dialogue } => Self::Ambiguous {
                index: Box::new(SyntaxPostfixIndexCandidate {
                    index: index.index,
                    graph: index.graph.rebased(offset, context)?,
                }),
                dialogue: Box::new(SyntaxPostfixDialogueCandidate {
                    content: dialogue.content.rebased(offset)?,
                    components: dialogue
                        .components
                        .iter()
                        .map(|component| component.rebased(offset))
                        .collect::<Option<Box<[_]>>>()?,
                    graph: dialogue.graph.rebased(offset, context)?,
                }),
            },
            Self::Invalid { index, dialogue } => Self::Invalid {
                index: index.rebased(offset)?,
                dialogue: dialogue.rebased(offset)?,
            },
        })
    }
}

impl SyntaxPostfixCandidateFailure {
    fn rebased(&self, offset: usize) -> Option<Self> {
        let site = match self.site {
            SyntaxPostfixCandidateFailureSite::Span(source) => {
                SyntaxPostfixCandidateFailureSite::Span(range(source, offset)?)
            }
            SyntaxPostfixCandidateFailureSite::Insertion(at) => {
                SyntaxPostfixCandidateFailureSite::Insertion(at.checked_add(offset)?)
            }
        };
        Some(Self {
            kind: self.kind,
            site,
        })
    }
}
