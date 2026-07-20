use super::ExprParser;
use crate::ast::common::TextRange;
use crate::expr::call_syntax::{ArgumentListSyntaxInit, CallArgumentSyntaxInit};
use crate::expr::{
    ArgumentListSyntax, ArgumentListTerminatorSyntax, CallArg, CallArgumentFormSyntax,
    CallArgumentRecoverySyntax, CallRecoveryBoundarySyntax, CallRecoveryTokenKind, Expr, ExprOp,
    ExprParseError, MAX_CALL_ARGUMENTS, MAX_NESTED_CALLS, Token,
};

pub(super) struct ParsedCallArguments {
    pub(super) args: Vec<CallArg>,
    pub(super) syntax: ArgumentListSyntax,
}

struct ParsedCallArgument {
    arg: CallArg,
    syntax: CallArgumentSyntaxInit,
}

struct NamedArgumentHead {
    name: String,
    range: TextRange,
    equals: TextRange,
}

struct CallArgumentListState {
    open_paren: TextRange,
    args: Vec<CallArg>,
    arguments: Vec<CallArgumentSyntaxInit>,
    separators: Vec<TextRange>,
    trailing_comma: Option<TextRange>,
}

impl CallArgumentListState {
    fn new(open_paren: TextRange) -> Self {
        Self {
            open_paren,
            args: Vec::new(),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
        }
    }

    fn finish(
        self,
        terminator: ArgumentListTerminatorSyntax,
    ) -> (Vec<CallArg>, ArgumentListSyntaxInit) {
        (
            self.args,
            ArgumentListSyntaxInit {
                open_paren: self.open_paren,
                arguments: self.arguments,
                separators: self.separators,
                trailing_comma: self.trailing_comma,
                terminator,
            },
        )
    }
}

enum CallArgumentListBoundary {
    Continue(TextRange),
    Closed {
        close_paren: TextRange,
        trailing_comma: Option<TextRange>,
    },
    Recovered(Option<(CallRecoveryTokenKind, TextRange)>),
}

impl ExprParser {
    pub(super) fn parse_call_args(&mut self) -> Result<ParsedCallArguments, ExprParseError> {
        self.active_call_depth = self
            .active_call_depth
            .checked_add(1)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        if self.active_call_depth > MAX_NESTED_CALLS {
            self.active_call_depth = self
                .active_call_depth
                .checked_sub(1)
                .ok_or_else(|| Self::offset_overflow_error(self.base))?;
            return Err(ExprParseError::at(
                "syntax.expr.call_nesting_limit",
                "call nesting exceeds the inclusive limit of 32",
                self.absolute_range(self.peek_lexed())?,
            ));
        }
        let result = self.parse_call_args_inner();
        self.active_call_depth = self
            .active_call_depth
            .checked_sub(1)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        result
    }

    fn parse_call_args_inner(&mut self) -> Result<ParsedCallArguments, ExprParseError> {
        let open = self.bump_lexed();
        if open.token != Token::LParen {
            return Err(ExprParseError::at(
                "syntax.expr.parse",
                "expected `(` in argument list",
                self.absolute_range(&open)?,
            ));
        }
        let open_paren = self.absolute_range(&open)?;
        let mut state = CallArgumentListState::new(open_paren);
        if let Some(terminator) = self.initial_call_terminator(open_paren)? {
            return self.finish_call_argument_state(state, terminator);
        }

        loop {
            if matches!(self.peek(), Token::Comma | Token::RParen | Token::Eof)
                || call_recovery_token(self.peek()).is_some()
            {
                return Err(ExprParseError::at(
                    "syntax.expr.empty_call_argument",
                    "call argument must not be empty",
                    self.absolute_range(self.peek_lexed())?,
                ));
            }
            let argument = self.parse_call_argument_recovering()?;
            if state.arguments.len() >= MAX_CALL_ARGUMENTS {
                return Err(ExprParseError::at(
                    "syntax.expr.call_argument_limit",
                    "call argument count exceeds the inclusive limit of 128",
                    argument.syntax.range,
                ));
            }
            state.args.push(argument.arg);
            state.arguments.push(argument.syntax);
            match self.call_argument_list_boundary()? {
                CallArgumentListBoundary::Continue(separator) => {
                    state.separators.push(separator);
                }
                CallArgumentListBoundary::Closed {
                    close_paren,
                    trailing_comma,
                } => {
                    state.trailing_comma = trailing_comma;
                    return self.finish_call_argument_state(
                        state,
                        ArgumentListTerminatorSyntax::Closed { close_paren },
                    );
                }
                CallArgumentListBoundary::Recovered(boundary) => {
                    let terminator = self.recovered_missing_terminator(open_paren, boundary)?;
                    return self.finish_call_argument_state(state, terminator);
                }
            }
        }
    }

