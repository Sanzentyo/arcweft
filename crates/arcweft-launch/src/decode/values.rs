//! Typed schema-1 record decoding after source-order shape validation.

use super::{
    index::{IndexedField, ManifestIndex},
    value,
};
use crate::{
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode},
    manifest::ProfileSpec,
    source_map::{
        ActivityImplementationField, ContentUnitField, ExternalModuleField, ManifestPath,
        ManifestPathSegment, ManifestRootField, ManifestSourceKey,
    },
};
use arcweft_manifest_model::{
    ActivityImplementationId, ActivityImplementationSpec, AdapterExportId, AdapterFamily,
    ContentRootRef, ContentUnitId, ContentUnitSpec, DependencyDemand, EntityIdRef,
    ExternalModuleId, ExternalModuleImportId, ExternalModuleImportSpec, ManifestVisibility,
    ModuleMountPath, NonEmptyVec, NormalizedProjectPath, PackageId, PackageVersion, ProfileId,
    RawDigest, SemanticDigest,
};
use arcweft_source::{SourceDocument, SourceSpan};
use std::{collections::BTreeMap, str::FromStr};

mod profile;

pub(super) struct DecodedSections {
    pub(super) content_units: BTreeMap<ContentUnitId, ContentUnitSpec>,
    pub(super) external_modules: BTreeMap<ExternalModuleImportId, ExternalModuleImportSpec>,
    pub(super) activity_implementations:
        BTreeMap<ActivityImplementationId, ActivityImplementationSpec>,
    pub(super) default_profile: Option<ProfileId>,
    pub(super) profiles: BTreeMap<ProfileId, ProfileSpec>,
}

pub(super) fn decode_sections(
    document: &SourceDocument,
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> DecodedSections {
    DecodedSections {
        content_units: decode_content_units(document, index, source_entries, diagnostics),
        external_modules: decode_external_modules(index, source_entries, diagnostics),
        activity_implementations: decode_activity_implementations(
            index,
            source_entries,
            diagnostics,
        ),
        default_profile: decode_default_profile(index, source_entries, diagnostics),
        profiles: profile::decode_profiles(document, index, source_entries, diagnostics),
    }
}

fn decode_content_units(
    document: &SourceDocument,
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ContentUnitId, ContentUnitSpec> {
    record_root_table(
        index,
        source_entries,
        "content-units",
        ManifestRootField::ContentUnits,
    );
    if reject_scalar_root(index, "content-units", diagnostics) {
        return BTreeMap::new();
    }

    let mut accepted = BTreeMap::new();
    for (raw_id, raw_span) in index.map_members("content-units") {
        let Ok(id) = ContentUnitId::new(raw_id.as_str()) else {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("content unit ID `{raw_id}` is invalid"),
                raw_span,
                Vec::new(),
            ));
            continue;
        };
        let base = vec!["content-units".to_owned(), raw_id];
        let typed_base = ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::ContentUnits),
            ManifestPathSegment::ContentUnit(id.clone()),
        ]);
        value::record_map_key(source_entries, typed_base.clone(), raw_span.clone());
        if let Some(table) = index.table_by_path(&base) {
            value::record_table(source_entries, typed_base.clone(), table);
        }
        if reject_scalar_member(index, &base, "content unit", diagnostics) {
            continue;
        }

        let roots_path = append(&base, "roots");
        let visibility_path = append(&base, "visibility");
        let demand_path = append(&base, "demand");
        let roots = required_field(
            index,
            &roots_path,
            &raw_span,
            "content unit roots",
            diagnostics,
        )
        .and_then(|field| {
            record_content_field(source_entries, &id, ContentUnitField::Roots, field);
            decode_content_roots(document, &id, field, source_entries, diagnostics)
        });
        let visibility = required_field(
            index,
            &visibility_path,
            &raw_span,
            "content unit visibility",
            diagnostics,
        )
        .and_then(|field| {
            record_content_field(source_entries, &id, ContentUnitField::Visibility, field);
            value::typed(
                field,
                ManifestDiagnosticCode::EnumInvalid,
                "content unit visibility",
                diagnostics,
            )
        });
        let demand = required_field(
            index,
            &demand_path,
            &raw_span,
            "content unit demand",
            diagnostics,
        )
        .and_then(|field| {
            record_content_field(source_entries, &id, ContentUnitField::Demand, field);
            value::typed(
                field,
                ManifestDiagnosticCode::EnumInvalid,
                "content unit demand",
                diagnostics,
            )
        });

        if let (Some(roots), Some(visibility), Some(demand)) = (roots, visibility, demand) {
            accepted.insert(
                id,
                ContentUnitSpec {
                    roots,
                    visibility,
                    demand,
                },
            );
        }
    }
    accepted
}

