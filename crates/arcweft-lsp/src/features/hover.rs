use crate::documents::DocumentSnapshot;
use crate::features::cascade::effective_dialogue_cascade_at;
use crate::profiles::LspProfile;
use arcweft_lang_syntax::ast::dialogue::{
    DialogueDefaultAssignOp, DialogueDefaultAssignment, DialogueDefaultsItem,
};
use arcweft_lang_syntax::ast::items::Item;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_verify_lsp::profile_hover;
use lsp_types::{Hover, HoverContents, MarkedString, Position};

/// Computes hover text for the word under the cursor.
pub fn hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    position: Position,
) -> Option<Hover> {
    let offset = document.line_index().byte_offset_from_position(position);
    if let Some(hover) = dialogue_defaults_hover(document, offset) {
        return Some(hover);
    }
    if let Some(hover) = effective_dialogue_style_hover(document, offset) {
        return Some(hover);
    }
    let word = word_at_position(document, position)?;
    profile_hover(&profile.context(), &word)
}

fn effective_dialogue_style_hover(document: &DocumentSnapshot, offset: usize) -> Option<Hover> {
    let spec = effective_dialogue_cascade_at(document, offset)?.spec;
    if spec.style_contributions.is_empty() {
        return None;
    }

    let mut lines = vec![format!("effective dialogue style for `{}`", spec.callee)];
    lines.extend(
        spec.style_contributions
            .iter()
            .filter(|contribution| contribution.active)
            .take(8)
            .map(|contribution| {
                format!(
                    "{} = {} ({:?}, {:?})",
                    contribution.path, contribution.value, contribution.layer, contribution.op
                )
            }),
    );
    let shadowed = spec
        .style_contributions
        .iter()
        .filter(|contribution| contribution.shadowed_by.is_some())
        .count();
    if shadowed > 0 {
        lines.push(format!("shadowed contributors: {shadowed}"));
    }

    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(lines.join("\n"))),
        range: None,
    })
}

fn dialogue_defaults_hover(document: &DocumentSnapshot, offset: usize) -> Option<Hover> {
    parse_source(document.text())
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::DialogueDefaults(defaults) => Some(defaults),
            _ => None,
        })
        .flat_map(DialogueDefaultsItem::assignments)
        .find(|assignment| {
            let range = assignment.range();
            range.start() <= offset && offset <= range.end()
        })
        .map(|assignment| dialogue_default_assignment_hover(document, assignment))
}

fn dialogue_default_assignment_hover(
    document: &DocumentSnapshot,
    assignment: &DialogueDefaultAssignment,
) -> Hover {
    Hover {
        contents: HoverContents::Scalar(MarkedString::String(format!(
            "dialogue default\npath: {}\nop: {}\nvalue: {}",
            assignment.path().dotted(),
            dialogue_default_op_label(assignment.op()),
            document_value_label(document, assignment)
        ))),
        range: None,
    }
}

fn dialogue_default_op_label(op: DialogueDefaultAssignOp) -> &'static str {
    match op {
        DialogueDefaultAssignOp::Replace => "=",
        DialogueDefaultAssignOp::Append => "+=",
    }
}

fn document_value_label(
    document: &DocumentSnapshot,
    assignment: &DialogueDefaultAssignment,
) -> String {
    document
        .text()
        .get(assignment.value_range().as_range())
        .map_or("", str::trim)
        .to_owned()
}

pub(crate) fn word_at_position(document: &DocumentSnapshot, position: Position) -> Option<String> {
    let offset = document.line_index().byte_offset_from_position(position);
    let text = document.text();
    let start = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_symbol_char(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let end = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_symbol_char(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    (start < end).then(|| text[start..end].to_owned())
}

fn is_symbol_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '@' | ':' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::DocumentStore;
    use crate::positions::PositionEncoding;
    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem};

    #[test]
    fn hover_describes_dialogue_default_assignment() {
        let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}
";
        let mut store = DocumentStore::default();
        let uri = "file:///story.arcw".parse().expect("uri");
        let document = store.open(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "arcweft".to_owned(),
                    version: 1,
                    text: source.to_owned(),
                },
            },
            PositionEncoding::Utf16,
        );
        let offset = source.find("14px").expect("value offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let hover = hover(&profile, &document, position).expect("dialogue default hover");

        match hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("dialogue default"));
                assert!(text.contains("path: rich_text.ruby.size"));
                assert!(text.contains("op: ="));
                assert!(text.contains("value: 14px"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn hover_describes_effective_dialogue_style_cascade() {
        let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

pub character alice {
    dialogue_style {
        text_color = rgb("#202122")
    }
}

flow opening {
    alice: |[夢](ゆめ)[p]
}
"##;
        let mut store = DocumentStore::default();
        let uri = "file:///story.arcw".parse().expect("uri");
        let document = store.open(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "arcweft".to_owned(),
                    version: 1,
                    text: source.to_owned(),
                },
            },
            PositionEncoding::Utf16,
        );
        let offset = source.find("夢").expect("dialogue content offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let hover = hover(&profile, &document, position).expect("effective style hover");

        match hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("effective dialogue style for `alice`"));
                assert!(text.contains("rich_text.ruby.size = 14px"));
                assert!(text.contains("text_color = rgb(\"#202122\")"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }
}
