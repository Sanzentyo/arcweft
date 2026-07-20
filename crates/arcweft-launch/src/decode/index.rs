//! Source-ordered manifest syntax indexing over one Taplo tree.

use crate::diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestRelatedSpan};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};
use std::collections::BTreeMap;
use taplo::{
    dom::{
        FromSyntax, Node,
        node::{DomNode, Key},
    },
    syntax::{SyntaxKind, SyntaxNode, SyntaxToken},
};

#[derive(Clone, Debug)]
pub(super) struct IndexedField {
    pub(super) path_spans: Vec<SourceSpan>,
    pub(super) key_span: SourceSpan,
    pub(super) value_span: SourceSpan,
    pub(super) value: SyntaxNode,
}

#[derive(Clone, Debug)]
pub(super) struct IndexedTable {
    pub(super) path_spans: Vec<SourceSpan>,
    pub(super) header_span: SourceSpan,
}

#[derive(Clone, Debug)]
pub(super) struct IndexedArrayTable {
    pub(super) path_spans: Vec<SourceSpan>,
    pub(super) header_span: SourceSpan,
    pub(super) fields: BTreeMap<Vec<String>, IndexedField>,
    pub(super) tables: BTreeMap<Vec<String>, IndexedTable>,
}

#[derive(Clone, Debug)]
pub(super) struct IndexedInlineArrayItem {
    pub(super) element_span: SourceSpan,
    pub(super) is_table: bool,
    pub(super) fields: BTreeMap<Vec<String>, IndexedField>,
    pub(super) tables: BTreeMap<Vec<String>, IndexedTable>,
}

#[derive(Default)]
pub(super) struct ManifestIndex {
    pub(super) fields: BTreeMap<Vec<String>, IndexedField>,
    pub(super) tables: BTreeMap<Vec<String>, IndexedTable>,
    pub(super) array_tables: BTreeMap<Vec<String>, Vec<IndexedArrayTable>>,
    pub(super) inline_arrays: BTreeMap<Vec<String>, Vec<IndexedInlineArrayItem>>,
    pub(super) diagnostics: Vec<ManifestDiagnostic>,
}

