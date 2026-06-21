use arcweft_lang_syntax::{
    lint::{SyntaxLintCode, lint_id_policy},
    source::ParsedSource,
};

use crate::model::TextEdit;

pub(crate) fn declaration_identity_edits(source: &str, parsed: &ParsedSource) -> Vec<TextEdit> {
    lint_id_policy(parsed.typed_tree())
        .into_iter()
        .filter(|lint| lint.code() == SyntaxLintCode::RedundantDeclIdentity)
        .filter_map(|lint| {
            redundant_decl_identity_edit(source, lint.range().start(), lint.range().end())
        })
        .collect()
}

fn redundant_decl_identity_edit(source: &str, id_start: usize, id_end: usize) -> Option<TextEdit> {
    let edit_start = id_start
        .checked_sub(1)
        .filter(|start| source.as_bytes().get(*start) == Some(&b'@'))
        .unwrap_or(id_start);
    let id_token = source.get(id_start..id_end)?;
    let line_end = source[id_end..]
        .find('\n')
        .map_or(source.len(), |offset| id_end + offset);
    let after_id_raw = source.get(id_end..line_end)?;
    let after_id = after_id_raw.trim_start();
    let id_gap = after_id_raw.len() - after_id.len();
    let name_len = after_id
        .find(|ch: char| ch.is_whitespace() || ch == '(' || ch == ':' || ch == '{')
        .unwrap_or(after_id.len());
    if name_len == 0 {
        return None;
    }
    let name = &after_id[..name_len];
    let id_tail = id_token.trim_start_matches('@').rsplit('.').next()?;
    if id_tail != name {
        return None;
    }
    let end = id_end + id_gap + name_len;
    Some(TextEdit {
        start: edit_start,
        end,
        replacement: name.to_owned(),
    })
}
