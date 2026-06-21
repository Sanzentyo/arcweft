use crate::id::Identifier;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Lossless, transport-neutral payload accepted by interaction contracts.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum InteractionPayload {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    Text(String),
    Entity(Identifier),
    List(Vec<Self>),
    Map(BTreeMap<String, Self>),
}

impl InteractionPayload {
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::I64(_) => "i64",
            Self::U64(_) => "u64",
            Self::F64(_) => "f64",
            Self::Text(_) => "text",
            Self::Entity(_) => "entity",
            Self::List(_) => "list",
            Self::Map(_) => "map",
        }
    }
}