impl ManifestIndex {
    fn record_field(&mut self, path: Vec<String>, field: IndexedField) {
        if let Some(first) = self.array_tables.get(&path).and_then(|items| items.first()) {
            self.diagnostics.push(diagnostic(
                ManifestDiagnosticCode::DuplicateField,
                "manifest array field collides with an already declared table array",
                field.key_span,
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.header_span.clone(),
                )],
            ));
            return;
        }
        if let Some(first) = self.tables.get(&path) {
            let (code, primary, related) = if let Some(key_index) = typed_map_key_index(&path) {
                (
                    ManifestDiagnosticCode::DuplicateMapId,
                    field.path_spans[key_index].clone(),
                    first.path_spans[key_index].clone(),
                )
            } else {
                (
                    ManifestDiagnosticCode::DuplicateTable,
                    field.key_span,
                    first.header_span.clone(),
                )
            };
            self.diagnostics.push(diagnostic(
                code,
                "manifest value collides with an already declared table",
                primary,
                vec![ManifestRelatedSpan::new("first declared here", related)],
            ));
            return;
        }
        if let Some(first) = self.fields.get(&path) {
            let code = if path.len() == 1 {
                ManifestDiagnosticCode::DuplicateRootKey
            } else {
                ManifestDiagnosticCode::DuplicateField
            };
            self.diagnostics.push(diagnostic(
                code,
                "manifest field is declared more than once",
                field.key_span,
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.key_span.clone(),
                )],
            ));
        } else {
            self.fields.insert(path, field);
        }
    }

    fn record_table(&mut self, path: Vec<String>, table: IndexedTable) {
        if let Some(first) = self.fields.get(&path) {
            let (code, primary, related) = if let Some(key_index) = typed_map_key_index(&path) {
                (
                    ManifestDiagnosticCode::DuplicateMapId,
                    table.path_spans[key_index].clone(),
                    first.path_spans[key_index].clone(),
                )
            } else {
                (
                    ManifestDiagnosticCode::DuplicateTable,
                    table.header_span,
                    first.key_span.clone(),
                )
            };
            self.diagnostics.push(diagnostic(
                code,
                "manifest table collides with an already declared value",
                primary,
                vec![ManifestRelatedSpan::new("first declared here", related)],
            ));
            return;
        }
        if let Some(first) = self.tables.get(&path) {
            let (code, primary, related) = if let Some(key_index) = typed_map_key_index(&path) {
                (
                    ManifestDiagnosticCode::DuplicateMapId,
                    table.path_spans[key_index].clone(),
                    first.path_spans[key_index].clone(),
                )
            } else {
                (
                    ManifestDiagnosticCode::DuplicateTable,
                    table.header_span,
                    first.header_span.clone(),
                )
            };
            self.diagnostics.push(diagnostic(
                code,
                "manifest table is declared more than once",
                primary,
                vec![ManifestRelatedSpan::new("first declared here", related)],
            ));
        } else {
            self.tables.insert(path, table);
        }
    }

    fn record_array_table(
        &mut self,
        path: Vec<String>,
        path_spans: Vec<SourceSpan>,
        header_span: SourceSpan,
    ) -> usize {
        if let Some(first) = self.fields.get(&path) {
            self.diagnostics.push(diagnostic(
                ManifestDiagnosticCode::DuplicateField,
                "manifest table array collides with an already declared array field",
                header_span.clone(),
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.key_span.clone(),
                )],
            ));
        } else if let Some(first) = self.tables.get(&path) {
            self.diagnostics.push(diagnostic(
                ManifestDiagnosticCode::DuplicateTable,
                "manifest table array collides with an already declared table",
                header_span.clone(),
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.header_span.clone(),
                )],
            ));
        }
        let items = self.array_tables.entry(path).or_default();
        let index = items.len();
        items.push(IndexedArrayTable {
            path_spans,
            header_span,
            fields: BTreeMap::new(),
            tables: BTreeMap::new(),
        });
        index
    }

    fn record_array_field(
        &mut self,
        array_path: &[String],
        item_index: usize,
        path: Vec<String>,
        field: IndexedField,
    ) {
        let missing_span = field.key_span.clone();
        let Some(item) = self
            .array_tables
            .get_mut(array_path)
            .and_then(|items| items.get_mut(item_index))
        else {
            self.diagnostics.push(diagnostic(
                ManifestDiagnosticCode::TomlSyntax,
                "table-array entry lost its source-order item context",
                missing_span,
                Vec::new(),
            ));
            return;
        };
        if let Some(first) = item.fields.get(&path) {
            self.diagnostics.push(diagnostic(
                ManifestDiagnosticCode::DuplicateField,
                "manifest field is declared more than once",
                field.key_span,
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.key_span.clone(),
                )],
            ));
        } else {
            item.fields.insert(path, field);
        }
    }

    fn record_array_nested_table(
        &mut self,
        array_path: &[String],
        item_index: usize,
        path: Vec<String>,
        table: IndexedTable,
    ) {
        let missing_span = table.header_span.clone();
        let Some(item) = self
            .array_tables
            .get_mut(array_path)
            .and_then(|items| items.get_mut(item_index))
        else {
            self.diagnostics.push(diagnostic(
                ManifestDiagnosticCode::TomlSyntax,
                "nested table lost its source-order table-array item context",
                missing_span,
                Vec::new(),
            ));
            return;
        };
        if let Some(first) = item.tables.get(&path) {
            self.diagnostics.push(diagnostic(
                ManifestDiagnosticCode::DuplicateTable,
                "manifest table is declared more than once",
                table.header_span,
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.header_span.clone(),
                )],
            ));
        } else {
            item.tables.insert(path, table);
        }
    }

    pub(super) fn field(&self, path: &[&str]) -> Option<&IndexedField> {
        self.fields
            .iter()
            .find_map(|(candidate, field)| path_matches(candidate, path).then_some(field))
    }

    pub(super) fn field_by_path(&self, path: &[String]) -> Option<&IndexedField> {
        self.fields.get(path)
    }

    pub(super) fn table(&self, path: &[&str]) -> Option<&IndexedTable> {
        self.tables
            .iter()
            .find_map(|(candidate, table)| path_matches(candidate, path).then_some(table))
    }

    pub(super) fn table_by_path(&self, path: &[String]) -> Option<&IndexedTable> {
        self.tables.get(path)
    }

    pub(super) fn array_items(&self, path: &[String]) -> &[IndexedArrayTable] {
        self.array_tables.get(path).map_or(&[], Vec::as_slice)
    }

    pub(super) fn inline_array_items(&self, path: &[String]) -> &[IndexedInlineArrayItem] {
        self.inline_arrays.get(path).map_or(&[], Vec::as_slice)
    }

    pub(super) fn has_root_occurrence(&self, name: &str) -> bool {
        self.fields
            .keys()
            .any(|path| path.first().is_some_and(|root| root == name))
            || self
                .tables
                .keys()
                .any(|path| path.first().is_some_and(|root| root == name))
            || self
                .array_tables
                .keys()
                .any(|path| path.first().is_some_and(|root| root == name))
    }

    pub(super) fn table_anchor(&self, document: &SourceDocument, name: &str) -> SourceSpan {
        self.table(&[name])
            .map(|table| table.header_span.clone())
            .or_else(|| {
                self.fields.iter().find_map(|(path, field)| {
                    (path.first().is_some_and(|root| root == name))
                        .then(|| field.path_spans[0].clone())
                })
            })
            .unwrap_or_else(|| document.end_span())
    }

    pub(super) fn map_members(&self, root: &str) -> BTreeMap<String, SourceSpan> {
        self.nested_map_members(&[root.to_owned()])
    }

    pub(super) fn nested_map_members(&self, prefix: &[String]) -> BTreeMap<String, SourceSpan> {
        let mut members = BTreeMap::<String, SourceSpan>::new();
        for (path, spans) in self
            .fields
            .iter()
            .map(|(path, field)| (path, &field.path_spans))
            .chain(
                self.tables
                    .iter()
                    .map(|(path, table)| (path, &table.path_spans)),
            )
            .chain(
                self.array_tables
                    .iter()
                    .filter_map(|(path, items)| items.first().map(|item| (path, &item.path_spans))),
            )
        {
            if path.starts_with(prefix)
                && let (Some(member), Some(span)) =
                    (path.get(prefix.len()), spans.get(prefix.len()))
            {
                match members.entry(member.clone()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(span.clone());
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry)
                        if span.range().start() < entry.get().range().start() =>
                    {
                        entry.insert(span.clone());
                    }
                    std::collections::btree_map::Entry::Occupied(_) => {}
                }
            }
        }
        members
    }
}

