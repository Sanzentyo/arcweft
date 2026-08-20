//! Parser-selected semantic projections for attached expression identities.
//!
//! These values are source-backed metadata, not a detached expression tree.
//! Structural expression children remain owned by the attached role graph,
//! while paths keep their existing [`crate::attachment::AttachedPath`] owner.

mod control;
mod dialogue;
mod pending;

#[cfg(test)]
mod projection_tests;

pub use control::{
    SyntaxMatchArmPart, SyntaxMatchArmProjection, SyntaxMatchBodyTerminator, SyntaxMatchProjection,
};
pub(crate) use dialogue::{
    CandidateNodeIndex, PendingCandidateGraph, PendingCandidateNode, PendingCandidateSemantic,
};
pub use dialogue::{
    SyntaxBracketTerminator, SyntaxBuiltinRichTextFx, SyntaxBuiltinRichTextTag,
    SyntaxCandidateQuality, SyntaxDialogueApplicationForm, SyntaxDialogueApplicationProjection,
    SyntaxDialogueConfigurationArgumentPart, SyntaxDialogueContent, SyntaxDialogueContentIssue,
    SyntaxDialogueContentProjection, SyntaxDialogueContentRecoveryBoundary,
    SyntaxDialogueNodeProjection, SyntaxDialogueNodeSourcePart, SyntaxIndexProjection,
    SyntaxLineBreakKind, SyntaxPostfixBoundaryToken, SyntaxPostfixBracketProjection,
    SyntaxPostfixBracketRecoveryBoundary, SyntaxPostfixCandidateFailure,
    SyntaxPostfixCandidateFailureKind, SyntaxPostfixCandidateFailureSite,
    SyntaxPostfixDialogueCandidate, SyntaxPostfixIndexCandidate, SyntaxProjectSymbolPath,
    SyntaxRichTextArgumentParts, SyntaxRichTextArgumentProjection,
    SyntaxRichTextArgumentSourcePart, SyntaxRichTextConditionalTag, SyntaxRichTextDirectStyle,
    SyntaxRichTextEndTagProjection, SyntaxRichTextHostEvent, SyntaxRichTextIssue,
    SyntaxRichTextLayoutSelector, SyntaxRichTextObjectSelector, SyntaxRichTextStyleSelector,
    SyntaxRichTextTagIdentity, SyntaxRichTextTagPayloadProjection, SyntaxRichTextTagProjection,
    SyntaxRichTextTagSourcePart, SyntaxRichTextTransformSelector, SyntaxRichTextValue,
};
pub(crate) use pending::{PendingExpressionComponent, PendingExpressionProjection};

use crate::grammar::SyntaxAwaitBranchKind;
use crate::id_ref::{SyntaxIdRefPart, SyntaxIdRefSyntax};
use crate::literal::{IntSuffix, SyntaxIntegerIssue, SyntaxIntegerLiteral, SyntaxLiteralSyntax};
use crate::name::{SyntaxName, SyntaxNameIssue};

