//! Dialogue profile and inline-failure policy decoding.

use super::super::{append, reject_scalar_member};
use crate::{
    decode::{index::ManifestIndex, value},
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode},
    manifest::DialogueProfileSpec,
    source_map::{
        DialogueField, FallbackStyleField, InlineFailureField, InlineFallbackField, ManifestPath,
        ManifestPathSegment, ManifestRootField, ManifestSourceKey, ProfileField,
    },
};
use arcweft_dialogue::{
    CharacterDialogueStyleValue, FallbackStylePolicy, InlineFailurePolicy, InlineFallback,
};
use arcweft_manifest_model::ProfileId;
use arcweft_source::{SourceDocument, SourceSpan};
use arcweft_view::{ViewId, ViewStyleSheetId};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

pub(super) fn decode_dialogue(
    document: &SourceDocument,
    index: &ManifestIndex,
    profile_id: &ProfileId,
    profile_base: &[String],
    profile_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> DialogueProfileSpec {
    let base = append(profile_base, "dialogue");
    if reject_scalar_member(index, &base, "profile dialogue policy", diagnostics) {
        if let Some(field) = index.field_by_path(&base) {
            value::record_field(
                source_entries,
                profile_path(
                    profile_id,
                    [ManifestPathSegment::ProfileField(ProfileField::Dialogue)],
                ),
                field,
            );
        }
        return DialogueProfileSpec::default();
    }
    let Some(anchor) =
        record_dialogue_table(index, profile_id, &base, profile_anchor, source_entries)
    else {
        return DialogueProfileSpec::default();
    };

    let view = decode_optional_dialogue_value::<ViewId>(
        index,
        profile_id,
        &base,
        "view",
        DialogueField::View,
        "dialogue View",
        source_entries,
        diagnostics,
    );
    let style = decode_optional_dialogue_value::<ViewStyleSheetId>(
        index,
        profile_id,
        &base,
        "style",
        DialogueField::Style,
        "dialogue base Style",
        source_entries,
        diagnostics,
    );
    let inline_failure = decode_inline_failure(
        document,
        index,
        profile_id,
        &base,
        &anchor,
        source_entries,
        diagnostics,
    );
    DialogueProfileSpec {
        view,
        style,
        inline_failure,
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_optional_dialogue_value<T>(
    index: &ManifestIndex,
    profile_id: &ProfileId,
    dialogue_base: &[String],
    field_name: &str,
    source_field: DialogueField,
    expectation: &str,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    let path = append(dialogue_base, field_name);
    let field = index.field_by_path(&path)?;
    value::record_field(
        source_entries,
        dialogue_path(profile_id, [ManifestPathSegment::Dialogue(source_field)]),
        field,
    );
    value::typed(
        field,
        ManifestDiagnosticCode::IdInvalid,
        expectation,
        diagnostics,
    )
}

fn decode_inline_failure(
    document: &SourceDocument,
    index: &ManifestIndex,
    profile_id: &ProfileId,
    dialogue_base: &[String],
    dialogue_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<InlineFailurePolicy> {
    let base = append(dialogue_base, "inline-failure");
    if let Some(field) = index.field_by_path(&base) {
        value::record_field(
            source_entries,
            dialogue_path(
                profile_id,
                [ManifestPathSegment::Dialogue(DialogueField::InlineFailure)],
            ),
            field,
        );
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            "dialogue inline-failure must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    }
    let has_policy = index.table_by_path(&base).is_some()
        || index
            .fields
            .keys()
            .any(|path| path.starts_with(&base) && path.len() > base.len())
        || index
            .tables
            .keys()
            .any(|path| path.starts_with(&base) && path.len() > base.len());
    if !has_policy {
        return None;
    }
    let anchor = if let Some(table) = index.table_by_path(&base) {
        value::record_table(
            source_entries,
            dialogue_path(
                profile_id,
                [ManifestPathSegment::Dialogue(DialogueField::InlineFailure)],
            ),
            table,
        );
        table.header_span.clone()
    } else {
        dialogue_anchor.clone()
    };
    let kind_path = append(&base, "kind");
    let kind = required_policy_text(
        index,
        &kind_path,
        &anchor,
        "inline-failure kind",
        inline_failure_path(
            profile_id,
            [ManifestPathSegment::InlineFailure(InlineFailureField::Kind)],
        ),
        source_entries,
        diagnostics,
    )?;

    match kind.as_str() {
        "fail_line" => {
            reject_present_policy_path(
                index,
                &append(&base, "fallback"),
                "fail_line inline policy cannot declare fallback",
                diagnostics,
            );
            Some(InlineFailurePolicy::FailLine)
        }
        "discard" => {
            reject_present_policy_path(
                index,
                &append(&base, "fallback"),
                "discard inline policy cannot declare fallback",
                diagnostics,
            );
            Some(InlineFailurePolicy::Discard)
        }
        "fallback" => decode_fallback(
            document,
            index,
            profile_id,
            &base,
            &anchor,
            source_entries,
            diagnostics,
        )
        .map(|fallback| InlineFailurePolicy::Fallback { fallback }),
        _ => {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::InlinePolicyInvalid,
                format!("unknown inline-failure kind `{kind}`"),
                index
                    .field_by_path(&kind_path)
                    .map_or_else(|| anchor.clone(), |field| field.value_span.clone()),
                Vec::new(),
            ));
            None
        }
    }
}

// This match is the closed wire contract for all fallback variants; keeping
// their mutually exclusive fields together makes strictness auditable.
#[allow(clippy::too_many_lines)]
fn decode_fallback(
    document: &SourceDocument,
    index: &ManifestIndex,
    profile_id: &ProfileId,
    policy_base: &[String],
    policy_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<InlineFallback> {
    let base = append(policy_base, "fallback");
    if let Some(field) = index.field_by_path(&base) {
        value::record_field(
            source_entries,
            inline_failure_path(
                profile_id,
                [ManifestPathSegment::InlineFailure(
                    InlineFailureField::Fallback,
                )],
            ),
            field,
        );
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            "inline fallback must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    }
    let Some(anchor) = record_policy_table(
        index,
        &base,
        inline_failure_path(
            profile_id,
            [ManifestPathSegment::InlineFailure(
                InlineFailureField::Fallback,
            )],
        ),
        policy_anchor,
        source_entries,
    ) else {
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            "fallback inline policy requires a fallback table",
            policy_anchor.clone(),
            Vec::new(),
        ));
        return None;
    };
    let kind_path = append(&base, "kind");
    let kind = required_policy_text(
        index,
        &kind_path,
        &anchor,
        "inline fallback kind",
        fallback_path(
            profile_id,
            [ManifestPathSegment::InlineFallback(
                InlineFallbackField::Kind,
            )],
        ),
        source_entries,
        diagnostics,
    )?;
    match kind.as_str() {
        "text" => {
            let text_path = append(&base, "text");
            let text = required_policy_text(
                index,
                &text_path,
                &anchor,
                "inline fallback text",
                fallback_path(
                    profile_id,
                    [ManifestPathSegment::InlineFallback(
                        InlineFallbackField::Text,
                    )],
                ),
                source_entries,
                diagnostics,
            );
            let style = decode_fallback_style(
                document,
                index,
                profile_id,
                &base,
                &anchor,
                source_entries,
                diagnostics,
            );
            text.zip(style)
                .map(|(text, style)| InlineFallback::Text { text, style })
        }
        "expr_source" => {
            reject_present_policy_path(
                index,
                &append(&base, "text"),
                "expr_source fallback cannot declare text",
                diagnostics,
            );
            decode_fallback_style(
                document,
                index,
                profile_id,
                &base,
                &anchor,
                source_entries,
                diagnostics,
            )
            .map(|style| InlineFallback::ExprSource { style })
        }
        "call_source" => {
            reject_present_policy_path(
                index,
                &append(&base, "text"),
                "call_source fallback cannot declare text",
                diagnostics,
            );
            decode_fallback_style(
                document,
                index,
                profile_id,
                &base,
                &anchor,
                source_entries,
                diagnostics,
            )
            .map(|style| InlineFallback::CallSource { style })
        }
        "value_plain" => {
            reject_present_policy_path(
                index,
                &append(&base, "text"),
                "value_plain fallback cannot declare text",
                diagnostics,
            );
            reject_present_policy_path(
                index,
                &append(&base, "style"),
                "value_plain fallback cannot declare style",
                diagnostics,
            );
            Some(InlineFallback::ValuePlain)
        }
        _ => {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::InlinePolicyInvalid,
                format!("unknown inline fallback kind `{kind}`"),
                index
                    .field_by_path(&kind_path)
                    .map_or_else(|| anchor.clone(), |field| field.value_span.clone()),
                Vec::new(),
            ));
            None
        }
    }
}

fn decode_fallback_style(
    document: &SourceDocument,
    index: &ManifestIndex,
    profile_id: &ProfileId,
    fallback_base: &[String],
    fallback_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<FallbackStylePolicy> {
    let base = append(fallback_base, "style");
    if let Some(field) = index.field_by_path(&base) {
        value::record_field(
            source_entries,
            fallback_path(
                profile_id,
                [ManifestPathSegment::InlineFallback(
                    InlineFallbackField::Style,
                )],
            ),
            field,
        );
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            "fallback style must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    }
    let Some(anchor) = record_policy_table(
        index,
        &base,
        fallback_path(
            profile_id,
            [ManifestPathSegment::InlineFallback(
                InlineFallbackField::Style,
            )],
        ),
        fallback_anchor,
        source_entries,
    ) else {
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            "inline fallback requires a style table",
            fallback_anchor.clone(),
            Vec::new(),
        ));
        return None;
    };
    let kind_path = append(&base, "kind");
    let kind = required_policy_text(
        index,
        &kind_path,
        &anchor,
        "fallback style kind",
        fallback_style_path(
            profile_id,
            [ManifestPathSegment::FallbackStyle(FallbackStyleField::Kind)],
        ),
        source_entries,
        diagnostics,
    )?;
    match kind.as_str() {
        "plain" => {
            reject_present_policy_path(
                index,
                &append(&base, "styles"),
                "plain fallback style cannot declare styles",
                diagnostics,
            );
            Some(FallbackStylePolicy::Plain)
        }
        "inherit_surrounding" => {
            reject_present_policy_path(
                index,
                &append(&base, "styles"),
                "inherit_surrounding fallback style cannot declare styles",
                diagnostics,
            );
            Some(FallbackStylePolicy::InheritSurrounding)
        }
        "apply" => decode_style_array(
            document,
            index,
            profile_id,
            &base,
            &anchor,
            source_entries,
            diagnostics,
        )
        .map(|styles| FallbackStylePolicy::Apply { styles }),
        _ => {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::InlinePolicyInvalid,
                format!("unknown fallback style kind `{kind}`"),
                index
                    .field_by_path(&kind_path)
                    .map_or_else(|| anchor.clone(), |field| field.value_span.clone()),
                Vec::new(),
            ));
            None
        }
    }
}

