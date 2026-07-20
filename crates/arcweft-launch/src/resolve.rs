//! Pure manifest-to-profile selection and cross-reference lowering.

use crate::{
    LaunchProfileSelection,
    accepted::SourceBackedManifest,
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestReport},
    manifest::{LaunchListenAddress, LaunchPlayerProfileSpec, LaunchPureProfileSpec, ProfileSpec},
    source_map::{
        ActivityBindingField, ActivityImplementationField, ManifestPath, ManifestPathSegment,
        ManifestRootField, ManifestSourceKey, ManifestSourceSlot, ProfileField,
    },
};
use arcweft_dialogue::DialoguePresentationProfile;
use arcweft_manifest_model::{
    ActivityId, ActivityImplementationId, ActivityImplementationSpec, AdapterProfileId,
    ContentUnitId, ContentUnitSpec, EntityIdRef, ExternalModuleImportId, ExternalModuleImportSpec,
    LaunchKind, NormalizedProjectPath, ProfileContentSpec, ProfileId,
};
use arcweft_source::SourceSpan;
use arcweft_view::ViewId;
use std::collections::BTreeMap;

/// One Activity binding resolved to its exact selected implementation facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedActivityBinding {
    implementation_id: ActivityImplementationId,
    implementation: ActivityImplementationSpec,
}

/// One profile content policy joined with its declared content unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProfileContent {
    unit: ContentUnitSpec,
    policy: ProfileContentSpec,
}

/// Self-contained, Sans-I/O profile selection from one accepted manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLaunchProfile {
    id: ProfileId,
    kind: LaunchKind,
    source: NormalizedProjectPath,
    entry: Option<EntityIdRef>,
    adapter: AdapterProfileId,
    external_modules: BTreeMap<ExternalModuleImportId, ExternalModuleImportSpec>,
    activity_bindings: BTreeMap<ActivityId, ResolvedActivityBinding>,
    dialogue: DialoguePresentationProfile,
    listen: Option<LaunchListenAddress>,
    pure: Option<LaunchPureProfileSpec>,
    content: BTreeMap<ContentUnitId, ResolvedProfileContent>,
    player: LaunchPlayerProfileSpec,
}

impl ResolvedActivityBinding {
    pub const fn implementation_id(&self) -> &ActivityImplementationId {
        &self.implementation_id
    }

    pub const fn implementation(&self) -> &ActivityImplementationSpec {
        &self.implementation
    }
}

impl ResolvedProfileContent {
    pub const fn unit(&self) -> &ContentUnitSpec {
        &self.unit
    }

    pub const fn policy(&self) -> &ProfileContentSpec {
        &self.policy
    }
}

impl ResolvedLaunchProfile {
    pub const fn id(&self) -> &ProfileId {
        &self.id
    }

    pub const fn kind(&self) -> LaunchKind {
        self.kind
    }

    pub const fn source(&self) -> &NormalizedProjectPath {
        &self.source
    }

    pub const fn entry(&self) -> Option<&EntityIdRef> {
        self.entry.as_ref()
    }

    pub const fn adapter(&self) -> &AdapterProfileId {
        &self.adapter
    }

    pub const fn external_modules(
        &self,
    ) -> &BTreeMap<ExternalModuleImportId, ExternalModuleImportSpec> {
        &self.external_modules
    }

    pub const fn activity_bindings(&self) -> &BTreeMap<ActivityId, ResolvedActivityBinding> {
        &self.activity_bindings
    }

    pub const fn dialogue(&self) -> &DialoguePresentationProfile {
        &self.dialogue
    }

    pub const fn listen(&self) -> Option<LaunchListenAddress> {
        self.listen
    }

    pub const fn pure(&self) -> Option<&LaunchPureProfileSpec> {
        self.pure.as_ref()
    }

    pub const fn content(&self) -> &BTreeMap<ContentUnitId, ResolvedProfileContent> {
        &self.content
    }

    pub const fn player(&self) -> &LaunchPlayerProfileSpec {
        &self.player
    }
}