/// Parser-selected semantic payload for the first attached expression slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionProjection {
    /// The empty tuple spelling `()`.
    Unit,
    /// One lexer-owned typed literal.
    Literal(SyntaxLiteralSyntax),
    /// One lexer-owned structured entity reference.
    EntityReference(SyntaxIdRefSyntax),
    /// Runtime lifetime-registry access, separate from a type region.
    LifetimePath(SyntaxLifetimeRegistryPath),
    /// Marker selecting the existing attached path child as semantic owner.
    Path,
    /// Enum shorthand selected from the name following `.`.
    ShortVariant(Result<SyntaxName, SyntaxNameIssue>),
    /// One authored expression placeholder marker.
    Placeholder(SyntaxPlaceholderKind),
    /// A non-empty tuple and its exact authored/recovered child slots.
    Tuple(Box<[SyntaxExpressionSlot]>),
    /// A general bracket sequence and its exact authored/recovered child slots.
    BracketSequence(Box<[SyntaxExpressionSlot]>),
    /// One compact, ID-less integer sequence selected by the literal lexer.
    NumericBracketSequence(SyntaxNumericSequence),
    /// A repeated array value and length, in that fixed order.
    ArrayRepeat([SyntaxExpressionSlot; 2]),
    /// One ordinary parenthesized or callback-block Call and its exact
    /// source-produced recovery state.
    Call(SyntaxCallProjection),
    /// One authored target followed by an ordinary dot and a present or
    /// source-owned missing member.
    Select(SyntaxSelectedMember),
    /// The one selected ordinary-index interpretation of a generic postfix
    /// bracket owner.
    Index(SyntaxIndexProjection),
    /// The one selected bracket or colon dialogue-content application.
    DialogueContentApplication(SyntaxDialogueApplicationProjection),
    /// A generic bracket whose two bounded interpretations are both viable or
    /// both failed.
    PostfixBracket(SyntaxPostfixBracketProjection),
    /// A pipeline left and right operand, in that fixed order.
    Pipe([SyntaxExpressionSlot; 2]),
    /// One prefix try operand.
    Try { operand: SyntaxExpressionSlot },
    /// One await operand and, when authored, its source-ordered `with`
    /// branches. Error propagation is represented by an enclosing `try`
    /// expression; `Some(empty)` retains an authored but missing/empty branch
    /// body distinct from an await without `with`.
    Await {
        operand: SyntaxExpressionSlot,
        branches: Option<Box<[Option<SyntaxAwaitBranchKind>]>>,
    },
    /// One shared or mutable borrow operand.
    Borrow {
        operand: SyntaxExpressionSlot,
        kind: SyntaxBorrowKind,
    },
    /// One dereference operand.
    Dereference { operand: SyntaxExpressionSlot },
    /// One logical or numeric unary operand.
    Unary {
        operand: SyntaxExpressionSlot,
        operator: SyntaxUnaryOperator,
    },
    /// Optional range endpoints and the authored inclusive-end marker.
    Range {
        start: Option<SyntaxExpressionSlot>,
        end: Option<SyntaxExpressionSlot>,
        inclusive: bool,
    },
    /// One path-qualified record and its fields in authored order.
    Record(Box<[SyntaxRecordField]>),
    /// One pathless record literal and its fields in authored order.
    RecordLiteral(Box<[SyntaxRecordField]>),
    /// One closed binary operator over fixed left and right operand slots.
    Binary {
        left: SyntaxExpressionSlot,
        operator: SyntaxBinaryOperator,
        right: SyntaxExpressionSlot,
    },
    /// One closure with ordered pattern/type parameters, optional result type,
    /// and one authored or recovered body expression.
    Closure(SyntaxClosureProjection),
    /// One value block whose typed statement list and tail remain attached to
    /// the exact nested `Block` owner. This also represents the documented
    /// unnamed `scope { ... }` sugar; the omitted name is not recovery.
    Block,
    /// One `result`, `task`, `seq`, or `stream` computation block whose
    /// statement list and tail remain attached to the exact nested `Block`
    /// owner.
    ComputationBlock(SyntaxComputationBlockKind),
    /// One named `scope` block. An invalid-present authored name remains typed
    /// recovery and is never replaced with a fabricated identifier. A truly
    /// omitted name is the ordinary [`Self::Block`] projection.
    NamedBlock(Result<SyntaxName, SyntaxNameIssue>),
    /// One value-producing `loop { ... }` expression. Its braced value block
    /// remains a structural child of this expression owner; `break`/`continue`
    /// are ordinary statements inside that block.
    Loop,
    /// One attached or detached Thread with an optional typed source name.
    /// Its statement-only body is owned by the exact Thread syntax node.
    Thread(SyntaxThreadProjection),
    /// One Choice expression whose ID, candidate body, and optional plan are
    /// owned by the exact attached Choice syntax node.
    Choice,
    /// One condition, required then branch, and authored or omitted else
    /// branch. An omitted else is retained as a zero-width source component
    /// and receives its final synthetic Unit identity during HIR lowering.
    If {
        condition: SyntaxExpressionSlot,
        then_branch: SyntaxExpressionSlot,
        else_branch: Option<SyntaxExpressionSlot>,
    },
    /// One typed binding pattern, scrutinee, optional guard, then branch, and
    /// authored or required-missing else branch. The pattern remains owned by
    /// the attached Pattern family rather than being encoded as an expression
    /// slot.
    IfLet {
        scrutinee: SyntaxExpressionSlot,
        guard: Option<SyntaxExpressionSlot>,
        then_branch: SyntaxExpressionSlot,
        else_branch: Option<SyntaxExpressionSlot>,
    },
    /// One scrutinee and source-ordered typed arms. Each arm retains its
    /// attached Pattern owner and authored or recovered Guard/Value slots.
    Match(SyntaxMatchProjection),
    /// Generic recovery used only when no known expression family applies.
    Error,
}

