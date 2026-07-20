//! Profile-local external module selection and Activity implementation bindings.

use super::{ProfileContext, profile_path, record_profile_field};
use crate::{
    decode::{
        index::{IndexedArrayTable, IndexedField, IndexedInlineArrayItem, ManifestIndex},
        value,
    },
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestRelatedSpan},
    source_map::{ActivityBindingField, ManifestPathSegment, ManifestSourceKey, ProfileField},
};
use arcweft_manifest_model::{
    ActivityBindingSpec, ActivityId, ActivityImplementationId, ExternalModuleImportId, ProfileId,
};
use arcweft_source::{SourceDocument, SourceSpan};
use std::collections::BTreeMap;

use super::super::append;

pub(super) fn decode_external_module_selection(
    document: &SourceDocument,
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Vec<ExternalModuleImportId> {
    let raw_path = append(context.base, "external-modules");
    let Some(field) = index.field_by_path(&raw_path) else {
        return Vec::new();
    };
    record_profile_field(
        source_entries,
        context.id,
        ProfileField::ExternalModules,
        field,
    );
    let Some(elements) =
        value::array_elements(document, field, "profile external-modules", diagnostics)
    else {
        return Vec::new();
    };
    let mut seen = BTreeMap::<ExternalModuleImportId, SourceSpan>::new();
    let mut selected = Vec::new();
    for (element_index, (node, span)) in elements.into_iter().enumerate() {
        let Some(source_index) = value::bounded_array_index(element_index, &span, diagnostics)
        else {
            continue;
        };
        value::record_array_element(
            source_entries,
            profile_path(
                context.id,
                [
                    ManifestPathSegment::ProfileField(ProfileField::ExternalModules),
                    ManifestPathSegment::Index(source_index),
                ],
            ),
            source_index,
            span.clone(),
        );
        let Some(raw) = value::node_text(
            &node,
            &span,
            ManifestDiagnosticCode::IdInvalid,
            "external module selection",
            diagnostics,
        ) else {
            continue;
        };
        let Ok(id) = ExternalModuleImportId::new(raw.as_str()) else {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("external module import ID `{raw}` is invalid"),
                span,
                Vec::new(),
            ));
            continue;
        };
        if let Some(first) = seen.get(&id) {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::DuplicateArrayId,
                format!("external module `{id}` is selected more than once"),
                span,
                vec![ManifestRelatedSpan::new(
                    "first declared here",
                    first.clone(),
                )],
            ));
        } else {
            seen.insert(id.clone(), span);
            selected.push(id);
        }
    }
    selected
}