fn decode_content_roots(
    document: &SourceDocument,
    id: &ContentUnitId,
    field: &IndexedField,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<NonEmptyVec<ContentRootRef>> {
    let elements = value::array_elements(document, field, "content unit roots", diagnostics)?;
    if elements.is_empty() {
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::ValueType,
            "content unit roots must not be empty",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    }
    let expected_count = elements.len();

    let mut roots = Vec::with_capacity(elements.len());
    for (element_index, (node, span)) in elements.into_iter().enumerate() {
        let Some(source_index) = value::bounded_array_index(element_index, &span, diagnostics)
        else {
            continue;
        };
        value::record_array_element(
            source_entries,
            ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::ContentUnits),
                ManifestPathSegment::ContentUnit(id.clone()),
                ManifestPathSegment::ContentUnitField(ContentUnitField::Roots),
                ManifestPathSegment::Index(source_index),
            ]),
            source_index,
            span.clone(),
        );
        let Some(raw) = value::node_text(
            &node,
            &span,
            ManifestDiagnosticCode::EntityRefInvalid,
            "content root",
            diagnostics,
        ) else {
            continue;
        };
        match EntityIdRef::new(raw) {
            Ok(reference) => roots.push(ContentRootRef(reference)),
            Err(message) => diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::EntityRefInvalid,
                message,
                span,
                Vec::new(),
            )),
        }
    }
    if roots.len() != expected_count {
        return None;
    }
    NonEmptyVec::new(roots)
}