pub(super) fn resolve_profile(
    accepted: &SourceBackedManifest,
    selection: LaunchProfileSelection<'_>,
) -> Result<ResolvedLaunchProfile, ManifestReport> {
    let (profile_id, profile) = select_profile(accepted, selection)?;
    let mut diagnostics = Vec::new();

    validate_reference_family(
        accepted,
        profile_id,
        profile.entry.as_ref(),
        ProfileReferenceField::Entry,
        "@entry.",
        ManifestDiagnosticCode::ReferenceEntryFamily,
        &mut diagnostics,
    );
    let external_modules =
        resolve_external_modules(accepted, profile_id, profile, &mut diagnostics);
    let activity_bindings = resolve_activity_bindings(
        accepted,
        profile_id,
        profile,
        &external_modules,
        &mut diagnostics,
    );
    let content = resolve_content(accepted, profile_id, profile, &mut diagnostics);

    let mut diagnostics = diagnostics.into_iter();
    if let Some(first) = diagnostics.next() {
        return Err(ManifestReport::from_first(first, diagnostics));
    }

    Ok(ResolvedLaunchProfile {
        id: profile_id.clone(),
        kind: profile.kind,
        source: profile.source.clone(),
        entry: profile.entry.clone(),
        adapter: profile
            .adapter
            .clone()
            .unwrap_or_else(AdapterProfileId::sans_io),
        external_modules,
        activity_bindings,
        dialogue: DialoguePresentationProfile::new(
            profile
                .dialogue
                .view
                .clone()
                .unwrap_or_else(ViewId::standard_dialogue),
            profile.dialogue.style.clone(),
            profile.dialogue.inline_failure.clone().unwrap_or_default(),
        ),
        listen: profile.listen,
        pure: profile.pure.clone(),
        content,
        player: profile.player.clone(),
    })
}

fn select_profile<'manifest>(
    accepted: &'manifest SourceBackedManifest,
    selection: LaunchProfileSelection<'_>,
) -> Result<(&'manifest ProfileId, &'manifest ProfileSpec), ManifestReport> {
    let manifest = accepted.manifest();
    match selection {
        LaunchProfileSelection::Explicit(requested) => manifest
            .profiles
            .iter()
            .find(|(id, _)| id.as_str() == requested)
            .ok_or_else(|| {
                report(
                    ManifestDiagnosticCode::ProfileMissing,
                    format!("launch profile `{requested}` is not declared"),
                    profiles_anchor(accepted),
                )
            }),
        LaunchProfileSelection::Automatic { previous } => {
            if let Some(default) = manifest.default_profile.as_ref() {
                return manifest.profiles.get_key_value(default).ok_or_else(|| {
                    report(
                        ManifestDiagnosticCode::ProfileDefaultMissing,
                        format!("default profile `{default}` is not declared"),
                        source_span(
                            accepted,
                            ManifestPath::new([ManifestPathSegment::Root(
                                ManifestRootField::DefaultProfile,
                            )]),
                            ManifestSourceSlot::ScalarValue,
                            profiles_anchor(accepted),
                        ),
                    )
                });
            }
            if let Some(previous) = previous
                && let Some(profile) = manifest
                    .profiles
                    .iter()
                    .find(|(id, _)| id.as_str() == previous)
            {
                return Ok(profile);
            }
            manifest.profiles.first_key_value().ok_or_else(|| {
                report(
                    ManifestDiagnosticCode::ProfileNone,
                    "manifest does not declare a launch profile",
                    profiles_anchor(accepted),
                )
            })
        }
    }
}

fn validate_reference_family(
    accepted: &SourceBackedManifest,
    profile_id: &ProfileId,
    reference: Option<&EntityIdRef>,
    field: ProfileReferenceField,
    expected_prefix: &str,
    code: ManifestDiagnosticCode,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) {
    let Some(reference) = reference else {
        return;
    };
    if reference.as_str().starts_with(expected_prefix) {
        return;
    }
    diagnostics.push(ManifestDiagnostic::new(
        code,
        format!(
            "{} must use the `{expected_prefix}*` family",
            field.description()
        ),
        source_span(
            accepted,
            ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile_id.clone()),
                ManifestPathSegment::ProfileField(field.source_field()),
            ]),
            ManifestSourceSlot::ScalarValue,
            profile_anchor(accepted, profile_id),
        ),
    ));
}

