use crate::documents::DocumentSnapshot;
use crate::features::cascade::effective_dialogue_cascade_at;
use crate::features::character_metadata::character_hover_markdown;
use crate::features::dialogue_view_metadata::{DialogueViewTypeMetadata, dialogue_view_types};
use crate::features::view_part_metadata::ViewPartMetadataIndex;
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{
    check::{
        EffectRow, EffectRowTail, TypeCheckReport, TypeJudgmentSubject, analyze_types,
        validate_typecheck_ready,
    },
    effect_model::CallableId,
    resolve::{registry_from_hir, validate_hir_references},
    types::TypeKind,
};
use arcweft_lang_syntax::ast::dialogue::{
    DialogueDefaultAssignOp, DialogueDefaultAssignment, DialogueDefaultsItem,
};
use arcweft_lang_syntax::ast::{
    common::TextRange,
    items::{Item, TypedSyntaxTree},
};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{
    LineDisplaySpec, RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource,
    RichTextStyleContribution,
};
use arcweft_verify_lsp::{LspPositionMapper, profile_hover};
use lsp_types::{Hover, HoverContents, MarkedString, Position};

/// Computes hover text for the word under the cursor.
pub fn hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    position: Position,
) -> Option<Hover> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)
        .ok()?;
    if let Some(hover) = crate::features::entry_roles::hover(profile, document, offset) {
        return Some(hover);
    }
    if let Some(text) = ViewPartMetadataIndex::for_document(profile, document)
        .and_then(|metadata| metadata.hover(offset))
    {
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(text)),
            range: None,
        });
    }
    if let Some(hover) = dialogue_defaults_hover(document, offset) {
        return Some(hover);
    }
    if let Some(hover) = effective_dialogue_style_hover(profile, document, offset) {
        return Some(hover);
    }
    let word = word_at_position_range(document, position);
    if let Some((word, word_range)) = word.as_ref()
        && let Some(hover) = callable_effect_row_hover(profile, document, word, *word_range)
    {
        return Some(hover);
    }
    if let Some(hover) = closure_effect_row_hover(profile, document, offset) {
        return Some(hover);
    }
    let (word, word_range) = word?;
    let expected_character_type = word
        .starts_with('.')
        .then(|| character_nominal_type_at(profile, document, word_range))
        .flatten();
    if let Some(text) = character_hover_markdown(profile, &word, expected_character_type.as_ref()) {
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(text)),
            range: None,
        });
    }
    if let Some(hover) = dialogue_view_hover(profile, document, &word) {
        return Some(hover);
    }
    profile_hover(&profile.context(), &word)
}

fn character_nominal_type_at(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    word_range: TextRange,
) -> Option<TypeKind> {
    let parsed = parse_source(document.text().to_owned());
    if !parsed.errors().is_empty() {
        return None;
    }
    let hir = lower_to_hir(parsed.typed_tree()).ok()?;
    let registry = registry_from_hir(&hir);
    if validate_hir_references(&hir, &registry).is_err() || validate_typecheck_ready(&hir).is_err()
    {
        return None;
    }
    let report = analyze_types(&hir, &profile.typecheck_env());
    report
        .judgments
        .iter()
        .filter(|judgment| {
            judgment.source_range.is_some_and(|range| {
                range.start() <= word_range.start() && word_range.end() <= range.end()
            })
        })
        .filter_map(|judgment| {
            judgment
                .expected_type()
                .filter(|ty| ty.character_nominal().is_some())
                .or_else(|| {
                    judgment
                        .ty
                        .character_nominal()
                        .is_some()
                        .then_some(&judgment.ty)
                })
                .map(|ty| {
                    (
                        judgment
                            .source_range
                            .map_or(usize::MAX, |range| range.end() - range.start()),
                        ty.clone(),
                    )
                })
        })
        .min_by_key(|(span, _)| *span)
        .map(|(_, ty)| ty)
}

fn dialogue_view_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    word: &str,
) -> Option<Hover> {
    for model in dialogue_view_types(profile, Some(document)) {
        if model.name == word {
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "Dialogue View input model\n\n{}",
                    model.declaration()
                ))),
                range: None,
            });
        }
        if let Some((projection, ty)) = DialogueViewTypeMetadata::fields()
            .into_iter()
            .find(|(projection, _)| projection.field() == word)
        {
            let ty = match ty {
                arcweft_lang_sema::types::TypeKind::Named(name) => name,
                other => format!("{other:?}"),
            };
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "{}.{}: {ty}\n\nRuntime-supplied dialogue View field.",
                    model.name,
                    projection.field()
                ))),
                range: None,
            });
        }
    }
    None
}