// Every required import pin is decoded together so a partially admitted import
// cannot escape this one record boundary.
#[allow(clippy::too_many_lines)]
fn decode_external_modules(
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ExternalModuleImportId, ExternalModuleImportSpec> {
    record_root_table(
        index,
        source_entries,
        "external-modules",
        ManifestRootField::ExternalModules,
    );
    if reject_scalar_root(index, "external-modules", diagnostics) {
        return BTreeMap::new();
    }

    let mut accepted = BTreeMap::new();
    for (raw_id, raw_span) in index.map_members("external-modules") {
        let Ok(id) = ExternalModuleImportId::new(raw_id.as_str()) else {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("external module import ID `{raw_id}` is invalid"),
                raw_span,
                Vec::new(),
            ));
            continue;
        };
        let base = vec!["external-modules".to_owned(), raw_id];
        let typed_base = ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::ExternalModules),
            ManifestPathSegment::ExternalModule(id.clone()),
        ]);
        value::record_map_key(source_entries, typed_base.clone(), raw_span.clone());
        if let Some(table) = index.table_by_path(&base) {
            value::record_table(source_entries, typed_base, table);
        }
        if reject_scalar_member(index, &base, "external module import", diagnostics) {
            continue;
        }

        let context = ExternalRecordContext {
            base: &base,
            anchor: &raw_span,
            id: &id,
        };
        let mount = decode_required_string_field(
            index,
            context,
            ExternalStringField {
                name: "mount",
                source_field: ExternalModuleField::Mount,
                code: ManifestDiagnosticCode::IdInvalid,
                expectation: "external module mount",
            },
            diagnostics,
            source_entries,
            ModuleMountPath::new,
        );
        let metadata = decode_required_string_field(
            index,
            context,
            ExternalStringField {
                name: "metadata",
                source_field: ExternalModuleField::Metadata,
                code: ManifestDiagnosticCode::PathInvalid,
                expectation: "external module metadata path",
            },
            diagnostics,
            source_entries,
            NormalizedProjectPath::new,
        );
        let metadata_hash = decode_required_digest::<RawDigest>(
            index,
            context,
            ExternalValueField {
                name: "metadata-hash",
                source_field: ExternalModuleField::MetadataHash,
            },
            diagnostics,
            source_entries,
        );
        let expected_package = decode_required_string_field(
            index,
            context,
            ExternalStringField {
                name: "expected-package",
                source_field: ExternalModuleField::ExpectedPackage,
                code: ManifestDiagnosticCode::IdInvalid,
                expectation: "external module expected package",
            },
            diagnostics,
            source_entries,
            PackageId::new,
        );
        let expected_version = decode_required_string_field(
            index,
            context,
            ExternalStringField {
                name: "expected-version",
                source_field: ExternalModuleField::ExpectedVersion,
                code: ManifestDiagnosticCode::VersionInvalid,
                expectation: "external module expected version",
            },
            diagnostics,
            source_entries,
            PackageVersion::new,
        );
        let expected_module = decode_required_string_field(
            index,
            context,
            ExternalStringField {
                name: "expected-module",
                source_field: ExternalModuleField::ExpectedModule,
                code: ManifestDiagnosticCode::IdInvalid,
                expectation: "external module expected module",
            },
            diagnostics,
            source_entries,
            ExternalModuleId::new,
        );
        let expected_family = decode_required_external_enum::<AdapterFamily>(
            index,
            context,
            ExternalEnumField {
                name: "expected-family",
                source_field: ExternalModuleField::ExpectedFamily,
                expectation: "external module expected family",
            },
            diagnostics,
            source_entries,
        );
        let expected_abi_hash = decode_required_digest::<SemanticDigest>(
            index,
            context,
            ExternalValueField {
                name: "expected-abi-hash",
                source_field: ExternalModuleField::ExpectedAbiHash,
            },
            diagnostics,
            source_entries,
        );
        let visibility = decode_required_external_enum::<ManifestVisibility>(
            index,
            context,
            ExternalEnumField {
                name: "visibility",
                source_field: ExternalModuleField::Visibility,
                expectation: "external module visibility",
            },
            diagnostics,
            source_entries,
        );
        let demand = decode_required_external_enum::<DependencyDemand>(
            index,
            context,
            ExternalEnumField {
                name: "demand",
                source_field: ExternalModuleField::Demand,
                expectation: "external module demand",
            },
            diagnostics,
            source_entries,
        );

        if let (
            Some(mount),
            Some(metadata),
            Some(metadata_hash),
            Some(expected_package),
            Some(expected_version),
            Some(expected_module),
            Some(expected_family),
            Some(expected_abi_hash),
            Some(visibility),
            Some(demand),
        ) = (
            mount,
            metadata,
            metadata_hash,
            expected_package,
            expected_version,
            expected_module,
            expected_family,
            expected_abi_hash,
            visibility,
            demand,
        ) {
            accepted.insert(
                id,
                ExternalModuleImportSpec {
                    mount,
                    metadata,
                    metadata_hash,
                    expected_package,
                    expected_version,
                    expected_module,
                    expected_family,
                    expected_abi_hash,
                    visibility,
                    demand,
                },
            );
        }
    }
    accepted
}