fn resolve_external_modules(
    accepted: &SourceBackedManifest,
    profile_id: &ProfileId,
    profile: &ProfileSpec,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ExternalModuleImportId, ExternalModuleImportSpec> {
    let mut resolved = BTreeMap::new();
    for (index, import_id) in profile.external_modules.iter().enumerate() {
        let Some(import) = accepted.manifest().external_modules.get(import_id) else {
            diagnostics.push(ManifestDiagnostic::new(
                ManifestDiagnosticCode::ReferenceExternalModuleMissing,
                format!("external module `{import_id}` is not declared"),
                indexed_profile_span(accepted, profile_id, ProfileField::ExternalModules, index),
            ));
            continue;
        };
        resolved.insert(import_id.clone(), import.clone());
    }
    resolved
}

fn resolve_activity_bindings(
    accepted: &SourceBackedManifest,
    profile_id: &ProfileId,
    profile: &ProfileSpec,
    selected_modules: &BTreeMap<ExternalModuleImportId, ExternalModuleImportSpec>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ActivityId, ResolvedActivityBinding> {
    let mut resolved = BTreeMap::new();
    for (index, binding) in profile.activity_bindings.iter().enumerate() {
        let Some(implementation) = accepted
            .manifest()
            .activity_implementations
            .get(&binding.implementation)
        else {
            diagnostics.push(ManifestDiagnostic::new(
                ManifestDiagnosticCode::ReferenceActivityImplementationMissing,
                format!(
                    "Activity implementation `{}` is not declared",
                    binding.implementation
                ),
                activity_binding_field_span(
                    accepted,
                    profile_id,
                    index,
                    ActivityBindingField::Implementation,
                ),
            ));
            continue;
        };
        if !selected_modules.contains_key(&implementation.module) {
            diagnostics.push(ManifestDiagnostic::new(
                ManifestDiagnosticCode::ReferenceActivityModuleNotSelected,
                format!(
                    "Activity implementation `{}` requires unselected module `{}`",
                    binding.implementation, implementation.module
                ),
                source_span(
                    accepted,
                    ManifestPath::new([
                        ManifestPathSegment::Root(ManifestRootField::ActivityImplementations),
                        ManifestPathSegment::ActivityImplementation(binding.implementation.clone()),
                        ManifestPathSegment::ActivityImplementationField(
                            ActivityImplementationField::Module,
                        ),
                    ]),
                    ManifestSourceSlot::ScalarValue,
                    activity_binding_field_span(
                        accepted,
                        profile_id,
                        index,
                        ActivityBindingField::Implementation,
                    ),
                ),
            ));
            continue;
        }
        resolved.insert(
            binding.activity.clone(),
            ResolvedActivityBinding {
                implementation_id: binding.implementation.clone(),
                implementation: implementation.clone(),
            },
        );
    }
    resolved
}

fn resolve_content(
    accepted: &SourceBackedManifest,
    profile_id: &ProfileId,
    profile: &ProfileSpec,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<ContentUnitId, ResolvedProfileContent> {
    let mut resolved = BTreeMap::new();
    for (content_id, policy) in &profile.content {
        let Some(unit) = accepted.manifest().content_units.get(content_id) else {
            diagnostics.push(ManifestDiagnostic::new(
                ManifestDiagnosticCode::ReferenceContentUnitMissing,
                format!("content unit `{content_id}` is not declared"),
                source_span(
                    accepted,
                    ManifestPath::new([
                        ManifestPathSegment::Root(ManifestRootField::Profiles),
                        ManifestPathSegment::Profile(profile_id.clone()),
                        ManifestPathSegment::ProfileField(ProfileField::Content),
                        ManifestPathSegment::ProfileContent(content_id.clone()),
                    ]),
                    ManifestSourceSlot::MapKey,
                    profile_anchor(accepted, profile_id),
                ),
            ));
            continue;
        };
        resolved.insert(
            content_id.clone(),
            ResolvedProfileContent {
                unit: unit.clone(),
                policy: policy.clone(),
            },
        );
    }
    resolved
}

fn indexed_profile_span(
    accepted: &SourceBackedManifest,
    profile_id: &ProfileId,
    field: ProfileField,
    index: usize,
) -> SourceSpan {
    let fallback = profile_anchor(accepted, profile_id);
    let Ok(index) = u32::try_from(index) else {
        return fallback;
    };
    source_span(
        accepted,
        ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::Profiles),
            ManifestPathSegment::Profile(profile_id.clone()),
            ManifestPathSegment::ProfileField(field),
            ManifestPathSegment::Index(index),
        ]),
        ManifestSourceSlot::ArrayElement { index },
        fallback,
    )
}

