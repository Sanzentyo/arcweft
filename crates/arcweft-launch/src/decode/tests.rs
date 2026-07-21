use super::decode;
use crate::{
    LaunchMathBackend, LaunchPlayerViewportFit, LaunchPureBackend,
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode},
    manifest::LaunchPureWorkers,
    source_map::{
        ActivityBindingField, ActivityImplementationField, BuildField, CharacterNamesField,
        ContentUnitField, DialogueField, ExternalModuleField, FallbackStyleField,
        InlineFailureField, InlineFallbackField, LocalizationField, ManifestPath,
        ManifestPathSegment, ManifestRootField, ManifestSourceKey, ManifestSourceMap,
        ManifestSourceSlot, PackageField, PlayerField, ProfileContentField, ProfileField,
        PureField, ViewportField,
    },
};
use arcweft_dialogue::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};
use arcweft_manifest_model::{
    ActivityImplementationId, ContentCompression, ContentPlacement, ContentResidency,
    ContentUnitId, ExternalModuleImportId, ProfileId,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};
use arcweft_view::ViewId;
use std::{collections::BTreeMap, net::SocketAddr, str::FromStr, sync::Arc};

fn document(source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("manifest-test").expect("id"),
            SourceName::Memory,
            source,
        )
        .expect("document"),
    )
}

fn minimal(extra: &str) -> String {
    format!("schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.2.3\"\n{extra}")
}

fn range_of(source: &str, needle: &str, occurrence: usize) -> SourceRange {
    let start = source
        .match_indices(needle)
        .nth(occurrence)
        .map(|(start, _)| start)
        .expect("fixture substring");
    SourceRange::new(start, start + needle.len())
}

fn source_key(
    segments: impl Into<Box<[ManifestPathSegment]>>,
    slot: ManifestSourceSlot,
) -> ManifestSourceKey {
    ManifestSourceKey {
        path: ManifestPath::new(segments),
        slot,
    }
}

#[test]
fn direct_tree_decode_preserves_root_records_and_source_map() {
    let source =
        minimal("[build]\nsource-dir = \"game\"\ntarget-dir = \"out\"\nincremental = false\n");
    let source_document = document(&source);
    let decoded = decode(Arc::clone(&source_document)).expect("decode");

    assert_eq!(decoded.manifest.schema.get(), 1);
    assert_eq!(decoded.manifest.package.id.as_str(), "org.arcweft.test");
    assert_eq!(decoded.manifest.package.version.to_string(), "1.2.3");
    assert_eq!(decoded.manifest.build.source_dir.as_str(), "game");
    assert_eq!(decoded.manifest.build.target_dir.as_str(), "out");
    assert!(!decoded.manifest.build.incremental);
    assert!(Arc::ptr_eq(&source_document, decoded.source_map.document()));

    for (key, expected) in [
        (
            source_key(
                [ManifestPathSegment::Root(ManifestRootField::Schema)],
                ManifestSourceSlot::FieldKey,
            ),
            range_of(&source, "schema", 0),
        ),
        (
            source_key(
                [
                    ManifestPathSegment::Root(ManifestRootField::Package),
                    ManifestPathSegment::Package(PackageField::Id),
                ],
                ManifestSourceSlot::ScalarValue,
            ),
            range_of(&source, "\"org.arcweft.test\"", 0),
        ),
        (
            source_key(
                [
                    ManifestPathSegment::Root(ManifestRootField::Build),
                    ManifestPathSegment::Build(BuildField::SourceDir),
                ],
                ManifestSourceSlot::FieldKey,
            ),
            range_of(&source, "source-dir", 0),
        ),
        (
            source_key(
                [
                    ManifestPathSegment::Root(ManifestRootField::Build),
                    ManifestPathSegment::Build(BuildField::Incremental),
                ],
                ManifestSourceSlot::ScalarValue,
            ),
            range_of(&source, "false", 0),
        ),
    ] {
        let span = decoded.source_map.get(&key).expect("source-map entry");
        assert_eq!(span.range(), expected);
        assert_eq!(span.source(), source_document.identity());
    }
}

#[test]
fn source_map_ranges_are_exact_utf8_byte_boundaries() {
    let source = "# 日本語の前置き\nschema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.2.3\"\n[build]\nsource-dir = \"物語\"\n[profiles.dev]\nkind = \"game\"\nsource = \"src/物語.arcw\"\n";
    let source_document = document(source);
    let decoded = decode(Arc::clone(&source_document)).expect("UTF-8 manifest");
    let profile_id = ProfileId::new("dev").expect("profile id");

    for (key, expected) in [
        (
            source_key(
                [
                    ManifestPathSegment::Root(ManifestRootField::Build),
                    ManifestPathSegment::Build(BuildField::SourceDir),
                ],
                ManifestSourceSlot::ScalarValue,
            ),
            range_of(source, "\"物語\"", 0),
        ),
        (
            source_key(
                [
                    ManifestPathSegment::Root(ManifestRootField::Profiles),
                    ManifestPathSegment::Profile(profile_id),
                    ManifestPathSegment::ProfileField(ProfileField::Source),
                ],
                ManifestSourceSlot::ScalarValue,
            ),
            range_of(source, "\"src/物語.arcw\"", 0),
        ),
    ] {
        let span = decoded.source_map.get(&key).expect("UTF-8 source span");
        assert_eq!(span.range(), expected);
        assert_eq!(span.source(), source_document.identity());
        assert!(source.is_char_boundary(span.range().start()));
        assert!(source.is_char_boundary(span.range().end()));
    }
}

#[test]
fn source_map_builder_rejects_a_span_from_another_document_revision() {
    let source_document = document(&minimal(""));
    let foreign_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("manifest-test").expect("id"),
            SourceName::Memory,
            minimal("[build]\nincremental = false\n"),
        )
        .expect("foreign document"),
    );
    let mut entries = BTreeMap::new();
    entries.insert(
        source_key(
            [ManifestPathSegment::Root(ManifestRootField::Schema)],
            ManifestSourceSlot::ScalarValue,
        ),
        foreign_document.start_span(),
    );

    assert!(ManifestSourceMap::try_new(source_document, entries).is_err());
}

