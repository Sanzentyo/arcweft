use super::call_syntax::{
    CallbackBlockSyntaxInit, CallbackParameterHeaderSyntaxInit, CallbackParameterSyntaxInit,
};
use super::{
    CallbackBlockSyntax, CallbackParameterTypeSyntax, ClosureParam, Expr, ExprOp, ExprParseError,
    ExprParser, LexedToken, MAX_CALLBACK_PARAMETERS, Token,
};
use crate::ast::common::TextRange;
use crate::cst::{split_top_level_punctuation, split_top_level_punctuation_once};
use crate::pattern::parse_pattern_at;
use crate::types::{AuthoredTypeRef, parse_type_ref};

#[derive(Default)]
struct ClosureReturnParse {
    return_type: Option<AuthoredTypeRef>,
    block_body: Option<ClosureBlockBody>,
}

struct ClosureBlockBody {
    source: String,
    base: usize,
}

struct BracedTokens {
    open: LexedToken,
    inner: Vec<LexedToken>,
    close: LexedToken,
}

struct ParsedCallbackParts<'a> {
    params: Vec<ClosureParam>,
    header: CallbackParameterHeaderSyntaxInit,
    body_tokens: &'a [LexedToken],
}

impl ExprParser {
    pub(super) fn parse_zero_arg_closure(&mut self) -> Result<Expr, ExprParseError> {
        let closure_return = self.parse_closure_return_type()?;
        let body = self.parse_closure_body(closure_return.block_body)?;
        Ok(Expr::Closure {
            params: Vec::new(),
            return_type: closure_return.return_type,
            body: Box::new(body),
        })
    }

    pub(super) fn parse_closure_after_open_pipe(&mut self) -> Result<Expr, ExprParseError> {
        let param_tokens = self.take_closure_param_tokens()?;
        let params_source = if param_tokens.is_empty() {
            ""
        } else {
            required_token_span_source(&param_tokens, &self.source, self.base, "closure parameter")?
        };
        let params_base = token_absolute_range(
            param_tokens
                .first()
                .ok_or_else(|| ExprParseError::new("expected closure parameter"))?,
            param_tokens
                .last()
                .ok_or_else(|| ExprParseError::new("expected closure parameter"))?,
            self.base,
        )?
        .start();
        let params = parse_closure_params(params_source, params_base)?;
        let closure_return = self.parse_closure_return_type()?;
        let body = self.parse_closure_body(closure_return.block_body)?;
        Ok(Expr::Closure {
            params,
            return_type: closure_return.return_type,
            body: Box::new(body),
        })
    }

    pub(super) fn parse_callback_block_closure(
        &mut self,
    ) -> Result<(Expr, CallbackBlockSyntax), ExprParseError> {
        let tokens = self.take_braced_tokens()?;
        let parts = callback_block_parts(&tokens.inner, &self.source, self.base)?;
        if parts.body_tokens.is_empty() {
            return Err(ExprParseError::new(
                "callback block requires a body expression",
            ));
        }
        let body_source = required_token_span_source(
            parts.body_tokens,
            &self.source,
            self.base,
            "callback body",
        )?;
        if body_source.trim().is_empty() {
            return Err(ExprParseError::new(
                "callback block requires a body expression",
            ));
        }
        let body_range =
            token_absolute_range(
                parts.body_tokens.first().ok_or_else(|| {
                    ExprParseError::new("callback block requires a body expression")
                })?,
                parts.body_tokens.last().ok_or_else(|| {
                    ExprParseError::new("callback block requires a body expression")
                })?,
                self.base,
            )?;
        let open_brace = self.absolute_range(&tokens.open)?;
        let close_brace = self.absolute_range(&tokens.close)?;
        let callback_range = TextRange::new(open_brace.start(), close_brace.end());
        let callback = CallbackBlockSyntax::try_from_parser(
            &self.validation_source,
            self.validation_base,
            CallbackBlockSyntaxInit {
                open_brace,
                parameters: parts.header,
                body: body_range,
                close_brace,
            },
        )
        .map_err(|error| {
            ExprParseError::at(
                "syntax.expr.call_invariant",
                &format!("invalid parser-owned callback syntax: {error}"),
                callback_range,
            )
        })?;
        let parsed_body = crate::parser::parse_callback_block_expr_body_recovering_at(
            body_source,
            body_range.start(),
        )?;
        let body = self.retain_nested_parsed_expr(parsed_body)?;
        Ok((
            Expr::Closure {
                params: parts.params,
                return_type: None,
                body: Box::new(body),
            },
            callback,
        ))
    }