fn activity_binding_field_span(
    accepted: &SourceBackedManifest,
    profile_id: &ProfileId,
    binding_index: usize,
    field: ActivityBindingField,
) -> SourceSpan {
    let fallback = profile_anchor(accepted, profile_id);
    let Ok(binding_index) = u32::try_from(binding_index) else {
        return fallback;
    };
    source_span(
        accepted,
        ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::Profiles),
            ManifestPathSegment::Profile(profile_id.clone()),
            ManifestPathSegment::ProfileField(ProfileField::ActivityBindings),
            ManifestPathSegment::ActivityBinding(binding_index),
            ManifestPathSegment::ActivityBindingField(field),
        ]),
        ManifestSourceSlot::ScalarValue,
        fallback,
    )
}

fn profile_anchor(accepted: &SourceBackedManifest, profile_id: &ProfileId) -> SourceSpan {
    source_span(
        accepted,
        ManifestPath::new([
            ManifestPathSegment::Root(ManifestRootField::Profiles),
            ManifestPathSegment::Profile(profile_id.clone()),
        ]),
        ManifestSourceSlot::MapKey,
        profiles_anchor(accepted),
    )
}

fn profiles_anchor(accepted: &SourceBackedManifest) -> SourceSpan {
    source_span(
        accepted,
        ManifestPath::new([ManifestPathSegment::Root(ManifestRootField::Profiles)]),
        ManifestSourceSlot::TableHeader,
        accepted.document().end_span(),
    )
}

fn source_span(
    accepted: &SourceBackedManifest,
    path: ManifestPath,
    slot: ManifestSourceSlot,
    fallback: SourceSpan,
) -> SourceSpan {
    accepted
        .source_map()
        .get(&ManifestSourceKey { path, slot })
        .cloned()
        .unwrap_or(fallback)
}

fn report(
    code: ManifestDiagnosticCode,
    message: impl Into<String>,
    primary: SourceSpan,
) -> ManifestReport {
    ManifestReport::single(ManifestDiagnostic::new(code, message, primary))
}

#[derive(Clone, Copy)]
enum ProfileReferenceField {
    Entry,
}