fn decode_style_array(
    document: &SourceDocument,
    index: &ManifestIndex,
    profile_id: &ProfileId,
    style_base: &[String],
    style_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<Vec<CharacterDialogueStyleValue>> {
    let path = append(style_base, "styles");
    let field = required_policy_field(index, &path, style_anchor, "fallback styles", diagnostics)?;
    value::record_field(
        source_entries,
        fallback_style_path(
            profile_id,
            [ManifestPathSegment::FallbackStyle(
                FallbackStyleField::Styles,
            )],
        ),
        field,
    );
    let elements = value::array_elements(document, field, "fallback styles", diagnostics)?;
    let expected_count = elements.len();
    let mut styles = Vec::with_capacity(expected_count);
    for (element_index, (node, span)) in elements.into_iter().enumerate() {
        let Some(source_index) = value::bounded_array_index(element_index, &span, diagnostics)
        else {
            continue;
        };
        value::record_array_element(
            source_entries,
            fallback_style_path(
                profile_id,
                [
                    ManifestPathSegment::FallbackStyle(FallbackStyleField::Styles),
                    ManifestPathSegment::Index(source_index),
                ],
            ),
            source_index,
            span.clone(),
        );
        if let Some(style) = value::typed_node(
            node,
            span,
            ManifestDiagnosticCode::InlinePolicyInvalid,
            "fallback style value",
            diagnostics,
        ) {
            styles.push(style);
        }
    }
    (styles.len() == expected_count).then_some(styles)
}

fn required_policy_text(
    index: &ManifestIndex,
    raw_path: &[String],
    anchor: &SourceSpan,
    expectation: &str,
    typed_path: ManifestPath,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<String> {
    let field = required_policy_field(index, raw_path, anchor, expectation, diagnostics)?;
    value::record_field(source_entries, typed_path, field);
    value::text(
        field,
        ManifestDiagnosticCode::InlinePolicyInvalid,
        expectation,
        diagnostics,
    )
}

fn required_policy_field<'index>(
    index: &'index ManifestIndex,
    raw_path: &[String],
    anchor: &SourceSpan,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<&'index crate::decode::index::IndexedField> {
    index.field_by_path(raw_path).or_else(|| {
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            format!("{expectation} is required"),
            anchor.clone(),
            Vec::new(),
        ));
        None
    })
}

