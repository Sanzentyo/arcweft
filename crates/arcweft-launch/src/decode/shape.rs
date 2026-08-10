//! Closed schema-1 shape validation before typed values are materialized.

use super::index::ManifestIndex;
use crate::diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode};
use arcweft_source::SourceSpan;

// Keeping this source-ordered pass together makes the accepted schema and the
// diagnostic ordering auditable as one closed grammar.
#[allow(clippy::too_many_lines)]
pub(super) fn validate_known_shape(index: &mut ManifestIndex) {
    let table_paths = index.tables.keys().cloned().collect::<Vec<_>>();
    let field_diagnostics = index
        .fields
        .iter()
        .filter_map(|(path, field)| {
            if path.is_empty() {
                return None;
            }
            if has_unknown_table_prefix(path, &table_paths) {
                return None;
            }
            if !is_known_root(&path[0]) {
                return (path.len() == 1).then(|| {
                    diagnostic(
                        ManifestDiagnosticCode::UnknownRootKey,
                        format!("manifest root key `{}` is not part of schema 1", path[0]),
                        field.path_spans[0].clone(),
                    )
                });
            }
            (!field_path_allowed(path)).then(|| {
                unknown_record_diagnostic(
                    path,
                    "field",
                    unknown_segment_span(&field.path_spans, path)
                        .unwrap_or_else(|| field.key_span.clone()),
                )
            })
        })
        .collect::<Vec<_>>();

    let table_diagnostics = index
        .tables
        .iter()
        .filter_map(|(path, table)| {
            if path.is_empty() {
                return None;
            }
            if !is_known_root(&path[0]) {
                return Some(diagnostic(
                    ManifestDiagnosticCode::UnknownTable,
                    format!(
                        "manifest table `{}` is not part of schema 1",
                        path.join(".")
                    ),
                    table.header_span.clone(),
                ));
            }
            (!table_path_allowed(path)).then(|| {
                unknown_record_diagnostic(
                    path,
                    "nested table",
                    unknown_segment_span(&table.path_spans, path)
                        .unwrap_or_else(|| table.header_span.clone()),
                )
            })
        })
        .collect::<Vec<_>>();

    let array_table_diagnostics = index
        .array_tables
        .iter()
        .flat_map(|(path, items)| {
            items.iter().flat_map(move |item| {
                if !array_table_path_allowed(path) {
                    let code = if path
                        .first()
                        .is_some_and(|root| !is_known_root(root.as_str()))
                    {
                        ManifestDiagnosticCode::UnknownTable
                    } else {
                        ManifestDiagnosticCode::UnknownField
                    };
                    return vec![diagnostic(
                        code,
                        format!(
                            "manifest table array `{}` is not part of schema 1",
                            path.join(".")
                        ),
                        item.header_span.clone(),
                    )];
                }
                let field_diagnostics = item
                    .fields
                    .iter()
                    .filter(|(field_path, _)| {
                        !matches!(
                            field_path.as_slice(),
                            [field] if matches!(field.as_str(), "activity" | "implementation")
                        )
                    })
                    .map(|(_, indexed)| {
                        diagnostic(
                            ManifestDiagnosticCode::UnknownField,
                            "manifest Activity binding contains an unknown field",
                            indexed
                                .path_spans
                                .get(path.len())
                                .cloned()
                                .unwrap_or_else(|| indexed.key_span.clone()),
                        )
                    });
                let table_diagnostics = item.tables.values().map(|table| {
                    diagnostic(
                        ManifestDiagnosticCode::UnknownField,
                        "manifest Activity binding contains an unknown nested table",
                        table.header_span.clone(),
                    )
                });
                field_diagnostics
                    .chain(table_diagnostics)
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();

    let inline_binding_diagnostics = index
        .inline_arrays
        .iter()
        .filter(|(path, _)| array_table_path_allowed(path))
        .flat_map(|(_, items)| {
            items.iter().flat_map(|item| {
                let field_diagnostics = item
                    .fields
                    .iter()
                    .filter(|(path, _)| {
                        !matches!(
                            path.as_slice(),
                            [field] if matches!(field.as_str(), "activity" | "implementation")
                        )
                    })
                    .map(|(_, field)| {
                        diagnostic(
                            ManifestDiagnosticCode::UnknownField,
                            "manifest Activity binding contains an unknown field",
                            field.key_span.clone(),
                        )
                    });
                let table_diagnostics = item.tables.values().map(|table| {
                    diagnostic(
                        ManifestDiagnosticCode::UnknownField,
                        "manifest Activity binding contains an unknown nested table",
                        table.header_span.clone(),
                    )
                });
                field_diagnostics
                    .chain(table_diagnostics)
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();

    index.diagnostics.extend(
        field_diagnostics
            .into_iter()
            .chain(table_diagnostics)
            .chain(array_table_diagnostics)
            .chain(inline_binding_diagnostics),
    );
}

fn is_known_root(root: &str) -> bool {
    matches!(
        root,
        "schema"
            | "package"
            | "build"
            | "resource-type-manifest"
            | "content-units"
            | "external-modules"
            | "activity-implementations"
            | "default-profile"
            | "profiles"
    )
}

fn field_path_allowed(path: &[String]) -> bool {
    match path {
        [root]
            if matches!(
                root.as_str(),
                "schema"
                    | "resource-type-manifest"
                    | "package"
                    | "build"
                    | "content-units"
                    | "external-modules"
                    | "activity-implementations"
                    | "default-profile"
                    | "profiles"
            ) =>
        {
            true
        }
        [root, field] if root == "package" => matches!(field.as_str(), "id" | "version"),
        [root, field] if root == "build" => {
            matches!(field.as_str(), "source-dir" | "target-dir" | "incremental")
        }
        [root, _, field] if root == "content-units" => {
            matches!(field.as_str(), "roots" | "visibility" | "demand")
        }
        [root, _, field] if root == "external-modules" => matches!(
            field.as_str(),
            "mount"
                | "metadata"
                | "metadata-hash"
                | "expected-package"
                | "expected-version"
                | "expected-module"
                | "expected-family"
                | "expected-abi-hash"
                | "visibility"
                | "demand"
        ),
        [root, _, field] if root == "activity-implementations" => {
            matches!(field.as_str(), "module" | "export")
        }
        [root, _, field] if root == "profiles" => matches!(
            field.as_str(),
            "kind"
                | "source"
                | "entry"
                | "adapter"
                | "external-modules"
                | "activity-bindings"
                | "dialogue"
                | "localization"
                | "listen"
                | "pure"
                | "content"
                | "player"
        ),
        [root, _, record, field] if root == "profiles" && record == "dialogue" => {
            matches!(field.as_str(), "view" | "style" | "inline-failure")
        }
        [root, _, record, field] if root == "profiles" && record == "pure" => matches!(
            field.as_str(),
            "backend"
                | "math-backend"
                | "math-wgpu-min-elements"
                | "workers"
                | "batch-min-len"
                | "object-artifacts"
        ),
        [root, _, record, _, field] if root == "profiles" && record == "content" => {
            matches!(field.as_str(), "residency" | "placement" | "compression")
        }
        [root, _, player, viewport, field]
            if root == "profiles" && player == "player" && viewport == "viewport" =>
        {
            matches!(field.as_str(), "design-width" | "design-height" | "fit")
        }
        [root, _, dialogue, policy, field]
            if root == "profiles" && dialogue == "dialogue" && policy == "inline-failure" =>
        {
            matches!(field.as_str(), "kind" | "fallback")
        }
        [root, _, dialogue, policy, fallback, field]
            if root == "profiles"
                && dialogue == "dialogue"
                && policy == "inline-failure"
                && fallback == "fallback" =>
        {
            matches!(field.as_str(), "kind" | "text" | "style")
        }
        [root, _, dialogue, policy, fallback, style, field]
            if root == "profiles"
                && dialogue == "dialogue"
                && policy == "inline-failure"
                && fallback == "fallback"
                && style == "style" =>
        {
            matches!(field.as_str(), "kind" | "styles")
        }
        _ => localization_field_path_allowed(path),
    }
}

fn localization_field_path_allowed(path: &[String]) -> bool {
    matches!(
        path,
        [root, _, localization, field]
            if root == "profiles"
                && localization == "localization"
                && field == "character_names"
    ) || matches!(
        path,
        [root, _, localization, character_names, field]
            if root == "profiles"
                && localization == "localization"
                && character_names == "character_names"
                && matches!(field.as_str(), "active" | "fallbacks")
    )
}

fn table_path_allowed(path: &[String]) -> bool {
    match path {
        [root]
            if matches!(
                root.as_str(),
                "package"
                    | "build"
                    | "content-units"
                    | "external-modules"
                    | "activity-implementations"
                    | "profiles"
            ) =>
        {
            true
        }
        [root, _]
            if matches!(
                root.as_str(),
                "content-units" | "external-modules" | "activity-implementations" | "profiles"
            ) =>
        {
            true
        }
        [root, _, record]
            if root == "profiles"
                && matches!(
                    record.as_str(),
                    "dialogue" | "localization" | "pure" | "content" | "player"
                ) =>
        {
            true
        }
        [root, _, localization, character_names]
            if root == "profiles"
                && localization == "localization"
                && character_names == "character_names" =>
        {
            true
        }
        [root, _, record, _] if root == "profiles" && record == "content" => true,
        [root, _, player, viewport]
            if root == "profiles" && player == "player" && viewport == "viewport" =>
        {
            true
        }
        [root, _, dialogue, policy]
            if root == "profiles" && dialogue == "dialogue" && policy == "inline-failure" =>
        {
            true
        }
        [root, _, dialogue, policy, fallback]
            if root == "profiles"
                && dialogue == "dialogue"
                && policy == "inline-failure"
                && fallback == "fallback" =>
        {
            true
        }
        [root, _, dialogue, policy, fallback, style]
            if root == "profiles"
                && dialogue == "dialogue"
                && policy == "inline-failure"
                && fallback == "fallback"
                && style == "style" =>
        {
            true
        }
        _ => false,
    }
}

fn array_table_path_allowed(path: &[String]) -> bool {
    matches!(
        path,
        [root, _, field] if root == "profiles" && field == "activity-bindings"
    )
}

fn has_unknown_table_prefix(path: &[String], tables: &[Vec<String>]) -> bool {
    tables.iter().any(|table| {
        table.len() < path.len()
            && path.starts_with(table)
            && table
                .first()
                .is_some_and(|root| !is_known_root(root) || !table_path_allowed(table))
    })
}

fn unknown_segment_span(spans: &[SourceSpan], path: &[String]) -> Option<SourceSpan> {
    let known_prefix = known_prefix_len(path);
    spans.get(known_prefix).cloned()
}

fn known_prefix_len(path: &[String]) -> usize {
    match path.first().map(String::as_str) {
        Some("schema" | "default-profile" | "package" | "build") => 1,
        Some("content-units" | "external-modules" | "activity-implementations") => {
            path.len().min(2)
        }
        Some("profiles") => match path.get(2).map(String::as_str) {
            None => path.len().min(2),
            Some("dialogue" | "localization" | "pure" | "content" | "player") => {
                match path.get(3).map(String::as_str) {
                    Some("inline-failure")
                        if path.get(2).is_some_and(|value| value == "dialogue") =>
                    {
                        match path.get(4).map(String::as_str) {
                            Some("fallback") => match path.get(5).map(String::as_str) {
                                Some("style") => 6,
                                _ => 5,
                            },
                            _ => 4,
                        }
                    }
                    Some("viewport") if path.get(2).is_some_and(|value| value == "player") => 4,
                    Some("character_names")
                        if path.get(2).is_some_and(|value| value == "localization") =>
                    {
                        4
                    }
                    Some(_) if path.get(2).is_some_and(|value| value == "content") => 4,
                    _ => 3,
                }
            }
            Some(_) => 2,
        },
        _ => 0,
    }
}

fn known_record_name(path: &[String]) -> &str {
    path.first().map_or("manifest", String::as_str)
}

fn unknown_record_diagnostic(
    path: &[String],
    member_kind: &str,
    primary: SourceSpan,
) -> ManifestDiagnostic {
    if matches!(
        path,
        [profiles, _, dialogue, policy, ..]
            if profiles == "profiles"
                && dialogue == "dialogue"
                && policy == "inline-failure"
    ) {
        return diagnostic(
            ManifestDiagnosticCode::InlinePolicyInvalid,
            format!("dialogue inline-failure contains an unknown {member_kind}"),
            primary,
        );
    }
    diagnostic(
        ManifestDiagnosticCode::UnknownField,
        format!(
            "manifest record `{}` contains an unknown {member_kind}",
            known_record_name(path)
        ),
        primary,
    )
}

fn diagnostic(
    code: ManifestDiagnosticCode,
    message: impl Into<String>,
    primary: SourceSpan,
) -> ManifestDiagnostic {
    ManifestDiagnostic::new(code, message, primary)
}
