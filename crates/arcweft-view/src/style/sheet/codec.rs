//! Serde decoding routed through checked Style model constructors.

use super::{
    ViewStyleDeclaration, ViewStylePatch, ViewStylePatchId, ViewStyleProgram, ViewStyleRule,
    ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId, ViewStyleToken, ViewStyleTokenId,
};
use crate::style::{
    ViewPropertyKind, ViewSpecifiedValue, ViewStyleAssignOp, ViewStyleSelector, ViewStyleValueKind,
};
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedToken {
    id: ViewStyleTokenId,
    value_kind: ViewStyleValueKind,
    value: ViewSpecifiedValue,
    source: ViewStyleSourceId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedDeclaration {
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
    op: ViewStyleAssignOp,
    source: ViewStyleSourceId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedRule {
    selector: ViewStyleSelector,
    declarations: Vec<ViewStyleDeclaration>,
    source_order: u32,
    source: ViewStyleSourceId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedSheet {
    id: ViewStyleSheetId,
    tokens: Vec<ViewStyleToken>,
    rules: Vec<ViewStyleRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedPatch {
    id: ViewStylePatchId,
    declarations: Vec<ViewStyleDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedProgram {
    sheets: Vec<ViewStyleSheet>,
    patches: Vec<ViewStylePatch>,
}

impl<'de> Deserialize<'de> for ViewStyleToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedToken::deserialize(deserializer)?;
        Self::new(
            encoded.id,
            encoded.value_kind,
            encoded.value,
            encoded.source,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ViewStyleDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedDeclaration::deserialize(deserializer)?;
        Self::new(encoded.property, encoded.value, encoded.op, encoded.source)
            .map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ViewStyleRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedRule::deserialize(deserializer)?;
        Self::new(
            encoded.selector,
            encoded.declarations,
            encoded.source_order,
            encoded.source,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ViewStyleSheet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedSheet::deserialize(deserializer)?;
        Self::from_canonical_parts(encoded.id, encoded.tokens, encoded.rules)
            .map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ViewStylePatch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedPatch::deserialize(deserializer)?;
        Ok(Self::new(encoded.id, encoded.declarations))
    }
}

impl<'de> Deserialize<'de> for ViewStyleProgram {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedProgram::deserialize(deserializer)?;
        Self::from_canonical_parts(encoded.sheets, encoded.patches)
            .map_err(serde::de::Error::custom)
    }
}
