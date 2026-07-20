//! Profile-specific content residency, placement, and compression policies.

use super::{ProfileContext, profile_path, record_optional_profile_table, record_profile_field};
use crate::{
    decode::{index::ManifestIndex, value},
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode},
    source_map::{
        ManifestPath, ManifestPathSegment, ManifestSourceKey, ProfileContentField, ProfileField,
    },
};
use arcweft_manifest_model::{
    ContentCompression, ContentPlacement, ContentResidency, ContentUnitId, ProfileContentSpec,
    ProfileId,
};
use arcweft_source::SourceSpan;
use std::collections::BTreeMap;

use super::super::{append, reject_scalar_member, required_field};

pub(super) fn decode_profile_content(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ContentUnitId, ProfileContentSpec> {
    let base = append(context.base, "content");
    if let Some(field) = index.field_by_path(&base) {
        record_profile_field(source_entries, context.id, ProfileField::Content, field);
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::ValueType,
            "profile content must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return BTreeMap::new();
    }
    if record_optional_profile_table(index, context, &base, ProfileField::Content, source_entries)
        .is_none()
    {
        return BTreeMap::new();
    }

    let mut accepted = BTreeMap::new();
    for (raw_id, raw_span) in index.nested_map_members(&base) {
        let Ok(id) = ContentUnitId::new(raw_id.as_str()) else {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("profile content unit ID `{raw_id}` is invalid"),
                raw_span,
                Vec::new(),
            ));
            continue;
        };
        let member_base = append(&base, &raw_id);
        value::record_map_key(
            source_entries,
            profile_content_path(context.id, &id, []),
            raw_span.clone(),
        );
        if let Some(table) = index.table_by_path(&member_base) {
            value::record_table(
                source_entries,
                profile_content_path(context.id, &id, []),
                table,
            );
        }
        if reject_scalar_member(index, &member_base, "profile content policy", diagnostics) {
            continue;
        }
        let content_context = ProfileContentContext {
            profile_id: context.id,
            content_id: &id,
            base: &member_base,
            anchor: &raw_span,
        };
        let residency = decode_required_content_enum::<ContentResidency>(
            index,
            content_context,
            ProfileContentValueField {
                source_field: ProfileContentField::Residency,
                name: "residency",
            },
            source_entries,
            diagnostics,
        );
        let placement = decode_required_content_enum::<ContentPlacement>(
            index,
            content_context,
            ProfileContentValueField {
                source_field: ProfileContentField::Placement,
                name: "placement",
            },
            source_entries,
            diagnostics,
        );
        let compression = decode_required_content_enum::<ContentCompression>(
            index,
            content_context,
            ProfileContentValueField {
                source_field: ProfileContentField::Compression,
                name: "compression",
            },
            source_entries,
            diagnostics,
        );
        if let (Some(residency), Some(placement), Some(compression)) =
            (residency, placement, compression)
        {
            accepted.insert(
                id,
                ProfileContentSpec {
                    residency,
                    placement,
                    compression,
                },
            );
        }
    }
    accepted
}

fn decode_required_content_enum<T>(
    index: &ManifestIndex,
    context: ProfileContentContext<'_>,
    spec: ProfileContentValueField,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let field = required_field(
        index,
        &append(context.base, spec.name),
        context.anchor,
        spec.name,
        diagnostics,
    )?;
    value::record_field(
        source_entries,
        profile_content_path(
            context.profile_id,
            context.content_id,
            [ManifestPathSegment::ProfileContentField(spec.source_field)],
        ),
        field,
    );
    value::typed(
        field,
        ManifestDiagnosticCode::EnumInvalid,
        spec.name,
        diagnostics,
    )
}

fn profile_content_path(
    profile_id: &ProfileId,
    content_id: &ContentUnitId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    profile_path(
        profile_id,
        [
            ManifestPathSegment::ProfileField(ProfileField::Content),
            ManifestPathSegment::ProfileContent(content_id.clone()),
        ]
        .into_iter()
        .chain(tail),
    )
}

#[derive(Clone, Copy)]
struct ProfileContentContext<'a> {
    profile_id: &'a ProfileId,
    content_id: &'a ContentUnitId,
    base: &'a [String],
    anchor: &'a SourceSpan,
}

#[derive(Clone, Copy)]
struct ProfileContentValueField {
    source_field: ProfileContentField,
    name: &'static str,
}