#[test]
fn required_and_typed_value_failures_use_exact_ranges() {
    let missing = "[build]\nincremental = true\n";
    let report = decode(document(missing)).expect_err("must reject");
    assert_eq!(
        report
            .diagnostics()
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.primary().range()))
            .collect::<Vec<_>>(),
        vec![
            (
                ManifestDiagnosticCode::SchemaMissing,
                SourceRange::new(0, 0)
            ),
            (
                ManifestDiagnosticCode::RequiredPackage,
                SourceRange::new(missing.len(), missing.len())
            ),
        ]
    );

    for (source, code, literal) in [
        (
            "schema = \"one\"\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n",
            ManifestDiagnosticCode::ValueType,
            "\"one\"",
        ),
        (
            "schema = 2\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n",
            ManifestDiagnosticCode::SchemaUnsupported,
            "2",
        ),
        (
            "schema = 1\n[package]\nid = \"bad\"\nversion = \"1.0.0\"\n",
            ManifestDiagnosticCode::IdInvalid,
            "\"bad\"",
        ),
        (
            "schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"latest\"\n",
            ManifestDiagnosticCode::VersionInvalid,
            "\"latest\"",
        ),
        (
            "schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = 1979-05-27\n",
            ManifestDiagnosticCode::ValueType,
            "1979-05-27",
        ),
        (
            "schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n[build]\nsource-dir = \"../escape\"\n",
            ManifestDiagnosticCode::PathInvalid,
            "\"../escape\"",
        ),
    ] {
        let report = decode(document(source)).expect_err("must reject");
        let diagnostic = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == code)
            .expect("expected diagnostic");
        assert_eq!(diagnostic.primary().range(), range_of(source, literal, 0));
    }
}

#[test]
fn unknown_shape_and_duplicates_retain_raw_first_and_later_ranges() {
    let shape = "schema = 1\nmystery = true\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\nextra = 4\n[build]\nother = false\n[mystery-table]\n";
    let report = decode(document(shape)).expect_err("must reject");
    for (code, range) in [
        (
            ManifestDiagnosticCode::UnknownRootKey,
            range_of(shape, "mystery", 0),
        ),
        (
            ManifestDiagnosticCode::UnknownField,
            range_of(shape, "extra", 0),
        ),
        (
            ManifestDiagnosticCode::UnknownField,
            range_of(shape, "other", 0),
        ),
        (
            ManifestDiagnosticCode::UnknownTable,
            range_of(shape, "[mystery-table]", 0),
        ),
    ] {
        assert!(
            report.diagnostics().iter().any(
                |diagnostic| diagnostic.code() == code && diagnostic.primary().range() == range
            )
        );
    }

    for (source, code, primary, related) in [
        (
            "schema = 1\nschema = 2\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n",
            ManifestDiagnosticCode::DuplicateRootKey,
            range_of(
                "schema = 1\nschema = 2\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n",
                "schema",
                1,
            ),
            range_of(
                "schema = 1\nschema = 2\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n",
                "schema",
                0,
            ),
        ),
        (
            "schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n[package]\n",
            ManifestDiagnosticCode::DuplicateTable,
            range_of(
                "schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n[package]\n",
                "[package]",
                1,
            ),
            range_of(
                "schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n[package]\n",
                "[package]",
                0,
            ),
        ),
        (
            "schema = 1\n[package]\nid = \"org.arcweft.test\"\nid = \"org.arcweft.other\"\nversion = \"1.0.0\"\n",
            ManifestDiagnosticCode::DuplicateField,
            range_of(
                "schema = 1\n[package]\nid = \"org.arcweft.test\"\nid = \"org.arcweft.other\"\nversion = \"1.0.0\"\n",
                "id",
                1,
            ),
            range_of(
                "schema = 1\n[package]\nid = \"org.arcweft.test\"\nid = \"org.arcweft.other\"\nversion = \"1.0.0\"\n",
                "id",
                0,
            ),
        ),
    ] {
        let report = decode(document(source)).expect_err("must reject");
        let diagnostic = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == code)
            .expect("duplicate diagnostic");
        assert_eq!(diagnostic.primary().range(), primary);
        assert_eq!(diagnostic.related()[0].label(), "first declared here");
        assert_eq!(diagnostic.related()[0].span().range(), related);
    }
}

#[test]
fn obsolete_shapes_are_rejected_by_the_current_grammar_without_special_recognizers() {
    for source in [
        minimal("default = \"dev\"\n").as_str(),
        minimal("[resources]\nasset-dir = \"assets\"\n").as_str(),
        minimal(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\nadapter-manifests = []\n",
        )
        .as_str(),
    ] {
        let report = decode(document(source)).expect_err("obsolete shape must not be accepted");
        assert!(report.diagnostics().iter().any(|diagnostic| {
            matches!(
                diagnostic.code(),
                ManifestDiagnosticCode::UnknownRootKey
                    | ManifestDiagnosticCode::UnknownTable
                    | ManifestDiagnosticCode::UnknownField
            )
        }));
    }
}