#[derive(Clone, Debug)]
struct KeySegment {
    value: String,
    span: SourceSpan,
}

enum CurrentTable {
    Regular {
        path: Vec<String>,
        spans: Vec<SourceSpan>,
    },
    Array {
        path: Vec<String>,
        spans: Vec<SourceSpan>,
        item_index: usize,
    },
}

struct ArrayItemContext<'a> {
    path: &'a [String],
    spans: &'a [SourceSpan],
    item_index: usize,
}

#[derive(Clone, Default)]
struct NestedPath {
    segments: Vec<String>,
    spans: Vec<SourceSpan>,
}

pub(super) fn index_document(document: &SourceDocument, root: &SyntaxNode) -> ManifestIndex {
    let mut index = ManifestIndex::default();
    let mut current = CurrentTable::Regular {
        path: Vec::new(),
        spans: Vec::new(),
    };

    for child in root.children() {
        match child.kind() {
            SyntaxKind::TABLE_HEADER => {
                let segments = child
                    .first_child()
                    .map(|key| key_segments(document, &key, &mut index.diagnostics))
                    .unwrap_or_default();
                let path = segments
                    .iter()
                    .map(|segment| segment.value.clone())
                    .collect::<Vec<_>>();
                let spans = segments
                    .iter()
                    .map(|segment| segment.span.clone())
                    .collect::<Vec<_>>();
                let header_span = node_span(document, &child, &mut index.diagnostics);
                index.record_table(
                    path.clone(),
                    IndexedTable {
                        path_spans: spans.clone(),
                        header_span,
                    },
                );
                current = CurrentTable::Regular { path, spans };
            }
            SyntaxKind::TABLE_ARRAY_HEADER => {
                let segments = child
                    .first_child()
                    .map(|key| key_segments(document, &key, &mut index.diagnostics))
                    .unwrap_or_default();
                let path = segments
                    .iter()
                    .map(|segment| segment.value.clone())
                    .collect::<Vec<_>>();
                let spans = segments
                    .iter()
                    .map(|segment| segment.span.clone())
                    .collect::<Vec<_>>();
                let header_span = node_span(document, &child, &mut index.diagnostics);
                let item_index = index.record_array_table(path.clone(), spans.clone(), header_span);
                current = CurrentTable::Array {
                    path,
                    spans,
                    item_index,
                };
            }
            SyntaxKind::ENTRY => match &current {
                CurrentTable::Regular { path, spans } => {
                    index_entry(document, &mut index, path, spans, &child);
                }
                CurrentTable::Array {
                    path,
                    spans,
                    item_index,
                } => {
                    index_array_entry(document, &mut index, path, spans, *item_index, &child);
                }
            },
            _ => {}
        }
    }

    index
}

