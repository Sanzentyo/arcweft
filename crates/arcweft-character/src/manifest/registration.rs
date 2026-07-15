use std::collections::BTreeMap;

use arcweft_source::{
    MAX_REGISTRATION_SOURCE_BYTES, SourceDocument, SourceDocumentIdentity, SourceRange, SourceSpan,
};

use super::{
    CharacterManifest, CharacterManifestError, CharacterManifestFingerprint,
    diagnostic::{
        CharacterIdentifierDomain, CharacterRegistrationDecodeError, CharacterRuntimeDecodeError,
        JsonStructuralErrorKind,
    },
    limits::{CharacterManifestLimitKind, CharacterManifestLimits},
};
use crate::id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId};

mod structural_json;

use structural_json::{RawJsonError, RawJsonMember, RawJsonNode, RawJsonParser};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterManifestRootField {
    Format,
    Version,
    Character,
    Canvas,
    Anchor,
    DefaultLook,
    Parts,
    Looks,
    Source,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterCanvasField {
    Width,
    Height,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterPointField {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterPartField {
    Id,
    Z,
    Variants,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterVariantField {
    Id,
    Asset,
    Rect,
    Opacity,
    Blend,
    Clipping,
    SourceLayer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterRectField {
    X,
    Y,
    Width,
    Height,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterSourceLayerField {
    Index,
    Group,
    Layer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterLookField {
    Id,
    Select,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterSelectionField {
    Part,
    Variant,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterSourceField {
    Kind,
    FileName,
    Blake3,
    Importer,
    Warnings,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterManifestTokenPath {
    Root(CharacterManifestRootField),
    Canvas(CharacterCanvasField),
    Anchor(CharacterPointField),
    Part {
        part: usize,
        field: CharacterPartField,
    },
    Variant {
        part: usize,
        variant: usize,
        field: CharacterVariantField,
    },
    VariantRect {
        part: usize,
        variant: usize,
        field: CharacterRectField,
    },
    VariantSourceLayer {
        part: usize,
        variant: usize,
        field: CharacterSourceLayerField,
    },
    Look {
        look: usize,
        field: CharacterLookField,
    },
    Selection {
        look: usize,
        selection: usize,
        field: CharacterSelectionField,
    },
    Source(CharacterSourceField),
    SourceWarning {
        warning: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterManifestToken {
    key: SourceSpan,
    value: SourceSpan,
}

impl CharacterManifestToken {
    pub const fn key(&self) -> &SourceSpan {
        &self.key
    }

    pub const fn value(&self) -> &SourceSpan {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterManifestSourceMap {
    document: SourceDocumentIdentity,
    tokens: BTreeMap<CharacterManifestTokenPath, CharacterManifestToken>,
}

impl CharacterManifestSourceMap {
    pub const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }

    pub fn token(&self, path: &CharacterManifestTokenPath) -> Option<&CharacterManifestToken> {
        self.tokens.get(path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedCharacterManifest {
    manifest: CharacterManifest,
    fingerprint: CharacterManifestFingerprint,
    source_map: CharacterManifestSourceMap,
}

impl SourceBackedCharacterManifest {
    pub fn decode_registration_json(
        document: &SourceDocument,
    ) -> Result<Self, CharacterRegistrationDecodeError> {
        let observed = u64::try_from(document.text().len()).unwrap_or(u64::MAX);
        if observed > MAX_REGISTRATION_SOURCE_BYTES {
            return Err(CharacterRegistrationDecodeError::SourceBytesLimit {
                observed,
                maximum: MAX_REGISTRATION_SOURCE_BYTES,
            });
        }
        let raw = RawJsonParser::new(document.text())
            .parse()
            .map_err(|error| error.bind(document))?;
        let source_map = CharacterManifestSourceMap::from_raw(document, &raw)?;
        validate_identifiers(document, &raw)?;
        enforce_manifest_limits(document, &raw)?;
        let manifest =
            serde_json::from_str::<CharacterManifest>(document.text()).map_err(|_| {
                CharacterRegistrationDecodeError::Syntax {
                    kind: JsonStructuralErrorKind::UnexpectedToken,
                    span: bound_span(document, raw.range),
                }
            })?;
        if let Err(error) = manifest.validate() {
            let span = validation_span(&manifest, &source_map, &error)
                .unwrap_or_else(|| bound_span(document, raw.range));
            return Err(CharacterRegistrationDecodeError::Validation { error, span });
        }
        ensure_required_tokens(&manifest, &source_map)?;
        let fingerprint = manifest.semantic_fingerprint_v1();
        Ok(Self {
            manifest,
            fingerprint,
            source_map,
        })
    }

    pub const fn manifest(&self) -> &CharacterManifest {
        &self.manifest
    }

    pub const fn fingerprint(&self) -> CharacterManifestFingerprint {
        self.fingerprint
    }

    pub const fn source_map(&self) -> &CharacterManifestSourceMap {
        &self.source_map
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum JsonObjectPathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JsonObjectPath(Vec<JsonObjectPathSegment>);

impl JsonObjectPath {
    pub fn segments(&self) -> &[JsonObjectPathSegment] {
        &self.0
    }

    fn with_key(&self, key: String) -> Self {
        let mut segments = self.0.clone();
        segments.push(JsonObjectPathSegment::Key(key));
        Self(segments)
    }

    fn with_index(&self, index: usize) -> Self {
        let mut segments = self.0.clone();
        segments.push(JsonObjectPathSegment::Index(index));
        Self(segments)
    }
}

pub(super) fn decode_runtime_json(
    source: &str,
) -> Result<CharacterManifest, CharacterRuntimeDecodeError> {
    let observed = u64::try_from(source.len()).unwrap_or(u64::MAX);
    if observed > MAX_REGISTRATION_SOURCE_BYTES {
        return Err(CharacterRuntimeDecodeError::SourceBytesLimit {
            observed,
            maximum: MAX_REGISTRATION_SOURCE_BYTES,
        });
    }
    let raw = RawJsonParser::new(source)
        .parse()
        .map_err(RawJsonError::into_runtime)?;
    validate_identifiers_runtime(&raw)?;
    enforce_manifest_limits_runtime(&raw)?;
    let manifest = serde_json::from_str::<CharacterManifest>(source).map_err(|_| {
        CharacterRuntimeDecodeError::Syntax {
            kind: JsonStructuralErrorKind::UnexpectedToken,
            range: raw.range,
        }
    })?;
    manifest
        .validate()
        .map_err(CharacterRuntimeDecodeError::Validation)?;
    Ok(manifest)
}

impl CharacterManifestSourceMap {
    fn from_raw(
        document: &SourceDocument,
        root: &RawJsonNode,
    ) -> Result<Self, CharacterRegistrationDecodeError> {
        let mut map = Self {
            document: document.identity().clone(),
            tokens: BTreeMap::new(),
        };
        let Some(object) = root.object() else {
            return Err(CharacterRegistrationDecodeError::Syntax {
                kind: JsonStructuralErrorKind::UnexpectedToken,
                span: bound_span(document, root.range),
            });
        };
        for (field, key) in [
            (CharacterManifestRootField::Format, "format"),
            (CharacterManifestRootField::Version, "version"),
            (CharacterManifestRootField::Character, "character"),
            (CharacterManifestRootField::Canvas, "canvas"),
            (CharacterManifestRootField::Anchor, "anchor"),
            (CharacterManifestRootField::DefaultLook, "default_look"),
            (CharacterManifestRootField::Parts, "parts"),
            (CharacterManifestRootField::Looks, "looks"),
            (CharacterManifestRootField::Source, "source"),
        ] {
            map.record_member(
                document,
                CharacterManifestTokenPath::Root(field),
                member(object, key),
            );
        }
        map.record_object_fields(
            document,
            member(object, "canvas").and_then(|value| value.value.object()),
            &[
                (
                    CharacterManifestTokenPath::Canvas(CharacterCanvasField::Width),
                    "width",
                ),
                (
                    CharacterManifestTokenPath::Canvas(CharacterCanvasField::Height),
                    "height",
                ),
            ],
        );
        map.record_object_fields(
            document,
            member(object, "anchor").and_then(|value| value.value.object()),
            &[
                (
                    CharacterManifestTokenPath::Anchor(CharacterPointField::X),
                    "x",
                ),
                (
                    CharacterManifestTokenPath::Anchor(CharacterPointField::Y),
                    "y",
                ),
            ],
        );
        if let Some(parts) = member(object, "parts").and_then(|value| value.value.array()) {
            for (part_index, part) in parts.iter().enumerate() {
                map.record_part(document, part_index, part);
            }
        }
        if let Some(looks) = member(object, "looks").and_then(|value| value.value.array()) {
            for (look_index, look) in looks.iter().enumerate() {
                map.record_look(document, look_index, look);
            }
        }
        if let Some(source) = member(object, "source").and_then(|value| value.value.object()) {
            map.record_source(document, source);
        }
        Ok(map)
    }

    fn record_part(&mut self, document: &SourceDocument, part: usize, node: &RawJsonNode) {
        let Some(object) = node.object() else { return };
        for (field, key) in [
            (CharacterPartField::Id, "id"),
            (CharacterPartField::Z, "z"),
            (CharacterPartField::Variants, "variants"),
        ] {
            self.record_member(
                document,
                CharacterManifestTokenPath::Part { part, field },
                member(object, key),
            );
        }
        if let Some(variants) = member(object, "variants").and_then(|value| value.value.array()) {
            for (variant, node) in variants.iter().enumerate() {
                self.record_variant(document, part, variant, node);
            }
        }
    }

    fn record_variant(
        &mut self,
        document: &SourceDocument,
        part: usize,
        variant: usize,
        node: &RawJsonNode,
    ) {
        let Some(object) = node.object() else { return };
        for (field, key) in [
            (CharacterVariantField::Id, "id"),
            (CharacterVariantField::Asset, "asset"),
            (CharacterVariantField::Rect, "rect"),
            (CharacterVariantField::Opacity, "opacity"),
            (CharacterVariantField::Blend, "blend"),
            (CharacterVariantField::Clipping, "clipping"),
            (CharacterVariantField::SourceLayer, "source_layer"),
        ] {
            self.record_member(
                document,
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field,
                },
                member(object, key),
            );
        }
        self.record_object_fields(
            document,
            member(object, "rect").and_then(|value| value.value.object()),
            &[
                (
                    CharacterManifestTokenPath::VariantRect {
                        part,
                        variant,
                        field: CharacterRectField::X,
                    },
                    "x",
                ),
                (
                    CharacterManifestTokenPath::VariantRect {
                        part,
                        variant,
                        field: CharacterRectField::Y,
                    },
                    "y",
                ),
                (
                    CharacterManifestTokenPath::VariantRect {
                        part,
                        variant,
                        field: CharacterRectField::Width,
                    },
                    "width",
                ),
                (
                    CharacterManifestTokenPath::VariantRect {
                        part,
                        variant,
                        field: CharacterRectField::Height,
                    },
                    "height",
                ),
            ],
        );
        self.record_object_fields(
            document,
            member(object, "source_layer").and_then(|value| value.value.object()),
            &[
                (
                    CharacterManifestTokenPath::VariantSourceLayer {
                        part,
                        variant,
                        field: CharacterSourceLayerField::Index,
                    },
                    "index",
                ),
                (
                    CharacterManifestTokenPath::VariantSourceLayer {
                        part,
                        variant,
                        field: CharacterSourceLayerField::Group,
                    },
                    "group",
                ),
                (
                    CharacterManifestTokenPath::VariantSourceLayer {
                        part,
                        variant,
                        field: CharacterSourceLayerField::Layer,
                    },
                    "layer",
                ),
            ],
        );
    }

    fn record_look(&mut self, document: &SourceDocument, look: usize, node: &RawJsonNode) {
        let Some(object) = node.object() else { return };
        self.record_member(
            document,
            CharacterManifestTokenPath::Look {
                look,
                field: CharacterLookField::Id,
            },
            member(object, "id"),
        );
        self.record_member(
            document,
            CharacterManifestTokenPath::Look {
                look,
                field: CharacterLookField::Select,
            },
            member(object, "select"),
        );
        if let Some(selections) = member(object, "select").and_then(|value| value.value.array()) {
            for (selection, node) in selections.iter().enumerate() {
                let Some(selection_object) = node.object() else {
                    continue;
                };
                self.record_member(
                    document,
                    CharacterManifestTokenPath::Selection {
                        look,
                        selection,
                        field: CharacterSelectionField::Part,
                    },
                    member(selection_object, "part"),
                );
                self.record_member(
                    document,
                    CharacterManifestTokenPath::Selection {
                        look,
                        selection,
                        field: CharacterSelectionField::Variant,
                    },
                    member(selection_object, "variant"),
                );
            }
        }
    }

    fn record_source(&mut self, document: &SourceDocument, object: &[RawJsonMember]) {
        for (field, key) in [
            (CharacterSourceField::Kind, "kind"),
            (CharacterSourceField::FileName, "file_name"),
            (CharacterSourceField::Blake3, "blake3"),
            (CharacterSourceField::Importer, "importer"),
            (CharacterSourceField::Warnings, "warnings"),
        ] {
            self.record_member(
                document,
                CharacterManifestTokenPath::Source(field),
                member(object, key),
            );
        }
        if let Some(warnings_member) = member(object, "warnings")
            && let Some(warnings) = warnings_member.value.array()
        {
            for (warning, value) in warnings.iter().enumerate() {
                self.tokens.insert(
                    CharacterManifestTokenPath::SourceWarning { warning },
                    CharacterManifestToken {
                        key: bound_span(document, warnings_member.key_range),
                        value: bound_span(document, value.range),
                    },
                );
            }
        }
    }

    fn record_object_fields(
        &mut self,
        document: &SourceDocument,
        object: Option<&[RawJsonMember]>,
        fields: &[(CharacterManifestTokenPath, &'static str)],
    ) {
        let Some(object) = object else { return };
        for (path, key) in fields {
            self.record_member(document, path.clone(), member(object, key));
        }
    }

    fn record_member(
        &mut self,
        document: &SourceDocument,
        path: CharacterManifestTokenPath,
        member: Option<&RawJsonMember>,
    ) {
        let Some(member) = member else { return };
        self.tokens.insert(
            path,
            CharacterManifestToken {
                key: bound_span(document, member.key_range),
                value: bound_span(document, member.value.range),
            },
        );
    }
}

fn ensure_required_tokens(
    manifest: &CharacterManifest,
    source_map: &CharacterManifestSourceMap,
) -> Result<(), CharacterRegistrationDecodeError> {
    let mut required = vec![
        CharacterManifestTokenPath::Root(CharacterManifestRootField::Format),
        CharacterManifestTokenPath::Root(CharacterManifestRootField::Version),
        CharacterManifestTokenPath::Root(CharacterManifestRootField::Character),
        CharacterManifestTokenPath::Root(CharacterManifestRootField::Canvas),
        CharacterManifestTokenPath::Root(CharacterManifestRootField::Anchor),
        CharacterManifestTokenPath::Root(CharacterManifestRootField::DefaultLook),
        CharacterManifestTokenPath::Root(CharacterManifestRootField::Parts),
        CharacterManifestTokenPath::Root(CharacterManifestRootField::Looks),
        CharacterManifestTokenPath::Canvas(CharacterCanvasField::Width),
        CharacterManifestTokenPath::Canvas(CharacterCanvasField::Height),
        CharacterManifestTokenPath::Anchor(CharacterPointField::X),
        CharacterManifestTokenPath::Anchor(CharacterPointField::Y),
    ];
    for (part, value) in manifest.parts().iter().enumerate() {
        required.extend([
            CharacterManifestTokenPath::Part {
                part,
                field: CharacterPartField::Id,
            },
            CharacterManifestTokenPath::Part {
                part,
                field: CharacterPartField::Z,
            },
            CharacterManifestTokenPath::Part {
                part,
                field: CharacterPartField::Variants,
            },
        ]);
        for (variant, _) in value.variants().iter().enumerate() {
            required.extend([
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field: CharacterVariantField::Id,
                },
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field: CharacterVariantField::Asset,
                },
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field: CharacterVariantField::Rect,
                },
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field: CharacterVariantField::Opacity,
                },
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field: CharacterVariantField::Blend,
                },
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field: CharacterVariantField::Clipping,
                },
            ]);
        }
    }
    for (look, value) in manifest.looks().iter().enumerate() {
        required.extend([
            CharacterManifestTokenPath::Look {
                look,
                field: CharacterLookField::Id,
            },
            CharacterManifestTokenPath::Look {
                look,
                field: CharacterLookField::Select,
            },
        ]);
        for (selection, _) in value.selections().iter().enumerate() {
            required.extend([
                CharacterManifestTokenPath::Selection {
                    look,
                    selection,
                    field: CharacterSelectionField::Part,
                },
                CharacterManifestTokenPath::Selection {
                    look,
                    selection,
                    field: CharacterSelectionField::Variant,
                },
            ]);
        }
    }
    for token in required {
        if source_map.token(&token).is_none() {
            return Err(CharacterRegistrationDecodeError::MissingToken {
                token,
                document: source_map.document().clone(),
            });
        }
    }
    Ok(())
}

fn validate_identifiers(
    document: &SourceDocument,
    root: &RawJsonNode,
) -> Result<(), CharacterRegistrationDecodeError> {
    for identifier in identifier_tokens(root) {
        if !identifier.is_valid() {
            return Err(CharacterRegistrationDecodeError::InvalidIdentifier {
                domain: identifier.domain,
                value: identifier.value.to_owned(),
                span: bound_span(document, identifier.range),
            });
        }
    }
    Ok(())
}

fn validate_identifiers_runtime(root: &RawJsonNode) -> Result<(), CharacterRuntimeDecodeError> {
    for identifier in identifier_tokens(root) {
        if !identifier.is_valid() {
            return Err(CharacterRuntimeDecodeError::InvalidIdentifier {
                domain: identifier.domain,
                value: identifier.value.to_owned(),
                range: identifier.range,
            });
        }
    }
    Ok(())
}

struct IdentifierToken<'a> {
    domain: CharacterIdentifierDomain,
    value: &'a str,
    range: SourceRange,
}

impl IdentifierToken<'_> {
    fn is_valid(&self) -> bool {
        match self.domain {
            CharacterIdentifierDomain::Character => CharacterId::try_new(self.value).is_ok(),
            CharacterIdentifierDomain::Part => CharacterPartId::try_new(self.value).is_ok(),
            CharacterIdentifierDomain::Variant => CharacterVariantId::try_new(self.value).is_ok(),
            CharacterIdentifierDomain::Look => CharacterLookId::try_new(self.value).is_ok(),
        }
    }
}

fn identifier_tokens(root: &RawJsonNode) -> Vec<IdentifierToken<'_>> {
    let mut values = Vec::new();
    let Some(object) = root.object() else {
        return values;
    };
    push_identifier(
        &mut values,
        member(object, "character"),
        CharacterIdentifierDomain::Character,
    );
    push_identifier(
        &mut values,
        member(object, "default_look"),
        CharacterIdentifierDomain::Look,
    );
    if let Some(parts) = member(object, "parts").and_then(|value| value.value.array()) {
        for part in parts {
            let Some(part) = part.object() else { continue };
            push_identifier(
                &mut values,
                member(part, "id"),
                CharacterIdentifierDomain::Part,
            );
            if let Some(variants) = member(part, "variants").and_then(|value| value.value.array()) {
                for variant in variants {
                    let Some(variant) = variant.object() else {
                        continue;
                    };
                    push_identifier(
                        &mut values,
                        member(variant, "id"),
                        CharacterIdentifierDomain::Variant,
                    );
                }
            }
        }
    }
    if let Some(looks) = member(object, "looks").and_then(|value| value.value.array()) {
        for look in looks {
            let Some(look) = look.object() else { continue };
            push_identifier(
                &mut values,
                member(look, "id"),
                CharacterIdentifierDomain::Look,
            );
            if let Some(selections) = member(look, "select").and_then(|value| value.value.array()) {
                for selection in selections {
                    let Some(selection) = selection.object() else {
                        continue;
                    };
                    push_identifier(
                        &mut values,
                        member(selection, "part"),
                        CharacterIdentifierDomain::Part,
                    );
                    push_identifier(
                        &mut values,
                        member(selection, "variant"),
                        CharacterIdentifierDomain::Variant,
                    );
                }
            }
        }
    }
    values
}

fn push_identifier<'a>(
    values: &mut Vec<IdentifierToken<'a>>,
    member: Option<&'a RawJsonMember>,
    domain: CharacterIdentifierDomain,
) {
    let Some(member) = member else { return };
    let Some(value) = member.value.string() else {
        return;
    };
    values.push(IdentifierToken {
        domain,
        value,
        range: member.value.range,
    });
}

fn enforce_manifest_limits(
    document: &SourceDocument,
    root: &RawJsonNode,
) -> Result<(), CharacterRegistrationDecodeError> {
    manifest_limit_observations(root)
        .into_iter()
        .try_for_each(|observation| {
            if observation.observed > observation.maximum {
                Err(CharacterRegistrationDecodeError::Limit {
                    kind: observation.kind,
                    observed: observation.observed,
                    maximum: observation.maximum,
                    span: Some(bound_span(document, observation.range)),
                })
            } else {
                Ok(())
            }
        })
}

fn enforce_manifest_limits_runtime(root: &RawJsonNode) -> Result<(), CharacterRuntimeDecodeError> {
    manifest_limit_observations(root)
        .into_iter()
        .try_for_each(|observation| {
            if observation.observed > observation.maximum {
                Err(CharacterRuntimeDecodeError::Limit {
                    kind: observation.kind,
                    observed: observation.observed,
                    maximum: observation.maximum,
                    range: Some(observation.range),
                })
            } else {
                Ok(())
            }
        })
}

struct LimitObservation {
    kind: CharacterManifestLimitKind,
    observed: u64,
    maximum: u64,
    range: SourceRange,
}

fn manifest_limit_observations(root: &RawJsonNode) -> Vec<LimitObservation> {
    let limits = CharacterManifestLimits::PRODUCTION;
    let mut observations = Vec::new();
    let Some(object) = root.object() else {
        return observations;
    };
    let parts_member = member(object, "parts");
    let parts = parts_member
        .and_then(|value| value.value.array())
        .unwrap_or_default();
    observations.push(LimitObservation {
        kind: CharacterManifestLimitKind::Parts,
        observed: u64::try_from(parts.len()).unwrap_or(u64::MAX),
        maximum: limits.parts(),
        range: parts_member.map_or(root.range, |value| value.value.range),
    });
    let mut variant_total = 0_u64;
    for part in parts {
        let variants_member = part.object().and_then(|value| member(value, "variants"));
        let variants = variants_member
            .and_then(|value| value.value.array())
            .unwrap_or_default();
        let count = u64::try_from(variants.len()).unwrap_or(u64::MAX);
        variant_total = variant_total.saturating_add(count);
        observations.push(LimitObservation {
            kind: CharacterManifestLimitKind::VariantsPerPart,
            observed: count,
            maximum: limits.variants_per_part(),
            range: variants_member.map_or(part.range, |value| value.value.range),
        });
    }
    observations.push(LimitObservation {
        kind: CharacterManifestLimitKind::VariantsPerManifest,
        observed: variant_total,
        maximum: limits.variants_per_manifest(),
        range: parts_member.map_or(root.range, |value| value.value.range),
    });
    let looks_member = member(object, "looks");
    let looks = looks_member
        .and_then(|value| value.value.array())
        .unwrap_or_default();
    observations.push(LimitObservation {
        kind: CharacterManifestLimitKind::Looks,
        observed: u64::try_from(looks.len()).unwrap_or(u64::MAX),
        maximum: limits.looks(),
        range: looks_member.map_or(root.range, |value| value.value.range),
    });
    let mut selections = 0_u64;
    for look in looks {
        selections = selections.saturating_add(
            look.object()
                .and_then(|value| member(value, "select"))
                .and_then(|value| value.value.array())
                .map_or(0, |values| u64::try_from(values.len()).unwrap_or(u64::MAX)),
        );
    }
    observations.push(LimitObservation {
        kind: CharacterManifestLimitKind::Selections,
        observed: selections,
        maximum: limits.selections(),
        range: looks_member.map_or(root.range, |value| value.value.range),
    });
    observations
}

fn validation_span(
    manifest: &CharacterManifest,
    source_map: &CharacterManifestSourceMap,
    error: &CharacterManifestError,
) -> Option<SourceSpan> {
    let path =
        match error {
            CharacterManifestError::UnsupportedFormat(_) => {
                CharacterManifestTokenPath::Root(CharacterManifestRootField::Format)
            }
            CharacterManifestError::UnsupportedVersion(_) => {
                CharacterManifestTokenPath::Root(CharacterManifestRootField::Version)
            }
            CharacterManifestError::EmptyCanvas => {
                CharacterManifestTokenPath::Root(CharacterManifestRootField::Canvas)
            }
            CharacterManifestError::MissingParts => {
                CharacterManifestTokenPath::Root(CharacterManifestRootField::Parts)
            }
            CharacterManifestError::MissingLooks => {
                CharacterManifestTokenPath::Root(CharacterManifestRootField::Looks)
            }
            CharacterManifestError::MissingDefaultLook { .. }
            | CharacterManifestError::UnknownLook { .. } => {
                CharacterManifestTokenPath::Root(CharacterManifestRootField::DefaultLook)
            }
            CharacterManifestError::DuplicatePart { part, .. }
            | CharacterManifestError::EmptyPart { part, .. }
            | CharacterManifestError::MissingLookPart { part, .. } => {
                let index = manifest
                    .parts()
                    .iter()
                    .rposition(|value| value.id() == part)?;
                CharacterManifestTokenPath::Part {
                    part: index,
                    field: CharacterPartField::Id,
                }
            }
            CharacterManifestError::DuplicateVariant { part, variant, .. }
            | CharacterManifestError::EmptyVariantRect { part, variant, .. } => {
                let part_index = manifest
                    .parts()
                    .iter()
                    .position(|value| value.id() == part)?;
                let variant_index = manifest.parts()[part_index]
                    .variants()
                    .iter()
                    .rposition(|value| value.id() == variant)?;
                CharacterManifestTokenPath::Variant {
                    part: part_index,
                    variant: variant_index,
                    field: CharacterVariantField::Id,
                }
            }
            CharacterManifestError::DuplicateAssetPath { asset, .. } => {
                let (part, variant) =
                    manifest
                        .parts()
                        .iter()
                        .enumerate()
                        .rev()
                        .find_map(|(part, value)| {
                            value.variants().iter().enumerate().rev().find_map(
                                |(variant, value)| {
                                    (value.asset() == asset).then_some((part, variant))
                                },
                            )
                        })?;
                CharacterManifestTokenPath::Variant {
                    part,
                    variant,
                    field: CharacterVariantField::Asset,
                }
            }
            CharacterManifestError::DuplicateLook { look, .. } => {
                let index = manifest
                    .looks()
                    .iter()
                    .rposition(|value| value.id() == look)?;
                CharacterManifestTokenPath::Look {
                    look: index,
                    field: CharacterLookField::Id,
                }
            }
            CharacterManifestError::DuplicateLookPart { look, part, .. }
            | CharacterManifestError::UnknownLookPart { look, part, .. }
            | CharacterManifestError::UnknownLookVariant { look, part, .. } => {
                let look_index = manifest
                    .looks()
                    .iter()
                    .position(|value| value.id() == look)?;
                let selection_index = manifest.looks()[look_index]
                    .selections()
                    .iter()
                    .rposition(|value| value.part() == part)?;
                CharacterManifestTokenPath::Selection {
                    look: look_index,
                    selection: selection_index,
                    field: CharacterSelectionField::Part,
                }
            }
        };
    source_map.token(&path).map(|token| token.value().clone())
}

fn member<'a>(object: &'a [RawJsonMember], name: &str) -> Option<&'a RawJsonMember> {
    object.iter().find(|member| member.key == name)
}

fn bound_span(document: &SourceDocument, range: SourceRange) -> SourceSpan {
    document
        .span(range)
        .expect("the structural scanner only emits UTF-8 token boundaries")
}

#[cfg(test)]
mod tests;