#[test]
fn reports_are_deterministic_and_malformed_toml_is_fail_closed() {
    let source = "unknown-a = true\nschema = 1\nschema = 2\nunknown-b = false\n";
    let first = decode(document(source)).expect_err("must reject");
    let second = decode(document(source)).expect_err("must reject");
    assert_eq!(first, second);
    assert_eq!(
        first
            .diagnostics()
            .iter()
            .map(ManifestDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![
            ManifestDiagnosticCode::UnknownRootKey,
            ManifestDiagnosticCode::DuplicateRootKey,
            ManifestDiagnosticCode::UnknownRootKey,
            ManifestDiagnosticCode::RequiredPackage,
        ]
    );

    let malformed = "schema = \n[package\n";
    let report = decode(document(malformed)).expect_err("must reject");
    assert!(
        report
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::TomlSyntax)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn canonical_profile_decodes_typed_policies_and_nested_source_map() {
    let source = r#"schema = 1
default-profile = "dev"

[package]
id = "org.arcweft.samples.dialogue"
version = "0.1.0"

[content-units.characters]
roots = ["@character.alice"]
visibility = "package"
demand = "required"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
entry = "@entry.game"
adapter = "sans-io"
listen = "127.0.0.1:8080"

[profiles.dev.dialogue]
inline-failure = { kind = "fallback", fallback = { kind = "text", text = "[missing]", style = { kind = "plain" } } }

[profiles.dev.pure]
backend = "auto"
math-backend = "wgpu"
math-wgpu-min-elements = 256
workers = 4
batch-min-len = 64
object-artifacts = false

[profiles.dev.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"

[profiles.dev.player.viewport]
design-width = 1280
design-height = 720
fit = "contain"
"#;
    let source_document = document(source);
    let decoded = decode(Arc::clone(&source_document)).expect("canonical manifest");
    let profile_id = ProfileId::new("dev").expect("profile id");
    let profile = decoded
        .manifest
        .profiles
        .get(&profile_id)
        .expect("decoded profile");

    assert_eq!(
        decoded
            .manifest
            .default_profile
            .as_ref()
            .map(ProfileId::as_str),
        Some("dev")
    );
    assert_eq!(profile.source.as_str(), "src/main.arcw");
    assert_eq!(
        profile
            .entry
            .as_ref()
            .map(arcweft_manifest_model::EntityIdRef::as_str),
        Some("@entry.game")
    );
    assert_eq!(
        profile.dialogue.inline_failure,
        Some(InlineFailurePolicy::Fallback {
            fallback: InlineFallback::Text {
                text: "[missing]".to_owned(),
                style: FallbackStylePolicy::Plain,
            },
        })
    );
    let pure = profile.pure.as_ref().expect("pure policy");
    assert_eq!(pure.backend, Some(LaunchPureBackend::Auto));
    assert_eq!(pure.math_backend, Some(LaunchMathBackend::Wgpu));
    assert_eq!(
        pure.math_wgpu_min_elements.map(std::num::NonZeroU32::get),
        Some(256)
    );
    assert_eq!(
        pure.workers,
        Some(LaunchPureWorkers::Count(
            std::num::NonZeroU32::new(4).expect("positive worker count")
        ))
    );
    assert_eq!(pure.batch_min_len.map(std::num::NonZeroU32::get), Some(64));
    assert_eq!(pure.object_artifacts, Some(false));
    assert_eq!(
        profile
            .listen
            .map(crate::manifest::LaunchListenAddress::socket_addr),
        Some(SocketAddr::from_str("127.0.0.1:8080").expect("socket address"))
    );
    let viewport = profile.player.viewport.as_ref().expect("player viewport");
    assert_eq!(
        viewport.design_width.map(std::num::NonZeroU32::get),
        Some(1280)
    );
    assert_eq!(
        viewport.design_height.map(std::num::NonZeroU32::get),
        Some(720)
    );
    assert_eq!(viewport.fit, LaunchPlayerViewportFit::Contain);
    let content_id = ContentUnitId::new("characters").expect("content id");
    assert_eq!(
        profile.content.get(&content_id),
        Some(&arcweft_manifest_model::ProfileContentSpec {
            residency: ContentResidency::Startup,
            placement: ContentPlacement::Embedded,
            compression: ContentCompression::None,
        })
    );

    let style_kind = source_key(
        [
            ManifestPathSegment::Root(ManifestRootField::Profiles),
            ManifestPathSegment::Profile(profile_id.clone()),
            ManifestPathSegment::ProfileField(ProfileField::Dialogue),
            ManifestPathSegment::Dialogue(DialogueField::InlineFailure),
            ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
            ManifestPathSegment::InlineFallback(InlineFallbackField::Style),
            ManifestPathSegment::FallbackStyle(FallbackStyleField::Kind),
        ],
        ManifestSourceSlot::ScalarValue,
    );
    let span = decoded
        .source_map
        .get(&style_kind)
        .expect("fallback style kind source");
    assert_eq!(span.range(), range_of(source, "\"plain\"", 0));
    assert_eq!(span.source(), source_document.identity());
    assert_eq!(
        style_kind.path.segments().first(),
        Some(&ManifestPathSegment::Root(ManifestRootField::Profiles))
    );

    for key in [
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::ContentUnits),
                ManifestPathSegment::ContentUnit(content_id.clone()),
                ManifestPathSegment::ContentUnitField(ContentUnitField::Roots),
                ManifestPathSegment::Index(0),
            ],
            ManifestSourceSlot::ArrayElement { index: 0 },
        ),
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile_id.clone()),
                ManifestPathSegment::ProfileField(ProfileField::Pure),
                ManifestPathSegment::Pure(PureField::Workers),
            ],
            ManifestSourceSlot::ScalarValue,
        ),
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile_id.clone()),
                ManifestPathSegment::ProfileField(ProfileField::Content),
                ManifestPathSegment::ProfileContent(content_id),
                ManifestPathSegment::ProfileContentField(ProfileContentField::Compression),
            ],
            ManifestSourceSlot::FieldKey,
        ),
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile_id),
                ManifestPathSegment::ProfileField(ProfileField::Player),
                ManifestPathSegment::Player(PlayerField::Viewport),
                ManifestPathSegment::Viewport(ViewportField::Fit),
            ],
            ManifestSourceSlot::ScalarValue,
        ),
    ] {
        assert_eq!(
            decoded
                .source_map
                .get(&key)
                .expect("nested source-map entry")
                .source(),
            source_document.identity()
        );
    }
}

#[test]
fn removed_dialogue_defaults_is_rejected_as_an_unknown_profile_field() {
    let source = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\ndefaults = \"@dialogue.mobile\"\n",
    );
    let report = decode(document(&source)).expect_err("removed field must be rejected");

    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::UnknownField)
    );
}

#[test]
fn dialogue_profile_uses_nominal_view_and_style_ids_with_exact_source_map_entries() {
    let source = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\nview = \"view.dialogue.mobile\"\nstyle = \"style.dialogue.mobile\"\n",
    );
    let decoded = decode(document(&source)).expect("typed dialogue profile");
    let profile_id = ProfileId::new("dev").expect("profile id");
    let profile = decoded
        .manifest
        .profiles
        .get(&profile_id)
        .expect("decoded profile");

    assert_eq!(
        profile
            .dialogue
            .view
            .as_ref()
            .map(arcweft_view::ViewId::as_str),
        Some("view.dialogue.mobile")
    );
    assert_eq!(
        profile
            .dialogue
            .style
            .as_ref()
            .map(|style| style.public_id().as_str()),
        Some("style.dialogue.mobile")
    );

    for (field, expected) in [
        (DialogueField::View, "\"view.dialogue.mobile\""),
        (DialogueField::Style, "\"style.dialogue.mobile\""),
    ] {
        let key = source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile_id.clone()),
                ManifestPathSegment::ProfileField(ProfileField::Dialogue),
                ManifestPathSegment::Dialogue(field),
            ],
            ManifestSourceSlot::ScalarValue,
        );
        let span = decoded.source_map.get(&key).expect("dialogue source span");
        assert_eq!(span.range(), range_of(&source, expected, 0));
    }
}