    fn parse_closure_return_type(&mut self) -> Result<ClosureReturnParse, ExprParseError> {
        if self.peek() != &Token::Op(ExprOp::ThinArrow) {
            return Ok(ClosureReturnParse::default());
        }
        self.bump();
        let type_tokens = self.take_closure_return_type_tokens()?;
        let type_source = required_token_span_source(
            &type_tokens,
            &self.source,
            self.base,
            "closure return type",
        )?;
        let mut return_type =
            parse_type_ref(type_source).map_err(|error| ExprParseError::new(&error.to_string()))?;
        let type_range = token_absolute_range(
            type_tokens
                .first()
                .ok_or_else(|| ExprParseError::new("expected closure return type"))?,
            type_tokens
                .last()
                .ok_or_else(|| ExprParseError::new("expected closure return type"))?,
            self.base,
        )?;
        return_type.rebase(type_range.start());
        if self.peek() != &Token::LBrace {
            return Err(ExprParseError::new(
                "closure return type annotation requires a block body",
            ));
        }
        let body_tokens = self.take_braced_tokens()?;
        let closure_range = token_absolute_range(&body_tokens.open, &body_tokens.close, self.base)?;
        let body_source = self
            .source
            .get(body_tokens.open.end..body_tokens.close.start)
            .ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.invalid_token_span",
                    "parser-owned closure body range is outside the expression source",
                    closure_range,
                )
            })?;
        let body_base = self.base.checked_add(body_tokens.open.end).ok_or_else(|| {
            ExprParseError::at(
                "syntax.expr.offset_overflow",
                "closure body source offset overflowed",
                TextRange::new(self.base, self.base),
            )
        })?;
        Ok(ClosureReturnParse {
            return_type: Some(return_type),
            block_body: Some(ClosureBlockBody {
                source: body_source.to_owned(),
                base: body_base,
            }),
        })
    }

    fn parse_closure_body(
        &mut self,
        block_body: Option<ClosureBlockBody>,
    ) -> Result<Expr, ExprParseError> {
        let Some(block_body) = block_body else {
            return self.parse_expr_bp(0);
        };
        let parsed = crate::parser::parse_callback_block_expr_body_recovering_at(
            &block_body.source,
            block_body.base,
        )?;
        self.retain_nested_parsed_expr(parsed)
    }

    fn take_closure_param_tokens(&mut self) -> Result<Vec<LexedToken>, ExprParseError> {
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        let mut tokens = Vec::new();
        loop {
            let lexed = self.bump_lexed();
            match &lexed.token {
                Token::Op(ExprOp::ClosurePipe)
                    if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
                {
                    return Ok(tokens);
                }
                Token::LParen => {
                    paren_depth = paren_depth
                        .checked_add(1)
                        .ok_or_else(|| ExprParseError::new("closure nesting depth overflowed"))?;
                }
                Token::RParen => {
                    paren_depth = paren_depth
                        .checked_sub(1)
                        .ok_or_else(|| ExprParseError::new("unbalanced closure parameter `)`"))?;
                }
                Token::LBracket => {
                    bracket_depth = bracket_depth
                        .checked_add(1)
                        .ok_or_else(|| ExprParseError::new("closure nesting depth overflowed"))?;
                }
                Token::RBracket => {
                    bracket_depth = bracket_depth
                        .checked_sub(1)
                        .ok_or_else(|| ExprParseError::new("unbalanced closure parameter `]`"))?;
                }
                Token::LBrace => {
                    brace_depth = brace_depth
                        .checked_add(1)
                        .ok_or_else(|| ExprParseError::new("closure nesting depth overflowed"))?;
                }
                Token::RBrace => {
                    brace_depth = brace_depth
                        .checked_sub(1)
                        .ok_or_else(|| ExprParseError::new("unbalanced closure parameter `}`"))?;
                }
                Token::Eof => return Err(ExprParseError::new("unclosed closure parameter list")),
                _ => {}
            }
            tokens.push(lexed);
        }
    }

    fn take_closure_return_type_tokens(&mut self) -> Result<Vec<LexedToken>, ExprParseError> {
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut tokens = Vec::new();
        loop {
            match self.peek() {
                Token::LBrace if paren_depth == 0 && bracket_depth == 0 => {
                    if tokens.is_empty() {
                        return Err(ExprParseError::new(
                            "expected closure return type after `->`",
                        ));
                    }
                    return Ok(tokens);
                }
                Token::Eof => {
                    return Err(ExprParseError::new(
                        "closure return type annotation requires a block body",
                    ));
                }
                Token::RParen if paren_depth == 0 => {
                    return Err(ExprParseError::new(
                        "closure return type annotation requires a block body",
                    ));
                }
                Token::RBracket if bracket_depth == 0 => {
                    return Err(ExprParseError::new(
                        "closure return type annotation requires a block body",
                    ));
                }
                Token::LParen => {
                    paren_depth = paren_depth
                        .checked_add(1)
                        .ok_or_else(|| ExprParseError::new("closure nesting depth overflowed"))?;
                }
                Token::RParen => {
                    paren_depth = paren_depth
                        .checked_sub(1)
                        .ok_or_else(|| ExprParseError::new("unbalanced closure return type `)`"))?;
                }
                Token::LBracket => {
                    bracket_depth = bracket_depth
                        .checked_add(1)
                        .ok_or_else(|| ExprParseError::new("closure nesting depth overflowed"))?;
                }
                Token::RBracket => {
                    bracket_depth = bracket_depth
                        .checked_sub(1)
                        .ok_or_else(|| ExprParseError::new("unbalanced closure return type `]`"))?;
                }
                _ => {}
            }
            tokens.push(self.bump_lexed());
        }
    }

    fn take_braced_tokens(&mut self) -> Result<BracedTokens, ExprParseError> {
        if self.peek() != &Token::LBrace {
            return Err(ExprParseError::new(&format!(
                "expected {:?}, found {:?}",
                Token::LBrace,
                self.peek()
            )));
        }
        let open = self.bump_lexed();
        let mut depth = 1_u32;
        let mut tokens = Vec::new();
        loop {
            let lexed = self.bump_lexed();
            match lexed.token {
                Token::LBrace => {
                    depth = depth
                        .checked_add(1)
                        .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
                    tokens.push(lexed);
                }
                Token::RBrace => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or_else(|| ExprParseError::new("unbalanced callback `}`"))?;
                    if depth == 0 {
                        return Ok(BracedTokens {
                            open,
                            inner: tokens,
                            close: lexed,
                        });
                    }
                    tokens.push(lexed);
                }
                Token::Eof => return Err(ExprParseError::new("unclosed callback block")),
                _ => tokens.push(lexed),
            }
        }
    }
}

