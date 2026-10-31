//! Complex bank identities use sorted rows, with duplicate rejection before
//! decoding the next owned body. The enclosing save decoder owns byte limits.

use crate::runtime_id::RuntimePresentationBankId;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, marker::PhantomData};

pub(super) fn serialize<S: serde::Serializer, L: Serialize>(
    banks: &BTreeMap<RuntimePresentationBankId, L>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut rows = serializer.serialize_seq(Some(banks.len()))?;
    for row in banks {
        rows.serialize_element(&row)?;
    }
    rows.end()
}

pub(super) fn deserialize<'de, D: serde::Deserializer<'de>, L: Deserialize<'de>>(
    deserializer: D,
) -> Result<BTreeMap<RuntimePresentationBankId, L>, D::Error> {
    struct Rows<L>(PhantomData<L>);
    struct Row<'map, L>(&'map mut BTreeMap<RuntimePresentationBankId, L>);
    impl<'de, L: Deserialize<'de>> serde::de::Visitor<'de> for Rows<L> {
        type Value = BTreeMap<RuntimePresentationBankId, L>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("unique presentation bank rows")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut rows: A,
        ) -> Result<Self::Value, A::Error> {
            let mut banks = BTreeMap::new();
            while rows.next_element_seed(Row(&mut banks))?.is_some() {}
            Ok(banks)
        }
    }
    impl<'de, L: Deserialize<'de>> serde::de::DeserializeSeed<'de> for Row<'_, L> {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
            deserializer.deserialize_tuple(2, self)
        }
    }
    impl<'de, L: Deserialize<'de>> serde::de::Visitor<'de> for Row<'_, L> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bank identity followed by its owned state")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut fields: A) -> Result<(), A::Error> {
            let bank = fields
                .next_element()?
                .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
            if self.0.contains_key(&bank) {
                return Err(serde::de::Error::custom(
                    "duplicate presentation bank identity",
                ));
            }
            let state = fields
                .next_element()?
                .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
            if fields.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::invalid_length(3, &self));
            }
            self.0.insert(bank, state);
            Ok(())
        }
    }
    deserializer.deserialize_seq(Rows(PhantomData))
}