#[test]
fn character_name_localization_policy_is_typed_ordered_and_revision_bound() {
    let source = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\
         [profiles.dev.localization.character_names]\n\
         active = \"ja-JP\"\nfallbacks = [\"en\", \"fr\"]\n",
    );
    let source_document = document(&source);
    let decoded = decode(Arc::clone(&source_document)).expect("typed localization policy");
    let profile_id = ProfileId::new("dev").expect("profile id");
    let policy = decoded
        .manifest
        .profiles
        .get(&profile_id)
        .and_then(|profile| profile.localization.character_names())
        .expect("Character-name policy");

    assert_eq!(policy.active().as_str(), "ja-JP");
    assert_eq!(
        policy
            .fallbacks()
            .iter()
            .map(arcweft_id::LocaleTag::as_str)
            .collect::<Vec<_>>(),
        ["en", "fr"]
    );

    let base = [
        ManifestPathSegment::Root(ManifestRootField::Profiles),
        ManifestPathSegment::Profile(profile_id),
        ManifestPathSegment::ProfileField(ProfileField::Localization),
        ManifestPathSegment::Localization(LocalizationField::CharacterNames),
    ];
    for (tail, slot, expected) in [
        (
            Vec::new(),
            ManifestSourceSlot::TableHeader,
            "[profiles.dev.localization.character_names]",
        ),
        (
            vec![ManifestPathSegment::CharacterNames(
                CharacterNamesField::Active,
            )],
            ManifestSourceSlot::ScalarValue,
            "\"ja-JP\"",
        ),
        (
            vec![ManifestPathSegment::CharacterNames(
                CharacterNamesField::Fallbacks,
            )],
            ManifestSourceSlot::ArrayElement { index: 0 },
            "\"en\"",
        ),
        (
            vec![ManifestPathSegment::CharacterNames(
                CharacterNamesField::Fallbacks,
            )],
            ManifestSourceSlot::ArrayElement { index: 1 },
            "\"fr\"",
        ),
    ] {
        let mut segments = base.to_vec();
        segments.extend(tail);
        let span = decoded
            .source_map
            .get(&source_key(segments, slot))
            .expect("localization source span");
        assert_eq!(span.range(), range_of(&source, expected, 0));
        assert_eq!(span.source(), source_document.identity());
    }
}

#[test]
fn character_name_locale_diagnostics_retain_exact_value_and_related_spans() {
    let noncanonical = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\
         [profiles.dev.localization.character_names]\n\
         active = \"ja-jp\"\nfallbacks = [\"en\"]\n",
    );
    let report = decode(document(&noncanonical)).expect_err("noncanonical active locale");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::CharacterNameLocaleInvalid)
        .expect("locale diagnostic");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&noncanonical, "\"ja-jp\"", 0)
    );

    let duplicate = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\
         [profiles.dev.localization.character_names]\n\
         active = \"ja-JP\"\nfallbacks = [\"en\", \"fr\", \"en\"]\n",
    );
    let report = decode(document(&duplicate)).expect_err("duplicate fallback");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == ManifestDiagnosticCode::CharacterNameFallbackDuplicate
        })
        .expect("duplicate fallback diagnostic");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&duplicate, "\"en\"", 1)
    );
    assert_eq!(
        diagnostic.related()[0].span().range(),
        range_of(&duplicate, "\"en\"", 0)
    );

    let active_repeated = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\
         [profiles.dev.localization.character_names]\n\
         active = \"ja-JP\"\nfallbacks = [\"ja-JP\"]\n",
    );
    let report = decode(document(&active_repeated)).expect_err("active repeated");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == ManifestDiagnosticCode::CharacterNameFallbackDuplicate
        })
        .expect("active repeat diagnostic");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&active_repeated, "\"ja-JP\"", 1)
    );
    assert_eq!(
        diagnostic.related()[0].span().range(),
        range_of(&active_repeated, "\"ja-JP\"", 0)
    );
}

#[test]
fn character_name_fallback_limit_is_exact_and_unknown_fields_fail_closed() {
    let exact = (0..16)
        .map(|index| format!("\"qaa-x{index}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let accepted = minimal(&format!(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\
         [profiles.dev.localization.character_names]\n\
         active = \"ja-JP\"\nfallbacks = [{exact}]\n"
    ));
    let decoded = decode(document(&accepted)).expect("16 fallbacks");
    assert_eq!(
        decoded
            .manifest
            .profiles
            .values()
            .next()
            .and_then(|profile| profile.localization.character_names())
            .expect("accepted policy")
            .fallbacks()
            .len(),
        16
    );

    let one_over = minimal(&format!(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\
         [profiles.dev.localization.character_names]\n\
         active = \"ja-JP\"\nfallbacks = [{exact}, \"qaa-x16\"]\n"
    ));
    let report = decode(document(&one_over)).expect_err("17 fallbacks");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::CharacterNameFallbackLimit)
        .expect("fallback limit diagnostic");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&one_over, "\"qaa-x16\"", 0)
    );

    let unknown = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\
         [profiles.dev.localization.character_names]\n\
         active = \"ja-JP\"\ndefault = \"en\"\n",
    );
    let report = decode(document(&unknown)).expect_err("unknown locale policy field");
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::UnknownField)
    );
}

#[test]
fn equivalent_dotted_policy_forms_decode_without_synthetic_tables() {
    let source = r#"schema = 1
package = { id = "org.arcweft.test", version = "1.0.0" }
profiles.dev.kind = "game"
profiles.dev.source = "src/main.arcw"
profiles.dev.dialogue.inline-failure.kind = "fallback"
profiles.dev.dialogue.inline-failure.fallback.kind = "expr_source"
profiles.dev.dialogue.inline-failure.fallback.style.kind = "inherit_surrounding"
profiles.dev.player.viewport.fit = "cover"
"#;
    let decoded = decode(document(source)).expect("dotted manifest");
    let profile = decoded
        .manifest
        .profiles
        .get(&ProfileId::new("dev").expect("profile id"))
        .expect("profile");
    assert_eq!(
        profile.dialogue.inline_failure,
        Some(InlineFailurePolicy::Fallback {
            fallback: InlineFallback::ExprSource {
                style: FallbackStylePolicy::InheritSurrounding,
            },
        })
    );
    assert_eq!(
        profile
            .player
            .viewport
            .as_ref()
            .map(|viewport| viewport.fit),
        Some(LaunchPlayerViewportFit::Cover)
    );
}