    fn initial_call_terminator(
        &mut self,
        open_paren: TextRange,
    ) -> Result<Option<ArgumentListTerminatorSyntax>, ExprParseError> {
        let terminator = match self.peek() {
            Token::RParen => {
                let close = self.bump_lexed();
                ArgumentListTerminatorSyntax::Closed {
                    close_paren: self.absolute_range(&close)?,
                }
            }
            Token::Eof => self.recovered_missing_terminator(open_paren, None)?,
            Token::Comma => return Ok(None),
            _ if call_recovery_token(self.peek()).is_some() => {
                let boundary = self.current_call_boundary()?;
                self.recovered_missing_terminator(open_paren, boundary)?
            }
            _ => return Ok(None),
        };
        Ok(Some(terminator))
    }

    fn call_argument_list_boundary(&mut self) -> Result<CallArgumentListBoundary, ExprParseError> {
        match self.peek() {
            Token::Comma => {
                let comma = self.bump_lexed();
                let comma_range = self.absolute_range(&comma)?;
                if self.peek() == &Token::RParen {
                    let close = self.bump_lexed();
                    return Ok(CallArgumentListBoundary::Closed {
                        close_paren: self.absolute_range(&close)?,
                        trailing_comma: Some(comma_range),
                    });
                }
                if self.peek() == &Token::Comma {
                    return Err(ExprParseError::at(
                        "syntax.expr.empty_call_argument",
                        "call argument must not be empty",
                        self.absolute_range(self.peek_lexed())?,
                    ));
                }
                Ok(CallArgumentListBoundary::Continue(comma_range))
            }
            Token::RParen => {
                let close = self.bump_lexed();
                Ok(CallArgumentListBoundary::Closed {
                    close_paren: self.absolute_range(&close)?,
                    trailing_comma: None,
                })
            }
            Token::Eof => Ok(CallArgumentListBoundary::Recovered(None)),
            _ if call_recovery_token(self.peek()).is_some() => Ok(
                CallArgumentListBoundary::Recovered(self.current_call_boundary()?),
            ),
            _ => Err(ExprParseError::at(
                "syntax.expr.missing_call_argument_separator",
                "expected `)` or `,` in argument list",
                self.absolute_range(self.peek_lexed())?,
            )),
        }
    }

    fn finish_call_argument_state(
        &self,
        state: CallArgumentListState,
        terminator: ArgumentListTerminatorSyntax,
    ) -> Result<ParsedCallArguments, ExprParseError> {
        let (args, init) = state.finish(terminator);
        self.finish_call_arguments(args, init)
    }

    fn parse_call_argument_recovering(&mut self) -> Result<ParsedCallArgument, ExprParseError> {
        let start = self.cursor;
        match self.parse_call_argument() {
            Ok(argument) => Ok(argument),
            Err(error) if error.permits_call_argument_recovery() => {
                self.cursor = start;
                self.recover_call_argument(start, error)
            }
            Err(error) => Err(error),
        }
    }

    fn parse_call_argument(&mut self) -> Result<ParsedCallArgument, ExprParseError> {
        let argument_start = self.cursor;
        let named = self.parse_named_arg_head()?;
        let value = self.parse_expr_bp_spanned(0)?;
        let spread = if self.peek() == &Token::Op(ExprOp::Spread) {
            let spread = self.bump_lexed();
            Some(self.absolute_range(&spread)?)
        } else {
            None
        };
        let argument_end = spread.map_or(value.range.end(), |ellipsis| ellipsis.end());
        let (arg, form, range) = if let Some(named) = named {
            if spread.is_some() {
                return Err(ExprParseError::at(
                    "syntax.expr.invalid_named_spread",
                    "named call arguments cannot use postfix spread",
                    TextRange::new(named.range.start(), argument_end),
                ));
            }
            (
                CallArg::Named {
                    name: named.name,
                    value: Box::new(value.expr),
                },
                CallArgumentFormSyntax::Named {
                    name: named.range,
                    equals: named.equals,
                },
                TextRange::new(named.range.start(), argument_end),
            )
        } else if let Some(ellipsis) = spread {
            (
                CallArg::Spread {
                    value: Box::new(value.expr),
                },
                CallArgumentFormSyntax::Spread { ellipsis },
                TextRange::new(value.range.start(), argument_end),
            )
        } else {
            (
                CallArg::Positional(value.expr),
                CallArgumentFormSyntax::Positional,
                value.range,
            )
        };
        if self.cursor == argument_start {
            return Err(ExprParseError::at(
                "syntax.expr.empty_call_argument",
                "call argument must not be empty",
                self.absolute_range(self.peek_lexed())?,
            ));
        }
        Ok(ParsedCallArgument {
            arg,
            syntax: CallArgumentSyntaxInit {
                range,
                value: value.range,
                form,
                recovery: CallArgumentRecoverySyntax::Parsed,
            },
        })
    }

