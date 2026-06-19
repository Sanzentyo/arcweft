use crate::ids::PublicId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Deterministic wire value used by Agent host requests and debug records.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AgentValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    String(String),
    Entity(PublicId),
    List(Vec<Self>),
    Map(BTreeMap<String, Self>),
}