impl ExpressionProjection {
    /// Whether this parser-selected family retains typed recovery.
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Literal(literal) => literal.value().issue().is_some(),
            Self::EntityReference(reference) => reference.value().is_err(),
            Self::LifetimePath(path) => path.has_recovery(),
            Self::ShortVariant(name) | Self::NamedBlock(name) => name.is_err(),
            Self::Tuple(slots) | Self::BracketSequence(slots) => {
                slots.iter().copied().any(SyntaxExpressionSlot::is_missing)
            }
            Self::NumericBracketSequence(sequence) => sequence.has_recovery(),
            Self::ArrayRepeat(slots) | Self::Pipe(slots) => {
                slots.iter().copied().any(SyntaxExpressionSlot::is_missing)
            }
            Self::Call(call) => call.has_recovery(),
            Self::Select(member) => matches!(member, SyntaxSelectedMember::Missing),
            Self::Index(index) => index.has_recovery(),
            Self::DialogueContentApplication(application) => application.has_recovery(),
            Self::PostfixBracket(postfix) => postfix.has_recovery(),
            Self::Await { operand, branches } => {
                operand.is_missing()
                    || branches.as_ref().is_some_and(|branches| {
                        branches.is_empty() || branches.iter().any(Option::is_none)
                    })
            }
            Self::Try { operand, .. }
            | Self::Borrow { operand, .. }
            | Self::Dereference { operand }
            | Self::Unary { operand, .. } => operand.is_missing(),
            Self::Range { start, end, .. } => start
                .iter()
                .chain(end)
                .copied()
                .any(SyntaxExpressionSlot::is_missing),
            Self::Record(fields) | Self::RecordLiteral(fields) => {
                fields.iter().any(SyntaxRecordField::has_recovery)
            }
            Self::Binary { left, right, .. } => left.is_missing() || right.is_missing(),
            Self::If {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.is_missing()
                    || then_branch.is_missing()
                    || else_branch
                        .iter()
                        .copied()
                        .any(SyntaxExpressionSlot::is_missing)
            }
            Self::IfLet {
                scrutinee,
                guard,
                then_branch,
                else_branch,
            } => {
                scrutinee.is_missing()
                    || guard.iter().copied().any(SyntaxExpressionSlot::is_missing)
                    || then_branch.is_missing()
                    || else_branch
                        .iter()
                        .copied()
                        .any(SyntaxExpressionSlot::is_missing)
                    || else_branch.is_none()
            }
            Self::Match(projection) => projection.has_recovery(),
            Self::Error => true,
            Self::Closure(closure) => {
                closure.body().is_missing()
                    || closure.syntax().terminator() == SyntaxClosureTerminator::RecoveredMissing
            }
            Self::Thread(thread) => thread.has_recovery(),
            Self::Unit
            | Self::Path
            | Self::Placeholder(_)
            | Self::Block
            | Self::ComputationBlock(_)
            | Self::Loop
            | Self::Choice => false,
        }
    }
}

/// Parser-selected computation-block family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxComputationBlockKind {
    Result,
    Option,
    Seq,
    Stream,
}

/// Execution mode selected by the Thread expression header.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxThreadMode {
    Attached,
    Detached,
}

/// Parser-owned semantic Thread header projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxThreadProjection {
    mode: SyntaxThreadMode,
    name: Option<Result<SyntaxName, SyntaxNameIssue>>,
}

impl SyntaxThreadProjection {
    pub(crate) const fn new(
        mode: SyntaxThreadMode,
        name: Option<Result<SyntaxName, SyntaxNameIssue>>,
    ) -> Self {
        Self { mode, name }
    }

    pub const fn mode(&self) -> SyntaxThreadMode {
        self.mode
    }

    pub const fn name(&self) -> Option<&Result<SyntaxName, SyntaxNameIssue>> {
        self.name.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.name.as_ref().is_some_and(Result::is_err)
    }
}

/// Terminal member state produced by the current Select grammar.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxSelectedMember {
    Name(SyntaxName),
    Missing,
}

/// Source-owned Call family selected by the active expression transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxCallProjection {
    Parenthesized(SyntaxParenthesizedCallProjection),
    CallbackBlock(SyntaxCallbackBlockCallProjection),
}

impl SyntaxCallProjection {
    pub const fn parenthesized(&self) -> Option<&SyntaxParenthesizedCallProjection> {
        match self {
            Self::Parenthesized(call) => Some(call),
            Self::CallbackBlock(_) => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Parenthesized(call) => call.has_recovery(),
            Self::CallbackBlock(call) => {
                call.callback().is_missing()
                    || call.terminator() == SyntaxCallArgumentListTerminator::RecoveredMissing
            }
        }
    }
}

/// One source-produced parenthesized Call projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxParenthesizedCallProjection {
    callee: SyntaxCallCalleeProjection,
    explicit_type_application: Option<SyntaxCallTypeApplicationProjection>,
    arguments: Box<[SyntaxCallArgumentProjection]>,
    terminator: SyntaxCallArgumentListTerminator,
}

