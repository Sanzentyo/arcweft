//! Player policy and physical viewport decoding.

use super::{ProfileContext, positive_u32, profile_path, record_optional_profile_table};
use crate::{
    LaunchPlayerViewportFit,
    decode::{index::ManifestIndex, value},
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode},
    manifest::{LaunchPlayerProfileSpec, LaunchPlayerViewportSpec},
    source_map::{
        ManifestPath, ManifestPathSegment, ManifestSourceKey, PlayerField, ProfileField,
        ViewportField,
    },
};
use arcweft_manifest_model::ProfileId;
use arcweft_source::SourceSpan;
use std::collections::BTreeMap;

use super::super::append;

pub(super) fn decode_player(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> LaunchPlayerProfileSpec {
    let base = append(context.base, "player");
    if let Some(field) = index.field_by_path(&base) {
        super::record_profile_field(source_entries, context.id, ProfileField::Player, field);
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::ValueType,
            "profile player policy must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return LaunchPlayerProfileSpec::default();
    }
    if record_optional_profile_table(index, context, &base, ProfileField::Player, source_entries)
        .is_none()
    {
        return LaunchPlayerProfileSpec::default();
    }
    let viewport = decode_viewport(index, context, &base, source_entries, diagnostics);
    LaunchPlayerProfileSpec { viewport }
}

fn decode_viewport(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    player_base: &[String],
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<LaunchPlayerViewportSpec> {
    let base = append(player_base, "viewport");
    if let Some(field) = index.field_by_path(&base) {
        value::record_field(
            source_entries,
            player_path(
                context.id,
                [ManifestPathSegment::Player(PlayerField::Viewport)],
            ),
            field,
        );
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::ValueType,
            "player viewport must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    }

    let table = index.table_by_path(&base);
    let has_nested = index
        .fields
        .keys()
        .any(|path| path.starts_with(&base) && path.len() > base.len())
        || index
            .tables
            .keys()
            .any(|path| path.starts_with(&base) && path.len() > base.len());
    if table.is_none() && !has_nested {
        return None;
    }
    if let Some(table) = table {
        value::record_table(
            source_entries,
            player_path(
                context.id,
                [ManifestPathSegment::Player(PlayerField::Viewport)],
            ),
            table,
        );
    }

    let design_width = decode_optional_viewport_dimension(
        index,
        context.id,
        &base,
        ViewportField::DesignWidth,
        "design-width",
        source_entries,
        diagnostics,
    );
    let design_height = decode_optional_viewport_dimension(
        index,
        context.id,
        &base,
        ViewportField::DesignHeight,
        "design-height",
        source_entries,
        diagnostics,
    );
    let fit = index
        .field_by_path(&append(&base, "fit"))
        .and_then(|field| {
            value::record_field(
                source_entries,
                viewport_path(
                    context.id,
                    [ManifestPathSegment::Viewport(ViewportField::Fit)],
                ),
                field,
            );
            value::typed(
                field,
                ManifestDiagnosticCode::EnumInvalid,
                "player viewport fit",
                diagnostics,
            )
        })
        .unwrap_or_default();
    if fit == LaunchPlayerViewportFit::Raw && (design_width.is_some() || design_height.is_some()) {
        let primary = index
            .field_by_path(&append(&base, "design-width"))
            .or_else(|| index.field_by_path(&append(&base, "design-height")))
            .map(|field| field.value_span.clone())
            .or_else(|| table.map(|table| table.header_span.clone()))
            .unwrap_or_else(|| context.anchor.clone());
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::PlayerViewportInvalid,
            "raw viewport fit cannot declare design dimensions",
            primary,
            Vec::new(),
        ));
    }
    Some(LaunchPlayerViewportSpec {
        design_width,
        design_height,
        fit,
    })
}

fn decode_optional_viewport_dimension(
    index: &ManifestIndex,
    profile_id: &ProfileId,
    base: &[String],
    source_field: ViewportField,
    field_name: &str,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<std::num::NonZeroU32> {
    let field = index.field_by_path(&append(base, field_name))?;
    value::record_field(
        source_entries,
        viewport_path(profile_id, [ManifestPathSegment::Viewport(source_field)]),
        field,
    );
    positive_u32(
        field,
        "player viewport dimension",
        ManifestDiagnosticCode::PlayerViewportInvalid,
        diagnostics,
    )
}

fn player_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    profile_path(
        profile_id,
        std::iter::once(ManifestPathSegment::ProfileField(ProfileField::Player)).chain(tail),
    )
}

fn viewport_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    player_path(
        profile_id,
        std::iter::once(ManifestPathSegment::Player(PlayerField::Viewport)).chain(tail),
    )
}