fn reject_present_policy_path(
    index: &ManifestIndex,
    path: &[String],
    message: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) {
    let exact = index
        .field_by_path(path)
        .map(|field| field.key_span.clone())
        .or_else(|| {
            index
                .table_by_path(path)
                .map(|table| table.header_span.clone())
        });
    let nested_field = index
        .fields
        .iter()
        .filter(|(candidate, _)| candidate.starts_with(path) && candidate.len() > path.len())
        .filter_map(|(_, field)| field.path_spans.get(path.len().saturating_sub(1)).cloned())
        .min_by_key(|span| (span.range().start(), span.range().end()));
    let nested_table = index
        .tables
        .iter()
        .filter(|(candidate, _)| candidate.starts_with(path) && candidate.len() > path.len())
        .filter_map(|(_, table)| table.path_spans.get(path.len().saturating_sub(1)).cloned())
        .min_by_key(|span| (span.range().start(), span.range().end()));
    let span = exact
        .into_iter()
        .chain(nested_field)
        .chain(nested_table)
        .min_by_key(|span| (span.range().start(), span.range().end()));
    if let Some(span) = span {
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            message,
            span,
            Vec::new(),
        ));
    }
}

fn record_policy_table(
    index: &ManifestIndex,
    raw_path: &[String],
    typed_path: ManifestPath,
    fallback_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
) -> Option<SourceSpan> {
    if let Some(table) = index.table_by_path(raw_path) {
        value::record_table(source_entries, typed_path, table);
        return Some(table.header_span.clone());
    }
    let has_nested = index
        .fields
        .keys()
        .any(|path| path.starts_with(raw_path) && path.len() > raw_path.len())
        || index
            .tables
            .keys()
            .any(|path| path.starts_with(raw_path) && path.len() > raw_path.len());
    has_nested.then(|| fallback_anchor.clone())
}