fn closure_effect_row_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Hover> {
    if !source_offset_may_be_closure_header(document.text(), offset) {
        return None;
    }
    let parsed = parse_source(document.text().to_owned());
    if !parsed.errors().is_empty() {
        return None;
    }
    let tree = parsed.typed_tree();
    let hir = lower_to_hir(tree).ok()?;
    let registry = registry_from_hir(&hir);
    if validate_hir_references(&hir, &registry).is_err() || validate_typecheck_ready(&hir).is_err()
    {
        return None;
    }
    let report = analyze_types(&hir, &profile.typecheck_env());
    if !report.diagnostics.is_empty() {
        return None;
    }
    let rows = report.effects.effect_rows();
    let target = closure_effect_hover_target(&report, document.text(), offset)?;
    let summary = rows.summary(&target.callable)?;
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(effect_row_hover_text(
            "closure expression",
            summary.inferred(),
            summary.upper_bound(),
            summary.forbidden(),
        ))),
        range: Some(
            document
                .line_index()
                .range_from_byte_span(target.header_range.start(), target.header_range.end()),
        ),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClosureEffectHoverTarget {
    callable: CallableId,
    header_range: TextRange,
}

fn closure_effect_hover_target(
    report: &TypeCheckReport,
    source: &str,
    offset: usize,
) -> Option<ClosureEffectHoverTarget> {
    report
        .judgments
        .iter()
        .filter_map(|judgment| {
            let TypeJudgmentSubject::Expr {
                id,
                kind: "closure",
            } = &judgment.subject
            else {
                return None;
            };
            let id = *id;
            let source_range = judgment.source_range?;
            let header_range = closure_header_range(source, source_range)?;
            if offset < header_range.start() || header_range.end() < offset {
                return None;
            }
            let callable = report.function_effect_callable_for_expression(id)?.clone();
            Some(ClosureEffectHoverTarget {
                callable,
                header_range,
            })
        })
        .min_by_key(|target| target.header_range.end() - target.header_range.start())
}

fn closure_header_range(source: &str, source_range: TextRange) -> Option<TextRange> {
    let closure_source = source.get(source_range.as_range())?;
    let first_pipe = closure_source.find('|')?;
    let second_pipe = closure_source.get(first_pipe + 1..)?.find('|')? + first_pipe + 1;
    Some(TextRange::new(
        source_range.start() + first_pipe,
        source_range.start() + second_pipe + 1,
    ))
}

fn source_offset_may_be_closure_header(source: &str, offset: usize) -> bool {
    if offset > source.len() {
        return false;
    }
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |index| offset + index);
    let Some(line) = source.get(line_start..line_end) else {
        return false;
    };
    let relative_offset = offset.saturating_sub(line_start);
    let mut search_start = 0usize;
    while let Some(first_offset) = line.get(search_start..).and_then(|text| text.find('|')) {
        let first = search_start + first_offset;
        let Some(second_offset) = line.get(first + 1..).and_then(|text| text.find('|')) else {
            return false;
        };
        let second = first + 1 + second_offset;
        if first <= relative_offset && relative_offset <= second + 1 {
            return true;
        }
        search_start = second + 1;
    }
    false
}

fn callable_effect_row_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    word: &str,
    word_range: TextRange,
) -> Option<Hover> {
    let parsed = parse_source(document.text().to_owned());
    if !parsed.errors().is_empty() {
        return None;
    }
    let tree = parsed.typed_tree();
    let callable = callable_at_word(tree, document.text(), word, word_range)?;
    let hir = lower_to_hir(tree).ok()?;
    let registry = registry_from_hir(&hir);
    if validate_hir_references(&hir, &registry).is_err() || validate_typecheck_ready(&hir).is_err()
    {
        return None;
    }
    let report = analyze_types(&hir, &profile.typecheck_env());
    if !report.diagnostics.is_empty() {
        return None;
    }
    let rows = report.effects.effect_rows();
    let summary = rows.summary(&callable.id)?;
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(effect_row_hover_text(
            callable.label.as_str(),
            summary.inferred(),
            summary.upper_bound(),
            summary.forbidden(),
        ))),
        range: Some(
            document
                .line_index()
                .range_from_byte_span(word_range.start(), word_range.end()),
        ),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallableHoverTarget {
    id: CallableId,
    label: String,
}

