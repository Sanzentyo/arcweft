use super::call_syntax::CallSyntaxInvariantError;
use super::{
    AssociatedMemberSeparatorSyntax, BinaryOp, CallExpr, CallRecoveryBoundarySyntax,
    CallbackBlockCallSyntax, DialogueContent, DottedPath, Expr, ExprOp, ExprParseError,
    ExprParseStats, FlowItem, LexedToken, Lexer, LinePlan, Literal, MAX_EXPR_DIAGNOSTICS,
    MAX_EXPR_RECOVERY_NODES, Name, ParenthesizedCallSyntax, ParenthesizedCalleeSyntax, ParsedExpr,
    PathMemberCalleeSyntax, Stmt, ThreadBlock, ThreadModifier, Token,
    flat_literal_bracket_seq_expr, literal_exprs_from_tokens, nonempty_joined_name, parse_expr,
    token_source,
};
use crate::ast::common::TextRange;
use crate::types::{
    AuthoredTypeRef, ParsedGenericCallee, ParsedTypeReceiver, TypeToken, TypeTokenCursor,
    TypeTokenKind,
};

mod call;

pub(super) struct SpannedExpr {
    pub(super) expr: Expr,
    pub(super) range: TextRange,
    type_receiver: Option<AuthoredTypeRef>,
    path_member: Option<PathMemberCalleeSyntax>,
    explicit_type_application: Option<Box<AuthoredTypeRef>>,
}

enum ParsedTypePrefix {
    Receiver(ParsedTypeReceiver),
    GenericCallee(ParsedGenericCallee),
}

pub(super) struct DialoguePrimary {
    pub(super) open: usize,
    pub(super) end: usize,
    pub(super) content: DialogueContent,
    pub(super) plan: Option<LinePlan>,
}

#[derive(Clone, Copy)]
pub(super) struct ExprParserScope<'a> {
    pub(super) source: &'a str,
    pub(super) base: usize,
    pub(super) validation_source: &'a str,
    pub(super) validation_base: usize,
    pub(super) owner_source: &'a str,
    pub(super) owner_base: usize,
    pub(super) end_boundary: CallRecoveryBoundarySyntax,
    pub(super) recovery_end: usize,
}

pub(super) struct ExprParser {
    pub(super) source: String,
    pub(super) base: usize,
    pub(super) validation_source: String,
    pub(super) validation_base: usize,
    pub(super) owner_source: String,
    pub(super) owner_base: usize,
    pub(super) end_boundary: CallRecoveryBoundarySyntax,
    pub(super) recovery_end: usize,
    pub(super) tokens: Vec<LexedToken>,
    pub(super) cursor: usize,
    stats: ExprParseStats,
    diagnostics: Vec<ExprParseError>,
    recovery_nodes: usize,
    active_call_depth: usize,
    pub(super) prefix_depth: usize,
    pub(super) control_body_brace_is_boundary: bool,
    pub(super) dialogue_primary: Option<DialoguePrimary>,
}

impl ExprParser {
    pub(super) fn new_scoped(scope: ExprParserScope<'_>) -> Self {
        Self {
            source: scope.source.to_owned(),
            base: scope.base,
            validation_source: scope.validation_source.to_owned(),
            validation_base: scope.validation_base,
            owner_source: scope.owner_source.to_owned(),
            owner_base: scope.owner_base,
            end_boundary: scope.end_boundary,
            recovery_end: scope.recovery_end,
            tokens: Lexer::new(scope.source).tokenize(),
            cursor: 0,
            stats: ExprParseStats::default(),
            diagnostics: Vec::new(),
            recovery_nodes: 0,
            active_call_depth: 0,
            prefix_depth: 0,
            control_body_brace_is_boundary: false,
            dialogue_primary: None,
        }
    }

    pub(super) fn with_dialogue_primary(
        mut self,
        dialogue_primary: Option<DialoguePrimary>,
    ) -> Self {
        self.dialogue_primary = dialogue_primary;
        self
    }

    pub(super) fn parse(mut self) -> Result<ParsedExpr, ExprParseError> {
        let parsed = self.parse_expr_bp_spanned(0)?;
        if self.peek() != &Token::Eof {
            let unexpected = self.peek_lexed();
            return Err(ExprParseError::at(
                "syntax.expr.unexpected_token",
                &format!("unexpected token after expression: {:?}", unexpected.token),
                self.absolute_range(unexpected)?,
            ));
        }
        Ok(ParsedExpr {
            expr: parsed.expr,
            range: parsed.range,
            diagnostics: self.diagnostics,
            stats: self.stats,
        })
    }

    pub(super) fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ExprParseError> {
        self.parse_expr_bp_spanned(min_bp).map(|parsed| parsed.expr)
    }