fn record_dialogue_table(
    index: &ManifestIndex,
    profile_id: &ProfileId,
    base: &[String],
    fallback_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
) -> Option<SourceSpan> {
    if let Some(table) = index.table_by_path(base) {
        value::record_table(
            source_entries,
            profile_path(
                profile_id,
                [ManifestPathSegment::ProfileField(ProfileField::Dialogue)],
            ),
            table,
        );
        return Some(table.header_span.clone());
    }
    let has_nested = index
        .fields
        .keys()
        .any(|path| path.starts_with(base) && path.len() > base.len())
        || index
            .tables
            .keys()
            .any(|path| path.starts_with(base) && path.len() > base.len());
    has_nested.then(|| fallback_anchor.clone())
}

fn profile_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    let mut segments = vec![
        ManifestPathSegment::Root(ManifestRootField::Profiles),
        ManifestPathSegment::Profile(profile_id.clone()),
    ];
    segments.extend(tail);
    ManifestPath::new(segments)
}

fn dialogue_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    profile_path(
        profile_id,
        std::iter::once(ManifestPathSegment::ProfileField(ProfileField::Dialogue)).chain(tail),
    )
}

fn inline_failure_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    dialogue_path(
        profile_id,
        std::iter::once(ManifestPathSegment::Dialogue(DialogueField::InlineFailure)).chain(tail),
    )
}

fn fallback_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    inline_failure_path(
        profile_id,
        std::iter::once(ManifestPathSegment::InlineFailure(
            InlineFailureField::Fallback,
        ))
        .chain(tail),
    )
}

fn fallback_style_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    fallback_path(
        profile_id,
        std::iter::once(ManifestPathSegment::InlineFallback(
            InlineFallbackField::Style,
        ))
        .chain(tail),
    )
}