fn callable_at_word(
    tree: &TypedSyntaxTree,
    source: &str,
    word: &str,
    word_range: TextRange,
) -> Option<CallableHoverTarget> {
    tree.items().iter().find_map(|item| match item {
        Item::Flow(flow)
            if flow.name() == Some(word)
                && declaration_header_contains_word(source, flow.range(), word_range) =>
        {
            Some(CallableHoverTarget {
                id: CallableId::new(format!("flow.{word}")),
                label: word.to_owned(),
            })
        }
        Item::Function(function)
            if function.signature().name() == word
                && declaration_header_contains_word(source, function.range(), word_range) =>
        {
            Some(CallableHoverTarget {
                id: CallableId::new(format!("fn.{word}")),
                label: word.to_owned(),
            })
        }
        _ => None,
    })
}

fn declaration_header_contains_word(
    source: &str,
    item_range: &TextRange,
    word_range: TextRange,
) -> bool {
    if word_range.start() < item_range.start() || item_range.end() < word_range.end() {
        return false;
    }
    let Some(item_source) = source.get(item_range.as_range()) else {
        return false;
    };
    let header_end = item_source.find('{').unwrap_or(item_source.len());
    word_range.end() <= item_range.start().saturating_add(header_end)
}

fn effect_row_hover_text(
    label: &str,
    inferred: &EffectRow,
    upper_bound: Option<&EffectRow>,
    forbidden: &EffectRow,
) -> String {
    let mut lines = vec![
        format!("effect row for `{label}`"),
        format!("inferred: {}", inferred.display_label()),
    ];
    if let Some(upper_bound) = upper_bound {
        lines.push(format!("upper bound: {}", upper_bound.display_label()));
    } else {
        lines.push("upper bound: inferred".to_owned());
    }
    if effect_row_has_visible_forbidden_value(forbidden) {
        lines.push(format!("forbidden: {}", forbidden.display_label()));
    }
    lines.join("\n")
}

fn effect_row_has_visible_forbidden_value(row: &EffectRow) -> bool {
    match row.tail() {
        EffectRowTail::Unknown => false,
        EffectRowTail::Closed => !row.concrete().is_empty(),
        EffectRowTail::Variable(_) => true,
    }
}

fn effective_dialogue_style_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Hover> {
    let cascade = effective_dialogue_cascade_at(document, offset, profile.dialogue_defaults())?;
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
        RichTextCascadeLayer::DialogueViewStyle,
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
        RichTextCascadeLayer::DialogueViewStyle => "dialogue_view_style",
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
    word_at_position_range(document, position).map(|(word, _)| word)
}

