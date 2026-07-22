use super::ExprParseError;
use crate::cst::{
    ArcweftPunctuation, find_matching_punctuation, find_top_level_punctuation,
    strip_prefix_arcweft_punctuation,
};

pub(super) struct ClosureSource<'a> {
    pub(super) params: &'a str,
    pub(super) header: &'a str,
    pub(super) return_type: Option<&'a str>,
    pub(super) body: ClosureBodySource<'a>,
}

pub(super) enum ClosureBodySource<'a> {
    Expr(&'a str),
    Block { inner: &'a str, whole: &'a str },
}

pub(super) fn split(source: &str) -> Result<Option<ClosureSource<'_>>, ExprParseError> {
    let Some(rest) = source.strip_prefix('|') else {
        return Ok(None);
    };
    let Some(close) = find_top_level_punctuation(rest, '|') else {
        return Ok(None);
    };
    let params = rest[..close].trim();
    let body = rest[close + 1..].trim();
    if body.is_empty() {
        return Ok(None);
    }
    let Some(after_arrow) = strip_prefix_arcweft_punctuation(body, ArcweftPunctuation::ThinArrow)
    else {
        return Ok(Some(ClosureSource {
            params,
            header: &source[..'|'.len_utf8() + close + '|'.len_utf8()],
            return_type: None,
            body: ClosureBodySource::Expr(body),
        }));
    };
    let after_arrow = after_arrow.trim_start();
    let open = find_top_level_punctuation(after_arrow, '{').ok_or_else(|| {
        ExprParseError::new("closure return type annotation requires a block body")
    })?;
    let return_type = after_arrow[..open].trim();
    if return_type.is_empty() {
        return Err(ExprParseError::new(
            "expected closure return type after `->`",
        ));
    }
    let close = find_matching_punctuation(after_arrow, open, '{', '}')
        .ok_or_else(|| ExprParseError::new("unclosed closure block body"))?;
    if !after_arrow[close + '}'.len_utf8()..].trim().is_empty() {
        return Err(ExprParseError::new(
            "unexpected tokens after closure block body",
        ));
    }
    Ok(Some(ClosureSource {
        params,
        header: &source[..subslice_offset(source, return_type) + return_type.len()],
        return_type: Some(return_type),
        body: ClosureBodySource::Block {
            inner: after_arrow[open + '{'.len_utf8()..close].trim(),
            whole: &after_arrow[open..close + '}'.len_utf8()],
        },
    }))
}

fn subslice_offset(source: &str, fragment: &str) -> usize {
    (fragment.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}