#[test]
fn dialogue_view_and_style_ids_distinguish_malformed_text_from_wrong_family() {
    for (field, value, code) in [
        ("view", "@view.dialogue", ManifestDiagnosticCode::IdInvalid),
        ("view", "style.dialogue", ManifestDiagnosticCode::IdFamily),
        (
            "style",
            "@style.dialogue",
            ManifestDiagnosticCode::IdInvalid,
        ),
        ("style", "view.dialogue", ManifestDiagnosticCode::IdFamily),
    ] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\n{field} = {value:?}\n"
        ));
        let report = decode(document(&source)).expect_err("invalid nominal identity must fail");
        let diagnostic = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == code)
            .expect("exact nominal identity diagnostic");
        assert_eq!(
            diagnostic.primary().range(),
            range_of(&source, &format!("{value:?}"), 0)
        );
    }
}

#[test]
fn dialogue_view_and_style_accept_authored_and_engine_owned_nominal_ids() {
    for (view, style) in [
        ("view.dialogue", "style.dialogue"),
        ("std.view.dialogue", "std.style.dialogue"),
    ] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\nview = {view:?}\nstyle = {style:?}\n"
        ));
        let decoded = decode(document(&source)).expect("nominal dialogue profile IDs");
        let profile = decoded
            .manifest
            .profiles
            .get(&ProfileId::new("dev").expect("profile ID"))
            .expect("profile");
        assert_eq!(
            profile.dialogue.view.as_ref().map(ViewId::as_str),
            Some(view)
        );
        assert_eq!(
            profile
                .dialogue
                .style
                .as_ref()
                .map(|id| id.public_id().as_str()),
            Some(style)
        );
    }
}

#[test]
fn canonical_manifest_serialization_omits_an_empty_dialogue_profile() {
    let source = minimal("[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n");
    let decoded = decode(document(&source)).expect("minimal profile");
    let encoded = serde_json::to_value(&decoded.manifest).expect("semantic manifest codec");
    assert!(encoded["profiles"]["dev"].get("dialogue").is_none());
}

#[test]
fn incomplete_inline_policy_uses_policy_diagnostic() {
    let source = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue.inline-failure]\n",
    );
    let report = decode(document(&source)).expect_err("must reject");
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::InlinePolicyInvalid)
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != ManifestDiagnosticCode::ValueMissing)
    );
}

#[test]
fn profile_scalar_policy_boundaries_are_typed_and_strict() {
    for listen in ["localhost:8080", "127.0.0.1", "not a socket"] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"server\"\nsource = \"src/main.arcw\"\nlisten = {listen:?}\n"
        ));
        let report = decode(document(&source)).expect_err("invalid listen must fail");
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == ManifestDiagnosticCode::ListenInvalid
                && diagnostic.primary().range() == range_of(&source, &format!("{listen:?}"), 0)
        }));
    }

    for listen in ["127.0.0.1:8080", "[::1]:8080"] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"server\"\nsource = \"src/main.arcw\"\nlisten = {listen:?}\n"
        ));
        let decoded = decode(document(&source)).expect("numeric socket address");
        let address = decoded
            .manifest
            .profiles
            .get(&ProfileId::new("dev").expect("profile"))
            .and_then(|profile| profile.listen)
            .expect("decoded listen address");
        assert_eq!(
            address.socket_addr(),
            SocketAddr::from_str(listen).expect("fixture socket")
        );
        let encoded = serde_json::to_value(address).expect("listen address serializes");
        assert_eq!(encoded, serde_json::Value::String(listen.to_owned()));
        assert_eq!(
            serde_json::from_value::<crate::manifest::LaunchListenAddress>(encoded)
                .expect("listen address deserializes"),
            address
        );
    }

    let automatic_workers = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.pure]\nworkers = \"auto\"\n",
    );
    let decoded = decode(document(&automatic_workers)).expect("auto workers");
    let profile = decoded
        .manifest
        .profiles
        .get(&ProfileId::new("dev").expect("profile"))
        .expect("decoded profile");
    assert_eq!(
        profile.pure.as_ref().and_then(|pure| pure.workers),
        Some(LaunchPureWorkers::Auto)
    );

    for workers in ["\"4\"", "0", "-1", "4294967296", "\"many\""] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.pure]\nworkers = {workers}\n"
        ));
        let report = decode(document(&source)).expect_err("invalid workers must fail");
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == ManifestDiagnosticCode::PureWorkersInvalid
                && diagnostic.primary().range() == range_of(&source, workers, 0)
        }));
    }

    for (field, value) in [
        ("math-wgpu-min-elements", "0"),
        ("math-wgpu-min-elements", "4294967296"),
        ("batch-min-len", "0"),
        ("batch-min-len", "4294967296"),
    ] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.pure]\n{field} = {value}\n"
        ));
        let report = decode(document(&source)).expect_err("invalid threshold must fail");
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == ManifestDiagnosticCode::PureThresholdInvalid
                && diagnostic.primary().range() == range_of(&source, value, 0)
        }));
    }
}

#[test]
fn viewport_resolved_defaults_and_raw_constraints_are_explicit() {
    let source = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.player.viewport]\n",
    );
    let decoded = decode(document(&source)).expect("default viewport");
    let viewport = decoded
        .manifest
        .profiles
        .get(&ProfileId::new("dev").expect("profile"))
        .and_then(|profile| profile.player.viewport)
        .expect("viewport");
    assert_eq!(viewport.fit(), LaunchPlayerViewportFit::Contain);
    assert_eq!(
        viewport.design_width().map(std::num::NonZeroU32::get),
        Some(1280)
    );
    assert_eq!(
        viewport.design_height().map(std::num::NonZeroU32::get),
        Some(720)
    );

    for field in ["design-width", "design-height"] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.player.viewport]\n{field} = 0\n"
        ));
        let report = decode(document(&source)).expect_err("zero dimension must fail");
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == ManifestDiagnosticCode::PlayerViewportInvalid
                && diagnostic.primary().range() == range_of(&source, "0", 0)
        }));
    }

    for field in ["design-width", "design-height"] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.player.viewport]\nfit = \"raw\"\n{field} = 640\n"
        ));
        let report = decode(document(&source)).expect_err("raw dimensions must fail");
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == ManifestDiagnosticCode::PlayerViewportInvalid
                && diagnostic.primary().range() == range_of(&source, "640", 0)
        }));
    }
}