impl SyntaxParenthesizedCallProjection {
    pub(crate) fn ordinary(
        explicit_type_application: Option<SyntaxCallTypeApplicationProjection>,
        arguments: Vec<SyntaxCallArgumentProjection>,
        terminator: SyntaxCallArgumentListTerminator,
    ) -> Self {
        Self {
            callee: SyntaxCallCalleeProjection::Ordinary,
            explicit_type_application,
            arguments: arguments.into_boxed_slice(),
            terminator,
        }
    }

    pub(crate) fn unresolved_dot(
        separator: SyntaxAssociatedSeparator,
        member: Result<SyntaxName, SyntaxNameIssue>,
        explicit_type_application: Option<SyntaxCallTypeApplicationProjection>,
        arguments: Vec<SyntaxCallArgumentProjection>,
        terminator: SyntaxCallArgumentListTerminator,
    ) -> Self {
        Self {
            callee: SyntaxCallCalleeProjection::UnresolvedDot { separator, member },
            explicit_type_application,
            arguments: arguments.into_boxed_slice(),
            terminator,
        }
    }

    pub(crate) fn associated(
        receiver: SyntaxAssociatedReceiver,
        separator: SyntaxAssociatedSeparator,
        member: Result<SyntaxName, SyntaxNameIssue>,
        explicit_type_application: Option<SyntaxCallTypeApplicationProjection>,
        arguments: Vec<SyntaxCallArgumentProjection>,
        terminator: SyntaxCallArgumentListTerminator,
    ) -> Self {
        Self {
            callee: SyntaxCallCalleeProjection::Associated {
                receiver,
                separator,
                member,
            },
            explicit_type_application,
            arguments: arguments.into_boxed_slice(),
            terminator,
        }
    }

    pub const fn callee(&self) -> &SyntaxCallCalleeProjection {
        &self.callee
    }

    pub const fn explicit_type_application(&self) -> Option<&SyntaxCallTypeApplicationProjection> {
        self.explicit_type_application.as_ref()
    }

    pub fn arguments(&self) -> &[SyntaxCallArgumentProjection] {
        &self.arguments
    }

    pub const fn terminator(&self) -> SyntaxCallArgumentListTerminator {
        self.terminator
    }

    pub fn has_recovery(&self) -> bool {
        self.callee.has_recovery()
            || match &self.explicit_type_application {
                Some(application) => application.has_recovery(),
                None => false,
            }
            || self
                .arguments
                .iter()
                .any(SyntaxCallArgumentProjection::has_recovery)
            || matches!(
                self.terminator,
                SyntaxCallArgumentListTerminator::RecoveredMissing
            )
    }
}

/// Source-produced Call callee classification before project value/type lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxCallCalleeProjection {
    Ordinary,
    UnresolvedDot {
        separator: SyntaxAssociatedSeparator,
        member: Result<SyntaxName, SyntaxNameIssue>,
    },
    Associated {
        receiver: SyntaxAssociatedReceiver,
        separator: SyntaxAssociatedSeparator,
        member: Result<SyntaxName, SyntaxNameIssue>,
    },
}

impl SyntaxCallCalleeProjection {
    fn has_recovery(&self) -> bool {
        match self {
            Self::Ordinary => false,
            Self::UnresolvedDot { member, .. } | Self::Associated { member, .. } => member.is_err(),
        }
    }
}

/// Source-level associated receiver presence before typed lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxAssociatedReceiver {
    Present,
}

/// Exact authored or recovered separator state of one associated Call.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxAssociatedSeparator {
    Present(SyntaxAssociatedCallSyntax),
}

impl SyntaxAssociatedSeparator {
    pub const fn intended(self) -> SyntaxAssociatedCallSyntax {
        match self {
            Self::Present(syntax) => syntax,
        }
    }
}

/// Syntax-owned associated separator family. HIR maps this directly without
/// introducing a cross-crate alias.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxAssociatedCallSyntax {
    DotFallback,
    ExplicitDoubleColon,
}

/// Revision-bound type-child relation owned by one attached Call projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallTypeChildRole {
    DotNominalReceiver,
    AssociatedReceiver,
    ExplicitCallTypeArgument { ordinal: u16 },
}

/// Explicit type application retained on the terminal Call target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxCallTypeApplicationProjection {
    spelling: SyntaxCallTypeApplicationSpelling,
    arguments: Box<[SyntaxCallTypeArgumentProjection]>,
    terminator: SyntaxCallTypeApplicationTerminator,
}