fn index_entry(
    document: &SourceDocument,
    index: &mut ManifestIndex,
    parent_path: &[String],
    parent_spans: &[SourceSpan],
    entry: &SyntaxNode,
) {
    let Some((key_segments, value)) = entry_parts(document, entry, &mut index.diagnostics) else {
        return;
    };
    let mut path = parent_path.to_vec();
    path.extend(key_segments.iter().map(|segment| segment.value.clone()));
    let mut path_spans = parent_spans.to_vec();
    path_spans.extend(key_segments.iter().map(|segment| segment.span.clone()));
    let Some(last_segment) = key_segments.last() else {
        return;
    };
    let key_span = last_segment.span.clone();

    if let Some(inline_table) = value
        .children()
        .find(|child| child.kind() == SyntaxKind::INLINE_TABLE)
    {
        index.record_table(
            path.clone(),
            IndexedTable {
                path_spans: path_spans.clone(),
                header_span: key_span,
            },
        );
        for nested in inline_table
            .children()
            .filter(|child| child.kind() == SyntaxKind::ENTRY)
        {
            index_entry(document, index, &path, &path_spans, &nested);
        }
        return;
    }

    if let Some(array) = value
        .children()
        .find(|child| child.kind() == SyntaxKind::ARRAY)
    {
        index_inline_array(document, index, &path, &array);
    }

    let value_span = semantic_value_span(document, &value, &mut index.diagnostics);
    index.record_field(
        path,
        IndexedField {
            path_spans,
            key_span,
            value_span,
            value,
        },
    );
}

fn index_inline_array(
    document: &SourceDocument,
    index: &mut ManifestIndex,
    path: &[String],
    array: &SyntaxNode,
) {
    let items = array
        .children()
        .filter(|child| child.kind() == SyntaxKind::VALUE)
        .map(|value| {
            let inline_table = value
                .children()
                .find(|child| child.kind() == SyntaxKind::INLINE_TABLE);
            let element_span = node_span(document, &value, &mut index.diagnostics);
            let mut item = IndexedInlineArrayItem {
                element_span,
                is_table: inline_table.is_some(),
                fields: BTreeMap::new(),
                tables: BTreeMap::new(),
            };
            if let Some(table) = inline_table {
                for entry in table
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::ENTRY)
                {
                    index_inline_array_entry(
                        document,
                        &mut index.diagnostics,
                        &mut item,
                        NestedPath::default(),
                        &entry,
                    );
                }
            }
            item
        })
        .collect::<Vec<_>>();
    index.inline_arrays.entry(path.to_vec()).or_insert(items);
}

fn index_inline_array_entry(
    document: &SourceDocument,
    diagnostics: &mut Vec<ManifestDiagnostic>,
    item: &mut IndexedInlineArrayItem,
    mut parent: NestedPath,
    entry: &SyntaxNode,
) {
    let Some((segments, value)) = entry_parts(document, entry, diagnostics) else {
        return;
    };
    parent
        .segments
        .extend(segments.iter().map(|segment| segment.value.clone()));
    parent
        .spans
        .extend(segments.iter().map(|segment| segment.span.clone()));
    let Some(last_segment) = segments.last() else {
        return;
    };
    let key_span = last_segment.span.clone();
    if let Some(table) = value
        .children()
        .find(|child| child.kind() == SyntaxKind::INLINE_TABLE)
    {
        let indexed = IndexedTable {
            path_spans: parent.spans.clone(),
            header_span: key_span,
        };
        if let Some(first) = item.tables.get(&parent.segments) {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::DuplicateTable,
                "manifest table is declared more than once",
                indexed.header_span,
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.header_span.clone(),
                )],
            ));
        } else {
            item.tables.insert(parent.segments.clone(), indexed);
        }
        for nested in table
            .children()
            .filter(|child| child.kind() == SyntaxKind::ENTRY)
        {
            index_inline_array_entry(document, diagnostics, item, parent.clone(), &nested);
        }
        return;
    }

    let value_span = semantic_value_span(document, &value, diagnostics);
    let indexed = IndexedField {
        path_spans: parent.spans.clone(),
        key_span,
        value_span,
        value,
    };
    if let Some(first) = item.fields.get(&parent.segments) {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::DuplicateField,
            "manifest field is declared more than once",
            indexed.key_span,
            vec![ManifestRelatedSpan::new(
                "first declared here",
                first.key_span.clone(),
            )],
        ));
    } else {
        item.fields.insert(parent.segments, indexed);
    }
}

fn index_array_entry(
    document: &SourceDocument,
    index: &mut ManifestIndex,
    array_path: &[String],
    array_spans: &[SourceSpan],
    item_index: usize,
    entry: &SyntaxNode,
) {
    let Some((key_segments, value)) = entry_parts(document, entry, &mut index.diagnostics) else {
        return;
    };
    let context = ArrayItemContext {
        path: array_path,
        spans: array_spans,
        item_index,
    };
    index_array_entry_parts(
        document,
        index,
        &context,
        NestedPath::default(),
        &key_segments,
        value,
    );
}

