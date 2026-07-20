//! Typed value and source-map operations shared by schema record decoders.

use super::index::{IndexedField, IndexedTable};
use crate::{
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestRelatedSpan},
    source_map::{ManifestPath, ManifestSourceKey, ManifestSourceSlot},
    tree_de,
};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use taplo::dom::{FromSyntax, Node, node::DomNode};

pub(super) fn node(field: &IndexedField) -> Node {
    Node::from_syntax(field.value.clone().into())
}

pub(super) fn text(
    field: &IndexedField,
    code: ManifestDiagnosticCode,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<String> {
    let Node::Str(value) = node(field) else {
        diagnostics.push(diagnostic(
            code,
            format!("{expectation} must be a string"),
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    };
    Some(value.value().to_owned())
}

pub(super) fn boolean(
    field: &IndexedField,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<bool> {
    let Node::Bool(value) = node(field) else {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueType,
            format!("{expectation} must be a boolean"),
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    };
    Some(value.value())
}

pub(super) fn typed<T>(
    field: &IndexedField,
    code: ManifestDiagnosticCode,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    tree_de::deserialize_node(node(field)).map_or_else(
        |error| {
            diagnostics.push(diagnostic(
                code,
                format!("{expectation}: {error}"),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

pub(super) fn array_elements(
    document: &SourceDocument,
    field: &IndexedField,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<Vec<(Node, SourceSpan)>> {
    let Node::Array(array) = node(field) else {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueType,
            format!("{expectation} must be an array"),
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    };
    Some(
        array
            .items()
            .read()
            .iter()
            .cloned()
            .map(|node| {
                let span = node.syntax().map_or_else(
                    || field.value_span.clone(),
                    |syntax| syntax_span(document, syntax, diagnostics),
                );
                (node, span)
            })
            .collect(),
    )
}

pub(super) fn node_text(
    node: &Node,
    span: &SourceSpan,
    code: ManifestDiagnosticCode,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<String> {
    let Node::Str(value) = node else {
        diagnostics.push(diagnostic(
            code,
            format!("{expectation} must be a string"),
            span.clone(),
            Vec::new(),
        ));
        return None;
    };
    Some(value.value().to_owned())
}

pub(super) fn typed_node<T>(
    node: Node,
    span: SourceSpan,
    code: ManifestDiagnosticCode,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    tree_de::deserialize_node(node).map_or_else(
        |error| {
            diagnostics.push(diagnostic(
                code,
                format!("{expectation}: {error}"),
                span,
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

pub(super) fn record_field(
    entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    path: ManifestPath,
    field: &IndexedField,
) {
    entries.insert(
        ManifestSourceKey {
            path: path.clone(),
            slot: ManifestSourceSlot::FieldKey,
        },
        field.key_span.clone(),
    );
    entries.insert(
        ManifestSourceKey {
            path,
            slot: ManifestSourceSlot::ScalarValue,
        },
        field.value_span.clone(),
    );
}

pub(super) fn record_table(
    entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    path: ManifestPath,
    table: &IndexedTable,
) {
    entries.insert(
        ManifestSourceKey {
            path,
            slot: ManifestSourceSlot::TableHeader,
        },
        table.header_span.clone(),
    );
}

pub(super) fn record_map_key(
    entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    path: ManifestPath,
    span: SourceSpan,
) {
    entries.insert(
        ManifestSourceKey {
            path,
            slot: ManifestSourceSlot::MapKey,
        },
        span,
    );
}

pub(super) fn record_array_element(
    entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    path: ManifestPath,
    index: u32,
    span: SourceSpan,
) {
    entries.insert(
        ManifestSourceKey {
            path,
            slot: ManifestSourceSlot::ArrayElement { index },
        },
        span,
    );
}

pub(super) fn bounded_array_index(
    index: usize,
    span: &SourceSpan,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<u32> {
    u32::try_from(index).map_or_else(
        |_| {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::ValueType,
                "manifest array has more than u32::MAX addressable elements",
                span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn syntax_span(
    document: &SourceDocument,
    syntax: &taplo::syntax::SyntaxElement,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> SourceSpan {
    let range = syntax.text_range();
    let range = usize::try_from(u32::from(range.start()))
        .ok()
        .zip(usize::try_from(u32::from(range.end())).ok())
        .map(|(start, end)| SourceRange::new(start, end));
    if let Some(range) = range
        && let Ok(span) = document.span(range)
    {
        return span;
    }
    let fallback = document.start_span();
    diagnostics.push(ManifestDiagnostic::new(
        ManifestDiagnosticCode::TomlSyntax,
        "Taplo produced a source range outside the exact UTF-8 document",
        fallback.clone(),
    ));
    fallback
}

pub(super) fn diagnostic(
    code: ManifestDiagnosticCode,
    message: impl Into<String>,
    primary: SourceSpan,
    related: Vec<ManifestRelatedSpan>,
) -> ManifestDiagnostic {
    let message = message.into();
    match ManifestDiagnostic::try_new(code, message.clone(), primary.clone(), related) {
        Ok(diagnostic) => diagnostic,
        Err(_) => ManifestDiagnostic::new(
            code,
            format!("{message}; related span came from a different source revision"),
            primary,
        ),
    }
}