impl SyntaxCallTypeApplicationProjection {
    pub(crate) fn new(
        spelling: SyntaxCallTypeApplicationSpelling,
        arguments: Vec<SyntaxCallTypeArgumentProjection>,
        terminator: SyntaxCallTypeApplicationTerminator,
    ) -> Self {
        Self {
            spelling,
            arguments: arguments.into_boxed_slice(),
            terminator,
        }
    }

    pub const fn spelling(&self) -> SyntaxCallTypeApplicationSpelling {
        self.spelling
    }

    pub fn arguments(&self) -> &[SyntaxCallTypeArgumentProjection] {
        &self.arguments
    }

    pub const fn terminator(&self) -> SyntaxCallTypeApplicationTerminator {
        self.terminator
    }

    fn has_recovery(&self) -> bool {
        self.arguments
            .iter()
            .any(|argument| !matches!(argument, SyntaxCallTypeArgumentProjection::Present))
            || !matches!(self.terminator, SyntaxCallTypeApplicationTerminator::Closed)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallTypeApplicationSpelling {
    DirectAngle,
    Turbofish,
}

/// A type slot is source state only; its revision-bound type identity lives in
/// the attached type-child table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallTypeArgumentProjection {
    Present,
    Missing,
    InvalidPresent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallTypeApplicationTerminator {
    Closed,
    RecoveredMissing,
    InvalidPresent,
}

/// Source-owned part of one explicit Call type argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallTypeArgumentPart {
    Whole,
    Type,
}

/// Source-owned component of one explicit Call type application.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallTypeApplicationComponentRole {
    Whole,
    TurbofishSeparator,
    OpenAngle,
    CloseAngle,
    RecoveryEnd,
    EmptyInsertion,
    Argument {
        argument: u16,
        part: SyntaxCallTypeArgumentPart,
    },
    Separator {
        following: u16,
    },
    TrailingSeparator,
}

/// One source-ordered parenthesized Call argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxCallArgumentProjection {
    Positional {
        value: SyntaxExpressionSlot,
    },
    Named {
        name: Result<SyntaxName, SyntaxNameIssue>,
        equals: SyntaxRequiredTokenState,
        value: SyntaxExpressionSlot,
    },
    Spread {
        value: SyntaxExpressionSlot,
        ellipsis: SyntaxRequiredTokenState,
    },
}