impl ProfileReferenceField {
    const fn source_field(self) -> ProfileField {
        match self {
            Self::Entry => ProfileField::Entry,
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Entry => "profile entry",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ResolvedActivityBinding, ResolvedProfileContent};
    use crate::{
        LaunchProfileSelection, accepted::SourceBackedManifest, diagnostic::ManifestDiagnosticCode,
    };
    use arcweft_dialogue::InlineFailurePolicy;
    use arcweft_manifest_model::{ActivityId, ContentUnitId, ExternalModuleImportId, ProfileId};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::sync::Arc;

    fn accepted(source: &str) -> SourceBackedManifest {
        SourceBackedManifest::decode(Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("resolved-manifest").expect("document id"),
                SourceName::Memory,
                source,
            )
            .expect("source document"),
        ))
        .expect("accepted manifest")
    }

    fn minimal(extra: &str) -> String {
        format!("schema = 1\n{extra}[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n")
    }

    #[test]
    fn selection_uses_explicit_default_previous_then_lexical_precedence() {
        let manifest = accepted(&minimal(
            "default-profile = \"beta\"\n[profiles.alpha]\nkind = \"game\"\nsource = \"src/a.arcw\"\n[profiles.beta]\nkind = \"cli\"\nsource = \"src/b.arcw\"\n",
        ));
        assert_eq!(
            manifest
                .resolve_profile(LaunchProfileSelection::Explicit("alpha"))
                .expect("explicit")
                .id
                .as_str(),
            "alpha"
        );
        assert_eq!(
            manifest
                .resolve_profile(LaunchProfileSelection::Automatic {
                    previous: Some("alpha"),
                })
                .expect("default")
                .id
                .as_str(),
            "beta"
        );

        let without_default = accepted(&minimal(
            "[profiles.beta]\nkind = \"cli\"\nsource = \"src/b.arcw\"\n[profiles.alpha]\nkind = \"game\"\nsource = \"src/a.arcw\"\n",
        ));
        assert_eq!(
            without_default
                .resolve_profile(LaunchProfileSelection::Automatic {
                    previous: Some("beta"),
                })
                .expect("previous")
                .id
                .as_str(),
            "beta"
        );
        assert_eq!(
            without_default
                .resolve_profile(LaunchProfileSelection::Automatic {
                    previous: Some("stale"),
                })
                .expect("lexical first")
                .id
                .as_str(),
            "alpha"
        );
        let resolved = without_default
            .resolve_profile(LaunchProfileSelection::Explicit("alpha"))
            .expect("profile with omitted dialogue policy");
        assert_eq!(resolved.dialogue().view().as_str(), "std.view.dialogue");
        assert_eq!(resolved.dialogue().style(), None);
        assert_eq!(
            resolved.dialogue().inline_failure(),
            &InlineFailurePolicy::FailLine
        );
    }

    #[test]
    fn selection_failures_do_not_fall_back() {
        let no_profiles = accepted(&minimal(""));
        let report = no_profiles
            .resolve_profile(LaunchProfileSelection::Automatic { previous: None })
            .expect_err("no profiles");
        assert_eq!(
            report.diagnostics()[0].code(),
            ManifestDiagnosticCode::ProfileNone
        );

        let one = accepted(&minimal(
            "default-profile = \"alpha\"\n[profiles.alpha]\nkind = \"game\"\nsource = \"src/a.arcw\"\n",
        ));
        let report = one
            .resolve_profile(LaunchProfileSelection::Explicit("missing"))
            .expect_err("explicit failure");
        assert_eq!(
            report.diagnostics()[0].code(),
            ManifestDiagnosticCode::ProfileMissing
        );

        let stale_default = accepted(&minimal(
            "default-profile = \"missing\"\n[profiles.alpha]\nkind = \"game\"\nsource = \"src/a.arcw\"\n",
        ));
        let report = stale_default
            .resolve_profile(LaunchProfileSelection::Automatic {
                previous: Some("alpha"),
            })
            .expect_err("invalid default");
        assert_eq!(
            report.diagnostics()[0].code(),
            ManifestDiagnosticCode::ProfileDefaultMissing
        );
    }

    #[test]
    fn resolved_profile_joins_selected_modules_activity_and_content() {
        let source = r#"schema = 1
[package]
id = "org.arcweft.test"
version = "1.0.0"
[content-units.characters]
roots = ["@character.alice"]
visibility = "package"
demand = "required"
[external-modules.native-http]
mount = "http"
metadata = "generated/http.json"
metadata-hash = "blake3:1111111111111111111111111111111111111111111111111111111111111111"
expected-package = "org.arcweft.adapters.http"
expected-version = "1.0.0"
expected-module = "native_http"
expected-family = "rust"
expected-abi-hash = "blake3:2222222222222222222222222222222222222222222222222222222222222222"
visibility = "package"
demand = "required"
[external-modules.unused]
mount = "unused"
metadata = "generated/unused.json"
metadata-hash = "blake3:3333333333333333333333333333333333333333333333333333333333333333"
expected-package = "org.arcweft.adapters.unused"
expected-version = "1.0.0"
expected-module = "unused"
expected-family = "wasm"
expected-abi-hash = "blake3:4444444444444444444444444444444444444444444444444444444444444444"
visibility = "private"
demand = "optional"
[activity-implementations.http-fetch]
module = "native-http"
export = "http_fetch"
[profiles.game]
kind = "game"
source = "src/main.arcw"
entry = "@entry.game"
external-modules = ["native-http"]
activity-bindings = [{ activity = "activity.fetch_http", implementation = "http-fetch" }]
[profiles.game.dialogue]
view = "view.dialogue.mobile"
style = "style.dialogue.mobile"
inline-failure = { kind = "discard" }
[profiles.game.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#;
        let resolved = accepted(source)
            .resolve_profile(LaunchProfileSelection::Explicit("game"))
            .expect("resolved profile");

        assert_eq!(resolved.id, ProfileId::new("game").expect("profile id"));
        assert_eq!(resolved.source.as_str(), "src/main.arcw");
        assert_eq!(resolved.adapter.as_str(), "sans-io");
        assert_eq!(
            resolved.dialogue().inline_failure(),
            &InlineFailurePolicy::Discard
        );
        assert_eq!(resolved.dialogue().view().as_str(), "view.dialogue.mobile");
        assert_eq!(
            resolved
                .dialogue()
                .style()
                .map(|style| style.public_id().as_str()),
            Some("style.dialogue.mobile")
        );
        assert_eq!(
            resolved
                .entry
                .as_ref()
                .map(arcweft_manifest_model::EntityIdRef::as_str),
            Some("@entry.game")
        );
        assert_eq!(
            resolved
                .external_modules
                .keys()
                .map(ExternalModuleImportId::as_str)
                .collect::<Vec<_>>(),
            ["native-http"]
        );
        assert!(matches!(
            resolved
                .activity_bindings
                .get(&ActivityId::new("activity.fetch_http").expect("Activity id")),
            Some(ResolvedActivityBinding { implementation, .. })
                if implementation.module.as_str() == "native-http"
        ));
        assert!(matches!(
            resolved
                .content
                .get(&ContentUnitId::new("characters").expect("content id")),
            Some(ResolvedProfileContent { unit, .. })
                if unit.roots.as_slice().len() == 1
        ));
    }

    #[test]
    fn cross_reference_failures_are_ordered_and_prevent_resolution() {
        let source = r#"schema = 1
[package]
id = "org.arcweft.test"
version = "1.0.0"
[activity-implementations.orphan]
module = "missing-module"
export = "run"
[profiles.game]
kind = "game"
source = "src/main.arcw"
entry = "@flow.not-an-entry"
external-modules = ["missing-module"]
activity-bindings = [
  { activity = "activity.missing", implementation = "undeclared" },
  { activity = "activity.orphan", implementation = "orphan" },
]
[profiles.game.content.missing]
residency = "startup"
placement = "embedded"
compression = "none"
"#;
        let report = accepted(source)
            .resolve_profile(LaunchProfileSelection::Explicit("game"))
            .expect_err("invalid references");
        let codes = report
            .diagnostics()
            .iter()
            .map(crate::diagnostic::ManifestDiagnostic::code)
            .collect::<Vec<_>>();
        for expected in [
            ManifestDiagnosticCode::ReferenceEntryFamily,
            ManifestDiagnosticCode::ReferenceExternalModuleMissing,
            ManifestDiagnosticCode::ReferenceActivityImplementationMissing,
            ManifestDiagnosticCode::ReferenceActivityModuleNotSelected,
            ManifestDiagnosticCode::ReferenceContentUnitMissing,
        ] {
            assert!(codes.contains(&expected), "missing {expected:?}: {codes:?}");
        }
    }
}
