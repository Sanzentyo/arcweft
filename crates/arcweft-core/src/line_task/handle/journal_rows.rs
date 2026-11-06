//! JSON object keys for activation-owned lease and command journals.
//!
//! The row retains the complete typed identity. Its object key is the canonical
//! coordinate within that activation, and decoding checks the two agree.

use super::{MAX_LINE_COMMAND_HISTORY, MAX_LINE_LIVE_HANDLES, RuntimeHandleLease};
use crate::presentation::{RuntimeLineCommandId, RuntimeLineHostCommand};
use crate::runtime_id::RuntimeLineHandleToken;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};
use std::{fmt, marker::PhantomData};

pub(super) trait Row {
    type Id: Clone + Ord;
    const LIMIT: usize;

    fn id(&self) -> &Self::Id;
    fn coordinate(&self) -> String;
}

impl Row for RuntimeHandleLease {
    type Id = RuntimeLineHandleToken;
    const LIMIT: usize = MAX_LINE_LIVE_HANDLES;

    fn id(&self) -> &Self::Id {
        self.token()
    }

    fn coordinate(&self) -> String {
        let token = self.token();
        format!("{}/{}", token.site(), token.issuance())
    }
}

impl Row for RuntimeLineHostCommand {
    type Id = RuntimeLineCommandId;
    const LIMIT: usize = MAX_LINE_COMMAND_HISTORY;

    fn id(&self) -> &Self::Id {
        self.command()
    }

    fn coordinate(&self) -> String {
        self.command().sequence().to_string()
    }
}

pub(super) fn serialize<R: Row + Serialize, S: Serializer>(
    rows: &BTreeMap<R::Id, R>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if rows.len() > R::LIMIT {
        return Err(S::Error::custom("line journal exceeds its row limit"));
    }
    let mut coordinates = BTreeSet::new();
    let mut map = serializer.serialize_map(Some(rows.len()))?;
    for (id, row) in rows {
        let coordinate = row.coordinate();
        if id != row.id() || !coordinates.insert(coordinate.clone()) {
            return Err(S::Error::custom(
                "line journal row identity differs from its key",
            ));
        }
        map.serialize_entry(&coordinate, row)?;
    }
    map.end()
}

pub(super) fn deserialize<'de, R: Row + Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<R::Id, R>, D::Error> {
    struct Journal<R>(PhantomData<R>);

    impl<'de, R: Row + Deserialize<'de>> Visitor<'de> for Journal<R> {
        type Value = BTreeMap<R::Id, R>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an activation-owned journal with exact row coordinates")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut rows = BTreeMap::new();
            let mut coordinates = BTreeSet::new();
            while let Some(coordinate) = map.next_key::<String>()? {
                if rows.len() == R::LIMIT || !coordinates.insert(coordinate.clone()) {
                    return Err(A::Error::custom(
                        "line journal exceeds its row limit or repeats a key",
                    ));
                }
                let row = map.next_value::<R>()?;
                if coordinate != row.coordinate() || rows.insert(row.id().clone(), row).is_some() {
                    return Err(A::Error::custom(
                        "line journal row identity differs from its key",
                    ));
                }
            }
            Ok(rows)
        }
    }

    deserializer.deserialize_map(Journal(PhantomData))
}
