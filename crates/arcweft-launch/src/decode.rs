//! The single Taplo syntax-tree entry used by the final manifest decoder.

mod index;
mod shape;
#[cfg(test)]
mod tests;
mod value;
mod values;

use self::index::{IndexedField, ManifestIndex, index_document};
use self::shape::validate_known_shape;
use self::values::decode_sections;
use crate::{
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestRelatedSpan, ManifestReport},
    manifest::ArcweftManifestDocument,
    source_map::{
        BuildField, ManifestPath, ManifestPathSegment, ManifestRootField, ManifestSourceKey,
        ManifestSourceMap, ManifestSourceSlot, PackageField,
    },
};
use arcweft_manifest_model::{
    BuildSpec, ManifestSchemaVersion, NormalizedProjectPath, PackageId, PackageSpec, PackageVersion,
};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};
use std::{collections::BTreeMap, sync::Arc};
use taplo::{
    dom::{FromSyntax, Node},
    parser,
};

/// Private accepted output until the atomic reader switch publishes its final surface.
#[derive(Clone, Debug)]
pub(crate) struct DecodedManifest {
    pub(crate) manifest: ArcweftManifestDocument,
    pub(crate) source_map: ManifestSourceMap,
}

/// Decodes schema/package/build facts directly from one Taplo syntax tree.
///
/// The decoder remains crate-private until the same atomic change migrates all
/// consumers and deletes both historical readers.
pub(crate) fn decode(document: Arc<SourceDocument>) -> Result<DecodedManifest, ManifestReport> {
    let parsed = parser::parse(document.text());
    let mut syntax_diagnostics = parsed.errors.iter().map(|error| {
        diagnostic(
            ManifestDiagnosticCode::TomlSyntax,
            error.message.clone(),
            parser_error_span(&document, error),
            Vec::new(),
        )
    });
    if let Some(first) = syntax_diagnostics.next() {
        return Err(ManifestReport::from_first(first, syntax_diagnostics));
    }

    let syntax = parsed.into_syntax();
    let mut index = index_document(&document, &syntax);
    validate_known_shape(&mut index);
    let mut diagnostics = std::mem::take(&mut index.diagnostics);

    let mut source_entries = BTreeMap::new();
    let schema = decode_schema(&document, &index, &mut source_entries, &mut diagnostics);
    let package = decode_package(&document, &index, &mut source_entries, &mut diagnostics);
    let build = decode_build(&index, &mut source_entries, &mut diagnostics);
    let resource_type_manifest =
        decode_resource_type_manifest_path(&index, &mut source_entries, &mut diagnostics);
    let sections = decode_sections(&document, &index, &mut source_entries, &mut diagnostics);

    let mut diagnostics = diagnostics.into_iter();
    if let Some(first) = diagnostics.next() {
        return Err(ManifestReport::from_first(first, diagnostics));
    }

    let Some(schema) = schema else {
        return Err(ManifestReport::single(ManifestDiagnostic::new(
            ManifestDiagnosticCode::SchemaMissing,
            "manifest schema was not materialized",
            document.start_span(),
        )));
    };
    let Some(package) = package else {
        return Err(ManifestReport::single(ManifestDiagnostic::new(
            ManifestDiagnosticCode::RequiredPackage,
            "manifest package was not materialized",
            document.end_span(),
        )));
    };

    let source_map_document = Arc::clone(&document);
    let source_map = ManifestSourceMap::try_new(document, source_entries).map_err(|_| {
        ManifestReport::single(ManifestDiagnostic::new(
            ManifestDiagnosticCode::TomlSyntax,
            "manifest source map contains a span from a different source revision",
            source_map_document.start_span(),
        ))
    })?;
    if !Arc::ptr_eq(&source_map_document, source_map.document()) {
        return Err(ManifestReport::single(ManifestDiagnostic::new(
            ManifestDiagnosticCode::TomlSyntax,
            "manifest source map does not retain the decoded source document",
            source_map_document.start_span(),
        )));
    }

    Ok(DecodedManifest {
        manifest: ArcweftManifestDocument {
            schema,
            package,
            build,
            resource_type_manifest,
            content_units: sections.content_units,
            external_modules: sections.external_modules,
            activity_implementations: sections.activity_implementations,
            default_profile: sections.default_profile,
            profiles: sections.profiles,
        },
        source_map,
    })
}

