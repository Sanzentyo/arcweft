//! Immutable owner-neutral closed-enum registry for one semantic world.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_id::closed_enum::{ClosedEnumDomainDescriptor, ClosedEnumDomainId, ClosedEnumValueId};
use thiserror::Error;

const DOMAIN_MARKER: &[u8] = b"arcweft.closed-enum-domain";
const CATALOG_DOMAIN: &[u8] = b"arcweft.registered-closed-enum-domains";
pub const CLOSED_ENUM_CATALOG_VERSION: u8 = 1;
pub const CLOSED_ENUM_MAX_DOMAINS: usize = 256;
pub const CLOSED_ENUM_MAX_MEMBERS_PER_DOMAIN: usize = u16::MAX as usize + 1;
pub const CLOSED_ENUM_MAX_MEMBER_NAME_BYTES: usize = 256;
pub const CLOSED_ENUM_MAX_CANONICAL_BYTES: usize = 1 << 20;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClosedEnumCatalogError {
    #[error("closed-enum catalog has unsupported version {actual}, expected {expected}")]
    UnsupportedVersion { actual: u8, expected: u8 },
    #[error("closed-enum catalog has {actual} domains, exceeding the limit of {limit}")]
    TooManyDomains { actual: usize, limit: usize },
    #[error("closed-enum domain {domain:?} is duplicated")]
    DuplicateDomain { domain: ClosedEnumDomainId },
    #[error("closed-enum domain {domain:?} has no members")]
    EmptyDomain { domain: ClosedEnumDomainId },
    #[error("closed-enum domain {domain:?} has {actual} members, exceeding the limit of {limit}")]
    TooManyMembers {
        domain: ClosedEnumDomainId,
        actual: usize,
        limit: usize,
    },
    #[error("closed-enum domain {domain:?} member {actual} is not contiguous at {expected}")]
    NonContiguousMemberTag {
        domain: ClosedEnumDomainId,
        expected: u16,
        actual: u16,
    },
    #[error("closed-enum domain {domain:?} member name `{name}` is not a canonical identifier")]
    InvalidMemberName {
        domain: ClosedEnumDomainId,
        name: String,
    },
    #[error("closed-enum domain {domain:?} member name `{name}` is duplicated")]
    DuplicateMemberName {
        domain: ClosedEnumDomainId,
        name: String,
    },
    #[error("closed-enum domain {domain:?} member name `{name}` exceeds the {limit}-byte limit")]
    MemberNameLimit {
        domain: ClosedEnumDomainId,
        name: String,
        limit: usize,
    },
    #[error("closed-enum catalog canonical bytes exceed the limit of {limit}")]
    CanonicalBytesLimit { limit: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredClosedEnumMember {
    value: ClosedEnumValueId,
    source_name: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredClosedEnumDomain {
    id: ClosedEnumDomainId,
    diagnostic_label: Arc<str>,
    members: Arc<[RegisteredClosedEnumMember]>,
    canonical_bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredClosedEnumDomainCatalog {
    domains: Arc<[RegisteredClosedEnumDomain]>,
    by_id: BTreeMap<ClosedEnumDomainId, usize>,
    digest: [u8; 32],
}

impl RegisteredClosedEnumDomainCatalog {
    /// Seals only owner-issued version-one descriptors.
    pub fn try_from_owner_descriptors(
        descriptors: impl IntoIterator<Item = ClosedEnumDomainDescriptor>,
    ) -> Result<Self, ClosedEnumCatalogError> {
        let mut domains = Vec::new();
        let mut seen = BTreeSet::new();
        for descriptor in descriptors {
            if domains.len() >= CLOSED_ENUM_MAX_DOMAINS {
                return Err(ClosedEnumCatalogError::TooManyDomains {
                    actual: CLOSED_ENUM_MAX_DOMAINS + 1,
                    limit: CLOSED_ENUM_MAX_DOMAINS,
                });
            }
            let domain = Self::validate_domain(descriptor)?;
            if !seen.insert(domain.id) {
                return Err(ClosedEnumCatalogError::DuplicateDomain { domain: domain.id });
            }
            domains.push(domain);
        }
        domains.sort_by_key(|domain| domain.id);
        let mut by_id = BTreeMap::new();
        let mut canonical = CappedBytes::new(CLOSED_ENUM_MAX_CANONICAL_BYTES);
        canonical
            .varint(u64::from(CLOSED_ENUM_CATALOG_VERSION))
            .map_err(|_| canonical_bytes_error())?;
        canonical
            .varint(canonical_len(domains.len())?)
            .map_err(|_| canonical_bytes_error())?;
        for (index, domain) in domains.iter().enumerate() {
            by_id.insert(domain.id, index);
            canonical
                .varint(canonical_len(domain.canonical_bytes.len())?)
                .map_err(|_| canonical_bytes_error())?;
            canonical
                .bytes(&domain.canonical_bytes)
                .map_err(|_| canonical_bytes_error())?;
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(CATALOG_DOMAIN);
        hasher.update(canonical.as_slice());
        Ok(Self {
            domains: domains.into_boxed_slice().into(),
            by_id,
            digest: *hasher.finalize().as_bytes(),
        })
    }

    fn validate_domain(
        descriptor: ClosedEnumDomainDescriptor,
    ) -> Result<RegisteredClosedEnumDomain, ClosedEnumCatalogError> {
        if descriptor.version() != CLOSED_ENUM_CATALOG_VERSION {
            return Err(ClosedEnumCatalogError::UnsupportedVersion {
                actual: descriptor.version(),
                expected: CLOSED_ENUM_CATALOG_VERSION,
            });
        }
        let id = descriptor.domain();
        let source_members = descriptor.members();
        if source_members.is_empty() {
            return Err(ClosedEnumCatalogError::EmptyDomain { domain: id });
        }
        if source_members.len() > CLOSED_ENUM_MAX_MEMBERS_PER_DOMAIN {
            return Err(ClosedEnumCatalogError::TooManyMembers {
                domain: id,
                actual: source_members.len(),
                limit: CLOSED_ENUM_MAX_MEMBERS_PER_DOMAIN,
            });
        }
        let mut names = BTreeSet::new();
        let mut members = Vec::with_capacity(source_members.len());
        for (index, member) in source_members.iter().enumerate() {
            let Some(expected) = u16::try_from(index).ok() else {
                return Err(ClosedEnumCatalogError::TooManyMembers {
                    domain: id,
                    actual: source_members.len(),
                    limit: CLOSED_ENUM_MAX_MEMBERS_PER_DOMAIN,
                });
            };
            if member.tag() != expected {
                return Err(ClosedEnumCatalogError::NonContiguousMemberTag {
                    domain: id,
                    expected,
                    actual: member.tag(),
                });
            }
            let name = member.source_name();
            if name.len() > CLOSED_ENUM_MAX_MEMBER_NAME_BYTES {
                return Err(ClosedEnumCatalogError::MemberNameLimit {
                    domain: id,
                    name: name.to_owned(),
                    limit: CLOSED_ENUM_MAX_MEMBER_NAME_BYTES,
                });
            }
            if !canonical_identifier(name) {
                return Err(ClosedEnumCatalogError::InvalidMemberName {
                    domain: id,
                    name: name.to_owned(),
                });
            }
            if !names.insert(name) {
                return Err(ClosedEnumCatalogError::DuplicateMemberName {
                    domain: id,
                    name: name.to_owned(),
                });
            }
            members.push(RegisteredClosedEnumMember {
                value: ClosedEnumValueId::new(id, member.tag()),
                source_name: Arc::from(name),
            });
        }
        let mut canonical = CappedBytes::new(CLOSED_ENUM_MAX_CANONICAL_BYTES);
        canonical
            .varint(canonical_len(DOMAIN_MARKER.len())?)
            .map_err(|_| ClosedEnumCatalogError::CanonicalBytesLimit {
                limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
            })?;
        canonical.bytes(DOMAIN_MARKER).map_err(|_| {
            ClosedEnumCatalogError::CanonicalBytesLimit {
                limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
            }
        })?;
        canonical
            .varint(u64::from(CLOSED_ENUM_CATALOG_VERSION))
            .map_err(|_| ClosedEnumCatalogError::CanonicalBytesLimit {
                limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
            })?;
        canonical.varint(u64::from(id.owner().tag())).map_err(|_| {
            ClosedEnumCatalogError::CanonicalBytesLimit {
                limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
            }
        })?;
        canonical.varint(u64::from(id.local_tag())).map_err(|_| {
            ClosedEnumCatalogError::CanonicalBytesLimit {
                limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
            }
        })?;
        canonical
            .varint(canonical_len(source_members.len())?)
            .map_err(|_| ClosedEnumCatalogError::CanonicalBytesLimit {
                limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
            })?;
        for member in source_members {
            canonical.varint(u64::from(member.tag())).map_err(|_| {
                ClosedEnumCatalogError::CanonicalBytesLimit {
                    limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
                }
            })?;
            canonical
                .varint(canonical_len(member.source_name().len())?)
                .map_err(|_| ClosedEnumCatalogError::CanonicalBytesLimit {
                    limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
                })?;
            canonical
                .bytes(member.source_name().as_bytes())
                .map_err(|_| ClosedEnumCatalogError::CanonicalBytesLimit {
                    limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
                })?;
        }
        Ok(RegisteredClosedEnumDomain {
            id,
            diagnostic_label: Arc::from(descriptor.diagnostic_label()),
            members: members.into_boxed_slice().into(),
            canonical_bytes: canonical.into_arc(),
        })
    }

    pub fn domain(&self, id: ClosedEnumDomainId) -> Option<&RegisteredClosedEnumDomain> {
        self.by_id.get(&id).map(|index| &self.domains[*index])
    }

    pub fn resolve(
        &self,
        domain: ClosedEnumDomainId,
        source_name: &str,
    ) -> Option<ClosedEnumValueId> {
        self.domain(domain)?
            .members
            .iter()
            .find_map(|member| (member.source_name.as_ref() == source_name).then_some(member.value))
    }

    pub fn member_name(&self, value: ClosedEnumValueId) -> Option<&str> {
        self.domain(value.domain())?
            .members
            .get(usize::from(value.variant()))
            .filter(|member| member.value == value)
            .map(|member| member.source_name.as_ref())
    }

    pub fn contains(&self, value: ClosedEnumValueId) -> bool {
        self.member_name(value).is_some()
    }

    pub fn domains(&self) -> &[RegisteredClosedEnumDomain] {
        &self.domains
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

impl RegisteredClosedEnumDomain {
    pub const fn id(&self) -> ClosedEnumDomainId {
        self.id
    }

    pub fn diagnostic_label(&self) -> &str {
        &self.diagnostic_label
    }

    pub fn members(&self) -> &[RegisteredClosedEnumMember] {
        &self.members
    }
}

impl RegisteredClosedEnumMember {
    pub const fn value(&self) -> ClosedEnumValueId {
        self.value
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }
}

struct CappedBytes {
    bytes: Vec<u8>,
    limit: usize,
}

impl CappedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), ()> {
        let next = self.bytes.len().checked_add(value.len()).ok_or(())?;
        if next > self.limit {
            return Err(());
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn varint(&mut self, value: u64) -> Result<(), ()> {
        let mut encoded = [0u8; 10];
        let mut index = 0usize;
        let mut value = value;
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            encoded[index] = byte;
            index += 1;
            if value == 0 {
                return self.bytes(&encoded[..index]);
            }
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn into_arc(self) -> Arc<[u8]> {
        self.bytes.into_boxed_slice().into()
    }
}

fn canonical_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
}

fn canonical_bytes_error() -> ClosedEnumCatalogError {
    ClosedEnumCatalogError::CanonicalBytesLimit {
        limit: CLOSED_ENUM_MAX_CANONICAL_BYTES,
    }
}

fn canonical_len(value: usize) -> Result<u64, ClosedEnumCatalogError> {
    u32::try_from(value)
        .map(u64::from)
        .map_err(|_| canonical_bytes_error())
}

/// Owner aggregate used by registration. Member names and tags come from the
/// presentation owner descriptors; sema only seals and indexes them.
pub(crate) fn production_owner_descriptors() -> impl Iterator<Item = ClosedEnumDomainDescriptor> {
    arcweft_presentation::rich_text::RICH_TEXT_CLOSED_ENUM_DOMAINS
        .iter()
        .copied()
        .chain(
            arcweft_presentation::fx::FX_CLOSED_ENUM_DOMAINS
                .iter()
                .copied(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::closed_enum::{
        ClosedEnumDomainDescriptor, ClosedEnumMemberDescriptor, ClosedEnumOwnerId,
    };

    #[test]
    fn production_catalog_resolves_owner_members_deterministically() {
        let first = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors(
            production_owner_descriptors(),
        )
        .expect("owner descriptors are valid");
        let second = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors(
            production_owner_descriptors(),
        )
        .expect("owner descriptors are deterministic");
        assert_eq!(first.digest(), second.digest());
        let value = arcweft_presentation::fx::FxTarget::Content.value_id();
        assert_eq!(first.resolve(value.domain(), "content"), Some(value));
        assert!(first.contains(value));
    }

    #[test]
    fn malformed_owner_descriptors_fail_closed() {
        static MEMBERS: [ClosedEnumMemberDescriptor; 2] = [
            ClosedEnumMemberDescriptor::new(0, "same"),
            ClosedEnumMemberDescriptor::new(2, "same"),
        ];
        let domain = ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, 200);
        let error = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors([
            ClosedEnumDomainDescriptor::new(domain, "test", &MEMBERS),
        ])
        .expect_err("non-contiguous descriptors must fail");
        assert!(matches!(
            error,
            ClosedEnumCatalogError::NonContiguousMemberTag { .. }
        ));
    }

    #[test]
    fn duplicate_domains_and_members_are_rejected() {
        static ONE_MEMBER: [ClosedEnumMemberDescriptor; 1] =
            [ClosedEnumMemberDescriptor::new(0, "same")];
        let domain = ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, 202);
        let duplicate_domain = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors([
            ClosedEnumDomainDescriptor::new(domain, "test", &ONE_MEMBER),
            ClosedEnumDomainDescriptor::new(domain, "test", &ONE_MEMBER),
        ])
        .expect_err("duplicate domains must fail");
        assert!(matches!(
            duplicate_domain,
            ClosedEnumCatalogError::DuplicateDomain { domain: actual } if actual == domain
        ));

        static DUPLICATE_MEMBERS: [ClosedEnumMemberDescriptor; 2] = [
            ClosedEnumMemberDescriptor::new(0, "same"),
            ClosedEnumMemberDescriptor::new(1, "same"),
        ];
        let duplicate_member = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors([
            ClosedEnumDomainDescriptor::new(domain, "test", &DUPLICATE_MEMBERS),
        ])
        .expect_err("duplicate member names must fail");
        assert!(matches!(
            duplicate_member,
            ClosedEnumCatalogError::DuplicateMemberName { domain: actual, .. }
                if actual == domain
        ));
    }

    #[test]
    fn unknown_domain_and_member_values_fail_closed() {
        static MEMBERS: [ClosedEnumMemberDescriptor; 1] =
            [ClosedEnumMemberDescriptor::new(0, "known")];
        let domain = ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, 203);
        let catalog = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors([
            ClosedEnumDomainDescriptor::new(domain, "test", &MEMBERS),
        ])
        .expect("descriptor is valid");
        assert_eq!(catalog.resolve(domain, "unknown"), None);
        assert_eq!(
            catalog.resolve(ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, 204), "known"),
            None
        );
        assert!(!catalog.contains(ClosedEnumValueId::new(domain, 1)));
    }

    #[test]
    fn canonical_domain_bytes_use_the_versioned_owner_neutral_prefix() {
        static MEMBERS: [ClosedEnumMemberDescriptor; 1] = [ClosedEnumMemberDescriptor::new(0, "x")];
        let descriptor = ClosedEnumDomainDescriptor::new(
            ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, 7),
            "test",
            &MEMBERS,
        );
        let catalog = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors([descriptor])
            .expect("descriptor is valid");

        let mut expected = vec![0x1a];
        expected.extend_from_slice(b"arcweft.closed-enum-domain");
        expected.extend_from_slice(&[1, 1, 7, 1, 0, 1, b'x']);
        assert_eq!(catalog.domains()[0].canonical_bytes.as_ref(), expected);
    }

    #[test]
    fn canonical_byte_budget_rejects_one_over_without_partial_append() {
        let mut bytes = CappedBytes::new(4);
        bytes.bytes(&[0, 1, 2, 3]).expect("exact budget fits");
        assert!(bytes.bytes(&[4]).is_err());
        assert_eq!(bytes.as_slice(), &[0, 1, 2, 3]);
    }

    #[test]
    fn member_name_budget_rejects_one_over() {
        let name: &'static str = Box::leak(
            "a".repeat(CLOSED_ENUM_MAX_MEMBER_NAME_BYTES + 1)
                .into_boxed_str(),
        );
        let members: &'static [ClosedEnumMemberDescriptor] =
            Box::leak(Box::new([ClosedEnumMemberDescriptor::new(0, name)]));
        let descriptor = ClosedEnumDomainDescriptor::new(
            ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, 201),
            "test",
            members,
        );

        let error = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors([descriptor])
            .expect_err("one byte over the member-name budget must fail");
        assert!(matches!(
            error,
            ClosedEnumCatalogError::MemberNameLimit {
                limit: CLOSED_ENUM_MAX_MEMBER_NAME_BYTES,
                ..
            }
        ));
    }

    #[test]
    fn large_descriptor_iterator_rejects_atomically_at_domain_limit() {
        static MEMBERS: [ClosedEnumMemberDescriptor; 1] = [ClosedEnumMemberDescriptor::new(0, "x")];
        let descriptors = (0..=CLOSED_ENUM_MAX_DOMAINS).map(|index| {
            ClosedEnumDomainDescriptor::new(
                ClosedEnumDomainId::new(
                    ClosedEnumOwnerId::Fx,
                    u8::try_from(index % 256).expect("reduced index fits u8"),
                ),
                "test",
                &MEMBERS,
            )
        });

        let error = RegisteredClosedEnumDomainCatalog::try_from_owner_descriptors(descriptors)
            .expect_err("the iterator must be bounded before collecting another domain");
        assert_eq!(
            error,
            ClosedEnumCatalogError::TooManyDomains {
                actual: CLOSED_ENUM_MAX_DOMAINS + 1,
                limit: CLOSED_ENUM_MAX_DOMAINS,
            }
        );
    }
}