#[test]
fn every_inline_failure_policy_variant_uses_the_owned_strict_wire_type() {
    let cases = [
        (r#"{ kind = "fail_line" }"#, InlineFailurePolicy::FailLine),
        (r#"{ kind = "discard" }"#, InlineFailurePolicy::Discard),
        (
            r#"{ kind = "fallback", fallback = { kind = "text", text = "?", style = { kind = "plain" } } }"#,
            InlineFailurePolicy::Fallback {
                fallback: InlineFallback::Text {
                    text: "?".to_owned(),
                    style: FallbackStylePolicy::Plain,
                },
            },
        ),
        (
            r#"{ kind = "fallback", fallback = { kind = "expr_source", style = { kind = "inherit_surrounding" } } }"#,
            InlineFailurePolicy::Fallback {
                fallback: InlineFallback::ExprSource {
                    style: FallbackStylePolicy::InheritSurrounding,
                },
            },
        ),
        (
            r#"{ kind = "fallback", fallback = { kind = "call_source", style = { kind = "apply", styles = [] } } }"#,
            InlineFailurePolicy::Fallback {
                fallback: InlineFallback::CallSource {
                    style: FallbackStylePolicy::Apply { styles: Vec::new() },
                },
            },
        ),
        (
            r#"{ kind = "fallback", fallback = { kind = "value_plain" } }"#,
            InlineFailurePolicy::Fallback {
                fallback: InlineFallback::ValuePlain,
            },
        ),
    ];
    for (wire, expected) in cases {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\ninline-failure = {wire}\n"
        ));
        let decoded = decode(document(&source)).expect("inline policy");
        let policy = decoded
            .manifest
            .profiles
            .get(&ProfileId::new("dev").expect("profile"))
            .expect("decoded profile")
            .dialogue
            .inline_failure
            .as_ref()
            .expect("authored inline policy");
        assert_eq!(policy, &expected);
        let encoded = serde_json::to_value(policy).expect("owned policy serializes");
        assert_eq!(
            serde_json::from_value::<InlineFailurePolicy>(encoded)
                .expect("owned policy deserializes"),
            expected
        );
    }
}

fn inline_failure_source_key(
    profile_id: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
    slot: ManifestSourceSlot,
) -> ManifestSourceKey {
    let mut segments = vec![
        ManifestPathSegment::Root(ManifestRootField::Profiles),
        ManifestPathSegment::Profile(profile_id.clone()),
        ManifestPathSegment::ProfileField(ProfileField::Dialogue),
        ManifestPathSegment::Dialogue(DialogueField::InlineFailure),
    ];
    segments.extend(tail);
    source_key(segments, slot)
}

fn assert_applied_style_element_spans(
    source_map: &ManifestSourceMap,
    source: &str,
    profile_id: &ProfileId,
    expected_styles: &[(u32, &str)],
) {
    for &(index, expected) in expected_styles {
        let key = inline_failure_source_key(
            profile_id,
            [
                ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
                ManifestPathSegment::InlineFallback(InlineFallbackField::Style),
                ManifestPathSegment::FallbackStyle(FallbackStyleField::Styles),
                ManifestPathSegment::Index(index),
            ],
            ManifestSourceSlot::ArrayElement { index },
        );
        assert_eq!(
            source_map.get(&key).expect("style array element").range(),
            range_of(source, expected, 0)
        );
    }
}

fn assert_inline_failure_scalar_spans(
    source_map: &ManifestSourceMap,
    source: &str,
    profile_id: &ProfileId,
) {
    for (tail, expected) in [
        (
            vec![ManifestPathSegment::InlineFailure(InlineFailureField::Kind)],
            "\"fallback\"",
        ),
        (
            vec![
                ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
                ManifestPathSegment::InlineFallback(InlineFallbackField::Kind),
            ],
            "\"call_source\"",
        ),
        (
            vec![
                ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
                ManifestPathSegment::InlineFallback(InlineFallbackField::Style),
                ManifestPathSegment::FallbackStyle(FallbackStyleField::Kind),
            ],
            "\"apply\"",
        ),
    ] {
        let key = inline_failure_source_key(profile_id, tail, ManifestSourceSlot::ScalarValue);
        assert_eq!(
            source_map
                .get(&key)
                .expect("nested inline policy scalar")
                .range(),
            range_of(source, expected, 0)
        );
    }
}

fn assert_inline_failure_key_spans(
    source_map: &ManifestSourceMap,
    source: &str,
    profile_id: &ProfileId,
) {
    for (tail, slot, expected, occurrence) in [
        (
            Vec::new(),
            ManifestSourceSlot::TableHeader,
            "inline-failure",
            0,
        ),
        (
            vec![ManifestPathSegment::InlineFailure(InlineFailureField::Kind)],
            ManifestSourceSlot::FieldKey,
            "kind",
            1,
        ),
        (
            vec![ManifestPathSegment::InlineFailure(
                InlineFailureField::Fallback,
            )],
            ManifestSourceSlot::TableHeader,
            "fallback",
            1,
        ),
        (
            vec![
                ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
                ManifestPathSegment::InlineFallback(InlineFallbackField::Kind),
            ],
            ManifestSourceSlot::FieldKey,
            "kind",
            2,
        ),
        (
            vec![
                ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
                ManifestPathSegment::InlineFallback(InlineFallbackField::Style),
            ],
            ManifestSourceSlot::TableHeader,
            "style",
            0,
        ),
        (
            vec![
                ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
                ManifestPathSegment::InlineFallback(InlineFallbackField::Style),
                ManifestPathSegment::FallbackStyle(FallbackStyleField::Kind),
            ],
            ManifestSourceSlot::FieldKey,
            "kind",
            3,
        ),
        (
            vec![
                ManifestPathSegment::InlineFailure(InlineFailureField::Fallback),
                ManifestPathSegment::InlineFallback(InlineFallbackField::Style),
                ManifestPathSegment::FallbackStyle(FallbackStyleField::Styles),
            ],
            ManifestSourceSlot::FieldKey,
            "styles",
            0,
        ),
    ] {
        let key = inline_failure_source_key(profile_id, tail, slot);
        assert_eq!(
            source_map
                .get(&key)
                .expect("nested inline policy key")
                .range(),
            range_of(source, expected, occurrence)
        );
    }
}