    fn parse_named_arg_head(&mut self) -> Result<Option<NamedArgumentHead>, ExprParseError> {
        let mut cursor = self.cursor;
        let Some(first) = self.tokens.get(cursor) else {
            return Ok(None);
        };
        let Token::Ident(first_name) = &first.token else {
            return Ok(None);
        };
        let mut parts = vec![first_name.clone()];
        let first_start = first.start;
        let mut last_end = first.end;
        cursor = cursor
            .checked_add(1)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        while matches!(self.token_at(cursor), Token::Dot) {
            let part_index = cursor
                .checked_add(1)
                .ok_or_else(|| Self::offset_overflow_error(self.base))?;
            let Token::Ident(part) = self.token_at(part_index) else {
                return Ok(None);
            };
            parts.push(part.clone());
            last_end = self
                .tokens
                .get(part_index)
                .ok_or_else(|| Self::offset_overflow_error(self.base))?
                .end;
            cursor = cursor
                .checked_add(2)
                .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        }
        if self.token_at(cursor) != &Token::Op(ExprOp::Assign) {
            return Ok(None);
        }
        let equals = self.absolute_range(&self.tokens[cursor])?;
        self.cursor = cursor
            .checked_add(1)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        Ok(Some(NamedArgumentHead {
            name: parts.join("."),
            range: TextRange::new(
                self.absolute_offset(first_start)?,
                self.absolute_offset(last_end)?,
            ),
            equals,
        }))
    }

