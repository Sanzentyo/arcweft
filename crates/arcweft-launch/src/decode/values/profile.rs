//! Strict profile records and retained launch policies.

use super::{append, record_root_table, reject_scalar_member, reject_scalar_root, required_field};
use crate::{
    decode::{
        index::{IndexedField, ManifestIndex},
        value,
    },
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode},
    manifest::{LaunchListenAddress, ProfileSpec},
    source_map::{
        ManifestPath, ManifestPathSegment, ManifestRootField, ManifestSourceKey, ProfileField,
    },
};
use arcweft_manifest_model::{
    AdapterProfileId, EntityIdRef, LaunchKind, NormalizedProjectPath, ProfileId,
};
use arcweft_source::{SourceDocument, SourceSpan};
use std::{collections::BTreeMap, num::NonZeroU32};
use taplo::dom::{Node, node::IntegerValue};

mod content;
mod external;
mod player;
mod policy;
mod pure;

pub(super) fn decode_profiles(
    document: &SourceDocument,
    index: &ManifestIndex,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ProfileId, ProfileSpec> {
    record_root_table(
        index,
        source_entries,
        "profiles",
        ManifestRootField::Profiles,
    );
    if reject_scalar_root(index, "profiles", diagnostics) {
        return BTreeMap::new();
    }

    let mut accepted = BTreeMap::new();
    for (raw_id, raw_span) in index.map_members("profiles") {
        let Ok(id) = ProfileId::new(raw_id.as_str()) else {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::IdInvalid,
                format!("profile ID `{raw_id}` is invalid"),
                raw_span,
                Vec::new(),
            ));
            continue;
        };
        let base = vec!["profiles".to_owned(), raw_id];
        let typed_base = profile_path(&id, []);
        value::record_map_key(source_entries, typed_base.clone(), raw_span.clone());
        if let Some(table) = index.table_by_path(&base) {
            value::record_table(source_entries, typed_base, table);
        }
        if reject_scalar_member(index, &base, "profile", diagnostics) {
            continue;
        }

        let context = ProfileContext {
            id: &id,
            base: &base,
            anchor: &raw_span,
        };
        if let Some(profile) = decode_profile(document, index, context, source_entries, diagnostics)
        {
            accepted.insert(id, profile);
        }
    }
    accepted
}

