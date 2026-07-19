use super::{CallArg, Expr};
use crate::ast::common::TextRange;
use thiserror::Error;

/// One authored call expression with semantic children and exact surface syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallExpr {
    callee: Box<Expr>,
    args: Vec<CallArg>,
    syntax: CallSurfaceSyntax,
}

/// Exhaustive authored surface carried by an ordinary call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallSurfaceSyntax {
    /// An authored parenthesized argument list.
    Parenthesized(ParenthesizedCallSyntax),
    /// A postfix callback block such as `items.map { item => item.label }`.
    CallbackBlock(CallbackBlockCallSyntax),
}

/// Exact syntax for a parenthesized call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParenthesizedCallSyntax {
    callee: TextRange,
    arguments: ArgumentListSyntax,
}

/// Exact syntax for a postfix callback-block call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackBlockCallSyntax {
    callee: TextRange,
    callback: CallbackBlockSyntax,
}

/// Exact authored syntax for one parenthesized argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentListSyntax {
    open_paren: TextRange,
    arguments: Vec<CallArgumentSyntax>,
    separators: Vec<TextRange>,
    trailing_comma: Option<TextRange>,
    terminator: ArgumentListTerminatorSyntax,
}

/// Authored or recovered termination of an argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentListTerminatorSyntax {
    /// The list has an authored closing parenthesis.
    Closed { close_paren: TextRange },
    /// The closing parenthesis is missing at an exact owner boundary.
    RecoveredMissing {
        insertion: usize,
        boundary: CallRecoveryBoundarySyntax,
    },
}

/// Boundary that stopped recovery of a missing closing parenthesis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallRecoveryBoundarySyntax {
    /// The owner-provided expression extent ended.
    EndOfExpression,
    /// An authored token remains owned by the enclosing construct.
    Token {
        kind: CallRecoveryTokenKind,
        range: TextRange,
    },
}

/// Token kinds that can terminate missing-parenthesis recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallRecoveryTokenKind {
    Comma,
    Semicolon,
    Colon,
    FatArrow,
    CloseParen,
    CloseBracket,
    CloseBrace,
}

impl CallRecoveryTokenKind {
    const fn spelling(self) -> &'static str {
        match self {
            Self::Comma => ",",
            Self::Semicolon => ";",
            Self::Colon => ":",
            Self::FatArrow => "=>",
            Self::CloseParen => ")",
            Self::CloseBracket => "]",
            Self::CloseBrace => "}",
        }
    }
}

/// Exact source ranges for one semantic call argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallArgumentSyntax {
    range: TextRange,
    value: TextRange,
    form: CallArgumentFormSyntax,
    recovery: CallArgumentRecoverySyntax,
}

/// Authored form of one call argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallArgumentFormSyntax {
    Positional,
    Named { name: TextRange, equals: TextRange },
    Spread { ellipsis: TextRange },
}

/// Whether one nonempty argument value was recovered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallArgumentRecoverySyntax {
    Parsed,
    Recovered { diagnostic: TextRange },
}

/// Exact authored syntax for one postfix callback block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackBlockSyntax {
    open_brace: TextRange,
    parameters: CallbackParameterHeaderSyntax,
    body: TextRange,
    close_brace: TextRange,
}

/// Parameter header of a postfix callback block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallbackParameterHeaderSyntax {
    ImplicitZero,
    Explicit {
        parameters: Vec<CallbackParameterSyntax>,
        separators: Vec<TextRange>,
        fat_arrow: TextRange,
    },
}

/// Exact authored syntax for one callback parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackParameterSyntax {
    range: TextRange,
    pattern: TextRange,
    type_ascription: Option<CallbackParameterTypeSyntax>,
}