    fn recover_call_argument(
        &mut self,
        start: usize,
        error: ExprParseError,
    ) -> Result<ParsedCallArgument, ExprParseError> {
        let end = self.call_argument_sync_end(start)?;
        if start == end {
            return Err(error);
        }

        self.cursor = start;
        let named = self.parse_named_arg_head()?;
        let value_start = self.cursor;
        self.cursor = start;

        let has_spread = end
            .checked_sub(1)
            .is_some_and(|index| self.token_at(index) == &Token::Op(ExprOp::Spread));
        let value_end = end
            .checked_sub(usize::from(has_spread))
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        if value_start >= value_end {
            return Err(error);
        }
        let value_range = self.token_index_range(value_start, value_end)?;
        let raw = self
            .source_for_token_range(value_start, value_end)
            .ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.invalid_recovery_range",
                    "malformed argument recovery lost its source range",
                    value_range,
                )
            })?
            .to_owned();
        if raw.trim().is_empty() {
            return Err(error);
        }

        let diagnostic = ExprParseError::recovered_call_argument(&error, value_range);
        self.retain_recovery_diagnostic(diagnostic)?;
        self.cursor = end;

        let value = Expr::Raw(raw);
        let (arg, form, range) = if let Some(named) = named {
            if has_spread {
                return Err(error);
            }
            (
                CallArg::Named {
                    name: named.name,
                    value: Box::new(value),
                },
                CallArgumentFormSyntax::Named {
                    name: named.range,
                    equals: named.equals,
                },
                TextRange::new(named.range.start(), value_range.end()),
            )
        } else if has_spread {
            let ellipsis_index = end
                .checked_sub(1)
                .ok_or_else(|| Self::offset_overflow_error(self.base))?;
            let ellipsis = self.absolute_range(
                self.tokens
                    .get(ellipsis_index)
                    .ok_or_else(|| Self::offset_overflow_error(self.base))?,
            )?;
            (
                CallArg::Spread {
                    value: Box::new(value),
                },
                CallArgumentFormSyntax::Spread { ellipsis },
                TextRange::new(value_range.start(), ellipsis.end()),
            )
        } else {
            (
                CallArg::Positional(value),
                CallArgumentFormSyntax::Positional,
                value_range,
            )
        };
        Ok(ParsedCallArgument {
            arg,
            syntax: CallArgumentSyntaxInit {
                range,
                value: value_range,
                form,
                recovery: CallArgumentRecoverySyntax::Recovered {
                    diagnostic: value_range,
                },
            },
        })
    }

    fn call_argument_sync_end(&self, start: usize) -> Result<usize, ExprParseError> {
        let mut cursor = start;
        let mut paren = 0_u32;
        let mut bracket = 0_u32;
        let mut brace = 0_u32;
        loop {
            let token = self.token_at(cursor);
            let top_level = paren == 0 && bracket == 0 && brace == 0;
            if top_level
                && (matches!(token, Token::Comma | Token::RParen | Token::Eof)
                    || call_recovery_token(token).is_some())
            {
                return Ok(cursor);
            }
            match token {
                Token::LParen => {
                    paren = paren
                        .checked_add(1)
                        .ok_or_else(|| Self::offset_overflow_error(self.base))?;
                }
                Token::RParen => {
                    let range = self.absolute_range(&self.tokens[cursor])?;
                    paren = paren.checked_sub(1).ok_or_else(|| {
                        ExprParseError::at(
                            "syntax.expr.unbalanced_call_argument",
                            "unbalanced `)` while recovering a call argument",
                            range,
                        )
                    })?;
                }
                Token::LBracket => {
                    bracket = bracket
                        .checked_add(1)
                        .ok_or_else(|| Self::offset_overflow_error(self.base))?;
                }
                Token::RBracket => {
                    let range = self.absolute_range(&self.tokens[cursor])?;
                    bracket = bracket.checked_sub(1).ok_or_else(|| {
                        ExprParseError::at(
                            "syntax.expr.unbalanced_call_argument",
                            "unbalanced `]` while recovering a call argument",
                            range,
                        )
                    })?;
                }
                Token::LBrace => {
                    brace = brace
                        .checked_add(1)
                        .ok_or_else(|| Self::offset_overflow_error(self.base))?;
                }
                Token::RBrace => {
                    let range = self.absolute_range(&self.tokens[cursor])?;
                    brace = brace.checked_sub(1).ok_or_else(|| {
                        ExprParseError::at(
                            "syntax.expr.unbalanced_call_argument",
                            "unbalanced `}` while recovering a call argument",
                            range,
                        )
                    })?;
                }
                Token::Eof => return Ok(cursor),
                _ => {}
            }
            cursor = cursor
                .checked_add(1)
                .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        }
    }

    fn current_call_boundary(
        &self,
    ) -> Result<Option<(CallRecoveryTokenKind, TextRange)>, ExprParseError> {
        let Some(kind) = call_recovery_token(self.peek()) else {
            return Ok(None);
        };
        Ok(Some((kind, self.absolute_range(self.peek_lexed())?)))
    }

    fn recovered_missing_terminator(
        &mut self,
        open_paren: TextRange,
        current_boundary: Option<(CallRecoveryTokenKind, TextRange)>,
    ) -> Result<ArgumentListTerminatorSyntax, ExprParseError> {
        let boundary = current_boundary.map_or(self.end_boundary, |(kind, range)| {
            CallRecoveryBoundarySyntax::Token { kind, range }
        });
        let insertion = match boundary {
            CallRecoveryBoundarySyntax::EndOfExpression => self.recovery_end,
            CallRecoveryBoundarySyntax::Token { range, .. } => range.start(),
        };
        self.retain_recovery_diagnostic(ExprParseError::missing_call_close(insertion, open_paren))?;
        Ok(ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion,
            boundary,
        })
    }

    fn finish_call_arguments(
        &self,
        args: Vec<CallArg>,
        init: ArgumentListSyntaxInit,
    ) -> Result<ParsedCallArguments, ExprParseError> {
        let validation_end = self
            .validation_base
            .checked_add(self.validation_source.len())
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        let boundary_outside_fragment = matches!(
            &init.terminator,
            ArgumentListTerminatorSyntax::RecoveredMissing {
                boundary: CallRecoveryBoundarySyntax::Token { range, .. },
                ..
            } if range.end() > validation_end
        );
        let (validation_source, validation_base) = if boundary_outside_fragment {
            (self.owner_source.as_str(), self.owner_base)
        } else {
            (self.validation_source.as_str(), self.validation_base)
        };
        let syntax = ArgumentListSyntax::try_from_parser(validation_source, validation_base, init)
            .map_err(|error| {
                Self::call_invariant_error(
                    error,
                    TextRange::new(self.validation_base, validation_end),
                )
            })?;
        Ok(ParsedCallArguments { args, syntax })
    }
}

fn call_recovery_token(token: &Token) -> Option<CallRecoveryTokenKind> {
    match token {
        Token::Comma => Some(CallRecoveryTokenKind::Comma),
        Token::Semicolon => Some(CallRecoveryTokenKind::Semicolon),
        Token::Colon => Some(CallRecoveryTokenKind::Colon),
        Token::Op(ExprOp::FatArrow) => Some(CallRecoveryTokenKind::FatArrow),
        Token::RParen => Some(CallRecoveryTokenKind::CloseParen),
        Token::RBracket => Some(CallRecoveryTokenKind::CloseBracket),
        Token::RBrace => Some(CallRecoveryTokenKind::CloseBrace),
        _ => None,
    }
}