// The source-order loop intentionally keeps duplicate detection and source-map
// publication adjacent to typed binding construction.
#[allow(clippy::too_many_lines)]
pub(super) fn decode_activity_bindings(
    document: &SourceDocument,
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Vec<ActivityBindingSpec> {
    let raw_path = append(context.base, "activity-bindings");
    let mut items = Vec::<BindingItem<'_>>::new();
    if let Some(field) = index.field_by_path(&raw_path) {
        record_profile_field(
            source_entries,
            context.id,
            ProfileField::ActivityBindings,
            field,
        );
        if value::array_elements(document, field, "profile activity-bindings", diagnostics)
            .is_some()
        {
            items.extend(
                index
                    .inline_array_items(&raw_path)
                    .iter()
                    .map(BindingItem::Inline),
            );
        }
    }
    items.extend(index.array_items(&raw_path).iter().map(BindingItem::Table));

    let mut accepted = Vec::new();
    let mut seen = BTreeMap::<ActivityId, SourceSpan>::new();
    for (binding_index, item) in items.into_iter().enumerate() {
        let element_span = item.element_span().clone();
        let Some(source_index) =
            value::bounded_array_index(binding_index, &element_span, diagnostics)
        else {
            continue;
        };
        value::record_array_element(
            source_entries,
            profile_path(
                context.id,
                [
                    ManifestPathSegment::ProfileField(ProfileField::ActivityBindings),
                    ManifestPathSegment::ActivityBinding(source_index),
                ],
            ),
            source_index,
            element_span.clone(),
        );
        if !item.is_table() {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::ValueType,
                "Activity binding array element must be a table",
                element_span,
                Vec::new(),
            ));
            continue;
        }
        let activity_field = item.field("activity");
        let activity_span = activity_field.map(|field| field.value_span.clone());
        let activity = decode_binding_id(
            activity_field,
            BindingValueContext {
                profile_id: context.id,
                binding_index: source_index,
                element_span: &element_span,
                source_field: ActivityBindingField::Activity,
                expectation: "Activity binding activity",
            },
            source_entries,
            diagnostics,
            ActivityId::new,
        );
        let implementation = decode_binding_id(
            item.field("implementation"),
            BindingValueContext {
                profile_id: context.id,
                binding_index: source_index,
                element_span: &element_span,
                source_field: ActivityBindingField::Implementation,
                expectation: "Activity binding implementation",
            },
            source_entries,
            diagnostics,
            ActivityImplementationId::new,
        );
        if let (Some(activity), Some(implementation)) = (activity, implementation) {
            let Some(activity_span) = activity_span else {
                continue;
            };
            if let Some(first) = seen.get(&activity) {
                diagnostics.push(value::diagnostic(
                    ManifestDiagnosticCode::DuplicateActivityBinding,
                    format!("Activity `{activity}` is bound more than once"),
                    activity_span,
                    vec![ManifestRelatedSpan::new(
                        "first declared here",
                        first.clone(),
                    )],
                ));
            } else {
                seen.insert(activity.clone(), activity_span);
                accepted.push(ActivityBindingSpec {
                    activity,
                    implementation,
                });
            }
        }
    }
    accepted
}

fn decode_binding_id<T, E>(
    field: Option<&IndexedField>,
    context: BindingValueContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
    parse: impl FnOnce(String) -> Result<T, E>,
) -> Option<T> {
    let field = field.or_else(|| {
        diagnostics.push(value::diagnostic(
            ManifestDiagnosticCode::ValueMissing,
            format!("{} is required", context.expectation),
            context.element_span.clone(),
            Vec::new(),
        ));
        None
    })?;
    value::record_field(
        source_entries,
        profile_path(
            context.profile_id,
            [
                ManifestPathSegment::ProfileField(ProfileField::ActivityBindings),
                ManifestPathSegment::ActivityBinding(context.binding_index),
                ManifestPathSegment::ActivityBindingField(context.source_field),
            ],
        ),
        field,
    );
    let raw = value::text(
        field,
        ManifestDiagnosticCode::IdInvalid,
        context.expectation,
        diagnostics,
    )?;
    parse(raw).map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("{} is invalid", context.expectation),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

enum BindingItem<'a> {
    Inline(&'a IndexedInlineArrayItem),
    Table(&'a IndexedArrayTable),
}

impl BindingItem<'_> {
    fn element_span(&self) -> &SourceSpan {
        match self {
            Self::Inline(item) => &item.element_span,
            Self::Table(item) => &item.header_span,
        }
    }

    fn is_table(&self) -> bool {
        match self {
            Self::Inline(item) => item.is_table,
            Self::Table(_) => true,
        }
    }

    fn field(&self, name: &str) -> Option<&IndexedField> {
        let path = [name.to_owned()];
        match self {
            Self::Inline(item) => item.fields.get(path.as_slice()),
            Self::Table(item) => item.fields.get(path.as_slice()),
        }
    }
}

#[derive(Clone, Copy)]
struct BindingValueContext<'a> {
    profile_id: &'a ProfileId,
    binding_index: u32,
    element_span: &'a SourceSpan,
    source_field: ActivityBindingField,
    expectation: &'static str,
}