    pub(super) fn parse_expr_bp_spanned(
        &mut self,
        min_bp: u8,
    ) -> Result<SpannedExpr, ExprParseError> {
        let expression_token_start = self.cursor;
        let expression_start = self.peek_lexed().start;
        let (type_prefix, recovered_callee) = if self.dialogue_primary.is_some() {
            (None, None)
        } else {
            match self.try_parse_type_prefix() {
                Ok(type_prefix) => (type_prefix, None),
                Err(diagnostic) => (
                    None,
                    Some(self.recover_type_callee(expression_token_start, diagnostic)?),
                ),
            }
        };
        let mut lhs = if let Some(recovered_callee) = recovered_callee {
            recovered_callee
        } else {
            match type_prefix {
                Some(ParsedTypePrefix::Receiver(receiver)) => {
                    let range = *receiver.authored().root_source().whole();
                    let path = receiver.authored().value().nominal_path().ok_or_else(|| {
                        ExprParseError::at(
                            "syntax.type.invalid_receiver",
                            "path-member receiver must have a nominal type head",
                            range,
                        )
                    })?;
                    let expression_path = DottedPath::from(path);
                    self.cursor = receiver.next_index();
                    SpannedExpr {
                        expr: Expr::Path(expression_path),
                        range,
                        type_receiver: Some(receiver.into_authored()),
                        path_member: None,
                        explicit_type_application: None,
                    }
                }
                Some(ParsedTypePrefix::GenericCallee(generic)) => {
                    let range = *generic.authored().root_source().whole();
                    let path = generic.authored().value().nominal_path().ok_or_else(|| {
                        ExprParseError::at(
                            "syntax.type.invalid_receiver",
                            "generic call target must have a nominal path head",
                            range,
                        )
                    })?;
                    let expression_path = DottedPath::from(path);
                    self.cursor = generic.next_index();
                    SpannedExpr {
                        expr: Expr::Path(expression_path),
                        range,
                        type_receiver: None,
                        path_member: None,
                        explicit_type_application: Some(Box::new(generic.into_authored())),
                    }
                }
                None => {
                    let prefix = self.parse_prefix()?;
                    SpannedExpr {
                        expr: prefix,
                        range: self.consumed_range(expression_start)?,
                        type_receiver: None,
                        path_member: None,
                        explicit_type_application: None,
                    }
                }
            }
        };
        loop {
            lhs = match self.peek() {
                Token::Question if min_bp <= 100 => self.parse_try_postfix(lhs)?,
                Token::LParen if min_bp <= 100 => self.parse_call_postfix(lhs)?,
                Token::LBracket if min_bp <= 100 => self.parse_index_postfix(lhs)?,
                Token::Dot if min_bp <= 100 => self.parse_select_postfix(lhs)?,
                Token::DoubleColon if min_bp <= 100 => self.parse_associated_select_postfix(lhs)?,
                Token::Op(ExprOp::Range | ExprOp::RangeInclusive) if min_bp <= 5 => {
                    self.parse_range_postfix(lhs)?
                }
                Token::Amp | Token::Star | Token::Op(_) => {
                    let Some((op, right_bp, binary)) = self.current_infix(min_bp) else {
                        break;
                    };
                    self.parse_infix(lhs, op, right_bp, binary)?
                }
                _ => break,
            };
        }
        Ok(lhs)
    }

    fn try_parse_type_prefix(&self) -> Result<Option<ParsedTypePrefix>, ExprParseError> {
        let tokens = self.type_token_view()?;
        let cursor = TypeTokenCursor::try_new(&tokens, self.cursor)
            .map_err(|error| self.type_lookahead_error(&error))?;
        if let Some(receiver) = cursor
            .parse_receiver()
            .map_err(|error| self.type_lookahead_error(&error))?
        {
            return Ok(Some(ParsedTypePrefix::Receiver(receiver)));
        }
        cursor
            .parse_generic_callee()
            .map(|generic| generic.map(ParsedTypePrefix::GenericCallee))
            .map_err(|error| self.type_lookahead_error(&error))
    }

