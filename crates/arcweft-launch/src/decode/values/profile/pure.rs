//! Profile-local pure execution policy decoding.

use super::{ProfileContext, positive_u32, profile_path, record_optional_profile_table};
use crate::{
    LaunchMathBackend, LaunchPureBackend,
    decode::{index::ManifestIndex, value},
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode},
    manifest::{LaunchPureProfileSpec, LaunchPureWorkers},
    source_map::{ManifestPath, ManifestPathSegment, ManifestSourceKey, ProfileField, PureField},
};
use arcweft_manifest_model::ProfileId;
use arcweft_source::SourceSpan;
use std::collections::BTreeMap;
use taplo::dom::Node;

use super::super::append;

pub(super) fn decode_pure(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<LaunchPureProfileSpec> {
    let base = append(context.base, "pure");
    if let Some(field) = index.field_by_path(&base) {
        super::record_profile_field(source_entries, context.id, ProfileField::Pure, field);
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::ValueType,
            "profile pure policy must be a table",
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    }
    record_optional_profile_table(index, context, &base, ProfileField::Pure, source_entries)?;
    let pure_context = PureContext {
        profile_id: context.id,
        base: &base,
    };
    let backend = decode_optional_pure_enum::<LaunchPureBackend>(
        index,
        pure_context,
        PureValueField {
            source_field: PureField::Backend,
            name: "backend",
            expectation: "pure backend",
        },
        source_entries,
        diagnostics,
    );
    let math_backend = decode_optional_pure_enum::<LaunchMathBackend>(
        index,
        pure_context,
        PureValueField {
            source_field: PureField::MathBackend,
            name: "math-backend",
            expectation: "pure math backend",
        },
        source_entries,
        diagnostics,
    );
    let math_wgpu_min_elements = decode_optional_positive_u32(
        index,
        pure_context,
        PureValueField {
            source_field: PureField::MathWgpuMinElements,
            name: "math-wgpu-min-elements",
            expectation: "pure math-wgpu-min-elements",
        },
        source_entries,
        diagnostics,
    );
    let workers = decode_workers(index, context, &base, source_entries, diagnostics);
    let batch_min_len = decode_optional_positive_u32(
        index,
        pure_context,
        PureValueField {
            source_field: PureField::BatchMinLen,
            name: "batch-min-len",
            expectation: "pure batch-min-len",
        },
        source_entries,
        diagnostics,
    );
    let object_artifacts = index
        .field_by_path(&append(&base, "object-artifacts"))
        .and_then(|field| {
            record_pure_field(
                source_entries,
                context.id,
                PureField::ObjectArtifacts,
                field,
            );
            value::boolean(field, "pure object-artifacts", diagnostics)
        });
    Some(LaunchPureProfileSpec {
        backend,
        math_backend,
        math_wgpu_min_elements,
        workers,
        batch_min_len,
        object_artifacts,
    })
}

fn decode_optional_pure_enum<T>(
    index: &ManifestIndex,
    context: PureContext<'_>,
    spec: PureValueField,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let field = index.field_by_path(&append(context.base, spec.name))?;
    record_pure_field(source_entries, context.profile_id, spec.source_field, field);
    value::typed(
        field,
        ManifestDiagnosticCode::EnumInvalid,
        spec.expectation,
        diagnostics,
    )
}

fn decode_optional_positive_u32(
    index: &ManifestIndex,
    context: PureContext<'_>,
    spec: PureValueField,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<std::num::NonZeroU32> {
    let field = index.field_by_path(&append(context.base, spec.name))?;
    record_pure_field(source_entries, context.profile_id, spec.source_field, field);
    positive_u32(
        field,
        spec.expectation,
        ManifestDiagnosticCode::PureThresholdInvalid,
        diagnostics,
    )
}

fn decode_workers(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    base: &[String],
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<LaunchPureWorkers> {
    let field = index.field_by_path(&append(base, "workers"))?;
    record_pure_field(source_entries, context.id, PureField::Workers, field);
    match value::node(field) {
        Node::Str(value) if value.value() == "auto" => Some(LaunchPureWorkers::Auto),
        Node::Integer(_) => positive_u32(
            field,
            "pure workers",
            ManifestDiagnosticCode::PureWorkersInvalid,
            diagnostics,
        )
        .map(LaunchPureWorkers::Count),
        _ => {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::PureWorkersInvalid,
                "pure workers must be `auto` or a positive u32 integer",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        }
    }
}

fn record_pure_field(
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    profile_id: &ProfileId,
    source_field: PureField,
    field: &crate::decode::index::IndexedField,
) {
    value::record_field(
        source_entries,
        pure_path(profile_id, [ManifestPathSegment::Pure(source_field)]),
        field,
    );
}

fn pure_path(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    profile_path(
        profile_id,
        std::iter::once(ManifestPathSegment::ProfileField(ProfileField::Pure)).chain(tail),
    )
}

#[derive(Clone, Copy)]
struct PureContext<'a> {
    profile_id: &'a ProfileId,
    base: &'a [String],
}

#[derive(Clone, Copy)]
struct PureValueField {
    source_field: PureField,
    name: &'static str,
    expectation: &'static str,
}