impl SyntaxCallArgumentProjection {
    pub const fn value(&self) -> SyntaxExpressionSlot {
        match self {
            Self::Positional { value } | Self::Named { value, .. } | Self::Spread { value, .. } => {
                *value
            }
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Positional { value } => value.is_missing(),
            Self::Named {
                name,
                equals,
                value,
            } => {
                name.is_err()
                    || !matches!(equals, SyntaxRequiredTokenState::Present)
                    || value.is_missing()
            }
            Self::Spread { value, ellipsis } => {
                value.is_missing() || !matches!(ellipsis, SyntaxRequiredTokenState::Present)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRequiredTokenState {
    Present,
    Missing,
    InvalidPresent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallArgumentListTerminator {
    Closed,
    RecoveredMissing,
}

/// Callback-block Call metadata. The callback expression remains the owner of
/// its parameter patterns, parameter types, fat arrow, and body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxCallbackBlockCallProjection {
    callback: SyntaxExpressionSlot,
    terminator: SyntaxCallArgumentListTerminator,
}

impl SyntaxCallbackBlockCallProjection {
    pub(crate) const fn new(
        callback: SyntaxExpressionSlot,
        terminator: SyntaxCallArgumentListTerminator,
    ) -> Self {
        Self {
            callback,
            terminator,
        }
    }

    pub const fn callback(&self) -> SyntaxExpressionSlot {
        self.callback
    }

    pub const fn terminator(&self) -> SyntaxCallArgumentListTerminator {
        self.terminator
    }
}

/// Parser-owned closure parameter shape. Pattern and type identities remain
/// attached children rather than being copied into this projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxClosureParameterProjection {
    has_type: bool,
}

impl SyntaxClosureParameterProjection {
    pub(crate) const fn new(has_type: bool) -> Self {
        Self { has_type }
    }

    pub const fn has_type(self) -> bool {
        self.has_type
    }
}

/// One attached closure source shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxClosureProjection {
    parameters: Box<[SyntaxClosureParameterProjection]>,
    has_result_type: bool,
    body: SyntaxExpressionSlot,
    syntax: SyntaxClosureSyntax,
}

impl SyntaxClosureProjection {
    pub(crate) fn new(
        parameters: Vec<SyntaxClosureParameterProjection>,
        has_result_type: bool,
        body: SyntaxExpressionSlot,
        syntax: SyntaxClosureSyntax,
    ) -> Self {
        Self {
            parameters: parameters.into_boxed_slice(),
            has_result_type,
            body,
            syntax,
        }
    }

    pub fn parameters(&self) -> &[SyntaxClosureParameterProjection] {
        &self.parameters
    }

    pub const fn has_result_type(&self) -> bool {
        self.has_result_type
    }

    pub const fn body(&self) -> SyntaxExpressionSlot {
        self.body
    }

    pub const fn syntax(&self) -> SyntaxClosureSyntax {
        self.syntax
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxClosureSyntax {
    Pipe {
        terminator: SyntaxClosureTerminator,
    },
    CallbackBlock {
        explicit_header: bool,
        terminator: SyntaxClosureTerminator,
    },
    /// `expr:` followed by an indentation-owned callback body.
    IndentedCallback {
        terminator: SyntaxClosureTerminator,
    },
}

impl SyntaxClosureSyntax {
    pub const fn terminator(self) -> SyntaxClosureTerminator {
        match self {
            Self::Pipe { terminator }
            | Self::CallbackBlock { terminator, .. }
            | Self::IndentedCallback { terminator } => terminator,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxClosureTerminator {
    Closed,
    RecoveredMissing,
}

/// Source-owned part of one closure parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxClosureParameterPart {
    Whole,
    Pattern,
    Colon,
    Type,
}

/// Parser-owned state of one structural expression child slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxExpressionSlot {
    /// The slot owns one attached expression identity.
    Authored,
    /// The grammar retained one exact `MissingExpression` insertion.
    Missing,
}

impl SyntaxExpressionSlot {
    /// Whether this slot requires a typed synthetic recovery operand in HIR.
    pub const fn is_missing(self) -> bool {
        matches!(self, Self::Missing)
    }
}

/// Parser-selected borrow semantics independent of the detached expression model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxBorrowKind {
    Shared,
    Mutable,
}

/// Parser-selected unary operation independent of the detached expression model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxUnaryOperator {
    Not,
    Negate,
}

/// Parser-selected binary operation independent of the detached expression model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxBinaryOperator {
    Implies,
    Or,
    And,
    In,
    Equal,
    NotEqual,
    GreaterOrEqual,
    LessOrEqual,
    Greater,
    Less,
    Merge,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

/// Parser-selected semantic shape of one record field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxRecordField {
    Explicit {
        name: Result<SyntaxName, SyntaxNameIssue>,
        value: SyntaxExpressionSlot,
    },
    Shorthand {
        name: Result<SyntaxName, SyntaxNameIssue>,
    },
}

impl SyntaxRecordField {
    pub(crate) fn explicit(
        name: Result<SyntaxName, SyntaxNameIssue>,
        value: SyntaxExpressionSlot,
    ) -> Self {
        Self::Explicit { name, value }
    }

    pub(crate) fn shorthand(name: Result<SyntaxName, SyntaxNameIssue>) -> Self {
        Self::Shorthand { name }
    }

    pub const fn name(&self) -> &Result<SyntaxName, SyntaxNameIssue> {
        match self {
            Self::Explicit { name, .. } | Self::Shorthand { name } => name,
        }
    }

    pub const fn value(&self) -> Option<SyntaxExpressionSlot> {
        match self {
            Self::Explicit { value, .. } => Some(*value),
            Self::Shorthand { .. } => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        self.name().is_err() || self.value().is_some_and(SyntaxExpressionSlot::is_missing)
    }
}

/// One valid ID-less element of a compact numeric bracket sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNumericSequenceElement {
    integer: SyntaxIntegerLiteral,
}

impl SyntaxNumericSequenceElement {
    pub(crate) const fn new(integer: SyntaxIntegerLiteral) -> Self {
        Self { integer }
    }

    /// Exact arbitrary-width integer selected by the shared literal lexer.
    pub const fn integer(&self) -> &SyntaxIntegerLiteral {
        &self.integer
    }

    /// Number of radix-valid digits charged to the numeric-sequence limits.
    pub fn digit_count(&self) -> usize {
        self.integer.digits().len()
    }
}

/// Typed recovery retained on the compact numeric-sequence family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxNumericSequenceRecovery {
    Complete,
    MissingFinalElement {
        ordinal: u32,
    },
    InvalidElement {
        ordinal: u32,
        issue: SyntaxIntegerIssue,
        /// Lexer-owned digit charge of the malformed numeric token.
        digit_count: usize,
    },
    ConflictingSuffix {
        ordinal: u32,
        first: IntSuffix,
        conflicting: IntSuffix,
    },
}

/// Parser-owned compact numeric sequence without per-element syntax identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNumericSequence {
    elements: Box<[SyntaxNumericSequenceElement]>,
    common_suffix: Option<IntSuffix>,
    recovery: SyntaxNumericSequenceRecovery,
    total_digit_count: usize,
}