    fn recover_type_callee(
        &mut self,
        expression_token_start: usize,
        diagnostic: ExprParseError,
    ) -> Result<SpannedExpr, ExprParseError> {
        if !diagnostic.permits_type_callee_recovery() {
            return Err(diagnostic);
        }
        let Some(call_open) = self.terminal_malformed_type_call_open(expression_token_start) else {
            return Err(diagnostic);
        };
        let range = self.token_index_range(expression_token_start, call_open)?;
        let raw = self
            .source_for_token_range(expression_token_start, call_open)
            .ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.invalid_scope",
                    "recovered callee tokens are outside the parser source",
                    range,
                )
            })?
            .to_owned();
        self.retain_recovery_diagnostic(ExprParseError::recovered_type_callee(&diagnostic))?;
        self.cursor = call_open;
        Ok(SpannedExpr {
            expr: Expr::Raw(raw),
            range,
            type_receiver: None,
            path_member: None,
            explicit_type_application: None,
        })
    }

    fn terminal_malformed_type_call_open(&self, start: usize) -> Option<usize> {
        for separator in start..self.tokens.len() {
            if !matches!(
                self.tokens.get(separator)?.token,
                Token::Dot | Token::DoubleColon
            ) {
                continue;
            }
            let next = self.tokens.get(separator.checked_add(1)?)?;
            let call_open = match next.token {
                Token::LParen => separator.checked_add(1)?,
                Token::Ident(_) => {
                    let call_open = separator.checked_add(2)?;
                    matches!(self.tokens.get(call_open)?.token, Token::LParen)
                        .then_some(call_open)?
                }
                _ => continue,
            };
            if self.call_open_terminates_expression(call_open) {
                return Some(call_open);
            }
        }
        None
    }

    fn call_open_terminates_expression(&self, call_open: usize) -> bool {
        let mut depth = 0usize;
        for (index, token) in self.tokens.iter().enumerate().skip(call_open) {
            match token.token {
                Token::LParen => {
                    let Some(next_depth) = depth.checked_add(1) else {
                        return false;
                    };
                    depth = next_depth;
                }
                Token::RParen => {
                    let Some(next_depth) = depth.checked_sub(1) else {
                        return false;
                    };
                    depth = next_depth;
                    if depth == 0 {
                        return self
                            .tokens
                            .get(index.saturating_add(1))
                            .is_some_and(|next| matches!(next.token, Token::Eof));
                    }
                }
                Token::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn type_lookahead_error(&self, error: &crate::types::TypeParseError) -> ExprParseError {
        let fallback = self
            .absolute_range(self.peek_lexed())
            .unwrap_or(TextRange::new(self.base, self.base));
        ExprParseError::at(
            error.code(),
            &error.to_string(),
            error.range().unwrap_or(fallback),
        )
    }

    fn type_token_view(&self) -> Result<Vec<TypeToken<'_>>, ExprParseError> {
        let mut output = Vec::with_capacity(self.tokens.len().saturating_sub(1));
        for token in self.tokens.iter().take(self.tokens.len().saturating_sub(1)) {
            let source = self.source.get(token.start..token.end).ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.invalid_scope",
                    "expression token is outside the parser source",
                    TextRange::new(self.base, self.base),
                )
            })?;
            let kind = match &token.token {
                Token::Ident(value) => TypeTokenKind::Identifier(value),
                Token::LifetimePath { .. } => TypeTokenKind::Lifetime(source),
                Token::Literal(Literal::Int(_)) => TypeTokenKind::Integer(source),
                Token::Bang => TypeTokenKind::Bang,
                Token::Amp => TypeTokenKind::Ampersand,
                Token::LParen => TypeTokenKind::OpenParen,
                Token::RParen => TypeTokenKind::CloseParen,
                Token::LBracket => TypeTokenKind::OpenBracket,
                Token::RBracket => TypeTokenKind::CloseBracket,
                Token::LBrace => TypeTokenKind::OpenBrace,
                Token::RBrace => TypeTokenKind::CloseBrace,
                Token::Comma => TypeTokenKind::Comma,
                Token::Dot => TypeTokenKind::Dot,
                Token::DoubleColon => TypeTokenKind::PathSeparator,
                Token::Colon => TypeTokenKind::Colon,
                Token::Op(ExprOp::Assign) => TypeTokenKind::Equals,
                Token::Op(ExprOp::ClosurePipe) => TypeTokenKind::Pipe,
                Token::Op(ExprOp::ThinArrow) => TypeTokenKind::ThinArrow,
                Token::Op(ExprOp::Lt) => TypeTokenKind::OpenAngle,
                Token::Op(ExprOp::Gt) => TypeTokenKind::CloseAngle,
                Token::Entity(_)
                | Token::RelativePath(_)
                | Token::Literal(_)
                | Token::Invalid(_)
                | Token::Underscore
                | Token::Caret
                | Token::Semicolon
                | Token::Question
                | Token::Star
                | Token::Op(_)
                | Token::Eof => TypeTokenKind::Other,
            };
            output.push(TypeToken::from_parser(kind, self.absolute_range(token)?));
        }
        Ok(output)
    }

    fn parse_try_postfix(&mut self, lhs: SpannedExpr) -> Result<SpannedExpr, ExprParseError> {
        let question = self.bump_lexed();
        let question = self.absolute_range(&question)?;
        let range = TextRange::new(lhs.range.start(), question.end());
        Ok(SpannedExpr {
            expr: Expr::Try(super::TryExpr::new(
                Box::new(lhs.expr),
                super::TryExprSource::new(
                    range,
                    lhs.range,
                    super::TryOperatorSource::PostfixQuestion { question },
                ),
            )),
            range,
            type_receiver: None,
            path_member: None,
            explicit_type_application: None,
        })
    }

    fn parse_call_postfix(&mut self, lhs: SpannedExpr) -> Result<SpannedExpr, ExprParseError> {
        let arguments = self.parse_call_args()?;
        let callee = match (lhs.path_member, lhs.explicit_type_application) {
            (Some(callee), None) => ParenthesizedCalleeSyntax::PathMember(Box::new(callee)),
            (None, Some(application)) => {
                ParenthesizedCalleeSyntax::try_with_type_application(lhs.range, *application)
                    .map_err(|error| Self::call_invariant_error(error, lhs.range))?
            }
            (None, None) => ParenthesizedCalleeSyntax::ordinary(lhs.range),
            (Some(_), Some(_)) => {
                return Err(ExprParseError::at(
                    "syntax.expr.call_invariant",
                    "path-member and ordinary generic call syntax cannot overlap",
                    lhs.range,
                ));
            }
        };
        let syntax = ParenthesizedCallSyntax::try_from_parser(callee, arguments.syntax)
            .map_err(|error| Self::call_invariant_error(error, lhs.range))?;
        let call = CallExpr::try_parenthesized(lhs.expr, arguments.args, syntax)
            .map_err(|error| Self::call_invariant_error(error, lhs.range))?;
        Ok(SpannedExpr {
            range: call.range(),
            expr: Expr::Call(call),
            type_receiver: None,
            path_member: None,
            explicit_type_application: None,
        })
    }

    fn parse_index_postfix(&mut self, lhs: SpannedExpr) -> Result<SpannedExpr, ExprParseError> {
        self.bump();
        let index = if self.peek() == &Token::RBracket {
            Expr::Tuple(Vec::new())
        } else {
            self.parse_expr_bp(0)?
        };
        self.expect(&Token::RBracket)?;
        Ok(SpannedExpr {
            expr: Expr::Index {
                target: Box::new(lhs.expr),
                index: Box::new(index),
            },
            range: TextRange::new(
                lhs.range.start(),
                self.absolute_offset(self.previous_lexed_end())?,
            ),
            type_receiver: None,
            path_member: None,
            explicit_type_application: None,
        })
    }

    fn parse_select_postfix(&mut self, lhs: SpannedExpr) -> Result<SpannedExpr, ExprParseError> {
        let separator = self.bump_lexed();
        let separator = AssociatedMemberSeparatorSyntax::Dot {
            range: self.absolute_range(&separator)?,
        };
        let member_start = self.cursor;
        let member_token = self.bump_lexed();
        let identifier_range = self.absolute_range(&member_token)?;
        let Token::Ident(member) = member_token.token else {
            return Err(ExprParseError::at(
                "syntax.expr.parse",
                "expected selector name after `.`",
                identifier_range,
            ));
        };
        let generic_member = TypeTokenCursor::try_new(&self.type_token_view()?, member_start)
            .map_err(|error| self.type_lookahead_error(&error))?
            .parse_generic_member()
            .map_err(|error| self.type_lookahead_error(&error))?;
        let member = Name::new(member);
        let explicit_type_application = if let Some(generic_member) = generic_member {
            debug_assert_eq!(
                generic_member.authored().root_source().whole().start(),
                identifier_range.start()
            );
            let next_index = generic_member.next_index();
            self.cursor = next_index;
            Some(Box::new(generic_member.into_authored()))
        } else {
            None
        };
        let selected_end = explicit_type_application
            .as_ref()
            .map_or(identifier_range.end(), |application| {
                application.root_source().whole().end()
            });
        let selected_range = TextRange::new(lhs.range.start(), selected_end);
        let path_member = lhs.type_receiver.and_then(|receiver| {
            (receiver.root_source().whole().end() == separator.range().start()
                && separator.range().end() == identifier_range.start()
                && explicit_type_application.is_none())
            .then(|| {
                PathMemberCalleeSyntax::try_from_parser(
                    &self.source,
                    self.base,
                    receiver,
                    separator,
                    member.clone(),
                    identifier_range,
                    selected_range,
                )
                .ok()
            })
            .flatten()
        });
        let selected = SpannedExpr {
            expr: Expr::select(lhs.expr, member.as_str().to_owned()),
            range: selected_range,
            type_receiver: None,
            path_member,
            explicit_type_application,
        };
        if self.peek() != &Token::LBrace || self.control_body_brace_is_boundary {
            return Ok(selected);
        }
        let (closure, callback) = self.parse_callback_block_closure()?;
        let syntax = CallbackBlockCallSyntax::try_from_parser(selected.range, callback)
            .map_err(|error| Self::call_invariant_error(error, selected.range))?;
        let call = CallExpr::try_callback_block(selected.expr, closure, syntax)
            .map_err(|error| Self::call_invariant_error(error, selected.range))?;
        Ok(SpannedExpr {
            range: call.range(),
            expr: Expr::Call(call),
            type_receiver: None,
            path_member: None,
            explicit_type_application: None,
        })
    }

    fn parse_associated_select_postfix(
        &mut self,
        lhs: SpannedExpr,
    ) -> Result<SpannedExpr, ExprParseError> {
        let Some(receiver) = lhs.type_receiver else {
            return Err(ExprParseError::at(
                "syntax.expr.parse",
                "explicit associated member requires an authored generic type receiver",
                lhs.range,
            ));
        };
        let separator = self.bump_lexed();
        let separator = AssociatedMemberSeparatorSyntax::Path {
            range: self.absolute_range(&separator)?,
        };
        let member_token = self.bump_lexed();
        let member_range = self.absolute_range(&member_token)?;
        let Token::Ident(member) = member_token.token else {
            return Err(ExprParseError::at(
                "syntax.expr.parse",
                "expected associated member name after `::`",
                member_range,
            ));
        };
        let member = Name::new(member);
        let range = TextRange::new(lhs.range.start(), member_range.end());
        let path_member = PathMemberCalleeSyntax::try_from_parser(
            &self.source,
            self.base,
            receiver,
            separator,
            member.clone(),
            member_range,
            range,
        )
        .map_err(|error| Self::call_invariant_error(error, range))?;
        Ok(SpannedExpr {
            expr: Expr::select(lhs.expr, member.as_str().to_owned()),
            range,
            type_receiver: None,
            path_member: Some(path_member),
            explicit_type_application: None,
        })
    }

    fn parse_range_postfix(&mut self, lhs: SpannedExpr) -> Result<SpannedExpr, ExprParseError> {
        let inclusive = matches!(self.bump(), Token::Op(ExprOp::RangeInclusive));
        let parsed_end = if matches!(
            self.peek(),
            Token::Eof | Token::Comma | Token::RParen | Token::RBracket | Token::RBrace
        ) {
            None
        } else {
            Some(self.parse_expr_bp_spanned(5)?)
        };
        let range_end = parsed_end.as_ref().map_or_else(
            || self.absolute_offset(self.previous_lexed_end()),
            |parsed| Ok(parsed.range.end()),
        )?;
        Ok(SpannedExpr {
            expr: Expr::Range {
                start: Some(Box::new(lhs.expr)),
                end: parsed_end.map(|parsed| Box::new(parsed.expr)),
                inclusive,
            },
            range: TextRange::new(lhs.range.start(), range_end),
            type_receiver: None,
            path_member: None,
            explicit_type_application: None,
        })
    }

    fn current_infix(&self, min_bp: u8) -> Option<(ExprOp, u8, BinaryOp)> {
        let op = match self.peek() {
            Token::Amp => ExprOp::Merge,
            Token::Star => ExprOp::Mul,
            Token::Op(op) => *op,
            _ => return None,
        };
        let (left_bp, right_bp, binary) = infix_binding_power(op)?;
        (left_bp >= min_bp).then_some((op, right_bp, binary))
    }

    fn parse_infix(
        &mut self,
        lhs: SpannedExpr,
        op: ExprOp,
        right_bp: u8,
        binary: BinaryOp,
    ) -> Result<SpannedExpr, ExprParseError> {
        self.bump();
        let rhs = self.parse_expr_bp_spanned(right_bp)?;
        let range = TextRange::new(lhs.range.start(), rhs.range.end());
        let expr = if op == ExprOp::Pipe {
            Expr::Pipe {
                lhs: Box::new(lhs.expr),
                rhs: Box::new(rhs.expr),
            }
        } else {
            Expr::Binary {
                lhs: Box::new(lhs.expr),
                op: binary,
                rhs: Box::new(rhs.expr),
            }
        };
        Ok(SpannedExpr {
            expr,
            range,
            type_receiver: None,
            path_member: None,
            explicit_type_application: None,
        })
    }

    pub(super) fn parse_control_head_expr(&mut self) -> Result<Expr, ExprParseError> {
        let previous = self.control_body_brace_is_boundary;
        self.control_body_brace_is_boundary = true;
        let parsed = self.parse_expr_bp(0);
        self.control_body_brace_is_boundary = previous;
        parsed
    }

    pub(super) fn parse_tuple_or_group(&mut self) -> Result<Expr, ExprParseError> {
        if self.peek() == &Token::RParen {
            self.bump();
            return Ok(Expr::Tuple(Vec::new()));
        }
        let mut items = Vec::new();
        loop {
            items.push(self.parse_expr_bp(0)?);
            match self.peek() {
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RParen {
                        self.bump();
                        return Ok(Expr::Tuple(items));
                    }
                }
                Token::RParen => {
                    self.bump();
                    return if items.len() == 1 {
                        Ok(items.remove(0))
                    } else {
                        Ok(Expr::Tuple(items))
                    };
                }
                _ => return Err(ExprParseError::new("expected `)` or `,` in tuple")),
            }
        }
    }

    pub(super) fn parse_bracket_seq(&mut self) -> Result<Expr, ExprParseError> {
        let mut items = Vec::new();
        if self.peek() == &Token::RBracket {
            self.bump();
            return Ok(Expr::BracketSeq(items));
        }
        if let Some(expr) = self.parse_flat_literal_bracket_seq()? {
            return Ok(expr);
        }
        loop {
            items.push(self.parse_expr_bp(0)?);
            match self.peek() {
                Token::Semicolon => {
                    self.bump();
                    if items.len() != 1 {
                        return Err(ExprParseError::new(
                            "array repeat literal expects one value before `;`",
                        ));
                    }
                    let len = self.parse_expr_bp(0)?;
                    self.expect(&Token::RBracket)?;
                    let value = items.remove(0);
                    return Ok(Expr::ArrayRepeat {
                        value: Box::new(value),
                        len: Box::new(len),
                    });
                }
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RBracket {
                        self.bump();
                        return Ok(Expr::BracketSeq(items));
                    }
                }
                Token::RBracket => {
                    self.bump();
                    return Ok(Expr::BracketSeq(items));
                }
                _ => {
                    return Err(ExprParseError::new(
                        "expected `]` or `,` in bracket sequence literal",
                    ));
                }
            }
        }
    }

    pub(super) fn parse_flat_literal_bracket_seq(
        &mut self,
    ) -> Result<Option<Expr>, ExprParseError> {
        let start = self.cursor;
        let mut fallback_items = None;
        let mut int_literals = Vec::new();
        let mut int_literal_ranges = Vec::new();
        let mut int_suffix = None;
        let mut int_suffix_seen = false;
        let mut all_int = true;
        loop {
            let Token::Literal(literal) = self.peek() else {
                self.cursor = start;
                return Ok(None);
            };
            match literal {
                Literal::Int(literal) if all_int => {
                    if int_suffix_seen && int_suffix != literal.suffix() {
                        all_int = false;
                    } else if !int_suffix_seen {
                        int_suffix = literal.suffix();
                        int_suffix_seen = true;
                    }
                    int_literals.push(literal.clone());
                    int_literal_ranges.push(self.absolute_range(self.peek_lexed())?);
                }
                _ => all_int = false,
            }
            if !all_int {
                fallback_items
                    .get_or_insert_with(|| {
                        literal_exprs_from_tokens(&self.tokens[start..self.cursor])
                    })
                    .push(Expr::Literal(literal.clone()));
            }
            self.bump();
            match self.peek() {
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RBracket {
                        self.bump();
                        let range = self.consumed_range(start)?;
                        let expr = flat_literal_bracket_seq_expr(
                            all_int,
                            int_literals,
                            int_literal_ranges,
                            fallback_items,
                        )
                        .map_err(|error| {
                            ExprParseError::at(
                                "syntax.expr.numeric_bracket_sequence_invariant",
                                &error.to_string(),
                                range,
                            )
                        })?;
                        if matches!(expr, Expr::NumericBracketSeq(_)) {
                            self.stats.numeric_seq_summaries = self
                                .stats
                                .numeric_seq_summaries
                                .checked_add(1)
                                .ok_or_else(|| Self::offset_overflow_error(self.base))?;
                        }
                        return Ok(Some(expr));
                    }
                }
                Token::RBracket => {
                    self.bump();
                    let range = self.consumed_range(start)?;
                    let expr = flat_literal_bracket_seq_expr(
                        all_int,
                        int_literals,
                        int_literal_ranges,
                        fallback_items,
                    )
                    .map_err(|error| {
                        ExprParseError::at(
                            "syntax.expr.numeric_bracket_sequence_invariant",
                            &error.to_string(),
                            range,
                        )
                    })?;
                    if matches!(expr, Expr::NumericBracketSeq(_)) {
                        self.stats.numeric_seq_summaries = self
                            .stats
                            .numeric_seq_summaries
                            .checked_add(1)
                            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
                    }
                    return Ok(Some(expr));
                }
                _ => {
                    self.cursor = start;
                    return Ok(None);
                }
            }
        }
    }

    pub(super) fn parse_thread_expr(&mut self) -> Result<Expr, ExprParseError> {
        let mut modifiers = Vec::new();
        let mut name_parts = Vec::new();
        while self.peek() != &Token::LBrace {
            match self.bump() {
                Token::Ident(value) if value == "detached" && name_parts.is_empty() => {
                    modifiers.push(ThreadModifier::Detached);
                }
                Token::Ident(value) => name_parts.push(value),
                Token::Eof => return Err(ExprParseError::new("expected `{` in thread expression")),
                token => {
                    return Err(ExprParseError::new(&format!(
                        "expected thread name or `{{`, found {token:?}"
                    )));
                }
            }
        }
        self.expect(&Token::LBrace)?;
        let mut depth = 1usize;
        let mut body_tokens = Vec::new();
        while depth > 0 {
            match self.bump() {
                Token::LBrace => {
                    depth += 1;
                    body_tokens.push("{".to_owned());
                }
                Token::RBrace => {
                    depth -= 1;
                    if depth > 0 {
                        body_tokens.push("}".to_owned());
                    }
                }
                Token::Eof => return Err(ExprParseError::new("unclosed thread expression block")),
                token => body_tokens.push(token_source(&token)),
            }
        }
        let body_source = body_tokens.join(" ");
        let body = if body_source.trim().is_empty() {
            Vec::new()
        } else {
            vec![FlowItem::Stmt(Stmt::Expr {
                expr: parse_expr(body_source.trim())?,
                expr_source: None,
                expr_range: None,
            })]
        };
        Ok(Expr::Thread {
            block: Box::new(ThreadBlock::new(
                modifiers,
                nonempty_joined_name(&name_parts),
                body,
            )),
        })
    }

    fn retain_recovery_diagnostic(
        &mut self,
        diagnostic: ExprParseError,
    ) -> Result<(), ExprParseError> {
        let next_nodes = self
            .recovery_nodes
            .checked_add(1)
            .ok_or_else(|| Self::offset_overflow_error(diagnostic.range().start()))?;
        if next_nodes > MAX_EXPR_RECOVERY_NODES {
            return Err(ExprParseError::at(
                "syntax.expr.recovery_node_limit",
                "expression recovery exceeds the inclusive node limit of 256",
                diagnostic.range(),
            ));
        }
        let next_diagnostics = self
            .diagnostics
            .len()
            .checked_add(1)
            .ok_or_else(|| Self::offset_overflow_error(diagnostic.range().start()))?;
        if next_diagnostics > MAX_EXPR_DIAGNOSTICS {
            return Err(ExprParseError::at(
                "syntax.expr.diagnostic_limit",
                "expression diagnostics exceed the inclusive limit of 128",
                diagnostic.range(),
            ));
        }
        self.recovery_nodes = next_nodes;
        self.diagnostics.push(diagnostic);
        Ok(())
    }

    pub(super) fn retain_nested_parsed_expr(
        &mut self,
        parsed: ParsedExpr,
    ) -> Result<Expr, ExprParseError> {
        let ParsedExpr {
            expr,
            diagnostics,
            stats,
            ..
        } = parsed;
        self.stats = self
            .stats
            .checked_add(stats)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        for diagnostic in diagnostics {
            self.retain_recovery_diagnostic(diagnostic)?;
        }
        Ok(expr)
    }

    fn token_index_range(&self, start: usize, end: usize) -> Result<TextRange, ExprParseError> {
        if start >= end {
            let insertion = self
                .tokens
                .get(start)
                .map_or(self.source.len(), |token| token.start);
            let insertion = self.absolute_offset(insertion)?;
            return Ok(TextRange::new(insertion, insertion));
        }
        let first = self
            .tokens
            .get(start)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        let last_index = end
            .checked_sub(1)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        let last = self
            .tokens
            .get(last_index)
            .ok_or_else(|| Self::offset_overflow_error(self.base))?;
        Ok(TextRange::new(
            self.absolute_offset(first.start)?,
            self.absolute_offset(last.end)?,
        ))
    }

    fn source_for_token_range(&self, start: usize, end: usize) -> Option<&str> {
        if start >= end {
            return None;
        }
        let first = self.tokens.get(start)?;
        let last = self.tokens.get(end.checked_sub(1)?)?;
        self.source.get(first.start..last.end)
    }

    pub(super) fn parse_record_fields(&mut self) -> Result<Vec<(String, Expr)>, ExprParseError> {
        let mut fields = Vec::new();
        if self.peek() == &Token::RBrace {
            self.bump();
            return Ok(fields);
        }
        loop {
            let name = self.take_ident("expected record field name")?;
            let value = if matches!(self.peek(), Token::Colon | Token::Op(ExprOp::Assign)) {
                self.bump();
                self.parse_expr_bp(0)?
            } else {
                Expr::Path(DottedPath::single(name.clone()))
            };
            fields.push((name, value));
            match self.peek() {
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RBrace {
                        self.bump();
                        return Ok(fields);
                    }
                }
                Token::RBrace => {
                    self.bump();
                    return Ok(fields);
                }
                _ => return Err(ExprParseError::new("expected `}` or `,` in record literal")),
            }
        }
    }

    pub(super) fn take_ident(&mut self, message: &str) -> Result<String, ExprParseError> {
        match self.bump() {
            Token::Ident(name) | Token::RelativePath(name) => Ok(name),
            _ => Err(ExprParseError::new(message)),
        }
    }

    pub(super) fn expect(&mut self, expected: &Token) -> Result<(), ExprParseError> {
        let found = self.bump();
        if &found == expected {
            Ok(())
        } else {
            Err(ExprParseError::new(&format!(
                "expected {expected:?}, found {found:?}"
            )))
        }
    }

    pub(super) fn peek(&self) -> &Token {
        self.token_at(self.cursor)
    }

    pub(super) fn peek_lexed(&self) -> &LexedToken {
        self.tokens.get(self.cursor).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer always appends an EOF token")
        })
    }

    pub(super) fn token_at(&self, index: usize) -> &Token {
        self.tokens
            .get(index)
            .map_or(&Token::Eof, |lexed| &lexed.token)
    }

    pub(super) fn peek_ident(&self, expected: &str) -> bool {
        matches!(self.peek(), Token::Ident(value) if value == expected)
    }

    pub(super) fn previous(&self) -> Option<&Token> {
        self.cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|lexed| &lexed.token)
    }

    pub(super) fn bump(&mut self) -> Token {
        let token = self.peek().clone();
        if !matches!(token, Token::Eof) {
            self.cursor += 1;
        }
        token
    }

    pub(super) fn bump_lexed(&mut self) -> LexedToken {
        let lexed = self.tokens.get(self.cursor).cloned().unwrap_or_else(|| {
            let end = self.source.len();
            LexedToken {
                token: Token::Eof,
                start: end,
                end,
            }
        });
        if !matches!(lexed.token, Token::Eof) {
            self.cursor += 1;
        }
        lexed
    }

    pub(super) fn previous_lexed_end(&self) -> usize {
        self.cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or(0, |token| token.end)
    }

    pub(super) fn consumed_range(&self, start: usize) -> Result<TextRange, ExprParseError> {
        Ok(TextRange::new(
            self.absolute_offset(start)?,
            self.absolute_offset(self.previous_lexed_end())?,
        ))
    }

    pub(super) fn absolute_range(&self, token: &LexedToken) -> Result<TextRange, ExprParseError> {
        Ok(TextRange::new(
            self.absolute_offset(token.start)?,
            self.absolute_offset(token.end)?,
        ))
    }

    pub(super) fn absolute_offset(&self, offset: usize) -> Result<usize, ExprParseError> {
        self.base
            .checked_add(offset)
            .ok_or_else(|| Self::offset_overflow_error(self.base))
    }

    fn offset_overflow_error(at: usize) -> ExprParseError {
        ExprParseError::at(
            "syntax.expr.offset_overflow",
            "expression source offset overflowed",
            TextRange::new(at, at),
        )
    }

    fn call_invariant_error(error: CallSyntaxInvariantError, range: TextRange) -> ExprParseError {
        ExprParseError::at(
            "syntax.expr.call_invariant",
            &format!("invalid parser-owned call syntax: {error}"),
            range,
        )
    }
}

