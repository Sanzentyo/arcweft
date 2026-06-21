use arcweft_lang_syntax::cst::CstLineEvents;
use std::collections::BTreeSet;

use crate::dialogue_sugar::{
    DialogueSugarContext, DialogueSugarMode, dialogue_text_canonical_edits,
};
use crate::edit::apply_text_edits;
use crate::model::TextEdit;
use crate::util::is_identifier;

pub(crate) fn parent_path_edits(line: &str, base: usize) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut search = 0;
    while let Some(offset) = line[search..].find("parent::") {
        let start = search + offset;
        edits.push(TextEdit {
            start: base + start,
            end: base + start + "parent".len(),
            replacement: "super".to_owned(),
        });
        search = start + "parent::".len();
    }
    edits
}

pub(crate) fn await_question_edit(line: &str, base: usize) -> Option<TextEdit> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("await? ")?;
    Some(TextEdit {
        start: base + leading,
        end: base + line.len(),
        replacement: format!("try await {rest}"),
    })
}

pub(crate) fn speaker_line_edit(
    line: &str,
    base: usize,
    speaker_presets: &BTreeSet<String>,
    character_aliases: &BTreeSet<String>,
) -> Option<TextEdit> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    if trimmed.starts_with("//")
        || trimmed.starts_with("///")
        || trimmed.starts_with("with:")
        || trimmed.starts_with("case ")
    {
        return None;
    }
    let (head, text) = trimmed.split_once(':')?;
    if head.contains(' ') || text.trim().is_empty() || head.starts_with('@') {
        return None;
    }
    let (base_name, args) = split_call_head(head.trim());
    if !is_identifier(base_name) {
        return None;
    }
    let text = canonical_dialogue_text_for_speaker_line(text.trim_start());
    let callee = if speaker_presets.contains(base_name) {
        args.map_or_else(
            || base_name.to_owned(),
            |args| format!("{base_name}({args})"),
        )
    } else if args.is_some() || character_aliases.contains(base_name) {
        args.map_or_else(
            || format!("{base_name}.say()"),
            |args| format!("{base_name}.say({args})"),
        )
    } else {
        format!("{base_name}.say()")
    };
    Some(TextEdit {
        start: base + leading,
        end: base + line.len(),
        replacement: format!("{callee}[{text}]"),
    })
}

fn canonical_dialogue_text_for_speaker_line(text: &str) -> String {
    let edits = dialogue_text_canonical_edits(
        text,
        0,
        DialogueSugarMode::All,
        &DialogueSugarContext::default(),
    );
    apply_text_edits(text, &edits).unwrap_or_else(|_| text.to_owned())
}

pub(crate) fn split_call_head(head: &str) -> (&str, Option<&str>) {
    let Some(open) = head.find('(') else {
        return (head, None);
    };
    if !head.ends_with(')') {
        return (head, None);
    }
    (&head[..open], Some(&head[open + 1..head.len() - 1]))
}

pub(crate) fn closing_brace_insert(lines: &CstLineEvents, with_start: usize) -> Option<TextEdit> {
    let index = lines.iter().position(|line| line.start() == with_start)?;
    let line = lines.get(index)?;
    let indent = leading_whitespace(line.text());
    let mut last_body = line;
    for candidate in lines.iter().skip(index + 1) {
        if candidate.trimmed().is_empty() {
            last_body = candidate;
            continue;
        }
        if leading_whitespace(candidate.text()).len() <= indent.len() {
            break;
        }
        last_body = candidate;
    }
    let insert_at = last_body.end();
    Some(TextEdit {
        start: insert_at,
        end: insert_at,
        replacement: format!("\n{indent}}}"),
    })
}

fn leading_whitespace(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}