fn word_at_position_range(
    document: &DocumentSnapshot,
    position: Position,
) -> Option<(String, TextRange)> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)
        .ok()?;
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
    (start < end).then(|| (text[start..end].to_owned(), TextRange::new(start, end)))
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
pub dialogue defaults {
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
pub dialogue defaults {
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

    #[test]
    fn hover_includes_expanded_fx_style_contributions() {
        let source = r##"
#[fx]
fn red(value: Color = rgb("#a8b5ff")) -> Fx {
    Fx.text(color = value)
}

pub character alice {}

flow opening {
    alice: [fx red()]colored[/fx][p]
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
        let offset = source.find("colored").expect("Fx content offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let hover = hover(&profile, &document, position).expect("effective style hover");

        match hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("effective dialogue style for `alice`"));
                assert!(text.contains("active contributors:"));
                assert!(
                    text.contains("rich_text.text.color = #a8b5ff"),
                    "unexpected Fx hover:\n{text}"
                );
                assert!(text.contains("inline_span"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn hover_describes_closed_flow_and_inferred_function_effect_rows() {
        let source = r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

fn load_story(path: String) -> String
effects { fs.read }
{
    fs.read_text(path = path)
}

flow @flow.opening opening
effects { fs.read }
{
    let body = load_story("story.arcw")
}
"#;
        let mut store = DocumentStore::default();
        let uri = "file:///effect-row-hover.arcw".parse().expect("uri");
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
        let offset = source.find("opening\n").expect("flow name offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let flow_hover = hover(&profile, &document, position).expect("effect row hover");

        match flow_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("effect row for `opening`"));
                assert!(text.contains("inferred: { fs.read }"));
                assert!(text.contains("upper bound: { fs.read }"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let offset = source.find("load_story").expect("function name offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let function_hover =
            hover(&profile, &document, position).expect("function effect row hover");

        match function_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("effect row for `load_story`"));
                assert!(text.contains("inferred: { fs.read | e"));
                assert!(text.contains("upper bound: { fs.read }"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn callable_effect_row_hover_ignores_body_name_references() {
        let source = r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

fn load_story(path: String) -> String
effects { fs.read }
{
    fs.read_text(path = path)
}

flow @flow.opening opening
effects { fs.read }
{
    let body = load_story("story.arcw")
}
"#;
        let mut store = DocumentStore::default();
        let uri = "file:///effect-row-body-hover.arcw".parse().expect("uri");
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
        let offset = source.rfind("load_story").expect("body call offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);

        assert!(
            hover(&profile, &document, position).is_none(),
            "body call references should not be treated as callable declarations"
        );
    }

    #[test]
    fn hover_describes_closure_expression_inferred_open_effect_row() {
        let source = r"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

flow @flow.opening opening
effects { }
{
    let later = |path: String| -> String {
        fs.read_text(path = path)
    }
}
";
        let mut store = DocumentStore::default();
        let uri = "file:///closure-effect-row-hover.arcw"
            .parse()
            .expect("uri");
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
        let offset = source.find("|path").expect("closure header offset") + 1;
        let position = document.line_index().position_from_byte_offset(offset);
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let closure_hover = hover(&profile, &document, position).expect("closure effect row hover");

        match closure_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("effect row for `closure expression`"));
                assert!(text.contains("inferred: { fs.read | e"));
                assert!(text.contains("upper bound: inferred"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let body_offset = source.rfind("fs.read_text").expect("body call offset");
        let body_position = document.line_index().position_from_byte_offset(body_offset);
        let body_hover = hover(&profile, &document, body_position);
        if let Some(body_hover) = body_hover {
            match body_hover.contents {
                HoverContents::Scalar(MarkedString::String(text)) => assert!(
                    !text.contains("effect row for `closure expression`"),
                    "closure expression hover should stay limited to the closure header: {text}"
                ),
                other => panic!("unexpected hover contents: {other:?}"),
            }
        }
    }

    #[test]
    fn hover_describes_closure_expression_expected_effect_row_bound() {
        let source = r"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

flow @flow.opening opening
effects { }
{
    let later: String -> String effects { fs.read } =
        |path: String| -> String {
            fs.read_text(path = path)
        }
}
";
        let mut store = DocumentStore::default();
        let uri = "file:///closure-effect-row-bound-hover.arcw"
            .parse()
            .expect("uri");
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
        let offset = source.find("|path").expect("closure header offset") + 1;
        let position = document.line_index().position_from_byte_offset(offset);
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let closure_hover = hover(&profile, &document, position).expect("closure effect row hover");

        match closure_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("effect row for `closure expression`"));
                assert!(text.contains("inferred: { fs.read | e"));
                assert!(text.contains("upper bound: { fs.read }"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn effect_row_hover_text_renders_open_rows_without_closed_projection() {
        use arcweft_lang_sema::{check::EffectVar, effects::EffectSet};

        let variable = EffectVar::from_index(9);
        let inferred = EffectRow::open(
            EffectSet::from_labels(["fs.read"]).expect("valid inferred row"),
            variable,
        );
        let upper_bound = EffectRow::open(EffectSet::new(), variable);
        let forbidden = EffectRow::closed(EffectSet::new());

        let text = effect_row_hover_text("callback", &inferred, Some(&upper_bound), &forbidden);
        assert!(text.contains("effect row for `callback`"));
        assert!(text.contains("inferred: { fs.read | e9 }"));
        assert!(text.contains("upper bound: { | e9 }"));
        assert!(
            !text.contains("forbidden:"),
            "empty closed forbidden row should stay hidden: {text}"
        );
    }
}