fn infix_binding_power(op: ExprOp) -> Option<(u8, u8, BinaryOp)> {
    Some(match op {
        ExprOp::FatArrow => (10, 10, BinaryOp::Implies),
        ExprOp::Pipe => (15, 16, BinaryOp::Implies),
        ExprOp::Or => (20, 21, BinaryOp::Or),
        ExprOp::And => (30, 31, BinaryOp::And),
        ExprOp::In => (40, 5, BinaryOp::In),
        ExprOp::Eq => (45, 46, BinaryOp::Eq),
        ExprOp::NotEq => (45, 46, BinaryOp::NotEq),
        ExprOp::Gte => (45, 46, BinaryOp::Gte),
        ExprOp::Lte => (45, 46, BinaryOp::Lte),
        ExprOp::Gt => (45, 46, BinaryOp::Gt),
        ExprOp::Lt => (45, 46, BinaryOp::Lt),
        ExprOp::Merge => (48, 49, BinaryOp::Merge),
        ExprOp::Add => (50, 51, BinaryOp::Add),
        ExprOp::NegOrSub => (50, 51, BinaryOp::Sub),
        ExprOp::Mul => (60, 61, BinaryOp::Mul),
        ExprOp::Div => (60, 61, BinaryOp::Div),
        ExprOp::Rem => (60, 61, BinaryOp::Rem),
        _ => return None,
    })
}