fn decode_activity_implementations(
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ActivityImplementationId, ActivityImplementationSpec> {
    record_root_table(
        index,
        source_entries,
        "activity-implementations",
        ManifestRootField::ActivityImplementations,
    );
    if reject_scalar_root(index, "activity-implementations", diagnostics) {
        return BTreeMap::new();
    }

    let mut accepted = BTreeMap::new();
    for (raw_id, raw_span) in index.map_members("activity-implementations") {
        let Ok(id) = ActivityImplementationId::new(raw_id.as_str()) else {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("Activity implementation ID `{raw_id}` is invalid"),
                raw_span,
                Vec::new(),
            ));
            continue;
        };
        let base = vec!["activity-implementations".to_owned(), raw_id];
        let typed_base = ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::ActivityImplementations),
            ManifestPathSegment::ActivityImplementation(id.clone()),
        ]);
        value::record_map_key(source_entries, typed_base.clone(), raw_span.clone());
        if let Some(table) = index.table_by_path(&base) {
            value::record_table(source_entries, typed_base, table);
        }
        if reject_scalar_member(index, &base, "Activity implementation", diagnostics) {
            continue;
        }

        let context = ActivityImplementationContext {
            base: &base,
            anchor: &raw_span,
            id: &id,
        };
        let module = decode_required_activity_id(
            index,
            context,
            ActivityImplementationStringField {
                name: "module",
                source_field: ActivityImplementationField::Module,
            },
            diagnostics,
            source_entries,
            ExternalModuleImportId::new,
        );
        let export = decode_required_activity_id(
            index,
            context,
            ActivityImplementationStringField {
                name: "export",
                source_field: ActivityImplementationField::Export,
            },
            diagnostics,
            source_entries,
            AdapterExportId::new,
        );
        if let (Some(module), Some(export)) = (module, export) {
            accepted.insert(id, ActivityImplementationSpec { module, export });
        }
    }
    accepted
}

fn decode_default_profile(
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<ProfileId> {
    let field = index.field(&["default-profile"])?;
    value::record_field(
        source_entries,
        ManifestPath::new([ManifestPathSegment::Root(ManifestRootField::DefaultProfile)]),
        field,
    );
    let raw = value::text(
        field,
        ManifestDiagnosticCode::ValueType,
        "default-profile",
        diagnostics,
    )?;
    ProfileId::new(raw.as_str()).map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("default profile ID `{raw}` is invalid"),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_required_external_enum<T>(
    index: &ManifestIndex,
    context: ExternalRecordContext<'_>,
    spec: ExternalEnumField,
    diagnostics: &mut Vec<ManifestDiagnostic>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let path = append(context.base, spec.name);
    let field = required_field(index, &path, context.anchor, spec.expectation, diagnostics)?;
    record_external_field(source_entries, context.id, spec.source_field, field);
    value::typed(
        field,
        ManifestDiagnosticCode::EnumInvalid,
        spec.expectation,
        diagnostics,
    )
}

fn decode_required_digest<T>(
    index: &ManifestIndex,
    context: ExternalRecordContext<'_>,
    spec: ExternalValueField,
    diagnostics: &mut Vec<ManifestDiagnostic>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
) -> Option<T>
where
    T: FromStr,
{
    let path = append(context.base, spec.name);
    let field = required_field(index, &path, context.anchor, spec.name, diagnostics)?;
    record_external_field(source_entries, context.id, spec.source_field, field);
    let raw = value::text(
        field,
        ManifestDiagnosticCode::DigestInvalid,
        spec.name,
        diagnostics,
    )?;
    raw.parse().map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::DigestInvalid,
                format!("{} is not a canonical BLAKE3 digest", spec.name),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_required_string_field<T, E>(
    index: &ManifestIndex,
    context: ExternalRecordContext<'_>,
    spec: ExternalStringField,
    diagnostics: &mut Vec<ManifestDiagnostic>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    parse: impl FnOnce(String) -> Result<T, E>,
) -> Option<T> {
    let path = append(context.base, spec.name);
    let field = required_field(index, &path, context.anchor, spec.expectation, diagnostics)?;
    record_external_field(source_entries, context.id, spec.source_field, field);
    let raw = value::text(field, spec.code, spec.expectation, diagnostics)?;
    parse(raw).map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                spec.code,
                format!("{} is invalid", spec.expectation),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_required_activity_id<T, E>(
    index: &ManifestIndex,
    context: ActivityImplementationContext<'_>,
    spec: ActivityImplementationStringField,
    diagnostics: &mut Vec<ManifestDiagnostic>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    parse: impl FnOnce(String) -> Result<T, E>,
) -> Option<T> {
    let path = append(context.base, spec.name);
    let field = required_field(index, &path, context.anchor, spec.name, diagnostics)?;
    value::record_field(
        source_entries,
        ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::ActivityImplementations),
            ManifestPathSegment::ActivityImplementation(context.id.clone()),
            ManifestPathSegment::ActivityImplementationField(spec.source_field),
        ]),
        field,
    );
    let raw = value::text(
        field,
        ManifestDiagnosticCode::IdInvalid,
        spec.name,
        diagnostics,
    )?;
    parse(raw).map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("{} is invalid", spec.name),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn required_field<'index>(
    index: &'index ManifestIndex,
    path: &[String],
    anchor: &SourceSpan,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<&'index IndexedField> {
    index.field_by_path(path).or_else(|| {
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::ValueMissing,
            format!("{expectation} is required"),
            anchor.clone(),
            Vec::new(),
        ));
        None
    })
}