/// Exact type-ascription ranges for one callback parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallbackParameterTypeSyntax {
    colon: TextRange,
    ty: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArgumentListSyntaxInit {
    pub(crate) open_paren: TextRange,
    pub(crate) arguments: Vec<CallArgumentSyntaxInit>,
    pub(crate) separators: Vec<TextRange>,
    pub(crate) trailing_comma: Option<TextRange>,
    pub(crate) terminator: ArgumentListTerminatorSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallArgumentSyntaxInit {
    pub(crate) range: TextRange,
    pub(crate) value: TextRange,
    pub(crate) form: CallArgumentFormSyntax,
    pub(crate) recovery: CallArgumentRecoverySyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallbackBlockSyntaxInit {
    pub(crate) open_brace: TextRange,
    pub(crate) parameters: CallbackParameterHeaderSyntaxInit,
    pub(crate) body: TextRange,
    pub(crate) close_brace: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallbackParameterHeaderSyntaxInit {
    ImplicitZero,
    Explicit {
        parameters: Vec<CallbackParameterSyntaxInit>,
        separators: Vec<TextRange>,
        fat_arrow: TextRange,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CallbackParameterSyntaxInit {
    pub(crate) range: TextRange,
    pub(crate) pattern: TextRange,
    pub(crate) type_ascription: Option<CallbackParameterTypeSyntax>,
}

/// Internal failure to uphold parser-owned call syntax invariants.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum CallSyntaxInvariantError {
    #[error("call syntax range is not on a UTF-8 boundary")]
    InvalidUtf8Boundary,
    #[error("call syntax token range does not denote the required token")]
    InvalidTokenRange,
    #[error("call syntax ranges are not in source order")]
    RangeOrder,
    #[error("semantic and syntactic argument counts differ")]
    ArgumentCountMismatch,
    #[error("semantic and syntactic argument forms differ")]
    ArgumentFormMismatch,
    #[error("argument separator count is invalid")]
    SeparatorCountMismatch,
    #[error("trailing comma placement is invalid")]
    InvalidTrailingComma,
    #[error("call recovery boundary is invalid")]
    InvalidRecoveryBoundary,
    #[error("callee range is invalid")]
    InvalidCalleeRange,
    #[error("callback call does not carry exactly one matching closure argument")]
    InvalidCallbackArgument,
    #[error("callback parameter header is invalid")]
    InvalidCallbackParameterHeader,
    #[error("callback body range is invalid")]
    InvalidCallbackBody,
    #[error("call syntax offset arithmetic overflowed")]
    OffsetOverflow,
}

impl CallExpr {
    /// Semantic callee expression.
    pub fn callee(&self) -> &Expr {
        &self.callee
    }

    /// Semantic arguments in authored order.
    pub fn args(&self) -> &[CallArg] {
        &self.args
    }

    /// Exact authored call surface.
    pub const fn syntax(&self) -> &CallSurfaceSyntax {
        &self.syntax
    }

    /// Parenthesized syntax when parentheses were authored.
    pub const fn parenthesized_syntax(&self) -> Option<&ParenthesizedCallSyntax> {
        match &self.syntax {
            CallSurfaceSyntax::Parenthesized(syntax) => Some(syntax),
            CallSurfaceSyntax::CallbackBlock(_) => None,
        }
    }

    /// Callback-block syntax when braces were authored.
    pub const fn callback_block_syntax(&self) -> Option<&CallbackBlockCallSyntax> {
        match &self.syntax {
            CallSurfaceSyntax::Parenthesized(_) => None,
            CallSurfaceSyntax::CallbackBlock(syntax) => Some(syntax),
        }
    }

    /// Exact authored callee range.
    pub const fn callee_range(&self) -> TextRange {
        self.syntax.callee_range()
    }

    /// Exact authored range of the complete call.
    pub const fn range(&self) -> TextRange {
        self.syntax.range()
    }

    pub(crate) fn try_parenthesized(
        callee: Expr,
        args: Vec<CallArg>,
        syntax: ParenthesizedCallSyntax,
    ) -> Result<Self, CallSyntaxInvariantError> {
        validate_callee_before_surface(syntax.callee, syntax.arguments.open_paren)?;
        validate_argument_shapes(&args, syntax.arguments.arguments())?;
        Ok(Self {
            callee: Box::new(callee),
            args,
            syntax: CallSurfaceSyntax::Parenthesized(syntax),
        })
    }

    pub(crate) fn try_callback_block(
        callee: Expr,
        closure: Expr,
        syntax: CallbackBlockCallSyntax,
    ) -> Result<Self, CallSyntaxInvariantError> {
        validate_callee_before_surface(syntax.callee, syntax.callback.open_brace)?;
        let Expr::Closure { params, .. } = &closure else {
            return Err(CallSyntaxInvariantError::InvalidCallbackArgument);
        };
        if params.len() != syntax.callback.parameters().parameters().len() {
            return Err(CallSyntaxInvariantError::InvalidCallbackArgument);
        }
        Ok(Self {
            callee: Box::new(callee),
            args: vec![CallArg::Positional(closure)],
            syntax: CallSurfaceSyntax::CallbackBlock(syntax),
        })
    }
}

impl CallSurfaceSyntax {
    /// Exact authored callee range.
    pub const fn callee_range(&self) -> TextRange {
        match self {
            Self::Parenthesized(syntax) => syntax.callee_range(),
            Self::CallbackBlock(syntax) => syntax.callee_range(),
        }
    }

    /// Exact authored range of the complete call.
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Parenthesized(syntax) => syntax.range(),
            Self::CallbackBlock(syntax) => syntax.range(),
        }
    }

    /// Parenthesized argument-list carrier, if this surface has one.
    pub const fn argument_list(&self) -> Option<&ArgumentListSyntax> {
        match self {
            Self::Parenthesized(syntax) => Some(syntax.argument_list()),
            Self::CallbackBlock(_) => None,
        }
    }
}

impl ParenthesizedCallSyntax {
    /// Exact authored callee range.
    pub const fn callee_range(&self) -> TextRange {
        self.callee
    }

    /// Exact authored argument-list syntax.
    pub const fn argument_list(&self) -> &ArgumentListSyntax {
        &self.arguments
    }

    /// Exact authored range of the complete call.
    pub const fn range(&self) -> TextRange {
        TextRange::new(self.callee.start(), self.arguments.end_offset())
    }

    pub(crate) fn try_from_parser(
        callee: TextRange,
        arguments: ArgumentListSyntax,
    ) -> Result<Self, CallSyntaxInvariantError> {
        validate_callee_before_surface(callee, arguments.open_paren)?;
        Ok(Self { callee, arguments })
    }
}

impl CallbackBlockCallSyntax {
    /// Exact authored callee range.
    pub const fn callee_range(&self) -> TextRange {
        self.callee
    }

    /// Exact authored callback-block syntax.
    pub const fn callback(&self) -> &CallbackBlockSyntax {
        &self.callback
    }

    /// Exact authored range of the complete call.
    pub const fn range(&self) -> TextRange {
        TextRange::new(self.callee.start(), self.callback.close_brace.end())
    }

    pub(crate) fn try_from_parser(
        callee: TextRange,
        callback: CallbackBlockSyntax,
    ) -> Result<Self, CallSyntaxInvariantError> {
        validate_callee_before_surface(callee, callback.open_brace)?;
        Ok(Self { callee, callback })
    }
}

impl ArgumentListSyntax {
    /// Exact opening-parenthesis range.
    pub const fn open_paren(&self) -> TextRange {
        self.open_paren
    }

    /// Argument syntax entries in authored order.
    pub fn arguments(&self) -> &[CallArgumentSyntax] {
        &self.arguments
    }

    /// Between-argument comma ranges.
    pub fn separators(&self) -> &[TextRange] {
        &self.separators
    }

    /// Trailing comma range, which is not a between-argument separator.
    pub const fn trailing_comma(&self) -> Option<TextRange> {
        self.trailing_comma
    }

    /// Authored or recovered list terminator.
    pub const fn terminator(&self) -> &ArgumentListTerminatorSyntax {
        &self.terminator
    }

    /// Exact closing parenthesis when authored.
    pub const fn close_paren(&self) -> Option<TextRange> {
        self.terminator.close_paren()
    }

    /// Missing-close recovery boundary, if any.
    pub const fn recovery_boundary(&self) -> Option<CallRecoveryBoundarySyntax> {
        match self.terminator {
            ArgumentListTerminatorSyntax::Closed { .. } => None,
            ArgumentListTerminatorSyntax::RecoveredMissing { boundary, .. } => Some(boundary),
        }
    }

    /// Exclusive end of the authored or recovered list.
    pub const fn end_offset(&self) -> usize {
        self.terminator.end_offset()
    }

    /// Exact range of the argument-list carrier.
    pub const fn range(&self) -> TextRange {
        TextRange::new(self.open_paren.start(), self.end_offset())
    }

    /// Range between the opening parenthesis and close/insertion point.
    pub const fn content_range(&self) -> TextRange {
        let end = match self.terminator {
            ArgumentListTerminatorSyntax::Closed { close_paren } => close_paren.start(),
            ArgumentListTerminatorSyntax::RecoveredMissing { insertion, .. } => insertion,
        };
        TextRange::new(self.open_paren.end(), end)
    }

    /// Whether a cursor can select this list for signature help.
    pub const fn contains_signature_cursor(&self, cursor: usize) -> bool {
        self.open_paren.end() <= cursor && cursor <= self.content_range().end()
    }

    /// Zero-based syntactic argument slot at a cursor inside this list.
    pub fn active_argument_slot(&self, cursor: usize) -> Option<usize> {
        self.contains_signature_cursor(cursor).then(|| {
            self.separators
                .iter()
                .filter(|separator| separator.end() <= cursor)
                .count()
                + usize::from(
                    self.trailing_comma
                        .is_some_and(|comma| comma.end() <= cursor),
                )
        })
    }

    pub(crate) fn try_from_parser(
        source: &str,
        source_base: usize,
        init: ArgumentListSyntaxInit,
    ) -> Result<Self, CallSyntaxInvariantError> {
        let validator = SourceValidator::new(source, source_base)?;
        validator.token(init.open_paren, "(")?;
        validate_terminator(&validator, &init.terminator)?;
        let content_end = terminator_content_end(&init.terminator);
        if init.open_paren.end() > content_end {
            return Err(CallSyntaxInvariantError::RangeOrder);
        }

        let arguments = init
            .arguments
            .into_iter()
            .map(|argument| CallArgumentSyntax::try_from_parser(&validator, argument))
            .collect::<Result<Vec<_>, _>>()?;
        if init.separators.len() != arguments.len().saturating_sub(1) {
            return Err(CallSyntaxInvariantError::SeparatorCountMismatch);
        }
        for separator in &init.separators {
            validator.token(*separator, ",")?;
        }
        if let Some(trailing) = init.trailing_comma {
            validator.token(trailing, ",")?;
            if arguments.is_empty() {
                return Err(CallSyntaxInvariantError::InvalidTrailingComma);
            }
        }

        let mut previous_end = init.open_paren.end();
        for (index, argument) in arguments.iter().enumerate() {
            if argument.range.start() < previous_end || argument.range.end() > content_end {
                return Err(CallSyntaxInvariantError::RangeOrder);
            }
            if let Some(separator) = init.separators.get(index) {
                let Some(next) = arguments.get(index + 1) else {
                    return Err(CallSyntaxInvariantError::SeparatorCountMismatch);
                };
                if separator.start() < argument.range.end() || separator.end() > next.range.start()
                {
                    return Err(CallSyntaxInvariantError::RangeOrder);
                }
                previous_end = separator.end();
            } else {
                previous_end = argument.range.end();
            }
        }
        if let Some(trailing) = init.trailing_comma {
            let last = arguments
                .last()
                .ok_or(CallSyntaxInvariantError::InvalidTrailingComma)?;
            if trailing.start() < last.range.end() || trailing.end() > content_end {
                return Err(CallSyntaxInvariantError::InvalidTrailingComma);
            }
        }

        Ok(Self {
            open_paren: init.open_paren,
            arguments,
            separators: init.separators,
            trailing_comma: init.trailing_comma,
            terminator: init.terminator,
        })
    }
}

impl ArgumentListTerminatorSyntax {
    /// Exact closing parenthesis when authored.
    pub const fn close_paren(&self) -> Option<TextRange> {
        match self {
            Self::Closed { close_paren } => Some(*close_paren),
            Self::RecoveredMissing { .. } => None,
        }
    }

    /// Exclusive end of the authored or recovered list.
    pub const fn end_offset(&self) -> usize {
        match self {
            Self::Closed { close_paren } => close_paren.end(),
            Self::RecoveredMissing { insertion, .. } => *insertion,
        }
    }
}

impl CallArgumentSyntax {
    /// Full authored argument range.
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Value-expression range.
    pub const fn value_range(&self) -> TextRange {
        self.value
    }

    /// Positional, named, or spread surface.
    pub const fn form(&self) -> &CallArgumentFormSyntax {
        &self.form
    }

    /// Parsed or recovered value state.
    pub const fn recovery(&self) -> CallArgumentRecoverySyntax {
        self.recovery
    }

    fn try_from_parser(
        validator: &SourceValidator<'_>,
        init: CallArgumentSyntaxInit,
    ) -> Result<Self, CallSyntaxInvariantError> {
        validator.nonempty_range(init.range)?;
        validator.nonempty_range(init.value)?;
        if init.value.start() < init.range.start() || init.value.end() > init.range.end() {
            return Err(CallSyntaxInvariantError::RangeOrder);
        }
        match &init.form {
            CallArgumentFormSyntax::Positional => {
                if init.range != init.value {
                    return Err(CallSyntaxInvariantError::ArgumentFormMismatch);
                }
            }
            CallArgumentFormSyntax::Named { name, equals } => {
                validator.nonempty_range(*name)?;
                validator.token(*equals, "=")?;
                if init.range.start() != name.start()
                    || name.end() > equals.start()
                    || equals.end() > init.value.start()
                    || init.range.end() != init.value.end()
                {
                    return Err(CallSyntaxInvariantError::ArgumentFormMismatch);
                }
            }
            CallArgumentFormSyntax::Spread { ellipsis } => {
                validator.token(*ellipsis, "...")?;
                if init.range.start() != init.value.start()
                    || init.value.end() > ellipsis.start()
                    || init.range.end() != ellipsis.end()
                {
                    return Err(CallSyntaxInvariantError::ArgumentFormMismatch);
                }
            }
        }
        if let CallArgumentRecoverySyntax::Recovered { diagnostic } = init.recovery {
            validator.nonempty_range(diagnostic)?;
            if diagnostic.start() < init.value.start() || diagnostic.end() > init.value.end() {
                return Err(CallSyntaxInvariantError::RangeOrder);
            }
        }
        Ok(Self {
            range: init.range,
            value: init.value,
            form: init.form,
            recovery: init.recovery,
        })
    }
}

impl CallbackBlockSyntax {
    /// Exact opening-brace range.
    pub const fn open_brace(&self) -> TextRange {
        self.open_brace
    }

    /// Exact callback parameter header.
    pub const fn parameters(&self) -> &CallbackParameterHeaderSyntax {
        &self.parameters
    }

    /// Exact nontrivia body range.
    pub const fn body_range(&self) -> TextRange {
        self.body
    }

    /// Exact closing-brace range.
    pub const fn close_brace(&self) -> TextRange {
        self.close_brace
    }

    /// Exact brace-delimited callback range.
    pub const fn closure_range(&self) -> TextRange {
        TextRange::new(self.open_brace.start(), self.close_brace.end())
    }

    pub(crate) fn try_from_parser(
        source: &str,
        source_base: usize,
        init: CallbackBlockSyntaxInit,
    ) -> Result<Self, CallSyntaxInvariantError> {
        let validator = SourceValidator::new(source, source_base)?;
        validator.token(init.open_brace, "{")?;
        validator.token(init.close_brace, "}")?;
        validator.nonempty_range(init.body)?;
        if init.open_brace.end() > init.body.start() || init.body.end() > init.close_brace.start() {
            return Err(CallSyntaxInvariantError::InvalidCallbackBody);
        }

        let parameters = match init.parameters {
            CallbackParameterHeaderSyntaxInit::ImplicitZero => {
                CallbackParameterHeaderSyntax::ImplicitZero
            }
            CallbackParameterHeaderSyntaxInit::Explicit {
                parameters,
                separators,
                fat_arrow,
            } => {
                if parameters.is_empty() {
                    return Err(CallSyntaxInvariantError::InvalidCallbackParameterHeader);
                }
                validator.token(fat_arrow, "=>")?;
                if separators.len() != parameters.len().saturating_sub(1) {
                    return Err(CallSyntaxInvariantError::SeparatorCountMismatch);
                }
                for separator in &separators {
                    validator.token(*separator, ",")?;
                }
                let parameters = parameters
                    .into_iter()
                    .map(|parameter| {
                        CallbackParameterSyntax::try_from_parser(&validator, parameter)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut previous_end = init.open_brace.end();
                for (index, parameter) in parameters.iter().enumerate() {
                    if parameter.range.start() < previous_end
                        || parameter.range.end() > fat_arrow.start()
                    {
                        return Err(CallSyntaxInvariantError::InvalidCallbackParameterHeader);
                    }
                    if let Some(separator) = separators.get(index) {
                        let Some(next) = parameters.get(index + 1) else {
                            return Err(CallSyntaxInvariantError::SeparatorCountMismatch);
                        };
                        if separator.start() < parameter.range.end()
                            || separator.end() > next.range.start()
                        {
                            return Err(CallSyntaxInvariantError::InvalidCallbackParameterHeader);
                        }
                        previous_end = separator.end();
                    } else {
                        previous_end = parameter.range.end();
                    }
                }
                if parameters
                    .last()
                    .is_none_or(|parameter| parameter.range.end() > fat_arrow.start())
                    || fat_arrow.end() > init.body.start()
                {
                    return Err(CallSyntaxInvariantError::InvalidCallbackParameterHeader);
                }
                CallbackParameterHeaderSyntax::Explicit {
                    parameters,
                    separators,
                    fat_arrow,
                }
            }
        };

        Ok(Self {
            open_brace: init.open_brace,
            parameters,
            body: init.body,
            close_brace: init.close_brace,
        })
    }
}

impl CallbackParameterHeaderSyntax {
    /// Callback parameters in authored order.
    pub fn parameters(&self) -> &[CallbackParameterSyntax] {
        match self {
            Self::ImplicitZero => &[],
            Self::Explicit { parameters, .. } => parameters,
        }
    }

    /// Between-parameter comma ranges.
    pub fn separators(&self) -> &[TextRange] {
        match self {
            Self::ImplicitZero => &[],
            Self::Explicit { separators, .. } => separators,
        }
    }

    /// Exact `=>` range for an explicit header.
    pub const fn fat_arrow(&self) -> Option<TextRange> {
        match self {
            Self::ImplicitZero => None,
            Self::Explicit { fat_arrow, .. } => Some(*fat_arrow),
        }
    }

    /// Whether this block has the implicit zero-parameter form.
    pub const fn is_implicit_zero(&self) -> bool {
        matches!(self, Self::ImplicitZero)
    }
}

impl CallbackParameterSyntax {
    /// Full authored parameter range.
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Exact pattern range.
    pub const fn pattern_range(&self) -> TextRange {
        self.pattern
    }

    /// Optional type-ascription ranges.
    pub const fn type_ascription(&self) -> Option<CallbackParameterTypeSyntax> {
        self.type_ascription
    }

    fn try_from_parser(
        validator: &SourceValidator<'_>,
        init: CallbackParameterSyntaxInit,
    ) -> Result<Self, CallSyntaxInvariantError> {
        validator.nonempty_range(init.range)?;
        validator.nonempty_range(init.pattern)?;
        if init.range.start() != init.pattern.start() || init.pattern.end() > init.range.end() {
            return Err(CallSyntaxInvariantError::InvalidCallbackParameterHeader);
        }
        if let Some(type_ascription) = init.type_ascription {
            validator.token(type_ascription.colon, ":")?;
            validator.nonempty_range(type_ascription.ty)?;
            if init.pattern.end() > type_ascription.colon.start()
                || type_ascription.colon.end() > type_ascription.ty.start()
                || init.range.end() != type_ascription.ty.end()
            {
                return Err(CallSyntaxInvariantError::InvalidCallbackParameterHeader);
            }
        } else if init.range != init.pattern {
            return Err(CallSyntaxInvariantError::InvalidCallbackParameterHeader);
        }
        Ok(Self {
            range: init.range,
            pattern: init.pattern,
            type_ascription: init.type_ascription,
        })
    }
}

impl CallbackParameterTypeSyntax {
    /// Exact colon range.
    pub const fn colon(&self) -> TextRange {
        self.colon
    }

    /// Exact type range.
    pub const fn ty_range(&self) -> TextRange {
        self.ty
    }

    pub(crate) const fn new(colon: TextRange, ty: TextRange) -> Self {
        Self { colon, ty }
    }
}

fn validate_argument_shapes(
    args: &[CallArg],
    syntax: &[CallArgumentSyntax],
) -> Result<(), CallSyntaxInvariantError> {
    if args.len() != syntax.len() {
        return Err(CallSyntaxInvariantError::ArgumentCountMismatch);
    }
    if args.iter().zip(syntax).any(|(arg, syntax)| {
        !matches!(
            (arg, syntax.form()),
            (CallArg::Positional(_), CallArgumentFormSyntax::Positional)
                | (CallArg::Named { .. }, CallArgumentFormSyntax::Named { .. })
                | (
                    CallArg::Spread { .. },
                    CallArgumentFormSyntax::Spread { .. }
                )
        )
    }) {
        return Err(CallSyntaxInvariantError::ArgumentFormMismatch);
    }
    Ok(())
}

fn validate_callee_before_surface(
    callee: TextRange,
    delimiter: TextRange,
) -> Result<(), CallSyntaxInvariantError> {
    if callee.start() >= callee.end() || callee.end() > delimiter.start() {
        return Err(CallSyntaxInvariantError::InvalidCalleeRange);
    }
    Ok(())
}

fn validate_terminator(
    validator: &SourceValidator<'_>,
    terminator: &ArgumentListTerminatorSyntax,
) -> Result<(), CallSyntaxInvariantError> {
    match terminator {
        ArgumentListTerminatorSyntax::Closed { close_paren } => validator.token(*close_paren, ")"),
        ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion,
            boundary,
        } => {
            validator.offset(*insertion)?;
            match boundary {
                CallRecoveryBoundarySyntax::EndOfExpression => {
                    if *insertion != validator.end {
                        return Err(CallSyntaxInvariantError::InvalidRecoveryBoundary);
                    }
                }
                CallRecoveryBoundarySyntax::Token { kind, range } => {
                    validator.token(*range, kind.spelling())?;
                    if *insertion != range.start() {
                        return Err(CallSyntaxInvariantError::InvalidRecoveryBoundary);
                    }
                }
            }
            Ok(())
        }
    }
}

const fn terminator_content_end(terminator: &ArgumentListTerminatorSyntax) -> usize {
    match terminator {
        ArgumentListTerminatorSyntax::Closed { close_paren } => close_paren.start(),
        ArgumentListTerminatorSyntax::RecoveredMissing { insertion, .. } => *insertion,
    }
}

struct SourceValidator<'a> {
    source: &'a str,
    base: usize,
    end: usize,
}

impl<'a> SourceValidator<'a> {
    fn new(source: &'a str, base: usize) -> Result<Self, CallSyntaxInvariantError> {
        let end = base
            .checked_add(source.len())
            .ok_or(CallSyntaxInvariantError::OffsetOverflow)?;
        Ok(Self { source, base, end })
    }

    fn offset(&self, offset: usize) -> Result<(), CallSyntaxInvariantError> {
        if offset < self.base || offset > self.end {
            return Err(CallSyntaxInvariantError::RangeOrder);
        }
        let relative = offset
            .checked_sub(self.base)
            .ok_or(CallSyntaxInvariantError::OffsetOverflow)?;
        if !self.source.is_char_boundary(relative) {
            return Err(CallSyntaxInvariantError::InvalidUtf8Boundary);
        }
        Ok(())
    }

    fn nonempty_range(&self, range: TextRange) -> Result<(), CallSyntaxInvariantError> {
        self.range(range)?;
        if range.start() == range.end() {
            return Err(CallSyntaxInvariantError::RangeOrder);
        }
        Ok(())
    }

    fn range(&self, range: TextRange) -> Result<(), CallSyntaxInvariantError> {
        if range.start() > range.end() {
            return Err(CallSyntaxInvariantError::RangeOrder);
        }
        self.offset(range.start())?;
        self.offset(range.end())
    }

    fn token(&self, range: TextRange, expected: &str) -> Result<(), CallSyntaxInvariantError> {
        self.nonempty_range(range)?;
        let start = range
            .start()
            .checked_sub(self.base)
            .ok_or(CallSyntaxInvariantError::OffsetOverflow)?;
        let end = range
            .end()
            .checked_sub(self.base)
            .ok_or(CallSyntaxInvariantError::OffsetOverflow)?;
        if self.source.get(start..end) != Some(expected) {
            return Err(CallSyntaxInvariantError::InvalidTokenRange);
        }
        Ok(())
    }
}
