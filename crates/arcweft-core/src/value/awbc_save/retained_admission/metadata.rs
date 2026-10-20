//! Borrowed metadata follows its owning serde grammar under the group's quota.

use super::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};
use crate::entry::schema::value_budget::ResourceByteKind;
use serde::{
    Serialize,
    ser::{self, Serializer},
};

impl RuntimeRetainedValueAdmission {
    /// Bounds metadata through its existing typed serialization grammar before
    /// copying any field. Fixed-width scalars, length-prefixed strings/blobs,
    /// collection counts and variant tags share the retained byte bank.
    /// Runtime carriers use value admission; the codec owns its actual wire limit.
    pub fn validate_metadata<T: ?Sized + Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = value
            .serialize(MetadataAdmission {
                admission: self,
                depth: 0,
                count: 0,
            })
            .and_then(|()| {
                self.work()
                    .failure()
                    .map_or(Ok(()), |error| Err(error.into()))
            });
        if let Err(error) = &result {
            self.retain_failure(error.clone());
        }
        result
    }
}

impl ser::Error for AwbcRuntimeValueSnapshotError {
    fn custom<T: std::fmt::Display>(message: T) -> Self {
        Self::Message {
            message: message.to_string(),
        }
    }
}

struct MetadataAdmission<'a> {
    admission: &'a mut RuntimeRetainedValueAdmission,
    depth: usize,
    count: usize,
}

impl MetadataAdmission<'_> {
    fn scalar(self, width: usize) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.admission.work().charge(self.depth)?;
        self.fixed(width)
    }
    fn fixed(&self, width: usize) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.admission
            .work()
            .encoded_fixed(ResourceByteKind::Metadata, &[0; 16][..width])
            .map_err(Into::into)
    }
    fn finish(self) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.admission
            .work()
            .encoded_count(ResourceByteKind::Metadata, self.count)
            .map_err(Into::into)
    }
    fn container(self, length: Option<usize>) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        self.admission.work().charge(self.depth)?;
        if let Some(length) = length {
            self.admission.work().collection(length)?;
        }
        Ok(self)
    }
    fn member<T: ?Sized + Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.count =
            self.count
                .checked_add(1)
                .ok_or_else(|| AwbcRuntimeValueSnapshotError::Message {
                    message: "retained metadata collection length overflow".to_owned(),
                })?;
        self.admission.work().collection(self.count)?;
        value.serialize(MetadataAdmission {
            admission: self.admission,
            depth: self.depth + 1,
            count: 0,
        })
    }
}

macro_rules! scalar {
    ($($name:ident($ty:ty)),* $(,)?) => { $(fn $name(self, _: $ty) -> Result<(), Self::Error> { self.scalar(std::mem::size_of::<$ty>()) })* };
}

impl Serializer for MetadataAdmission<'_> {
    type Ok = ();
    type Error = AwbcRuntimeValueSnapshotError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;
    scalar!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char)
    );
    fn serialize_str(self, value: &str) -> Result<(), Self::Error> {
        self.admission.work().charge(self.depth)?;
        self.admission.validate_metadata_string(value)
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<(), Self::Error> {
        self.admission.work().charge(self.depth)?;
        self.admission.validate_metadata_collection(value.len())?;
        let mut work = self.admission.work();
        work.encoded_count(ResourceByteKind::Metadata, value.len())?;
        work.encoded_fixed(ResourceByteKind::Metadata, value)?;
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Self::Error> {
        self.scalar(1)
    }
    fn serialize_some<T: ?Sized + Serialize>(mut self, value: &T) -> Result<(), Self::Error> {
        self = self.container(Some(1))?;
        self.member(value)?;
        self.finish()
    }
    fn serialize_unit(self) -> Result<(), Self::Error> {
        self.scalar(0)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Self::Error> {
        self.scalar(0)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<(), Self::Error> {
        self.scalar(4)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        mut self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self = self.container(Some(1))?;
        self.member(value)?;
        self.finish()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        mut self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self = self.container(Some(1))?;
        self.fixed(4)?;
        self.member(value)?;
        self.finish()
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self, Self::Error> {
        self.container(len)
    }
    fn serialize_tuple(self, len: usize) -> Result<Self, Self::Error> {
        self.container(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self, Self::Error> {
        self.container(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        len: usize,
    ) -> Result<Self, Self::Error> {
        let container = self.container(Some(len))?;
        container.fixed(4)?;
        Ok(container)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self, Self::Error> {
        self.container(len)
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<Self, Self::Error> {
        self.container(Some(len))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        len: usize,
    ) -> Result<Self, Self::Error> {
        let container = self.container(Some(len))?;
        container.fixed(4)?;
        Ok(container)
    }
}

macro_rules! sequence {
    ($trait:ident, $method:ident) => {
        impl ser::$trait for MetadataAdmission<'_> {
            type Ok = ();
            type Error = AwbcRuntimeValueSnapshotError;
            fn $method<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
                self.member(value)
            }
            fn end(self) -> Result<(), Self::Error> {
                self.finish()
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);
impl ser::SerializeMap for MetadataAdmission<'_> {
    type Ok = ();
    type Error = AwbcRuntimeValueSnapshotError;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.member(value)
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(MetadataAdmission {
            admission: self.admission,
            depth: self.depth + 1,
            count: 0,
        })
    }
    fn end(self) -> Result<(), Self::Error> {
        self.finish()
    }
}
macro_rules! fields {
    ($trait:ident) => {
        impl ser::$trait for MetadataAdmission<'_> {
            type Ok = ();
            type Error = AwbcRuntimeValueSnapshotError;
            fn serialize_field<T: ?Sized + Serialize>(
                &mut self,
                _: &'static str,
                value: &T,
            ) -> Result<(), Self::Error> {
                self.member(value)
            }
            fn end(self) -> Result<(), Self::Error> {
                self.finish()
            }
        }
    };
}
fields!(SerializeStruct);
fields!(SerializeStructVariant);