#[test]
fn applied_inline_fallback_styles_publish_every_nested_source_span() {
    let zero_layout = std::iter::repeat_n("0", 32).collect::<Vec<_>>().join(", ");
    let one_layout = std::iter::repeat_n("1", 32).collect::<Vec<_>>().join(", ");
    let first_style = format!("{{ layout = [{zero_layout}], value = {{ Record = [] }} }}");
    let second_style = format!("{{ layout = [{one_layout}], value = {{ Record = [] }} }}");
    let source = minimal(&format!(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\ninline-failure = {{ kind = \"fallback\", fallback = {{ kind = \"call_source\", style = {{ kind = \"apply\", styles = [{first_style}, {second_style}] }} }} }}\n"
    ));
    let decoded = decode(document(&source)).expect("applied fallback styles");
    let profile_id = ProfileId::new("dev").expect("profile id");
    let profile = decoded.manifest.profiles.get(&profile_id).expect("profile");
    let InlineFailurePolicy::Fallback {
        fallback:
            InlineFallback::CallSource {
                style: FallbackStylePolicy::Apply { styles },
            },
    } = profile
        .dialogue
        .inline_failure
        .as_ref()
        .expect("authored inline policy")
    else {
        panic!("expected applied call-source fallback");
    };
    assert_eq!(styles.len(), 2);
    let policy = profile
        .dialogue
        .inline_failure
        .as_ref()
        .expect("authored inline policy");
    let encoded = serde_json::to_value(policy).expect("applied policy serializes");
    assert_eq!(
        serde_json::from_value::<InlineFailurePolicy>(encoded)
            .expect("applied policy deserializes"),
        policy.clone()
    );

    assert_applied_style_element_spans(
        &decoded.source_map,
        &source,
        &profile_id,
        &[(0, first_style.as_str()), (1, second_style.as_str())],
    );

    assert_inline_failure_scalar_spans(&decoded.source_map, &source, &profile_id);

    assert_inline_failure_key_spans(&decoded.source_map, &source, &profile_id);
}

#[test]
fn inline_failure_unknown_fields_and_kinds_have_no_aliases() {
    for (policy, field_occurrence) in [
        (r#"{ kind = "fail_line", unexpected = true }"#, 0),
        (
            r#"{ kind = "fallback", fallback = { kind = "value_plain", unexpected = true } }"#,
            0,
        ),
        (
            r#"{ kind = "fallback", fallback = { kind = "text", text = "?", style = { kind = "plain", unexpected = true } } }"#,
            0,
        ),
    ] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\ninline-failure = {policy}\n"
        ));
        let report = decode(document(&source)).expect_err("unknown policy member must fail");
        assert!(
            report.diagnostics().iter().any(|diagnostic| {
                diagnostic.code() == ManifestDiagnosticCode::InlinePolicyInvalid
                    && diagnostic.primary().range()
                        == range_of(&source, "unexpected", field_occurrence)
            }),
            "expected exact inline policy field diagnostic for {policy}: {:?}",
            report.diagnostics()
        );
    }

    for policy in [
        r#"{ kind = "unknown" }"#,
        r#"{ kind = "fail-line" }"#,
        r#"{ kind = "fallback", fallback = { kind = "unknown", style = { kind = "plain" } } }"#,
        r#"{ kind = "fallback", fallback = { kind = "text", text = "?", style = { kind = "unknown" } } }"#,
    ] {
        let source = minimal(&format!(
            "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.dialogue]\ninline-failure = {policy}\n"
        ));
        let report = decode(document(&source)).expect_err("unknown policy member must fail");
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::InlinePolicyInvalid),
            "expected inline policy diagnostic for {policy}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn generated_module_profile_decodes_bindings_and_array_sources() {
    let source = r#"schema = 1
default-profile = "server"
[package]
id = "org.arcweft.samples.generated"
version = "0.1.0"
[external-modules."native-http"]
mount = "http"
metadata = "generated/native-http.adapter.json"
metadata-hash = "blake3:1111111111111111111111111111111111111111111111111111111111111111"
expected-package = "org.arcweft.adapters.native-http"
expected-version = "1.0.0"
expected-module = "native_http"
expected-family = "rust"
expected-abi-hash = "blake3:2222222222222222222222222222222222222222222222222222222222222222"
visibility = "package"
demand = "required"
[activity-implementations.http-fetch]
module = "native-http"
export = "http_fetch"
[profiles.server]
kind = "server"
source = "src/server.arcw"
external-modules = ["native-http"]
activity-bindings = [{ activity = "activity.fetch_http", implementation = "http-fetch" }]
listen = "[::1]:8080"
"#;
    let source_document = document(source);
    let decoded = decode(Arc::clone(&source_document)).expect("generated profile");
    let module_id = ExternalModuleImportId::new("native-http").expect("module id");
    let implementation_id = ActivityImplementationId::new("http-fetch").expect("implementation id");
    let profile_id = ProfileId::new("server").expect("profile id");
    let module = decoded
        .manifest
        .external_modules
        .get(&module_id)
        .expect("external module");
    let implementation = decoded
        .manifest
        .activity_implementations
        .get(&implementation_id)
        .expect("Activity implementation");
    let profile = decoded.manifest.profiles.get(&profile_id).expect("profile");

    assert_eq!(module.mount.as_str(), "http");
    assert_eq!(
        module.metadata.as_str(),
        "generated/native-http.adapter.json"
    );
    assert_eq!(implementation.module, module_id);
    assert_eq!(implementation.export.as_str(), "http_fetch");
    assert_eq!(
        profile.external_modules.as_slice(),
        std::slice::from_ref(&module_id)
    );
    assert_eq!(profile.activity_bindings.len(), 1);
    assert_eq!(
        profile
            .listen
            .map(crate::manifest::LaunchListenAddress::socket_addr),
        Some(SocketAddr::from_str("[::1]:8080").expect("IPv6 socket"))
    );

    for key in [
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::ExternalModules),
                ManifestPathSegment::ExternalModule(module_id.clone()),
            ],
            ManifestSourceSlot::MapKey,
        ),
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::ExternalModules),
                ManifestPathSegment::ExternalModule(module_id.clone()),
                ManifestPathSegment::ExternalModuleField(ExternalModuleField::MetadataHash),
            ],
            ManifestSourceSlot::ScalarValue,
        ),
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::ActivityImplementations),
                ManifestPathSegment::ActivityImplementation(implementation_id),
                ManifestPathSegment::ActivityImplementationField(
                    ActivityImplementationField::Export,
                ),
            ],
            ManifestSourceSlot::FieldKey,
        ),
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile_id.clone()),
                ManifestPathSegment::ProfileField(ProfileField::ExternalModules),
                ManifestPathSegment::Index(0),
            ],
            ManifestSourceSlot::ArrayElement { index: 0 },
        ),
        source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile_id),
                ManifestPathSegment::ProfileField(ProfileField::ActivityBindings),
                ManifestPathSegment::ActivityBinding(0),
                ManifestPathSegment::ActivityBindingField(ActivityBindingField::Implementation),
            ],
            ManifestSourceSlot::ScalarValue,
        ),
    ] {
        assert_eq!(
            decoded
                .source_map
                .get(&key)
                .expect("generated source-map entry")
                .source(),
            source_document.identity()
        );
    }

    let module_map_key = decoded
        .source_map
        .get(&source_key(
            [
                ManifestPathSegment::Root(ManifestRootField::ExternalModules),
                ManifestPathSegment::ExternalModule(module_id),
            ],
            ManifestSourceSlot::MapKey,
        ))
        .expect("module map key");
    assert_eq!(
        module_map_key.range(),
        range_of(source, "\"native-http\"", 0)
    );
}

