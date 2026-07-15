//! Serde decoding routed through checked selector constructors.

use super::{
    ViewStyleCombinator, ViewStylePredicate, ViewStyleSelector, ViewStyleSelectorSequence,
};
use crate::{ViewElementKind, ViewPartName};
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedSelectorSequence {
    relation_to_previous: Option<ViewStyleCombinator>,
    element: Option<ViewElementKind>,
    part: Option<ViewPartName>,
    predicates: Vec<ViewStylePredicate>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedSelector {
    sequences: Vec<ViewStyleSelectorSequence>,
}

impl<'de> Deserialize<'de> for ViewStyleSelectorSequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedSelectorSequence::deserialize(deserializer)?;
        Self::new(
            encoded.relation_to_previous,
            encoded.element,
            encoded.part,
            encoded.predicates,
        )
        .ok_or_else(|| serde::de::Error::custom("Style selector sequence must not be empty"))
    }
}

impl<'de> Deserialize<'de> for ViewStyleSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedSelector::deserialize(deserializer)?;
        Self::new(encoded.sequences).ok_or_else(|| {
            serde::de::Error::custom(
                "Style selector must start without a combinator and relate every later sequence",
            )
        })
    }
}