impl SyntaxNumericSequence {
    pub(crate) fn try_new(
        elements: Vec<SyntaxNumericSequenceElement>,
        common_suffix: Option<IntSuffix>,
        recovery: SyntaxNumericSequenceRecovery,
    ) -> Option<Self> {
        let retained_len = u32::try_from(elements.len()).ok()?;
        let recovery_is_valid = match &recovery {
            SyntaxNumericSequenceRecovery::Complete => true,
            SyntaxNumericSequenceRecovery::MissingFinalElement { ordinal }
            | SyntaxNumericSequenceRecovery::InvalidElement { ordinal, .. } => {
                *ordinal == retained_len
            }
            SyntaxNumericSequenceRecovery::ConflictingSuffix {
                ordinal,
                first,
                conflicting,
            } => {
                *ordinal < retained_len
                    && Some(*first) == common_suffix
                    && first != conflicting
                    && elements
                        .get(usize::try_from(*ordinal).ok()?)
                        .and_then(|element| element.integer().suffix())
                        == Some(*conflicting)
            }
        };
        if !recovery_is_valid
            || elements.iter().enumerate().any(|(ordinal, element)| {
                let suffix = element.integer().suffix();
                let is_conflicting = matches!(
                    &recovery,
                    SyntaxNumericSequenceRecovery::ConflictingSuffix {
                        ordinal: conflict,
                        ..
                    } if usize::try_from(*conflict).ok() == Some(ordinal)
                );
                !is_conflicting && suffix.is_some() && suffix != common_suffix
            })
            || common_suffix.is_some()
                && !elements
                    .iter()
                    .any(|element| element.integer().suffix() == common_suffix)
        {
            return None;
        }

        let mut total_digit_count = elements.iter().try_fold(0usize, |total, element| {
            total.checked_add(element.digit_count())
        })?;
        if let SyntaxNumericSequenceRecovery::InvalidElement { digit_count, .. } = &recovery {
            total_digit_count = total_digit_count.checked_add(*digit_count)?;
        }
        Some(Self {
            elements: elements.into_boxed_slice(),
            common_suffix,
            recovery,
            total_digit_count,
        })
    }

    /// Valid prefix elements retained in authored order.
    pub fn elements(&self) -> &[SyntaxNumericSequenceElement] {
        &self.elements
    }

    /// The one suffix shared by every explicitly suffixed valid element.
    pub const fn common_suffix(&self) -> Option<IntSuffix> {
        self.common_suffix
    }

    /// Exact typed recovery selected by the parser transaction.
    pub const fn recovery(&self) -> &SyntaxNumericSequenceRecovery {
        &self.recovery
    }

    /// Source-role count, including one missing or invalid terminal slot.
    pub fn source_element_count(&self) -> usize {
        self.elements.len()
            + usize::from(matches!(
                &self.recovery,
                SyntaxNumericSequenceRecovery::MissingFinalElement { .. }
                    | SyntaxNumericSequenceRecovery::InvalidElement { .. }
            ))
    }

    /// Checked sum used for the aggregate numeric digit limit.
    pub const fn total_digit_count(&self) -> usize {
        self.total_digit_count
    }

    /// Whether the known numeric family retained typed recovery.
    pub const fn has_recovery(&self) -> bool {
        !matches!(&self.recovery, SyntaxNumericSequenceRecovery::Complete)
    }
}

/// Closed meaning of an authored placeholder marker.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxPlaceholderKind {
    /// `_`, the partial-application placeholder.
    PartialApplication,
    /// `^`, the left value supplied by a pipe expression.
    PipeLeft,
}

/// Runtime lifetime-registry path retained through typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLifetimeRegistryPath {
    scope: SyntaxLifetimeRegistryScope,
    segments: Box<[Result<SyntaxName, SyntaxNameIssue>]>,
    optional: bool,
}

impl SyntaxLifetimeRegistryPath {
    pub(crate) fn new(
        scope: SyntaxLifetimeRegistryScope,
        segments: Vec<Result<SyntaxName, SyntaxNameIssue>>,
        optional: bool,
    ) -> Self {
        Self {
            scope,
            segments: segments.into_boxed_slice(),
            optional,
        }
    }

    /// Scope selected by the lifetime token transaction.
    pub const fn scope(&self) -> &SyntaxLifetimeRegistryScope {
        &self.scope
    }