fn reject_scalar_root(
    index: &ManifestIndex,
    root: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> bool {
    let Some(field) = index.field(&[root]) else {
        return false;
    };
    diagnostics.push(value::diagnostic(
        ManifestDiagnosticCode::ValueType,
        format!("{root} must be a table"),
        field.value_span.clone(),
        Vec::new(),
    ));
    true
}

fn reject_scalar_member(
    index: &ManifestIndex,
    base: &[String],
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> bool {
    let Some(field) = index.field_by_path(base) else {
        return false;
    };
    diagnostics.push(value::diagnostic(
        ManifestDiagnosticCode::ValueType,
        format!("{expectation} must be a table"),
        field.value_span.clone(),
        Vec::new(),
    ));
    true
}

fn record_root_table(
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    raw_root: &str,
    root: ManifestRootField,
) {
    if let Some(table) = index.table(&[raw_root]) {
        value::record_table(
            source_entries,
            ManifestPath::new([ManifestPathSegment::Root(root)]),
            table,
        );
    }
}

fn record_content_field(
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    id: &ContentUnitId,
    source_field: ContentUnitField,
    field: &IndexedField,
) {
    value::record_field(
        source_entries,
        ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::ContentUnits),
            ManifestPathSegment::ContentUnit(id.clone()),
            ManifestPathSegment::ContentUnitField(source_field),
        ]),
        field,
    );
}

fn record_external_field(
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    id: &ExternalModuleImportId,
    source_field: ExternalModuleField,
    field: &IndexedField,
) {
    value::record_field(
        source_entries,
        ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::ExternalModules),
            ManifestPathSegment::ExternalModule(id.clone()),
            ManifestPathSegment::ExternalModuleField(source_field),
        ]),
        field,
    );
}

fn append(base: &[String], segment: &str) -> Vec<String> {
    let mut path = base.to_vec();
    path.push(segment.to_owned());
    path
}

#[derive(Clone, Copy)]
struct ExternalRecordContext<'a> {
    base: &'a [String],
    anchor: &'a SourceSpan,
    id: &'a ExternalModuleImportId,
}

#[derive(Clone, Copy)]
struct ExternalStringField {
    name: &'static str,
    source_field: ExternalModuleField,
    code: ManifestDiagnosticCode,
    expectation: &'static str,
}

#[derive(Clone, Copy)]
struct ExternalEnumField {
    name: &'static str,
    source_field: ExternalModuleField,
    expectation: &'static str,
}

#[derive(Clone, Copy)]
struct ExternalValueField {
    name: &'static str,
    source_field: ExternalModuleField,
}

#[derive(Clone, Copy)]
struct ActivityImplementationContext<'a> {
    base: &'a [String],
    anchor: &'a SourceSpan,
    id: &'a ActivityImplementationId,
}

#[derive(Clone, Copy)]
struct ActivityImplementationStringField {
    name: &'static str,
    source_field: ActivityImplementationField,
}