#[test]
fn quoted_and_unquoted_profile_ids_collide_at_raw_map_key_ranges() {
    let source = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.\"dev\"]\n",
    );
    let report = decode(document(&source)).expect_err("duplicate profile");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::DuplicateMapId)
        .expect("duplicate map ID");

    assert_eq!(
        diagnostic.primary().range(),
        range_of(&source, "\"dev\"", 0)
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        diagnostic.related()[0].span().range(),
        range_of(&source, "dev", 0)
    );
}

#[test]
fn every_typed_root_map_rejects_quoted_and_unquoted_duplicate_ids() {
    for (label, source, first, later, later_occurrence) in [
        (
            "content unit",
            minimal(
                "[content-units.chapter]\nroots = [\"@flow.chapter\"]\n[content-units.\"chapter\"]\n",
            ),
            "chapter",
            "\"chapter\"",
            0,
        ),
        (
            "external module",
            minimal(
                "[external-modules.native]\nmount = \"native\"\nmetadata = \"generated/native.json\"\nmetadata-hash = \"blake3:1111111111111111111111111111111111111111111111111111111111111111\"\nvisibility = \"package\"\ndemand = \"required\"\n[external-modules.\"native\"]\n",
            ),
            "native",
            "\"native\"",
            1,
        ),
        (
            "Activity implementation",
            minimal(
                "[activity-implementations.player]\nmodule = \"native\"\nexport = \"player\"\n[activity-implementations.\"player\"]\n",
            ),
            "player",
            "\"player\"",
            1,
        ),
    ] {
        let report = decode(document(&source)).expect_err("duplicate map ID must fail");
        let diagnostic = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::DuplicateMapId)
            .unwrap_or_else(|| panic!("missing duplicate map diagnostic for {label}"));
        assert_eq!(
            diagnostic.primary().range(),
            range_of(&source, later, later_occurrence),
            "later raw key for {label}"
        );
        assert_eq!(diagnostic.related().len(), 1, "{label}");
        assert_eq!(
            diagnostic.related()[0].span().range(),
            range_of(&source, first, 0),
            "first raw key for {label}"
        );
    }
}

#[test]
fn nested_table_and_field_duplicates_survive_inline_table_indexing() {
    let nested_table = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n[profiles.dev.player.viewport]\nfit = \"contain\"\n[profiles.dev.player]\nviewport = { fit = \"cover\" }\n",
    );
    let report = decode(document(&nested_table)).expect_err("duplicate nested table must fail");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::DuplicateTable)
        .expect("duplicate nested table");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&nested_table, "viewport", 1)
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        diagnostic.related()[0].span().range(),
        range_of(&nested_table, "[profiles.dev.player.viewport]", 0)
    );

    let nested_field = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\npure = { backend = \"cpu\" }\n[profiles.dev.pure]\nbackend = \"auto\"\n",
    );
    let report = decode(document(&nested_field)).expect_err("duplicate nested field must fail");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::DuplicateField)
        .expect("duplicate nested field");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&nested_field, "backend", 1)
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        diagnostic.related()[0].span().range(),
        range_of(&nested_field, "backend", 0)
    );
}

#[test]
fn profile_array_duplicates_keep_first_and_later_element_ranges() {
    let duplicate_module = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\nexternal-modules = [\"native\", \"native\"]\n",
    );
    let report =
        decode(document(&duplicate_module)).expect_err("duplicate selected module must fail");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::DuplicateArrayId)
        .expect("duplicate array ID");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&duplicate_module, "\"native\"", 1)
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        diagnostic.related()[0].span().range(),
        range_of(&duplicate_module, "\"native\"", 0)
    );

    let duplicate_binding = minimal(
        "[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\nactivity-bindings = [\n  { activity = \"activity.player\", implementation = \"player-a\" },\n  { activity = \"activity.player\", implementation = \"player-b\" },\n]\n",
    );
    let report =
        decode(document(&duplicate_binding)).expect_err("duplicate Activity binding must fail");
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::DuplicateActivityBinding)
        .expect("duplicate Activity binding");
    assert_eq!(
        diagnostic.primary().range(),
        range_of(&duplicate_binding, "\"activity.player\"", 1)
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        diagnostic.related()[0].span().range(),
        range_of(&duplicate_binding, "\"activity.player\"", 0)
    );
}