fn index_array_entry_parts(
    document: &SourceDocument,
    index: &mut ManifestIndex,
    context: &ArrayItemContext<'_>,
    mut parent: NestedPath,
    key_segments: &[KeySegment],
    value: SyntaxNode,
) {
    parent
        .segments
        .extend(key_segments.iter().map(|segment| segment.value.clone()));
    parent
        .spans
        .extend(key_segments.iter().map(|segment| segment.span.clone()));
    let mut path_spans = context.spans.to_vec();
    path_spans.extend(parent.spans.iter().cloned());
    let Some(last_segment) = key_segments.last() else {
        return;
    };
    let key_span = last_segment.span.clone();

    if let Some(inline_table) = value
        .children()
        .find(|child| child.kind() == SyntaxKind::INLINE_TABLE)
    {
        index.record_array_nested_table(
            context.path,
            context.item_index,
            parent.segments.clone(),
            IndexedTable {
                path_spans: path_spans.clone(),
                header_span: key_span,
            },
        );
        for nested in inline_table
            .children()
            .filter(|child| child.kind() == SyntaxKind::ENTRY)
        {
            let Some((segments, nested_value)) =
                entry_parts(document, &nested, &mut index.diagnostics)
            else {
                continue;
            };
            index_array_entry_parts(
                document,
                index,
                context,
                parent.clone(),
                &segments,
                nested_value,
            );
        }
        return;
    }

    let value_span = semantic_value_span(document, &value, &mut index.diagnostics);
    index.record_array_field(
        context.path,
        context.item_index,
        parent.segments,
        IndexedField {
            path_spans,
            key_span,
            value_span,
            value,
        },
    );
}

fn entry_parts(
    document: &SourceDocument,
    entry: &SyntaxNode,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<(Vec<KeySegment>, SyntaxNode)> {
    let key = entry
        .children()
        .find(|child| child.kind() == SyntaxKind::KEY)?;
    let value = entry
        .children()
        .find(|child| child.kind() == SyntaxKind::VALUE)?;
    let segments = key_segments(document, &key, diagnostics);
    (!segments.is_empty()).then_some((segments, value))
}

fn key_segments(
    document: &SourceDocument,
    key: &SyntaxNode,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Vec<KeySegment> {
    key.children_with_tokens()
        .filter_map(taplo::rowan::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::IDENT)
        .map(|token| {
            let value = Key::from_syntax(token.clone().into()).value().to_owned();
            KeySegment {
                value,
                span: token_span(document, &token, diagnostics),
            }
        })
        .collect()
}

fn typed_map_key_index(path: &[String]) -> Option<usize> {
    if path.len() == 2
        && path.first().is_some_and(|root| {
            matches!(
                root.as_str(),
                "content-units" | "external-modules" | "activity-implementations" | "profiles"
            )
        })
    {
        Some(1)
    } else if matches!(
        path,
        [root, _, content, _] if root == "profiles" && content == "content"
    ) {
        Some(3)
    } else {
        None
    }
}

fn path_matches(candidate: &[String], expected: &[&str]) -> bool {
    candidate.len() == expected.len()
        && candidate
            .iter()
            .zip(expected)
            .all(|(candidate, expected)| candidate == expected)
}

fn diagnostic(
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

fn checked_span(
    document: &SourceDocument,
    start: u32,
    end: u32,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> SourceSpan {
    let range = usize::try_from(start)
        .ok()
        .zip(usize::try_from(end).ok())
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

fn semantic_value_span(
    document: &SourceDocument,
    value: &SyntaxNode,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> SourceSpan {
    let node = Node::from_syntax(value.clone().into());
    let Some(syntax) = node.syntax() else {
        return node_span(document, value, diagnostics);
    };
    let range = syntax.text_range();
    checked_span(
        document,
        u32::from(range.start()),
        u32::from(range.end()),
        diagnostics,
    )
}

pub(super) fn node_span(
    document: &SourceDocument,
    node: &SyntaxNode,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> SourceSpan {
    let range = node.text_range();
    checked_span(
        document,
        u32::from(range.start()),
        u32::from(range.end()),
        diagnostics,
    )
}

fn token_span(
    document: &SourceDocument,
    token: &SyntaxToken,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> SourceSpan {
    let range = token.text_range();
    checked_span(
        document,
        u32::from(range.start()),
        u32::from(range.end()),
        diagnostics,
    )
}