fn decode_profile(
    document: &SourceDocument,
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<ProfileSpec> {
    let kind = decode_required_profile_enum::<LaunchKind>(
        index,
        context,
        ProfileValueField {
            name: "kind",
            source_field: ProfileField::Kind,
            expectation: "profile kind",
        },
        source_entries,
        diagnostics,
    );
    let source = decode_required_profile_path(index, context, source_entries, diagnostics);
    let entry = decode_optional_profile_id(
        index,
        context,
        ProfileValueField {
            name: "entry",
            source_field: ProfileField::Entry,
            expectation: "profile entry",
        },
        ManifestDiagnosticCode::EntityRefInvalid,
        source_entries,
        diagnostics,
        EntityIdRef::new,
    );
    let adapter = decode_optional_profile_id(
        index,
        context,
        ProfileValueField {
            name: "adapter",
            source_field: ProfileField::Adapter,
            expectation: "profile adapter",
        },
        ManifestDiagnosticCode::IdInvalid,
        source_entries,
        diagnostics,
        AdapterProfileId::new,
    );
    let external_modules = external::decode_external_module_selection(
        document,
        index,
        context,
        source_entries,
        diagnostics,
    );
    let activity_bindings =
        external::decode_activity_bindings(document, index, context, source_entries, diagnostics);
    let dialogue = policy::decode_dialogue(
        document,
        index,
        context.id,
        context.base,
        context.anchor,
        source_entries,
        diagnostics,
    );
    let listen = decode_listen(index, context, source_entries, diagnostics);
    let pure = pure::decode_pure(index, context, source_entries, diagnostics);
    let content_policies =
        content::decode_profile_content(index, context, source_entries, diagnostics);
    let player = player::decode_player(index, context, source_entries, diagnostics);

    kind.zip(source).map(|(kind, source)| ProfileSpec {
        kind,
        source,
        entry,
        adapter,
        external_modules,
        activity_bindings,
        dialogue,
        listen,
        pure,
        content: content_policies,
        player,
    })
}

fn decode_required_profile_path(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<NormalizedProjectPath> {
    let path = append(context.base, "source");
    let field = required_field(index, &path, context.anchor, "profile source", diagnostics)?;
    record_profile_field(source_entries, context.id, ProfileField::Source, field);
    let raw = value::text(
        field,
        ManifestDiagnosticCode::PathInvalid,
        "profile source",
        diagnostics,
    )?;
    NormalizedProjectPath::new(raw).map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::PathInvalid,
                "profile source is not a normalized project path",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_required_profile_enum<T>(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    spec: ProfileValueField,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let path = append(context.base, spec.name);
    let field = required_field(index, &path, context.anchor, spec.expectation, diagnostics)?;
    record_profile_field(source_entries, context.id, spec.source_field, field);
    value::typed(
        field,
        ManifestDiagnosticCode::EnumInvalid,
        spec.expectation,
        diagnostics,
    )
}

fn decode_optional_profile_id<T, E>(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    spec: ProfileValueField,
    code: ManifestDiagnosticCode,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
    parse: impl FnOnce(String) -> Result<T, E>,
) -> Option<T> {
    let field = index.field_by_path(&append(context.base, spec.name))?;
    record_profile_field(source_entries, context.id, spec.source_field, field);
    let raw = value::text(field, code, spec.expectation, diagnostics)?;
    parse(raw).map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                code,
                format!("{} is invalid", spec.expectation),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn decode_listen(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<LaunchListenAddress> {
    let field = index.field_by_path(&append(context.base, "listen"))?;
    record_profile_field(source_entries, context.id, ProfileField::Listen, field);
    let raw = value::text(
        field,
        ManifestDiagnosticCode::ListenInvalid,
        "profile listen address",
        diagnostics,
    )?;
    LaunchListenAddress::parse(&raw).map_or_else(
        |_| {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::ListenInvalid,
                "profile listen must be a numeric socket address",
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn positive_u32(
    field: &IndexedField,
    expectation: &str,
    code: ManifestDiagnosticCode,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<NonZeroU32> {
    let Node::Integer(value) = value::node(field) else {
        diagnostics.push(value::diagnostic(
            code,
            format!("{expectation} must be a positive u32 integer"),
            field.value_span.clone(),
            Vec::new(),
        ));
        return None;
    };
    positive_integer(value.value(), field, expectation, code, diagnostics)
}

fn positive_integer(
    value: IntegerValue,
    field: &IndexedField,
    expectation: &str,
    code: ManifestDiagnosticCode,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<NonZeroU32> {
    let count = match value {
        IntegerValue::Positive(value) => u32::try_from(value).ok().and_then(NonZeroU32::new),
        IntegerValue::Negative(_) => None,
    };
    if count.is_none() {
        diagnostics.push(value::diagnostic(
            code,
            format!("{expectation} must be in 1..=u32::MAX"),
            field.value_span.clone(),
            Vec::new(),
        ));
    }
    count
}

fn record_optional_profile_table(
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    base: &[String],
    source_field: ProfileField,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
) -> Option<SourceSpan> {
    if let Some(table) = index.table_by_path(base) {
        value::record_table(
            source_entries,
            profile_path(
                context.id,
                [ManifestPathSegment::ProfileField(source_field)],
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
    has_nested.then(|| context.anchor.clone())
}

fn record_profile_field(
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    profile_id: &ProfileId,
    source_field: ProfileField,
    field: &IndexedField,
) {
    value::record_field(
        source_entries,
        profile_path(
            profile_id,
            [ManifestPathSegment::ProfileField(source_field)],
        ),
        field,
    );
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

#[derive(Clone, Copy)]
struct ProfileContext<'a> {
    id: &'a ProfileId,
    base: &'a [String],
    anchor: &'a SourceSpan,
}

#[derive(Clone, Copy)]
struct ProfileValueField {
    name: &'static str,
    source_field: ProfileField,
    expectation: &'static str,
}