fn callback_block_parts<'a>(
    tokens: &'a [LexedToken],
    source: &str,
    base: usize,
) -> Result<ParsedCallbackParts<'a>, ExprParseError> {
    let Some(arrow) = top_level_callback_arrow(tokens)? else {
        return Ok(ParsedCallbackParts {
            params: Vec::new(),
            header: CallbackParameterHeaderSyntaxInit::ImplicitZero,
            body_tokens: tokens,
        });
    };
    let params = callback_block_params(&tokens[..arrow], source, base)?;
    let (parameters, separators) = callback_parameter_syntax(&tokens[..arrow], source, base)?;
    let fat_arrow = token_absolute_range(&tokens[arrow], &tokens[arrow], base)?;
    Ok(ParsedCallbackParts {
        params,
        header: CallbackParameterHeaderSyntaxInit::Explicit {
            parameters,
            separators,
            fat_arrow,
        },
        body_tokens: &tokens[arrow + 1..],
    })
}

fn top_level_callback_arrow(tokens: &[LexedToken]) -> Result<Option<usize>, ExprParseError> {
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut brace_depth = 0_u32;
    for (index, lexed) in tokens.iter().enumerate() {
        match &lexed.token {
            Token::LParen => {
                paren_depth = paren_depth
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RParen => {
                paren_depth = paren_depth
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `)`"))?;
            }
            Token::LBracket => {
                bracket_depth = bracket_depth
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RBracket => {
                bracket_depth = bracket_depth
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `]`"))?;
            }
            Token::LBrace => {
                brace_depth = brace_depth
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RBrace => {
                brace_depth = brace_depth
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `}`"))?;
            }
            Token::Op(ExprOp::FatArrow)
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
            {
                return Ok(Some(index));
            }
            _ => {}
        }
    }
    Ok(None)
}

fn callback_parameter_syntax(
    tokens: &[LexedToken],
    source: &str,
    base: usize,
) -> Result<(Vec<CallbackParameterSyntaxInit>, Vec<TextRange>), ExprParseError> {
    if tokens.is_empty() {
        return Err(ExprParseError::new(
            "callback block parameter list must appear before `=>`",
        ));
    }
    let mut parameters = Vec::new();
    let mut separators = Vec::new();
    let mut start = 0_usize;
    let mut paren = 0_u32;
    let mut bracket = 0_u32;
    let mut brace = 0_u32;
    for (index, token) in tokens.iter().enumerate() {
        match &token.token {
            Token::LParen => {
                paren = paren
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RParen => {
                paren = paren
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `)`"))?;
            }
            Token::LBracket => {
                bracket = bracket
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RBracket => {
                bracket = bracket
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `]`"))?;
            }
            Token::LBrace => {
                brace = brace
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RBrace => {
                brace = brace
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `}`"))?;
            }
            Token::Comma if paren == 0 && bracket == 0 && brace == 0 => {
                if parameters.len() >= MAX_CALLBACK_PARAMETERS {
                    return Err(ExprParseError::at(
                        "syntax.expr.callback_parameter_limit",
                        "callback parameter count exceeds the inclusive limit of 128",
                        token_absolute_range(token, token, base)?,
                    ));
                }
                parameters.push(callback_parameter_syntax_entry(
                    &tokens[start..index],
                    source,
                    base,
                )?);
                separators.push(token_absolute_range(token, token, base)?);
                start = index
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback parameter index overflowed"))?;
            }
            _ => {}
        }
    }
    if parameters.len() >= MAX_CALLBACK_PARAMETERS {
        let last = tokens
            .last()
            .ok_or_else(|| ExprParseError::new("expected callback parameter"))?;
        return Err(ExprParseError::at(
            "syntax.expr.callback_parameter_limit",
            "callback parameter count exceeds the inclusive limit of 128",
            token_absolute_range(last, last, base)?,
        ));
    }
    parameters.push(callback_parameter_syntax_entry(
        &tokens[start..],
        source,
        base,
    )?);
    Ok((parameters, separators))
}

fn callback_parameter_syntax_entry(
    tokens: &[LexedToken],
    source: &str,
    base: usize,
) -> Result<CallbackParameterSyntaxInit, ExprParseError> {
    let first = tokens
        .first()
        .ok_or_else(|| ExprParseError::new("expected callback parameter"))?;
    let last = tokens
        .last()
        .ok_or_else(|| ExprParseError::new("expected callback parameter"))?;
    let range = token_absolute_range(first, last, base)?;
    let colon = top_level_parameter_colon(tokens)?;
    let (pattern, type_ascription) = if let Some(colon_index) = colon {
        let pattern_last = colon_index
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .ok_or_else(|| ExprParseError::new("expected pattern before callback `:`"))?;
        let ty_index = colon_index
            .checked_add(1)
            .ok_or_else(|| ExprParseError::new("callback parameter index overflowed"))?;
        let ty_first = tokens
            .get(ty_index)
            .ok_or_else(|| ExprParseError::new("expected type after callback `:`"))?;
        let colon_range = token_absolute_range(&tokens[colon_index], &tokens[colon_index], base)?;
        (
            token_absolute_range(first, pattern_last, base)?,
            Some(CallbackParameterTypeSyntax::new(
                colon_range,
                token_absolute_range(ty_first, last, base)?,
            )),
        )
    } else {
        (range, None)
    };
    let parameter_source = source
        .get(first.start..last.end)
        .ok_or_else(|| ExprParseError::new("callback parameter range is outside source"))?;
    parse_closure_param(parameter_source, range.start())?;
    Ok(CallbackParameterSyntaxInit {
        range,
        pattern,
        type_ascription,
    })
}

fn top_level_parameter_colon(tokens: &[LexedToken]) -> Result<Option<usize>, ExprParseError> {
    let mut paren = 0_u32;
    let mut bracket = 0_u32;
    let mut brace = 0_u32;
    for (index, token) in tokens.iter().enumerate() {
        match &token.token {
            Token::LParen => {
                paren = paren
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RParen => {
                paren = paren
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `)`"))?;
            }
            Token::LBracket => {
                bracket = bracket
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RBracket => {
                bracket = bracket
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `]`"))?;
            }
            Token::LBrace => {
                brace = brace
                    .checked_add(1)
                    .ok_or_else(|| ExprParseError::new("callback nesting depth overflowed"))?;
            }
            Token::RBrace => {
                brace = brace
                    .checked_sub(1)
                    .ok_or_else(|| ExprParseError::new("unbalanced callback parameter `}`"))?;
            }
            Token::Colon if paren == 0 && bracket == 0 && brace == 0 => {
                return Ok(Some(index));
            }
            _ => {}
        }
    }
    Ok(None)
}

fn callback_block_params(
    tokens: &[LexedToken],
    source: &str,
    base: usize,
) -> Result<Vec<ClosureParam>, ExprParseError> {
    if tokens.is_empty() {
        return Err(ExprParseError::new(
            "callback block parameter list must appear before `=>`",
        ));
    }
    let params_source =
        required_token_span_source(tokens, source, base, "callback parameter list")?.trim();
    if params_source.is_empty() {
        return Err(ExprParseError::new(
            "callback block parameter list must appear before `=>`",
        ));
    }
    let params_range = token_absolute_range(
        tokens
            .first()
            .ok_or_else(|| ExprParseError::new("expected callback parameter"))?,
        tokens
            .last()
            .ok_or_else(|| ExprParseError::new("expected callback parameter"))?,
        base,
    )?;
    parse_closure_params(params_source, params_range.start())
}

pub(super) fn parse_closure_params(
    source: &str,
    base: usize,
) -> Result<Vec<ClosureParam>, ExprParseError> {
    let trimmed = source.trim();
    let base = base + subslice_offset(source, trimmed);
    let source = trimmed;
    if source.is_empty() {
        return Ok(Vec::new());
    }
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(|param| parse_closure_param(param, base + subslice_offset(source, param)))
        .collect()
}

fn parse_closure_param(source: &str, base: usize) -> Result<ClosureParam, ExprParseError> {
    let trimmed = source.trim();
    let base = base + subslice_offset(source, trimmed);
    let source = trimmed;
    if source.is_empty() {
        return Err(ExprParseError::new("expected closure parameter"));
    }
    let (pattern, ty) = split_top_level_punctuation_once(source, ':')
        .map_or((source, None), |(pattern, ty)| {
            (pattern.trim(), Some(ty.trim()))
        });
    let ty = ty
        .filter(|ty| !ty.is_empty())
        .map(|type_source| {
            let mut parsed = parse_type_ref(type_source)?;
            parsed.rebase(base + subslice_offset(source, type_source));
            Ok::<AuthoredTypeRef, crate::types::TypeParseError>(parsed)
        })
        .transpose()
        .map_err(|error| {
            ExprParseError::new(&format!("invalid closure parameter type: {error}"))
        })?;
    let pattern_source = pattern;
    let pattern = parse_pattern_at(
        pattern_source,
        base + subslice_offset(source, pattern_source),
    );
    Ok(ClosureParam::new(pattern, ty))
}

fn subslice_offset(source: &str, fragment: &str) -> usize {
    (fragment.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}

fn required_token_span_source<'a>(
    tokens: &[LexedToken],
    source: &'a str,
    base: usize,
    role: &str,
) -> Result<&'a str, ExprParseError> {
    let first = tokens
        .first()
        .ok_or_else(|| ExprParseError::new(&format!("expected {role}")))?;
    let last = tokens
        .last()
        .ok_or_else(|| ExprParseError::new(&format!("expected {role}")))?;
    let range = token_absolute_range(first, last, base)?;
    source.get(first.start..last.end).ok_or_else(|| {
        ExprParseError::at(
            "syntax.expr.invalid_token_span",
            &format!("parser-owned {role} range is outside the expression source"),
            range,
        )
    })
}

fn token_absolute_range(
    first: &LexedToken,
    last: &LexedToken,
    base: usize,
) -> Result<TextRange, ExprParseError> {
    let start = base.checked_add(first.start).ok_or_else(|| {
        ExprParseError::at(
            "syntax.expr.offset_overflow",
            "callback source range overflowed",
            TextRange::new(base, base),
        )
    })?;
    let end = base.checked_add(last.end).ok_or_else(|| {
        ExprParseError::at(
            "syntax.expr.offset_overflow",
            "callback source range overflowed",
            TextRange::new(base, base),
        )
    })?;
    Ok(TextRange::new(start, end))
}
