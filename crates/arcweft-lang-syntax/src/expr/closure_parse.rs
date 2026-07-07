use super::{ClosureParam, Expr, ExprOp, ExprParseError, ExprParser, LexedToken, Token};
use crate::cst::{split_top_level_punctuation, split_top_level_punctuation_once};
use crate::pattern::parse_pattern;
use crate::types::{TypeRef, parse_type_ref};

#[derive(Default)]
struct ClosureReturnParse {
    return_type: Option<TypeRef>,
    block_body: Option<String>,
}

impl ExprParser {
    pub(super) fn parse_closure_arg(&mut self) -> Result<Expr, ExprParseError> {
        self.expect(&Token::Op(ExprOp::ClosurePipe))?;
        self.parse_closure_after_open_pipe()
    }

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
        let params_source = token_span_source(&param_tokens, &self.source).unwrap_or_default();
        let params = parse_closure_params(params_source)?;
        let closure_return = self.parse_closure_return_type()?;
        let body = self.parse_closure_body(closure_return.block_body)?;
        Ok(Expr::Closure {
            params,
            return_type: closure_return.return_type,
            body: Box::new(body),
        })
    }

    pub(super) fn parse_callback_block_closure(&mut self) -> Result<Expr, ExprParseError> {
        let tokens = self.take_braced_tokens()?;
        let (params, body_tokens) = callback_block_parts(&tokens, &self.source)?;
        let body_source = token_span_source(body_tokens, &self.source)
            .unwrap_or_default()
            .trim();
        if body_source.trim().is_empty() {
            return Err(ExprParseError::new(
                "callback block requires a body expression",
            ));
        }
        Ok(Expr::Closure {
            params,
            return_type: None,
            body: Box::new(crate::parser::parse_callback_block_expr_body(body_source)),
        })
    }

    fn parse_closure_return_type(&mut self) -> Result<ClosureReturnParse, ExprParseError> {
        if self.peek() != &Token::Op(ExprOp::ThinArrow) {
            return Ok(ClosureReturnParse::default());
        }
        self.bump();
        let type_tokens = self.take_closure_return_type_tokens()?;
        let type_source = token_span_source(&type_tokens, &self.source).unwrap_or_default();
        let return_type =
            parse_type_ref(type_source).map_err(|error| ExprParseError::new(&error.to_string()))?;
        if self.peek() != &Token::LBrace {
            return Err(ExprParseError::new(
                "closure return type annotation requires a block body",
            ));
        }
        let body_tokens = self.take_braced_tokens()?;
        let body_source = token_span_source(&body_tokens, &self.source).unwrap_or_default();
        Ok(ClosureReturnParse {
            return_type: Some(return_type),
            block_body: Some(body_source.trim().to_owned()),
        })
    }

    fn parse_closure_body(&mut self, block_body: Option<String>) -> Result<Expr, ExprParseError> {
        block_body.map_or_else(
            || self.parse_expr_bp(0),
            |body| Ok(crate::parser::parse_callback_block_expr_body(&body)),
        )
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
                Token::LParen => paren_depth = paren_depth.saturating_add(1),
                Token::RParen => paren_depth = paren_depth.saturating_sub(1),
                Token::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                Token::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                Token::LBrace => brace_depth = brace_depth.saturating_add(1),
                Token::RBrace => brace_depth = brace_depth.saturating_sub(1),
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
                Token::LParen => paren_depth = paren_depth.saturating_add(1),
                Token::RParen => paren_depth = paren_depth.saturating_sub(1),
                Token::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                Token::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
            tokens.push(self.bump_lexed());
        }
    }

    fn take_braced_tokens(&mut self) -> Result<Vec<LexedToken>, ExprParseError> {
        if self.peek() != &Token::LBrace {
            return Err(ExprParseError::new(&format!(
                "expected {:?}, found {:?}",
                Token::LBrace,
                self.peek()
            )));
        }
        self.cursor += 1;
        let mut depth = 1_u32;
        let mut tokens = Vec::new();
        loop {
            let lexed = self.bump_lexed();
            match lexed.token {
                Token::LBrace => {
                    depth = depth.saturating_add(1);
                    tokens.push(lexed);
                }
                Token::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Ok(tokens);
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
) -> Result<(Vec<ClosureParam>, &'a [LexedToken]), ExprParseError> {
    let Some(arrow) = top_level_callback_arrow(tokens) else {
        return Ok((Vec::new(), tokens));
    };
    let params = callback_block_params(&tokens[..arrow], source)?;
    Ok((params, &tokens[arrow + 1..]))
}

fn top_level_callback_arrow(tokens: &[LexedToken]) -> Option<usize> {
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut brace_depth = 0_u32;
    tokens.iter().enumerate().find_map(|(index, lexed)| {
        match &lexed.token {
            Token::LParen => paren_depth = paren_depth.saturating_add(1),
            Token::RParen => paren_depth = paren_depth.saturating_sub(1),
            Token::LBracket => bracket_depth = bracket_depth.saturating_add(1),
            Token::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            Token::LBrace => brace_depth = brace_depth.saturating_add(1),
            Token::RBrace => brace_depth = brace_depth.saturating_sub(1),
            Token::Op(ExprOp::FatArrow)
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
            {
                return Some(index);
            }
            _ => {}
        }
        None
    })
}

fn callback_block_params(
    tokens: &[LexedToken],
    source: &str,
) -> Result<Vec<ClosureParam>, ExprParseError> {
    let params_source = token_span_source(tokens, source).unwrap_or_default().trim();
    if params_source.is_empty() {
        return Err(ExprParseError::new(
            "callback block parameter list must appear before `=>`",
        ));
    }
    parse_closure_params(params_source)
}

pub(super) fn parse_closure_params(source: &str) -> Result<Vec<ClosureParam>, ExprParseError> {
    let source = source.trim();
    if source.is_empty() {
        return Ok(Vec::new());
    }
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(parse_closure_param)
        .collect()
}

fn parse_closure_param(source: &str) -> Result<ClosureParam, ExprParseError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(ExprParseError::new("expected closure parameter"));
    }
    let (pattern, ty) = split_top_level_punctuation_once(source, ':')
        .map_or((source, None), |(pattern, ty)| {
            (pattern.trim(), Some(ty.trim()))
        });
    let ty = ty
        .filter(|ty| !ty.is_empty())
        .map(parse_type_ref)
        .transpose()
        .map_err(|error| {
            ExprParseError::new(&format!("invalid closure parameter type: {error}"))
        })?;
    Ok(ClosureParam::new(parse_pattern(pattern), ty))
}

fn token_span_source<'a>(tokens: &[LexedToken], source: &'a str) -> Option<&'a str> {
    let first = tokens.first()?;
    let last = tokens.last()?;
    source.get(first.start..last.end)
}
