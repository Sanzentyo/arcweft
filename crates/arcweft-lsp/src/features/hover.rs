use crate::documents::DocumentSnapshot;
use crate::features::cascade::effective_dialogue_cascade_at;
use crate::profiles::LspProfile;
use arcweft_lang_syntax::ast::dialogue::{
    DialogueDefaultAssignOp, DialogueDefaultAssignment, DialogueDefaultsItem,
};
use arcweft_lang_syntax::ast::items::Item;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{
    LineDisplaySpec, RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource,
    RichTextStyleContribution,
};
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
    let cascade = effective_dialogue_cascade_at(document, offset)?;
    let contributions = cascade.selected_contributions();
    if contributions.is_empty() {
        return None;
    }

    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(effective_style_hover_text(
            &cascade.spec,
            cascade.selected_path.as_deref(),
            &contributions,
        ))),
        range: None,
    })
}

fn effective_style_hover_text(
    spec: &LineDisplaySpec,
    selected_path: Option<&str>,
    contributions: &[&RichTextStyleContribution],
) -> String {
    let mut lines = vec![selected_path.map_or_else(
        || format!("effective dialogue style for `{}`", spec.callee),
        |path| format!("effective dialogue style `{path}` for `{}`", spec.callee),
    )];
    let active = contributions
        .iter()
        .copied()
        .filter(|contribution| contribution.active)
        .collect::<Vec<_>>();
    if !active.is_empty() {
        lines.push("active contributors:".to_owned());
        lines.extend(
            active
                .iter()
                .take(8)
                .map(|contribution| format!("  {}", contribution_label(contribution))),
        );
    }

    let shadowed = contributions
        .iter()
        .copied()
        .filter(|contribution| contribution.shadowed_by.is_some())
        .collect::<Vec<_>>();
    if !shadowed.is_empty() {
        lines.push("shadowed contributors:".to_owned());
        lines.extend(shadowed.iter().take(8).map(|contribution| {
            let shadowed_by = contribution
                .shadowed_by
                .map_or("?".to_owned(), |index| format!("#{index}"));
            format!(
                "  {} (shadowed by {shadowed_by})",
                contribution_label(contribution)
            )
        }));
    }

    let unset_layers = unset_cascade_layers(spec);
    if !unset_layers.is_empty() {
        lines.push(format!("unset layers: {}", unset_layers.join(", ")));
    }

    lines.join("\n")
}

fn contribution_label(contribution: &RichTextStyleContribution) -> String {
    format!(
        "{} = {} ({}, {}, {})",
        contribution.path,
        contribution.value,
        cascade_layer_label(contribution.layer),
        assign_op_label(contribution.op),
        setting_source_label(&contribution.source)
    )
}

fn unset_cascade_layers(spec: &LineDisplaySpec) -> Vec<&'static str> {
    all_cascade_layers()
        .into_iter()
        .filter(|layer| {
            !spec
                .style_contributions
                .iter()
                .any(|contribution| contribution.layer == *layer)
        })
        .map(cascade_layer_label)
        .collect()
}

fn all_cascade_layers() -> [RichTextCascadeLayer; 7] {
    [
        RichTextCascadeLayer::InlineSpan,
        RichTextCascadeLayer::LineOptions,
        RichTextCascadeLayer::SpeakerPreset,
        RichTextCascadeLayer::CharacterDialogueStyle,
        RichTextCascadeLayer::DialogueWindowTheme,
        RichTextCascadeLayer::DialogueDefaults,
        RichTextCascadeLayer::EngineDefaults,
    ]
}

fn cascade_layer_label(layer: RichTextCascadeLayer) -> &'static str {
    match layer {
        RichTextCascadeLayer::InlineSpan => "inline_span",
        RichTextCascadeLayer::LineOptions => "line_options",
        RichTextCascadeLayer::SpeakerPreset => "speaker_preset",
        RichTextCascadeLayer::CharacterDialogueStyle => "character_dialogue_style",
        RichTextCascadeLayer::DialogueWindowTheme => "dialogue_window_theme",
        RichTextCascadeLayer::DialogueDefaults => "dialogue_defaults",
        RichTextCascadeLayer::EngineDefaults => "engine_defaults",
    }
}

fn assign_op_label(op: RichTextAssignOp) -> &'static str {
    match op {
        RichTextAssignOp::Replace => "replace",
        RichTextAssignOp::Append => "append",
    }
}

fn setting_source_label(source: &RichTextSettingSource) -> String {
    match source {
        RichTextSettingSource::SourceFile {
            item_id,
            public_id,
            range,
        } => {
            let identity = item_id
                .as_deref()
                .or(public_id.as_deref())
                .unwrap_or("source");
            range.map_or_else(
                || format!("source_file:{identity}"),
                |range| format!("source_file:{identity}@{}..{}", range.start, range.end),
            )
        }
        RichTextSettingSource::EngineDefault { key } => format!("engine_default:{key}"),
    }
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
        text {
            color = rgb("#101112")
        }
        ruby {
            size = 14px
        }
    }
}

pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#202122")
            }
        }
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
                assert!(text.contains("active contributors:"));
                assert!(text.contains("rich_text.ruby.size = 14px"));
                assert!(text.contains("rich_text.text.color = rgb(\"#202122\")"));
                assert!(text.contains("shadowed contributors:"));
                assert!(text.contains("rich_text.text.color = rgb(\"#101112\")"));
                assert!(text.contains("unset layers:"));
                assert!(text.contains("line_options"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }
}