    /// Ordered registry-key segments, including typed recovered names.
    pub fn segments(&self) -> &[Result<SyntaxName, SyntaxNameIssue>] {
        &self.segments
    }

    /// Whether the terminal `?` belongs to this registry read.
    pub const fn is_optional(&self) -> bool {
        self.optional
    }

    /// Whether the recognized lifetime-path family contains typed recovery.
    pub fn has_recovery(&self) -> bool {
        self.scope.has_recovery() || self.segments.iter().any(Result::is_err)
    }
}

/// Runtime registry scope selected before HIR lifetime-access checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxLifetimeRegistryScope {
    Frame,
    Tick,
    Cue,
    Line,
    Scene,
    Flow,
    Session,
    Global,
    Persistent,
    Named(SyntaxName),
    Recovered(SyntaxNameIssue),
}

impl SyntaxLifetimeRegistryScope {
    pub(crate) fn from_name(name: Result<SyntaxName, SyntaxNameIssue>) -> Self {
        match name {
            Ok(name) => match name.as_str() {
                "frame" => Self::Frame,
                "tick" => Self::Tick,
                "cue" => Self::Cue,
                "line" => Self::Line,
                "scene" => Self::Scene,
                "flow" => Self::Flow,
                "session" => Self::Session,
                "global" => Self::Global,
                "persistent" => Self::Persistent,
                _ => Self::Named(name),
            },
            Err(issue) => Self::Recovered(issue),
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

/// Typed authored part of a literal expression.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpressionLiteralPart {
    Body,
    Prefix,
    Suffix,
    Unit,
}

/// Source-component role fixed by the expression grammar transaction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpressionComponentRole {
    Literal(ExpressionLiteralPart),
    EntityReference(SyntaxIdRefPart),
    LifetimeScope,
    LifetimeKeySegment {
        ordinal: u32,
    },
    LifetimeOptionalMarker,
    ShortVariantMarker,
    ShortVariantName,
    PlaceholderMarker,
    Element {
        ordinal: u32,
    },
    NumericElement {
        ordinal: u32,
    },
    NumericCommonSuffix,
    RepeatValue,
    RepeatLength,
    CallCallee,
    CallAssociatedReceiver,
    CallAssociatedSeparator,
    CallAssociatedMember,
    CallArgumentListOpen,
    CallArgumentListClose,
    CallArgumentListRecoveryEnd,
    CallArgumentListEmptyInsertion,
    CallArgumentSeparator {
        following: u16,
    },
    CallArgumentTrailingSeparator,
    CallArgument {
        argument: u16,
        part: SyntaxCallArgumentPart,
    },
    CallTypeApplication(SyntaxCallTypeApplicationComponentRole),
    Target,
    OpenBracket,
    CloseBracket,
    Colon,
    Content,
    ContentBody,
    Plan,
    ConfigurationArgument {
        argument: u16,
        part: SyntaxDialogueConfigurationArgumentPart,
    },
    DialogueNode {
        ordinal: u32,
        part: SyntaxDialogueNodeSourcePart,
    },
    RichTextTag {
        tag: u32,
        part: SyntaxRichTextTagSourcePart,
    },
    RichTextArgument {
        tag: u32,
        argument: u16,
        part: SyntaxRichTextArgumentSourcePart,
    },
    SelectedMember,
    Index,
    LeftOperand,
    RightOperand,
    Operand,
    Operator,
    RangeStart,
    RangeEnd,
    RangeInclusiveMarker,
    AwaitWith,
    AwaitBranch {
        ordinal: u32,
    },
    RecordPath,
    RecordField {
        field: u32,
        part: ExpressionRecordFieldPart,
    },
    ClosureParameter {
        parameter: u16,
        part: SyntaxClosureParameterPart,
    },
    ClosureOpenDelimiter,
    ClosureCloseDelimiter,
    ClosureRecoveryEnd,
    ClosureParameterSeparator {
        following: u16,
    },
    ClosureFatArrow,
    ReturnType,
    Body,
    Statement {
        ordinal: u32,
    },
    Tail,
    Name,
    ThreadMode,
    Condition,
    Pattern,
    Scrutinee,
    Guard,
    ThenBranch,
    ElseBranch,
    MatchArm {
        arm: u32,
        part: SyntaxMatchArmPart,
    },
    Recovery,
}

/// Source-owned part of one ordinary Call argument.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallArgumentPart {
    Whole,
    Name,
    Equals,
    Value,
    Spread,
}

/// Source-owned part of one record field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpressionRecordFieldPart {
    Whole,
    Name,
    Colon,
    Value,
}
