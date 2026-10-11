//! Exact IEEE storage projection for dense float sequence serde.

macro_rules! float_storage {
    ($module:ident, $scalar:ident, $bits:ty) => {
        pub(crate) mod $module {
            use crate::value::DenseSeqStorage;
            use serde::de::Error as _;
            use serde::ser::{SerializeSeq, SerializeStruct};
            use serde::{Deserialize, Deserializer, Serialize, Serializer};

            struct Bits<'a>(&'a [std::primitive::$scalar]);

            impl Serialize for Bits<'_> {
                fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                    let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
                    for value in self.0 {
                        sequence.serialize_element(&value.to_bits())?;
                    }
                    sequence.end()
                }
            }

            pub(crate) fn serialize<S: Serializer>(
                storage: &DenseSeqStorage<std::primitive::$scalar>,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                let mut image = serializer.serialize_struct("DenseSeqStorage", 1)?;
                image.serialize_field("values", &Bits(storage.as_slice()))?;
                image.end()
            }

            pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<DenseSeqStorage<std::primitive::$scalar>, D::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Wire {
                    values: Vec<$bits>,
                }
                let wire = Wire::deserialize(deserializer)?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(wire.values.len())
                    .map_err(D::Error::custom)?;
                values.extend(
                    wire.values
                        .into_iter()
                        .map(std::primitive::$scalar::from_bits),
                );
                Ok(DenseSeqStorage::new(values))
            }
        }
    };
}

float_storage!(width32, f32, u32);
float_storage!(width64, f64, u64);