fn decode_resource_type_manifest_path(
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<NormalizedProjectPath> {
    let field = index.field(&["resource-type-manifest"])?;
    record_field_source(
        source_entries,
        ManifestPath::new([ManifestPathSegment::Root(
            ManifestRootField::ResourceTypeManifest,
        )]),
        field,
    );
    let Node::Str(value) = Node::from_syntax(field.value.clone().into()) else {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueType,
            "resource-type-manifest must be a string",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    };
    NormalizedProjectPath::new(value.value()).map_or_else(
        |_| {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::PathInvalid,
                "resource-type-manifest path is invalid",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_schema(
    document: &SourceDocument,
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<ManifestSchemaVersion> {
    let Some(field) = index.field(&["schema"]) else {
        if !index.has_root_occurrence("schema") {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::SchemaMissing,
                "manifest schema is required",
                document.start_span(),
                Vec::new(),
            ));
        }
        return None;
    };

    record_field_source(
        source_entries,
        ManifestPath::new([ManifestPathSegment::Root(ManifestRootField::Schema)]),
        field,
    );
    match Node::from_syntax(field.value.clone().into()) {
        Node::Integer(value) if value.value().as_positive() == Some(1) => {
            Some(ManifestSchemaVersion::V1)
        }
        Node::Integer(_) => {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::SchemaUnsupported,
                "manifest schema must be exactly 1",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        }
        _ => {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::ValueType,
                "manifest schema must be an integer",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        }
    }
}

fn decode_package(
    document: &SourceDocument,
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<PackageSpec> {
    if let Some(field) = index.field(&["package"]) {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueType,
            "package must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    }
    if !index.has_root_occurrence("package") {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::RequiredPackage,
            "package is required",
            document.end_span(),
            Vec::new(),
        ));
        return None;
    }
    if let Some(table) = index.table(&["package"]) {
        source_entries.insert(
            ManifestSourceKey {
                path: ManifestPath::new([ManifestPathSegment::Root(ManifestRootField::Package)]),
                slot: ManifestSourceSlot::TableHeader,
            },
            table.header_span.clone(),
        );
    }

    let anchor = index.table_anchor(document, "package");
    let id = index.field(&["package", "id"]).and_then(|field| {
        record_field_source(
            source_entries,
            ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::Package),
                ManifestPathSegment::Package(PackageField::Id),
            ]),
            field,
        );
        decode_package_id(field, diagnostics)
    });
    if index.field(&["package", "id"]).is_none() {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueMissing,
            "package id is required",
            anchor.clone(),
            Vec::new(),
        ));
    }

    let version = index.field(&["package", "version"]).and_then(|field| {
        record_field_source(
            source_entries,
            ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::Package),
                ManifestPathSegment::Package(PackageField::Version),
            ]),
            field,
        );
        decode_package_version(field, diagnostics)
    });
    if index.field(&["package", "version"]).is_none() {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueMissing,
            "package version is required",
            anchor,
            Vec::new(),
        ));
    }

    id.zip(version)
        .map(|(id, version)| PackageSpec { id, version })
}

fn decode_package_id(
    field: &IndexedField,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<PackageId> {
    let Node::Str(value) = Node::from_syntax(field.value.clone().into()) else {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueType,
            "package id must be a string",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    };
    PackageId::new(value.value()).map_or_else(
        |_| {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                "package id is invalid",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_package_version(
    field: &IndexedField,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<PackageVersion> {
    let Node::Str(value) = Node::from_syntax(field.value.clone().into()) else {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueType,
            "package version must be a string",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    };
    PackageVersion::new(value.value()).map_or_else(
        |_| {
            diagnostics.push(diagnostic(
                ManifestDiagnosticCode::VersionInvalid,
                "package version must be an exact semantic version",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_build(
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BuildSpec {
    if let Some(field) = index.field(&["build"]) {
        diagnostics.push(diagnostic(
            ManifestDiagnosticCode::ValueType,
            "build must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return BuildSpec::default();
    }
    if let Some(table) = index.table(&["build"]) {
        source_entries.insert(
            ManifestSourceKey {
                path: ManifestPath::new([ManifestPathSegment::Root(ManifestRootField::Build)]),
                slot: ManifestSourceSlot::TableHeader,
            },
            table.header_span.clone(),
        );
    }

    let mut build = BuildSpec::default();
    if let Some(field) = index.field(&["build", "source-dir"]) {
        record_field_source(
            source_entries,
            ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::Build),
                ManifestPathSegment::Build(BuildField::SourceDir),
            ]),
            field,
        );
        match Node::from_syntax(field.value.clone().into()) {
            Node::Str(value) => match NormalizedProjectPath::new(value.value()) {
                Ok(path) => build.source_dir = path,
                Err(_) => diagnostics.push(diagnostic(
                    ManifestDiagnosticCode::PathInvalid,
                    "build source-dir is invalid",
                    field.value_span.clone(),
                    Vec::new(),
                )),
            },
            _ => diagnostics.push(diagnostic(
                ManifestDiagnosticCode::ValueType,
                "build source-dir must be a string",
                field.value_span.clone(),
                Vec::new(),
            )),
        }
    }
    if let Some(field) = index.field(&["build", "target-dir"]) {
        record_field_source(
            source_entries,
            ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::Build),
                ManifestPathSegment::Build(BuildField::TargetDir),
            ]),
            field,
        );
        match Node::from_syntax(field.value.clone().into()) {
            Node::Str(value) => match NormalizedProjectPath::new(value.value()) {
                Ok(path) => build.target_dir = path,
                Err(_) => diagnostics.push(diagnostic(
                    ManifestDiagnosticCode::PathInvalid,
                    "build target-dir is invalid",
                    field.value_span.clone(),
                    Vec::new(),
                )),
            },
            _ => diagnostics.push(diagnostic(
                ManifestDiagnosticCode::ValueType,
                "build target-dir must be a string",
                field.value_span.clone(),
                Vec::new(),
            )),
        }
    }
    if let Some(field) = index.field(&["build", "incremental"]) {
        record_field_source(
            source_entries,
            ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::Build),
                ManifestPathSegment::Build(BuildField::Incremental),
            ]),
            field,
        );
        match Node::from_syntax(field.value.clone().into()) {
            Node::Bool(value) => build.incremental = value.value(),
            _ => diagnostics.push(diagnostic(
                ManifestDiagnosticCode::ValueType,
                "build incremental must be a boolean",
                field.value_span.clone(),
                Vec::new(),
            )),
        }
    }
    build
}

fn record_field_source(
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

fn parser_error_span(document: &SourceDocument, error: &parser::Error) -> SourceSpan {
    let range = usize::try_from(u32::from(error.range.start()))
        .ok()
        .zip(usize::try_from(u32::from(error.range.end())).ok())
        .map(|(start, end)| SourceRange::new(start, end));
    range
        .and_then(|range| document.span(range).ok())
        .unwrap_or_else(|| document.start_span())
}
