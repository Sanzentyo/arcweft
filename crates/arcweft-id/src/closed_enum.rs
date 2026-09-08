//! Neutral identities for Arcweft-owned closed enum domains and values.

use crate::canonical::append_canonical_varint;
use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

pub const CLOSED_ENUM_IDENTITY_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ClosedEnumOwnerId {
    RichText = 0,
    Fx = 1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ClosedEnumDomainId {
    owner: ClosedEnumOwnerId,
    local_tag: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ClosedEnumValueId {
    domain: ClosedEnumDomainId,
    variant: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosedEnumMemberDescriptor {
    tag: u16,
    source_name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosedEnumDomainDescriptor {
    domain: ClosedEnumDomainId,
    diagnostic_label: &'static str,
    members: &'static [ClosedEnumMemberDescriptor],
}

impl ClosedEnumMemberDescriptor {
    pub const fn new(tag: u16, source_name: &'static str) -> Self {
        Self { tag, source_name }
    }
    pub const fn tag(self) -> u16 {
        self.tag
    }
    pub const fn source_name(self) -> &'static str {
        self.source_name
    }
}

impl ClosedEnumDomainDescriptor {
    pub const fn new(
        domain: ClosedEnumDomainId,
        diagnostic_label: &'static str,
        members: &'static [ClosedEnumMemberDescriptor],
    ) -> Self {
        Self {
            domain,
            diagnostic_label,
            members,
        }
    }
    pub const fn domain(self) -> ClosedEnumDomainId {
        self.domain
    }
    pub const fn version(self) -> u8 {
        CLOSED_ENUM_IDENTITY_VERSION
    }
    pub const fn diagnostic_label(self) -> &'static str {
        self.diagnostic_label
    }
    pub const fn members(self) -> &'static [ClosedEnumMemberDescriptor] {
        self.members
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ClosedEnumIdentityError {
    #[error("unknown closed-enum owner tag {0}")]
    UnknownOwner(u8),
}

impl ClosedEnumOwnerId {
    pub const ALL: [Self; 2] = [Self::RichText, Self::Fx];
    pub const fn tag(self) -> u8 {
        self as u8
    }
    pub const fn from_tag(tag: u8) -> Result<Self, ClosedEnumIdentityError> {
        match tag {
            0 => Ok(Self::RichText),
            1 => Ok(Self::Fx),
            value => Err(ClosedEnumIdentityError::UnknownOwner(value)),
        }
    }
    pub fn canonical_bytes(self) -> Vec<u8> {
        let mut bytes = vec![CLOSED_ENUM_IDENTITY_VERSION];
        append_canonical_varint(&mut bytes, u64::from(self.tag()));
        bytes
    }
}

impl ClosedEnumDomainId {
    pub const fn new(owner: ClosedEnumOwnerId, local_tag: u8) -> Self {
        Self { owner, local_tag }
    }
    pub const fn owner(self) -> ClosedEnumOwnerId {
        self.owner
    }
    pub const fn local_tag(self) -> u8 {
        self.local_tag
    }
    pub fn canonical_bytes(self) -> Vec<u8> {
        let mut bytes = self.owner.canonical_bytes();
        append_canonical_varint(&mut bytes, u64::from(self.local_tag));
        bytes
    }
}

impl ClosedEnumValueId {
    pub const fn new(domain: ClosedEnumDomainId, variant: u16) -> Self {
        Self { domain, variant }
    }
    pub const fn domain(self) -> ClosedEnumDomainId {
        self.domain
    }
    pub const fn variant(self) -> u16 {
        self.variant
    }
    pub fn canonical_bytes(self) -> Vec<u8> {
        let mut bytes = self.domain.canonical_bytes();
        append_canonical_varint(&mut bytes, u64::from(self.variant));
        bytes
    }
}

impl Serialize for ClosedEnumOwnerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.tag())
    }
}

impl<'de> Deserialize<'de> for ClosedEnumOwnerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl de::Visitor<'_> for Visitor {
            type Value = ClosedEnumOwnerId;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("closed-enum owner tag 0 or 1")
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let tag = u8::try_from(value)
                    .map_err(|_| E::custom("closed-enum owner tag exceeds u8"))?;
                ClosedEnumOwnerId::from_tag(tag).map_err(E::custom)
            }
        }
        deserializer.deserialize_u8(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_wire_round_trips_and_rejects_unknown_tags() {
        assert_eq!(serde_json::to_string(&ClosedEnumOwnerId::Fx).unwrap(), "1");
        assert_eq!(
            serde_json::from_str::<ClosedEnumOwnerId>("0").unwrap(),
            ClosedEnumOwnerId::RichText
        );
        assert!(serde_json::from_str::<ClosedEnumOwnerId>("2").is_err());
        assert!(serde_json::from_str::<ClosedEnumOwnerId>("256").is_err());
    }

    #[test]
    fn domain_and_value_round_trip_without_public_fields() {
        let domain = ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, 4);
        let value = ClosedEnumValueId::new(domain, 300);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            serde_json::from_slice::<ClosedEnumValueId>(&bytes).unwrap(),
            value
        );
        assert_eq!(value.canonical_bytes(), [1, 1, 4, 0xac, 0x02]);
    }
}
